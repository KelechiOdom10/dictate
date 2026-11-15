//! dictate — hold a key, speak, get text in whatever app has focus.
//!
//! The pipeline is three contracts, deliberately separate so each can be
//! built and tested alone:
//!
//!   audio::start / audio::stop  ->  Vec<f32>  (16 kHz, mono, in memory)
//!   transcribe::transcribe      ->  String
//!   insert::paste               ->  ()       (into the focused app)
//!
//! Keep them separate. A pipeline that only works end to end is a pipeline
//! you cannot measure a part of.
//!
//! Audio never crosses the IPC boundary. Ten seconds at 16 kHz is 160 000
//! floats, which serialises to megabytes of JSON for no benefit; the frontend
//! receives status and small scalar levels, never raw samples.

mod audio;
mod insert;
mod transcribe;

mod focus;
mod history;
mod models;
mod overlay;
mod problem;
mod settings;
mod surface;

use history::{HistoryList, NewTake, TakeStatus};
use models::{ModelEntry, SpeechModel};
use problem::Problem;
use settings::{HistoryRetention, Settings, ShortcutMode};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Instant,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Idle,
    Recording,
    Transcribing,
}

struct Runtime {
    settings: Settings,
    phase: Phase,
    take_id: u64,
    take_start: Option<TakeStart>,
    capturing_shortcut: bool,
    shortcut_registered: bool,
}

/// What we know at key-down: when the take began and which app had focus.
struct TakeStart {
    started_at_ms: i64,
    app: Option<String>,
}

/// The open store, or the message to show when history is off.
type History = Mutex<Result<history::Store, String>>;

/// Where model weights live (`<app data>/models`) and which are downloading.
struct Models {
    dir: PathBuf,
    downloads: Mutex<models::Downloads>,
}

trait SettingsPlatform {
    fn input_devices(&self) -> Result<Vec<String>, String>;
    fn model_installed(&self, model: SpeechModel) -> bool;
    fn login_enabled(&self) -> Result<bool, String>;
    fn set_login(&self, enabled: bool) -> Result<(), String>;
    fn register(&self, chord: tauri_plugin_global_shortcut::Shortcut) -> Result<(), String>;
    fn unregister(&self, chord: tauri_plugin_global_shortcut::Shortcut) -> Result<(), String>;
    fn persist(&self, settings: &Settings) -> Result<(), String>;
}

impl SettingsPlatform for AppHandle {
    fn input_devices(&self) -> Result<Vec<String>, String> {
        audio::input_devices()
    }
    fn model_installed(&self, model: SpeechModel) -> bool {
        models::path(&self.state::<Models>().dir, model).is_file()
    }
    fn login_enabled(&self) -> Result<bool, String> {
        self.autolaunch().is_enabled().map_err(|e| e.to_string())
    }
    fn set_login(&self, enabled: bool) -> Result<(), String> {
        if enabled {
            self.autolaunch().enable()
        } else {
            self.autolaunch().disable()
        }
        .map_err(|e| e.to_string())
    }
    fn register(&self, chord: tauri_plugin_global_shortcut::Shortcut) -> Result<(), String> {
        self.global_shortcut()
            .register(chord)
            .map_err(|e| e.to_string())
    }
    fn unregister(&self, chord: tauri_plugin_global_shortcut::Shortcut) -> Result<(), String> {
        self.global_shortcut()
            .unregister(chord)
            .map_err(|e| e.to_string())
    }
    fn persist(&self, settings: &Settings) -> Result<(), String> {
        let path = self
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("settings.json");
        settings::save(&path, settings)
    }
}

impl Runtime {
    fn save_settings(
        &mut self,
        settings: Settings,
        platform: &impl SettingsPlatform,
    ) -> Result<bool, String> {
        if self.phase != Phase::Idle || audio::is_recording() {
            return Err("Finish dictating before changing settings".into());
        }
        let changes = settings.validate_changes(
            &self.settings,
            || platform.input_devices(),
            |model| platform.model_installed(model),
        )?;
        let was_enabled = platform.login_enabled()?;
        if let Some((old_chord, chord)) = changes.shortcut {
            platform
                .register(chord)
                .map_err(|e| format!("Shortcut unavailable: {e}"))?;
            if self.shortcut_registered
                && let Err(error) = platform.unregister(old_chord)
            {
                platform
                    .unregister(chord)
                    .map_err(|e| format!("{error}; remove new shortcut: {e}"))?;
                return Err(error);
            }
        }
        let result = (|| {
            if settings.launch_at_login != was_enabled {
                platform.set_login(settings.launch_at_login)?;
            }
            platform.persist(&settings)
        })();
        if let Err(error) = result {
            let mut errors = vec![error];
            if settings.launch_at_login != was_enabled
                && let Err(error) = platform.set_login(was_enabled)
            {
                errors.push(format!("Restore launch at login: {error}"));
            }
            if let Some((old_chord, chord)) = changes.shortcut {
                if let Err(error) = platform.unregister(chord) {
                    errors.push(format!("Remove new shortcut: {error}"));
                }
                if self.shortcut_registered
                    && let Err(error) = platform.register(old_chord)
                {
                    errors.push(format!("Restore shortcut: {error}"));
                }
            }
            return Err(errors.join("; "));
        }
        if changes.shortcut.is_some() {
            self.capturing_shortcut = false;
            self.shortcut_registered = true;
        }
        self.settings = settings;
        Ok(changes.shortcut.is_some())
    }

