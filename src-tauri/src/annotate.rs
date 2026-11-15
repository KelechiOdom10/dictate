//! Annotate session, audio only: tap the Annotate shortcut to start, tap again
//! to stop. Stopping writes a folder with `audio.wav` and `annotation.json`.
//!
//! The rules here are pure: the phase a tap moves to, whether dictation may
//! start, and how a finished recording becomes an `Annotation`. `lib.rs` holds
//! the phase, calls `audio` and `transcribe`, and drives the pill. Ink drawn
//! during the session comes from `ink.rs`. Screen video and frames come in
//! later stages.
//!
//! `audio.rs` keeps one shared recording, so Annotate and dictation must never
//! record at the same time: a dictation `stop()` would take Annotate's audio.

use std::io;
use std::path::Path;

use crate::package::{self, Annotation, Audio, Display, Session, Stroke};
use crate::transcribe::Word;

/// Session folders live in `<app data>/annotations/<id>/`.
pub const FOLDER: &str = "annotations";
pub const AUDIO_FILE: &str = "audio.wav";
pub const ANNOTATION_FILE: &str = "annotation.json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    /// The microphone belongs to Annotate.
    Recording,
    /// Recording stopped; the WAV, transcript and JSON are being written.
    Packaging,
}

/// The phase after a tap of the Annotate shortcut. A session only starts when
/// dictation is idle, and a tap while packaging does nothing.
pub fn tap(phase: Phase, dictation_idle: bool) -> Phase {
    match phase {
        Phase::Idle if dictation_idle => Phase::Recording,
        Phase::Idle => Phase::Idle,
        Phase::Recording => Phase::Packaging,
        Phase::Packaging => Phase::Packaging,
    }
}

/// The phase once the package is written, whether or not that succeeded.
pub fn packaged(phase: Phase) -> Phase {
    match phase {
        Phase::Packaging => Phase::Idle,
        other => other,
    }
}

/// Dictation's shortcut does nothing until a session is fully finished: while
/// recording it would steal Annotate's audio, and while packaging the pill is
/// busy showing the save.
pub fn blocks_dictation(phase: Phase) -> bool {
    phase != Phase::Idle
}

pub const DICTATION_BLOCKED: &str = "Dictation is off while Annotate records";
pub const DICTATION_BUSY: &str = "Finish dictating before starting Annotate";

/// What the shell knows when a session starts. Times are on the session clock:
/// 0 is the moment the start tap was handled.
#[derive(Clone, Debug)]
pub struct Started {
    pub unix_ms: i64,
    /// How long `audio::start` took to return. Words are shifted by this.
    pub audio_offset_ms: u64,
    pub display: Display,
    pub frontmost_app: Option<package::App>,
}

/// The `annotation.json` for a session without video: times, duration, the
/// transcript and the ink, through the package builder, which groups strokes
/// into marks. No video means no frames, so the builder's frames are dropped
/// rather than pointing at missing images.
pub fn annotation(
    started: Started,
    duration_ms: u64,
    model: String,
    words: Vec<Word>,
    strokes: Vec<Stroke>,
) -> Annotation {
    let mut annotation = package::build(Session {
        id: session_id(started.unix_ms),
        started_at: rfc3339(started.unix_ms),
        duration_ms,
        display: started.display,
        frontmost_app: started.frontmost_app,
        video: None,
        audio: Audio {
            file: AUDIO_FILE.to_string(),
            offset_ms: started.audio_offset_ms,
            sample_rate: 16_000,
        },
        transcript_model: model,
        words,
        strokes,
        missing_permissions: Vec::new(),
    });
    annotation.frames.clear();
    annotation
}

/// Writes `audio.wav` and `annotation.json` into `folder`, creating it.
pub fn write(folder: &Path, samples: &[f32], annotation: &Annotation) -> io::Result<()> {
    std::fs::create_dir_all(folder)?;
    crate::audio::save_wav(&folder.join(AUDIO_FILE), samples).map_err(io::Error::other)?;
    let mut json = serde_json::to_string_pretty(annotation).map_err(io::Error::other)?;
    json.push('\n');
    std::fs::write(folder.join(ANNOTATION_FILE), json)
}

/// `2026-11-14_14-56-36`, UTC, the folder name and `id`.
pub fn session_id(unix_ms: i64) -> String {
    let (date, (h, m, s), _) = utc(unix_ms);
    format!("{date}_{h:02}-{m:02}-{s:02}")
}

/// `2026-11-14T14:56:36.159Z`.
pub fn rfc3339(unix_ms: i64) -> String {
    let (date, (h, m, s), ms) = utc(unix_ms);
    format!("{date}T{h:02}:{m:02}:{s:02}.{ms:03}Z")
}

