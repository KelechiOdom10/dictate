//! Annotate session: tap the Annotate shortcut to start recording the screen
//! and microphone, draw while it records, tap again to stop. Stopping builds
//! the package folder and copies its summary.
//!
//! The rules here are pure: the phase a tap moves to, whether dictation may
//! start, and how a finished recording becomes an `Annotation`. `package`
//! is the one path from a stopped session to a finished folder, and runs
//! without AppKit state, so a test can feed it a generated MP4. `lib.rs`
//! holds the phase, calls `audio`, `screen` and `transcribe`, and drives the
//! pill and the floating bar. Ink drawn during the session comes from
//! `ink.rs`.
//!
//! `audio.rs` keeps one shared recording, so Annotate and dictation must never
//! record at the same time: a dictation `stop()` would take Annotate's audio.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use ts_rs::TS;

use crate::package::{self, Annotation, Audio, Display, Session, Stroke, Video};
use crate::problem::Problem;
use crate::transcribe::Word;

/// Session folders live in `<app data>/annotations/<id>/`.
pub const FOLDER: &str = "annotations";
pub const AUDIO_FILE: &str = "audio.wav";
pub const VIDEO_FILE: &str = "recording.mp4";
/// What a screen recording that failed to stop managed to write. Kept, but
/// not used for frames: its length and size are unknown.
pub const PARTIAL_VIDEO_FILE: &str = "recording-partial.mp4";
pub const ANNOTATION_FILE: &str = "annotation.json";
pub const SUMMARY_FILE: &str = "summary.md";
/// History shows at most this many sessions, newest first.
pub const LIST_LIMIT: usize = 200;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    /// The microphone is recording and `screen::start` is running off the
    /// main thread. Taps do nothing until it finishes, as in `screen.rs`.
    Starting,
    /// The microphone and screen belong to Annotate.
    Recording,
    /// Recording stopped; the WAV, transcript and JSON are being written.
    Packaging,
}

/// The phase after a tap of the Annotate shortcut. A session only starts when
/// dictation is idle, and a tap while starting or packaging does nothing.
pub fn tap(phase: Phase, dictation_idle: bool) -> Phase {
    match phase {
        Phase::Idle if dictation_idle => Phase::Starting,
        Phase::Idle => Phase::Idle,
        Phase::Starting => Phase::Starting,
        Phase::Recording => Phase::Packaging,
        Phase::Packaging => Phase::Packaging,
    }
}

