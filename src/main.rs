// No console window behind the app on Windows (release builds).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! Silvery — private meeting notes for macOS, Windows and Linux. Records, transcribes on the computer and summarises
//! with a local LLM through Ollama.

mod app;
mod audio;
mod llm;
mod markdown;
mod mic;
mod platform;
mod model;
mod theme;
mod transcribe;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub const APP_NAME: &str = "Silvery";

/// The Silvery logo (assets/logo.svg rendered at 256 px), as width, height and RGBA pixels.
pub fn logo_rgba() -> (u32, u32, Vec<u8>) {
    let img = image::load_from_memory(include_bytes!("../assets/logo-256.png"))
        .expect("logo image")
        .to_rgba8();
    let (w, h) = img.dimensions();
    (w, h, img.into_raw())
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let shot_path = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);

    if args.iter().any(|a| a == "--install-app") {
        match mic::install_bundle() {
            Ok(p) => println!("installed {}", p.display()),
            Err(e) => println!("install failed: {e}"),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--mic-check") {
        // Listens for 3 seconds (nothing is saved) and reports whether sound arrived.
        println!("permission: {:?}", mic::permission());
        if args.iter().any(|a| a == "--ask") {
            let (tx, rx) = std::sync::mpsc::channel();
            mic::request(move |g| {
                let _ = tx.send(g);
            });
            println!("asked, granted: {:?}", rx.recv_timeout(std::time::Duration::from_secs(20)));
            println!("permission now: {:?}", mic::permission());
        }
        println!("inputs: {:?}  default: {:?}", mic::input_names(), mic::default_name());
        match audio::Recorder::start(egui::Context::default(), "") {
            Ok(r) => {
                std::thread::sleep(std::time::Duration::from_secs(3));
                println!(
                    "device: {}  samples: {}  heard anything: {}  peak: {:.4}",
                    r.device,
                    r.len(),
                    r.heard_anything(),
                    r.recent_peak(3.0)
                );
            }
            Err(e) => println!("could not start: {e}"),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--self-test") {
        self_test(&args);
        return Ok(());
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_inner_size([1240.0, 800.0])
        .with_min_inner_size([860.0, 560.0]);
    {
        let (width, height, rgba) = logo_rgba();
        viewport = viewport.with_icon(Arc::new(egui::IconData { rgba, width, height }));
    }

    let mut options = eframe::NativeOptions::default();
    if shot_path.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
        #[cfg(target_os = "macos")]
        {
            options.event_loop_builder = Some(Box::new(|b| {
                use winit::platform::macos::EventLoopBuilderExtMacOS;
                b.with_activate_ignoring_other_apps(false);
            }));
        }
        options.run_and_return = false;
    }
    options.viewport = viewport;

    let shot = shot_path.map(|path| app::ScreenshotPlan {
        path,
        frames: 120,
        done: Arc::new(AtomicBool::new(false)),
        requested: false,
    });

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, shot)))),
    )
}

/// Checks the AI and speech engines without opening the window:
/// `--self-test [speech.wav]`.
fn self_test(args: &[String]) {
    let store = model::Store::new(false);
    let settings = store.load_settings();
    println!("Ollama at {}", settings.ollama_url);
    match llm::list_models(&settings.ollama_url) {
        Ok(models) => {
            println!("models: {models:?}");
            let model = if models.contains(&settings.model) {
                settings.model.clone()
            } else {
                llm::pick_default(&models).unwrap_or_default()
            };
            println!("--- summary with {model} ---");
            let mut m = model::sample_meeting();
            m.summary.clear();
            let t0 = std::time::Instant::now();
            let r = llm::chat_stream(&settings.ollama_url, &model, &llm::summary_messages(&m, 0), |d| {
                print!("{d}");
                use std::io::Write;
                let _ = std::io::stdout().flush();
                true
            });
            println!("\n--- result: {r:?} in {:.1}s ---", t0.elapsed().as_secs_f32());
        }
        Err(e) => println!("AI check failed: {e}"),
    }
    if let Some(wav) = args.iter().skip_while(|a| *a != "--self-test").nth(1) {
        let path = store.whisper_path(&settings.whisper_model);
        if !path.exists() {
            println!("downloading speech model {}…", settings.whisper_model);
            if let Err(e) = transcribe::download(&settings.whisper_model, path.clone(), |_| {}) {
                println!("download failed: {e}");
                return;
            }
        }
        let bytes = std::fs::read(wav).expect("read wav");
        // 16-bit mono 16 kHz PCM after the "data" chunk header
        let start = bytes.windows(4).position(|w| w == b"data").map(|p| p + 8).unwrap_or(44);
        let samples: Vec<f32> = bytes[start..]
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
            .collect();
        let ctx = egui::Context::default();
        let (tx, rx) = std::sync::mpsc::channel();
        let w = transcribe::spawn_worker(tx, ctx);
        let t0 = std::time::Instant::now();
        let _ = w.send(transcribe::Cmd::Job(transcribe::Job {
            meeting_id: "test".into(),
            offset_ms: 0,
            samples,
            model_path: path,
            language: settings.language.clone(),
        }));
        while let Ok(msg) = rx.recv() {
            if let app::Msg::Transcribe(out) = msg {
                match out {
                    transcribe::Out::Segments { segments, .. } => {
                        for s in segments {
                            println!("[{}] {}", model::fmt_clock(s.start_ms), s.text);
                        }
                        println!("transcribed in {:.1}s", t0.elapsed().as_secs_f32());
                        break;
                    }
                    transcribe::Out::Error(e) => {
                        println!("transcription error: {e}");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}
