use crate::problem::Problem;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

static TRANSCRIPTION: Mutex<()> = Mutex::new(());

/// Transcribe 16 kHz mono audio with the GGML model at `model`.
pub fn transcribe(samples: &[f32], model: &Path) -> Result<String, Problem> {
    if samples.is_empty() || samples.iter().any(|sample| !sample.is_finite()) {
        return Err(failed("Expected nonempty, finite 16 kHz mono audio"));
    }
    let _guard = TRANSCRIPTION.try_lock().map_err(|_| {
        log::warn!("[transcribe] rejected: transcription is already running");
        Problem::new(
            "Still transcribing",
            "Wait for the last dictation to finish.",
        )
    })?;
    let cli = whisper_cli()?;
    if !model.is_file() {
        return Err(missing_model(model));
    }
    run(&cli, model, samples).map_err(failed)
}

fn failed(cause: impl std::fmt::Display) -> Problem {
    Problem::new("Transcription failed", "Hold the shortcut and try again.").because(cause)
}

fn run(cli: &Path, model: &Path, samples: &[f32]) -> Result<String, String> {
    let temp =
        tempfile::tempdir().map_err(|e| format!("Create transcription temp directory: {e}"))?;
    let wav = temp.path().join("audio.wav");
    write_wav(&wav, samples)?;
    let output_base = temp.path().join("transcript");
    let output = Command::new(cli)
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(&wav)
        .args(["-l", "en", "-nt", "-np", "-otxt", "-of"])
        .arg(&output_base)
        .output()
        .map_err(|e| format!("Run {}: {e}", cli.display()))?;
    if !output.status.success() {
        return Err(format!(
            "whisper-cli failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = std::fs::read_to_string(output_base.with_extension("txt"))
        .map_err(|e| format!("Read whisper-cli transcript: {e}"))?;
    Ok(text.trim().to_string())
}

/// The model file name without extension, e.g. `ggml-base.en`.
pub fn model_name(model: &Path) -> String {
    model
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Unavailable".into())
}

pub const WHISPER_INSTALL: &str = "brew install whisper.cpp";

pub fn whisper_cli() -> Result<PathBuf, Problem> {
    if let Some(path) = std::env::var_os("DICTATE_WHISPER_CLI") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path.clone()).ok_or_else(|| {
            Problem::new(
                "whisper-cli not found",
                "Point DICTATE_WHISPER_CLI at the whisper-cli executable.",
            )
            .because(format!("no file at {}", path.display()))
        });
    }
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join("whisper-cli"))
        .chain([
            PathBuf::from("/opt/homebrew/bin/whisper-cli"),
            PathBuf::from("/usr/local/bin/whisper-cli"),
        ])
        .find(|path| path.is_file())
        .ok_or_else(whisper_missing)
}

pub fn whisper_missing() -> Problem {
    Problem::new(
        "whisper-cli not found",
        format!("Run `{WHISPER_INSTALL}` in Terminal, then try again."),
    )
    .because("not on PATH, /opt/homebrew/bin or /usr/local/bin")
}

pub fn missing_model(path: &Path) -> Problem {
    Problem::new(
        "Speech model missing",
        "Download a speech model in Settings › Models.",
    )
    .because(format!("no file at {}", path.display()))
}

