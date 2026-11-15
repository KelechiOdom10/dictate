//! Screen recording stage for Annotate: one display in, one MP4 out.
//!
//!   screen::start(target, options)  ->  ()
//!   screen::stop()                  ->  VideoInfo  (path to an H.264 MP4)
//!
//! ScreenCaptureKit encodes and writes the file itself through
//! `SCRecordingOutput` (macOS 15), so no frames pass through Rust. A frame
//! counter rides along on the same stream only to report delivery stats.
//!
//! Our own process is removed from the picture with the content filter, which
//! also covers windows opened after recording starts.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use core_graphics::access::ScreenCaptureAccess;
use core_graphics::display::CGDisplay;
use screencapturekit::cm::SCFrameStatus;
use screencapturekit::prelude::*;
use screencapturekit::recording_output::{
    RecordingCallbacks, SCRecordingOutput, SCRecordingOutputCodec, SCRecordingOutputConfiguration,
    SCRecordingOutputFileType,
};
use screencapturekit::shareable_content::SCShareableContentInfo;

/// Text stays readable for an agent at this size; larger costs file size only.
pub const MAX_LONG_EDGE: u32 = 2560;
pub const DEFAULT_FPS: u32 = 15;
const FINALIZE_TIMEOUT: Duration = Duration::from_secs(10);
const SESSION_DIR_PREFIX: &str = "dictate-screen-";
/// Session folders older than this are left over from failed or abandoned
/// takes, so `start` removes them.
const STALE_SESSION_AGE: Duration = Duration::from_secs(24 * 60 * 60);

pub const OWN_APP_NOT_FOUND: &str = "Could not find dictate among the apps ScreenCaptureKit \
can see, so its windows cannot be left out of the recording. Recording was not started. \
Turn on \"Show in screen recordings\" to record anyway.";

