//! Microphone recording, converted to 16 kHz mono for speech recognition.

use cpal::traits::{DeviceTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

pub const RATE: usize = 16_000;

pub struct Recorder {
    pub samples: Arc<Mutex<Vec<f32>>>,
    #[allow(dead_code)]
    pub level: Arc<AtomicU32>,
    /// Set only when recording can't go on (the microphone was disconnected or access was removed).
    pub failed: Arc<Mutex<Option<String>>>,
    /// Name of the microphone being used.
    pub device: String,
    /// Set once any non-silent sample has arrived (all zeros usually means macOS blocks the mic).
    pub any_signal: Arc<AtomicBool>,
    /// Brief hiccups (buffer under/overruns, route changes, reconnects) that recording survived.
    pub skips: Arc<AtomicU32>,
    /// When recording started.
    pub started: std::time::Instant,
    stop: Option<mpsc::Sender<Ctl>>,
    paused: Arc<AtomicBool>,
}

/// Messages to the microphone thread.
enum Ctl {
    Stop,
    /// The stream stopped working but the microphone is still there: open it again.
    Rebuild,
}

/// What a problem reported by the microphone means for the recording.
#[derive(Debug, PartialEq)]
pub enum Severity {
    /// A brief glitch (for example a buffer under/overrun): keep recording.
    Skip,
    /// The stream must be reopened; the microphone itself is fine.
    Reopen,
    /// Recording can't continue (the microphone is gone or access was removed).
    Stop(&'static str),
}

pub fn classify(kind: cpal::ErrorKind) -> Severity {
    use cpal::ErrorKind as K;
    match kind {
        K::DeviceNotAvailable => Severity::Stop(
            "The microphone was disconnected, so recording stopped. Everything recorded until then is kept. Press Record to continue with another microphone.",
        ),
        K::HostUnavailable => Severity::Stop(
            "The computer's sound system stopped, so recording stopped. Everything recorded until then is kept.",
        ),
        K::PermissionDenied => Severity::Stop(
            "Microphone access was turned off, so recording stopped. Everything recorded until then is kept.",
        ),
        K::StreamInvalidated => Severity::Reopen,
        // Buffer under/overruns, audio route changes, busy moments, performance notices and
        // anything unclassified: a fraction of a second may be lost, but recording goes on.
        _ => Severity::Skip,
    }
}

/// State shared between the recorder, its microphone thread and the audio callbacks.
#[derive(Clone)]
struct Shared {
    samples: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
    failed: Arc<Mutex<Option<String>>>,
    any_signal: Arc<AtomicBool>,
    skips: Arc<AtomicU32>,
    paused: Arc<AtomicBool>,
    ctx: egui::Context,
}

/// Open the microphone and start the stream.
fn open_stream(device: &cpal::Device, sh: &Shared, ctl: mpsc::Sender<Ctl>) -> Result<cpal::Stream, String> {
    let cfg = device
        .default_input_config()
        .map_err(|e| format!("The microphone can't be opened: {e}"))?;
    let channels = cfg.channels() as usize;
    let in_rate = cfg.sample_rate() as f64;
    let fmt = cfg.sample_format();
    let stream_cfg: cpal::StreamConfig = cfg.config();
    let mut rs = Resampler::new(in_rate, RATE as f64);

    let s = sh.clone();
    let mut push = move |mono: &[f32]| {
        if s.paused.load(Ordering::Relaxed) {
            s.level.store(0f32.to_bits(), Ordering::Relaxed);
            return;
        }
        let peak = mono.iter().fold(0f32, |a, &b| a.max(b.abs()));
        if peak > 1e-5 {
            s.any_signal.store(true, Ordering::Relaxed);
        }
        let prev = f32::from_bits(s.level.load(Ordering::Relaxed));
        s.level.store((peak.max(prev * 0.85)).to_bits(), Ordering::Relaxed);
        let out = rs.process(mono);
        if let Ok(mut v) = s.samples.lock() {
            v.extend_from_slice(&out);
        }
        s.ctx.request_repaint();
    };
    let e = sh.clone();
    let err_fn = move |err: cpal::Error| match classify(err.kind()) {
        Severity::Skip => {
            e.skips.fetch_add(1, Ordering::Relaxed);
            eprintln!("microphone glitch (recording continues): {err}");
        }
        Severity::Reopen => {
            eprintln!("microphone stream needs reopening: {err}");
            let _ = ctl.send(Ctl::Rebuild);
        }
        Severity::Stop(msg) => {
            if let Ok(mut f) = e.failed.lock() {
                *f = Some(msg.to_string());
            }
            e.ctx.request_repaint();
        }
    };
    fn to_mono<T: Copy>(data: &[T], ch: usize, conv: impl Fn(T) -> f32) -> Vec<f32> {
        data.chunks(ch.max(1))
            .map(|c| c.iter().map(|&x| conv(x)).sum::<f32>() / c.len() as f32)
            .collect()
    }
    let stream = match fmt {
        cpal::SampleFormat::F32 => device.build_input_stream(
            stream_cfg,
            move |d: &[f32], _: &_| push(&to_mono(d, channels, |x| x)),
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            stream_cfg,
            move |d: &[i16], _: &_| push(&to_mono(d, channels, |x| x as f32 / 32768.0)),
            err_fn,
            None,
        ),
        cpal::SampleFormat::I32 => device.build_input_stream(
            stream_cfg,
            move |d: &[i32], _: &_| push(&to_mono(d, channels, |x| x as f32 / 2147483648.0)),
            err_fn,
            None,
        ),
        other => return Err(format!("Unsupported microphone format {other:?}")),
    }
    .map_err(|e| format!("The microphone can't be opened: {e}. Check {}.", crate::platform::MIC_SETTINGS))?;
    stream.play().map_err(|e| format!("The microphone can't start: {e}"))?;
    Ok(stream)
}

impl Recorder {
    pub fn start(ctx: egui::Context, mic: &str) -> Result<Recorder, String> {
        let sh = Shared {
            samples: Arc::new(Mutex::new(Vec::<f32>::with_capacity(RATE * 60))),
            level: Arc::new(AtomicU32::new(0)),
            failed: Arc::new(Mutex::new(None)),
            any_signal: Arc::new(AtomicBool::new(false)),
            skips: Arc::new(AtomicU32::new(0)),
            paused: Arc::new(AtomicBool::new(false)),
            ctx,
        };
        let mic = mic.to_string();
        let (ctl_tx, ctl_rx) = mpsc::channel::<Ctl>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<String, String>>();

        let th = sh.clone();
        let ctl_for_thread = ctl_tx.clone();
        std::thread::spawn(move || {
            let (device, device_name) = match crate::mic::find(&mic) {
                Some(d) => d,
                None => {
                    let _ = ready_tx.send(Err("No microphone found.".into()));
                    return;
                }
            };
            let mut stream = match open_stream(&device, &th, ctl_for_thread.clone()) {
                Ok(s) => Some(s),
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(device_name));

            // Run until stopped. If the stream needs reopening, reopen it (a few tries).
            while let Ok(cmd) = ctl_rx.recv() {
                match cmd {
                    Ctl::Stop => break,
                    Ctl::Rebuild => {
                        drop(stream.take());
                        for attempt in 0..5u64 {
                            std::thread::sleep(std::time::Duration::from_millis(200 + 400 * attempt));
                            if let Some((dev, _)) = crate::mic::find(&mic) {
                                if let Ok(s) = open_stream(&dev, &th, ctl_for_thread.clone()) {
                                    stream = Some(s);
                                    break;
                                }
                            }
                        }
                        if stream.is_some() {
                            th.skips.fetch_add(1, Ordering::Relaxed);
                        } else if let Ok(mut f) = th.failed.lock() {
                            *f = Some("The microphone stopped and couldn't be reopened, so recording stopped. Everything recorded until then is kept.".into());
                            th.ctx.request_repaint();
                        }
                    }
                }
            }
            drop(stream);
        });

        match ready_rx.recv() {
            Ok(Ok(device)) => Ok(Recorder {
                samples: sh.samples,
                level: sh.level,
                failed: sh.failed,
                device,
                any_signal: sh.any_signal,
                skips: sh.skips,
                started: std::time::Instant::now(),
                stop: Some(ctl_tx),
                paused: sh.paused,
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("The microphone thread stopped unexpectedly.".into()),
        }
    }

    /// How many brief hiccups the recording survived.
    pub fn skips(&self) -> u32 {
        self.skips.load(Ordering::Relaxed)
    }

    /// A recorder filled with made-up speech-like sound, for test pictures only.
    pub fn demo() -> Recorder {
        let n = RATE * 8;
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let syll = ((t * 4.3).sin() * (t * 1.7 + 0.5).sin()).abs();
                let gap = if (t * 0.6).fract() > 0.82 { 0.02 } else { 1.0 };
                0.25 * syll * gap * (t * 220.0 * std::f32::consts::TAU).sin()
            })
            .collect();
        Recorder {
            samples: Arc::new(Mutex::new(samples)),
            level: Arc::new(AtomicU32::new(0)),
            failed: Arc::new(Mutex::new(None)),
            device: "Demo".into(),
            any_signal: Arc::new(AtomicBool::new(true)),
            skips: Arc::new(AtomicU32::new(0)),
            started: std::time::Instant::now() - std::time::Duration::from_secs(20),
            stop: None,
            paused: Arc::new(AtomicBool::new(false)),
        }
    }

    #[allow(dead_code)]
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
    /// Loudest sample in the last `secs` seconds.
    pub fn recent_peak(&self, secs: f32) -> f32 {
        self.samples
            .lock()
            .map(|v| {
                let n = (secs * RATE as f32) as usize;
                v[v.len().saturating_sub(n)..].iter().fold(0f32, |a, &b| a.max(b.abs()))
            })
            .unwrap_or(0.0)
    }
    /// Loudness (RMS) of the last `secs` seconds split into `bins` slices, oldest first.
    pub fn levels(&self, secs: f32, bins: usize) -> Vec<f32> {
        let per = ((secs * RATE as f32) as usize / bins.max(1)).max(1);
        self.samples
            .lock()
            .map(|v| {
                let end = v.len();
                (0..bins)
                    .map(|i| {
                        let back = (bins - i) * per;
                        if back > end {
                            return 0.0;
                        }
                        let s = &v[end - back..end - back + per];
                        (s.iter().map(|x| x * x).sum::<f32>() / per as f32).sqrt()
                    })
                    .collect()
            })
            .unwrap_or_else(|_| vec![0.0; bins])
    }
    pub fn heard_anything(&self) -> bool {
        self.any_signal.load(Ordering::Relaxed)
    }
    pub fn len(&self) -> usize {
        self.samples.lock().map(|v| v.len()).unwrap_or(0)
    }
    pub fn set_paused(&self, p: bool) {
        self.paused.store(p, Ordering::Relaxed);
    }
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
    pub fn stop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(Ctl::Stop);
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Simple streaming linear-interpolation resampler.
struct Resampler {
    step: f64,
    pos: f64,
    last: f32,
}

impl Resampler {
    fn new(from: f64, to: f64) -> Self {
        Self { step: from / to, pos: 0.0, last: 0.0 }
    }
    fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if input.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity((input.len() as f64 / self.step) as usize + 2);
        // pos is measured relative to input[0]; -1 means "self.last".
        while self.pos < input.len() as f64 - 1.0 + 1e-9 || (self.pos < 0.0) {
            let i = self.pos.floor();
            let frac = (self.pos - i) as f32;
            let a = if i < 0.0 { self.last } else { input[i as usize] };
            let bi = i + 1.0;
            let b = if bi < 0.0 { self.last } else if (bi as usize) < input.len() { input[bi as usize] } else { a };
            out.push(a + (b - a) * frac);
            self.pos += self.step;
        }
        if let Some(&l) = input.last() {
            self.last = l;
        }
        self.pos -= input.len() as f64;
        out
    }
}

/// Find a quiet place near the end of `buf[from..to]` to cut a chunk, so words aren't split.
pub fn quiet_cut(buf: &[f32], from: usize, to: usize) -> usize {
    let win = RATE / 10; // 100 ms
    let search_start = to.saturating_sub(RATE * 3).max(from + RATE);
    let mut best = to;
    let mut best_e = f32::MAX;
    let mut i = search_start;
    while i + win <= to.min(buf.len()) {
        let e: f32 = buf[i..i + win].iter().map(|x| x * x).sum();
        if e < best_e {
            best_e = e;
            best = i + win / 2;
        }
        i += win / 2;
    }
    best
}

/// Write 16 kHz mono float samples to a 16-bit WAV file.
pub fn write_wav(path: &std::path::Path, samples: &[f32]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let data_len = (samples.len() * 2) as u32;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&(RATE as u32).to_le_bytes())?;
    f.write_all(&((RATE * 2) as u32).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        f.write_all(&v.to_le_bytes())?;
    }
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glitches_keep_recording_and_disconnects_stop() {
        use cpal::ErrorKind as K;
        // the error from the person's recording
        assert_eq!(classify(cpal::Error::new(K::Xrun).kind()), Severity::Skip);
        assert_eq!(classify(K::DeviceChanged), Severity::Skip);
        assert_eq!(classify(K::DeviceBusy), Severity::Skip);
        assert_eq!(classify(K::RealtimeDenied), Severity::Skip);
        assert_eq!(classify(K::BackendError), Severity::Skip);
        assert_eq!(classify(K::StreamInvalidated), Severity::Reopen);
        assert!(matches!(classify(K::DeviceNotAvailable), Severity::Stop(_)));
        assert!(matches!(classify(K::PermissionDenied), Severity::Stop(_)));
    }
}
