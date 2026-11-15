use crate::problem::Problem;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use transcribe_cpp::TimestampKind;

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
/// Transcribe 16 kHz mono audio with the model file at `model`: a GGUF file
/// runs in process on the held engine, anything else through whisper-cli.
pub fn transcribe(samples: &[f32], model: &Path) -> Result<String, Problem> {
    validate(samples)?;
    let _guard = lock()?;
    let text = match engine(model) {
        Engine::InProcess => {
            if !model.is_file() {
                return Err(missing_model(model));
            }
            held::run(model, samples, TimestampKind::None)?.text
        }
        Engine::WhisperCli => {
            let cli = ready(model)?;
            run_whisper(&cli, model, samples, &["-nt", "-otxt"], "txt").map_err(failed)?
        }
    };
    Ok(text.trim().to_string())
}

/// Like `transcribe`, with the same model and the same failures, but returns
/// each word with its time on the audio clock.
///
/// `-ml 1 -sow` makes whisper-cli emit one segment per word, and `-ojf` writes
/// each segment's offsets in ms, so every JSON segment becomes one `Word`.
///
/// whisper-cli pulls word times towards the start of its window when the
/// audio opens with silence (4 s of silence made the first word 4 s early), and
/// less so after long pauses. So the audio is cut into runs of speech with
/// `speech_spans`, each run is transcribed alone, and its offset is added back.
///
/// The in-process engine returns word times itself. The same `speech_spans`
/// cut is kept for it: times were already right without it, but it keeps one
/// decode from holding a whole long Annotate session in memory.
pub fn transcribe_timed(samples: &[f32], model: &Path) -> Result<Vec<Word>, Problem> {
    validate(samples)?;
    let _guard = lock()?;
    let cli = match engine(model) {
        Engine::InProcess if !model.is_file() => return Err(missing_model(model)),
        Engine::InProcess => None,
        Engine::WhisperCli => Some(ready(model)?),
    };
    let mut words = Vec::new();
    for span in speech_spans(samples) {
        let offset_ms = span.start as u64 * 1000 / SAMPLE_RATE as u64;
        let span_words = match &cli {
            None => engine_words(held::run(model, &samples[span], TimestampKind::Word)?),
            Some(cli) => {
                let json = run_whisper(
                    cli,
                    model,
                    &samples[span],
                    &["-ojf", "-ml", "1", "-sow"],
                    "json",
                )
                .map_err(failed)?;
                parse_words(&json).map_err(failed)?
            }
        };
        words.extend(span_words.into_iter().map(|word| Word {
            start_ms: word.start_ms + offset_ms,
            end_ms: word.end_ms + offset_ms,
            text: word.text,
        }));
    }
    Ok(words)
}

/// Which program turns a model file into text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Engine {
    /// `transcribe-cpp` in this process, on a model held in memory.
    InProcess,
    /// A new whisper-cli process per call.
    WhisperCli,
}

fn engine(model: &Path) -> Engine {
    if model.extension().is_some_and(|ext| ext == "gguf") {
        Engine::InProcess
    } else {
        Engine::WhisperCli
    }
}

/// Loads `model` and runs one short decode so the first dictation doesn't pay
/// for loading or Metal's first-run setup, or, for a whisper-cli model or a
/// missing file, lets go of whatever model is held. Call it at startup and
/// whenever the chosen model changes. Waits for a running transcription.
pub fn prepare(model: &Path) -> Result<(), Problem> {
    if engine(model) == Engine::WhisperCli || !model.is_file() {
        held::unload();
        return match engine(model) {
            Engine::InProcess => Err(missing_model(model)),
            Engine::WhisperCli => Ok(()),
        };
    }
    let started = std::time::Instant::now();
    held::run(model, &[0.0; SAMPLE_RATE], TimestampKind::None)?;
    log::info!(
        "[transcribe] ready {} in {} ms",
        model_name(model),
        started.elapsed().as_millis()
    );
    Ok(())
}

/// The model file the in-process engine holds, if any.
pub fn held_model() -> Option<PathBuf> {
    held::path()
}

/// Whether `model` needs whisper-cli installed to transcribe.
pub fn needs_whisper_cli(model: &Path) -> bool {
    engine(model) == Engine::WhisperCli
}

