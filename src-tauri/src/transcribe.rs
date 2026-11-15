use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

static TRANSCRIPTION: Mutex<()> = Mutex::new(());

pub fn transcribe(samples: &[f32]) -> Result<String, String> {
    if samples.is_empty() || samples.iter().any(|sample| !sample.is_finite()) {
        return Err("Expected nonempty, finite 16 kHz mono audio".into());
    }
    let _guard = TRANSCRIPTION.try_lock().map_err(|_| {
        eprintln!("[transcribe] rejected: transcription is already running");
        "Transcription is already running; try again when it finishes".to_string()
    })?;
    let cli = whisper_cli()?;
    let model = model_path()?;
    let temp =
        tempfile::tempdir().map_err(|e| format!("Create transcription temp directory: {e}"))?;
    let wav = temp.path().join("audio.wav");
    write_wav(&wav, samples)?;
    let output_base = temp.path().join("transcript");
    let output = Command::new(&cli)
        .arg("-m")
        .arg(&model)
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
pub fn model_name() -> String {
    model_path()
        .ok()
        .and_then(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "Unavailable".into())
}

fn whisper_cli() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("DICTATE_WHISPER_CLI") {
        let path = PathBuf::from(path);
        return path
            .is_file()
            .then_some(path)
            .ok_or_else(|| "DICTATE_WHISPER_CLI must point to the whisper-cli executable".into());
    }
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join("whisper-cli"))
        .chain([
            PathBuf::from("/opt/homebrew/bin/whisper-cli"),
            PathBuf::from("/usr/local/bin/whisper-cli"),
        ])
        .find(|path| path.is_file())
        .ok_or_else(|| {
            "whisper-cli is missing: install whisper-cpp, add whisper-cli to PATH, or set DICTATE_WHISPER_CLI to its absolute path".into()
        })
}

fn model_path() -> Result<PathBuf, String> {
    let path = std::env::var_os("DICTATE_WHISPER_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../models/ggml-base.en.bin")
        });
    if !path.is_file() {
        return Err(format!(
            "Whisper model is missing at {}: download ggml-base.en.bin into models/ or set DICTATE_WHISPER_MODEL to an English GGML model path",
            path.display()
        ));
    }
    Ok(path)
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

    #[test]
    fn rejects_empty_and_nonfinite_audio() {
        for samples in [&[][..], &[f32::NAN], &[f32::INFINITY]] {
            assert!(
                transcribe(samples)
                    .unwrap_err()
                    .contains("finite 16 kHz mono")
            );
        }
    }

    #[test]
    fn generated_speech_transcribes_through_whisper_cli() {
        if let Err(error) = whisper_cli().and_then(|_| model_path()) {
            eprintln!("SKIP generated speech transcription: {error}");
            return;
        }
        if !cfg!(target_os = "macos") {
            eprintln!("SKIP generated speech transcription: requires macOS say and afconvert");
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let aiff = temp.path().join("speech.aiff");
        let wav = temp.path().join("speech.wav");
        assert!(
            Command::new("say")
                .args(["-v", "Samantha", "-o"])
                .arg(&aiff)
                .arg("The quick brown fox jumps over the lazy dog.")
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
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .map(|sample| f32::from(sample.unwrap()) / 32768.0)
            .collect();
        let start = std::time::Instant::now();
        let text = transcribe(&samples).unwrap();
        eprintln!(
            "model={} samples={} transcription_ms={} text={text:?}",
            model_path().unwrap().display(),
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
}