pub const PERMISSION_DENIED: &str = "Screen Recording permission is not granted to dictate. \
Open System Settings → Privacy & Security → Screen & System Audio Recording, \
turn dictate on, then quit and reopen dictate.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayTarget {
    Main,
    Id(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureOptions {
    pub fps: u32,
    /// The "Show in screen recordings" setting. Off by default.
    pub include_own_windows: bool,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self {
            fps: DEFAULT_FPS,
            include_own_windows: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoInfo {
    pub path: PathBuf,
    pub display_id: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Length of the written movie, as ScreenCaptureKit reports it.
    pub duration_ms: u64,
    pub file_bytes: u64,
    /// Wall time between `start` returning and `stop` being called.
    pub wall_ms: u64,
    pub frames: FrameCounts,
    pub own_windows_excluded: bool,
}

/// What ScreenCaptureKit delivered, by frame status. `idle` frames mean the
/// screen did not change, not that a frame was lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameCounts {
    pub complete: u64,
    pub idle: u64,
    pub other: u64,
}

/// A failed `stop`. `partial_recording` is the MP4 that may hold part of the
/// take, so the caller can keep or delete it instead of leaving it orphaned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopError {
    pub message: String,
    pub partial_recording: Option<PathBuf>,
}

impl fmt::Display for StopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.partial_recording {
            Some(path) => write!(
                f,
                "{} (partial recording: {})",
                self.message,
                path.display()
            ),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for StopError {}

impl From<StopError> for String {
    fn from(error: StopError) -> Self {
        error.to_string()
    }
}

impl FrameCounts {
    pub fn delivered(&self) -> u64 {
        self.complete + self.idle + self.other
    }
}

/// Without the grant, ScreenCaptureKit records only the wallpaper and our own
/// windows, so check before starting rather than ship an empty video.
pub fn has_permission() -> bool {
    ScreenCaptureAccess.preflight()
}

/// Shows the system prompt the first time; afterwards macOS only answers.
/// A grant takes effect after dictate restarts.
pub fn request_permission() -> bool {
    ScreenCaptureAccess.request()
}

pub fn is_recording() -> bool {
    state()
        .lock()
        .is_ok_and(|slot| matches!(*slot, Slot::Recording(_)))
}

/// The permission check, content listing and capture start are blocking
/// system calls, so they run with the lock released. The `Starting` slot keeps
/// a second `start` (or a `stop`) out while they run.
pub fn start(target: DisplayTarget, options: CaptureOptions) -> Result<(), String> {
    if options.fps == 0 {
        return Err("fps must be at least 1".into());
    }
    let reservation = StartReservation::claim()?;
    remove_stale_sessions(&std::env::temp_dir(), SystemTime::now());
    if !has_permission() && !request_permission() {
        return Err(PERMISSION_DENIED.into());
    }
    reservation.fill(Recording::begin(target, options)?);
    Ok(())
}

pub fn stop() -> Result<VideoInfo, StopError> {
    let not_stoppable = |message: &str| StopError {
        message: message.into(),
        partial_recording: None,
    };
    let mut slot = state()
        .lock()
        .map_err(|_| not_stoppable("Screen state lock poisoned"))?;
    let recording = match std::mem::replace(&mut *slot, Slot::Idle) {
        Slot::Recording(recording) => recording,
        Slot::Starting => {
            *slot = Slot::Starting;
            return Err(not_stoppable("Screen recording is still starting"));
        }
        Slot::Idle => return Err(not_stoppable("Screen is not recording")),
    };
    drop(slot);
    recording.finish()
}

enum Slot {
    Idle,
    Starting,
    Recording(Recording),
}

fn state() -> &'static Mutex<Slot> {
    static STATE: Mutex<Slot> = Mutex::new(Slot::Idle);
    &STATE
}

/// Holds the `Starting` slot. Dropping it without `fill` (an error or a
/// panic during start) puts the slot back to `Idle`.
struct StartReservation {
    filled: bool,
}

impl StartReservation {
    fn claim() -> Result<Self, String> {
        let mut slot = state().lock().map_err(|_| "Screen state lock poisoned")?;
        match *slot {
            Slot::Idle => {
                *slot = Slot::Starting;
                Ok(Self { filled: false })
            }
            Slot::Starting | Slot::Recording(_) => Err("Screen recording already running".into()),
        }
    }

    fn fill(mut self, recording: Recording) {
        if let Ok(mut slot) = state().lock() {
            *slot = Slot::Recording(recording);
            self.filled = true;
        }
    }
}

impl Drop for StartReservation {
    fn drop(&mut self) {
        if self.filled {
            return;
        }
        if let Ok(mut slot) = state().lock() {
            *slot = Slot::Idle;
        }
    }
}

struct Recording {
    stream: SCStream,
    output: SCRecordingOutput,
    finished: Receiver<Result<(), String>>,
    counts: Arc<Counters>,
    started: Instant,
    path: PathBuf,
    display_id: u32,
    width: u32,
    height: u32,
    fps: u32,
    own_windows_excluded: bool,
}

impl Recording {
    fn begin(target: DisplayTarget, options: CaptureOptions) -> Result<Self, String> {
        let content = SCShareableContent::get().map_err(|e| {
            format!("List screen content: {e}. If this says the user declined, {PERMISSION_DENIED}")
        })?;
        let display_id = match target {
            DisplayTarget::Main => CGDisplay::main().id,
            DisplayTarget::Id(id) => id,
        };
        let displays = content.displays();
        let display = displays
            .iter()
            .find(|d| d.display_id() == display_id)
            .ok_or_else(|| {
                let ids: Vec<u32> = displays.iter().map(SCDisplay::display_id).collect();
                format!("Display {display_id} is not among the capturable displays {ids:?}")
            })?;

        let pid = std::process::id() as i32;
        let own_app = own_app_to_exclude(
            options.include_own_windows,
            content
                .applications()
                .into_iter()
                .find(|app| app.process_id() == pid),
        )?;
        let mut filter = SCContentFilter::create().with_display(display);
        if let Some(app) = &own_app {
            filter = filter.with_excluding_applications(&[app], &[]);
        }
        let filter = filter
            .build()
            .map_err(|e| format!("Build content filter: {e}"))?;

        let (pixel_w, pixel_h) = SCShareableContentInfo::for_filter(&filter)
            .map(|info| info.pixel_size())
            .unwrap_or((display.width(), display.height()));
        let (width, height) = output_size(pixel_w, pixel_h, MAX_LONG_EDGE);

        let config = SCStreamConfiguration::new()
            .with_width(width)
            .with_height(height)
            .with_fps(options.fps)
            .with_shows_cursor(true);

        let path = session_dir()?.join("recording.mp4");
        let recording_config = SCRecordingOutputConfiguration::new()
            .map_err(|e| format!("Recording config: {e}"))?
            .with_output_url(&path)
            .map_err(|e| format!("Recording path {}: {e:?}", path.display()))?
            .with_video_codec(SCRecordingOutputCodec::H264)
            .with_output_file_type(SCRecordingOutputFileType::MP4);

        let (finished_tx, finished) = mpsc::channel();
        let fail_tx = Mutex::new(finished_tx.clone());
        let finish_tx = Mutex::new(finished_tx);
        let callbacks = RecordingCallbacks::new()
            .on_fail(move |error| {
                if let Ok(tx) = fail_tx.lock() {
                    let _ = tx.send(Err(error));
                }
            })
            .on_finish(move || {
                if let Ok(tx) = finish_tx.lock() {
                    let _ = tx.send(Ok(()));
                }
            });
        let output = SCRecordingOutput::new_with_delegate(&recording_config, callbacks)
            .ok_or("SCRecordingOutput unavailable; Annotate needs macOS 15 or later")?;

        let counts = Arc::new(Counters::default());
        let mut stream =
            SCStream::new(&filter, &config).map_err(|e| format!("Create stream: {e}"))?;
        stream
            .add_output_handler(
                FrameCounter(Arc::clone(&counts)),
                SCStreamOutputType::Screen,
            )
            .map_err(|e| format!("Attach frame counter: {e}"))?;
        stream
            .add_recording_output(&output)
            .map_err(|e| format!("Attach recording output: {e}"))?;
        stream.start_capture().map_err(|e| {
            format!("Start capture: {e}. If this says the user declined, {PERMISSION_DENIED}")
        })?;

        Ok(Self {
            stream,
            output,
            finished,
            counts,
            started: Instant::now(),
            path,
            display_id,
            width,
            height,
            fps: options.fps,
            own_windows_excluded: own_app.is_some(),
        })
    }

    fn finish(self) -> Result<VideoInfo, StopError> {
        let wall_ms = self.started.elapsed().as_millis() as u64;
        let failed = |message: String| StopError {
            message,
            partial_recording: Some(self.path.clone()),
        };
        self.stream
            .stop_capture()
            .map_err(|e| failed(format!("Stop capture: {e}")))?;
        match self.finished.recv_timeout(FINALIZE_TIMEOUT) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(failed(format!("Recording failed: {error}"))),
            Err(_) => {
                return Err(failed(format!(
                    "Recording did not finish within {} s",
                    FINALIZE_TIMEOUT.as_secs()
                )));
            }
        }
        let duration = self.output.recorded_duration();
        let duration_ms = if duration.timescale > 0 {
            (duration.value.max(0) as u64 * 1000) / duration.timescale as u64
        } else {
            0
        };
        let file_bytes = std::fs::metadata(&self.path)
            .map(|m| m.len())
            .map_err(|e| failed(format!("Read {}: {e}", self.path.display())))?;
        Ok(VideoInfo {
            path: self.path,
            display_id: self.display_id,
            width: self.width,
            height: self.height,
            fps: self.fps,
            duration_ms,
            file_bytes,
            wall_ms,
            frames: self.counts.snapshot(),
            own_windows_excluded: self.own_windows_excluded,
        })
    }
}

