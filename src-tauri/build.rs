/// Every `#[tauri::command]` in `lib.rs`. Declaring them makes each one an
/// `allow-<name>` permission, so a window can call only the commands its
/// capability grants: the ink and bar panels get a few `annotate_*`
/// commands, `main` and `settings` the rest. A command left out of this list
/// is callable from no window; `lib.rs` tests that the list matches.
const APP_COMMANDS: &[&str] = &[
    "pipeline_status",
    "dismiss_error",
    "record_start",
    "record_stop",
    "finish_dictation",
    "get_settings",
    "save_settings",
    "capture_shortcut",
    "input_devices",
    "microphone_test",
    "microphone_level",
    "resize_overlay",
    "dictation_stop",
    "dictation_cancel",
    "settings_ready",
    "open_notice_settings",
    "annotate_bar_state",
    "annotate_pen",
    "annotate_pen_down",
    "annotate_pen_up",
    "annotate_stop",
    "annotate_copy_image",
    "annotate_dismiss",
    "list_annotations",
    "reveal_annotation",
    "copy_annotation",
    "delete_annotation",
    "screen_permission",
    "development_build",
    "open_screen_recording_settings",
    "report_settings_refresh_error",
    "list_history",
    "delete_history",
    "storage_usage",
    "list_models",
    "download_model",
    "cancel_download",
    "delete_model",
    "open_model_licence",
    "whisper_status",
    "app_folders",
    "update_status",
    "check_for_updates",
    "open_location",
];

fn main() {
    link_swift_compatibility_libs();
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS)),
    )
    .expect("tauri build");
}

/// screencapturekit's Swift bridge needs `libswiftCompatibility56.a` at link
/// time and the system Swift runtime at load time. Its own build script only
/// searches the Xcode toolchain layout, so with Command Line Tools alone the
/// link fails on `__swift_FORCE_LOAD_$_swiftCompatibility56`. Its rpath is a
/// dependency link-arg, which Cargo does not pass to our binaries, so without
/// ours they abort with "Library not loaded: @rpath/libswift_Concurrency.dylib".
fn link_swift_compatibility_libs() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    let Ok(output) = std::process::Command::new("xcode-select")
        .arg("-p")
        .output()
    else {
        return;
    };
    let developer_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let libs = std::path::Path::new(&developer_dir).join("usr/lib/swift/macosx");
    if libs.join("libswiftCompatibility56.a").exists() {
        println!("cargo:rustc-link-search=native={}", libs.display());
    }
}
