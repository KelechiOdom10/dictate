//! The app icon follows the selected theme.
//!
//! The Dock icon is not a file on disk at runtime. macOS asks the application
//! for its icon image at launch and whenever the app sets one; the .icns inside
//! the bundle is what Finder, Launchpad and Spotlight read, and nothing here
//! can change that after the fact. So this module draws the same artboard as
//! `docs/icon-drafts/a-ember-wave.svg` from the live palette and hands it to
//! AppKit. Tauri 2 has no app-level icon setter (only window, webview and tray
//! icons), which is why the last step is objc2.
//!
//! Two honest limits, both stated in docs/release.md:
//!
//! - dictate runs with `ActivationPolicy::Accessory` (`lib.rs` setup), so it has
//!   no Dock tile and no app-switcher entry. On a normal launch there is nothing
//!   on screen for this icon to repaint. It becomes visible the day the app takes
//!   a Regular activation policy.
//! - The menu-bar glyph is a template image and is deliberately not themed.
//!   macOS tints template images for light and dark menu bars by itself.
use crate::settings::{AppearanceMode, Settings, Theme};
use tiny_skia::{FillRule, Paint, Path, PathBuilder, Pixmap, Transform};

/// The artboard. The drafts are drawn on 1024; halving keeps the encode cheap
/// and still exceeds the largest size anything draws an accessory icon at.
const SIZE: f32 = 512.0;

/// Geometry from the draft, scaled by 0.5. The plate is the macOS grid square:
/// 824 of 1024 inset by 100, corners at 185.
const PLATE: (f32, f32, f32, f32) = (50.0, 50.0, 412.0, 412.0);
const PLATE_RADIUS: f32 = 92.5;
const BAR_HALF_WIDTH: f32 = 16.0;
const BAR_CENTERS: [f32; 5] = [156.0, 206.0, 256.0, 306.0, 356.0];
/// Half of each bar's total length: the draft's 64 px stroke plus its round caps.
const BAR_HALF_HEIGHTS: [f32; 5] = [41.0, 76.0, 116.0, 86.0, 51.0];
/// The middle bar is the live one, and the only bar the accent touches.
const LIVE_BAR: usize = 2;

/// Plate, ink and accent, read from `src/ui/palettes.generated.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub plate: [u8; 3],
    pub bars: [u8; 3],
    pub accent: [u8; 3],
}

/// Plate and ink are shared by every theme today; the accent is where a theme
/// differs, and only Paper differs from the rest.
///
/// Values from `src/ui/palettes.generated.json`: `canvas` is the plate, `text`
/// the ink, `live` the accent. Ember, Capy, Rose Pine and Neutral all carry the
/// same live orange there, so they share an accent until the generated palette
/// makes them differ. Paper's live colour is its ink, which is why a Paper icon
/// comes out monochrome by construction rather than by omission.
pub fn palette(theme: Theme, dark: bool) -> Palette {
    let (plate, bars) = if dark {
        ([26, 24, 22], [241, 238, 234])
    } else {
        ([251, 250, 248], [28, 26, 24])
    };
    let accent = match theme {
        Theme::Paper => bars,
        Theme::Ember | Theme::Capy | Theme::RosePine | Theme::Neutral => {
            if dark {
                [246, 125, 81]
            } else {
                [223, 88, 34]
            }
        }
    };
    Palette {
        plate,
        bars,
        accent,
    }
}

fn paint(rgb: [u8; 3]) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(rgb[0], rgb[1], rgb[2], 255);
    paint.anti_alias = true;
    paint
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