#[derive(Default)]
struct Counters {
    complete: AtomicU64,
    idle: AtomicU64,
    other: AtomicU64,
}

impl Counters {
    fn snapshot(&self) -> FrameCounts {
        FrameCounts {
            complete: self.complete.load(Ordering::Relaxed),
            idle: self.idle.load(Ordering::Relaxed),
            other: self.other.load(Ordering::Relaxed),
        }
    }
}

struct FrameCounter(Arc<Counters>);

impl SCStreamOutputTrait for FrameCounter {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
        if !matches!(of_type, SCStreamOutputType::Screen) {
            return;
        }
        let counter = match sample.frame_status() {
            Some(SCFrameStatus::Complete) => &self.0.complete,
            Some(SCFrameStatus::Idle) => &self.0.idle,
            _ => &self.0.other,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// Recording with our own windows in it by accident is worse than not
/// recording: the default promises a clean video, so a miss is an error.
fn own_app_to_exclude<App>(
    include_own_windows: bool,
    own_app: Option<App>,
) -> Result<Option<App>, String> {
    match (include_own_windows, own_app) {
        (true, _) => Ok(None),
        (false, Some(app)) => Ok(Some(app)),
        (false, None) => Err(OWN_APP_NOT_FOUND.into()),
    }
}

/// Scale down to fit `max_long_edge`, keep the aspect ratio, and round to even
/// numbers because H.264 encodes 4:2:0 chroma in 2×2 blocks.
fn output_size(width: u32, height: u32, max_long_edge: u32) -> (u32, u32) {
    let long = width.max(height).max(1);
    let scale = (f64::from(max_long_edge) / f64::from(long)).min(1.0);
    let even = |n: u32| ((f64::from(n) * scale).round() as u32 / 2 * 2).max(2);
    (even(width), even(height))
}

fn session_dir() -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("{SESSION_DIR_PREFIX}{stamp}"));
    create_dir(&dir)?;
    Ok(dir)
}

/// Reads the age from the folder name, which `session_dir` stamps with the
/// creation time in ms, so a folder touched later still ages out.
fn is_stale_session(name: &str, now: SystemTime) -> bool {
    let Some(created_ms) = name
        .strip_prefix(SESSION_DIR_PREFIX)
        .and_then(|stamp| stamp.parse::<u64>().ok())
    else {
        return false;
    };
    let created = UNIX_EPOCH + Duration::from_millis(created_ms);
    now.duration_since(created)
        .is_ok_and(|age| age > STALE_SESSION_AGE)
}