fn utc(unix_ms: i64) -> (String, (i64, i64, i64), i64) {
    let ms = unix_ms.rem_euclid(1000);
    let secs = unix_ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let day_secs = secs.rem_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    (
        format!("{y:04}-{mo:02}-{d:02}"),
        (day_secs / 3600, day_secs % 3600 / 60, day_secs % 60),
        ms,
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's
/// `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The display under the pointer, which is the one Annotate records and draws
/// on. Falls back to the main display.
#[cfg(target_os = "macos")]
pub fn recorded_display() -> Display {
    use core_graphics::display::CGDisplay;
    let displays: Vec<Display> = CGDisplay::active_displays()
        .unwrap_or_default()
        .into_iter()
        .map(|id| display(CGDisplay::new(id)))
        .collect();
    let (x, y) = crate::annotate_ui::pointer().unwrap_or((0.0, 0.0));
    crate::ink::display_at(&displays, x, y)
        .cloned()
        .unwrap_or_else(|| display(CGDisplay::main()))
}

/// A display in global points, as the package records it.
#[cfg(target_os = "macos")]
fn display(display: core_graphics::display::CGDisplay) -> Display {
    let bounds = display.bounds();
    let scale = if bounds.size.width > 0.0 {
        display.pixels_wide() as f64 / bounds.size.width
    } else {
        1.0
    };
    Display {
        id: display.id,
        frame: package::Rect {
            x: bounds.origin.x,
            y: bounds.origin.y,
            w: bounds.size.width,
            h: bounds.size.height,
        },
        scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_starts_tap_stops_and_packaging_returns_to_idle() {
        let mut phase = Phase::Idle;
        phase = tap(phase, true);
        assert_eq!(phase, Phase::Recording);
        phase = tap(phase, true);
        assert_eq!(phase, Phase::Packaging);
        phase = packaged(phase);
        assert_eq!(phase, Phase::Idle);
    }

    #[test]
    fn taps_while_packaging_are_ignored() {
        assert_eq!(tap(Phase::Packaging, true), Phase::Packaging);
        assert_eq!(tap(Phase::Packaging, false), Phase::Packaging);
    }

    #[test]
    fn a_session_cannot_start_while_dictating() {
        assert_eq!(tap(Phase::Idle, false), Phase::Idle);
    }

    #[test]
    fn packaged_only_moves_out_of_packaging() {
        assert_eq!(packaged(Phase::Idle), Phase::Idle);
        assert_eq!(packaged(Phase::Recording), Phase::Recording);
    }

    #[test]
    fn dictation_is_blocked_until_the_session_is_idle() {
        assert!(!blocks_dictation(Phase::Idle));
        assert!(blocks_dictation(Phase::Recording));
        assert!(blocks_dictation(Phase::Packaging));
    }

    #[test]
    fn timestamps_are_utc() {
        // 2026-11-14T14:56:36.159Z
        let ms = 1_794_668_196_159;
        assert_eq!(session_id(ms), "2026-11-14_14-56-36");
        assert_eq!(rfc3339(ms), "2026-11-14T14:56:36.159Z");
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        // Leap day.
        assert_eq!(session_id(1_709_210_096_000), "2024-02-29_12-34-56");
    }

    fn started() -> Started {
        Started {
            unix_ms: 1_794_668_196_159,
            audio_offset_ms: 40,
            display: Display {
                id: 1,
                frame: package::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 1512.0,
                    h: 982.0,
                },
                scale: 2.0,
            },
            frontmost_app: None,
        }
    }

    fn word(start_ms: u64, end_ms: u64, text: &str) -> Word {
        Word {
            start_ms,
            end_ms,
            text: text.to_string(),
        }
    }

    #[test]
    fn audio_only_annotation_has_times_and_transcript_and_no_frames() {
        let words = vec![
            word(500, 800, "This"),
            word(800, 1000, "button"),
            word(1000, 1400, "overlaps."),
            word(7000, 7400, "Fix"),
            word(7400, 7800, "it."),
        ];
        let annotation = annotation(started(), 10_000, "ggml-base.en".into(), words, Vec::new());

        assert_eq!(annotation.id, "2026-11-14_14-56-36");
        assert_eq!(annotation.started_at, "2026-11-14T14:56:36.159Z");
        assert_eq!(annotation.duration_ms, 10_000);
        assert_eq!(annotation.audio.file, "audio.wav");
        assert_eq!(annotation.audio.offset_ms, 40);
        assert!(annotation.video.is_none());
        assert!(annotation.frames.is_empty());
        assert!(annotation.strokes.is_empty() && annotation.marks.is_empty());
        assert_eq!(annotation.transcript.model, "ggml-base.en");
        assert_eq!(annotation.transcript.text, "This button overlaps. Fix it.");
        assert_eq!(annotation.transcript.segments.len(), 2);
        // Words move from the audio clock to the session clock.
        assert_eq!(annotation.transcript.words[0].t0, 540);
        assert_eq!(annotation.transcript.words[4].t1, 7840);
    }

    #[test]
    fn strokes_drawn_during_the_session_reach_annotation_json_as_marks() {
        // Three overlapping strokes 1 s apart, drawn on the recorded display.
        let mut ink = crate::ink::Ink::new(started().display);
        for (t, cx) in [(2_000, 400.0), (3_000, 420.0), (4_000, 440.0)] {
            ink.press(t, cx, 300.0);
            ink.sample(t + 100, cx + 200.0, 300.0);
            ink.sample(t + 200, cx + 200.0, 500.0);
            ink.sample(t + 300, cx, 500.0);
            ink.sample(t + 400, cx + 5.0, 305.0);
            ink.release(t + 500);
        }
        let annotation = annotation(
            started(),
            10_000,
            "ggml-base.en".into(),
            Vec::new(),
            ink.finish(9_000),
        );

        assert_eq!(annotation.strokes.len(), 3);
        assert_eq!(annotation.strokes[0].t0, 2_000);
        assert_eq!(&annotation.strokes[0].pts[..3], &[2_646, 3_055, 0]);
        assert_eq!(annotation.marks.len(), 1);
        assert_eq!(annotation.marks[0].strokes, vec![0, 1, 2]);
        assert_eq!(annotation.marks[0].shape, package::Shape::Circle);
        let json = serde_json::to_value(&annotation).unwrap();
        assert_eq!(json["strokes"][2]["t1"], 4_500);
    }

    #[test]
    fn write_creates_the_folder_with_a_16k_wav_and_json() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join(FOLDER).join("2026-11-14_14-56-36");
        let samples = vec![0.0f32; 160_000];
        let annotation = annotation(
            started(),
            10_000,
            "ggml-base.en".into(),
            Vec::new(),
            Vec::new(),
        );

        write(&folder, &samples, &annotation).unwrap();

        let wav = hound::WavReader::open(folder.join(AUDIO_FILE)).unwrap();
        assert_eq!(wav.spec().sample_rate, 16_000);
        assert_eq!(wav.spec().channels, 1);
        assert_eq!(wav.duration(), 160_000);
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(folder.join(ANNOTATION_FILE)).unwrap()).unwrap();
        assert_eq!(json["schema"], package::SCHEMA);
        assert_eq!(json["durationMs"], 10_000);
        assert_eq!(json["audio"]["file"], "audio.wav");
        assert_eq!(json["frames"], serde_json::json!([]));
        let read_back: Annotation = serde_json::from_value(json).unwrap();
        assert_eq!(read_back, annotation);
    }

    /// The stop-to-saved path with the real whisper-cli: 10 s of `say` speech
    /// and silence in, a folder with a 160k-sample WAV and a transcript out.
    /// Needs `DICTATE_WHISPER_MODEL`.
    #[test]
    #[ignore = "needs whisper-cli and DICTATE_WHISPER_MODEL"]
    #[cfg(target_os = "macos")]
    fn ten_seconds_of_speech_become_a_session_folder() {
        let model = std::path::PathBuf::from(std::env::var("DICTATE_WHISPER_MODEL").unwrap());
        let temp = tempfile::tempdir().unwrap();
        let wav = temp.path().join("speech.wav");
        assert!(
            std::process::Command::new("say")
                .args([
                    "-v",
                    "Samantha",
                    "--file-format=WAVE",
                    "--data-format=LEI16@16000",
                    "-o"
                ])
                .arg(&wav)
                .arg("This button overlaps the label. Move it to the right.")
                .status()
                .unwrap()
                .success()
        );
        let mut samples: Vec<f32> = hound::WavReader::open(&wav)
            .unwrap()
            .samples::<i16>()
            .map(|s| f32::from(s.unwrap()) / 32768.0)
            .collect();
        samples.resize(160_000, 0.0);

        let started_at = std::time::Instant::now();
        let words = crate::transcribe::transcribe_timed(&samples, &model).unwrap();
        let annotation = annotation(
            started(),
            10_000,
            crate::transcribe::model_name(&model),
            words,
            Vec::new(),
        );
        let folder = temp.path().join(FOLDER).join(&annotation.id);
        write(&folder, &samples, &annotation).unwrap();
        eprintln!(
            "stop_to_saved_ms={} words={} text={:?}",
            started_at.elapsed().as_millis(),
            annotation.transcript.words.len(),
            annotation.transcript.text
        );

        assert_eq!(
            hound::WavReader::open(folder.join(AUDIO_FILE))
                .unwrap()
                .duration(),
            160_000
        );
        assert!(annotation.transcript.text.to_lowercase().contains("button"));
        let words = &annotation.transcript.words;
        assert!(words.windows(2).all(|pair| pair[0].t0 <= pair[1].t0));
        assert!(words.iter().all(|word| word.t0 >= 40 && word.t1 <= 10_040));
    }
}