    fn capture_shortcut(
        &mut self,
        enabled: bool,
        mut registration: impl FnMut(bool) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.capturing_shortcut == enabled {
            return Ok(());
        }
        if self.phase != Phase::Idle {
            return Err("Finish dictating before recording a shortcut".into());
        }
        if enabled && self.shortcut_registered {
            registration(false)?;
            self.shortcut_registered = false;
        } else if !enabled && !self.shortcut_registered {
            self.capturing_shortcut = false;
            registration(true)?;
            self.shortcut_registered = true;
        }
        self.capturing_shortcut = enabled;
        Ok(())
    }
}

#[tauri::command]
fn pipeline_status(app: AppHandle) -> surface::Status {
    surface::status(&app)
}

/// Begin capture. Called on key-down.
#[tauri::command]
fn record_start() -> Result<(), String> {
    Ok(audio::start()?)
}

/// Stop capture and report how many samples arrived.
///
/// Deliberately returns a count rather than the samples: it is enough to prove
/// the microphone worked (≈ duration × 16000) without shipping the audio over
/// IPC. Session 2's check reads this number.
#[tauri::command]
fn record_stop() -> Result<usize, String> {
    Ok(audio::stop()?.len())
}

/// The whole pipeline, in Rust: stop, transcribe, paste. Returns the text so
/// the frontend can display it, but the text also went to the focused app.
#[tauri::command]
fn finish_dictation(app: AppHandle) -> Result<String, String> {
    let samples = audio::stop()?;
    let model = active_model_path(&app)?;
    let text = dictation_text(&samples, &model)?.text;
    if !text.is_empty() {
        insert::paste(&app, &text)?;
    }
    Ok(text)
}

const MIN_TAKE_SAMPLES: usize = 4_800;
const MIN_TAKE_RMS: f64 = 0.003;

fn take_skip_reason(samples: &[f32]) -> Option<String> {
    let seconds = samples.len() as f64 / 16_000.0;
    if samples.len() < MIN_TAKE_SAMPLES {
        return Some(format!("{seconds:.2} s, below minimum 0.30 s"));
    }
    let rms =
        (samples.iter().map(|&s| f64::from(s).powi(2)).sum::<f64>() / samples.len() as f64).sqrt();
    if !rms.is_finite() {
        return Some("nonfinite audio".into());
    }
    (rms < MIN_TAKE_RMS)
        .then(|| format!("{seconds:.2} s, RMS {rms:.6} below minimum {MIN_TAKE_RMS}"))
}

fn transcript_has_text(text: &str) -> bool {
    text.trim().chars().any(char::is_alphanumeric)
}

struct Transcript {
    text: String,
    inference_ms: Option<i64>,
    skip_reason: Option<String>,
}

fn dictation_text(samples: &[f32], model: &Path) -> Result<Transcript, Problem> {
    if let Some(reason) = take_skip_reason(samples) {
        println!("[hotkey] skipped: {reason}");
        return Ok(Transcript {
            text: String::new(),
            inference_ms: None,
            skip_reason: Some(reason),
        });
    }
    let started = Instant::now();
    let text = transcribe::transcribe(samples, model)?;
    let inference_ms = Some(started.elapsed().as_millis() as i64);
    if !transcript_has_text(&text) {
        println!("[hotkey] skipped: transcript has no letters or numbers");
        return Ok(Transcript {
            text: String::new(),
            inference_ms,
            skip_reason: Some("No words detected".into()),
        });
    }
    Ok(Transcript {
        text: text.trim().to_string(),
        inference_ms,
        skip_reason: None,
    })
}

fn with_history<T>(
    app: &AppHandle,
    read: impl FnOnce(&history::Store) -> Result<T, String>,
) -> Result<T, String> {
    let state = app.state::<History>();
    let store = state.lock().map_err(|e| e.to_string())?;
    read(store.as_ref().map_err(Clone::clone)?)
}

fn record_take(app: &AppHandle, take: &NewTake, retention: HistoryRetention) {
    let result = with_history(app, |store| {
        store.insert(take)?;
        store.prune(retention, history::now_ms())
    });
    match result {
        Ok(_) => {
            if let Err(error) = app.emit("history-changed", ()) {
                eprintln!("[history] emit: {error}");
            }
        }
        Err(error) => eprintln!("[history] {error}"),
    }
}

