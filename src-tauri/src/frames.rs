//! Annotate frames stage: the key frames the package builder chose, pulled out
//! of the recording, with the ink drawn in, plus a numbered contact sheet.
//!
//!   frames::render(folder, &annotation, video)  ->  Rendered
//!
//! Writes, at the paths `annotation` names:
//!   frames-full/frame-NN.png   video resolution, with ink
//!   frames/frame-NN.png        long edge ≤ FRAME_LONG_EDGE, with ink
//!   contact-sheet.png          every frame in a grid, numbered like summary.md
//!
//! Frames come out of the MP4 through AVFoundation (`extract`). Ink, scaling
//! and the sheet are drawn with tiny-skia, whose pixel format (premultiplied
//! RGBA) is what the extractor writes into, so there is one copy per frame.

mod extract;
mod sheet;

pub use sheet::{Area, Cell, SHEET_MAX_EDGE, SheetLayout, layout};

use std::borrow::Cow;
use std::path::Path;
use std::time::Instant;

use tiny_skia::{
    FillRule, FilterQuality, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, Transform,
};

use crate::package::{Annotation, Display, Frame, POINT_SCALE, Stroke, Video};

/// Long edge of `frames/`: the image-size guidance of common chat models
/// (design doc §5).
pub const FRAME_LONG_EDGE: u32 = 1568;
pub const INK_RGB: [u8; 3] = [0xFF, 0x3B, 0x30];
/// Ink width in display points, so it looks the same at any resolution.
pub const INK_WIDTH_PT: f64 = 4.0;
/// Contact-sheet cells are ~580 px wide, where 4 pt comes out near 1.5 px and
/// the ink is hard to see, so each cell's ink is redrawn at least this wide.
pub const SHEET_INK_MIN_PX: f32 = 3.0;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rendered {
    pub frames: usize,
    pub full_size: (u32, u32),
    pub frame_size: (u32, u32),
    pub sheet_size: (u32, u32),
    /// Delivered minus requested video time, per frame, in ms.
    pub time_errors_ms: Vec<i64>,
    pub extract_ms: u64,
    /// Ink, scaling and PNG writing for every frame, run in parallel.
    pub frames_ms: u64,
    pub sheet_ms: u64,
}

/// `video` is the session's MP4. Frame times are on the session clock and are
/// moved to the video's clock with `video_time`.
pub fn render(folder: &Path, annotation: &Annotation, video: &Path) -> Result<Rendered, String> {
    if annotation.frames.is_empty() {
        return Ok(Rendered::default());
    }
    let times: Vec<u64> = annotation
        .frames
        .iter()
        .map(|frame| video_time(frame.t, annotation.video.as_ref()))
        .collect();

    let started = Instant::now();
    let extracted = extract::frames(video, &times)?;
    let extract_ms = elapsed_ms(started);
    let time_errors_ms = extracted
        .iter()
        .zip(&times)
        .map(|(image, &asked)| image.actual_ms as i64 - asked as i64)
        .collect();
    let full_size = extracted
        .first()
        .map_or((0, 0), |e| (e.pixmap.width(), e.pixmap.height()));

    for frame in &annotation.frames {
        for file in [&frame.file, &frame.full] {
            if let Some(parent) = folder.join(file).parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Create {}: {e}", parent.display()))?;
            }
        }
    }

    let started = Instant::now();
    let small: Vec<Pixmap> = std::thread::scope(|scope| {
        let workers: Vec<_> = annotation
            .frames
            .iter()
            .zip(extracted)
            .map(|(frame, image)| {
                scope.spawn(move || finish_frame(folder, annotation, frame, image.pixmap))
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .map_err(|_| "A frame worker panicked".to_string())?
            })
            .collect::<Result<Vec<_>, String>>()
    })?;
    let frames_ms = elapsed_ms(started);

    let started = Instant::now();
    let cells: Vec<sheet::Frame> = annotation
        .frames
        .iter()
        .zip(&small)
        .map(|(frame, image)| sheet::Frame {
            image,
            number: frame.n,
            strokes: frame_strokes(annotation, frame),
        })
        .collect();
    let contact_sheet = sheet::render(&cells, &annotation.display)?;
    save_png(&contact_sheet, &folder.join(&annotation.contact_sheet))?;
    let sheet_ms = elapsed_ms(started);

    Ok(Rendered {
        frames: small.len(),
        full_size,
        frame_size: small.first().map_or((0, 0), |p| (p.width(), p.height())),
        sheet_size: (contact_sheet.width(), contact_sheet.height()),
        time_errors_ms,
        extract_ms,
        frames_ms,
        sheet_ms,
    })
}

/// A session time as a time in the MP4: the video's offset taken off, then
/// held at its last frame. The session can outlast the recording by a few
/// hundred ms, and AVFoundation fails a request past the end outright.
pub fn video_time(t: u64, video: Option<&Video>) -> u64 {
    let Some(video) = video else {
        return t;
    };
    let frame_ms = 1000_u64.div_ceil(u64::from(video.fps.max(1)));
    t.saturating_sub(video.offset_ms)
        .min(video.duration_ms.saturating_sub(frame_ms))
}

