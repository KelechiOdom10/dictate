//! Whether a take holds any speech at all, decided by Silero VAD before the
//! take reaches the speech model.
//!
//! Without it, a short take of room tone reaches Parakeet, which answers with
//! a filler word ("Okay.", "Uh") that is then pasted. An energy gate fixed
//! that once but also threw away real speech spoken quietly, so the decision
//! here is about the shape of the sound, not its level: each one-second
//! window is brought to the same loudness before Silero scores it. That keeps
//! whispered speech, and a key click in an otherwise quiet take cannot drown
//! out the words the way it would if the whole take were scaled at once.
//!
//! The model (Silero VAD v5.1.2, MIT, 885 KB) is compiled into the app, so it
//! works offline and needs no download. See `resources/README.md`.

use std::{path::PathBuf, sync::Mutex};
use transcribe_cpp::{Model, VadOptions, VadSession};

const MODEL: &[u8] = include_bytes!("../resources/ggml-silero-v5.1.2.bin");
const MODEL_FILE: &str = "dictate-ggml-silero-v5.1.2.bin";

/// One second at 16 kHz, stepped half a second at a time, so a short word on
/// a window edge is still whole in the next one.
const WINDOW: usize = 16_000;
const HOP: usize = 8_000;
/// The RMS each window is brought to before scoring.
const LEVEL: f32 = 0.05;
/// A frame at or above this probability is speech (Silero's own default).
const SPEECH: f32 = 0.5;

static SESSION: Mutex<Option<VadSession>> = Mutex::new(None);

