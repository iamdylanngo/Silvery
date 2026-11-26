//! Playing back a meeting's recording through the speakers.
//!
//! All recordings of a meeting are joined into one timeline. That matches the transcript's times,
//! because each new recording continues where the previous one ended.

use crate::audio::RATE;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

struct State {
    /// Position in source samples (16 kHz).
    pos: f64,
    playing: bool,
    volume: f32,
}

pub struct Player {
    pub meeting_id: String,
    samples: Arc<Vec<i16>>,
    state: Arc<Mutex<State>>,
    stop: Option<mpsc::Sender<()>>,
    /// Set when the speakers couldn't be opened.
    pub error: Option<String>,
}

/// Read one of Silvery's WAV files (16-bit mono 16 kHz) and append its samples.
fn read_wav(path: &Path, out: &mut Vec<i16>) -> std::io::Result<()> {
    let bytes = std::fs::read(path)?;
    let start = bytes
        .windows(4)
        .position(|w| w == b"data")
        .map(|p| p + 8)
        .unwrap_or(44)
        .min(bytes.len());
    out.extend(bytes[start..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])));
    Ok(())
}

impl Player {
    /// Load every recording of a meeting, in order. Returns None if there is no audio.
    pub fn load(meeting_id: &str, dir: &Path, files: &[String]) -> Option<Player> {
        let mut samples = Vec::new();
        for f in files {
            let _ = read_wav(&dir.join(f), &mut samples);
        }
        if samples.is_empty() {
            return None;
        }
        Some(Player::from_samples(meeting_id, samples))
    }

    /// A player over the given samples. The speakers are only opened on the first Play.
    pub fn from_samples(meeting_id: &str, samples: Vec<i16>) -> Player {
        Player {
            meeting_id: meeting_id.to_string(),
            samples: Arc::new(samples),
            state: Arc::new(Mutex::new(State { pos: 0.0, playing: false, volume: 1.0 })),
            stop: None,
            error: None,
        }
    }

    pub fn duration_ms(&self) -> i64 {
        self.samples.len() as i64 * 1000 / RATE as i64
    }
    pub fn position_ms(&self) -> i64 {
        self.state.lock().map(|s| (s.pos * 1000.0 / RATE as f64) as i64).unwrap_or(0)
    }
    pub fn is_playing(&self) -> bool {
        self.state.lock().map(|s| s.playing).unwrap_or(false)
    }
    pub fn seek_ms(&self, ms: i64) {
        if let Ok(mut s) = self.state.lock() {
            let max = self.samples.len().saturating_sub(1) as f64;
            s.pos = (ms.max(0) as f64 * RATE as f64 / 1000.0).min(max);
        }
    }
    pub fn set_volume(&self, v: f32) {
        if let Ok(mut s) = self.state.lock() {
            s.volume = v;
        }
    }

    pub fn play(&mut self, ctx: &egui::Context) {
        if self.stop.is_none() {
            if let Err(e) = self.open_output(ctx.clone()) {
                self.error = Some(e);
                return;
            }
        }
        self.error = None;
        if let Ok(mut s) = self.state.lock() {
            // Start again from the beginning if it had reached the end.
            if s.pos >= self.samples.len().saturating_sub(1) as f64 {
                s.pos = 0.0;
            }
            s.playing = true;
        }
    }

    pub fn pause(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.playing = false;
        }
    }

    /// Open the speakers on a thread that owns the output stream (streams can't move between threads).
    fn open_output(&mut self, ctx: egui::Context) -> Result<(), String> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let samples = self.samples.clone();
        let state = self.state.clone();
        std::thread::spawn(move || {
            let device = match cpal::default_host().default_output_device() {
                Some(d) => d,
                None => {
                    let _ = ready_tx.send(Err("No speakers or headphones found.".into()));
                    return;
                }
            };
            let cfg = match device.default_output_config() {
                Ok(c) => c,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("The speakers can't be opened: {e}")));
                    return;
                }
            };
            let channels = cfg.channels() as usize;
            let step = RATE as f64 / cfg.sample_rate() as f64;
            let fmt = cfg.sample_format();
            let stream_cfg: cpal::StreamConfig = cfg.config();

            // Fills `frames` output frames with the next sound, as values from -1 to 1.
            let next = move |frames: usize, write: &mut dyn FnMut(usize, f32)| {
                let mut s = match state.lock() {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let n = samples.len();
                for f in 0..frames {
                    let v = if s.playing && n > 1 && s.pos < (n - 1) as f64 {
                        let i = s.pos as usize;
                        let frac = (s.pos - i as f64) as f32;
                        let a = samples[i] as f32 / 32768.0;
                        let b = samples[i + 1] as f32 / 32768.0;
                        s.pos += step;
                        (a + (b - a) * frac) * s.volume
                    } else {
                        if s.playing {
                            s.playing = false; // reached the end
                            ctx.request_repaint();
                        }
                        0.0
                    };
                    write(f, v);
                }
            };
            let err_fn = |e: cpal::Error| eprintln!("playback: {e}");
            let stream = match fmt {
                cpal::SampleFormat::F32 => device.build_output_stream(
                    stream_cfg,
                    move |d: &mut [f32], _: &_| {
                        next(d.len() / channels, &mut |f, v| {
                            for c in 0..channels {
                                d[f * channels + c] = v;
                            }
                        })
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_output_stream(
                    stream_cfg,
                    move |d: &mut [i16], _: &_| {
                        next(d.len() / channels, &mut |f, v| {
                            for c in 0..channels {
                                d[f * channels + c] = (v.clamp(-1.0, 1.0) * 32767.0) as i16;
                            }
                        })
                    },
                    err_fn,
                    None,
                ),
                other => {
                    let _ = ready_tx.send(Err(format!("Unsupported speaker format {other:?}")));
                    return;
                }
            };
            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("The speakers can't be opened: {e}")));
                    return;
                }
            };
            if let Err(e) = stream.play() {
                let _ = ready_tx.send(Err(format!("Playback can't start: {e}")));
                return;
            }
            let _ = ready_tx.send(Ok(()));
            let _ = stop_rx.recv();
            drop(stream);
        });
        match ready_rx.recv() {
            Ok(Ok(())) => {
                self.stop = Some(stop_tx);
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Playback stopped unexpectedly.".into()),
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
    }
}