/// The phase once `screen::start` returns: recording if it started, idle if
/// not. Only moves out of `Starting`.
pub fn screen_started(phase: Phase, started: bool) -> Phase {
    match (phase, started) {
        (Phase::Starting, true) => Phase::Recording,
        (Phase::Starting, false) => Phase::Idle,
        (other, _) => other,
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

/// Without the grant ScreenCaptureKit records only the wallpaper, so a
/// session does not start.
pub fn screen_permission_needed() -> Problem {
    Problem::new(
        "Annotate needs Screen Recording",
        "Open System Settings › Privacy & Security › Screen & System Audio Recording, turn on dictate, then reopen dictate.",
    )
}

/// `screen::start` failed for a reason other than the permission.
pub fn screen_start_failed(cause: impl std::fmt::Display) -> Problem {
    Problem::new(
        "Annotate couldn’t record the screen",
        "Try again. If it keeps failing, turn on “Show in screen recordings” in Settings › Annotate.",
    )
    .because(cause)
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
    /// When `screen::start` returned. ScreenCaptureKit's first frame comes
    /// a little later; R4 in the design measures how much.
    pub video_offset_ms: u64,
    pub display: Display,
    pub frontmost_app: Option<package::App>,
}

/// The screen recording a session made, as `screen::stop` handed it over.
/// `path` is wherever the recorder wrote it; `package` moves it in.
#[derive(Clone, Debug)]
pub struct Recording {
    pub path: PathBuf,
    /// When the video starts on the session clock.
    pub offset_ms: u64,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_ms: u64,
}

/// Everything a stopped session hands to `package`.
#[derive(Clone, Debug)]
pub struct Recorded {
    pub started: Started,
    pub duration_ms: u64,
    /// The speech model's name, as `transcribe::model_name` gives it.
    pub model: String,
    pub strokes: Vec<Stroke>,
    /// `None` when the screen recording failed to finish: the package is
    /// then audio, transcript and ink only, with no frames.
    pub video: Option<Recording>,
    /// When `video` is `None`, the MP4 the failed stop left behind, if any.
    pub partial_video: Option<PathBuf>,
}

/// The `annotation.json` for a session. Without video there is nothing to
/// pull frames from, so the builder's frames are dropped rather than
/// pointing at missing images.
pub fn annotation(
    started: Started,
    duration_ms: u64,
    model: String,
    words: Vec<Word>,
    strokes: Vec<Stroke>,
    video: Option<Video>,
) -> Annotation {
    let has_video = video.is_some();
    let mut annotation = package::build(Session {
        id: session_id(started.unix_ms),
        started_at: rfc3339(started.unix_ms),
        duration_ms,
        display: started.display,
        frontmost_app: started.frontmost_app,
        video,
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
    if !has_video {
        annotation.frames.clear();
    }
    annotation
}

/// A finished package.
#[derive(Debug)]
pub struct Packaged {
    pub folder: PathBuf,
    pub annotation: Annotation,
    /// The text of `summary.md`, which is what gets copied.
    pub summary: String,
    pub transcribe_ms: u64,
    #[cfg(target_os = "macos")]
    pub rendered: Option<crate::frames::Rendered>,
    /// The package was written, but without something the user expects.
    pub warning: Option<Problem>,
}

impl Packaged {
    pub fn contact_sheet(&self) -> Option<PathBuf> {
        (!self.annotation.frames.is_empty())
            .then(|| self.folder.join(&self.annotation.contact_sheet))
    }
}

/// Stopped session in, finished folder out: the MP4 moved in as
/// `recording.mp4`, `audio.wav`, the transcript, key frames, the contact
/// sheet, `annotation.json` and `summary.md`. `stopped` is when the stop tap
/// was handled; the time from it to here is stored as `packagingMs`.
///
/// A failed transcription or failed frames still leave a package, without
/// that part, and come back as `warning`. So does audio or video that failed
/// to stop: whatever was captured is kept and the package is marked partial.
/// Only failing to write the folder itself is an error.
pub fn package(
    folder: &Path,
    samples: Result<&[f32], Problem>,
    recorded: Recorded,
    stopped: Instant,
    transcribe: impl FnOnce(&[f32]) -> Result<Vec<Word>, Problem>,
) -> io::Result<Packaged> {
    std::fs::create_dir_all(folder)?;
    let mut warnings: Vec<Problem> = Vec::new();
    let mut partial: Vec<String> = Vec::new();
    let video = match recorded.video {
        Some(recording) => match move_file(&recording.path, &folder.join(VIDEO_FILE)) {
            Ok(()) => Some(Video {
                file: VIDEO_FILE.to_string(),
                offset_ms: recording.offset_ms,
                width: recording.width,
                height: recording.height,
                fps: recording.fps,
                duration_ms: recording.duration_ms,
            }),
            Err(error) => {
                warnings.push(
                    no_images().because(format!("Move {}: {error}", recording.path.display())),
                );
                None
            }
        },
        None => {
            let kept = recorded.partial_video.and_then(|from| {
                move_file(&from, &folder.join(PARTIAL_VIDEO_FILE))
                    .inspect_err(|error| {
                        log::warn!("[annotate] keep partial {}: {error}", from.display())
                    })
                    .ok()
            });
            let (note, fix) = match kept {
                Some(()) => (
                    format!(
                        "Video cut short: the screen recording did not finish, so there are no frames. What it caught is in {PARTIAL_VIDEO_FILE}."
                    ),
                    format!(
                        "The screen recording stopped early, so there are no frames. What it caught is in {PARTIAL_VIDEO_FILE}."
                    ),
                ),
                None => (
                    "No video: the screen recording did not finish, so there are no frames."
                        .to_string(),
                    "The screen recording did not finish, so there are no frames. The summary and audio are in the folder."
                        .to_string(),
                ),
            };
            partial.push(note);
            warnings.push(partial_saved(fix).because("The screen recording did not finish"));
            None
        }
    };
    let samples = match samples {
        Ok(samples) => Some(samples),
        Err(error) => {
            partial.push(
                "No audio: the microphone did not stop cleanly, so there is no transcript.".into(),
            );
            warnings.push(
                partial_saved("There is no audio or transcript. Record again to get them.")
                    .because(error),
            );
            None
        }
    };
    // Without audio an empty WAV keeps `audio.file` pointing at a real file.
    crate::audio::save_wav(&folder.join(AUDIO_FILE), samples.unwrap_or_default())
        .map_err(io::Error::other)?;

    let transcribing = Instant::now();
    let words = match samples.map(transcribe) {
        Some(Ok(words)) => words,
        Some(Err(error)) => {
            warnings.push(
                Problem::new("Annotation saved without a transcript", error.fix.clone())
                    .because(error),
            );
            Vec::new()
        }
        None => Vec::new(),
    };
    let transcribe_ms = elapsed_ms(transcribing);

    let mut annotation = annotation(
        recorded.started,
        recorded.duration_ms,
        recorded.model,
        words,
        recorded.strokes,
        video,
    );
    #[cfg(target_os = "macos")]
    let rendered = match annotation.video.is_some() && !annotation.frames.is_empty() {
        true => match crate::frames::render(folder, &annotation, &folder.join(VIDEO_FILE)) {
            Ok(rendered) => Some(rendered),
            Err(error) => {
                annotation.frames.clear();
                warnings.push(no_images().because(error));
                None
            }
        },
        false => None,
    };
    annotation.packaging_ms = Some(elapsed_ms(stopped));
    annotation.partial = partial;
    let summary = package::write_package(folder, &annotation)?;
    Ok(Packaged {
        folder: folder.to_path_buf(),
        annotation,
        summary,
        transcribe_ms,
        #[cfg(target_os = "macos")]
        rendered,
        warning: warnings.into_iter().reduce(Problem::and),
    })
}

fn partial_saved(fix: impl Into<String>) -> Problem {
    Problem::new("Partial annotation saved", fix)
}

fn no_images() -> Problem {
    Problem::new(
        "Annotation saved without images",
        "The summary and audio are in the folder. Record again to get frames.",
    )
}

/// `rename`, or copy and delete when the temp folder is on another volume.
fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to)?;
    std::fs::remove_file(from)
}

fn elapsed_ms(since: Instant) -> u64 {
    since.elapsed().as_millis() as u64
}

/// One session, as History lists it.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationEntry {
    /// The folder name, `2026-11-14_14-56-36`.
    pub id: String,
    /// RFC 3339, UTC.
    pub started_at: String,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub app: Option<String>,
    pub frames: u32,
    pub marks: u32,
    pub text: String,
    /// Absolute path of the folder, for display.
    pub folder: String,
    pub has_contact_sheet: bool,
    #[ts(type = "number | null")]
    pub packaging_ms: Option<u64>,
    /// What a partial session lost, one sentence each. Empty when whole.
    pub partial: Vec<String>,
}

/// The sessions under `dir`, newest first. Folders without a readable
/// `annotation.json` (an unfinished or foreign folder) are left out.
pub fn list(dir: &Path) -> io::Result<Vec<AnnotationEntry>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut ids: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|id| is_session_id(id))
        .collect();
    ids.sort_unstable_by(|a, b| b.cmp(a));
    Ok(ids
        .into_iter()
        .filter_map(|id| {
            let folder = dir.join(&id);
            let json = std::fs::read(folder.join(ANNOTATION_FILE)).ok()?;
            let annotation: Annotation = serde_json::from_slice(&json)
                .inspect_err(|error| log::warn!("[annotate] skip {id}: {error}"))
                .ok()?;
            Some(AnnotationEntry {
                has_contact_sheet: !annotation.frames.is_empty()
                    && folder.join(&annotation.contact_sheet).is_file(),
                id,
                started_at: annotation.started_at,
                duration_ms: annotation.duration_ms,
                app: annotation.frontmost_app.map(|app| app.name),
                frames: annotation.frames.len() as u32,
                marks: annotation.marks.len() as u32,
                text: annotation.transcript.text,
                folder: folder.display().to_string(),
                packaging_ms: annotation.packaging_ms,
                partial: annotation.partial,
            })
        })
        .take(LIST_LIMIT)
        .collect())
}

