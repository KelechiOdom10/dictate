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

mod audio;
mod insert;
mod transcribe;

/// Reports which half of the pipeline is reachable. Proves the frontend can
/// call Rust and that the module tree is wired, before any audio exists.
#[tauri::command]
fn pipeline_status() -> String {
    let recorder = audio::is_recording();
    format!("modules loaded; recording={recorder}")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![pipeline_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