fn remove_stale_sessions(root: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        if !is_dir || !is_stale_session(&name.to_string_lossy(), now) {
            continue;
        }
        if let Err(error) = std::fs::remove_dir_all(entry.path()) {
            eprintln!(
                "[screen] could not remove stale session {}: {error}",
                entry.path().display()
            );
        }
    }
}

fn create_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Create {}: {e}", dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_size_keeps_native_pixels_under_the_cap() {
        assert_eq!(output_size(2560, 1662, 2560), (2560, 1662));
        assert_eq!(output_size(1920, 1080, 2560), (1920, 1080));
    }

    #[test]
    fn output_size_caps_the_long_edge_and_keeps_aspect() {
        assert_eq!(output_size(3024, 1964, 2560), (2560, 1662));
        assert_eq!(output_size(5120, 2880, 2560), (2560, 1440));
    }

    #[test]
    fn output_size_caps_a_portrait_display_by_height() {
        assert_eq!(output_size(2160, 3840, 2560), (1440, 2560));
    }

    #[test]
    fn output_size_is_always_even_and_non_zero() {
        let (w, h) = output_size(1513, 983, 2560);
        assert_eq!((w % 2, h % 2), (0, 0));
        assert_eq!(output_size(0, 0, 2560), (2, 2));
    }

    #[test]
    fn delivered_counts_every_status() {
        let counts = FrameCounts {
            complete: 40,
            idle: 400,
            other: 2,
        };
        assert_eq!(counts.delivered(), 442);
    }

    #[test]
    fn stop_without_start_is_an_error() {
        let error = stop().unwrap_err();
        assert_eq!(error.message, "Screen is not recording");
        assert_eq!(error.partial_recording, None);
    }

    #[test]
    fn missing_own_app_refuses_to_record_by_default() {
        assert_eq!(
            own_app_to_exclude::<u32>(false, None).unwrap_err(),
            OWN_APP_NOT_FOUND
        );
    }

    #[test]
    fn found_own_app_is_excluded_by_default() {
        assert_eq!(own_app_to_exclude(false, Some(7)), Ok(Some(7)));
    }

    #[test]
    fn including_own_windows_never_needs_our_app() {
        assert_eq!(own_app_to_exclude::<u32>(true, None), Ok(None));
        assert_eq!(own_app_to_exclude(true, Some(7)), Ok(None));
    }

    #[test]
    fn stop_error_names_the_partial_recording() {
        let error = StopError {
            message: "Recording failed: disk full".into(),
            partial_recording: Some(PathBuf::from("/tmp/dictate-screen-1/recording.mp4")),
        };
        assert_eq!(
            String::from(error),
            "Recording failed: disk full (partial recording: /tmp/dictate-screen-1/recording.mp4)"
        );
    }

    const DAY_MS: u64 = 24 * 60 * 60 * 1000;

    #[test]
    fn session_ages_out_after_a_day() {
        let now = UNIX_EPOCH + Duration::from_millis(10 * DAY_MS);
        let stamp = |age_ms: u64| format!("{SESSION_DIR_PREFIX}{}", 10 * DAY_MS - age_ms);
        assert!(is_stale_session(&stamp(DAY_MS + 1), now));
        assert!(!is_stale_session(&stamp(DAY_MS), now));
        assert!(!is_stale_session(&stamp(60_000), now));
    }

    #[test]
    fn only_our_stamped_folders_count_as_sessions() {
        let now = UNIX_EPOCH + Duration::from_millis(10 * DAY_MS);
        assert!(!is_stale_session("dictate-screen-notanumber", now));
        assert!(!is_stale_session("other-1", now));
        assert!(!is_stale_session("dictate-screen-", now));
    }

    #[test]
    fn stale_sessions_are_removed_and_the_rest_kept() {
        let root = tempfile::tempdir().unwrap();
        let now = UNIX_EPOCH + Duration::from_millis(10 * DAY_MS);
        let old = root
            .path()
            .join(format!("{SESSION_DIR_PREFIX}{}", 8 * DAY_MS));
        let fresh = root
            .path()
            .join(format!("{SESSION_DIR_PREFIX}{}", 10 * DAY_MS - 1));
        let unrelated = root.path().join("dictate-other-1");
        for dir in [&old, &fresh, &unrelated] {
            std::fs::create_dir(dir).unwrap();
        }
        std::fs::write(old.join("recording.mp4"), b"partial").unwrap();

        remove_stale_sessions(root.path(), now);

        assert!(!old.exists());
        assert!(fresh.exists());
        assert!(unrelated.exists());
    }
}
