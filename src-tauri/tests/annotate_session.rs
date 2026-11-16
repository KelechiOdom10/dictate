//! A whole Annotate session from stop to package, without Screen Recording
//! permission: a generated MP4 stands in for what `screen::stop` hands over,
//! 20 s of samples for the microphone, three circled marks for the ink, and
//! a fixed word list for whisper. `annotate::package` then has to produce
//! everything the clipboard text points at.
//!
//!   cargo test --test annotate_session -- --nocapture

#![cfg(target_os = "macos")]

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{FPS, HEIGHT, WIDTH, write_test_video};
use dictate_lib::annotate::{self, Recorded, Recording, Started};
use dictate_lib::package::{Display, FrameReason, Rect, Stroke};
use dictate_lib::transcribe::Word;
use tiny_skia::Pixmap;

const SECONDS: u64 = 20;
const SAMPLE_RATE: usize = 16_000;

fn display() -> Display {
    Display {
        id: 1,
        frame: Rect {
            x: 0.0,
            y: 0.0,
            w: 1512.0,
            h: 945.0,
        },
        scale: 2.0,
    }
}

fn circle(display: &Display, t0: u64, t1: u64, [x0, y0, x1, y1]: [f64; 4]) -> Stroke {
    let steps = 40u64;
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let pts = (0..=steps)
        .flat_map(|i| {
            let a = i as f64 / steps as f64 * std::f64::consts::TAU;
            let (x, y) = display.stroke_point(
                display.frame.x + (cx + rx * a.cos()) * display.frame.w,
                display.frame.y + (cy + ry * a.sin()) * display.frame.h,
            );
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
            end_ms: start_ms + 250,
            text: text.to_string(),
        })
        .collect()
}

/// A quiet 220 Hz hum, so the WAV is not all zeros.
fn samples() -> Vec<f32> {
    (0..SECONDS as usize * SAMPLE_RATE)
        .map(|i| 0.01 * (i as f32 * 220.0 * std::f32::consts::TAU / SAMPLE_RATE as f32).sin())
        .collect()
}