/// The engine's words with times in ms. Punctuation stays on its word
/// ("dog."), as `segments` expects.
fn engine_words(transcript: transcribe_cpp::Transcript) -> Vec<Word> {
    transcript
        .words
        .into_iter()
        .filter_map(|word| {
            let text = word.text.trim();
            (!text.is_empty()).then(|| Word {
                start_ms: word.t0_ms.max(0) as u64,
                end_ms: word.t1_ms.max(word.t0_ms).max(0) as u64,
                text: text.to_string(),
            })
        })
        .collect()
}

/// The one model `transcribe-cpp` keeps loaded. Loading Parakeet takes about
/// 70 ms once Metal's shaders are compiled, and its first decode 60–360 ms, so
/// it stays loaded between takes. Only one is held: loading another, or
/// `unload`, drops the old session and with it the model's memory.
mod held {
    use super::{Problem, failed};
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use transcribe_cpp::{Model, RunOptions, Session, TimestampKind, Transcript};

    struct Held {
        path: PathBuf,
        session: Session,
    }

    static HELD: Mutex<Option<Held>> = Mutex::new(None);

    fn slot() -> std::sync::MutexGuard<'static, Option<Held>> {
        HELD.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn path() -> Option<PathBuf> {
        slot().as_ref().map(|held| held.path.clone())
    }

    pub fn unload() {
        if let Some(held) = slot().take() {
            log::info!("[transcribe] unloaded {}", held.path.display());
        }
    }

    /// Runs one decode on `model`, loading it first if another model, or
    /// none, is held.
    pub fn run(
        model: &Path,
        samples: &[f32],
        timestamps: TimestampKind,
    ) -> Result<Transcript, Problem> {
        // The engine and ggml print every Metal kernel they compile. Send it
        // through `log`, where the app keeps warnings and errors only.
        static LOGGING: std::sync::Once = std::sync::Once::new();
        LOGGING.call_once(transcribe_cpp::init_logging);
        let mut slot = slot();
        if slot.as_ref().is_none_or(|held| held.path != model) {
            if let Some(old) = slot.take() {
                log::info!("[transcribe] unloaded {}", old.path.display());
            }
            let started = std::time::Instant::now();
            let session = Model::load(model)
                .and_then(|loaded| loaded.session())
                .map_err(|e| failed(format!("Load {}: {e}", model.display())))?;
            log::info!(
                "[transcribe] loaded {} in {} ms",
                model.display(),
                started.elapsed().as_millis()
            );
            *slot = Some(Held {
                path: model.to_path_buf(),
                session,
            });
        }
        let held = slot.as_mut().expect("loaded above");
        let options = RunOptions {
            timestamps,
            ..RunOptions::default()
        };
        held.session.run(samples, &options).map_err(failed)
    }
}

const SAMPLE_RATE: usize = 16_000;
/// Energy is measured over windows this long.
const VAD_WINDOW_MS: usize = 20;
/// A quiet stretch at least this long splits the audio into separate runs.
pub const SPLIT_PAUSE_MS: usize = 1_500;
/// Audio kept before and after each run, so whisper hears the word edges.
const SPAN_PAD_MS: usize = 250;
/// A run with less voiced audio than this is a click or a breath, not speech.
const MIN_VOICED_MS: usize = 100;
/// A window is voiced above this many times the noise floor (the quietest
/// tenth of windows), clamped so pure digital silence and a take that is all
/// speech both get a sensible threshold.
const VOICE_OVER_FLOOR: f32 = 4.0;
const VOICE_RMS_MIN: f32 = 0.005;
const VOICE_RMS_MAX: f32 = 0.02;

