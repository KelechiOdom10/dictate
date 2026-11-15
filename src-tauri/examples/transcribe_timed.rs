//! Run the `transcribe_timed` stage alone on a WAV file and print each word
//! and each rebuilt segment with its times.
//!
//!   cargo run --example transcribe_timed -- path/to/speech.wav
//!
//! The WAV must be 16 kHz mono 16-bit PCM, the repo's audio format, e.g.
//! `afconvert -f WAVE -d LEI16@16000 -c 1 in.aiff out.wav`.
//! Set DICTATE_WHISPER_MODEL to try another model.

use dictate_lib::transcribe;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: transcribe_timed <16 kHz mono WAV>")?;
    let mut reader = hound::WavReader::open(&path).map_err(|e| format!("Open {path}: {e}"))?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000 || spec.channels != 1 || spec.bits_per_sample != 16 {
        return Err(format!(
            "{path} is {} Hz, {} channel(s), {} bit; expected 16000 Hz mono 16 bit",
            spec.sample_rate, spec.channels, spec.bits_per_sample
        ));
    }
    let samples = reader
        .samples::<i16>()
        .map(|sample| sample.map(|s| f32::from(s) / 32768.0))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Read {path}: {e}"))?;

    let started = std::time::Instant::now();
    let words = transcribe::transcribe_timed(&samples)?;
    let elapsed_ms = started.elapsed().as_millis();

    for word in &words {
        println!("{:>7} {:>7}  {}", word.start_ms, word.end_ms, word.text);
    }
    println!();
    for segment in transcribe::segments(&words) {
        println!(
            "{:>7} {:>7}  {}",
            segment.start_ms, segment.end_ms, segment.text
        );
    }
    println!(
        "\naudio {} ms, {} words, transcribe_timed {elapsed_ms} ms",
        samples.len() * 1000 / 16_000,
        words.len()
    );
    Ok(())
}
