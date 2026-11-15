//! Builds a whole Annotate package from a generated 60 s MP4 with four marks:
//! frame count, frame timing, ink placement and the stop-to-package time.
//!
//! The MP4 is written by `common` with AVAssetWriter, so no screen recording
//! permission and no ffmpeg are needed. Each video frame carries its own index
//! as a 10-bit barcode across the top, so the test can tell exactly which frame
//! AVFoundation returned.
//!
//!   cargo test --test annotate_frames -- --nocapture
//!   DICTATE_SAMPLE_PACKAGE=target/annotate-sample cargo test --test annotate_frames
//!
//! The second form keeps the package (contact sheet included) for a look.

#![cfg(target_os = "macos")]

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::{FPS, HEIGHT, WIDTH, read_barcode, write_test_video};
use dictate_lib::frames;
use dictate_lib::package::{self, Audio, Display, FrameReason, Rect, Session, Stroke, Video};
use dictate_lib::transcribe::Word;
use tiny_skia::Pixmap;

const SECONDS: u64 = 60;
const VIDEO_OFFSET_MS: u64 = 120;

/// A second display, left of and above the main one, so stored points are
/// made relative to a non-zero origin. 1512 × 945 pt at 2560 × 1600 px.
fn display() -> Display {
    Display {
        id: 2,
        frame: Rect {
            x: -1512.0,
            y: -200.0,
            w: 1512.0,
            h: 945.0,
        },
        scale: 2.0,
    }
}

/// An ellipse drawn on `display` from global coordinates, closing on itself,
/// filling `[x0, y0, x1, y1]` (fractions of the display).
fn circle(display: &Display, t0: u64, t1: u64, [x0, y0, x1, y1]: [f64; 4]) -> Stroke {
    let steps = 48u64;
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let pts = (0..=steps)
        .flat_map(|i| {
            let a = i as f64 / steps as f64 * std::f64::consts::TAU;
            let fx = cx + rx * a.cos();
            let fy = cy + ry * a.sin();
            let global_x = display.frame.x + fx * display.frame.w;
            let global_y = display.frame.y + fy * display.frame.h;
            let (x, y) = display.stroke_point(global_x, global_y);
            [x, y, ((t1 - t0) * i / steps) as u32]
        })
        .collect();
    Stroke { t0, t1, pts }
}

fn words(spoken: &[(u64, &str)]) -> Vec<Word> {
    spoken
        .iter()
        .map(|&(start_ms, text)| Word {
            start_ms,
            end_ms: start_ms + 280,
            text: text.to_string(),
        })
        .collect()
}

const MARK_BOXES: [[f64; 4]; 4] = [
    [0.10, 0.15, 0.35, 0.40],
    [0.60, 0.20, 0.90, 0.55],
    [0.30, 0.55, 0.70, 0.85],
    [0.05, 0.60, 0.25, 0.95],
];

fn session(display: Display) -> Session {
    let strokes = vec![
        circle(&display, 6_000, 8_000, MARK_BOXES[0]),
        circle(&display, 18_000, 20_500, MARK_BOXES[1]),
        circle(&display, 33_000, 35_000, MARK_BOXES[2]),
        circle(&display, 50_000, 52_000, MARK_BOXES[3]),
    ];
    Session {
        id: "2026-11-15_10-00-00".into(),
        started_at: "2026-11-15T10:00:00.000Z".into(),
        duration_ms: SECONDS * 1000,
        display,
        frontmost_app: None,
        video: Some(Video {
            file: "recording.mp4".into(),
            offset_ms: VIDEO_OFFSET_MS,
            width: WIDTH as u32,
            height: HEIGHT as u32,
            fps: FPS as u32,
            duration_ms: SECONDS * 1000,
        }),
        audio: Audio {
            file: "audio.wav".into(),
            offset_ms: 0,
            sample_rate: 16_000,
        },
        transcript_model: "ggml-base.en".into(),
        words: words(&[
            (8_200, "this"),
            (8_450, "button"),
            (8_700, "is"),
            (8_950, "too"),
            (9_200, "small."),
            (25_000, "now"),
            (25_400, "look"),
            (25_800, "at"),
            (26_100, "the"),
            (26_400, "empty"),
            (26_800, "state."),
            (35_200, "make"),
            (35_450, "this"),
            (35_700, "the"),
            (35_950, "accent"),
            (36_200, "colour."),
            (52_300, "and"),
            (52_600, "cut"),
            (52_900, "this."),
        ]),
        strokes,
        missing_permissions: vec![],
    }
}

