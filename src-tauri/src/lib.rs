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
//! only ever needs text.

mod audio;
mod insert;
mod transcribe;

mod settings;
mod surface;

use settings::{Settings, ShortcutMode};
use std::{sync::Mutex, time::Instant};
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
    capturing_shortcut: bool,
    shortcut_registered: bool,
}

impl Runtime {
    fn shortcut_changes(
        &self,
        chord: tauri_plugin_global_shortcut::Shortcut,
    ) -> Result<bool, String> {
        Ok(chord != self.settings.chord()?)
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
    audio::start()
}

/// Stop capture and report how many samples arrived.
///
/// Deliberately returns a count rather than the samples: it is enough to prove
/// the microphone worked (≈ duration × 16000) without shipping the audio over
/// IPC. Session 2's check reads this number.
#[tauri::command]
fn record_stop() -> Result<usize, String> {
    audio::stop().map(|samples| samples.len())
}

/// The whole pipeline, in Rust: stop, transcribe, paste. Returns the text so
/// the frontend can display it, but the text also went to the focused app.
#[tauri::command]
fn finish_dictation(app: AppHandle) -> Result<String, String> {
    let samples = audio::stop()?;
    let text = dictation_text(&samples)?;
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

fn dictation_text(samples: &[f32]) -> Result<String, String> {
    if let Some(reason) = take_skip_reason(samples) {
        println!("[hotkey] skipped: {reason}");
        return Ok(String::new());
    }
    let text = transcribe::transcribe(samples)?;
    if !transcript_has_text(&text) {
        println!("[hotkey] skipped: transcript has no letters or numbers");
        return Ok(String::new());
    }
    Ok(text.trim().to_string())
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
    if runtime.phase != Phase::Idle || audio::is_recording() {
        return Err("Finish dictating before changing settings".into());
    }
    let chord = settings.chord()?;
    if let Some(name) = &settings.microphone
        && !audio::input_devices()?.contains(name)
    {
        return Err(format!("Microphone unavailable: {name}"));
    }
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("settings.json");
    let was_enabled = app.autolaunch().is_enabled().map_err(|e| e.to_string())?;
    let old_chord = runtime.settings.chord()?;
    let changed = runtime.shortcut_changes(chord)?;
    if changed {
        app.global_shortcut()
            .register(chord)
            .map_err(|e| format!("Shortcut unavailable: {e}"))?;
        if runtime.shortcut_registered
            && let Err(error) = app.global_shortcut().unregister(old_chord)
        {
            app.global_shortcut()
                .unregister(chord)
                .map_err(|e| format!("{error}; remove new shortcut: {e}"))?;
            return Err(error.to_string());
        }
    }
    let change_login = |enabled| {
        if enabled {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        }
    };
    let result = (|| {
        if settings.launch_at_login != was_enabled {
            change_login(settings.launch_at_login).map_err(|e| e.to_string())?;
        }
        settings::save(&path, &settings)
    })();
    if let Err(error) = result {
        let mut errors = vec![error];
        if settings.launch_at_login != was_enabled
            && let Err(error) = change_login(was_enabled)
        {
            errors.push(format!("Restore launch at login: {error}"));
        }
        if changed {
            if let Err(error) = app.global_shortcut().unregister(chord) {
                errors.push(format!("Remove new shortcut: {error}"));
            }
            if runtime.shortcut_registered
                && let Err(error) = app.global_shortcut().register(old_chord)
            {
                errors.push(format!("Restore shortcut: {error}"));
            }
        }
        return Err(errors.join("; "));
    }
    if changed {
        runtime.capturing_shortcut = false;
        runtime.shortcut_registered = true;
        surface::clear_error(&app);
    }
    runtime.settings = settings.clone();
    surface::update_shortcut(&app, &settings.shortcut);
    if let Err(error) = app.emit("settings-changed", &settings) {
        report_error(
            &app,
            &format!("Settings saved, but could not refresh the shortcut label: {error}"),
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

fn report_error(app: &AppHandle, error: &str) {
    eprintln!("[dictate] {error}");
    surface::set_error(app, error);
    if let Some(window) = app.get_webview_window("main")
        && let Err(error) = window.show()
    {
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
        surface::clear_error(app);
        let result = audio::start_device(runtime.settings.microphone.clone()).map(|fallback| {
            surface::set_notice(app, microphone_notice(fallback));
        });
        if let Err(error) = result {
            report_error(app, &error);
            return;
        }
        runtime.phase = Phase::Recording;
        surface::set_phase(app, "recording");
        if let Some(window) = app.get_webview_window("main")
            && let Err(error) = window.show()
        {
            report_error(app, &error.to_string());
        }
    } else {
        let released = Instant::now();
        match audio::stop() {
            Ok(samples) => {
                runtime.phase = Phase::Transcribing;
                surface::set_phase(app, "transcribing");
                let app = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let result = dictation_text(&samples).and_then(|text| {
                        if !text.is_empty() {
                            insert::paste(&app, &text)?;
                        }
                        Ok(())
                    });
                    if let Err(error) = result {
                        report_error(&app, &error);
                    }
                    println!(
                        "[dictate] release_to_complete_ms={}",
                        released.elapsed().as_millis()
                    );
                    if let Ok(mut runtime) = app.state::<Mutex<Runtime>>().lock() {
                        runtime.phase = Phase::Idle;
                    }
                    surface::set_phase(&app, "idle");
                    if surface::status(&app).should_hide()
                        && let Some(window) = app.get_webview_window("main")
                        && let Err(error) = window.hide()
                    {
                        report_error(&app, &error.to_string());
                    }
                });
            }
            Err(error) => {
                runtime.phase = Phase::Idle;
                surface::set_phase(app, "idle");
                report_error(app, &error);
            }
        }
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
            microphone_level
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let (settings, mut startup_error) =
                settings::load_or_default(&app.path().app_data_dir()?.join("settings.json"));
            let shortcut_registered = match settings.chord().and_then(|chord| {
                app.global_shortcut()
                    .register(chord)
                    .map_err(|e| e.to_string())
            }) {
                Ok(()) => true,
                Err(error) => {
                    let message = format!(
                        "Shortcut unavailable: {error}. Choose another shortcut in Settings."
                    );
                    startup_error = Some(match startup_error {
                        Some(previous) => format!("{previous}\n{message}"),
                        None => message,
                    });
                    false
                }
            };
            app.manage(Mutex::new(Runtime {
                settings: settings.clone(),
                phase: Phase::Idle,
                capturing_shortcut: false,
                shortcut_registered,
            }));
            surface::setup(app, &settings)?;
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
            capturing_shortcut: false,
            shortcut_registered: false,
        }
    }

    #[test]
    fn unrelated_settings_do_not_retry_an_unavailable_shortcut() {
        let mut runtime = unavailable_shortcut();
        for settings in [
            Settings {
                microphone: Some("USB microphone".into()),
                ..Settings::default()
            },
            Settings {
                shortcut_mode: ShortcutMode::Toggle,
                ..Settings::default()
            },
            Settings {
                launch_at_login: true,
                ..Settings::default()
            },
        ] {
            assert!(!runtime.shortcut_changes(settings.chord().unwrap()).unwrap());
            runtime.settings = settings;
            assert!(!runtime.shortcut_registered);
        }
        assert!(
            runtime
                .shortcut_changes("Control+KeyD".parse().unwrap())
                .unwrap()
        );
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

    #[test]
    fn silence_and_short_speech_are_skipped() {
        assert!(take_skip_reason(&vec![0.0; 32_000]).is_some());
        assert!(take_skip_reason(&vec![0.2; 2_400]).is_some());
        assert!(take_skip_reason(&vec![0.002; 16_000]).is_some());
        assert!(take_skip_reason(&[]).is_some());
        assert_eq!(dictation_text(&vec![0.0; 32_000]).unwrap(), "");
        assert_eq!(dictation_text(&vec![0.2; 2_400]).unwrap(), "");
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