#[tauri::command]
fn list_history(app: AppHandle, search: String, limit: u32) -> Result<HistoryList, String> {
    let retention = app
        .state::<Mutex<Runtime>>()
        .lock()
        .map_err(|e| e.to_string())?
        .settings
        .history_retention;
    with_history(&app, |store| {
        store.prune(retention, history::now_ms())?;
        store.list(&search, limit)
    })
}

#[tauri::command]
fn delete_history(app: AppHandle, id: i64) -> Result<(), String> {
    with_history(&app, |store| store.delete(id))
}

fn active_model_path(app: &AppHandle) -> Result<PathBuf, String> {
    let model = app
        .state::<Mutex<Runtime>>()
        .lock()
        .map_err(|e| e.to_string())?
        .settings
        .speech_model;
    Ok(models::path(&app.state::<Models>().dir, model))
}

#[tauri::command]
fn list_models(app: AppHandle) -> Result<Vec<ModelEntry>, String> {
    let active = app
        .state::<Mutex<Runtime>>()
        .lock()
        .map_err(|e| e.to_string())?
        .settings
        .speech_model;
    let models = app.state::<Models>();
    let downloads = models.downloads.lock().map_err(|e| e.to_string())?;
    Ok(downloads.entries(&models.dir, active))
}

fn models_changed(app: &AppHandle) {
    if let Err(error) = app.emit("models-changed", ()) {
        eprintln!("[models] emit: {error}");
    }
}

#[tauri::command]
fn download_model(app: AppHandle, model: SpeechModel) -> Result<(), String> {
    let models = app.state::<Models>();
    if models::path(&models.dir, model).is_file() {
        return Ok(());
    }
    let cancel = models
        .downloads
        .lock()
        .map_err(|e| e.to_string())?
        .begin(model)?;
    let dir = models.dir.clone();
    models_changed(&app);
    std::thread::spawn(move || {
        let mut last_emit = Instant::now();
        let result = models::fetch(model, &dir, &cancel, |received| {
            if let Ok(mut downloads) = app.state::<Models>().downloads.lock() {
                downloads.progress(model, received);
            }
            if last_emit.elapsed().as_millis() >= 150 {
                last_emit = Instant::now();
                models_changed(&app);
            }
        });
        match &result {
            Ok(()) => println!("[models] downloaded {}", models::info(model).file_name),
            Err(error) => eprintln!("[models] {}: {error}", models::info(model).file_name),
        }
        if let Ok(mut downloads) = app.state::<Models>().downloads.lock() {
            downloads.finish(model, &result);
        }
        models_changed(&app);
    });
    Ok(())
}

#[tauri::command]
fn cancel_download(app: AppHandle, model: SpeechModel) -> Result<(), String> {
    app.state::<Models>()
        .downloads
        .lock()
        .map_err(|e| e.to_string())?
        .cancel(model);
    models_changed(&app);
    Ok(())
}

#[tauri::command]
fn delete_model(app: AppHandle, model: SpeechModel) -> Result<(), String> {
    let runtime = app.state::<Mutex<Runtime>>();
    let runtime = runtime.lock().map_err(|e| e.to_string())?;
    if runtime.settings.speech_model == model {
        return Err("Choose another model before deleting this one".into());
    }
    let models = app.state::<Models>();
    if models
        .downloads
        .lock()
        .map_err(|e| e.to_string())?
        .is_running(model)
    {
        return Err("Cancel the download first".into());
    }
    models::delete(&models.dir, model)?;
    drop(runtime);
    models_changed(&app);
    Ok(())
}