/// True when any frame of `samples` (16 kHz mono) is speech. An error means
/// the model couldn't run, and the caller should transcribe anyway.
pub fn has_speech(samples: &[f32]) -> Result<bool, String> {
    let mut session = SESSION.lock().unwrap_or_else(|e| e.into_inner());
    if session.is_none() {
        *session = Some(open()?);
    }
    let Some(session) = session.as_mut() else {
        return Err("VAD session unavailable".into());
    };
    let last = samples.len().saturating_sub(WINDOW);
    let starts = (0..last).step_by(HOP).chain(std::iter::once(last));
    for start in starts {
        let window = levelled(&samples[start..(start + WINDOW).min(samples.len())]);
        let result = session
            .run(&window, &VadOptions::default())
            .map_err(|e| format!("VAD: {e}"))?;
        if result.probs.iter().any(|p| *p >= SPEECH) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn levelled(window: &[f32]) -> Vec<f32> {
    let rms = (window.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>()
        / window.len().max(1) as f64)
        .sqrt() as f32;
    if rms == 0.0 {
        return window.to_vec();
    }
    window
        .iter()
        .map(|s| (s * LEVEL / rms).clamp(-1.0, 1.0))
        .collect()
}

/// The loader takes a path, so the compiled-in model is written once to the
/// temp folder and read from there.
fn open() -> Result<VadSession, String> {
    let path: PathBuf = std::env::temp_dir().join(MODEL_FILE);
    let current = std::fs::metadata(&path).is_ok_and(|m| m.len() == MODEL.len() as u64);
    if !current {
        let mut file = tempfile::NamedTempFile::new_in(std::env::temp_dir())
            .map_err(|e| format!("VAD model: {e}"))?;
        std::io::Write::write_all(&mut file, MODEL).map_err(|e| format!("VAD model: {e}"))?;
        file.persist(&path).map_err(|e| format!("VAD model: {e}"))?;
    }
    Model::load(&path)
        .and_then(|model| model.vad_session())
        .map_err(|e| format!("VAD model: {e}"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::process::Command;

    pub(crate) fn say(text: &str) -> Option<Vec<f32>> {
        if !cfg!(target_os = "macos") {
            eprintln!("SKIP: requires macOS say and afconvert");
            return None;
        }
        let temp = tempfile::tempdir().unwrap();
        let aiff = temp.path().join("speech.aiff");
        let wav = temp.path().join("speech.wav");
        assert!(
            Command::new("say")
                .args(["-v", "Samantha", "-o"])
                .arg(&aiff)
                .arg(text)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("afconvert")
                .args(["-f", "WAVE", "-d", "LEI16@16000", "-c", "1"])
                .arg(&aiff)
                .arg(&wav)
                .status()
                .unwrap()
                .success()
        );
        let mut reader = hound::WavReader::open(wav).unwrap();
        Some(
            reader
                .samples::<i16>()
                .map(|s| f32::from(s.unwrap()) / 32768.0)
                .collect(),
        )
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
            as f32
    }

    pub(crate) fn at_rms(samples: &[f32], target: f32) -> Vec<f32> {
        let gain = target / rms(samples);
        samples.iter().map(|s| s * gain).collect()
    }

    /// Pink-ish noise, the shape of a quiet room, at `level` RMS.
    pub(crate) fn room_tone(seconds: f32, level: f32, seed: u64) -> Vec<f32> {
        let mut state = seed;
        let mut pink = [0.0f32; 3];
        let noise: Vec<f32> = (0..(seconds * 16_000.0) as usize)
            .map(|_| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let white = ((state >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0;
                pink[0] = 0.99765 * pink[0] + white * 0.0990460;
                pink[1] = 0.96300 * pink[1] + white * 0.2965164;
                pink[2] = 0.57000 * pink[2] + white * 1.0526913;
                pink[0] + pink[1] + pink[2] + white * 0.1848
            })
            .collect();
        at_rms(&noise, level)
    }

    fn click(samples: &mut [f32], at: usize, amplitude: f32) {
        for (i, sample) in samples.iter_mut().skip(at).take(80).enumerate() {
            *sample += if i % 2 == 0 { amplitude } else { -amplitude };
        }
    }

    /// The worst case is a take with no speech: every window is scored.
    #[test]
    #[ignore = "Measures VAD cost on this machine; run explicitly"]
    fn measure_vad_cost() {
        has_speech(&room_tone(1.0, 0.001, 1)).unwrap();
        for seconds in [1.0, 10.0, 60.0] {
            let tone = room_tone(seconds, 0.001, 2);
            let started = std::time::Instant::now();
            assert!(!has_speech(&tone).unwrap());
            println!(
                "vad_no_speech seconds={seconds} ms={:.1}",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
    }

    #[test]
    fn room_tone_has_no_speech_at_any_length_or_level() {
        for seconds in [0.3, 0.5, 0.75, 1.0, 1.5, 3.0, 10.0] {
            for level in [0.00005, 0.0003, 0.001, 0.0029, 0.01] {
                for seed in 1..=3 {
                    let mut tone = room_tone(seconds, level, seed);
                    assert!(
                        !has_speech(&tone).unwrap(),
                        "{seconds} s at {level} seed {seed}"
                    );
                    click(&mut tone, 200, 0.3);
                    assert!(
                        !has_speech(&tone).unwrap(),
                        "{seconds} s at {level} seed {seed} with a click"
                    );
                }
            }
        }
    }

    #[test]
    fn quiet_and_short_speech_is_speech_even_after_a_key_click() {
        for text in [
            "The quick brown fox jumps over the lazy dog.",
            "Okay.",
            "Yes.",
            "Send it.",
        ] {
            let Some(speech) = say(text) else { return };
            for level in [0.05, 0.002295, 0.000239, 0.000075] {
                let spoken = at_rms(&speech, level);
                assert!(has_speech(&spoken).unwrap(), "{text} at {level}");
                let floor = (level * 0.3).max(0.00002);
                let mut take = room_tone(1.5, floor, 3);
                take.extend(&spoken);
                take.extend(room_tone(1.5, floor, 4));
                let end = take.len() - 400;
                click(&mut take, 200, 0.3);
                click(&mut take, end, 0.3);
                assert!(
                    has_speech(&take).unwrap(),
                    "{text} at {level} in a take with clicks"
                );
            }
        }
    }
}
