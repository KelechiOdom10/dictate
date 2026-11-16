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
    state().lock().is_ok_and(|active| active.is_some())
}

pub fn start(target: DisplayTarget, options: CaptureOptions) -> Result<(), String> {
    if options.fps == 0 {
        return Err("fps must be at least 1".into());
    }
    let mut slot = state().lock().map_err(|_| "Screen state lock poisoned")?;
    if slot.is_some() {
        return Err("Screen recording already running".into());
    }
    if !has_permission() && !request_permission() {
        return Err(PERMISSION_DENIED.into());
    }
    *slot = Some(Recording::begin(target, options)?);
    Ok(())
}

pub fn stop() -> Result<VideoInfo, String> {
    let recording = state()
        .lock()
        .map_err(|_| "Screen state lock poisoned")?
        .take()
        .ok_or("Screen is not recording")?;
    recording.finish()
}

fn state() -> &'static Mutex<Option<Recording>> {
    static STATE: Mutex<Option<Recording>> = Mutex::new(None);
    &STATE
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

        let own_app = (!options.include_own_windows)
            .then(|| {
                let pid = std::process::id() as i32;
                content
                    .applications()
                    .into_iter()
                    .find(|app| app.process_id() == pid)
            })
            .flatten();
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

    fn finish(self) -> Result<VideoInfo, String> {
        let wall_ms = self.started.elapsed().as_millis() as u64;
        self.stream
            .stop_capture()
            .map_err(|e| format!("Stop capture: {e}"))?;
        match self.finished.recv_timeout(FINALIZE_TIMEOUT) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(format!("Recording failed: {error}")),
            Err(_) => {
                return Err(format!(
                    "Recording did not finish within {} s",
                    FINALIZE_TIMEOUT.as_secs()
                ));
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
            .map_err(|e| format!("Read {}: {e}", self.path.display()))?;
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
    let dir = std::env::temp_dir().join(format!("dictate-screen-{stamp}"));
    create_dir(&dir)?;
    Ok(dir)
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
        assert_eq!(stop().unwrap_err(), "Screen is not recording");
    }
}
