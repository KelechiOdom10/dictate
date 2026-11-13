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
    let text = transcribe::transcribe(&samples)?;
    insert::paste(&app, &text)?;
    Ok(text)
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
                                let _ = window.hide();
                                if let Err(error) = audio::stop() {
                                    println!("[hotkey] audio::stop: {error}");
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
