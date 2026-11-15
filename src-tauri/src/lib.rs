//! dictate — hold a key, speak, get text in whatever app has focus.
//!
//! The pipeline is three contracts, deliberately separate so each can be
//! built and tested alone:
//!
//!   audio::start / audio::stop  ->  Vec<f32>  (16 kHz, mono, in memory)
//!   transcribe::transcribe      ->  String
//!   transcribe::transcribe_timed -> Vec<Word>  (ms times, for Annotate)
//!   audio::frames -> transcribe::LiveTake -> String
//!                               (Parakeet: decode 100 ms frames while recording)
//!   insert::paste               ->  ()       (into the focused app)
//!
//! Keep them separate. A pipeline that only works end to end is a pipeline
//! you cannot measure a part of.
//!
//! Audio never crosses the IPC boundary. Ten seconds at 16 kHz is 160 000
//! floats, which serialises to megabytes of JSON for no benefit; the frontend
//! receives status and small scalar levels, never raw samples.

pub mod annotate;
mod annotate_ui;
mod audio;
#[cfg(target_os = "macos")]
mod clipboard;
#[cfg(target_os = "macos")]
pub mod frames;
mod insert;
pub mod package;
#[cfg(target_os = "macos")]
pub mod screen;
pub mod transcribe;

mod focus;
mod history;
mod ink;
mod models;
mod overlay;
pub mod problem;
mod settings;
mod sound;
mod surface;

use history::{HistoryList, NewTake, TakeStatus};
use models::{ModelEntry, SpeechModel};
use problem::Problem;
use settings::{HistoryRetention, MicMode, PasteMethod, Settings, ShortcutMode};
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
    annotate: annotate::Phase,
    /// False when the Annotate chord failed to register or equals the
    /// dictation chord. Its presses then belong to dictation, or to no one.
    annotate_registered: bool,
    /// Set while an Annotate session records: its clock and what was known at
    /// the start tap.
    annotate_started: Option<(Instant, annotate::Started)>,
    /// The dictation take being decoded while it records, with Parakeet.
    live: Option<transcribe::LiveTake>,
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
        if self.annotate != annotate::Phase::Idle {
            return Err("Stop Annotate before changing settings".into());
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
        let undo_shortcut = |errors: &mut Vec<String>| {
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
        };
        let old_annotate = self
            .annotate_registered
            .then(|| self.settings.annotate_chord().ok())
            .flatten();
        if let Some(new_annotate) = changes.annotate {
            let swapped = (|| {
                if let Some(old) = old_annotate {
                    platform.unregister(old)?;
                }
                if let Some(new) = new_annotate
                    && let Err(error) = platform.register(new)
                {
                    let mut errors = vec![format!("Annotate shortcut unavailable: {error}")];
                    if let Some(old) = old_annotate
                        && let Err(error) = platform.register(old)
                    {
                        errors.push(format!("Restore Annotate shortcut: {error}"));
                    }
                    return Err(errors.join("; "));
                }
                Ok(())
            })();
            if let Err(error) = swapped {
                let mut errors = vec![error];
                undo_shortcut(&mut errors);
                return Err(errors.join("; "));
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
            if let Some(new_annotate) = changes.annotate {
                if let Some(new) = new_annotate
                    && let Err(error) = platform.unregister(new)
                {
                    errors.push(format!("Remove new Annotate shortcut: {error}"));
                }
                if let Some(old) = old_annotate
                    && let Err(error) = platform.register(old)
                {
                    errors.push(format!("Restore Annotate shortcut: {error}"));
                }
            }
            undo_shortcut(&mut errors);
            return Err(errors.join("; "));
        }
        if changes.shortcut.is_some() {
            self.capturing_shortcut = false;
            self.shortcut_registered = true;
        }
        if let Some(new_annotate) = changes.annotate {
            self.annotate_registered = new_annotate.is_some();
        }
        self.settings = settings;
        Ok(changes.shortcut.is_some() || changes.annotate.is_some())
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
    let settings = app
        .state::<Mutex<Runtime>>()
        .lock()
        .map_err(|e| e.to_string())?
        .settings
        .clone();
    save_debug_recording(&app, &samples, settings.debug_recordings);
    let model = active_model_path(&app)?;
    let text = output_text(&dictation_text(&samples, &model, None)?.text, &settings);
    if !text.is_empty() {
        insert::paste(&app, &text, insert::Delivery::from_settings(&settings))?;
    }
    Ok(text)
}

/// The transcript as Output settings shape it. Empty stays empty.
fn output_text(text: &str, settings: &Settings) -> String {
    insert::shape(text, settings.capitalize_first, settings.trailing_space)
}

const DEBUG_RECORDINGS_KEPT: usize = 20;

fn debug_recordings_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("debug-recordings"))
}

/// Saves a take's audio when Advanced → Save debug recordings is on, or when
/// launched with `DICTATE_DEBUG_WAV=1`. Keeps the newest 20.
fn save_debug_recording(app: &AppHandle, samples: &[f32], enabled: bool) {
    let forced = std::env::var_os("DICTATE_DEBUG_WAV").is_some_and(|value| value == "1");
    if !(enabled || forced) || samples.is_empty() {
        return;
    }
    let result = debug_recordings_dir(app).and_then(|dir| {
        let path = write_debug_recording(&dir, samples, history::now_ms())?;
        prune_debug_recordings(&dir, DEBUG_RECORDINGS_KEPT)?;
        Ok(path)
    });
    match result {
        Ok(path) => log::info!("[audio] debug WAV: {}", path.display()),
        Err(error) => log::warn!("[audio] debug WAV: {error}"),
    }
}

fn write_debug_recording(dir: &Path, samples: &[f32], now_ms: i64) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Create {}: {e}", dir.display()))?;
    let path = dir.join(format!("take-{now_ms}.wav"));
    audio::save_wav(&path, samples)?;
    Ok(path)
}

