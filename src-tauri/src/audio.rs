//! Microphone capture.
//!
//! Contract: `start()` begins capture, `stop()` returns everything recorded
//! since it began, already resampled to 16 kHz mono f32. Nothing touches disk.
//!
//! 16 kHz mono f32 is not an arbitrary choice: it is what Whisper wants, and
//! resampling at capture time rather than after means you never debug a
//! format mismatch inside a transcription bug.

use std::sync::Mutex;

/// Samples captured since `start()`. Empty until you fill it.
static BUFFER: Mutex<Vec<f32>> = Mutex::new(Vec::new());

pub fn is_recording() -> bool {
    // TODO(you): make this report the real state once a stream can be open.
    false
}

/// Begin capture on the default input device.
///
/// TODO(you):
/// - `cpal::default_host().default_input_device()`
/// - take the device's `default_input_config()`, then build an input stream
///   with `build_input_stream_raw` so you get the device's native sample
///   format rather than praying it is f32.
/// - push samples into `BUFFER`, converting i16 -> f32 (`s as f32 / 32768.0`)
///   if the device is integer. Most Macs are f32, most USB mics are not.
/// - **keep the stream alive**: store it in a `static` or in Tauri's managed
///   state. A `Stream` that goes out of scope stops capture silently, and the
///   symptom is an empty buffer with no error. This is the first bug everyone
///   writes in this project.
/// - resample to 16 kHz. Macs commonly hand you 44.1 or 48 kHz; whisper will
///   produce nonsense if you skip this and it will look like a model problem.
pub fn start() -> Result<(), String> {
    Err("audio::start not implemented".into())
}

/// Stop capture and return the buffered samples, drained.
///
/// TODO(you): stop the stream, take the buffer, resample if you deferred it,
/// and clear it so the next recording does not start with stale audio.
pub fn stop() -> Result<Vec<f32>, String> {
    Err("audio::stop not implemented".into())
}