fn write_wav(path: &Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("Create transcription WAV: {e}"))?;
    for sample in samples {
        let pcm = (sample.clamp(-1.0, 1.0) * 32768.0).clamp(-32768.0, 32767.0) as i16;
        writer
            .write_sample(pcm)
            .map_err(|e| format!("Write transcription WAV: {e}"))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("Finalize transcription WAV: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_preserves_rate_channels_and_pcm_amplitude() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("audio.wav");
        write_wav(&path, &[-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0]).unwrap();
        let mut reader = hound::WavReader::open(path).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        assert_eq!(
            reader
                .samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [-32768, -32768, -16384, 0, 16384, 32767, 32767]
        );
    }

    /// The spike's model: `DICTATE_WHISPER_MODEL`, else this checkout's
    /// `models/ggml-base.en.bin`. Tests only; the app uses the chosen model.
    fn test_model() -> PathBuf {
        std::env::var_os("DICTATE_WHISPER_MODEL")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../models/ggml-base.en.bin")
            })
    }

    #[test]
    fn models_page_shows_the_same_install_command() {
        assert!(
            include_str!("../../src/models.ts")
                .contains(&format!("WHISPER_INSTALL = \"{WHISPER_INSTALL}\";"))
        );
        assert!(whisper_missing().fix.contains(WHISPER_INSTALL));
    }

    #[test]
    fn missing_model_points_to_the_models_page() {
        if whisper_cli().is_err() {
            eprintln!("SKIP missing model: whisper-cli is checked first");
            return;
        }
        let problem = transcribe(&[0.1], Path::new("/nonexistent/ggml-base.en.bin")).unwrap_err();
        assert_eq!(problem.title, "Speech model missing");
        assert!(problem.fix.contains("Settings › Models"));
        assert!(
            problem
                .detail
                .unwrap()
                .contains("/nonexistent/ggml-base.en.bin")
        );
    }

    #[test]
    fn rejects_empty_and_nonfinite_audio() {
        for samples in [&[][..], &[f32::NAN], &[f32::INFINITY]] {
            let problem = transcribe(samples, &test_model()).unwrap_err();
            assert_eq!(problem.title, "Transcription failed");
            assert!(problem.detail.unwrap().contains("finite 16 kHz mono"));
        }
    }

    #[test]
    fn generated_speech_transcribes_through_whisper_cli() {
        let model = test_model();
        if let Err(error) = whisper_cli() {
            eprintln!("SKIP generated speech transcription: {error}");
            return;
        }
        if !model.is_file() {
            eprintln!(
                "SKIP generated speech transcription: no model at {}",
                model.display()
            );
            return;
        }
        if !cfg!(target_os = "macos") {
            eprintln!("SKIP generated speech transcription: requires macOS say and afconvert");
            return;
        }
        let samples = generated_speech("The quick brown fox jumps over the lazy dog.");
        let start = std::time::Instant::now();
        let text = transcribe(&samples, &model).unwrap();
        eprintln!(
            "model={} samples={} transcription_ms={} text={text:?}",
            model.display(),
            samples.len(),
            start.elapsed().as_millis()
        );
        let words = text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        assert_eq!(words, "the quick brown fox jumps over the lazy dog");
    }

    fn generated_speech(sentence: &str) -> Vec<f32> {
        let temp = tempfile::tempdir().unwrap();
        let aiff = temp.path().join("speech.aiff");
        let wav = temp.path().join("speech.wav");
        assert!(
            Command::new("say")
                .args(["-v", "Samantha", "-o"])
                .arg(&aiff)
                .arg(sentence)
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
        reader
            .samples::<i16>()
            .map(|sample| f32::from(sample.unwrap()) / 32768.0)
            .collect()
    }

    /// Times every catalog model on one generated sentence: one warm-up, then
    /// five runs. Point `DICTATE_MEASURE_MODELS_DIR` at a folder holding the
    /// GGML files. Results go in `docs/spike-results.md` and `models::CATALOG`.
    #[test]
    #[ignore = "Needs every model downloaded; run explicitly with --ignored --nocapture"]
    fn measure_catalog_models() {
        let dir = PathBuf::from(std::env::var_os("DICTATE_MEASURE_MODELS_DIR").unwrap());
        let sentence = "Please move the Thursday design review to Friday morning, \
                        and send Priya the updated budget before lunch.";
        let samples = generated_speech(sentence);
        println!(
            "fixture_samples={} seconds={:.2}",
            samples.len(),
            samples.len() as f64 / 16_000.0
        );
        for info in &crate::models::CATALOG {
            let model = crate::models::path(&dir, info.id);
            transcribe(&samples, &model).unwrap();
            let mut timings = Vec::new();
            let mut text = String::new();
            for _ in 0..5 {
                let start = std::time::Instant::now();
                text = transcribe(&samples, &model).unwrap();
                timings.push(start.elapsed().as_millis());
            }
            timings.sort();
            println!(
                "model={} median_ms={} min_ms={} max_ms={} text={text:?}",
                info.file_name, timings[2], timings[0], timings[4]
            );
        }
    }
}