/// The folder of session `id` under `dir`, if `id` is a session id and not
/// a path that leaves `dir`.
pub fn session_folder(dir: &Path, id: &str) -> Option<PathBuf> {
    is_session_id(id).then(|| dir.join(id))
}

/// Removes one session folder. An id that is not a session name, or a folder
/// that is not directly under `dir`, is refused rather than followed.
pub fn delete(dir: &Path, id: &str) -> io::Result<bool> {
    let Some(folder) = session_folder(dir, id) else {
        return Ok(false);
    };
    if folder.parent() != Some(dir) || !folder.is_dir() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&folder)?;
    Ok(true)
}

/// `2026-11-14_14-56-36`: what `session_id` makes, and nothing else.
fn is_session_id(id: &str) -> bool {
    let shape = "dddd-dd-dd_dd-dd-dd";
    id.len() == shape.len()
        && id.chars().zip(shape.chars()).all(|(c, s)| match s {
            'd' => c.is_ascii_digit(),
            other => c == other,
        })
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

/// The display active at the start of the session; capture and ink keep it.
#[cfg(target_os = "macos")]
pub fn recorded_display() -> Display {
    crate::focus::active_display()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_starts_tap_stops_and_packaging_returns_to_idle() {
        let mut phase = Phase::Idle;
        phase = tap(phase, true);
        assert_eq!(phase, Phase::Starting);
        phase = screen_started(phase, true);
        assert_eq!(phase, Phase::Recording);
        phase = tap(phase, true);
        assert_eq!(phase, Phase::Packaging);
        phase = packaged(phase);
        assert_eq!(phase, Phase::Idle);
    }

    #[test]
    fn taps_while_the_screen_starts_are_ignored_and_a_failed_start_is_idle() {
        assert_eq!(tap(Phase::Starting, true), Phase::Starting);
        assert_eq!(screen_started(Phase::Starting, false), Phase::Idle);
        assert_eq!(screen_started(Phase::Packaging, true), Phase::Packaging);
        assert!(blocks_dictation(Phase::Starting));
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
            video_offset_ms: 120,
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
        let annotation = annotation(
            started(),
            10_000,
            "ggml-base.en".into(),
            words,
            Vec::new(),
            None,
        );

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
            None,
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

    fn recorded(strokes: Vec<Stroke>) -> Recorded {
        Recorded {
            started: started(),
            duration_ms: 10_000,
            model: "ggml-base.en".into(),
            strokes,
            video: None,
            partial_video: None,
        }
    }

    #[test]
    fn package_without_video_writes_audio_json_and_summary_and_warns() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join(FOLDER).join("2026-11-14_14-56-36");
        let samples = vec![0.0f32; 160_000];

        let packaged = package(
            &folder,
            Ok(&samples),
            recorded(Vec::new()),
            Instant::now(),
            |_| Ok(vec![word(500, 800, "Hello.")]),
        )
        .unwrap();

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
        assert!(json["packagingMs"].is_u64());
        let read_back: Annotation = serde_json::from_value(json).unwrap();
        assert_eq!(read_back, packaged.annotation);
        assert_eq!(
            std::fs::read_to_string(folder.join(SUMMARY_FILE)).unwrap(),
            packaged.summary
        );
        assert!(packaged.summary.contains("“Hello.”"));
        assert!(packaged.contact_sheet().is_none());
        assert_eq!(packaged.warning.unwrap().title, "Partial annotation saved");
        assert_eq!(
            packaged.annotation.partial,
            ["No video: the screen recording did not finish, so there are no frames."]
        );
    }

    #[test]
    fn a_failed_screen_stop_keeps_its_partial_mp4() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("2026-11-14_14-56-36");
        let leftover = dir.path().join("dictate-screen-1").join("take.mp4");
        std::fs::create_dir_all(leftover.parent().unwrap()).unwrap();
        std::fs::write(&leftover, b"partial").unwrap();
        let mut session = recorded(Vec::new());
        session.partial_video = Some(leftover.clone());

        let packaged = package(&folder, Ok(&[0.0; 16_000]), session, Instant::now(), |_| {
            Ok(vec![word(500, 800, "Hello.")])
        })
        .unwrap();

        assert!(!leftover.exists());
        assert_eq!(
            std::fs::read(folder.join(PARTIAL_VIDEO_FILE)).unwrap(),
            b"partial"
        );
        assert!(packaged.annotation.video.is_none());
        assert!(packaged.annotation.partial[0].starts_with("Video cut short"));
        assert!(
            packaged
                .summary
                .contains("Partial session: Video cut short")
        );
        assert!(packaged.summary.contains("“Hello.”"));
        assert_eq!(
            list(dir.path()).unwrap()[0].partial,
            packaged.annotation.partial
        );
    }

    #[test]
    fn a_failed_audio_stop_keeps_the_video_and_marks_the_session_partial() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("2026-11-14_14-56-36");
        let mp4 = dir.path().join("take.mp4");
        std::fs::write(&mp4, b"video").unwrap();
        let mut session = recorded(Vec::new());
        session.video = Some(Recording {
            path: mp4,
            offset_ms: 100,
            width: 2560,
            height: 1600,
            fps: 15,
            duration_ms: 9_000,
        });

        let packaged = package(
            &folder,
            Err(Problem::new("Microphone stopped badly", "Try again.")),
            session,
            Instant::now(),
            |_| panic!("nothing to transcribe"),
        )
        .unwrap();

        assert!(folder.join(VIDEO_FILE).is_file());
        assert!(packaged.annotation.video.is_some());
        assert_eq!(
            hound::WavReader::open(folder.join(AUDIO_FILE))
                .unwrap()
                .duration(),
            0
        );
        assert!(packaged.annotation.transcript.words.is_empty());
        assert_eq!(
            packaged.annotation.partial,
            ["No audio: the microphone did not stop cleanly, so there is no transcript."]
        );
        let warning = packaged.warning.unwrap();
        assert_eq!(warning.title, "Partial annotation saved");
        assert!(warning.detail.unwrap().contains("Microphone stopped badly"));
    }

    #[test]
    fn a_failed_transcription_still_writes_the_package() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("2026-11-14_14-56-36");
        let packaged = package(
            &folder,
            Ok(&[0.0; 16_000]),
            recorded(Vec::new()),
            Instant::now(),
            |_| Err(Problem::new("Transcription failed", "Try again.")),
        )
        .unwrap();
        assert!(folder.join(ANNOTATION_FILE).is_file());
        assert!(packaged.annotation.transcript.words.is_empty());
        let warning = packaged.warning.unwrap();
        assert_eq!(warning.title, "Partial annotation saved", "{warning}");
        assert!(warning.fix.contains("without a transcript"), "{warning}");
    }

    #[test]
    fn a_missing_video_file_is_a_warning_not_a_lost_session() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("2026-11-14_14-56-36");
        let mut session = recorded(Vec::new());
        session.video = Some(Recording {
            path: dir.path().join("gone.mp4"),
            offset_ms: 100,
            width: 2560,
            height: 1600,
            fps: 15,
            duration_ms: 9_000,
        });
        let packaged = package(&folder, Ok(&[0.0; 16_000]), session, Instant::now(), |_| {
            Ok(Vec::new())
        })
        .unwrap();
        assert!(packaged.annotation.video.is_none());
        assert!(
            packaged
                .warning
                .unwrap()
                .detail
                .unwrap()
                .contains("gone.mp4")
        );
    }

    #[test]
    fn list_returns_sessions_newest_first_and_skips_other_folders() {
        let dir = tempfile::tempdir().unwrap();
        for (id, text) in [
            ("2026-11-14_09-00-00", "First."),
            ("2026-11-15_10-30-00", "Second."),
        ] {
            let mut session = recorded(Vec::new());
            session.started.unix_ms = 0;
            let words = vec![word(0, 300, text)];
            package(
                &dir.path().join(id),
                Ok(&[0.0; 1_600]),
                session,
                Instant::now(),
                |_| Ok(words),
            )
            .unwrap();
        }
        std::fs::create_dir_all(dir.path().join("not-a-session")).unwrap();
        std::fs::create_dir_all(dir.path().join("2026-11-16_00-00-00")).unwrap();

        let entries = list(dir.path()).unwrap();

        let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["2026-11-15_10-30-00", "2026-11-14_09-00-00"]);
        assert_eq!(entries[0].text, "Second.");
        assert_eq!(entries[0].duration_ms, 10_000);
        assert!(!entries[0].has_contact_sheet);
        assert!(entries[0].packaging_ms.is_some());
        assert!(list(&dir.path().join("missing")).unwrap().is_empty());
    }

    #[test]
    fn only_session_ids_name_a_folder() {
        let dir = Path::new("/data/annotations");
        assert_eq!(
            session_folder(dir, "2026-11-14_14-56-36"),
            Some(dir.join("2026-11-14_14-56-36"))
        );
        for bad in [
            "..",
            "../../etc",
            "2026-11-14_14-56-3/",
            "",
            "2026-11-14T14-56-36",
        ] {
            assert_eq!(session_folder(dir, bad), None, "{bad}");
        }
    }

    #[test]
    fn typescript_contract_matches_rust_annotation_entry() {
        let contract = format!("export {}\n", AnnotationEntry::decl());
        let checked_in = include_str!("../../src/annotations.generated.ts");
        let compact = |text: &str| {
            text.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .replace(',', ";")
                .replace(";}", "}")
        };
        assert_eq!(compact(checked_in), compact(&contract));
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
        let folder = temp.path().join(FOLDER).join(session_id(started().unix_ms));
        let mut session = recorded(Vec::new());
        session.model = crate::transcribe::model_name(&model);
        let annotation = package(&folder, Ok(&samples), session, started_at, |samples| {
            crate::transcribe::transcribe_timed(samples, &model)
        })
        .unwrap()
        .annotation;
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

    /// Deleting removes one session folder, and refuses anything that is not
    /// a session name directly inside the annotations folder.
    #[test]
    fn delete_removes_one_session_folder_and_refuses_everything_else() {
        let root = std::env::temp_dir().join("dictate-annotate-delete");
        let _ = std::fs::remove_dir_all(&root);
        let keep = root.join("2026-11-14_14-56-36");
        let gone = root.join("2026-11-15_09-00-00");
        std::fs::create_dir_all(&keep).unwrap();
        std::fs::create_dir_all(&gone).unwrap();
        std::fs::write(gone.join("session.json"), "{}").unwrap();
        assert!(super::delete(&root, "2026-11-15_09-00-00").unwrap());
        assert!(!gone.exists(), "the session folder is gone");
        assert!(keep.is_dir(), "the other session stays");
        assert!(!super::delete(&root, "..").unwrap());
        assert!(!super::delete(&root, "2026-11-15_09-00-00").unwrap());
        assert!(!super::delete(&root, "../../etc").unwrap());
        assert!(root.is_dir(), "the root itself is never removed");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
