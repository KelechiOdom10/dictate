//! Writes the themed app icons as PNGs so they can be looked at without
//! launching the app.
//!
//!   cargo run --example themed_icons -- target/themed-icons
//!
//! These are the images `app_icon::sync` hands to AppKit when the theme or the
//! appearance changes. What they look like on screen is unverified: the app is
//! an accessory app and has no Dock tile to put them in.

use dictate_lib::app_icon;
use std::path::PathBuf;

fn main() {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/themed-icons"));
    if let Err(error) = std::fs::create_dir_all(&out) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
    for (name, png) in app_icon::variants() {
        let path = out.join(format!("{name}.png"));
        if let Err(error) = std::fs::write(&path, &png) {
            eprintln!("error: {path:?}: {error}");
            std::process::exit(1);
        }
        println!("{} ({} bytes)", path.display(), png.len());
    }
}