fn finish_frame(
    folder: &Path,
    annotation: &Annotation,
    frame: &Frame,
    mut full: Pixmap,
) -> Result<Pixmap, String> {
    draw_ink(
        &mut full,
        &annotation.display,
        &frame_strokes(annotation, frame),
    );
    save_png(&full, &folder.join(&frame.full))?;
    let (width, height) = fit(full.width(), full.height(), FRAME_LONG_EDGE);
    let small = resize(&full, width, height).ok_or("Scale a frame down")?;
    save_png(&small, &folder.join(&frame.file))?;
    Ok(small)
}

/// The strokes of every mark the frame lists, and nothing else (see
/// `package::Frame::marks`). A speech frame lists no marks, so has no ink.
pub fn frame_strokes<'a>(annotation: &'a Annotation, frame: &Frame) -> Vec<&'a Stroke> {
    frame
        .marks
        .iter()
        .filter_map(|id| annotation.marks.iter().find(|mark| &mark.id == id))
        .flat_map(|mark| mark.strokes.iter())
        .filter_map(|&index| annotation.strokes.get(index))
        .collect()
}

/// A stored point (fractions of the recorded display ×10,000) in pixels of an
/// image of that display. The display's origin plays no part: points were
/// made relative to it when stored (`Display::stroke_point`).
pub fn to_pixel(x: u32, y: u32, width: u32, height: u32) -> (f32, f32) {
    (
        (f64::from(x) / POINT_SCALE * f64::from(width)) as f32,
        (f64::from(y) / POINT_SCALE * f64::from(height)) as f32,
    )
}

/// `INK_WIDTH_PT` scaled from display points to the image's pixels.
pub fn ink_width(display: &Display, image_width: u32) -> f32 {
    let points_wide = if display.frame.w > 0.0 {
        display.frame.w
    } else {
        f64::from(image_width)
    };
    (INK_WIDTH_PT * f64::from(image_width) / points_wide) as f32
}

pub fn draw_ink(pixmap: &mut Pixmap, display: &Display, strokes: &[&Stroke]) {
    let width = ink_width(display, pixmap.width());
    draw_strokes(pixmap, strokes, width);
}

/// `strokes` in `INK_RGB`, `width` pixels wide, round-capped. A stroke that
/// never moved is a dot.
pub fn draw_strokes(pixmap: &mut Pixmap, strokes: &[&Stroke], width: f32) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(INK_RGB[0], INK_RGB[1], INK_RGB[2], 255);
    paint.anti_alias = true;
    let style = tiny_skia::Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    let (w, h) = (pixmap.width(), pixmap.height());
    for stroke in strokes {
        let points: Vec<(f32, f32)> = stroke
            .pts
            .as_chunks::<3>()
            .0
            .iter()
            .map(|&[x, y, _]| to_pixel(x, y, w, h))
            .collect();
        let Some(&(x0, y0)) = points.first() else {
            continue;
        };
        if points.iter().all(|&p| p == (x0, y0)) {
            if let Some(dot) = PathBuilder::from_circle(x0, y0, width / 2.0) {
                pixmap.fill_path(&dot, &paint, FillRule::Winding, Transform::identity(), None);
            }
            continue;
        }
        let mut path = PathBuilder::new();
        path.move_to(x0, y0);
        for &(x, y) in &points[1..] {
            path.line_to(x, y);
        }
        if let Some(path) = path.finish() {
            pixmap.stroke_path(&path, &paint, &style, Transform::identity(), None);
        }
    }
}

/// The largest size with the same aspect ratio whose long edge is at most
/// `long_edge`. Never scales up.
pub fn fit(width: u32, height: u32, long_edge: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= long_edge || longest == 0 {
        return (width, height);
    }
    let scale = |side: u32| {
        ((u64::from(side) * u64::from(long_edge) + u64::from(longest) / 2) / u64::from(longest))
            .max(1) as u32
    };
    (scale(width), scale(height))
}

/// Halves with a 2×2 average until within 2× of the target, then finishes
/// with bicubic. One bicubic step from 2560 to 600 px skips most source pixels
/// and makes small text shimmer.
pub fn resize(source: &Pixmap, width: u32, height: u32) -> Option<Pixmap> {
    let mut current = Cow::Borrowed(source);
    while current.width() >= width * 2 && current.height() >= height * 2 {
        let half = scale_to(
            &current,
            current.width() / 2,
            current.height() / 2,
            FilterQuality::Bilinear,
        )?;
        current = Cow::Owned(half);
    }
    if (current.width(), current.height()) == (width, height) {
        return Some(current.into_owned());
    }
    scale_to(&current, width, height, FilterQuality::Bicubic)
}

