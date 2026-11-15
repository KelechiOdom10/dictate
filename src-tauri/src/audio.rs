use crate::problem::Problem;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{FftFixedInOut, Resampler};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock, mpsc};

const OUTPUT_RATE: usize = 16_000;
/// Audio a warm microphone keeps from just before the shortcut, so the first
/// word is not clipped while the take starts.
const PREROLL_MS: usize = 300;
type Reply<T> = mpsc::Sender<Result<T, String>>;

enum Command {
    Start(Option<String>, std::time::Duration, Reply<bool>),
    Monitor(Option<String>, Reply<()>),
    StopMonitor,
    Level(Reply<f32>),
    RecordingLevel(Reply<f32>),
    Stop(mpsc::Sender<Result<Vec<f32>, Problem>>),
    Status(mpsc::Sender<bool>),
    /// `Some(device)` keeps that microphone open between takes; `None` closes it.
    Warm(Option<Option<String>>, Reply<()>),
}

#[derive(Default)]
struct Captured {
    samples: Vec<f32>,
    error: Option<String>,
    level: f32,
    retain: bool,
    preroll: VecDeque<f32>,
    preroll_frames: usize,
    /// Frames a take drops before it keeps any, so the start sound is not in it.
    skip_frames: usize,
}

struct Recording {
    stream: cpal::Stream,
    captured: Arc<Mutex<Captured>>,
    rate: usize,
    fallback: bool,
    preference: Option<String>,
    /// The microphone the stream actually opened.
    device: String,
}

