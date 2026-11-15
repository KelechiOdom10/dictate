use crate::problem::Problem;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

static TRANSCRIPTION: Mutex<()> = Mutex::new(());

/// A pause at least this long between two words starts a new segment.
pub const SEGMENT_PAUSE_MS: u64 = 700;

/// One spoken word, in ms from the first sample passed to `transcribe_timed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// A sentence or a run of speech between pauses, rebuilt from words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}
/// Transcribe 16 kHz mono audio with the GGML model at `model`.
pub fn transcribe(samples: &[f32], model: &Path) -> Result<String, Problem> {
    validate(samples)?;
    let _guard = lock()?;
    let cli = ready(model)?;
    let text = run_whisper(&cli, model, samples, &["-nt", "-otxt"], "txt").map_err(failed)?;
    Ok(text.trim().to_string())
}

/// Like `transcribe`, with the same model and the same failures, but returns
/// each word with its time on the audio clock.
///
/// `-ml 1 -sow` makes whisper-cli emit one segment per word, and `-ojf` writes
/// each segment's offsets in ms, so every JSON segment becomes one `Word`.
pub fn transcribe_timed(samples: &[f32], model: &Path) -> Result<Vec<Word>, Problem> {
    validate(samples)?;
    let _guard = lock()?;
    let cli = ready(model)?;
    let json =
        run_whisper(&cli, model, samples, &["-ojf", "-ml", "1", "-sow"], "json").map_err(failed)?;
    parse_words(&json).map_err(failed)
}

/// Splits after a word ending a sentence, or at a pause of `SEGMENT_PAUSE_MS`
/// or more before the next word.
pub fn segments(words: &[Word]) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut current: Vec<&Word> = Vec::new();
    for (index, word) in words.iter().enumerate() {
        current.push(word);
        let ends_sentence = word.text.ends_with(['.', '!', '?']);
        let pause_follows = words
            .get(index + 1)
            .is_some_and(|next| next.start_ms.saturating_sub(word.end_ms) >= SEGMENT_PAUSE_MS);
        if ends_sentence || pause_follows || index + 1 == words.len() {
            segments.push(Segment {
                start_ms: current[0].start_ms,
                end_ms: word.end_ms,
                text: current
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            });
            current.clear();
        }
    }
    segments
}

#[derive(serde::Deserialize)]
struct WhisperJson {
    transcription: Vec<WhisperSegment>,
}

#[derive(serde::Deserialize)]
struct WhisperSegment {
    offsets: WhisperOffsets,
    text: String,
}

#[derive(serde::Deserialize)]
struct WhisperOffsets {
    from: u64,
    to: u64,
}

/// Drops empty segments and whisper's bracketed non-speech markers such as
/// `[BLANK_AUDIO]`, which carry times but no words.
fn parse_words(json: &str) -> Result<Vec<Word>, String> {
    let parsed: WhisperJson =
        serde_json::from_str(json).map_err(|e| format!("Parse whisper-cli JSON: {e}"))?;
    Ok(parsed
        .transcription
        .into_iter()
        .filter_map(|segment| {
            let text = segment.text.trim();
            let non_speech = text.starts_with('[') && text.ends_with(']');
            (!text.is_empty() && !non_speech).then(|| Word {
                start_ms: segment.offsets.from,
                end_ms: segment.offsets.to,
                text: text.to_string(),
            })
        })
        .collect())
}

fn validate(samples: &[f32]) -> Result<(), Problem> {
    if samples.is_empty() || samples.iter().any(|sample| !sample.is_finite()) {
        return Err(failed("Expected nonempty, finite 16 kHz mono audio"));
    }
    Ok(())
}

fn lock() -> Result<std::sync::MutexGuard<'static, ()>, Problem> {
    TRANSCRIPTION.try_lock().map_err(|_| {
        log::warn!("[transcribe] rejected: transcription is already running");
        Problem::new(
            "Still transcribing",
            "Wait for the last dictation to finish.",
        )
    })
}

/// whisper-cli, once both it and the model file are present.
fn ready(model: &Path) -> Result<PathBuf, Problem> {
    let cli = whisper_cli()?;
    if !model.is_file() {
        return Err(missing_model(model));
    }
    Ok(cli)
}