/// `None` when whisper-cli was found; otherwise the problem, with the
/// install command in its fix.
#[tauri::command]
fn whisper_status() -> Option<Problem> {
    transcribe::whisper_cli().err()
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Result<Settings, String> {
    let state = app.state::<Mutex<Runtime>>();
    let mut settings = state.lock().map_err(|e| e.to_string())?.settings.clone();
    settings.launch_at_login = app.autolaunch().is_enabled().map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    let state = app.state::<Mutex<Runtime>>();
    let mut runtime = state.lock().map_err(|e| e.to_string())?;
    let shortcut_changed = runtime.save_settings(settings.clone(), &app)?;
    drop(runtime);
    if shortcut_changed {
        surface::clear_error(&app);
    }
    *app.state::<Mutex<settings::OverlayPosition>>()
        .lock()
        .map_err(|e| e.to_string())? = settings.overlay_position;
    surface::update_shortcut(&app, &settings.shortcut);
    if let Err(error) = app.emit("settings-changed", &settings) {
        report_error(
            &app,
            &Problem::new(
                "Settings saved, display not refreshed",
                "Close and reopen Settings to see the change.",
            )
            .because(error),
        );
    }
    Ok(settings)
}

#[tauri::command]
fn capture_shortcut(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<Mutex<Runtime>>();
    let mut runtime = state.lock().map_err(|e| e.to_string())?;
    let chord = runtime.settings.chord()?;
    let was_registered = runtime.shortcut_registered;
    let result = runtime.capture_shortcut(enabled, |register| {
        if register {
            app.global_shortcut().register(chord)
        } else {
            app.global_shortcut().unregister(chord)
        }
        .map_err(|e| e.to_string())
    });
    if !was_registered && runtime.shortcut_registered {
        surface::clear_error(&app);
    }
    result
}

#[tauri::command]
fn dismiss_error(app: AppHandle) -> Result<(), String> {
    surface::clear_error(&app);
    if surface::status(&app).phase == "idle"
        && let Some(window) = app.get_webview_window("main")
    {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn input_devices() -> Result<Vec<String>, String> {
    audio::input_devices()
}

#[tauri::command]
fn microphone_test(app: AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        let settings = get_settings(app.clone())?;
        audio::monitor_start(settings.microphone)
    } else {
        audio::monitor_stop()
    }
}

#[tauri::command]
fn microphone_level() -> Result<f32, String> {
    audio::microphone_level()
}

fn microphone_notice(fallback: bool) -> Option<String> {
    fallback.then(|| "Using the default microphone".into())
}

#[tauri::command]
fn report_settings_refresh_error(app: AppHandle) {
    report_error(&app, &settings_refresh_failed());
}

fn settings_refresh_failed() -> Problem {
    Problem::new("Settings couldn’t refresh", "Open Settings and save again.")
}

#[tauri::command]
fn resize_overlay(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    overlay::resize(&app, width, height)
}

/// Show a problem in the overlay, tray and Settings. Never call this while
/// holding the `Runtime` lock: showing the overlay asks the focused app for
/// its window over Accessibility.
fn report_error(app: &AppHandle, error: &Problem) {
    eprintln!("[dictate] {error}");
    surface::set_error(app, error);
    if let Err(error) = overlay::show(app) {
        eprintln!("[dictate] show error: {error}");
    }
    if let Err(error) = app.emit("dictation-error", error) {
        eprintln!("[dictate] emit error: {error}");
    }
}

fn hotkey(
    app: &AppHandle,
    shortcut: &tauri_plugin_global_shortcut::Shortcut,
    event: ShortcutState,
) {
    let state = app.state::<Mutex<Runtime>>();
    let Ok(mut runtime) = state.lock() else {
        return;
    };
    if runtime.capturing_shortcut || runtime.settings.chord().ok().as_ref() != Some(shortcut) {
        return;
    }
    let next = shortcut_transition(runtime.settings.shortcut_mode, runtime.phase, event);
    if next == runtime.phase {
        return;
    }
    if next == Phase::Recording {
        let model = models::path(&app.state::<Models>().dir, runtime.settings.speech_model);
        if !model.is_file() {
            drop(runtime);
            report_error(app, &transcribe::missing_model(&model));
            return;
        }
        runtime.take_id += 1;
        runtime.take_start = Some(TakeStart {
            started_at_ms: history::now_ms(),
            app: focus::frontmost_app(),
        });
        surface::clear_error(app);
        let result = audio::start_device(runtime.settings.microphone.clone()).map(|fallback| {
            surface::set_notice(app, microphone_notice(fallback));
        });
        if let Err(error) = result {
            drop(runtime);
            report_error(app, &error);
            return;
        }
        runtime.phase = Phase::Recording;
        surface::set_phase(app, "recording");
        let take_id = runtime.take_id;
        drop(runtime);
        if let Err(error) = overlay::show(app) {
            report_error(
                app,
                &Problem::new(
                    "Overlay couldn’t open",
                    "Recording continues; release to finish.",
                )
                .because(error),
            );
        }
        let meter_app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            while meter_app
                .state::<Mutex<Runtime>>()
                .lock()
                .is_ok_and(|runtime| {
                    runtime.take_id == take_id && runtime.phase == Phase::Recording
                })
            {
                if let Ok(level) = audio::recording_level() {
                    let _ = meter_app.emit("recording-level", level);
                }
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
        });
    } else {
        let released = Instant::now();
        let start = runtime.take_start.take().unwrap_or(TakeStart {
            started_at_ms: history::now_ms(),
            app: None,
        });
        let retention = runtime.settings.history_retention;
        let model = models::path(&app.state::<Models>().dir, runtime.settings.speech_model);
        match audio::stop() {
            Ok(samples) => {
                runtime.phase = Phase::Transcribing;
                surface::set_phase(app, "transcribing");
                let take_id = runtime.take_id;
                let app = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let transcript = dictation_text(&samples, &model);
                    let result = transcript.as_ref().map_err(Clone::clone).and_then(|t| {
                        if !t.text.is_empty() {
                            insert::paste(&app, &t.text)?;
                        }
                        Ok(!t.text.is_empty())
                    });
                    let total_ms = released.elapsed().as_millis() as i64;
                    let (text, inference_ms, skip_reason) = match transcript {
                        Ok(t) => (t.text, t.inference_ms, t.skip_reason),
                        Err(_) => (String::new(), None, None),
                    };
                    let (status, detail) = match &result {
                        Ok(true) => (TakeStatus::Pasted, None),
                        Ok(false) => (TakeStatus::Skipped, skip_reason),
                        Err(error) => (TakeStatus::Failed, Some(error.to_string())),
                    };
                    record_take(
                        &app,
                        &NewTake {
                            text,
                            created_at_ms: start.started_at_ms,
                            app: start.app,
                            model: transcribe::model_name(&model),
                            audio_ms: (samples.len() / 16) as i64,
                            inference_ms,
                            total_ms,
                            status,
                            detail,
                        },
                        retention,
                    );
                    let outcome = match result {
                        Ok(true) => "done",
                        Ok(false) => "skipped",
                        Err(error) => {
                            report_error(&app, &error);
                            "idle"
                        }
                    };
                    println!("[dictate] release_to_complete_ms={total_ms}");
                    if let Ok(mut runtime) = app.state::<Mutex<Runtime>>().lock() {
                        surface::set_phase(&app, outcome);
                        runtime.phase = Phase::Idle;
                    }
                    let completed = surface::status(&app);
                    std::thread::sleep(std::time::Duration::from_millis(if outcome == "done" {
                        900
                    } else {
                        1800
                    }));
                    let state = app.state::<Mutex<Runtime>>();
                    let Ok(runtime) = state.lock() else {
                        return;
                    };
                    if runtime.take_id != take_id
                        || runtime.phase != Phase::Idle
                        || surface::status(&app).phase != completed.phase
                    {
                        return;
                    }
                    surface::set_phase(&app, "idle");
                    drop(runtime);
                    if surface::status(&app).should_hide()
                        && let Some(window) = app.get_webview_window("main")
                        && let Err(error) = window.hide()
                    {
                        report_error(
                            &app,
                            &Problem::new(
                                "Overlay couldn’t close",
                                "Dismiss it from the menu bar.",
                            )
                            .because(error),
                        );
                    }
                });
            }
            Err(error) => {
                runtime.phase = Phase::Idle;
                surface::set_phase(app, "idle");
                drop(runtime);
                record_take(
                    app,
                    &failed_capture(
                        &start,
                        &model,
                        released.elapsed().as_millis() as i64,
                        &error,
                    ),
                    retention,
                );
                report_error(app, &error);
            }
        }
    }
}