/// The icon for one theme in one appearance, or `None` if a shape fails to
/// build. Nothing is cached: four palettes, drawn on demand.
pub fn pixmap(theme: Theme, dark: bool) -> Option<Pixmap> {
    let palette = palette(theme, dark);
    let mut pixmap = Pixmap::new(SIZE as u32, SIZE as u32)?;
    let (x, y, w, h) = PLATE;
    let plate = rounded_rect(x, y, w, h, PLATE_RADIUS)?;
    pixmap.fill_path(
        &plate,
        &paint(palette.plate),
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    for (bar, (center, half_height)) in BAR_CENTERS
        .iter()
        .zip(BAR_HALF_HEIGHTS)
        .enumerate()
    {
        let colour = if bar == LIVE_BAR {
            palette.accent
        } else {
            palette.bars
        };
        let capsule = rounded_rect(
            center - BAR_HALF_WIDTH,
            SIZE / 2.0 - half_height,
            BAR_HALF_WIDTH * 2.0,
            half_height * 2.0,
            BAR_HALF_WIDTH,
        )?;
        pixmap.fill_path(
            &capsule,
            &paint(colour),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    Some(pixmap)
}

/// PNG bytes for one theme, which is what AppKit is handed.
pub fn png(theme: Theme, dark: bool) -> Option<Vec<u8>> {
    pixmap(theme, dark)?.encode_png().ok()
}

/// Every theme's icon as PNG, in review order, for
/// `cargo run --example themed_icons`.
pub fn variants() -> Vec<(&'static str, Vec<u8>)> {
    [
        ("ember-light", Theme::Ember, false),
        ("ember-dark", Theme::Ember, true),
        ("paper-light", Theme::Paper, false),
        ("paper-dark", Theme::Paper, true),
    ]
    .into_iter()
    .filter_map(|(name, theme, dark)| png(theme, dark).map(|png| (name, png)))
    .collect()
}

/// Draws the icon for `theme` and passes it to `set`.
///
/// Split out from [`sync`] so the theme-to-icon path is testable without an
/// application or a window server: the test supplies its own setter.
pub fn sync_with(theme: Theme, dark: bool, set: impl FnOnce(&[u8])) -> bool {
    match png(theme, dark) {
        Some(bytes) => {
            set(&bytes);
            true
        }
        None => false,
    }
}

/// Re-draws the app icon for the current settings, on the main thread because
/// AppKit only takes a new application icon there.
pub fn sync(app: &tauri::AppHandle, settings: &Settings) {
    let theme = settings.theme;
    let mode = settings.appearance_mode;
    if let Err(error) = app.run_on_main_thread(move || {
        if !sync_with(theme, is_dark(mode), set_application_icon) {
            log::warn!("[icon] could not draw the {theme:?} icon");
        }
    }) {
        log::warn!("[icon] main thread dispatch: {error}");
    }
}

/// What `System` resolves to right now. Only AppKit knows, and it can change
/// while the app runs; the icon is redrawn when the setting changes, not when
/// the OS switches appearance underneath it.
#[cfg(target_os = "macos")]
fn is_dark(mode: AppearanceMode) -> bool {
    match mode {
        AppearanceMode::Light => false,
        AppearanceMode::Dark => true,
        AppearanceMode::System => current_appearance_is_dark(),
    }
}

#[cfg(not(target_os = "macos"))]
fn is_dark(mode: AppearanceMode) -> bool {
    mode == AppearanceMode::Dark
}

#[cfg(target_os = "macos")]
fn set_application_icon(png: &[u8]) {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let Some(mtm) = MainThreadMarker::new() else {
        log::warn!("[icon] not on the main thread");
        return;
    };
    // AppKit decodes the PNG into its own image, so the bytes do not need to
    // outlive this call the way a raw bitmap plane would.
    let data = NSData::with_bytes(png);
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        log::warn!("[icon] AppKit could not decode the icon");
        return;
    };
    unsafe {
        NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image));
    }
}

#[cfg(target_os = "macos")]
fn current_appearance_is_dark() -> bool {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSAppearanceNameDarkAqua};

    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let appearance = NSApplication::sharedApplication(mtm).effectiveAppearance();
    // SAFETY: AppKit owns this string constant for the life of the process.
    let dark = unsafe { NSAppearanceNameDarkAqua };
    appearance.name().isEqualToString(dark)
}

#[cfg(not(target_os = "macos"))]
fn set_application_icon(_png: &[u8]) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(theme: Theme, dark: bool, x: u32, y: u32) -> [u8; 4] {
        let pixmap = pixmap(theme, dark).expect("icon renders");
        let at = ((y * pixmap.width() + x) * 4) as usize;
        let data = pixmap.data();
        [data[at], data[at + 1], data[at + 2], data[at + 3]]
    }

    #[test]
    fn palette_follows_theme_and_appearance() {
        assert_eq!(palette(Theme::Ember, false).accent, [223, 88, 34]);
        assert_eq!(palette(Theme::Ember, true).accent, [246, 125, 81]);
        assert_eq!(palette(Theme::Paper, false).accent, palette(Theme::Paper, false).bars);
        assert_eq!(palette(Theme::Paper, true).plate, [26, 24, 22]);
        // The four themes that share Ember's live orange share its accent.
        for theme in [Theme::Capy, Theme::RosePine, Theme::Neutral] {
            assert_eq!(palette(theme, false), palette(Theme::Ember, false));
            assert_eq!(palette(theme, true), palette(Theme::Ember, true));
        }
    }

    #[test]
    fn the_middle_bar_carries_the_accent_and_the_others_carry_ink() {
        let accent = palette(Theme::Ember, false).accent;
        let middle = pixel(Theme::Ember, false, 256, 256);
        assert_eq!([middle[0], middle[1], middle[2]], accent);
        assert_eq!(middle[3], 255);

        let ink = palette(Theme::Ember, false).bars;
        let neighbour = pixel(Theme::Ember, false, 206, 256);
        assert_eq!([neighbour[0], neighbour[1], neighbour[2]], ink);
    }

    #[test]
    fn plate_and_its_outside_are_as_the_draft_has_them() {
        let plate = palette(Theme::Ember, false).plate;
        let inside = pixel(Theme::Ember, false, 100, 256);
        assert_eq!([inside[0], inside[1], inside[2]], plate);
        // The rounded corner outside the plate is transparent, not paper.
        assert_eq!(pixel(Theme::Ember, false, 2, 2)[3], 0);
    }

    #[test]
    fn paper_is_monochrome_where_ember_is_not() {
        let paper = pixel(Theme::Paper, false, 256, 256);
        assert_eq!([paper[0], paper[1], paper[2]], palette(Theme::Paper, false).bars);
        let ember = pixel(Theme::Ember, false, 256, 256);
        assert_ne!([paper[0], paper[1], paper[2]], [ember[0], ember[1], ember[2]]);
    }

    #[test]
    fn a_theme_change_redraws_and_sets_the_icon() {
        let mut seen: Vec<Vec<u8>> = Vec::new();
        assert!(sync_with(Theme::Ember, false, |png| seen.push(png.to_vec())));
        assert!(sync_with(Theme::Paper, false, |png| seen.push(png.to_vec())));
        assert_eq!(seen.len(), 2, "the setter runs once per theme");
        for png in &seen {
            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        }
        assert_ne!(seen[0], seen[1], "the two themes do not share an icon");
    }
}