/// Sample ranges that hold speech, split at pauses of `SPLIT_PAUSE_MS` or
/// more and padded by `SPAN_PAD_MS`. A simple energy detector: 16 kHz mono in.
pub fn speech_spans(samples: &[f32]) -> Vec<std::ops::Range<usize>> {
    let window = SAMPLE_RATE * VAD_WINDOW_MS / 1000;
    let rms: Vec<f32> = samples
        .chunks(window)
        .map(|chunk| (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt())
        .collect();
    if rms.is_empty() {
        return Vec::new();
    }
    let mut sorted = rms.clone();
    sorted.sort_by(f32::total_cmp);
    let floor = sorted[sorted.len() / 10];
    let threshold = (floor * VOICE_OVER_FLOOR).clamp(VOICE_RMS_MIN, VOICE_RMS_MAX);

    let split_windows = SPLIT_PAUSE_MS / VAD_WINDOW_MS;
    let mut runs: Vec<(usize, usize, usize)> = Vec::new();
    for (index, _) in rms.iter().enumerate().filter(|(_, r)| **r >= threshold) {
        match runs.last_mut() {
            Some((_, last, voiced)) if index - *last < split_windows => {
                *last = index;
                *voiced += 1;
            }
            _ => runs.push((index, index, 1)),
        }
    }
    let pad = SAMPLE_RATE * SPAN_PAD_MS / 1000;
    runs.into_iter()
        .filter(|&(_, _, voiced)| voiced * VAD_WINDOW_MS >= MIN_VOICED_MS)
        .map(|(first, last, _)| {
            (first * window).saturating_sub(pad)..((last + 1) * window + pad).min(samples.len())
        })
        .collect()
}

/// Splits after a word ending a sentence, or at a pause of `SEGMENT_PAUSE_MS`
/// or more before the next word.
pub fn segments(words: &[Word]) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut current: Vec<&Word> = Vec::new();
    for (index, word) in words.iter().enumerate() {
        current.push(word);
        let ends_sentence = ends_sentence(&word.text);
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

/// Abbreviations that end in a full stop without ending a sentence. Compared
/// lowercased, with surrounding quotes and brackets removed.
const ABBREVIATIONS: &[&str] = &[
    "e.g.", "i.e.", "etc.", "vs.", "approx.", "mr.", "mrs.", "ms.", "dr.", "st.", "no.", "fig.",
    "cf.", "a.m.", "p.m.",
];

fn ends_sentence(word: &str) -> bool {
    let bare = word
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '“' | '”' | '‘' | '’' | '(' | ')'))
        .to_lowercase();
    bare.ends_with(['!', '?']) || (bare.ends_with('.') && !ABBREVIATIONS.contains(&bare.as_str()))
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
        let _turn = transcription_turn();
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
    fn segments_do_not_split_after_common_abbreviations() {
        let words = [
            word(0, 300, "Use"),
            word(300, 600, "a"),
            word(600, 900, "verb,"),
            word(900, 1200, "e.g."),
            word(1200, 1500, "Save,"),
            word(1500, 1800, "not"),
            word(1800, 2100, "OK"),
            word(2100, 2400, "etc."),
            word(2400, 2700, "Approx."),
            word(2700, 3000, "done."),
            word(3000, 3300, "Next"),
        ];
        let texts: Vec<String> = segments(&words).into_iter().map(|s| s.text).collect();
        assert_eq!(
            texts,
            ["Use a verb, e.g. Save, not OK etc. Approx. done.", "Next"]
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

    fn tone(ms: u64) -> Vec<f32> {
        (0..ms * 16)
            .map(|i| (i as f32 * 0.07).sin() * 0.1)
            .collect()
    }

    fn spans_ms(samples: &[f32]) -> Vec<(u64, u64)> {
        speech_spans(samples)
            .into_iter()
            .map(|span| (span.start as u64 / 16, span.end as u64 / 16))
            .collect()
    }

    #[test]
    fn speech_spans_skip_leading_and_trailing_silence() {
        let mut samples = near_silence(4_000);
        samples.extend(tone(2_000));
        samples.extend(near_silence(3_000));
        assert_eq!(spans_ms(&samples), [(3_750, 6_250)]);
    }

    #[test]
    fn speech_spans_split_only_at_long_pauses() {
        let mut samples = tone(1_000);
        samples.extend(near_silence(1_000));
        samples.extend(tone(1_000));
        samples.extend(near_silence(SPLIT_PAUSE_MS as u64 + 500));
        samples.extend(tone(1_000));
        assert_eq!(spans_ms(&samples), [(0, 3_250), (4_750, 6_000)]);
    }

    #[test]
    fn speech_spans_ignore_silence_and_clicks() {
        assert_eq!(spans_ms(&[]), []);
        assert_eq!(spans_ms(&vec![0.0; 32_000]), []);
        let mut samples = near_silence(2_000);
        samples.extend(tone(40));
        samples.extend(near_silence(2_000));
        assert_eq!(spans_ms(&samples), []);
    }

    #[test]
    fn no_words_make_no_segments() {
        assert_eq!(segments(&[]), []);
    }

    /// `transcribe` refuses to run twice at once, and the held engine is one
    /// slot, so the tests that transcribe take turns instead of failing each
    /// other.
    fn transcription_turn() -> std::sync::MutexGuard<'static, ()> {
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
        let _turn = transcription_turn();
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

    /// Room-noise stand-in: deterministic noise at about -50 dBFS.
    fn near_silence(ms: u64) -> Vec<f32> {
        let mut state: u32 = 0x9E37_79B9;
        (0..ms * 16)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state as f32 / u32::MAX as f32 - 0.5) * 0.006
            })
            .collect()
    }

    /// Where the speech in a clean `say` clip really starts, in ms: the first
    /// 10 ms window louder than -40 dBFS.
    fn onset_ms(samples: &[f32]) -> u64 {
        let window = 160;
        let first = samples
            .chunks(window)
            .position(|chunk| {
                (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt() > 0.01
            })
            .expect("the clip has speech");
        (first * window) as u64 / 16
    }

    fn report_onset(label: &str, words: &[Word], expected_ms: u64) -> u64 {
        let first = words.first().expect("whisper heard words");
        let error = first.start_ms.abs_diff(expected_ms);
        eprintln!(
            "{label}: first word {:?} at {} ms, true onset {expected_ms} ms, error {error} ms",
            first.text, first.start_ms
        );
        error
    }

    #[test]
    fn leading_silence_keeps_word_times_on_the_audio_clock() {
        let _turn = transcription_turn();
        let test = "leading silence word times";
        let Some(speech) = generated_speech(test, "Move the button. Then make it blue.") else {
            return;
        };
        let mut samples = near_silence(4_000);
        samples.extend(&speech);
        samples.extend(near_silence(4_000));
        let expected = 4_000 + onset_ms(&speech);

        let words = transcribe_timed(&samples, &test_model()).unwrap();
        let error = report_onset(test, &words, expected);
        assert!(error <= ONSET_TOLERANCE_MS, "words: {words:?}");
        let speech_end = 4_000 + speech.len() as u64 / 16;
        assert!(
            words.last().unwrap().end_ms <= speech_end + ONSET_TOLERANCE_MS,
            "words: {words:?}"
        );
    }

    #[test]
    fn a_long_pause_mid_take_keeps_later_words_on_time() {
        let _turn = transcription_turn();
        let test = "mid-take pause word times";
        let Some(first) = generated_speech(test, "Move the button.") else {
            return;
        };
        let Some(second) = generated_speech(test, "Then make it blue.") else {
            return;
        };
        let mut samples = near_silence(1_000);
        samples.extend(&first);
        samples.extend(near_silence(4_000));
        let second_start = samples.len() as u64 / 16;
        samples.extend(&second);
        samples.extend(near_silence(1_000));

        let words = transcribe_timed(&samples, &test_model()).unwrap();
        report_onset(test, &words, 1_000 + onset_ms(&first));
        let then = words
            .iter()
            .position(|w| w.text.to_lowercase().starts_with("then"))
            .unwrap_or_else(|| panic!("no \"then\" in {words:?}"));
        let error = report_onset(test, &words[then..], second_start + onset_ms(&second));
        assert!(error <= ONSET_TOLERANCE_MS, "words: {words:?}");
    }

    const ONSET_TOLERANCE_MS: u64 = 300;

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
        let _turn = transcription_turn();
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

    /// Parakeet Unified for the in-process tests: `DICTATE_PARAKEET_MODEL`,
    /// else this checkout's `models/` folder.
    fn parakeet_model() -> PathBuf {
        std::env::var_os("DICTATE_PARAKEET_MODEL")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../models")
                    .join(
                        crate::models::info(crate::models::SpeechModel::ParakeetUnifiedEn)
                            .file_name,
                    )
            })
    }

    fn parakeet_speech(test: &str, text: &str) -> Option<Vec<f32>> {
        let model = parakeet_model();
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

    const FOX: &str = "The quick brown fox jumps over the lazy dog.";

    #[test]
    fn gguf_files_run_in_process_and_the_rest_through_whisper_cli() {
        assert_eq!(engine(Path::new("/m/parakeet.gguf")), Engine::InProcess);
        assert_eq!(engine(Path::new("/m/ggml-base.en.bin")), Engine::WhisperCli);
        assert!(!needs_whisper_cli(&parakeet_model()));
    }

    #[test]
    fn missing_gguf_model_points_to_the_models_page_without_whisper_cli() {
        let _turn = transcription_turn();
        let model = Path::new("/nonexistent/parakeet.gguf");
        for problem in [
            transcribe(&[0.1], model).unwrap_err(),
            transcribe_timed(&[0.1], model).unwrap_err(),
            prepare(model).unwrap_err(),
        ] {
            assert_eq!(problem.title, "Speech model missing");
            assert!(
                problem
                    .detail
                    .unwrap()
                    .contains("/nonexistent/parakeet.gguf")
            );
        }
        assert_eq!(held_model(), None);
    }

    #[test]
    fn engine_words_trim_text_and_keep_punctuation() {
        let word = |t0_ms, t1_ms, text: &str| transcribe_cpp::Word {
            t0_ms,
            t1_ms,
            text: text.into(),
            ..Default::default()
        };
        let transcript = transcribe_cpp::Transcript {
            words: vec![
                word(480, 640, " The"),
                word(-20, 10, "dog."),
                word(900, 880, "late"),
                word(1000, 1100, " "),
            ],
            ..Default::default()
        };
        assert_eq!(
            engine_words(transcript),
            [
                super::Word {
                    start_ms: 480,
                    end_ms: 640,
                    text: "The".into()
                },
                super::Word {
                    start_ms: 0,
                    end_ms: 10,
                    text: "dog.".into()
                },
                super::Word {
                    start_ms: 900,
                    end_ms: 900,
                    text: "late".into()
                },
            ]
        );
    }

    #[test]
    fn parakeet_transcribes_the_fox_exactly() {
        let _turn = transcription_turn();
        let Some(samples) = parakeet_speech("parakeet fox", FOX) else {
            return;
        };
        let model = parakeet_model();
        prepare(&model).unwrap();
        let start = std::time::Instant::now();
        let text = transcribe(&samples, &model).unwrap();
        eprintln!(
            "parakeet fox: warm transcribe_ms={} text={text:?}",
            start.elapsed().as_millis()
        );
        assert_eq!(text, FOX);
    }

    #[test]
    fn parakeet_word_times_after_three_seconds_of_silence() {
        let _turn = transcription_turn();
        let test = "parakeet leading silence";
        let Some(fox) = parakeet_speech(test, FOX) else {
            return;
        };
        let mut samples = vec![0.0; 3 * SAMPLE_RATE];
        samples.extend(&fox);
        let words = transcribe_timed(&samples, &parakeet_model()).unwrap();
        let error = report_onset(test, &words, 3_480);
        assert!(error <= 250, "words: {words:?}");
        let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts.join(" "), FOX);
        for pair in words.windows(2) {
            assert!(pair[0].start_ms <= pair[1].start_ms, "words: {words:?}");
        }
    }

    #[test]
    fn switching_models_unloads_the_held_one() {
        let _turn = transcription_turn();
        let model = parakeet_model();
        if !model.is_file() {
            eprintln!("SKIP switching models: no model at {}", model.display());
            return;
        }
        prepare(&model).unwrap();
        assert_eq!(held_model(), Some(model.clone()));
        prepare(Path::new("/models/ggml-base.en.bin")).unwrap();
        assert_eq!(held_model(), None);
        prepare(&model).unwrap();
        assert_eq!(held_model(), Some(model));
        prepare(Path::new("/models/deleted.gguf")).unwrap_err();
        assert_eq!(held_model(), None);
    }

    /// Warm Parakeet on the fox clip (n = 11) and on `measure_catalog_models`'
    /// 5.9 s sentence (n = 5), after one warm-up. Asserts the fox median is
    /// within 100 ms. Run alone, so other tests don't share the GPU.
    #[test]
    #[ignore = "Timing; run with --ignored --nocapture --test-threads=1"]
    fn measure_parakeet_warm_latency() {
        let _turn = transcription_turn();
        let model = parakeet_model();
        let cold = std::time::Instant::now();
        prepare(&model).unwrap();
        println!("prepare_ms={}", cold.elapsed().as_millis());
        let median = |samples: &[f32], runs: usize| {
            let mut timings: Vec<u128> = (0..runs)
                .map(|_| {
                    let start = std::time::Instant::now();
                    transcribe(samples, &model).unwrap();
                    start.elapsed().as_millis()
                })
                .collect();
            timings.sort();
            (timings[runs / 2], timings[0], timings[runs - 1])
        };
        let fox = say(FOX);
        let (fox_median, min, max) = median(&fox, 11);
        println!(
            "fox seconds={:.2} median_ms={fox_median} min_ms={min} max_ms={max}",
            fox.len() as f64 / 16_000.0
        );
        let sentence = say("Please move the Thursday design review to Friday morning, \
             and send Priya the updated budget before lunch.");
        let (sentence_median, min, max) = median(&sentence, 5);
        println!(
            "sentence seconds={:.2} median_ms={sentence_median} min_ms={min} max_ms={max} text={:?}",
            sentence.len() as f64 / 16_000.0,
            transcribe(&sentence, &model).unwrap()
        );
        assert!(fox_median <= 100, "fox median {fox_median} ms");
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