/// The History row for a take whose audio never arrived.
fn failed_capture(start: &TakeStart, model: &Path, total_ms: i64, error: &Problem) -> NewTake {
    NewTake {
        text: String::new(),
        created_at_ms: start.started_at_ms,
        app: start.app.clone(),
        model: transcribe::model_name(model),
        audio_ms: 0,
        inference_ms: None,
        total_ms,
        status: TakeStatus::Failed,
        detail: Some(error.to_string()),
    }
}

fn shortcut_transition(mode: ShortcutMode, phase: Phase, event: ShortcutState) -> Phase {
    match (mode, phase, event) {
        (_, Phase::Idle, ShortcutState::Pressed) => Phase::Recording,
        (ShortcutMode::Hold, Phase::Recording, ShortcutState::Released)
        | (ShortcutMode::Toggle, Phase::Recording, ShortcutState::Pressed) => Phase::Transcribing,
        _ => phase,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| hotkey(app, shortcut, event.state()))
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            pipeline_status,
            dismiss_error,
            record_start,
            record_stop,
            finish_dictation,
            get_settings,
            save_settings,
            capture_shortcut,
            input_devices,
            microphone_test,
            microphone_level,
            resize_overlay,
            report_settings_refresh_error,
            list_history,
            delete_history,
            list_models,
            download_model,
            cancel_download,
            delete_model,
            whisper_status
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let data_dir = app.path().app_data_dir()?;
            let (mut settings, mut startup_error) =
                settings::load_or_default(&data_dir.join("settings.json"));
            let models_dir = data_dir.join("models");
            let removed = models::remove_partials(&models_dir);
            if removed > 0 {
                println!("[models] removed {removed} unfinished download(s)");
            }
            if let Some(model) = models::adopt_legacy(&models_dir, &models::legacy_candidates()) {
                settings.speech_model = model;
                if let Err(error) = settings::save(&data_dir.join("settings.json"), &settings) {
                    eprintln!("[models] save adopted model: {error}");
                }
            }
            let store = history::Store::open(&data_dir.join("history.sqlite3")).and_then(|store| {
                store
                    .prune(settings.history_retention, history::now_ms())
                    .map_err(history::OpenError::Failed)?;
                Ok(store)
            });
            let store = store.map_err(|error| {
                let problem = error.problem();
                let message = problem.fix.clone();
                startup_error = Some(match startup_error.take() {
                    Some(previous) => previous.and(problem),
                    None => problem,
                });
                message
            });
            app.manage::<History>(Mutex::new(store));
            app.manage(Models {
                dir: models_dir,
                downloads: Mutex::default(),
            });
            let shortcut_registered = match settings.chord().and_then(|chord| {
                app.global_shortcut()
                    .register(chord)
                    .map_err(|e| e.to_string())
            }) {
                Ok(()) => true,
                Err(error) => {
                    let problem = Problem::new(
                        "Shortcut unavailable",
                        "Choose another shortcut in Settings.",
                    )
                    .because(error);
                    startup_error = Some(match startup_error {
                        Some(previous) => previous.and(problem),
                        None => problem,
                    });
                    false
                }
            };
            app.manage(Mutex::new(Runtime {
                settings: settings.clone(),
                phase: Phase::Idle,
                take_id: 0,
                take_start: None,
                capturing_shortcut: false,
                shortcut_registered,
            }));
            app.manage(Mutex::new(settings.overlay_position));
            surface::setup(app, &settings)?;
            if let Some(window) = app.get_webview_window("main") {
                window.set_ignore_cursor_events(true)?;
                window.set_focusable(false)?;
            }
            if let Some(error) = &startup_error {
                report_error(app.handle(), error);
            }
            if (startup_error.is_some() || !std::env::args().any(|arg| arg == "--autostart"))
                && let Some(window) = app.get_webview_window("settings")
            {
                window.show()?;
                window.set_focus()?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "settings"
                && let tauri::WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                if let Err(error) = capture_shortcut(window.app_handle().clone(), false) {
                    eprintln!("[settings] {error}");
                }
                if let Err(error) = window.hide() {
                    eprintln!("[settings] {error}");
                }
                if let Err(error) = audio::monitor_stop() {
                    eprintln!("[settings] {error}");
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod take_tests {
    use super::*;

    #[test]
    fn missing_microphone_is_a_notice_and_does_not_block_a_take() {
        assert_eq!(
            microphone_notice(true).as_deref(),
            Some("Using the default microphone")
        );
        assert_eq!(microphone_notice(false), None);
        let status = surface::Status {
            phase: "idle".into(),
            error: None,
            notice: microphone_notice(true),
        };
        assert!(status.should_hide());
    }

    #[test]
    #[ignore = "Measures local input device listing; run explicitly with --ignored --nocapture"]
    fn measure_input_device_listing() {
        let mut timings = Vec::new();
        for _ in 0..30 {
            let start = Instant::now();
            let devices = audio::input_devices().unwrap();
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
            assert!(!devices.is_empty());
        }
        println!("device_listing_cold_ms={:.3}", timings[0]);
        timings.sort_by(f64::total_cmp);
        println!(
            "device_listing_n=30 median_ms={:.3} p95_ms={:.3} max_ms={:.3}",
            timings[15], timings[28], timings[29]
        );
    }

    fn unavailable_shortcut() -> Runtime {
        Runtime {
            settings: Settings::default(),
            phase: Phase::Idle,
            take_id: 0,
            take_start: None,
            capturing_shortcut: false,
            shortcut_registered: false,
        }
    }

    struct TestSettingsPlatform {
        path: std::path::PathBuf,
        login: std::cell::Cell<bool>,
        device_checks: std::cell::Cell<usize>,
        registered: std::cell::RefCell<Vec<tauri_plugin_global_shortcut::Shortcut>>,
    }

    impl SettingsPlatform for TestSettingsPlatform {
        fn input_devices(&self) -> Result<Vec<String>, String> {
            self.device_checks.set(self.device_checks.get() + 1);
            Ok(Vec::new())
        }
        fn model_installed(&self, model: SpeechModel) -> bool {
            model == SpeechModel::SmallEn
        }
        fn login_enabled(&self) -> Result<bool, String> {
            Ok(self.login.get())
        }
        fn set_login(&self, enabled: bool) -> Result<(), String> {
            self.login.set(enabled);
            Ok(())
        }
        fn register(&self, chord: tauri_plugin_global_shortcut::Shortcut) -> Result<(), String> {
            self.registered.borrow_mut().push(chord);
            Ok(())
        }
        fn unregister(&self, chord: tauri_plugin_global_shortcut::Shortcut) -> Result<(), String> {
            let mut registered = self.registered.borrow_mut();
            let index = registered
                .iter()
                .position(|registered| *registered == chord)
                .expect("Shortcut must be registered before unregistering");
            registered.remove(index);
            Ok(())
        }
        fn persist(&self, settings: &Settings) -> Result<(), String> {
            settings::save(&self.path, settings)
        }
    }

    #[test]
    fn save_settings_persists_unrelated_changes_with_unplugged_microphone_and_taken_shortcut() {
        let dir = tempfile::tempdir().unwrap();
        let platform = TestSettingsPlatform {
            path: dir.path().join("settings.json"),
            login: std::cell::Cell::new(false),
            device_checks: std::cell::Cell::new(0),
            registered: std::cell::RefCell::new(Vec::new()),
        };
        let mut runtime = unavailable_shortcut();
        runtime.settings.microphone = Some("Unplugged USB microphone".into());
        for settings in [
            Settings {
                shortcut_mode: ShortcutMode::Toggle,
                ..runtime.settings.clone()
            },
            Settings {
                launch_at_login: true,
                ..runtime.settings.clone()
            },
            Settings {
                theme: settings::Theme::Paper,
                ..runtime.settings.clone()
            },
            Settings {
                appearance_mode: settings::AppearanceMode::Dark,
                ..runtime.settings.clone()
            },
        ] {
            assert!(!runtime.save_settings(settings.clone(), &platform).unwrap());
            assert_eq!(settings::load(&platform.path).unwrap(), settings);
            assert_eq!(runtime.settings, settings);
            assert_eq!(platform.login.get(), settings.launch_at_login);
            assert!(!runtime.shortcut_registered);
            assert!(platform.registered.borrow().is_empty());
            assert_eq!(platform.device_checks.get(), 0);
        }
        let before = runtime.settings.clone();
        let invalid = Settings {
            shortcut: "Space".into(),
            ..before.clone()
        };
        assert!(
            runtime
                .save_settings(invalid, &platform)
                .unwrap_err()
                .contains("Choose a shortcut")
        );
        let missing = Settings {
            microphone: Some("Another missing microphone".into()),
            ..before.clone()
        };
        assert_eq!(
            runtime.save_settings(missing, &platform).unwrap_err(),
            "Microphone unavailable: Another missing microphone"
        );
        assert_eq!(platform.device_checks.get(), 1);
        let not_downloaded = Settings {
            speech_model: SpeechModel::MediumEn,
            ..before.clone()
        };
        assert_eq!(
            runtime
                .save_settings(not_downloaded, &platform)
                .unwrap_err(),
            "Download Medium English before choosing it"
        );
        assert_eq!(runtime.settings, before);
        assert_eq!(settings::load(&platform.path).unwrap(), before);
        let changed = Settings {
            shortcut: "Control+KeyD".into(),
            ..before
        };
        assert!(runtime.save_settings(changed.clone(), &platform).unwrap());
        assert_eq!(
            platform.registered.borrow().as_slice(),
            &[changed.chord().unwrap()]
        );
        assert_eq!(platform.device_checks.get(), 1);
        assert!(runtime.shortcut_registered);
        assert_eq!(settings::load(&platform.path).unwrap(), changed);
        let downloaded = Settings {
            speech_model: SpeechModel::SmallEn,
            ..changed
        };
        assert!(
            !runtime
                .save_settings(downloaded.clone(), &platform)
                .unwrap()
        );
        assert_eq!(runtime.settings.speech_model, SpeechModel::SmallEn);
        assert_eq!(settings::load(&platform.path).unwrap(), downloaded);
    }

    #[test]
    fn failed_cancel_registration_always_exits_capture_and_can_be_retried() {
        let mut runtime = unavailable_shortcut();
        runtime
            .capture_shortcut(true, |_| panic!("No registered shortcut to remove"))
            .unwrap();
        assert!(
            runtime
                .capture_shortcut(false, |_| Err("Taken".into()))
                .is_err()
        );
        assert!(!runtime.capturing_shortcut);
        assert!(!runtime.shortcut_registered);
        runtime
            .capture_shortcut(true, |_| panic!("No registered shortcut to remove"))
            .unwrap();
        runtime.capture_shortcut(false, |_| Ok(())).unwrap();
        assert!(!runtime.capturing_shortcut);
        assert!(runtime.shortcut_registered);
    }

    #[test]
    fn toggle_ignores_release_and_processing_rejects_new_takes() {
        assert_eq!(
            shortcut_transition(ShortcutMode::Toggle, Phase::Idle, ShortcutState::Pressed),
            Phase::Recording
        );
        assert_eq!(
            shortcut_transition(
                ShortcutMode::Toggle,
                Phase::Recording,
                ShortcutState::Released
            ),
            Phase::Recording
        );
        assert_eq!(
            shortcut_transition(
                ShortcutMode::Toggle,
                Phase::Recording,
                ShortcutState::Pressed
            ),
            Phase::Transcribing
        );
        assert_eq!(
            shortcut_transition(ShortcutMode::Hold, Phase::Recording, ShortcutState::Pressed),
            Phase::Recording
        );
        assert_eq!(
            shortcut_transition(
                ShortcutMode::Hold,
                Phase::Recording,
                ShortcutState::Released
            ),
            Phase::Transcribing
        );
        for mode in [ShortcutMode::Hold, ShortcutMode::Toggle] {
            for event in [ShortcutState::Pressed, ShortcutState::Released] {
                assert_eq!(
                    shortcut_transition(mode, Phase::Transcribing, event),
                    Phase::Transcribing
                );
            }
        }
    }

    /// The overlay story renders `src/problems.fixture.json`; this keeps it in
    /// step with the messages the pipeline really reports.
    #[test]
    fn overlay_story_fixture_matches_real_problems() {
        let real = serde_json::json!({
            "accessibility": insert::accessibility_needed(),
            "settingsRefresh": settings_refresh_failed(),
            "noSamples": audio::no_samples(),
            "missingModel": transcribe::missing_model(std::path::Path::new(
                "/Applications/dictate.app/models/ggml-base.en.bin"
            )),
            "whisperMissing": transcribe::whisper_missing(),
        });
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../src/problems.fixture.json")).unwrap();
        assert_eq!(fixture, real);
    }

    #[test]
    fn failed_capture_is_recorded_as_a_failed_take() {
        let take = failed_capture(
            &TakeStart {
                started_at_ms: 1_000,
                app: Some("Notes".into()),
            },
            Path::new("/models/ggml-base.en.bin"),
            12,
            &audio::no_samples(),
        );
        assert_eq!(take.status, TakeStatus::Failed);
        assert_eq!(take.created_at_ms, 1_000);
        assert_eq!(take.app.as_deref(), Some("Notes"));
        assert_eq!(take.total_ms, 12);
        assert_eq!(take.audio_ms, 0);
        assert_eq!(take.model, "ggml-base.en");
        assert!(take.text.is_empty());
        assert!(
            take.detail
                .unwrap()
                .contains("No sound from the microphone")
        );
        let dir = tempfile::tempdir().unwrap();
        let store = history::Store::open(&dir.path().join("history.sqlite3")).unwrap();
        store
            .insert(&failed_capture(
                &TakeStart {
                    started_at_ms: 1,
                    app: None,
                },
                Path::new("/models/ggml-base.en.bin"),
                1,
                &audio::no_samples(),
            ))
            .unwrap();
        assert_eq!(
            store.list("", 10).unwrap().entries[0].status,
            TakeStatus::Failed
        );
    }

    #[test]
    fn silence_and_short_speech_are_skipped() {
        assert!(take_skip_reason(&vec![0.0; 32_000]).is_some());
        assert!(take_skip_reason(&vec![0.2; 2_400]).is_some());
        assert!(take_skip_reason(&vec![0.002; 16_000]).is_some());
        assert!(take_skip_reason(&[]).is_some());
        for samples in [vec![0.0; 32_000], vec![0.2; 2_400]] {
            let transcript = dictation_text(&samples, Path::new("/nonexistent.bin")).unwrap();
            assert_eq!(transcript.text, "");
            assert_eq!(transcript.inference_ms, None);
            assert!(transcript.skip_reason.is_some());
        }
    }

    #[test]
    fn punctuation_is_dropped_without_filtering_words() {
        for text in ["", "  ", " … — !? ", "。！？"] {
            assert!(!transcript_has_text(text), "{text:?}");
        }
        for text in ["you", " I ", "42", "café", "你好"] {
            assert!(transcript_has_text(text), "{text:?}");
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn one_second_of_generated_speech_passes() {
        let temp = tempfile::tempdir().unwrap();
        let aiff = temp.path().join("speech.aiff");
        let wav = temp.path().join("speech.wav");
        assert!(
            std::process::Command::new("say")
                .args(["-v", "Samantha", "-o"])
                .arg(&aiff)
                .arg("The quick brown fox jumps over the lazy dog.")
                .status()
                .unwrap()
                .success()
        );
        assert!(
            std::process::Command::new("afconvert")
                .args(["-f", "WAVE", "-d", "LEI16@16000", "-c", "1"])
                .arg(aiff)
                .arg(&wav)
                .status()
                .unwrap()
                .success()
        );
        let mut reader = hound::WavReader::open(wav).unwrap();
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .take(16_000)
            .map(|s| f32::from(s.unwrap()) / 32768.0)
            .collect();
        assert_eq!(samples.len(), 16_000);
        assert!(take_skip_reason(&samples).is_none());
        assert!(take_skip_reason(&samples[..2_400]).is_some());
    }
}
