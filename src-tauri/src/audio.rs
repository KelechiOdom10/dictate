use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{FftFixedInOut, Resampler};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

const OUTPUT_RATE: usize = 16_000;
type Reply<T> = mpsc::Sender<Result<T, String>>;

enum Command {
    Start(Reply<()>),
    Stop(Reply<Vec<f32>>),
    Status(mpsc::Sender<bool>),
}

#[derive(Default)]
struct Captured {
    samples: Vec<f32>,
    error: Option<String>,
}

struct Recording {
    stream: cpal::Stream,
    captured: Arc<Mutex<Captured>>,
    rate: usize,
}

fn controller() -> Result<&'static Mutex<mpsc::Sender<Command>>, String> {
    static CONTROLLER: OnceLock<Result<Mutex<mpsc::Sender<Command>>, String>> = OnceLock::new();
    CONTROLLER
        .get_or_init(|| {
            let (sender, receiver) = mpsc::channel();
            std::thread::Builder::new()
                .name("microphone".into())
                .spawn(move || {
                    let mut recording: Option<Recording> = None;
                    for command in receiver {
                        match command {
                            Command::Start(reply) => {
                                let result = if let Some(active) = &recording {
                                    active
                                        .captured
                                        .lock()
                                        .map_err(|e| e.to_string())
                                        .and_then(|data| data.error.clone().map_or(Ok(()), Err))
                                } else {
                                    open_recording().map(|active| recording = Some(active))
                                };
                                let _ = reply.send(result);
                            }
                            Command::Stop(reply) => {
                                let result = recording
                                    .take()
                                    .ok_or_else(|| "Microphone is not recording".into())
                                    .and_then(Recording::finish);
                                let _ = reply.send(result);
                            }
                            Command::Status(reply) => {
                                let active = recording.as_ref().is_some_and(|active| {
                                    active
                                        .captured
                                        .lock()
                                        .is_ok_and(|data| data.error.is_none())
                                });
                                let _ = reply.send(active);
                            }
                        }
                    }
                })
                .map_err(|e| format!("Start audio thread: {e}"))?;
            Ok(Mutex::new(sender))
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn send(command: Command) -> Result<(), String> {
    controller()?
        .lock()
        .map_err(|e| e.to_string())?
        .send(command)
        .map_err(|e| format!("Audio thread unavailable: {e}"))
}

pub fn is_recording() -> bool {
    let (reply, result) = mpsc::channel();
    send(Command::Status(reply)).is_ok() && result.recv().unwrap_or(false)
}

pub fn start() -> Result<(), String> {
    let (reply, result) = mpsc::channel();
    send(Command::Start(reply))?;
    result
        .recv()
        .map_err(|e| format!("Audio start reply: {e}"))?
}

pub fn stop() -> Result<Vec<f32>, String> {
    let (reply, result) = mpsc::channel();
    send(Command::Stop(reply))?;
    result
        .recv()
        .map_err(|e| format!("Audio stop reply: {e}"))?
}

fn open_recording() -> Result<Recording, String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("No default microphone")?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("Microphone config: {e}"))?;
    let name = device.name().map_err(|e| format!("Microphone name: {e}"))?;
    println!(
        "[audio] device={name:?} rate={} channels={} format={:?}",
        config.sample_rate().0,
        config.channels(),
        config.sample_format()
    );
    let captured = Arc::new(Mutex::new(Captured::default()));
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config.config(), &captured)?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config.config(), &captured)?,
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config.config(), &captured)?,
        format => return Err(format!("Unsupported microphone sample format: {format}")),
    };
    stream
        .play()
        .map_err(|e| format!("Start microphone: {e}"))?;
    Ok(Recording {
        stream,
        captured,
        rate: config.sample_rate().0 as usize,
    })
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    captured: &Arc<Mutex<Captured>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let samples = Arc::clone(captured);
    let errors = Arc::clone(captured);
    let channels = config.channels as usize;
    if channels == 0 {
        return Err("Microphone has no channels".into());
    }
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if let Ok(mut captured) = samples.lock() {
                    append_mono(&mut captured.samples, data, channels);
                }
            },
            move |error| {
                if let Ok(mut captured) = errors.lock() {
                    captured.error = Some(format!("Microphone stream: {error}"));
                }
            },
            None,
        )
        .map_err(|e| format!("Build microphone stream: {e}"))
}