#[test]
fn a_generated_minute_with_four_marks_becomes_a_package() {
    let keep = std::env::var_os("DICTATE_SAMPLE_PACKAGE").map(PathBuf::from);
    let temp = tempfile::tempdir().unwrap();
    let folder = keep.clone().unwrap_or_else(|| temp.path().join("package"));
    std::fs::create_dir_all(&folder).unwrap();
    let video = folder.join("recording.mp4");

    let written = Instant::now();
    if let Err(error) = write_test_video(&video, SECONDS) {
        eprintln!("SKIP annotate frames: cannot write a test MP4 with AVAssetWriter: {error}");
        return;
    }
    eprintln!(
        "test video: {WIDTH}x{HEIGHT} {FPS} fps {SECONDS} s, {} bytes, written in {} ms",
        std::fs::metadata(&video).unwrap().len(),
        written.elapsed().as_millis()
    );

    let display = display();
    let session = session(display.clone());

    // Stop-to-package: everything after the recorder hands over the session
    // and the finished MP4. Transcription is timed separately (spike-results).
    let stopped = Instant::now();
    let annotation = package::build(session);
    let summary = package::write_package(&folder, &annotation).unwrap();
    let rendered = frames::render(&folder, &annotation, &video).unwrap();
    let stop_to_package = stopped.elapsed();

    eprintln!("{summary}");
    eprintln!(
        "stop_to_package_ms={} extract_ms={} frames_ms={} sheet_ms={} frames={} full={:?} small={:?} sheet={:?} time_errors_ms={:?}",
        stop_to_package.as_millis(),
        rendered.extract_ms,
        rendered.frames_ms,
        rendered.sheet_ms,
        rendered.frames,
        rendered.full_size,
        rendered.frame_size,
        rendered.sheet_size,
        rendered.time_errors_ms,
    );

    let reasons: Vec<FrameReason> = annotation.frames.iter().map(|f| f.reason).collect();
    assert_eq!(
        reasons,
        [
            FrameReason::Mark,
            FrameReason::Mark,
            FrameReason::Speech,
            FrameReason::Mark,
            FrameReason::Mark
        ]
    );
    assert_eq!(rendered.frames, 5);
    assert_eq!(rendered.full_size, (WIDTH as u32, HEIGHT as u32));
    assert_eq!(rendered.frame_size, (1568, 980));
    assert!(rendered.sheet_size.0.max(rendered.sheet_size.1) <= frames::SHEET_MAX_EDGE);
    assert_eq!(annotation.frames[0].said, "this button is too small.");

    for frame in &annotation.frames {
        let full = Pixmap::load_png(folder.join(&frame.full)).unwrap();
        let small = Pixmap::load_png(folder.join(&frame.file)).unwrap();
        assert_eq!((small.width(), small.height()), (1568, 980));

        let video_ms = frame.t - VIDEO_OFFSET_MS;
        let expected_index = video_ms * FPS / 1000;
        let shown = read_barcode(&full);
        assert!(
            shown.abs_diff(expected_index) <= 1,
            "frame {} at {video_ms} ms shows video frame {shown}, expected {expected_index}",
            frame.n
        );

        let ink = ink_bounds(&full);
        let marks: Vec<[f64; 4]> = frame
            .marks
            .iter()
            .map(|id| {
                annotation
                    .marks
                    .iter()
                    .find(|m| &m.id == id)
                    .unwrap()
                    .bounds
            })
            .collect();
        match (marks.as_slice(), ink) {
            ([], None) => {}
            ([], Some(ink)) => panic!("speech frame {} has ink at {ink:?}", frame.n),
            (_, None) => panic!("frame {} lists marks but has no ink", frame.n),
            (boxes, Some(ink)) => {
                let expected = boxes.iter().fold([1.0_f64, 1.0, 0.0, 0.0], |acc, b| {
                    [
                        acc[0].min(b[0]),
                        acc[1].min(b[1]),
                        acc[2].max(b[2]),
                        acc[3].max(b[3]),
                    ]
                });
                for side in 0..4 {
                    assert!(
                        (ink[side] - expected[side]).abs() <= 0.01,
                        "frame {}: ink {ink:?} vs mark box {expected:?}",
                        frame.n
                    );
                }
            }
        }
    }

    let sheet = Pixmap::load_png(folder.join(&annotation.contact_sheet)).unwrap();
    assert_eq!((sheet.width(), sheet.height()), rendered.sheet_size);

    assert!(
        stop_to_package < Duration::from_secs(5),
        "stop to package took {stop_to_package:?}"
    );
    if let Some(keep) = keep {
        eprintln!("package kept at {}", keep.display());
    }
}

