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

/// The colour the middle bar carries: the theme's **identity** colour.
///
/// The rule: the icon paints the theme's identity colour, and the focus ring is
/// only its contrast-safe variant, which is why the two coincide for four of the
/// five themes.
///
/// - **Ember's identity colour is its live/recording orange**, the `live` role
///   for the Ember overlay palette in `src/ui/palettes.generated.json`. Not its
///   focus ring: that ring is deliberately desaturated so it reads against
///   surfaces, while draft A was chosen for "five ink bars, the middle one
///   ember", so the ember has to stay ember.
/// - **Paper, Capy, Rose Pine and Neutral** have no separate identity colour, so
///   their focus ring is it: `--focus-outline-color` in
///   `src/themes/<slug>/<slug>.css`, generated from the accent dial in
///   `themes/<slug>/<slug>.settings.json`.
///
/// `scripts/check-icon-colours.mjs` holds each half to its own source, so this
/// table cannot drift from a re-dialled theme or from the overlay palette.
const IDENTITY: [(Theme, [u8; 3], [u8; 3]); 5] = [
    // #DF5822 light, #F67D51 dark: ember's `live` role.
    (Theme::Ember, [223, 88, 34], [246, 125, 81]),
    // #1c1b19 / #ababab: paper's focus ring, which is its ink.
    (Theme::Paper, [28, 27, 25], [171, 171, 171]),
    // #8372d3 / #a99cf5: capy's lavender ring.
    (Theme::Capy, [131, 114, 211], [169, 156, 245]),
    // #966dc3 / #c4a7e7: rose pine's iris ring.
    (Theme::RosePine, [150, 109, 195], [196, 167, 231]),
    // #71717a in both appearances: neutral's grey ring.
    (Theme::Neutral, [113, 113, 122], [113, 113, 122]),
];

/// The plate and the ink are shared by every theme, from the `canvas` and `text`
/// roles in `src/ui/palettes.generated.json`; the accent is the theme's identity
/// colour, documented on [`IDENTITY`].
pub fn palette(theme: Theme, dark: bool) -> Palette {
    let (plate, bars) = if dark {
        ([26, 24, 22], [241, 238, 234])
    } else {
        ([251, 250, 248], [28, 26, 24])
    };
    let accent = IDENTITY
        .iter()
        .find(|(candidate, ..)| *candidate == theme)
        .map(|(_, light, dark_colour)| if dark { *dark_colour } else { *light })
        .unwrap_or(bars);
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
        ("capy-light", Theme::Capy, false),
        ("capy-dark", Theme::Capy, true),
        ("rose-pine-light", Theme::RosePine, false),
        ("rose-pine-dark", Theme::RosePine, true),
        ("neutral-light", Theme::Neutral, false),
        ("neutral-dark", Theme::Neutral, true),
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
    fn every_theme_has_its_own_identity_colour() {
        let light: Vec<[u8; 3]> = [
            Theme::Ember,
            Theme::Paper,
            Theme::Capy,
            Theme::RosePine,
            Theme::Neutral,
        ]
        .into_iter()
        .map(|theme| palette(theme, false).accent)
        .collect();
        assert_eq!(light[0], [223, 88, 34], "ember stays ember, not its brown ring");
        assert_eq!(light[2], [131, 114, 211], "capy is lavender");
        assert_eq!(light[3], [150, 109, 195], "rose pine is iris");
        for (index, colour) in light.iter().enumerate() {
            for other in &light[index + 1..] {
                assert_ne!(colour, other, "two themes share an accent: {light:?}");
            }
        }
    }

    #[test]
    fn dark_appearance_uses_the_dark_identity_colour() {
        assert_eq!(palette(Theme::Ember, true).accent, [246, 125, 81]);
        assert_eq!(palette(Theme::Capy, true).accent, [169, 156, 245]);
        assert_eq!(palette(Theme::RosePine, true).accent, [196, 167, 231]);
        // Paper has no identity colour of its own, so its grey ring is it.
        assert_eq!(palette(Theme::Paper, true).accent, [171, 171, 171]);
    }

    #[test]
    fn paper_stays_monochrome() {
        let paper = palette(Theme::Paper, false);
        for (accent, ink) in paper.accent.iter().zip(paper.bars) {
            assert!(accent.abs_diff(ink) <= 2, "paper's ring is its ink");
        }
        assert_ne!(palette(Theme::Ember, false).accent, paper.bars);
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