fn failed(cause: impl std::fmt::Display) -> Problem {
    Problem::new("Transcription failed", "Hold the shortcut and try again.").because(cause)
}

/// Writes the audio as a WAV, runs whisper-cli with the shared flags plus
/// `output_args`, and returns the output file with `extension`. Callers hold
/// the `TRANSCRIPTION` guard.
fn run_whisper(
    cli: &Path,
    model: &Path,
    samples: &[f32],
    output_args: &[&str],
    extension: &str,
) -> Result<String, String> {
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
        .args(["-l", "en", "-np"])
        .args(output_args)
        .arg("-of")
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
    std::fs::read_to_string(output_base.with_extension(extension))
        .map_err(|e| format!("Read whisper-cli transcript: {e}"))
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
        let _turn = whisper_turn();
        let model = Path::new("/nonexistent/ggml-base.en.bin");
        for problem in [
            transcribe(&[0.1], model).unwrap_err(),
            transcribe_timed(&[0.1], model).unwrap_err(),
        ] {
            assert_eq!(problem.title, "Speech model missing");
            assert!(problem.fix.contains("Settings › Models"));
            assert!(
                problem
                    .detail
                    .unwrap()
                    .contains("/nonexistent/ggml-base.en.bin")
            );
        }
    }

    #[test]
    fn rejects_empty_and_nonfinite_audio() {
        for samples in [&[][..], &[f32::NAN], &[f32::INFINITY]] {
            for problem in [
                transcribe(samples, &test_model()).unwrap_err(),
                transcribe_timed(samples, &test_model()).unwrap_err(),
            ] {
                assert_eq!(problem.title, "Transcription failed");
                assert!(problem.detail.unwrap().contains("finite 16 kHz mono"));
            }
        }
    }

    fn word(start_ms: u64, end_ms: u64, text: &str) -> Word {
        Word {
            start_ms,
            end_ms,
            text: text.into(),
        }
    }

    #[test]
    fn parses_word_offsets_and_skips_empty_and_non_speech() {
        let json = r#"{"transcription": [
            {"offsets": {"from": 0, "to": 40}, "text": ""},
            {"offsets": {"from": 40, "to": 270}, "text": " Here", "tokens": []},
            {"offsets": {"from": 600, "to": 1180}, "text": " button,"},
            {"offsets": {"from": 1200, "to": 3000}, "text": " [BLANK_AUDIO]"},
            {"offsets": {"from": 3350, "to": 4230}, "text": " 3.5"}
        ]}"#;
        assert_eq!(
            parse_words(json).unwrap(),
            [
                word(40, 270, "Here"),
                word(600, 1180, "button,"),
                word(3350, 4230, "3.5"),
            ]
        );
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(
            parse_words(r#"{"result": {}}"#)
                .unwrap_err()
                .starts_with("Parse whisper-cli JSON")
        );
    }

    #[test]
    fn segments_split_at_sentence_ends() {
        let words = [
            word(40, 270, "Here"),
            word(270, 600, "it"),
            word(600, 1180, "is."),
            word(1180, 1500, "Isn't"),
            word(1500, 1800, "it?"),
            word(1800, 2100, "Yes"),
        ];
        assert_eq!(
            segments(&words),
            [
                Segment {
                    start_ms: 40,
                    end_ms: 1180,
                    text: "Here it is.".into()
                },
                Segment {
                    start_ms: 1180,
                    end_ms: 1800,
                    text: "Isn't it?".into()
                },
                Segment {
                    start_ms: 1800,
                    end_ms: 2100,
                    text: "Yes".into()
                },
            ]
        );
    }

    #[test]
    fn segments_split_at_a_pause_of_700_ms_or_more() {
        let words = [
            word(0, 300, "move"),
            word(999, 1300, "this"),
            word(2000, 2300, "over"),
            word(2400, 2700, "there"),
        ];
        let texts: Vec<String> = segments(&words).into_iter().map(|s| s.text).collect();
        assert_eq!(texts, ["move this", "over there"]);
    }

    #[test]
    fn no_words_make_no_segments() {
        assert_eq!(segments(&[]), []);
    }

    /// `transcribe` refuses to run twice at once, so the tests that call
    /// whisper-cli take turns instead of failing each other.
    fn whisper_turn() -> std::sync::MutexGuard<'static, ()> {
        static TURN: Mutex<()> = Mutex::new(());
        TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Speaks `text` with macOS `say` and returns it as 16 kHz mono f32, or
    /// `None` after printing why the integration test is skipped.
    fn generated_speech(test: &str, text: &str) -> Option<Vec<f32>> {
        if let Err(error) = whisper_cli() {
            eprintln!("SKIP {test}: {error}");
            return None;
        }
        let model = test_model();
        if !model.is_file() {
            eprintln!("SKIP {test}: no model at {}", model.display());
            return None;
        }
        if !cfg!(target_os = "macos") {
            eprintln!("SKIP {test}: requires macOS say and afconvert");
            return None;
        }
        Some(say(text))
    }

    fn say(text: &str) -> Vec<f32> {
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
        reader
            .samples::<i16>()
            .map(|sample| f32::from(sample.unwrap()) / 32768.0)
            .collect()
    }

    #[test]
    fn generated_speech_transcribes_through_whisper_cli() {
        let _turn = whisper_turn();
        let Some(samples) = generated_speech(
            "generated speech transcription",
            "The quick brown fox jumps over the lazy dog.",
        ) else {
            return;
        };
        let start = std::time::Instant::now();
        let text = transcribe(&samples, &test_model()).unwrap();
        eprintln!(
            "model={} samples={} transcription_ms={} text={text:?}",
            test_model().display(),
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

    const MINUTE_OF_SPEECH: &str = "Here is the settings page I want to change. \
The save button sits too close to the cancel button, so people press the wrong one. \
Move it to the right edge and give it the accent color. \
Now look at the list of recent recordings. \
Each row shows the date, but not the length, and the length is what I scan for. \
Put the length in the second column and make the date smaller. \
The empty state says nothing useful. \
It should tell people how to start their first recording and which shortcut to press. \
Down here the model picker lists five models with no hint about speed or accuracy. \
Add one short line under each name, for example fast and good enough for notes. \
Finally, the window opens far too wide on a small laptop screen. \
Cap the width and let the sidebar collapse when there is not enough room. \
One more thing about the history page. \
Searching should match words inside a recording, not only its title. \
When I open an old recording, show the frames first and the text below them. \
The delete button needs a confirmation, because a lost recording cannot come back. \
That is everything for this page, thank you.";

    #[test]
    fn generated_minute_gets_increasing_word_times_through_whisper_cli() {
        let _turn = whisper_turn();
        let test = "generated speech timed transcription";
        let Some(samples) = generated_speech(test, MINUTE_OF_SPEECH) else {
            return;
        };
        let audio_ms = samples.len() as u64 * 1000 / 16_000;
        let start = std::time::Instant::now();
        let words = transcribe_timed(&samples, &test_model()).unwrap();
        let elapsed = start.elapsed();
        let rebuilt = segments(&words);
        eprintln!(
            "model={} audio_ms={audio_ms} transcribe_timed_ms={} words={} segments={} last_word_end_ms={}",
            test_model().display(),
            elapsed.as_millis(),
            words.len(),
            rebuilt.len(),
            words.last().map_or(0, |w| w.end_ms),
        );
        assert!(
            words.len() > 100,
            "expected a minute of words, got {}",
            words.len()
        );
        for pair in words.windows(2) {
            assert!(
                pair[0].start_ms <= pair[1].start_ms && pair[0].end_ms <= pair[1].end_ms,
                "word times go backwards: {:?} then {:?}",
                pair[0],
                pair[1]
            );
        }
        assert!(words.iter().all(|w| w.start_ms <= w.end_ms));
        let last_end = words.last().unwrap().end_ms;
        assert!(
            last_end.abs_diff(audio_ms) <= 500,
            "last word ends at {last_end} ms, audio is {audio_ms} ms"
        );
        assert!(
            rebuilt.len() >= 10,
            "expected about one segment per sentence"
        );
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
        let samples = say(sentence);
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
