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
    Frames(Reply<mpsc::Receiver<Vec<f32>>>),
    Status(mpsc::Sender<bool>),
    /// `Some(device)` keeps that microphone open between takes; `None` closes it.
    Warm(Option<Option<String>>, Reply<()>),
}

#[derive(Default)]
struct Captured {
    rate: usize,
    samples: Vec<f32>,
    error: Option<String>,
    level: f32,
    retain: bool,
    preroll: VecDeque<f32>,
    preroll_frames: usize,
    /// Frames a take records as silence, so the start sound is not in it but
    /// the timing after it is unchanged.
    mute_frames: usize,
    /// Set by `frames`: where the take's audio goes, 16 kHz, while it records.
    tap: Option<Tap>,
}

/// Sends a take's audio on as 16 kHz frames of about `FRAME_MS` while it
/// records, so speech can be decoded before the take ends.
struct Tap {
    sender: mpsc::Sender<Vec<f32>>,
    converter: To16k,
    /// How many of the take's samples, at the device rate, have been sent.
    sent: usize,
}

/// Length of the frames `frames` delivers.
pub const FRAME_MS: usize = 100;

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
                            Command::Start(device, mute, reply) => {
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
                                            active.mute(mute);
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
                            Command::Frames(reply) => {
                                let result = recording
                                    .as_ref()
                                    .ok_or_else(|| "Recording is stopped".to_string())
                                    .and_then(Recording::tap);
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

/// Starts a take on `device`, recording its first `mute` of new audio as silence.
pub fn start_device(device: Option<String>, mute: std::time::Duration) -> Result<bool, Problem> {
    let (reply, result) = mpsc::channel();
    send(Command::Start(device, mute, reply))
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

/// The running take's audio as 16 kHz mono frames of about `FRAME_MS`,
/// starting from its first sample (pre-roll included). The receiver ends when
/// the take stops, after the last partial frame. Together the frames hold the
/// same audio `stop` returns.
pub fn frames() -> Result<mpsc::Receiver<Vec<f32>>, String> {
    let (reply, result) = mpsc::channel();
    send(Command::Frames(reply))?;
    result
        .recv()
        .map_err(|e| format!("Audio frames reply: {e}"))?
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
/// default changes, for example when AirPods connect. A fallback is also stale
/// once the named microphone is connected again.
fn warm_stream_serves(
    preference: &Option<String>,
    opened: &str,
    requested: &Option<String>,
    current_default: impl FnOnce() -> Option<String>,
    connected: impl FnOnce(&str) -> bool,
) -> bool {
    if preference != requested {
        return false;
    }
    match requested {
        Some(name) if name == opened => true,
        Some(name) if connected(name) => false,
        _ => current_default().as_deref() == Some(opened),
    }
}

/// Lists devices afresh: the cache would miss a microphone plugged in since.
fn microphone_connected(name: &str) -> bool {
    list_devices().is_ok_and(|devices| devices.iter().any(|(label, _)| label == name))
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
        rate,
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
        if captured.retain && captured.mute_frames > 0 {
            captured.mute_frames -= 1;
            captured.samples.push(0.0);
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
    let frame = captured.rate * FRAME_MS / 1000;
    if captured
        .tap
        .as_ref()
        .is_some_and(|tap| captured.samples.len() - tap.sent >= frame)
    {
        send_new(captured);
    }
}

/// Converts and sends the samples recorded since the last frame. A tap that
/// fails is dropped, which ends its receiver early.
fn send_new(captured: &mut Captured) {
    let Some(tap) = captured.tap.as_mut() else {
        return;
    };
    let new = &captured.samples[tap.sent..];
    tap.sent = captured.samples.len();
    let sent = tap
        .converter
        .push(new)
        .and_then(|frame| deliver(&tap.sender, frame));
    if let Err(error) = sent {
        log::warn!("[audio] frames stopped: {error}");
        captured.tap = None;
    }
}

/// Sends the rest of the take and closes the frames receiver.
fn close_tap(captured: &mut Captured) {
    send_new(captured);
    if let Some(tap) = captured.tap.take()
        && let Err(error) = tap
            .converter
            .finish()
            .and_then(|rest| deliver(&tap.sender, rest))
    {
        log::warn!("[audio] frames stopped: {error}");
    }
}

fn deliver(sender: &mpsc::Sender<Vec<f32>>, frame: Vec<f32>) -> Result<(), String> {
    if frame.is_empty() {
        return Ok(());
    }
    sender
        .send(frame)
        .map_err(|_| "frames receiver dropped".to_string())
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
            microphone_connected,
        )
    }

    fn mute(&self, duration: std::time::Duration) {
        if let Ok(mut captured) = self.captured.lock() {
            captured.mute_frames = (self.rate as u128 * duration.as_millis() / 1000) as usize;
        }
    }

    fn failed(&self) -> bool {
        self.captured
            .lock()
            .map_or(true, |captured| captured.error.is_some())
    }

    /// Starts sending this take's audio, from its first sample, as frames.
    fn tap(&self) -> Result<mpsc::Receiver<Vec<f32>>, String> {
        let (sender, receiver) = mpsc::channel();
        let mut captured = self.captured.lock().map_err(|e| e.to_string())?;
        captured.tap = Some(Tap {
            sender,
            converter: To16k::new(self.rate)?,
            sent: 0,
        });
        send_new(&mut captured);
        Ok(receiver)
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
            close_tap(&mut captured);
            captured.retain = false;
            captured.mute_frames = 0;
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
        close_tap(&mut captured);
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
    let mut converter = To16k::new(rate)?;
    let mut output = converter.push(&samples)?;
    output.extend(converter.finish()?);
    Ok(output)
}

/// Converts mono audio at `rate` to 16 kHz a block at a time. Pushing a take in
/// any number of pieces, then `finish`, gives the same samples as one push:
/// the resampler's start-up delay is dropped and the end is flushed and cut to
/// the take's length.
struct To16k {
    /// `None` when the device already records at 16 kHz.
    resampler: Option<FftFixedInOut<f32>>,
    rate: usize,
    pending: Vec<f32>,
    /// Output samples still to drop for the resampler's delay.
    delay: usize,
    received: usize,
    emitted: usize,
}

impl To16k {
    fn new(rate: usize) -> Result<Self, String> {
        let resampler = (rate != OUTPUT_RATE)
            .then(|| {
                FftFixedInOut::<f32>::new(rate, OUTPUT_RATE, 1024, 1)
                    .map_err(|e| format!("Create resampler: {e}"))
            })
            .transpose()?;
        Ok(Self {
            delay: resampler.as_ref().map_or(0, |r| r.output_delay()),
            resampler,
            rate,
            pending: Vec::new(),
            received: 0,
            emitted: 0,
        })
    }

    fn push(&mut self, input: &[f32]) -> Result<Vec<f32>, String> {
        self.received += input.len();
        let Some(resampler) = self.resampler.as_ref() else {
            self.emitted += input.len();
            return Ok(input.to_vec());
        };
        let chunk = resampler.input_frames_next();
        self.pending.extend_from_slice(input);
        let mut output = Vec::new();
        let pending = std::mem::take(&mut self.pending);
        let mut blocks = pending.chunks_exact(chunk);
        for block in blocks.by_ref() {
            self.convert(block.to_vec(), &mut output)?;
        }
        self.pending.extend_from_slice(blocks.remainder());
        self.emitted += output.len();
        Ok(output)
    }

    fn finish(mut self) -> Result<Vec<f32>, String> {
        let Some(resampler) = self.resampler.as_ref() else {
            return Ok(Vec::new());
        };
        let chunk = resampler.input_frames_next();
        let length = (self.received * OUTPUT_RATE / self.rate).saturating_sub(self.emitted);
        let mut output = Vec::new();
        let mut block = std::mem::take(&mut self.pending);
        while output.len() < length {
            block.resize(chunk, 0.0);
            self.convert(std::mem::take(&mut block), &mut output)?;
        }
        output.truncate(length);
        Ok(output)
    }

    fn convert(&mut self, block: Vec<f32>, output: &mut Vec<f32>) -> Result<(), String> {
        let resampler = self
            .resampler
            .as_mut()
            .expect("only called when resampling");
        let converted = resampler
            .process(&[block], None)
            .map_err(|e| format!("Resample audio: {e}"))?;
        let skip = self.delay.min(converted[0].len());
        self.delay -= skip;
        output.extend_from_slice(&converted[0][skip..]);
        Ok(())
    }
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
        let unlisted = |_: &str| -> bool { panic!("only a fallback lists devices") };
        let usb_missing = |name: &str| name != "USB Mic";
        let usb_back = |_: &str| true;

        assert!(warm_stream_serves(
            &None, &built_in, &None, unchanged, unlisted
        ));
        assert!(
            !warm_stream_serves(&None, &built_in, &None, airpods, unlisted),
            "system default moved to AirPods"
        );
        assert!(!warm_stream_serves(
            &None,
            &built_in,
            &None,
            || None,
            unlisted
        ));

        let named = Some(built_in.clone());
        assert!(warm_stream_serves(
            &named, &built_in, &named, unasked, unlisted
        ));
        assert!(!warm_stream_serves(
            &named, &built_in, &None, unchanged, unlisted
        ));
        assert!(!warm_stream_serves(
            &None, &built_in, &named, unchanged, unlisted
        ));

        let missing = Some("USB Mic".to_string());
        assert!(
            warm_stream_serves(&missing, &built_in, &missing, unchanged, usb_missing),
            "fallback to the default stays while the chosen mic is missing and the default holds"
        );
        assert!(!warm_stream_serves(
            &missing,
            &built_in,
            &missing,
            airpods,
            usb_missing
        ));
        assert!(
            !warm_stream_serves(&missing, &built_in, &missing, unasked, usb_back),
            "fallback reopens on the chosen mic once it is connected again"
        );
    }

    #[test]
    fn a_take_mutes_its_first_frames_when_asked_but_keeps_the_preroll_and_timing() {
        let mut take = Captured {
            preroll_frames: 2,
            ..Captured::default()
        };
        capture_block(&mut take, &[0.1_f32, 0.2], 1);
        take.samples = take.preroll.drain(..).collect();
        take.retain = true;
        take.mute_frames = 2;
        capture_block(&mut take, &[0.9_f32, 0.9, 0.3, 0.4], 1);
        assert_eq!(take.samples, [0.1, 0.2, 0.0, 0.0, 0.3, 0.4]);
        assert_eq!(take.mute_frames, 0);
    }

    fn sine(rate: usize, seconds: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5)
            .collect()
    }

    #[test]
    fn converting_in_pieces_matches_converting_the_whole_take() {
        for rate in [16_000, 44_100, 48_000] {
            let input = sine(rate, 1.37);
            let whole = resample(input.clone(), rate).unwrap();
            let mut converter = To16k::new(rate).unwrap();
            let mut pieces = Vec::new();
            for block in input.chunks(rate / 10 + 7) {
                pieces.extend(converter.push(block).unwrap());
            }
            pieces.extend(converter.finish().unwrap());
            assert_eq!(
                pieces.len(),
                input.len() * OUTPUT_RATE / rate,
                "rate {rate}"
            );
            assert_eq!(pieces, whole, "rate {rate}");
        }
    }

    #[test]
    fn tapped_take_sends_100_ms_frames_that_add_up_to_the_take() {
        let rate = 48_000;
        let mut take = Captured {
            rate,
            retain: true,
            samples: vec![0.25; 4_800 * 3],
            ..Captured::default()
        };
        let (sender, frames) = mpsc::channel();
        take.tap = Some(Tap {
            sender,
            converter: To16k::new(rate).unwrap(),
            sent: 0,
        });
        send_new(&mut take);
        for block in sine(rate, 2.0).chunks(480) {
            capture_block(&mut take, block, 1);
        }
        let streamed: Vec<Vec<f32>> = frames.try_iter().collect();
        assert!(streamed.len() >= 20, "got {} frames", streamed.len());
        assert!(streamed[0].len() > 4_000, "the 300 ms pre-roll goes first");
        assert!(
            streamed[1..]
                .iter()
                .all(|f| (1_200..=1_800).contains(&f.len())),
            "then about 100 ms each: {:?}",
            streamed.iter().map(Vec::len).collect::<Vec<_>>()
        );
        close_tap(&mut take);
        assert!(take.tap.is_none());
        let mut all: Vec<f32> = streamed.into_iter().flatten().collect();
        all.extend(frames.iter().flatten());
        assert_eq!(all, resample(take.samples, rate).unwrap());
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

#[cfg(test)]
mod stop_latency_tests {
    #[test]
    fn thirteen_second_take_at_96k_preserves_duration() {
        let samples: Vec<f32> = (0..1_337_856)
            .map(|i| (i as f32 * 0.03).sin() * 0.1)
            .collect();
        let started = std::time::Instant::now();
        let converted = super::resample(samples, 96_000).unwrap();
        eprintln!("96k stop resample: {} ms", started.elapsed().as_millis());
        assert_eq!(converted.len(), 222_976);
    }
}
