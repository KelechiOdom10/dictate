//! dictate — hold a key, speak, get text in whatever app has focus.
//!
//! The pipeline is three contracts, deliberately separate so each can be
//! built and tested alone:
//!
//!   audio::start / audio::stop  ->  Vec<f32>  (16 kHz, mono, in memory)
//!   transcribe::transcribe      ->  String
//!   transcribe::transcribe_timed -> Vec<Word>  (ms times, for Annotate)
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
pub mod package;
#[cfg(target_os = "macos")]
pub mod screen;
pub mod transcribe;

use std::time::Instant;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Hold-to-talk chord. Change here, not in two places.
fn chord() -> Shortcut {
    // Alt+Shift+Space: unused by Spotlight, Raycast and the usual suspects.
    Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::Space)
}

#[tauri::command]
fn pipeline_status() -> String {
    format!("modules loaded; recording={}", audio::is_recording())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    let state = event.state();
                    println!("[hotkey] {shortcut:?} state={state:?}");

                    if let Some(window) = app.get_webview_window("main") {
                        // Showing and hiding is the whole session-1 experiment:
                        // if the overlay never disappears, you are only getting
                        // Pressed and the interaction has to be a toggle.
                        match state {
                            ShortcutState::Pressed => {
                                let _ = window.show();
                                // Surfaced, not swallowed: while the stages are
                                // stubs this line is how you know the handler
                                // ran at all.
                                if let Err(error) = record_start() {
                                    println!("[hotkey] record_start: {error}");
                                }
                            }
                            ShortcutState::Released => {
                                let released = Instant::now();
                                let _ = window.hide();
                                // Release drains the buffer; a later record_stop finds no recording.
                                match audio::stop() {
                                    Ok(samples) => {
                                        println!(
                                            "[hotkey] {} samples ({:.2}s @ 16 kHz)",
                                            samples.len(),
                                            samples.len() as f32 / 16_000.0
                                        );
                                        let app = app.clone();
                                        tauri::async_runtime::spawn_blocking(move || {
                                            match dictation_text(&samples) {
                                                Ok(text) if text.is_empty() => {},
                                                Ok(text) => {
                                                    println!("[transcribe] release_to_text_ms={} text={text:?}", released.elapsed().as_millis());
                                                    match insert::paste(&app, &text) {
                                                    Ok(()) => println!(
                                                        "[insert] release_to_paste_complete_ms={}",
                                                        released.elapsed().as_millis()
                                                    ),
                                                    Err(error) => eprintln!("[insert] {error}"),
                                                    }
                                                },
                                                Err(error) => eprintln!("[transcribe] {error}"),
                                            }
                                        });
                                    }
                                    Err(error) => eprintln!("[hotkey] audio::stop: {error}"),
                                }
                            }
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            pipeline_status,
            record_start,
            record_stop,
            finish_dictation
        ])
        .setup(|app| {
            // Accessory keeps the process out of the Dock and prevents the app
            // becoming active, which is the difference between text landing in
            // the field you were typing in and landing in your own overlay.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.global_shortcut()
                .register(chord())
                .expect("failed to register the dictation chord");

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod take_tests {
    use super::*;

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