fn prune_debug_recordings(dir: &Path, keep: usize) -> Result<(), String> {
    let mut takes: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "wav")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("take-"))
        })
        .collect();
    takes.sort_by_key(|path| {
        path.file_stem()
            .and_then(|stem| {
                stem.to_string_lossy()
                    .strip_prefix("take-")?
                    .parse::<i64>()
                    .ok()
            })
            .unwrap_or(0)
    });
    let excess = takes.len().saturating_sub(keep);
    for path in &takes[..excess] {
        std::fs::remove_file(path).map_err(|e| format!("Remove {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Opens or closes the always-on microphone to match Advanced settings.
fn apply_mic_mode(settings: &Settings) -> Result<(), Problem> {
    let device = (settings.mic_mode == MicMode::Warm).then(|| settings.microphone.clone());
    audio::keep_warm(device).map_err(|cause| {
        Problem::new(
            "Microphone couldn’t stay ready",
            "It opens when you dictate instead. Check it in Settings → General.",
        )
        .because(cause)
    })
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppFolders {
    data: String,
    logs: String,
}

#[tauri::command]
fn app_folders(app: AppHandle) -> Result<AppFolders, String> {
    let path = app.path();
    Ok(AppFolders {
        data: path
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .display()
            .to_string(),
        logs: path
            .app_log_dir()
            .map_err(|e| e.to_string())?
            .display()
            .to_string(),
    })
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum Location {
    Data,
    Logs,
    Source,
}

const SOURCE_URL: &str = "https://github.com/KelechiOdom10/dictate";

#[tauri::command]
fn open_location(app: AppHandle, location: Location) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let folder = match location {
        Location::Source => {
            return app
                .opener()
                .open_url(SOURCE_URL, None::<&str>)
                .map_err(|e| e.to_string());
        }
        Location::Data => app.path().app_data_dir(),
        Location::Logs => app.path().app_log_dir(),
    }
    .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&folder).map_err(|e| format!("Create {}: {e}", folder.display()))?;
    app.opener()
        .open_path(folder.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
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

/// The take's text, from `live` when the take was decoded while it recorded,
/// else in one batch. A skipped take discards `live`.
fn dictation_text(
    samples: &[f32],
    model: &Path,
    live: Option<transcribe::LiveTake>,
) -> Result<Transcript, Problem> {
    if let Some(reason) = take_skip_reason(samples) {
        log::info!("[hotkey] skipped: {reason}");
        return Ok(Transcript {
            text: String::new(),
            inference_ms: None,
            skip_reason: Some(reason),
        });
    }
    let started = Instant::now();
    let text = match live {
        Some(live) => live.finish(samples)?,
        None => transcribe::transcribe(samples, model)?,
    };
    let inference_ms = Some(started.elapsed().as_millis() as i64);
    if !transcript_has_text(&text) {
        log::info!("[hotkey] skipped: transcript has no letters or numbers");
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
                log::warn!("[history] emit: {error}");
            }
        }
        Err(error) => log::warn!("[history] {error}"),
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

/// Loads and warms the chosen model on its own thread when it runs in
/// process, or lets go of the held one when it doesn't, so the next take
/// doesn't wait for a load. Reads the latest choice while holding a lock, so
/// quick switches end on the last model chosen whatever order threads run in.
fn prepare_active_model(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        static PREPARING: Mutex<()> = Mutex::new(());
        let _preparing = PREPARING.lock().unwrap_or_else(|e| e.into_inner());
        match active_model_path(&app) {
            Ok(model) => {
                if let Err(problem) = transcribe::prepare(&model) {
                    log::warn!("[transcribe] prepare {}: {problem}", model.display());
                }
            }
            Err(error) => log::warn!("[transcribe] prepare: {error}"),
        }
    });
}

#[tauri::command]
fn open_model_licence(app: AppHandle, model: SpeechModel) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let licence = models::info(model)
        .licence
        .ok_or_else(|| format!("{} has no licence link", models::info(model).name))?;
    app.opener()
        .open_url(licence.url, None::<&str>)
        .map_err(|e| e.to_string())
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
    surface::refresh_menu(app);
    if let Err(error) = app.emit("models-changed", ()) {
        log::warn!("[models] emit: {error}");
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
            Ok(()) => log::info!("[models] downloaded {}", models::info(model).file_name),
            Err(error) => log::warn!("[models] {}: {error}", models::info(model).file_name),
        }
        if let Ok(mut downloads) = app.state::<Models>().downloads.lock() {
            downloads.finish(model, &result);
        }
        if result.is_ok() {
            prepare_active_model(&app);
        }
        models_changed(&app);
    });
    Ok(())
}

/// Proof that setup has managed `Runtime`. Only `manage_runtime` makes one.
struct RuntimeManaged(());

fn manage_runtime(app: &tauri::App, runtime: Runtime) -> RuntimeManaged {
    app.manage(Mutex::new(runtime));
    RuntimeManaged(())
}

/// Adopts a legacy model on its own thread. Adoption saves the chosen model
/// into `Runtime`, so it needs `RuntimeManaged`: before then `app.state`
/// panics and the model would stay "Preparing" for good.
fn spawn_legacy_adoption(
    _: &RuntimeManaged,
    app: AppHandle,
    pending: SpeechModel,
    dir: PathBuf,
    candidates: Vec<PathBuf>,
) {
    std::thread::spawn(move || adopt_legacy_model(&app, pending, &dir, &candidates));
}