#[test]
fn a_stopped_session_with_three_marks_becomes_a_full_package() {
    let temp = tempfile::tempdir().unwrap();
    // Where `screen` leaves the take: its own temp folder.
    let recorder = temp.path().join("dictate-screen-test");
    std::fs::create_dir_all(&recorder).unwrap();
    let mp4 = recorder.join("recording.mp4");
    if let Err(error) = write_test_video(&mp4, SECONDS) {
        eprintln!("SKIP annotate session: cannot write a test MP4 with AVAssetWriter: {error}");
        return;
    }

    let display = display();
    let started = Started {
        unix_ms: 1_794_736_800_000,
        audio_offset_ms: 35,
        video_offset_ms: 140,
        display: display.clone(),
        frontmost_app: None,
    };
    let strokes = vec![
        circle(&display, 2_000, 3_500, [0.10, 0.15, 0.35, 0.40]),
        circle(&display, 8_000, 9_500, [0.60, 0.20, 0.90, 0.55]),
        circle(&display, 14_000, 15_500, [0.30, 0.55, 0.70, 0.85]),
    ];
    let spoken = words(&[
        (3_600, "this"),
        (3_850, "button"),
        (4_100, "is"),
        (4_350, "too"),
        (4_600, "small."),
        (9_600, "and"),
        (9_850, "this"),
        (10_100, "list"),
        (10_350, "is"),
        (10_600, "dense."),
        (15_600, "cut"),
        (15_850, "the"),
        (16_100, "footer."),
    ]);
    let recorded = Recorded {
        started: started.clone(),
        duration_ms: SECONDS * 1000,
        model: "ggml-base.en".into(),
        strokes,
        video: Some(Recording {
            path: mp4.clone(),
            offset_ms: started.video_offset_ms,
            width: WIDTH as u32,
            height: HEIGHT as u32,
            fps: FPS as u32,
            duration_ms: SECONDS * 1000,
        }),
    };
    let annotations = temp.path().join(annotate::FOLDER);
    let folder = annotations.join(annotate::session_id(started.unix_ms));
    let samples = samples();

    let stopped = Instant::now();
    let packaged = annotate::package(&folder, &samples, recorded, stopped, |_| Ok(spoken)).unwrap();
    let stop_to_package = stopped.elapsed();
    eprintln!("{}", packaged.summary);
    eprintln!(
        "stop_to_package_ms={} packaging_ms={:?} render={:?}",
        stop_to_package.as_millis(),
        packaged.annotation.packaging_ms,
        packaged.rendered
    );

    assert!(packaged.warning.is_none(), "{:?}", packaged.warning);
    // The recorder's MP4 moved into the package.
    assert!(!mp4.exists());
    assert!(folder.join(annotate::VIDEO_FILE).is_file());
    let wav = hound::WavReader::open(folder.join(annotate::AUDIO_FILE)).unwrap();
    assert_eq!(wav.spec().sample_rate, 16_000);
    assert_eq!(wav.duration() as usize, samples.len());

    let annotation = &packaged.annotation;
    assert_eq!(annotation.marks.len(), 3);
    assert_eq!(annotation.frames.len(), 3);
    assert!(
        annotation
            .frames
            .iter()
            .all(|frame| frame.reason == FrameReason::Mark)
    );
    assert_eq!(annotation.frames[0].said, "this button is too small.");
    let video = annotation.video.as_ref().unwrap();
    assert_eq!(
        (video.file.as_str(), video.offset_ms),
        ("recording.mp4", 140)
    );
    for frame in &annotation.frames {
        let full = Pixmap::load_png(folder.join(&frame.full)).unwrap();
        assert_eq!((full.width(), full.height()), (WIDTH as u32, HEIGHT as u32));
        let small = Pixmap::load_png(folder.join(&frame.file)).unwrap();
        assert_eq!(small.width().max(small.height()), 1568);
    }
    let sheet = packaged.contact_sheet().unwrap();
    assert!(Pixmap::load_png(&sheet).is_ok());

    // `annotation.json` on disk is the one returned, `packagingMs` included.
    let on_disk: dictate_lib::package::Annotation =
        serde_json::from_slice(&std::fs::read(folder.join(annotate::ANNOTATION_FILE)).unwrap())
            .unwrap();
    assert_eq!(&on_disk, annotation);
    assert!(annotation.packaging_ms.unwrap() <= stop_to_package.as_millis() as u64);

    // The copied text names files that exist, by absolute path.
    let summary = std::fs::read_to_string(folder.join(annotate::SUMMARY_FILE)).unwrap();
    assert_eq!(summary, packaged.summary);
    let sheet_line = format!("Before you answer, open {}:", sheet.display());
    assert!(summary.contains(&sheet_line), "{summary}");
    let first = folder.join("frames/frame-01.png");
    assert!(summary.contains(&format!("{} … frame-03.png", first.display())));
    assert!(summary.contains("- #1 (0:03) circled region"));
    for path in absolute_paths(&summary) {
        assert!(Path::new(&path).exists(), "{path} is named but missing");
    }

    // History lists it, with the contact sheet ready for "Copy image".
    let entries = annotate::list(&annotations).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, annotation.id);
    assert_eq!((entries[0].frames, entries[0].marks), (3, 3));
    assert!(entries[0].has_contact_sheet);
    assert_eq!(entries[0].packaging_ms, annotation.packaging_ms);

    assert!(
        stop_to_package < Duration::from_secs(5),
        "stop to package took {stop_to_package:?}"
    );
}

/// Every `/…​.png` path in `text`.
fn absolute_paths(text: &str) -> Vec<String> {
    text.split([' ', ':', '\n'])
        .filter(|word| word.starts_with('/') && word.ends_with(".png"))
        .map(str::to_string)
        .collect()
}