fn append_mono<T>(output: &mut Vec<f32>, input: &[T], channels: usize)
where
    T: cpal::Sample,
    f32: cpal::FromSample<T>,
{
    output.extend(input.chunks_exact(channels).map(|frame| {
        frame
            .iter()
            .map(|sample| sample.to_sample::<f32>().clamp(-1.0, 1.0))
            .sum::<f32>()
            / channels as f32
    }));
}

impl Recording {
    fn finish(self) -> Result<Vec<f32>, String> {
        drop(self.stream);
        let mut captured = self.captured.lock().map_err(|e| e.to_string())?;
        if let Some(error) = captured.error.take() {
            return Err(error);
        }
        let samples = std::mem::take(&mut captured.samples);
        if samples.is_empty() {
            return Err("Microphone delivered no samples".into());
        }
        resample(samples, self.rate)
    }
}

fn resample(samples: Vec<f32>, rate: usize) -> Result<Vec<f32>, String> {
    if rate == OUTPUT_RATE {
        return Ok(samples);
    }
    let length = samples.len() * OUTPUT_RATE / rate;
    let mut resampler = FftFixedInOut::<f32>::new(rate, OUTPUT_RATE, 1024, 1)
        .map_err(|e| format!("Create resampler: {e}"))?;
    let delay = resampler.output_delay();
    let chunk = resampler.input_frames_next();
    let mut output = Vec::new();
    for block in samples.chunks(chunk) {
        let mut padded = vec![0.0; chunk];
        padded[..block.len()].copy_from_slice(block);
        let converted = resampler
            .process(&[padded], None)
            .map_err(|e| format!("Resample audio: {e}"))?;
        output.extend_from_slice(&converted[0]);
    }
    while output.len() < length + delay {
        let converted = resampler
            .process(&[vec![0.0; chunk]], None)
            .map_err(|e| format!("Flush resampler: {e}"))?;
        output.extend_from_slice(&converted[0]);
    }
    Ok(output[delay..delay + length].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_sine_keeps_duration_pitch_and_amplitude() {
        let stereo: Vec<f32> = (0..48_000)
            .flat_map(|i| {
                let sample = (i as f32 * std::f32::consts::TAU * 440.0 / 48_000.0).sin() * 0.5;
                [sample, sample]
            })
            .collect();
        let mut mono = Vec::new();
        append_mono(&mut mono, &stereo, 2);
        let output = resample(mono, 48_000).unwrap();
        assert_eq!(output.len(), 16_000);
        let error = output[100..15_900]
            .iter()
            .enumerate()
            .map(|(i, value)| {
                let expected =
                    ((i + 100) as f32 * std::f32::consts::TAU * 440.0 / 16_000.0).sin() * 0.5;
                (value - expected).abs()
            })
            .sum::<f32>()
            / 15_800.0;
        assert!(error < 0.01, "mean error {error}");
    }

    #[test]
    fn resampling_filters_above_nyquist_and_flushes_short_recordings() {
        let input = (0..48_000)
            .map(|i| (i as f32 * std::f32::consts::TAU * 12_000.0 / 48_000.0).sin())
            .collect();
        let output = resample(input, 48_000).unwrap();
        let rms = (output[100..15_900].iter().map(|x| x * x).sum::<f32>() / 15_800.0).sqrt();
        assert!(rms < 0.01, "aliased RMS {rms}");
        let short = resample(vec![0.5; 441], 44_100).unwrap();
        assert_eq!(short.len(), 160);
        assert!((short[80] - 0.5).abs() < 0.01);
    }

    #[test]
    fn integer_formats_and_downmix_preserve_range() {
        let mut output = Vec::new();
        append_mono(&mut output, &[i16::MIN, i16::MAX, 16384, 16384], 2);
        assert!(output[0].abs() < 0.0001);
        assert_eq!(output[1], 0.5);
        output.clear();
        append_mono(&mut output, &[0_u16, u16::MAX, 32768, 32768], 2);
        assert!(output[0].abs() < 0.0001);
        assert_eq!(output[1], 0.0);
    }
}
