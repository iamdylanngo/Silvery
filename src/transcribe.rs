//! On-device speech recognition with whisper.cpp, plus the one-time model download.

use crate::model::Segment;
use std::path::PathBuf;
use std::sync::mpsc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperModel {
    pub id: &'static str,
    pub label: &'static str,
    pub size_mb: u32,
}

pub const MODELS: &[WhisperModel] = &[
    WhisperModel { id: "base.en", label: "English — fast (recommended)", size_mb: 148 },
    WhisperModel { id: "small.en", label: "English — more accurate", size_mb: 488 },
    // Whisper large-v3-turbo, compressed: far better for Vietnamese and other languages,
    // and for meetings that switch between languages.
    WhisperModel { id: "large-v3-turbo-q5_0", label: "Any language — best (Vietnamese, mixed)", size_mb: 574 },
    WhisperModel { id: "base", label: "Any language — fast", size_mb: 148 },
    WhisperModel { id: "small", label: "Any language — more accurate", size_mb: 488 },
];

/// The recommended model for languages other than English.
pub const MULTILINGUAL_BEST: &str = "large-v3-turbo-q5_0";

/// English-only models can't understand other languages.
pub fn is_english_only(id: &str) -> bool {
    id.ends_with(".en")
}

pub fn model_label(id: &str) -> String {
    MODELS
        .iter()
        .find(|m| m.id == id)
        .map(|m| m.label.to_string())
        .unwrap_or_else(|| id.to_string())
}

pub struct Job {
    pub meeting_id: String,
    pub offset_ms: i64,
    pub samples: Vec<f32>,
    pub model_path: PathBuf,
    pub language: String,
}

pub enum Out {
    /// `lang` is the spoken language the speech engine detected (a code like "vi"), if any.
    Segments { meeting_id: String, segments: Vec<Segment>, lang: Option<String> },
    Busy(bool),
    Error(String),
}

pub enum Cmd {
    Job(Job),
    ModelReady,
}

/// Start the background worker. Jobs are handled in order; jobs that arrive before the
/// model is downloaded wait until `Cmd::ModelReady`.
pub fn spawn_worker(out: mpsc::Sender<crate::app::Msg>, ctx: egui::Context) -> mpsc::Sender<Cmd> {
    let (tx, rx) = mpsc::channel::<Cmd>();
    std::thread::spawn(move || {
        whisper_rs::install_logging_hooks();
        let mut loaded: Option<(PathBuf, WhisperContext)> = None;
        let mut waiting: Vec<Job> = Vec::new();
        let send = |o: Out| {
            let _ = out.send(crate::app::Msg::Transcribe(o));
            ctx.request_repaint();
        };
        while let Ok(cmd) = rx.recv() {
            let mut jobs = match cmd {
                Cmd::Job(j) => vec![j],
                Cmd::ModelReady => std::mem::take(&mut waiting),
            };
            for job in jobs.drain(..) {
                if !job.model_path.exists() {
                    waiting.push(job);
                    continue;
                }
                if loaded.as_ref().map(|(p, _)| p != &job.model_path).unwrap_or(true) {
                    let params = WhisperContextParameters::default();
                    match WhisperContext::new_with_params(&job.model_path, params) {
                        Ok(c) => loaded = Some((job.model_path.clone(), c)),
                        Err(e) => {
                            send(Out::Error(format!("The speech model couldn't be loaded ({e}). Try downloading it again in Settings.")));
                            continue;
                        }
                    }
                }
                let ctxw = &loaded.as_ref().unwrap().1;
                send(Out::Busy(true));
                match run(ctxw, &job) {
                    Ok((segments, lang)) => send(Out::Segments { meeting_id: job.meeting_id.clone(), segments, lang }),
                    Err(e) => send(Out::Error(e)),
                }
                send(Out::Busy(false));
            }
        }
    });
    tx
}

fn run(ctx: &WhisperContext, job: &Job) -> Result<(Vec<Segment>, Option<String>), String> {
    // Skip silent chunks: whisper tends to invent text for silence.
    let rms = (job.samples.iter().map(|x| x * x).sum::<f32>() / job.samples.len().max(1) as f32).sqrt();
    if job.samples.len() < crate::audio::RATE / 2 || rms < 0.002 {
        return Ok((Vec::new(), None));
    }
    let mut state = ctx.create_state().map_err(|e| e.to_string())?;
    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8) as i32;
    p.set_n_threads(threads);
    p.set_print_progress(false);
    p.set_print_realtime(false);
    p.set_print_special(false);
    p.set_print_timestamps(false);
    p.set_suppress_blank(true);
    p.set_no_context(true);
    let lang = if job.model_path.to_string_lossy().contains(".en.") {
        "en".to_string()
    } else {
        job.language.clone()
    };
    p.set_language(Some(lang.as_str()));
    state.full(p, &job.samples).map_err(|e| format!("Transcription failed: {e}"))?;
    let mut segs = Vec::new();
    for i in 0..state.full_n_segments() {
        if let Some(s) = state.get_segment(i) {
            let text = s.to_str_lossy().map(|c| c.to_string()).unwrap_or_default();
            let t = text.trim();
            if t.is_empty() || is_noise(t) || s.no_speech_probability() > 0.8 {
                continue;
            }
            segs.push(Segment {
                start_ms: job.offset_ms + s.start_timestamp() * 10,
                end_ms: job.offset_ms + s.end_timestamp() * 10,
                text: t.to_string(),
            });
        }
    }
    // The language that was spoken (chosen in Settings, or detected by the speech engine).
    let spoken = if segs.is_empty() {
        None
    } else if lang != "auto" {
        Some(lang.clone())
    } else {
        whisper_rs::get_lang_str(state.full_lang_id_from_state()).map(|s| s.to_string())
    };
    Ok((segs, spoken))
}

/// "vi" → "Vietnamese", using the speech engine's own list of language names.
pub fn language_name(code: &str) -> String {
    whisper_rs::get_lang_id(code)
        .and_then(whisper_rs::get_lang_str_full)
        .map(|n| {
            let mut c = n.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .unwrap_or_else(|| code.to_string())
}

fn is_noise(t: &str) -> bool {
    let l = t.to_lowercase();
    (l.starts_with('[') && l.ends_with(']'))
        || (l.starts_with('(') && l.ends_with(')'))
        || l == "you"
        || l == "thank you."
        || l.chars().all(|c| !c.is_alphanumeric())
}

/// Download a whisper model, reporting progress 0..1.
pub fn download(id: &str, dest: PathBuf, mut progress: impl FnMut(f32)) -> Result<(), String> {
    use std::io::{Read, Write};
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{id}.bin");
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let resp = ureq::get(&url)
        .call()
        .map_err(|e| format!("Download failed: {e}. Check your internet connection."))?;
    let total = resp.body().content_length().unwrap_or(0);
    let mut reader = resp.into_body().into_reader();
    let tmp = dest.with_extension("part");
    let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 1 << 16];
    let mut got: u64 = 0;
    let mut last_report = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("Download interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        got += n as u64;
        if total > 0 && got - last_report > 1 << 20 {
            last_report = got;
            progress(got as f32 / total as f32);
        }
    }
    f.flush().map_err(|e| e.to_string())?;
    drop(f);
    if got < 1_000_000 {
        let _ = std::fs::remove_file(&tmp);
        return Err("Download failed: the file was incomplete.".into());
    }
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    progress(1.0);
    Ok(())
}