/// The session clock runs on past the end of the MP4 (the recorder stops a
/// little after the last frame). A speech frame at the session's end lands
/// past the video, and must come out as the last frame instead of failing
/// the whole render.
#[test]
fn a_session_longer_than_its_video_still_renders_every_frame() {
    const VIDEO_SECONDS: u64 = 5;
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("package");
    std::fs::create_dir_all(&folder).unwrap();
    let video = folder.join("recording.mp4");
    if let Err(error) = write_test_video(&video, VIDEO_SECONDS) {
        eprintln!("SKIP annotate frames: cannot write a test MP4 with AVAssetWriter: {error}");
        return;
    }

    let display = display();
    let mut session = session(display.clone());
    session.duration_ms = 6_500;
    session.strokes = vec![circle(&display, 1_000, 2_000, MARK_BOXES[0])];
    session.words = words(&[(5_800, "the"), (6_000, "end.")]);
    if let Some(video) = session.video.as_mut() {
        video.duration_ms = VIDEO_SECONDS * 1000;
    }

    let annotation = package::build(session);
    let last = annotation.frames.last().unwrap();
    assert_eq!(last.reason, FrameReason::Speech);
    assert!(
        last.t - VIDEO_OFFSET_MS > VIDEO_SECONDS * 1000,
        "the speech frame at {} ms should fall past the video",
        last.t
    );

    package::write_package(&folder, &annotation).unwrap();
    let rendered = frames::render(&folder, &annotation, &video).unwrap();
    assert_eq!(rendered.frames, annotation.frames.len());
    assert!(folder.join(&annotation.contact_sheet).is_file());

    let full = Pixmap::load_png(folder.join(&last.full)).unwrap();
    let last_index = VIDEO_SECONDS * FPS - 1;
    let shown = read_barcode(&full);
    assert!(
        shown.abs_diff(last_index) <= 1,
        "frame past the end shows video frame {shown}, expected the last ({last_index})"
    );
}

/// Bounds of the red ink as fractions `[x0, y0, x1, y1]`, or `None`.
fn ink_bounds(pixmap: &Pixmap) -> Option<[f64; 4]> {
    let (w, h) = (pixmap.width(), pixmap.height());
    let mut bounds: Option<[u32; 4]> = None;
    for (i, p) in pixmap.pixels().iter().enumerate() {
        if p.red() > 200 && p.green() < 120 && p.blue() < 120 {
            let (x, y) = (i as u32 % w, i as u32 / w);
            bounds = Some(match bounds {
                None => [x, y, x, y],
                Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
            });
        }
    }
    bounds.map(|[x0, y0, x1, y1]| {
        [
            f64::from(x0) / f64::from(w),
            f64::from(y0) / f64::from(h),
            f64::from(x1 + 1) / f64::from(w),
            f64::from(y1 + 1) / f64::from(h),
        ]
    })
}