fn controller() -> Result<&'static mpsc::Sender<Command>, String> {
    static CONTROLLER: OnceLock<Result<mpsc::Sender<Command>, String>> = OnceLock::new();
    CONTROLLER
        .get_or_init(|| {
            let (sender, receiver) = mpsc::channel();
            std::thread::Builder::new()
                .name("microphone".into())
                .spawn(move || {
                    let mut recording: Option<Recording> = None;
                    let mut monitor: Option<Recording> = None;
                    let mut warm_device: Option<Option<String>> = None;
                    let mut warm: Option<Recording> = None;
                    for command in receiver {
                        match command {
                            Command::Start(device, skip, reply) => {
                                let result = if let Some(active) = &recording {
                                    active.captured.lock().map_err(|e| e.to_string()).and_then(
                                        |data| data.error.clone().map_or(Ok(active.fallback), Err),
                                    )
                                } else {
                                    let armed = warm
                                        .take()
                                        .filter(|idle| idle.serves(&device))
                                        .and_then(|idle| idle.arm().ok());
                                    armed
                                        .map(Ok)
                                        .unwrap_or_else(|| {
                                            open_recording(device.as_deref(), Mode::Take)
                                        })
                                        .map(|active| {
                                            let fallback = active.fallback;
                                            active.skip(skip);
                                            recording = Some(active);
                                            fallback
                                        })
                                };
                                let _ = reply.send(result);
                            }
                            Command::Warm(device, reply) => {
                                warm_device = device;
                                let result = match &warm_device {
                                    None => {
                                        warm = None;
                                        Ok(())
                                    }
                                    Some(device) => {
                                        if warm.as_ref().is_some_and(|idle| {
                                            !idle.serves(device) || idle.failed()
                                        }) {
                                            warm = None;
                                        }
                                        if warm.is_none() && recording.is_none() {
                                            open_recording(device.as_deref(), Mode::Warm)
                                                .map(|idle| warm = Some(idle))
                                        } else {
                                            Ok(())
                                        }
                                    }
                                };
                                let _ = reply.send(result);
                            }
                            Command::Monitor(device, reply) => {
                                monitor = None;
                                let result = open_recording(device.as_deref(), Mode::Monitor)
                                    .map(|active| monitor = Some(active));
                                let _ = reply.send(result);
                            }
                            Command::StopMonitor => monitor = None,
                            Command::Level(ref reply) | Command::RecordingLevel(ref reply) => {
                                let active = if matches!(command, Command::RecordingLevel(_)) {
                                    &recording
                                } else {
                                    &monitor
                                };
                                let result = active
                                    .as_ref()
                                    .ok_or_else(|| "Microphone test is stopped".to_string())
                                    .and_then(|active| {
                                        active.captured.lock().map_err(|e| e.to_string())
                                    })
                                    .and_then(|data| {
                                        data.error.clone().map_or(Ok(data.level), Err)
                                    });
                                let _ = reply.send(result);
                            }
                            Command::Stop(reply) => {
                                let result = match recording.take() {
                                    None => Err(Problem::new(
                                        "Recording already stopped",
                                        "Hold the shortcut and speak again.",
                                    )),
                                    Some(active)
                                        if warm_device.as_ref() == Some(&active.preference)
                                            && !active.failed() =>
                                    {
                                        let samples = active.drain();
                                        warm = Some(active);
                                        samples
                                    }
                                    Some(active) => active.finish(),
                                };
                                let _ = reply.send(result);
                                if let (None, Some(device)) = (&warm, &warm_device) {
                                    match open_recording(device.as_deref(), Mode::Warm) {
                                        Ok(idle) => warm = Some(idle),
                                        Err(error) => log::warn!("[audio] keep warm: {error}"),
                                    }
                                }
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
            Ok(sender)
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn send(command: Command) -> Result<(), String> {
    controller()?
        .send(command)
        .map_err(|e| format!("Audio thread unavailable: {e}"))
}

pub fn is_recording() -> bool {
    let (reply, result) = mpsc::channel();
    send(Command::Status(reply)).is_ok() && result.recv().unwrap_or(false)
}

pub fn start() -> Result<(), Problem> {
    start_device(None, std::time::Duration::ZERO).map(|_| ())
}

/// Starts a take on `device`, dropping its first `skip` of new audio.
pub fn start_device(device: Option<String>, skip: std::time::Duration) -> Result<bool, Problem> {
    let (reply, result) = mpsc::channel();
    send(Command::Start(device, skip, reply))
        .and_then(|()| {
            result
                .recv()
                .map_err(|e| format!("Audio start reply: {e}"))?
        })
        .map_err(|cause| {
            Problem::new(
                "Microphone unavailable",
                "Choose another microphone in Settings, then try again.",
            )
            .because(cause)
        })
}

pub fn stop() -> Result<Vec<f32>, Problem> {
    let (reply, result) = mpsc::channel();
    send(Command::Stop(reply)).map_err(recording_failed)?;
    result
        .recv()
        .map_err(|e| recording_failed(format!("Audio stop reply: {e}")))?
}

pub fn no_samples() -> Problem {
    Problem::new(
        "No sound from the microphone",
        "Check it in Settings → General → Test microphone.",
    )
    .because("Microphone delivered no samples")
}

fn recording_failed(cause: String) -> Problem {
    Problem::new("Recording failed", "Hold the shortcut and try again.").because(cause)
}

type Devices = Vec<(String, cpal::Device)>;

fn device_cache() -> &'static Mutex<Option<Devices>> {
    static DEVICES: OnceLock<Mutex<Option<Devices>>> = OnceLock::new();
    DEVICES.get_or_init(|| Mutex::new(None))
}

fn list_devices() -> Result<Devices, String> {
    cpal::default_host()
        .input_devices()
        .map_err(|e| e.to_string())?
        .map(|device| {
            device
                .name()
                .map(|name| (name, device))
                .map_err(|e| e.to_string())
        })
        .collect()
}

pub fn input_devices() -> Result<Vec<String>, String> {
    let devices = list_devices()?;
    let names = devices.iter().map(|(name, _)| name.clone()).collect();
    *device_cache().lock().map_err(|e| e.to_string())? = Some(devices);
    Ok(names)
}

fn selected_device(
    name: &str,
) -> Result<Option<(cpal::Device, cpal::SupportedStreamConfig)>, String> {
    let mut cache = device_cache().lock().map_err(|e| e.to_string())?;
    if let Some(device) = cache
        .as_ref()
        .and_then(|devices| devices.iter().find(|(label, _)| label == name))
        .map(|(_, device)| device)
        && let Ok(config) = device.default_input_config()
    {
        return Ok(Some((device.clone(), config)));
    }
    let devices = list_devices()?;
    let device = devices
        .iter()
        .find(|(label, _)| label == name)
        .map(|(_, device)| device.clone());
    *cache = Some(devices);
    device
        .map(|device| {
            let config = device
                .default_input_config()
                .map_err(|e| format!("Microphone config: {e}"))?;
            Ok((device, config))
        })
        .transpose()
}

/// Keeps `device` open between takes (`Some`), or closes it (`None`).
pub fn keep_warm(device: Option<Option<String>>) -> Result<(), String> {
    let (reply, result) = mpsc::channel();
    send(Command::Warm(device, reply))?;
    result.recv().map_err(|e| e.to_string())?
}

pub fn monitor_start(device: Option<String>) -> Result<(), String> {
    let (reply, result) = mpsc::channel();
    send(Command::Monitor(device, reply))?;
    result.recv().map_err(|e| e.to_string())?
}

pub fn monitor_stop() -> Result<(), String> {
    send(Command::StopMonitor)
}

pub fn microphone_level() -> Result<f32, String> {
    let (reply, result) = mpsc::channel();
    send(Command::Level(reply))?;
    result.recv().map_err(|e| e.to_string())?
}

pub fn recording_level() -> Result<f32, String> {
    let (reply, result) = mpsc::channel();
    send(Command::RecordingLevel(reply))?;
    result.recv().map_err(|e| e.to_string())?
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Keep every sample for transcription.
    Take,
    /// Report the level only.
    Monitor,
    /// Stay open between takes, keeping only the pre-roll.
    Warm,
}

fn default_device_name() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|device| device.name().ok())
}

/// Whether a warm stream that `opened` for `preference` still records what
/// `requested` asks for. A stream that opened the system default (because
/// nothing was named, or the named microphone was missing) is stale once the
/// default changes, for example when AirPods connect.
fn warm_stream_serves(
    preference: &Option<String>,
    opened: &str,
    requested: &Option<String>,
    current_default: impl FnOnce() -> Option<String>,
) -> bool {
    if preference != requested {
        return false;
    }
    match requested {
        Some(name) if name == opened => true,
        _ => current_default().as_deref() == Some(opened),
    }
}

fn open_recording(name: Option<&str>, mode: Mode) -> Result<Recording, String> {
    let preference = name.map(str::to_owned);
    let host = cpal::default_host();
    let selected = name.map(selected_device).transpose()?.flatten();
    let fallback = name.is_some() && selected.is_none();
    let (device, config) = match selected {
        Some(selected) => selected,
        None => {
            let device = host.default_input_device().ok_or("No default microphone")?;
            let config = device
                .default_input_config()
                .map_err(|e| format!("Microphone config: {e}"))?;
            (device, config)
        }
    };
    let name = device.name().map_err(|e| format!("Microphone name: {e}"))?;
    log::info!(
        "[audio] device={name:?} rate={} channels={} format={:?}",
        config.sample_rate().0,
        config.channels(),
        config.sample_format()
    );
    let rate = config.sample_rate().0 as usize;
    let captured = Arc::new(Mutex::new(Captured {
        retain: mode == Mode::Take,
        preroll_frames: if mode == Mode::Warm {
            rate * PREROLL_MS / 1000
        } else {
            0
        },
        ..Captured::default()
    }));
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
        rate,
        fallback,
        preference,
        device: name,
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
                    capture_block(&mut captured, data, channels);
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

fn capture_block<T>(captured: &mut Captured, input: &[T], channels: usize)
where
    T: cpal::Sample,
    f32: cpal::FromSample<T>,
{
    let frames = input.chunks_exact(channels);
    let count = frames.len().max(1);
    let mut squares = 0.0;
    for frame in frames {
        let mono = frame
            .iter()
            .map(|sample| sample.to_sample::<f32>().clamp(-1.0, 1.0))
            .sum::<f32>()
            / channels as f32;
        squares += mono * mono;
        if captured.retain && captured.skip_frames > 0 {
            captured.skip_frames -= 1;
        } else if captured.retain {
            captured.samples.push(mono);
        } else if captured.preroll_frames > 0 {
            if captured.preroll.len() == captured.preroll_frames {
                captured.preroll.pop_front();
            }
            captured.preroll.push_back(mono);
        }
    }
    captured.level = (squares / count as f32).sqrt();
}

#[cfg(test)]
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
    fn serves(&self, requested: &Option<String>) -> bool {
        warm_stream_serves(
            &self.preference,
            &self.device,
            requested,
            default_device_name,
        )
    }

    fn skip(&self, duration: std::time::Duration) {
        if let Ok(mut captured) = self.captured.lock() {
            captured.skip_frames = (self.rate as u128 * duration.as_millis() / 1000) as usize;
        }
    }

    fn failed(&self) -> bool {
        self.captured
            .lock()
            .map_or(true, |captured| captured.error.is_some())
    }

    /// Turns an idle warm stream into a take, starting with its pre-roll.
    fn arm(self) -> Result<Self, String> {
        {
            let mut captured = self.captured.lock().map_err(|e| e.to_string())?;
            if let Some(error) = captured.error.clone() {
                return Err(error);
            }
            let preroll = std::mem::take(&mut captured.preroll);
            captured.samples.clear();
            captured.samples.extend(preroll);
            captured.retain = true;
        }
        Ok(self)
    }

    /// Ends a take but keeps the stream open, back in pre-roll mode.
    fn drain(&self) -> Result<Vec<f32>, Problem> {
        let samples = {
            let mut captured = self
                .captured
                .lock()
                .map_err(|e| recording_failed(e.to_string()))?;
            captured.retain = false;
            captured.skip_frames = 0;
            captured.preroll.clear();
            std::mem::take(&mut captured.samples)
        };
        Self::check(samples, self.rate, None)
    }

    fn finish(self) -> Result<Vec<f32>, Problem> {
        drop(self.stream);
        let mut captured = self
            .captured
            .lock()
            .map_err(|e| recording_failed(e.to_string()))?;
        let error = captured.error.take();
        let samples = std::mem::take(&mut captured.samples);
        Self::check(samples, self.rate, error)
    }

    fn check(samples: Vec<f32>, rate: usize, error: Option<String>) -> Result<Vec<f32>, Problem> {
        if let Some(error) = error {
            return Err(Problem::new(
                "Microphone disconnected",
                "Reconnect it or choose another microphone in Settings.",
            )
            .because(error));
        }
        if samples.is_empty() {
            return Err(no_samples());
        }
        resample(samples, rate).map_err(recording_failed)
    }
}

/// Writes 16 kHz mono samples as a 32-bit float WAV at `path`.
pub fn save_wav(path: &std::path::Path, samples: &[f32]) -> Result<(), String> {
    let mut writer = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 1,
            sample_rate: OUTPUT_RATE as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .map_err(|e| e.to_string())?;
    for sample in samples {
        writer.write_sample(*sample).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
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
    #[ignore = "Measures real named-microphone startup; run explicitly"]
    fn measure_named_microphone_startup() {
        let name = cpal::default_host()
            .default_input_device()
            .unwrap()
            .name()
            .unwrap();
        assert!(input_devices().unwrap().contains(&name));
        let mut timings = Vec::new();
        for _ in 0..30 {
            let start = std::time::Instant::now();
            assert!(!start_device(Some(name.clone()), std::time::Duration::ZERO).unwrap());
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
            std::thread::sleep(std::time::Duration::from_millis(150));
            assert!(!stop().unwrap().is_empty());
        }
        println!("named_startup_device={name:?} first_ms={:.3}", timings[0]);
        timings.sort_by(f64::total_cmp);
        println!(
            "named_startup_n=30 median_ms={:.3} p95_ms={:.3} max_ms={:.3}",
            timings[15], timings[28], timings[29]
        );
    }

    #[test]
    #[ignore = "Opens the real microphone briefly; run explicitly"]
    fn disconnected_preference_records_and_monitors_with_default_microphone() {
        let missing = Some("dictate-nonexistent-microphone-test".into());
        assert!(start_device(missing.clone(), std::time::Duration::ZERO).unwrap());
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(!stop().unwrap().is_empty());
        monitor_start(missing).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(microphone_level().unwrap().is_finite());
        monitor_stop().unwrap();
    }

    #[test]
    #[ignore = "Measures local cached device selection without opening the microphone"]
    fn measure_cached_device_selection() {
        let names = input_devices().unwrap();
        let name = names.first().expect("A microphone is required");
        let mut timings = Vec::new();
        for _ in 0..30 {
            let start = std::time::Instant::now();
            assert!(selected_device(name).unwrap().is_some());
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        assert!(
            selected_device("dictate-nonexistent-microphone-test")
                .unwrap()
                .is_none()
        );
        timings.sort_by(f64::total_cmp);
        println!(
            "cached_selection_n=30 median_ms={:.3} p95_ms={:.3} max_ms={:.3}",
            timings[15], timings[28], timings[29]
        );
    }

    #[test]
    fn recording_and_monitor_report_the_same_rms_without_retaining_monitor_audio() {
        let mut recording = Captured {
            retain: true,
            ..Captured::default()
        };
        let mut monitor = Captured::default();
        for captured in [&mut recording, &mut monitor] {
            capture_block(captured, &[0.5_f32, 0.5, -0.5, -0.5], 2);
            assert_eq!(captured.level, 0.5);
        }
        assert_eq!(recording.samples, [0.5, -0.5]);
        assert!(monitor.samples.is_empty());
        capture_block(&mut recording, &[0.0_f32, 0.0], 2);
        assert_eq!(recording.level, 0.0);
    }

    #[test]
    fn debug_wav_retains_normalized_samples() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("take.wav");
        save_wav(&path, &[-0.5, 0.0, 0.5]).unwrap();
        let mut reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_format, hound::SampleFormat::Float);
        assert_eq!(
            reader
                .samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [-0.5, 0.0, 0.5]
        );
    }

    #[test]
    fn warm_stream_keeps_only_the_latest_preroll_and_hands_it_to_the_take() {
        let mut warm = Captured {
            preroll_frames: 3,
            ..Captured::default()
        };
        capture_block(&mut warm, &[0.1_f32, 0.2, 0.3, 0.4, 0.5], 1);
        assert!(warm.samples.is_empty());
        assert_eq!(warm.preroll, [0.3, 0.4, 0.5]);
        warm.samples = warm.preroll.drain(..).collect();
        warm.retain = true;
        capture_block(&mut warm, &[0.6_f32], 1);
        assert_eq!(warm.samples, [0.3, 0.4, 0.5, 0.6]);
        assert!(warm.preroll.is_empty());
    }

    #[test]
    #[ignore = "Opens the real microphone and keeps it warm; run explicitly"]
    fn warm_microphone_starts_takes_with_preroll_and_stays_open() {
        keep_warm(Some(None)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        let started = std::time::Instant::now();
        start_device(None, std::time::Duration::ZERO).unwrap();
        let start_ms = started.elapsed().as_secs_f64() * 1000.0;
        std::thread::sleep(std::time::Duration::from_millis(200));
        let first = stop().unwrap();
        // 200 ms of take plus up to 300 ms of pre-roll.
        assert!(first.len() > 16_000 * 4 / 10, "{} samples", first.len());
        std::thread::sleep(std::time::Duration::from_millis(400));
        start_device(None, std::time::Duration::ZERO).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        let second = stop().unwrap();
        assert!(second.len() > 16_000 * 4 / 10, "{} samples", second.len());
        keep_warm(None).unwrap();
        println!(
            "warm_start_ms={start_ms:.3} first_samples={} second_samples={}",
            first.len(),
            second.len()
        );
    }

    #[test]
    fn warm_stream_is_reused_only_while_it_records_the_requested_microphone() {
        let built_in = "MacBook Pro Microphone".to_string();
        let airpods = || Some("AirPods Pro".to_string());
        let unchanged = || Some("MacBook Pro Microphone".to_string());
        let unasked = || -> Option<String> { panic!("named match needs no lookup") };

        assert!(warm_stream_serves(&None, &built_in, &None, unchanged));
        assert!(
            !warm_stream_serves(&None, &built_in, &None, airpods),
            "system default moved to AirPods"
        );
        assert!(!warm_stream_serves(&None, &built_in, &None, || None));

        let named = Some(built_in.clone());
        assert!(warm_stream_serves(&named, &built_in, &named, unasked));
        assert!(!warm_stream_serves(&named, &built_in, &None, unchanged));
        assert!(!warm_stream_serves(&None, &built_in, &named, unchanged));

        let missing = Some("USB Mic".to_string());
        assert!(
            warm_stream_serves(&missing, &built_in, &missing, unchanged),
            "fallback to the default stays while the default holds"
        );
        assert!(!warm_stream_serves(&missing, &built_in, &missing, airpods));
    }

    #[test]
    fn a_take_drops_its_first_frames_when_asked_but_keeps_the_preroll() {
        let mut take = Captured {
            preroll_frames: 2,
            ..Captured::default()
        };
        capture_block(&mut take, &[0.1_f32, 0.2], 1);
        take.samples = take.preroll.drain(..).collect();
        take.retain = true;
        take.skip_frames = 2;
        capture_block(&mut take, &[0.9_f32, 0.9, 0.3, 0.4], 1);
        assert_eq!(take.samples, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(take.skip_frames, 0);
    }

    #[test]
    #[ignore = "Queries the real default microphone; run explicitly"]
    fn measure_default_device_lookup() {
        let mut timings = Vec::new();
        for _ in 0..30 {
            let start = std::time::Instant::now();
            assert!(default_device_name().is_some());
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        timings.sort_by(f64::total_cmp);
        println!(
            "default_lookup_n=30 median_ms={:.3} p95_ms={:.3} max_ms={:.3}",
            timings[15], timings[28], timings[29]
        );
    }

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