fn scale_to(source: &Pixmap, width: u32, height: u32, quality: FilterQuality) -> Option<Pixmap> {
    let mut out = Pixmap::new(width, height)?;
    out.draw_pixmap(
        0,
        0,
        source.as_ref(),
        &PixmapPaint {
            quality,
            ..PixmapPaint::default()
        },
        Transform::from_scale(
            width as f32 / source.width() as f32,
            height as f32 / source.height() as f32,
        ),
        None,
    );
    Some(out)
}

fn save_png(pixmap: &Pixmap, path: &Path) -> Result<(), String> {
    pixmap
        .save_png(path)
        .map_err(|e| format!("Write {}: {e}", path.display()))
}

fn elapsed_ms(since: Instant) -> u64 {
    since.elapsed().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::Rect;

    fn display(x: f64, y: f64, w: f64, h: f64) -> Display {
        Display {
            id: 2,
            frame: Rect { x, y, w, h },
            scale: 2.0,
        }
    }

    #[test]
    fn stored_points_map_to_pixels_at_any_resolution() {
        assert_eq!(to_pixel(0, 0, 2560, 1600), (0.0, 0.0));
        assert_eq!(to_pixel(10_000, 10_000, 2560, 1600), (2560.0, 1600.0));
        assert_eq!(to_pixel(2_500, 5_000, 2560, 1600), (640.0, 800.0));
        assert_eq!(to_pixel(2_500, 5_000, 1280, 800), (320.0, 400.0));
    }

    #[test]
    fn a_point_on_a_second_display_with_a_non_zero_origin_lands_on_its_frame() {
        let left_and_above = display(-1920.0, -300.0, 1920.0, 1080.0);
        let (x, y) = left_and_above.stroke_point(-1440.0, 240.0);
        assert_eq!((x, y), (2_500, 5_000));
        assert_eq!(to_pixel(x, y, 3840, 2160), (960.0, 1080.0));

        let right = display(1512.0, 0.0, 2560.0, 1440.0);
        assert_eq!(right.stroke_point(1512.0 + 1280.0, 720.0), (5_000, 5_000));
        assert_eq!(right.stroke_point(1000.0, 2000.0), (0, 10_000), "clamped");
    }

    #[test]
    fn ink_width_is_four_points_at_the_image_scale() {
        let main = display(0.0, 0.0, 1512.0, 982.0);
        assert!((ink_width(&main, 3024) - 8.0).abs() < 1e-4);
        assert!((ink_width(&main, 1512) - 4.0).abs() < 1e-4);
        assert!((ink_width(&main, 1568) - 4.148).abs() < 1e-3);
    }

    #[test]
    fn fit_keeps_the_aspect_ratio_and_never_scales_up() {
        assert_eq!(fit(2560, 1600, 1568), (1568, 980));
        assert_eq!(fit(1600, 2560, 1568), (980, 1568));
        assert_eq!(fit(1200, 800, 1568), (1200, 800));
        assert_eq!(fit(0, 0, 1568), (0, 0));
    }

    #[test]
    fn ink_lands_inside_the_marked_box() {
        let mut pixmap = Pixmap::new(400, 250).unwrap();
        let stroke = Stroke {
            t0: 0,
            t1: 500,
            pts: vec![2_500, 2_000, 0, 7_500, 2_000, 100, 7_500, 8_000, 200],
        };
        draw_ink(&mut pixmap, &display(0.0, 0.0, 400.0, 250.0), &[&stroke]);
        let red: Vec<(u32, u32)> = (0..250)
            .flat_map(|y| (0..400).map(move |x| (x, y)))
            .filter(|&(x, y)| pixmap.pixel(x, y).is_some_and(|p| p.red() > 200))
            .collect();
        let bounds = (
            red.iter().map(|p| p.0).min().unwrap(),
            red.iter().map(|p| p.1).min().unwrap(),
            red.iter().map(|p| p.0).max().unwrap(),
            red.iter().map(|p| p.1).max().unwrap(),
        );
        // The path spans x 100–300, y 50–200 and the ink is 4 px wide with round
        // caps, so it covers 98.0–302.0: pixels 98 to 301.
        assert_eq!(bounds, (98, 48, 301, 201));
    }

    #[test]
    fn video_time_takes_off_the_offset_and_stays_before_the_last_frame() {
        let video = Video {
            file: "recording.mp4".into(),
            offset_ms: 120,
            width: 2560,
            height: 1600,
            fps: 15,
            duration_ms: 60_520,
        };
        assert_eq!(video_time(5_120, Some(&video)), 5_000);
        assert_eq!(video_time(60, Some(&video)), 0);
        assert_eq!(video_time(61_500, Some(&video)), 60_453);
        assert_eq!(video_time(60_573, Some(&video)), 60_453);
        assert_eq!(video_time(61_500, None), 61_500);
    }

    #[test]
    fn resize_reaches_the_exact_size() {
        let source = Pixmap::new(2560, 1600).unwrap();
        for (w, h) in [(1568, 980), (580, 363), (2560, 1600), (37, 23)] {
            let out = resize(&source, w, h).unwrap();
            assert_eq!((out.width(), out.height()), (w, h));
        }
    }
}