/// Copies and verifies a model from before the Models page, off the launch
/// path (it hashes up to 1.6 GB). The Models page shows it as preparing.
fn adopt_legacy_model(app: &AppHandle, pending: SpeechModel, dir: &Path, candidates: &[PathBuf]) {
    let adopted = models::adopt_legacy(dir, candidates);
    if let Ok(mut downloads) = app.state::<Models>().downloads.lock() {
        downloads.finish(pending, &Ok(()));
    }
    if let Some(model) = adopted {
        let state = app.state::<Mutex<Runtime>>();
        let saved = state
            .lock()
            .map_err(|e| e.to_string())
            .and_then(|mut runtime| {
                if models::path(dir, runtime.settings.speech_model).is_file() {
                    return Ok(None);
                }
                let mut settings = runtime.settings.clone();
                settings.speech_model = model;
                app.persist(&settings)?;
                runtime.settings = settings.clone();
                Ok(Some(settings))
            });
        match saved {
            Ok(Some(mut settings)) => {
                prepare_active_model(app);
                settings.launch_at_login = app.autolaunch().is_enabled().unwrap_or(false);
                if let Err(error) = app.emit("settings-changed", &settings) {
                    log::warn!("[models] emit adopted model: {error}");
                }
            }
            Ok(None) => {}
            Err(error) => log::warn!("[models] save adopted model: {error}"),
        }
    }
    models_changed(app);
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

/// `None` when whisper-cli was found or the chosen model doesn't need it;
/// otherwise the problem, with the install command in its fix.
#[tauri::command]
fn whisper_status(app: AppHandle) -> Option<Problem> {
    let needs_cli =
        active_model_path(&app).map_or(true, |model| transcribe::needs_whisper_cli(&model));
    needs_cli.then(transcribe::whisper_cli)?.err()
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
    let previous = runtime.settings.clone();
    let shortcut_changed = runtime.save_settings(settings.clone(), &app)?;
    drop(runtime);
    if previous.speech_model != settings.speech_model {
        prepare_active_model(&app);
    }
    if (previous.mic_mode, &previous.microphone) != (settings.mic_mode, &settings.microphone) {
        let app = app.clone();
        // Opening the microphone takes 60–80 ms; keep the save responsive.
        // Holding the lock while reading the settings again and applying them
        // means the last toggle wins, whatever order these threads run in.
        static APPLYING: Mutex<()> = Mutex::new(());
        std::thread::spawn(move || {
            let _applying = APPLYING.lock().unwrap_or_else(|e| e.into_inner());
            let latest = app
                .state::<Mutex<Runtime>>()
                .lock()
                .map(|runtime| runtime.settings.clone())
                .map_err(|e| e.to_string());
            if let Ok(latest) = latest
                && let Err(problem) = apply_mic_mode(&latest)
            {
                report_error(&app, &problem);
            }
        });
    }
    if !previous.annotate_enabled && settings.annotate_enabled && !screen::has_permission() {
        screen::request_permission();
    }
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
fn annotate_bar_state(app: AppHandle) -> annotate_ui::BarState {
    annotate_ui::bar_state(&app)
}

#[tauri::command]
fn annotate_pen(app: AppHandle) {
    annotate_ui::pen(&app, |now_ms| ink::PenInput::Button { now_ms });
}

#[tauri::command]
fn annotate_pen_down(app: AppHandle) {
    annotate_ui::pen_down(&app);
}

#[tauri::command]
fn annotate_pen_up(app: AppHandle) {
    annotate_ui::pen_up(&app);
}

/// The stop button on the floating bar: the same as a tap of the shortcut,
/// but only ever stops.
#[tauri::command]
fn annotate_stop(app: AppHandle) {
    let state = app.state::<Mutex<Runtime>>();
    let Ok(runtime) = state.lock() else {
        return;
    };
    if runtime.annotate == annotate::Phase::Recording {
        annotate_tap(&app, runtime);
    }
}

#[tauri::command]
fn resize_overlay(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    overlay::resize(&app, width, height)
}

/// Show a problem in the overlay, tray and Settings. Never call this while
/// holding the `Runtime` lock: showing the overlay asks the focused app for
/// its window over Accessibility.
fn report_error(app: &AppHandle, error: &Problem) {
    log::warn!("[dictate] {error}");
    surface::set_error(app, error);
    if let Err(error) = overlay::show(app) {
        log::warn!("[dictate] show error: {error}");
    }
    if let Err(error) = app.emit("dictation-error", error) {
        log::warn!("[dictate] emit error: {error}");
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
    if runtime.capturing_shortcut {
        return;
    }
    match shortcut_target(&runtime.settings, runtime.annotate_registered, shortcut) {
        Some(Target::Dictation) => {}
        Some(Target::Annotate) => {
            annotate_shortcut(app, runtime, event);
            return;
        }
        None => {
            drop(runtime);
            if *shortcut == annotate_ui::escape() && event == ShortcutState::Pressed {
                annotate_ui::pen(app, |_| ink::PenInput::Escape);
            }
            return;
        }
    }
    let next = shortcut_transition(runtime.settings.shortcut_mode, runtime.phase, event);
    if next == runtime.phase {
        return;
    }
    if dictation_blocked(runtime.annotate, runtime.phase, next) {
        drop(runtime);
        flash_notice(app, annotate::DICTATION_BLOCKED);
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
        let sound_feedback = runtime.settings.sound_feedback;
        let mute = if sound_feedback {
            sound::START_MUTE
        } else {
            std::time::Duration::ZERO
        };
        let result =
            audio::start_device(runtime.settings.microphone.clone(), mute).map(|fallback| {
                if sound_feedback {
                    sound::play(sound::Cue::Start);
                }
                surface::set_notice(app, microphone_notice(fallback));
            });
        if let Err(error) = result {
            drop(runtime);
            report_error(app, &error);
            return;
        }
        runtime.phase = Phase::Recording;
        runtime.live = start_live_take(&model);
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
        let settings = runtime.settings.clone();
        let live = runtime.live.take();
        match audio::stop() {
            Ok(samples) => {
                if settings.sound_feedback {
                    sound::play(sound::Cue::Stop);
                }
                runtime.phase = Phase::Transcribing;
                surface::set_phase(app, "transcribing");
                let take_id = runtime.take_id;
                let app = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    save_debug_recording(&app, &samples, settings.debug_recordings);
                    let delivery = insert::Delivery::from_settings(&settings);
                    let transcript = dictation_text(&samples, &model, live).map(|t| Transcript {
                        text: output_text(&t.text, &settings),
                        ..t
                    });
                    if let Ok(t) = &transcript {
                        log::info!(
                            "[dictate] release_to_text_ms={} inference_ms={:?} audio_ms={} model={}",
                            released.elapsed().as_millis(),
                            t.inference_ms,
                            samples.len() / 16,
                            transcribe::model_name(&model)
                        );
                    }
                    let result = transcript.as_ref().map_err(Clone::clone).and_then(|t| {
                        if !t.text.is_empty() {
                            insert::paste(&app, &t.text, delivery)?;
                        }
                        Ok(!t.text.is_empty())
                    });
                    let total_ms = released.elapsed().as_millis() as i64;
                    let (text, inference_ms, skip_reason) = match transcript {
                        Ok(t) => (t.text.trim_end().to_string(), t.inference_ms, t.skip_reason),
                        Err(_) => (String::new(), None, None),
                    };
                    let copy_only = delivery.method == PasteMethod::CopyOnly;
                    let (status, detail) = match &result {
                        Ok(true) if copy_only => (TakeStatus::Copied, None),
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
                        Ok(true) if copy_only => "copied",
                        Ok(true) => "done",
                        Ok(false) => "skipped",
                        Err(error) => {
                            report_error(&app, &error);
                            "idle"
                        }
                    };
                    log::info!("[dictate] release_to_complete_ms={total_ms}");
                    if let Ok(mut runtime) = app.state::<Mutex<Runtime>>().lock() {
                        surface::set_phase(&app, outcome);
                        runtime.phase = Phase::Idle;
                    }
                    let completed = surface::status(&app);
                    std::thread::sleep(std::time::Duration::from_millis(match outcome {
                        "done" => 900,
                        _ => 1800,
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
                drop(live);
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

/// Starts decoding the take that just started recording, when `model` streams.
/// Without it the take is transcribed in one batch at release.
fn start_live_take(model: &Path) -> Option<transcribe::LiveTake> {
    if !transcribe::streams(model) {
        return None;
    }
    match audio::frames() {
        Ok(frames) => transcribe::LiveTake::start(model, frames),
        Err(error) => {
            log::warn!("[dictate] no live frames, will transcribe at release: {error}");
            None
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Dictation,
    Annotate,
}

/// Which feature a shortcut event belongs to. Dictation is checked first, so a
/// saved dictation shortcut that equals the Annotate default keeps working.
fn shortcut_target(
    settings: &Settings,
    annotate_registered: bool,
    shortcut: &tauri_plugin_global_shortcut::Shortcut,
) -> Option<Target> {
    let dictation = settings.chord().ok();
    if dictation.as_ref() == Some(shortcut) {
        return Some(Target::Dictation);
    }
    let annotate = settings.annotate_chord().ok();
    (annotate_registered && annotate.as_ref() == Some(shortcut) && annotate != dictation)
        .then_some(Target::Annotate)
}

/// A dictation take may not start while Annotate holds the microphone or is
/// still saving: `audio` keeps one shared recording.
fn dictation_blocked(annotate: annotate::Phase, phase: Phase, next: Phase) -> bool {
    phase == Phase::Idle && next == Phase::Recording && annotate::blocks_dictation(annotate)
}

/// Show `notice` in the pill for a few seconds, then put back what was there.
fn flash_notice(app: &AppHandle, notice: &'static str) {
    let previous = surface::status(app).notice;
    surface::set_notice(app, Some(notice.into()));
    surface::refresh(app);
    if let Err(error) = overlay::show(app) {
        log::warn!("[annotate] show notice: {error}");
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        if surface::status(&app).notice.as_deref() == Some(notice) {
            surface::set_notice(&app, previous);
            surface::refresh(&app);
            if surface::status(&app).should_hide()
                && let Some(window) = app.get_webview_window("main")
                && let Err(error) = window.hide()
            {
                log::warn!("[annotate] hide notice: {error}");
            }
        }
    });
}

/// The Annotate shortcut. Idle: a press starts a session. Recording: a press
/// opens a provisional stroke and the release decides, by the tap-or-hold
/// rule in `ink`, whether it was a stroke to keep or a tap that stops.
fn annotate_shortcut(
    app: &AppHandle,
    runtime: std::sync::MutexGuard<'_, Runtime>,
    event: ShortcutState,
) {
    match (runtime.annotate, event) {
        (annotate::Phase::Recording, ShortcutState::Pressed) => {
            drop(runtime);
            annotate_ui::pressed(app);
        }
        (annotate::Phase::Recording, ShortcutState::Released) => {
            if annotate_ui::released(app) {
                annotate_tap(app, runtime);
            }
        }
        (_, ShortcutState::Pressed) => annotate_tap(app, runtime),
        (_, ShortcutState::Released) => {}
    }
}

/// One tap of the Annotate shortcut, or the bar's stop button: start a
/// session, or stop it and save the package in the background.
fn annotate_tap(app: &AppHandle, mut runtime: std::sync::MutexGuard<'_, Runtime>) {
    let next = annotate::tap(runtime.annotate, runtime.phase == Phase::Idle);
    if next == runtime.annotate {
        if runtime.annotate == annotate::Phase::Idle {
            drop(runtime);
            flash_notice(app, annotate::DICTATION_BUSY);
        }
        return;
    }
    let model = models::path(&app.state::<Models>().dir, runtime.settings.speech_model);
    match next {
        annotate::Phase::Starting => {
            if !model.is_file() {
                drop(runtime);
                report_error(app, &transcribe::missing_model(&model));
                return;
            }
            if !screen::has_permission() {
                drop(runtime);
                // The first time, this shows the system prompt and puts
                // dictate in the Screen Recording list; afterwards it only
                // answers.
                screen::request_permission();
                report_error(app, &annotate::screen_permission_needed());
                return;
            }
            let clock = Instant::now();
            let unix_ms = history::now_ms();
            surface::clear_error(app);
            let fallback = match audio::start_device(
                runtime.settings.microphone.clone(),
                Default::default(),
            ) {
                Ok(fallback) => fallback,
                Err(error) => {
                    drop(runtime);
                    report_error(app, &error);
                    return;
                }
            };
            let audio_offset_ms = clock.elapsed().as_millis() as u64;
            let start = ScreenStart {
                clock,
                unix_ms,
                audio_offset_ms,
                display: annotate::recorded_display(),
                frontmost_app: focus::frontmost_app_id(),
                options: screen::CaptureOptions {
                    include_own_windows: runtime.settings.annotate_show_in_recordings,
                    ..Default::default()
                },
                fallback,
            };
            runtime.annotate = next;
            drop(runtime);
            // `screen::start` lists shareable content and starts the capture,
            // both blocking system calls, so it runs here and not on the main
            // thread that delivers shortcuts.
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let began = Instant::now();
                let result =
                    screen::start(screen::DisplayTarget::Id(start.display.id), start.options);
                let screen_start_ms = began.elapsed().as_millis() as u64;
                finish_annotate_start(&app, start, result, screen_start_ms);
            });
        }
        annotate::Phase::Packaging => {
            let Some((clock, started)) = runtime.annotate_started.take() else {
                runtime.annotate = annotate::Phase::Idle;
                return;
            };
            let stopped = Instant::now();
            let duration_ms = clock.elapsed().as_millis() as u64;
            let samples = audio::stop();
            runtime.annotate = next;
            surface::set_notice(app, None);
            surface::set_phase(app, "packaging");
            drop(runtime);
            // After the lock is released: hiding the panels and releasing Esc
            // wait on the main thread.
            let strokes = annotate_ui::end(app);
            if let Err(error) = overlay::show(app) {
                log::warn!("[annotate] show packaging: {error}");
            }
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let (video, partial_video) = stop_screen(&started);
                let recorded = annotate::Recorded {
                    started,
                    duration_ms,
                    model: transcribe::model_name(&model),
                    strokes,
                    video,
                    partial_video,
                };
                // Whatever was captured is kept: only a session with neither
                // audio nor any video is lost.
                let outcome = match &samples {
                    Err(error) if recorded.video.is_none() && recorded.partial_video.is_none() => {
                        Err(error.clone())
                    }
                    _ => save_annotation(
                        &app,
                        samples.as_deref().map_err(Clone::clone),
                        recorded,
                        stopped,
                        &model,
                    ),
                };
                if let Ok(mut runtime) = app.state::<Mutex<Runtime>>().lock() {
                    runtime.annotate = annotate::packaged(runtime.annotate);
                    surface::set_phase(&app, "idle");
                }
                let packaged = match outcome {
                    Ok(packaged) => packaged,
                    Err(error) => return report_error(&app, &error),
                };
                if let Err(error) = clipboard::copy_text(&packaged.summary) {
                    return report_error(
                        &app,
                        &Problem::new(
                            "Annotation saved, not copied",
                            "Open Settings › History › Annotations and choose Copy again.",
                        )
                        .because(error),
                    );
                }
                notify_copied(&app);
                if let Err(error) = app.emit("annotations-changed", ()) {
                    log::warn!("[annotate] emit annotations-changed: {error}");
                }
                // The pill and the bar share the bottom of the screen, so
                // only one shows: a warning in the pill, or the bar's Copied.
                if let Some(warning) = &packaged.warning {
                    return report_error(&app, warning);
                }
                if surface::status(&app).should_hide()
                    && let Some(window) = app.get_webview_window("main")
                    && let Err(error) = window.hide()
                {
                    log::warn!("[annotate] hide: {error}");
                }
                annotate_ui::show_copied(&app, packaged.contact_sheet());
            });
        }
        annotate::Phase::Idle | annotate::Phase::Recording => {}
    }
}

/// What the start tap knew, carried to the thread that starts the screen.
struct ScreenStart {
    clock: Instant,
    unix_ms: i64,
    audio_offset_ms: u64,
    display: package::Display,
    frontmost_app: Option<package::App>,
    options: screen::CaptureOptions,
    fallback: bool,
}

/// `screen::start` returned: move `Starting` to `Recording` and show the
/// panels, or release the microphone and report why it did not start.
fn finish_annotate_start(
    app: &AppHandle,
    start: ScreenStart,
    result: Result<(), String>,
    screen_start_ms: u64,
) {
    let state = app.state::<Mutex<Runtime>>();
    let mut runtime = state.lock().unwrap_or_else(|e| e.into_inner());
    runtime.annotate = annotate::screen_started(runtime.annotate, result.is_ok());
    if let Err(error) = result {
        if let Err(stop) = audio::stop() {
            log::warn!("[annotate] release microphone: {stop}");
        }
        drop(runtime);
        log::warn!("[annotate] screen start failed after screen_start_ms={screen_start_ms}");
        report_error(
            app,
            &if error == screen::PERMISSION_DENIED {
                annotate::screen_permission_needed()
            } else {
                annotate::screen_start_failed(error)
            },
        );
        return;
    }
    let ScreenStart {
        clock,
        unix_ms,
        audio_offset_ms,
        display,
        frontmost_app,
        options,
        fallback,
    } = start;
    let video_offset_ms = clock.elapsed().as_millis() as u64;
    runtime.annotate_started = Some((
        clock,
        annotate::Started {
            unix_ms,
            audio_offset_ms,
            video_offset_ms,
            display: display.clone(),
            frontmost_app,
        },
    ));
    let notice = microphone_notice(fallback);
    let show_pill = notice.is_some();
    surface::set_notice(app, notice);
    surface::set_phase(app, "annotating");
    drop(runtime);
    log::info!(
        "[annotate] started, screen_start_ms={screen_start_ms} audio_offset_ms={audio_offset_ms} video_offset_ms={video_offset_ms} display={} at ({}, {}) own_windows_in_video={}",
        display.id,
        display.frame.x,
        display.frame.y,
        options.include_own_windows,
    );
    annotate_ui::begin(app, display, clock, unix_ms);
    // The floating bar shows the session; the pill only appears to carry a
    // notice.
    if show_pill && let Err(error) = overlay::show(app) {
        log::warn!("[annotate] show notice: {error}");
    }
}

/// Stops the screen recording. A failure leaves a session without video,
/// not a lost session, and hands back any MP4 the failed stop left behind.
fn stop_screen(started: &annotate::Started) -> (Option<annotate::Recording>, Option<PathBuf>) {
    match screen::stop() {
        Ok(video) => {
            log::info!(
                "[annotate] video {}x{} {} fps duration_ms={} bytes={} frames={:?} finalize_wall_ms={} own_windows_excluded={}",
                video.width,
                video.height,
                video.fps,
                video.duration_ms,
                video.file_bytes,
                video.frames,
                video.wall_ms,
                video.own_windows_excluded,
            );
            let recording = annotate::Recording {
                path: video.path,
                offset_ms: started.video_offset_ms,
                width: video.width,
                height: video.height,
                fps: video.fps,
                duration_ms: video.duration_ms,
            };
            (Some(recording), None)
        }
        Err(error) => {
            log::warn!("[annotate] screen stop: {error}");
            (None, error.partial_recording)
        }
    }
}

fn notify_copied(app: &AppHandle) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(error) = app
        .notification()
        .builder()
        .title("Annotation copied")
        .body("Paste it into your agent. The images are in the folder it names.")
        .show()
    {
        log::warn!("[annotate] notify: {error}");
    }
}

/// Builds the package from a stopped session: video moved in, audio,
/// transcript with word times, frames, contact sheet, `annotation.json` and
/// `summary.md`. Logs the stop-to-package time, which is also stored in
/// `annotation.json` as `packagingMs`.
fn save_annotation(
    app: &AppHandle,
    samples: Result<&[f32], Problem>,
    recorded: annotate::Recorded,
    stopped: Instant,
    model: &Path,
) -> Result<annotate::Packaged, Problem> {
    let dir = annotations_dir(app)?;
    let folder = dir.join(annotate::session_id(recorded.started.unix_ms));
    let temp_video = recorded
        .video
        .as_ref()
        .map(|video| video.path.clone())
        .or_else(|| recorded.partial_video.clone());
    let sample_count = samples.as_ref().map_or(0, |samples| samples.len());
    let packaged = annotate::package(&folder, samples, recorded, stopped, |samples| {
        transcribe::transcribe_timed(samples, model)
    })
    .map_err(|e| {
        Problem::new(
            "Annotation not saved",
            "Check there is free disk space, then try again.",
        )
        .because(e)
    })?;
    // `screen` writes each take into its own temp folder; the MP4 has moved
    // out, so remove the folder now rather than at the next day's sweep.
    if let Some(parent) = temp_video.as_deref().and_then(Path::parent)
        && let Err(error) = std::fs::remove_dir(parent)
    {
        log::warn!("[annotate] remove {}: {error}", parent.display());
    }
    let annotation = &packaged.annotation;
    log::info!(
        "[annotate] packaged {} samples={} duration_ms={} words={} strokes={} marks={} frames={} transcribe_ms={} stop_to_package_ms={} partial={} render={:?}",
        folder.display(),
        sample_count,
        annotation.duration_ms,
        annotation.transcript.words.len(),
        annotation.strokes.len(),
        annotation.marks.len(),
        annotation.frames.len(),
        packaged.transcribe_ms,
        annotation.packaging_ms.unwrap_or_default(),
        !annotation.partial.is_empty(),
        packaged.rendered,
    );
    Ok(packaged)
}

fn annotations_dir(app: &AppHandle) -> Result<PathBuf, Problem> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| Problem::new("Annotations folder unavailable", "Restart dictate.").because(e))?
        .join(annotate::FOLDER))
}

fn annotation_folder(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    annotate::session_folder(&annotations_dir(app)?, id)
        .filter(|folder| folder.is_dir())
        .ok_or_else(|| format!("No annotation {id}"))
}

#[tauri::command]
fn list_annotations(app: AppHandle) -> Result<Vec<annotate::AnnotationEntry>, String> {
    annotate::list(&annotations_dir(&app)?).map_err(|e| format!("Read annotations: {e}"))
}

#[tauri::command]
fn reveal_annotation(app: AppHandle, id: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let folder = annotation_folder(&app, &id)?;
    app.opener()
        .reveal_item_in_dir(folder.join(annotate::SUMMARY_FILE))
        .map_err(|e| e.to_string())
}

/// History's "Copy again": the session's summary, as at stop.
#[tauri::command]
fn copy_annotation(app: AppHandle, id: String) -> Result<(), String> {
    let folder = annotation_folder(&app, &id)?;
    let summary = std::fs::read_to_string(folder.join(annotate::SUMMARY_FILE))
        .map_err(|e| format!("Read summary: {e}"))?;
    clipboard::copy_text(&summary)?;
    notify_copied(&app);
    Ok(())
}

/// The floating bar's "Copy image": the contact sheet of the session that
/// just finished.
#[tauri::command]
fn annotate_copy_image(app: AppHandle) -> Result<(), String> {
    let sheet = annotate_ui::copied_contact_sheet(&app).ok_or("This annotation has no images")?;
    clipboard::copy_png(&sheet)
}

#[tauri::command]
fn annotate_dismiss(app: AppHandle) {
    annotate_ui::dismiss(&app);
}

#[tauri::command]
fn development_build() -> bool {
    tauri::is_dev()
}

#[tauri::command]
fn screen_permission() -> bool {
    screen::has_permission()
}

/// Asks once (which lists dictate under Screen Recording), then opens that
/// pane of System Settings.
#[tauri::command]
fn open_screen_recording_settings(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    if !screen::has_permission() {
        screen::request_permission();
    }
    app.opener()
        .open_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            None::<&str>,
        )
        .map_err(|e| e.to_string())
}

fn annotate_shortcut_taken() -> Problem {
    Problem::new(
        "Annotate shortcut unavailable",
        "Choose another in Settings › Annotate, or quit the app that uses it.",
    )
}

const ANNOTATE_SHORTCUT_CLASH: &str =
    "The Annotate shortcut is the same as your dictation shortcut. Change one in Settings.";

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
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("dictate".into()),
                    }),
                ])
                .level(log::LevelFilter::Info)
                .level_for("transcribe_cpp", log::LevelFilter::Warn)
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
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
            annotate_bar_state,
            annotate_pen,
            annotate_pen_down,
            annotate_pen_up,
            annotate_stop,
            annotate_copy_image,
            annotate_dismiss,
            list_annotations,
            reveal_annotation,
            copy_annotation,
            screen_permission,
            development_build,
            open_screen_recording_settings,
            report_settings_refresh_error,
            list_history,
            delete_history,
            list_models,
            download_model,
            cancel_download,
            delete_model,
            open_model_licence,
            whisper_status,
            app_folders,
            open_location
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let data_dir = app.path().app_data_dir()?;
            let (settings, mut startup_error) =
                settings::load_or_default(&data_dir.join("settings.json"));
            let models_dir = data_dir.join("models");
            let removed = models::remove_partials(&models_dir);
            if removed > 0 {
                log::info!("[models] removed {removed} unfinished download(s)");
            }
            let legacy = models::legacy_candidates();
            let pending = models::legacy_pending(&models_dir, &legacy);
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
            let mut downloads = models::Downloads::default();
            if let Some(model) = pending {
                downloads.prepare(model);
            }
            app.manage(Models {
                dir: models_dir.clone(),
                downloads: Mutex::new(downloads),
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
            let annotate_registered = match settings.annotate_chord() {
                _ if !settings.annotate_enabled => Ok(false),
                Ok(chord) if settings.chord().ok() == Some(chord) => Err(Problem::new(
                    "Annotate shortcut unavailable",
                    ANNOTATE_SHORTCUT_CLASH,
                )),
                Ok(chord) => app
                    .global_shortcut()
                    .register(chord)
                    .map(|()| true)
                    .map_err(|error| annotate_shortcut_taken().because(error)),
                Err(error) => Err(annotate_shortcut_taken().because(error)),
            };
            let annotate_registered = match annotate_registered {
                Ok(registered) => registered,
                Err(problem) => {
                    startup_error = Some(match startup_error {
                        Some(previous) => previous.and(problem),
                        None => problem,
                    });
                    false
                }
            };
            let runtime_managed = manage_runtime(
                app,
                Runtime {
                    settings: settings.clone(),
                    phase: Phase::Idle,
                    take_id: 0,
                    take_start: None,
                    capturing_shortcut: false,
                    shortcut_registered,
                    annotate: annotate::Phase::Idle,
                    annotate_registered,
                    annotate_started: None,
                    live: None,
                },
            );
            if let Some(model) = pending {
                spawn_legacy_adoption(
                    &runtime_managed,
                    app.handle().clone(),
                    model,
                    models_dir,
                    legacy,
                );
            }
            app.manage(Mutex::new(settings.overlay_position));
            prepare_active_model(app.handle());
            if settings.mic_mode == MicMode::Warm
                && let Err(problem) = apply_mic_mode(&settings)
            {
                startup_error = Some(match startup_error {
                    Some(previous) => previous.and(problem),
                    None => problem,
                });
            }
            surface::setup(app, &settings)?;
            annotate_ui::setup(app)?;
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
                    log::warn!("[settings] {error}");
                }
                if let Err(error) = window.hide() {
                    log::warn!("[settings] {error}");
                }
                if let Err(error) = audio::monitor_stop() {
                    log::warn!("[settings] {error}");
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
            annotate: annotate::Phase::Idle,
            annotate_registered: false,
            annotate_started: None,
            live: None,
        }
    }

    /// `build.rs` declares the app commands, so a window may call only what
    /// its capability grants. The declared list must match what is
    /// registered, every command must reach some window, and the Annotate
    /// panels get only their own buttons' commands.
    #[test]
    fn every_command_is_declared_and_the_annotate_panels_get_only_their_own() {
        use std::collections::BTreeSet;
        fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
            let from = text.find(start).unwrap() + start.len();
            &text[from..from + text[from..].find(end).unwrap()]
        }
        let declared: BTreeSet<String> = between(
            include_str!("../build.rs"),
            "const APP_COMMANDS: &[&str] = &[",
            "];",
        )
        .split(',')
        .map(|name| name.trim().trim_matches('"').to_string())
        .filter(|name| !name.is_empty())
        .collect();
        let registered: BTreeSet<String> =
            between(include_str!("lib.rs"), "generate_handler![", "])")
                .split(',')
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .collect();
        assert_eq!(declared, registered);

        let grants = |json: &str| -> (Vec<String>, BTreeSet<String>) {
            let capability: serde_json::Value = serde_json::from_str(json).unwrap();
            let windows = capability["windows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|w| w.as_str().unwrap().to_string())
                .collect();
            let commands = capability["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|p| p.as_str()?.strip_prefix("allow-"))
                .map(|name| name.replace('-', "_"))
                .collect();
            (windows, commands)
        };
        let set = |names: &[&str]| -> BTreeSet<String> {
            names.iter().map(|name| name.to_string()).collect()
        };
        let (ink_windows, ink) = grants(include_str!("../capabilities/annotate.json"));
        let (bar_windows, bar) = grants(include_str!("../capabilities/annotate-bar.json"));
        let (default_windows, default) = grants(include_str!("../capabilities/default.json"));
        assert_eq!(ink_windows, ["ink"]);
        assert_eq!(
            ink,
            set(&["get_settings", "annotate_pen_down", "annotate_pen_up"])
        );
        assert_eq!(bar_windows, ["annotate-bar"]);
        assert_eq!(
            bar,
            set(&[
                "get_settings",
                "annotate_bar_state",
                "annotate_pen",
                "annotate_stop",
                "annotate_copy_image",
                "annotate_dismiss",
            ])
        );
        assert_eq!(default_windows, ["main", "settings"]);
        // Every window themes itself from the stored settings, so get_settings is
        // the one read-only command the panels share with main and settings.
        let shared = set(&["get_settings"]);
        assert_eq!(&default & &ink, shared);
        assert_eq!(&default & &bar, shared);
        assert_eq!(&ink & &bar, shared);
        let granted: BTreeSet<String> = default.union(&ink).chain(&bar).cloned().collect();
        assert_eq!(granted, declared);
    }

    #[test]
    fn a_dictation_shortcut_equal_to_the_annotate_one_stays_dictation() {
        let annotate: tauri_plugin_global_shortcut::Shortcut =
            Settings::default().annotate_chord().unwrap();
        let clash = Settings {
            shortcut: Settings::default().annotate_shortcut,
            ..Settings::default()
        };
        // Even if registration were reported, dictation wins the clash.
        assert_eq!(
            shortcut_target(&clash, true, &annotate),
            Some(Target::Dictation)
        );
        assert_eq!(
            shortcut_target(&clash, false, &annotate),
            Some(Target::Dictation)
        );

        let apart = Settings::default();
        let dictation = apart.chord().unwrap();
        assert_eq!(
            shortcut_target(&apart, true, &dictation),
            Some(Target::Dictation)
        );
        assert_eq!(
            shortcut_target(&apart, true, &annotate),
            Some(Target::Annotate)
        );
        // Annotate's chord did not register: its presses go nowhere.
        assert_eq!(shortcut_target(&apart, false, &annotate), None);
        let other: tauri_plugin_global_shortcut::Shortcut = "Alt+KeyQ".parse().unwrap();
        assert_eq!(shortcut_target(&apart, true, &other), None);
    }

    #[test]
    fn dictation_cannot_start_during_an_annotate_session() {
        use annotate::Phase as Annotate;
        for mode in [ShortcutMode::Hold, ShortcutMode::Toggle] {
            let next = shortcut_transition(mode, Phase::Idle, ShortcutState::Pressed);
            assert!(dictation_blocked(Annotate::Recording, Phase::Idle, next));
            assert!(dictation_blocked(Annotate::Packaging, Phase::Idle, next));
            assert!(!dictation_blocked(Annotate::Idle, Phase::Idle, next));
        }
        // A hold-mode release after a blocked press changes nothing, so the
        // session keeps its audio.
        assert_eq!(
            shortcut_transition(ShortcutMode::Hold, Phase::Idle, ShortcutState::Released),
            Phase::Idle
        );
        // An unrelated transition is never blocked.
        assert!(!dictation_blocked(
            Annotate::Recording,
            Phase::Recording,
            Phase::Transcribing
        ));
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
    fn annotate_toggle_and_shortcut_change_what_is_registered() {
        let dir = tempfile::tempdir().unwrap();
        let platform = TestSettingsPlatform {
            path: dir.path().join("settings.json"),
            login: std::cell::Cell::new(false),
            device_checks: std::cell::Cell::new(0),
            registered: std::cell::RefCell::new(Vec::new()),
        };
        let mut runtime = unavailable_shortcut();
        let s = Settings::default().annotate_chord().unwrap();
        platform.register(s).unwrap();
        runtime.annotate_registered = true;

        let off = Settings {
            annotate_enabled: false,
            ..runtime.settings.clone()
        };
        assert!(runtime.save_settings(off.clone(), &platform).unwrap());
        assert!(platform.registered.borrow().is_empty());
        assert!(!runtime.annotate_registered);

        let moved = Settings {
            annotate_enabled: true,
            annotate_shortcut: "Control+Shift+KeyA".into(),
            ..off
        };
        assert!(runtime.save_settings(moved.clone(), &platform).unwrap());
        assert_eq!(
            platform.registered.borrow().as_slice(),
            &[moved.annotate_chord().unwrap()]
        );
        assert!(runtime.annotate_registered);
        assert_eq!(settings::load(&platform.path).unwrap(), moved);

        runtime.annotate = annotate::Phase::Recording;
        let during = Settings {
            annotate_show_in_recordings: true,
            ..moved.clone()
        };
        assert_eq!(
            runtime.save_settings(during, &platform).unwrap_err(),
            "Stop Annotate before changing settings"
        );
        assert_eq!(runtime.settings, moved);
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
            let transcript = dictation_text(&samples, Path::new("/nonexistent.bin"), None).unwrap();
            assert_eq!(transcript.text, "");
            assert_eq!(transcript.inference_ms, None);
            assert!(transcript.skip_reason.is_some());
        }
    }

    #[test]
    fn debug_recordings_keep_only_the_newest_takes() {
        let dir = tempfile::tempdir().unwrap();
        let recordings = dir.path().join("debug-recordings");
        for now_ms in [5, 30, 100, 2_000] {
            write_debug_recording(&recordings, &[0.1, -0.1], now_ms).unwrap();
        }
        std::fs::write(recordings.join("notes.txt"), "kept").unwrap();
        prune_debug_recordings(&recordings, 2).unwrap();
        let mut left: Vec<String> = std::fs::read_dir(&recordings)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["notes.txt", "take-100.wav", "take-2000.wav"]);
    }

    #[test]
    fn output_text_applies_settings_and_leaves_skips_empty() {
        let settings = Settings {
            capitalize_first: true,
            trailing_space: true,
            ..Settings::default()
        };
        assert_eq!(output_text("send it now", &settings), "Send it now ");
        assert_eq!(output_text("", &settings), "");
        assert_eq!(
            output_text("send it now", &Settings::default()),
            "send it now"
        );
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
