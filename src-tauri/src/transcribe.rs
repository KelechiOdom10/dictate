//! Speech to text.
//!
//! Contract: 16 kHz mono f32 in, plain text out. No punctuation decisions, no
//! formatting, no cleverness — those are separate problems and you will solve
//! them later with a prompt, not with the acoustic model.
//!
//! Spike strategy: shell out to the `whisper-cli` binary. It is slower and
//! clumsier than linking the library, and that is the point — it gives you a
//! working baseline whose latency you can measure, so that when you move to
//! `whisper-rs` in-process you have a number to beat rather than a vibe.

/// Transcribe 16 kHz mono f32 samples.
///
/// TODO(you):
/// - write the samples to a temp `.wav` (16-bit PCM, 16 kHz mono). `hound`
///   makes this four lines. This temp file exists only because you are
///   shelling out; it disappears when you go in-process.
/// - run `whisper-cli -m <model> -f <wav> -nt -np` via `std::process::Command`.
///   `-nt` drops timestamps, `-np` drops progress output you would have to
///   regex out of stderr anyway.
/// - trim the result. `whisper-cli` emits a leading space and occasionally a
///   trailing newline.
/// - time it, and print the duration. You need the number for the gate.
///
/// First run downloads nothing — you have to fetch a model yourself. `base.en`
/// is the right first choice: ~142 MB, fast enough to feel instant, accurate
/// enough to be useful. `small.en` is the quality step up at roughly 3x the
/// latency. Pick between them with a measurement, not a guess.
pub fn transcribe(_samples: &[f32]) -> Result<String, String> {
    Err("transcribe::transcribe not implemented".into())
}
