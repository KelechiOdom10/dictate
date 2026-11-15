fn main() {
    link_swift_compatibility_libs();
    tauri_build::build()
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
