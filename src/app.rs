//! The window: meeting list on the left, the open meeting on the right.

use crate::audio::{self, Recorder, RATE};
use crate::llm::{self, TEMPLATES};
use crate::markdown;
use crate::model::{fmt_clock, ChatMsg, Meeting, Settings, Store, UNTITLED};
use crate::theme::*;
use crate::transcribe::{self, Cmd, Job};
use egui::{Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Sense, Stroke, Vec2};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

const CHUNK_SECS: usize = 12;

pub enum Msg {
    Models(Result<Vec<String>, String>),
    Llm { id: String, kind: LlmKind, delta: String },
    LlmDone { id: String, kind: LlmKind, err: Option<String> },
    Transcribe(transcribe::Out),
    Download(Result<f32, String>),
    MicPermission,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LlmKind {
    Summary,
    Chat,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Summary,
    Transcript,
    Notes,
    Ask,
}

#[derive(PartialEq)]
enum Ollama {
    Checking,
    Ready,
    Offline(String),
}

struct Recording {
    id: String,
    rec: Recorder,
    base_ms: i64,
    sent_upto: usize,
}

struct Generating {
    id: String,
    kind: LlmKind,
    cancel: Arc<AtomicBool>,
}

pub struct ScreenshotPlan {
    pub path: std::path::PathBuf,
    pub frames: u32,
    pub done: Arc<AtomicBool>,
    pub requested: bool,
}

pub struct App {
    store: Store,
    settings: Settings,
    meetings: Vec<Meeting>,
    selected: Option<String>,
    tab: Tab,
    search: String,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    worker: Sender<Cmd>,
    models: Vec<String>,
    ollama: Ollama,
    recording: Option<Recording>,
    jobs_outstanding: usize,
    transcribing: bool,
    transcribe_error: Option<String>,
    downloading: Option<f32>,
    download_error: Option<String>,
    generating: Option<Generating>,
    llm_error: Option<(String, String)>,
    pending_summary: Option<String>,
    ask_input: String,
    edit_summary: bool,
    show_settings: bool,
    confirm_delete: Option<String>,
    dirty: HashSet<String>,
    last_save: Instant,
    toast: Option<(String, Instant)>,
    mic_error: Option<String>,
    shot: Option<ScreenshotPlan>,
    frame_no: u32,
    tried_launch: bool,
    recheck_at: Option<Instant>,
    mic_perm: crate::mic::Permission,
    mic_names: Vec<String>,
    mic_default: Option<String>,
    mic_test: Option<Recorder>,
    logo: egui::TextureHandle,
    player: Option<crate::playback::Player>,
    menu_tx: Sender<crate::menu::Action>,
    menu_rx: Receiver<crate::menu::Action>,
    #[cfg(target_os = "macos")]
    native_menu: Option<crate::menu::NativeMenu>,
    show_guide: bool,
    last_play_seg: Option<usize>,
}

/// The Silvery logo tile (without the icon margin), for the sidebar.
fn load_logo(ctx: &egui::Context) -> egui::TextureHandle {
    let img = image::load_from_memory(include_bytes!("../assets/logo-tile-128.png"))
        .map(|i| i.to_rgba8())
        .unwrap_or_else(|_| image::RgbaImage::from_pixel(1, 1, image::Rgba([79, 91, 232, 255])));
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    ctx.load_texture("silvery-logo", color, egui::TextureOptions::LINEAR)
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, shot: Option<ScreenshotPlan>) -> Self {
        apply(&cc.egui_ctx);
        let store = Store::new(shot.is_some());
        let mut settings = store.load_settings();
        let mut meetings = store.load_meetings();
        if meetings.is_empty() && (!settings.seen_sample || shot.is_some()) {
            let s = crate::model::sample_meeting();
            store.save_meeting(&s);
            meetings.push(s);
            settings.seen_sample = true;
            store.save_settings(&settings);
        }
        let (tx, rx) = mpsc::channel();
        let (menu_tx, menu_rx) = mpsc::channel::<crate::menu::Action>();
        let worker = transcribe::spawn_worker(tx.clone(), cc.egui_ctx.clone());
        let selected = meetings.first().map(|m| m.id.clone());
        let mut app = Self {
            store,
            settings,
            meetings,
            selected,
            tab: Tab::Summary,
            search: String::new(),
            tx,
            rx,
            worker,
            models: Vec::new(),
            ollama: Ollama::Checking,
            recording: None,
            jobs_outstanding: 0,
            transcribing: false,
            transcribe_error: None,
            downloading: None,
            download_error: None,
            generating: None,
            llm_error: None,
            pending_summary: None,
            ask_input: String::new(),
            edit_summary: false,
            show_settings: false,
            confirm_delete: None,
            dirty: HashSet::new(),
            last_save: Instant::now(),
            toast: None,
            mic_error: None,
            shot,
            frame_no: 0,
            tried_launch: false,
            recheck_at: None,
            mic_perm: crate::mic::Permission::Unknown,
            mic_names: Vec::new(),
            mic_default: None,
            mic_test: None,
            logo: load_logo(&cc.egui_ctx),
            player: None,
            menu_tx: menu_tx.clone(),
            menu_rx,
            #[cfg(target_os = "macos")]
            native_menu: None,
            show_guide: false,
            last_play_seg: None,
        };
        app.refresh_mics();
        if app.shot.is_some() {
            // Lets a check picture a specific screen, e.g. SILVERY_VIEW=ask
            match std::env::var("SILVERY_VIEW").unwrap_or_default().as_str() {
                "transcript" => app.tab = Tab::Transcript,
                "notes" => app.tab = Tab::Notes,
                "ask" => app.tab = Tab::Ask,
                "settings" => app.show_settings = true,
                "playback" => {
                    if let Some(id) = app.selected.clone() {
                        // made-up audio as long as the example transcript, paused at 0:36
                        let n = RATE * 100;
                        let samples: Vec<i16> = (0..n).map(|i| ((i as f32 * 0.05).sin() * 3000.0) as i16).collect();
                        let p = crate::playback::Player::from_samples(&id, samples);
                        p.seek_ms(36_000);
                        app.player = Some(p);
                        app.tab = Tab::Transcript;
                    }
                }
                "recording" => {
                    if let Some(id) = app.selected.clone() {
                        let rec = Recorder::demo();
                        let sent_upto = rec.len();
                        app.recording = Some(Recording { id, rec, base_ms: 0, sent_upto });
                        app.tab = Tab::Transcript;
                    }
                }
                _ => {}
            }
        }
        #[cfg(target_os = "macos")]
        {
            let (w, h, rgba) = crate::logo_rgba();
            let icon = muda::Icon::from_rgba(rgba, w, h).ok();
            app.native_menu = crate::menu::NativeMenu::install(app.menu_tx.clone(), cc.egui_ctx.clone(), icon);
        }
        if app.shot.is_some() {
            if std::env::var("SILVERY_VIEW").as_deref() == Ok("guide") {
                app.show_guide = true;
            }
            // SILVERY_ACTIONS=new,transcript,… runs menu actions at start (for checks)
            if let Ok(list) = std::env::var("SILVERY_ACTIONS") {
                for a in list.split(',').filter_map(|n| crate::menu::Action::from_id(n.trim())) {
                    let _ = app.menu_tx.send(a);
                }
            }
        }
        app.check_ollama(&cc.egui_ctx);
        app
    }

    // ---------- helpers ----------

    fn meeting(&self, id: &str) -> Option<&Meeting> {
        self.meetings.iter().find(|m| m.id == id)
    }
    fn meeting_mut(&mut self, id: &str) -> Option<&mut Meeting> {
        self.meetings.iter_mut().find(|m| m.id == id)
    }
    fn touch(&mut self, id: &str) {
        self.dirty.insert(id.to_string());
    }
    fn flash(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }
    fn save_dirty(&mut self, force: bool) {
        if self.dirty.is_empty() || (!force && self.last_save.elapsed() < Duration::from_millis(800)) {
            return;
        }
        for id in std::mem::take(&mut self.dirty) {
            if let Some(m) = self.meeting(&id) {
                self.store.save_meeting(m);
            }
        }
        self.last_save = Instant::now();
    }
    fn whisper_path(&self) -> std::path::PathBuf {
        self.store.whisper_path(&self.settings.whisper_model)
    }

    fn refresh_mics(&mut self) {
        self.mic_perm = crate::mic::permission();
        self.mic_names = crate::mic::input_names();
        self.mic_default = crate::mic::default_name();
    }

    fn ask_mic_permission(&self, ctx: &egui::Context) {
        if self.mic_perm == crate::mic::Permission::NotAsked {
            let tx = self.tx.clone();
            let c = ctx.clone();
            crate::mic::request(move |_| {
                let _ = tx.send(Msg::MicPermission);
                c.request_repaint();
            });
        }
    }

    fn check_ollama(&mut self, ctx: &egui::Context) {
        self.ollama = Ollama::Checking;
        let url = self.settings.ollama_url.clone();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Models(llm::list_models(&url)));
            ctx.request_repaint();
        });
    }

    fn start_download(&mut self, ctx: &egui::Context) {
        if self.downloading.is_some() || self.store.readonly {
            return;
        }
        self.downloading = Some(0.0);
        self.download_error = None;
        let id = self.settings.whisper_model.clone();
        let dest = self.whisper_path();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let tx2 = tx.clone();
            let c2 = ctx.clone();
            let r = transcribe::download(&id, dest, move |p| {
                let _ = tx2.send(Msg::Download(Ok(p)));
                c2.request_repaint();
            });
            let _ = tx.send(Msg::Download(match r {
                Ok(()) => Ok(2.0),
                Err(e) => Err(e),
            }));
            ctx.request_repaint();
        });
    }

    fn new_meeting(&mut self) {
        let mut m = Meeting::new();
        m.template = self.settings.default_template;
        let id = m.id.clone();
        self.meetings.insert(0, m);
        self.selected = Some(id.clone());
        self.tab = Tab::Notes;
        self.edit_summary = false;
        self.touch(&id);
    }

    fn save_to_downloads(&mut self, id: &str) {
        if let Some(m) = self.meeting(id) {
            let dir = dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let safe: String = m
                .title
                .chars()
                .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' })
                .collect();
            let path = dir.join(format!("{}.md", safe.trim()));
            let msg = match std::fs::write(&path, m.to_markdown()) {
                Ok(()) => format!("Saved to Downloads as \"{}.md\"", safe.trim()),
                Err(e) => format!("Couldn't save: {e}"),
            };
            self.flash(msg);
        }
    }

    /// What the menus should show right now.
    fn menu_state(&self) -> crate::menu::State {
        let sel = self.selected.clone().filter(|id| self.meeting(id).is_some());
        let playing = self.player.as_ref().map(|p| p.is_playing()).unwrap_or(false);
        crate::menu::State {
            has_meeting: sel.is_some(),
            recording: self.recording.is_some(),
            paused: self.recording.as_ref().map(|r| r.rec.is_paused()).unwrap_or(false),
            has_audio: sel.as_deref().map(|id| self.has_audio(id)).unwrap_or(false),
            playing,
        }
    }

    /// Do what a menu item or keyboard shortcut asked for.
    fn run_action(&mut self, ctx: &egui::Context, a: crate::menu::Action) {
        use crate::menu::Action as A;
        let sel = self.selected.clone().filter(|id| self.meeting(id).is_some());
        match a {
            A::NewMeeting => self.new_meeting(),
            A::CopyText => {
                if let Some(m) = sel.as_deref().and_then(|id| self.meeting(id)) {
                    ctx.copy_text(m.to_markdown());
                    self.flash("Copied to clipboard");
                }
            }
            A::SaveToDownloads => {
                if let Some(id) = sel {
                    self.save_to_downloads(&id);
                }
            }
            A::Delete => self.confirm_delete = sel,
            A::RecordStop => {
                if self.recording.is_some() {
                    self.stop_recording();
                } else if let Some(id) = sel {
                    self.start_recording(ctx, &id);
                }
            }
            A::PauseRecording => {
                if let Some(r) = &self.recording {
                    r.rec.set_paused(!r.rec.is_paused());
                }
            }
            A::PlayPause => {
                if let Some(id) = sel {
                    let playing = self.player.as_ref().filter(|p| p.meeting_id == id).map(|p| p.is_playing()).unwrap_or(false);
                    if playing {
                        if let Some(p) = &self.player {
                            p.pause();
                        }
                    } else {
                        self.play_from(ctx, &id, None);
                    }
                }
            }
            A::Summary => self.tab = Tab::Summary,
            A::Transcript => self.tab = Tab::Transcript,
            A::Notes => self.tab = Tab::Notes,
            A::Ask => self.tab = Tab::Ask,
            A::Search => ctx.memory_mut(|m| m.request_focus(egui::Id::unique("silvery-search"))),
            A::Guide => self.show_guide = true,
            A::Settings => {
                self.refresh_mics();
                self.show_settings = true;
            }
            A::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    /// The user guide, shown inside Silvery so it works offline.
    fn guide_window(&mut self, ctx: &egui::Context) {
        if !self.show_guide {
            return;
        }
        let mut open = true;
        let screen = ctx.content_rect();
        egui::Window::new("Silvery user guide")
            .fade_in(false)
            .title_bar(false)
            .collapsible(false)
            .resizable(true)
            .default_size([720.0, (screen.height() - 120.0).min(760.0)])
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(Frame::new().fill(BG).stroke(Stroke::new(1.0, BORDER)).corner_radius(14).inner_margin(20).shadow(ctx.global_style().visuals.window_shadow))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("User guide").size(19.0).strong().color(TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Done").clicked() {
                            open = false;
                        }
                    });
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    markdown::show(ui, &guide_text());
                });
            });
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_guide = false;
        }
    }

    /// Whether this meeting has recorded audio that can be played back.
    fn has_audio(&self, id: &str) -> bool {
        if self.player.as_ref().map(|p| p.meeting_id == id).unwrap_or(false) {
            return true;
        }
        self.meeting(id)
            .map(|m| m.recordings.iter().any(|f| self.store.recordings_dir().join(f).exists()))
            .unwrap_or(false)
    }

    /// Make sure the player holds this meeting's audio. Returns false if there is none.
    fn ensure_player(&mut self, id: &str) -> bool {
        if self.player.as_ref().map(|p| p.meeting_id == id).unwrap_or(false) {
            return true;
        }
        self.player = None;
        let files = self.meeting(id).map(|m| m.recordings.clone()).unwrap_or_default();
        self.player = crate::playback::Player::load(id, &self.store.recordings_dir(), &files);
        self.last_play_seg = None;
        self.player.is_some()
    }

    /// Play this meeting's recording from `ms` (or from where it was, if None).
    fn play_from(&mut self, ctx: &egui::Context, id: &str, ms: Option<i64>) {
        if self.recording.is_some() || !self.ensure_player(id) {
            return;
        }
        if let Some(p) = self.player.as_mut() {
            if let Some(ms) = ms {
                p.seek_ms(ms);
            }
            p.play(ctx);
        }
    }

    fn start_recording(&mut self, ctx: &egui::Context, id: &str) {
        self.player = None;
        if self.recording.is_some() {
            return;
        }
        self.mic_error = None;
        self.mic_test = None;
        self.refresh_mics();
        self.ask_mic_permission(ctx);
        if !self.whisper_path().exists() {
            self.start_download(ctx);
        }
        match Recorder::start(ctx.clone(), &self.settings.mic) {
            Ok(rec) => {
                let base_ms = self.meeting(id).map(|m| m.duration_ms).unwrap_or(0);
                self.recording = Some(Recording { id: id.to_string(), rec, base_ms, sent_upto: 0 });
                self.tab = Tab::Transcript;
            }
            Err(e) => self.mic_error = Some(e),
        }
    }

    fn send_chunk(&mut self, final_chunk: bool) {
        let Some(r) = self.recording.as_mut() else { return };
        let (chunk, cut) = {
            let buf = r.rec.samples.lock().unwrap();
            let len = buf.len();
            if len <= r.sent_upto {
                return;
            }
            if !final_chunk && len - r.sent_upto < CHUNK_SECS * RATE {
                return;
            }
            let cut = if final_chunk { len } else { audio::quiet_cut(&buf, r.sent_upto, len) };
            (buf[r.sent_upto..cut].to_vec(), cut)
        };
        let offset_ms = r.base_ms + (r.sent_upto as i64 * 1000 / RATE as i64);
        r.sent_upto = cut;
        let job = Job {
            meeting_id: r.id.clone(),
            offset_ms,
            samples: chunk,
            model_path: self.store.whisper_path(&self.settings.whisper_model),
            language: self.settings.language.clone(),
        };
        self.jobs_outstanding += 1;
        let _ = self.worker.send(Cmd::Job(job));
    }

    fn stop_recording(&mut self) {
        self.send_chunk(true);
        let Some(mut r) = self.recording.take() else { return };
        r.rec.stop();
        let samples = r.rec.samples.lock().map(|v| v.clone()).unwrap_or_default();
        let id = r.id.clone();
        if let Some(m) = self.meeting_mut(&id) {
            m.duration_ms = r.base_ms + samples.len() as i64 * 1000 / RATE as i64;
            let name = format!("{}-{}.wav", m.id, m.recordings.len() + 1);
            m.recordings.push(name.clone());
            if !self.store.readonly && !samples.is_empty() {
                let path = self.store.recordings_dir().join(name);
                std::thread::spawn(move || {
                    let _ = audio::write_wav(&path, &samples);
                });
            }
        }
        self.touch(&id);
        self.pending_summary = Some(id);
    }

    fn start_llm(&mut self, ctx: &egui::Context, id: &str, kind: LlmKind) {
        if self.generating.is_some() {
            return;
        }
        let model = self.settings.model.clone();
        if model.is_empty() {
            self.llm_error = Some((id.to_string(), "No AI model is chosen. Open Settings and pick one.".into()));
            return;
        }
        let Some(m) = self.meeting_mut(id) else { return };
        let msgs = match kind {
            LlmKind::Summary => {
                let t = m.template;
                m.summary.clear();
                llm::summary_messages(m, t)
            }
            LlmKind::Chat => {
                let msgs = llm::ask_messages(m);
                m.chat.push(ChatMsg { role: "assistant".into(), content: String::new() });
                msgs
            }
        };
        self.llm_error = None;
        let cancel = Arc::new(AtomicBool::new(false));
        self.generating = Some(Generating { id: id.to_string(), kind, cancel: cancel.clone() });
        let url = self.settings.ollama_url.clone();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let id = id.to_string();
        std::thread::spawn(move || {
            let tx2 = tx.clone();
            let (c2, id2) = (ctx.clone(), id.clone());
            let r = llm::chat_stream(&url, &model, &msgs, move |d| {
                let _ = tx2.send(Msg::Llm { id: id2.clone(), kind, delta: d.to_string() });
                c2.request_repaint();
                !cancel.load(Ordering::Relaxed)
            });
            let _ = tx.send(Msg::LlmDone { id, kind, err: r.err() });
            ctx.request_repaint();
        });
    }

    fn handle_messages(&mut self, ctx: &egui::Context) {
        if self.recheck_at.map(|t| Instant::now() >= t).unwrap_or(false) {
            self.recheck_at = None;
            self.check_ollama(ctx);
        }
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Models(Ok(list)) => {
                    self.models = list;
                    self.ollama = Ollama::Ready;
                    if !self.models.contains(&self.settings.model) {
                        if let Some(d) = llm::pick_default(&self.models) {
                            self.settings.model = d;
                            self.store.save_settings(&self.settings);
                        }
                    }
                }
                Msg::Models(Err(e)) => {
                    self.ollama = Ollama::Offline(e);
                    // Start Ollama in the background once, so people don't have to.
                    if !self.tried_launch && self.shot.is_none() && launch_ollama() {
                        self.tried_launch = true;
                        self.ollama = Ollama::Checking;
                        self.recheck_at = Some(Instant::now() + Duration::from_secs(4));
                        ctx.request_repaint_after(Duration::from_secs(4));
                    }
                }
                Msg::Llm { id, kind, delta } => {
                    if let Some(m) = self.meeting_mut(&id) {
                        match kind {
                            LlmKind::Summary => m.summary.push_str(&delta),
                            LlmKind::Chat => {
                                if let Some(last) = m.chat.last_mut() {
                                    last.content.push_str(&delta);
                                }
                            }
                        }
                    }
                }
                Msg::LlmDone { id, kind, err } => {
                    self.generating = None;
                    if let Some(m) = self.meeting_mut(&id) {
                        match kind {
                            LlmKind::Summary => {
                                let clean = llm::strip_think(&m.summary).trim().to_string();
                                let mut lines = clean.lines();
                                let mut body = clean.clone();
                                if let Some(first) = lines.next() {
                                    if let Some(t) = first.trim().strip_prefix("# ") {
                                        if m.title == UNTITLED {
                                            m.title = t.trim().trim_matches('*').to_string();
                                        }
                                        body = lines.collect::<Vec<_>>().join("\n").trim().to_string();
                                    }
                                }
                                m.summary = if body.is_empty() { String::new() } else { body + "\n" };
                            }
                            LlmKind::Chat => {
                                if let Some(last) = m.chat.last_mut() {
                                    last.content = llm::strip_think(&last.content).trim().to_string();
                                    if last.content.is_empty() && err.is_some() {
                                        m.chat.pop();
                                    }
                                }
                            }
                        }
                    }
                    if let Some(e) = err {
                        self.llm_error = Some((id.clone(), e));
                        self.check_ollama(ctx);
                    }
                    self.touch(&id);
                }
                Msg::Transcribe(out) => match out {
                    transcribe::Out::Busy(b) => self.transcribing = b,
                    transcribe::Out::Segments { meeting_id, segments } => {
                        self.jobs_outstanding = self.jobs_outstanding.saturating_sub(1);
                        if let Some(m) = self.meeting_mut(&meeting_id) {
                            m.transcript.extend(segments);
                            m.transcript.sort_by_key(|s| s.start_ms);
                        }
                        self.touch(&meeting_id);
                    }
                    transcribe::Out::Error(e) => {
                        self.jobs_outstanding = self.jobs_outstanding.saturating_sub(1);
                        self.transcribe_error = Some(e);
                    }
                },
                Msg::Download(Ok(p)) if p > 1.5 => {
                    self.downloading = None;
                    let _ = self.worker.send(Cmd::ModelReady);
                    self.flash("Speech model ready");
                }
                Msg::Download(Ok(p)) => self.downloading = Some(p),
                Msg::MicPermission => self.refresh_mics(),
                Msg::Download(Err(e)) => {
                    self.downloading = None;
                    self.download_error = Some(e);
                }
            }
        }

        // Write the summary automatically once the last words are transcribed.
        if let Some(id) = self.pending_summary.clone() {
            let model_missing = !self.whisper_path().exists();
            if self.jobs_outstanding == 0 || (model_missing && self.downloading.is_none()) {
                self.pending_summary = None;
                let has_material = self
                    .meeting(&id)
                    .map(|m| !m.transcript.is_empty() || !m.notes.trim().is_empty())
                    .unwrap_or(false);
                if has_material && self.ollama == Ollama::Ready && self.generating.is_none() {
                    if self.selected.as_deref() == Some(id.as_str()) {
                        self.tab = Tab::Summary;
                    }
                    self.start_llm(ctx, &id, LlmKind::Summary);
                }
            }
        }
    }

    // ---------- sidebar ----------

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
            ui.painter().image(
                self.logo.id(),
                r,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new(crate::APP_NAME).size(17.0).strong().color(TEXT));
                ui.label(RichText::new(format!("Private · runs on {}", crate::platform::THIS)).size(11.5).color(MUTED));
            });
        });
        ui.add_space(14.0);
        let w = ui.available_width();
        let b = egui::Button::new(RichText::new("+  New meeting").color(Color32::WHITE).size(14.5))
            .fill(ACCENT)
            .corner_radius(9)
            .min_size(Vec2::new(w, 38.0));
        if ui.add(b).clicked() {
            self.new_meeting();
        }
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .id(egui::Id::unique("silvery-search"))
                .hint_text("Search meetings")
                .desired_width(w)
                .margin(Margin::symmetric(10, 7)),
        );
        ui.add_space(8.0);

        egui::Panel::bottom("sidebar_bottom")
            .frame(Frame::new().fill(SIDEBAR).inner_margin(Margin::symmetric(0, 8)))
            .show(ui, |ui| self.sidebar_footer(ui));

        let q = self.search.to_lowercase();
        let today = chrono::Local::now().date_naive();
        let mut clicked: Option<String> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let mut last_day = None;
            for m in &self.meetings {
                if !q.is_empty()
                    && !m.title.to_lowercase().contains(&q)
                    && !m.notes.to_lowercase().contains(&q)
                    && !m.summary.to_lowercase().contains(&q)
                    && !m.transcript.iter().any(|s| s.text.to_lowercase().contains(&q))
                {
                    continue;
                }
                let day = m.created.date_naive();
                if last_day != Some(day) {
                    last_day = Some(day);
                    let label = if day == today {
                        "Today".to_string()
                    } else if Some(day) == today.pred_opt() {
                        "Yesterday".to_string()
                    } else {
                        m.created.format("%A %e %B").to_string()
                    };
                    ui.add_space(10.0);
                    ui.label(RichText::new(label).size(11.0).color(MUTED).strong());
                    ui.add_space(2.0);
                }
                let selected = self.selected.as_deref() == Some(m.id.as_str());
                let is_rec = self.recording.as_ref().map(|r| r.id == m.id).unwrap_or(false);

                // One compact line per meeting: title on the left, time and length on the right.
                let width = ui.available_width();
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
                let painter = ui.painter();
                if selected {
                    painter.rect(rect, 7, Color32::WHITE, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
                } else if resp.hovered() {
                    painter.rect_filled(rect, 7, Color32::from_black_alpha(8));
                }
                let mut meta = m.created.format("%H:%M").to_string();
                if m.duration_ms > 0 {
                    let mins = (m.duration_ms / 60_000).max(1);
                    meta.push_str(&if mins >= 60 {
                        format!(" · {}h {:02}m", mins / 60, mins % 60)
                    } else {
                        format!(" · {mins}m")
                    });
                }
                let meta_g = painter.layout_no_wrap(meta, egui::FontId::proportional(12.0), MUTED);
                let right = rect.right() - 9.0;
                painter.galley(egui::pos2(right - meta_g.size().x, rect.center().y - meta_g.size().y / 2.0), meta_g.clone(), MUTED);

                let mut left = rect.left() + 9.0;
                if is_rec {
                    painter.circle_filled(egui::pos2(left + 4.0, rect.center().y), 4.0, RED);
                    left += 14.0;
                }
                let mut job = egui::text::LayoutJob::simple_singleline(
                    m.title.clone(),
                    egui::FontId::proportional(14.0),
                    if selected { TEXT } else { BODY },
                );
                job.wrap = egui::text::TextWrapping {
                    max_width: (right - meta_g.size().x - 10.0 - left).max(20.0),
                    max_rows: 1,
                    break_anywhere: true,
                    overflow_character: Some('…'),
                };
                let title_g = ui.fonts_mut(|f| f.layout_job(job));
                painter.galley(egui::pos2(left, rect.center().y - title_g.size().y / 2.0), title_g, TEXT);

                let resp = resp.on_hover_text(&m.title);
                if resp.clicked() {
                    clicked = Some(m.id.clone());
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                ui.add_space(1.0);
            }
            if self.meetings.is_empty() {
                ui.add_space(20.0);
                ui.label(RichText::new("No meetings yet").color(MUTED));
            }
        });
        if let Some(id) = clicked {
            if self.selected.as_deref() != Some(id.as_str()) {
                if self.player.as_ref().map(|p| p.meeting_id != id).unwrap_or(false) {
                    self.player = None;
                }
                self.selected = Some(id);
                self.edit_summary = false;
                self.llm_error = None;
            }
        }
    }

    fn sidebar_footer(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let (color, text) = match &self.ollama {
                Ollama::Ready if !self.settings.model.is_empty() => (GREEN, format!("AI: {}", self.settings.model)),
                Ollama::Ready => (AMBER, "AI: no model installed".to_string()),
                Ollama::Checking => (AMBER, "AI: connecting…".to_string()),
                Ollama::Offline(_) => (RED, "AI offline — open Ollama".to_string()),
            };
            let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 4.0, color);
            ui.add(egui::Label::new(RichText::new(text).size(12.5).color(BODY)).truncate());
        });
        ui.add_space(4.0);
        let w = ui.available_width();
        if ui
            .add(egui::Button::new(RichText::new("Settings").size(13.5)).min_size(Vec2::new(w, 32.0)))
            .clicked()
        {
            self.show_settings = true;
        }
    }

    // ---------- main area ----------

    fn main_area(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected.clone().filter(|id| self.meeting(id).is_some()) else {
            self.empty_state(ui);
            return;
        };
        let ctx = ui.ctx().clone();

        // Title row
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.menu_button(RichText::new("•••").size(14.0), |ui| {
                    if ui.button("Copy as text").clicked() {
                        if let Some(m) = self.meeting(&id) {
                            ui.ctx().copy_text(m.to_markdown());
                        }
                        self.flash("Copied to clipboard");
                        ui.close();
                    }
                    if ui.button("Save to Downloads").clicked() {
                        self.save_to_downloads(&id);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(RichText::new("Delete meeting").color(RED)).clicked() {
                        self.confirm_delete = Some(id.clone());
                        ui.close();
                    }
                });
                let m = self.meetings.iter_mut().find(|m| m.id == id).unwrap();
                let r = ui.add(
                    egui::TextEdit::singleline(&mut m.title)
                        .font(egui::FontId::proportional(26.0))
                        .text_color(TEXT)
                        .frame(Frame::NONE)
                        .desired_width(ui.available_width())
                        .hint_text(UNTITLED),
                );
                if r.changed() {
                    self.dirty.insert(id.clone());
                }
            });
        });
        let m = self.meeting(&id).unwrap();
        let mut sub = m.created.format("%A %e %B %Y · %H:%M").to_string();
        if m.duration_ms > 0 {
            sub.push_str(&format!(" · {}", fmt_clock(m.duration_ms)));
        }
        ui.label(RichText::new(sub).color(MUTED).size(13.0));
        ui.add_space(14.0);

        self.record_bar(ui, &ctx, &id);
        ui.add_space(14.0);
        self.tabs(ui);
        ui.add_space(10.0);

        match self.tab {
            Tab::Summary => self.summary_tab(ui, &ctx, &id),
            Tab::Transcript => self.transcript_tab(ui, &id),
            Tab::Notes => self.notes_tab(ui, &id),
            Tab::Ask => self.ask_tab(ui, &ctx, &id),
        }
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            ui.label(RichText::new("No meeting open").size(22.0).color(TEXT));
            ui.add_space(6.0);
            ui.label(RichText::new("Start a new meeting to record, take notes and get a summary.").color(MUTED));
            ui.add_space(14.0);
            if ui
                .add(egui::Button::new(RichText::new("+  New meeting").color(Color32::WHITE)).fill(ACCENT).min_size(Vec2::new(180.0, 40.0)))
                .clicked()
            {
                self.new_meeting();
            }
        });
    }

    fn record_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, id: &str) {
        let here = self.recording.as_ref().map(|r| r.id == id).unwrap_or(false);
        let elsewhere = self.recording.is_some() && !here;
        let mut action: Option<&str> = None;
        Frame::new()
            .fill(CARD)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(12)
            .inner_margin(Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if here {
                        let r = self.recording.as_ref().unwrap();
                        let paused = r.rec.is_paused();
                        let ms = r.base_ms + r.rec.len() as i64 * 1000 / RATE as i64;
                        let t = ui.input(|i| i.time);
                        let alpha = if paused { 0.35 } else { 0.55 + 0.45 * ((t * 3.0).sin() as f32).abs() };
                        let (dot, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 6.0, RED.gamma_multiply(alpha));
                        ui.label(
                            RichText::new(if paused { "Paused" } else { "Recording" }).color(RED).strong().size(15.0),
                        );
                        ui.label(RichText::new(fmt_clock(ms)).monospace().size(15.0).color(TEXT));
                        ui.add_space(8.0);
                        sound_wave(ui, &r.rec, paused);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let stop = egui::Button::new(RichText::new("      Stop").color(Color32::WHITE).size(14.5))
                                .fill(RED)
                                .min_size(Vec2::new(96.0, 34.0));
                            let sr = ui.add(stop);
                            let c = egui::pos2(sr.rect.left() + 24.0, sr.rect.center().y);
                            ui.painter().rect_filled(egui::Rect::from_center_size(c, Vec2::splat(10.0)), 2, Color32::WHITE);
                            if sr.clicked() {
                                action = Some("stop");
                            }
                            let label = if paused { "Resume" } else { "Pause" };
                            if ui.add(egui::Button::new(label).min_size(Vec2::new(80.0, 34.0))).clicked() {
                                action = Some("pause");
                            }
                        });
                        ctx.request_repaint_after(Duration::from_millis(50));
                    } else {
                        let rec = egui::Button::new(RichText::new("      Record").color(Color32::WHITE).size(15.0))
                            .fill(if elsewhere { MUTED } else { RED })
                            .corner_radius(9)
                            .min_size(Vec2::new(120.0, 36.0));
                        let rr = ui.add_enabled(!elsewhere, rec);
                        ui.painter().circle_filled(egui::pos2(rr.rect.left() + 24.0, rr.rect.center().y), 6.0, Color32::WHITE);
                        if rr.clicked() {
                            action = Some("start");
                        }
                        ui.add_space(6.0);
                        if !elsewhere && self.has_audio(id) {
                            self.player_controls(ui, ctx, id);
                            return;
                        }
                        let hint = if elsewhere {
                            "Another meeting is being recorded."
                        } else if self.meeting(id).map(|m| m.transcript.is_empty()).unwrap_or(true) {
                            "Record the conversation. It's turned into text right here, privately."
                        } else {
                            "Record more. New words are added to the transcript."
                        };
                        ui.label(RichText::new(hint).color(BODY));
                    }
                });

                if here {
                    let r = self.recording.as_ref().unwrap();
                    let paused = r.rec.is_paused();
                    let elapsed = r.rec.started.elapsed().as_secs_f32();
                    let heard = r.rec.heard_anything();
                    let quiet_long = elapsed > 12.0 && r.rec.recent_peak(10.0) < 0.004;
                    if !paused && !heard && elapsed > 3.0 {
                        ui.add_space(8.0);
                        Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0, RED)).corner_radius(10).inner_margin(12).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new("Silvery isn't receiving any sound, so nothing can be transcribed.").strong().color(RED));
                            let as_app = crate::mic::in_app_bundle();
                            let why = if !as_app {
                                &"macOS only lets Silvery hear the microphone when it runs as its own app. Press Stop, then “Open Silvery app”. macOS will ask once for microphone access.".to_string()
                            } else if self.mic_perm == crate::mic::Permission::Denied {
                                &format!("{} is blocking the microphone for this app. Turn it on in {}, then press Stop and Record again.", crate::platform::OS, crate::platform::MIC_SETTINGS)
                            } else {
                                &format!("{} may be blocking the microphone, or this microphone is muted. Check {}, or choose another microphone, then press Stop and Record again.", crate::platform::OS, crate::platform::MIC_SETTINGS)
                            };
                            ui.label(RichText::new(why).color(BODY).size(13.0));
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if !as_app {
                                    let b = egui::Button::new(RichText::new("Open Silvery app").color(Color32::WHITE)).fill(ACCENT);
                                    if ui.add(b).clicked() {
                                        action = Some("bundle");
                                    }
                                }
                                if crate::platform::CAN_OPEN_MIC_SETTINGS && ui.button("Open microphone settings").clicked() {
                                    action = Some("privacy");
                                }
                                if ui.button("Choose another microphone").clicked() {
                                    action = Some("settings");
                                }
                            });
                        });
                    } else if !paused && quiet_long {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("It has been very quiet for a while. Speak closer to the microphone or choose another one in Settings.")
                                .size(12.5)
                                .color(AMBER),
                        );
                    }
                    // Brief hiccups don't stop the recording; just say so quietly.
                    let skips = r.rec.skips();
                    if skips > 0 {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(if skips == 1 {
                                "A moment of audio was skipped. Recording continues.".to_string()
                            } else {
                                format!("{skips} brief moments of audio were skipped. Recording continues.")
                            })
                            .size(12.0)
                            .color(MUTED),
                        );
                    }
                } else if !elsewhere && !crate::mic::in_app_bundle() && self.mic_perm != crate::mic::Permission::Allowed {
                    ui.add_space(8.0);
                    Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0, AMBER)).corner_radius(10).inner_margin(12).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("The microphone isn't available yet").strong().color(TEXT));
                        ui.label(
                            RichText::new("macOS only lets Silvery hear the microphone when it runs as its own app. Press the button: Silvery opens as an app and macOS asks once for microphone access. Your meetings come with you.")
                                .size(13.0)
                                .color(BODY),
                        );
                        ui.add_space(4.0);
                        let b = egui::Button::new(RichText::new("Open Silvery app").color(Color32::WHITE)).fill(ACCENT);
                        if ui.add(b).clicked() {
                            action = Some("bundle");
                        }
                    });
                } else if !elsewhere {
                    if self.mic_perm == crate::mic::Permission::Denied {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{} is blocking the microphone for this app, so recordings would be silent.", crate::platform::OS)).size(12.5).color(RED));
                            if crate::platform::CAN_OPEN_MIC_SETTINGS && ui.small_button("Open microphone settings").clicked() {
                                action = Some("privacy");
                            }
                        });
                    }
                }

                // Status line: download, transcription, errors
                if let Some(p) = self.downloading {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Getting the speech model ready (one time only)…").size(12.5).color(BODY));
                        ui.add(egui::ProgressBar::new(p).desired_width(180.0).show_percentage());
                    });
                } else if self.transcribing || (self.jobs_outstanding > 0 && !here) {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Turning speech into text…").size(12.5).color(BODY));
                    });
                }
                let player_error = self.player.as_ref().and_then(|p| p.error.clone());
                for e in [&self.mic_error, &self.download_error, &self.transcribe_error, &player_error].into_iter().flatten() {
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("{e}")).size(12.5).color(RED));
                }
                if self.download_error.is_some() && ui.small_button("Try download again").clicked() {
                    action = Some("download");
                }
            });
        match action {
            Some("start") => self.start_recording(ctx, id),
            Some("stop") => self.stop_recording(),
            Some("pause") => {
                if let Some(r) = &self.recording {
                    r.rec.set_paused(!r.rec.is_paused());
                }
            }
            Some("download") => self.start_download(ctx),
            Some("privacy") => crate::platform::open_mic_settings(),
            Some("bundle") => {
                if self.recording.is_some() {
                    self.stop_recording();
                }
                self.save_dirty(true);
                match crate::mic::open_as_app() {
                    Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(e) => self.mic_error = Some(e),
                }
            }
            Some("settings") => {
                self.refresh_mics();
                self.show_settings = true;
            }
            _ => {}
        }
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (t, name) in [
                (Tab::Summary, "Summary"),
                (Tab::Transcript, "Transcript"),
                (Tab::Notes, "My notes"),
                (Tab::Ask, "Ask"),
            ] {
                let sel = self.tab == t;
                let b = egui::Button::new(RichText::new(name).size(14.5).color(if sel { ACCENT } else { BODY }))
                    .fill(if sel { ACCENT_SOFT } else { Color32::TRANSPARENT })
                    .stroke(Stroke::NONE)
                    .corner_radius(CornerRadius::same(8))
                    .min_size(Vec2::new(0.0, 32.0));
                if ui.add(b).clicked() {
                    self.tab = t;
                }
            }
        });
        let r = ui.cursor();
        ui.painter().hline(r.x_range(), r.top() + 4.0, Stroke::new(1.0, BORDER));
        ui.add_space(6.0);
    }

    fn summary_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, id: &str) {
        let busy_here = self.generating.as_ref().map(|g| g.id == id && g.kind == LlmKind::Summary).unwrap_or(false);
        let has_summary = self.meeting(id).map(|m| !m.summary.trim().is_empty()).unwrap_or(false);
        let has_material = self
            .meeting(id)
            .map(|m| !m.transcript.is_empty() || !m.notes.trim().is_empty())
            .unwrap_or(false);

        // toolbar
        ui.horizontal(|ui| {
            let mut tpl = self.meeting(id).map(|m| m.template).unwrap_or(0);
            ui.label(RichText::new("Style").color(MUTED));
            egui::ComboBox::from_id_salt("tpl")
                .selected_text(TEMPLATES[tpl.min(TEMPLATES.len() - 1)].name)
                .width(150.0)
                .show_ui(ui, |ui| {
                    for (i, t) in TEMPLATES.iter().enumerate() {
                        ui.selectable_value(&mut tpl, i, t.name);
                    }
                });
            if self.meeting(id).map(|m| m.template != tpl).unwrap_or(false) {
                if let Some(m) = self.meeting_mut(id) {
                    m.template = tpl;
                }
                self.settings.default_template = tpl;
                self.store.save_settings(&self.settings);
                self.touch(id);
            }
            if busy_here {
                if ui.button("Stop writing").clicked() {
                    if let Some(g) = &self.generating {
                        g.cancel.store(true, Ordering::Relaxed);
                    }
                }
            } else {
                let label = if has_summary { "Rewrite summary" } else { "Write summary" };
                let b = egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(ACCENT);
                let enabled = has_material && self.generating.is_none() && self.recording.as_ref().map(|r| r.id != id).unwrap_or(true);
                if ui.add_enabled(enabled, b).on_disabled_hover_text("Record or type some notes first").clicked() {
                    self.edit_summary = false;
                    self.start_llm(ctx, id, LlmKind::Summary);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if has_summary && !busy_here {
                    if ui.button("Copy").clicked() {
                        if let Some(m) = self.meeting(id) {
                            ui.ctx().copy_text(llm::strip_think(&m.summary));
                        }
                        self.flash("Summary copied");
                    }
                    let lbl = if self.edit_summary { "Done" } else { "Edit" };
                    if ui.button(lbl).clicked() {
                        self.edit_summary = !self.edit_summary;
                    }
                }
            });
        });
        ui.add_space(8.0);

        if let Some((eid, e)) = &self.llm_error {
            if eid == id {
                let e = e.clone();
                Frame::new().fill(RED_SOFT).corner_radius(10).inner_margin(12).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(format!("The AI couldn't write the summary. {e}")).color(RED));
                });
                ui.add_space(8.0);
            }
        }

        let model = self.settings.model.clone();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if busy_here {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(format!("Writing with {model} on {}…", crate::platform::THIS)).color(MUTED));
                });
                ui.add_space(6.0);
            }
            let edit = self.edit_summary && !busy_here;
            let Some(m) = self.meetings.iter_mut().find(|m| m.id == id) else { return };
            if edit {
                let r = ui.add(
                    egui::TextEdit::multiline(&mut m.summary)
                        .desired_width(f32::INFINITY)
                        .desired_rows(20)
                        .font(egui::TextStyle::Monospace),
                );
                if r.changed() {
                    self.dirty.insert(id.to_string());
                }
            } else if !m.summary.trim().is_empty() {
                let shown = llm::strip_think(&m.summary);
                let shown = shown.trim_start().strip_prefix("# ").map(|s| s.split_once('\n').map(|x| x.1).unwrap_or("")).unwrap_or(&shown).to_string();
                if let Some(line) = markdown::show(ui, &shown) {
                    m.summary = markdown::toggle_line(&shown, line);
                    self.dirty.insert(id.to_string());
                }
            } else if !busy_here {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No summary yet").size(17.0).color(TEXT));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(if has_material {
                            "Press “Write summary” and the AI on this computer will write it."
                        } else {
                            "Press Record or type in “My notes”. When you stop recording,\nthe summary is written for you automatically."
                        })
                        .color(MUTED),
                    );
                });
            }
        });
    }

    fn transcript_tab(&mut self, ui: &mut egui::Ui, id: &str) {
        let recording_here = self.recording.as_ref().map(|r| r.id == id).unwrap_or(false);
        let mut copied = false;
        let can_play = !recording_here && self.recording.is_none() && self.has_audio(id);
        // Where playback is in this meeting, if it has been started or moved.
        let play_ms = self
            .player
            .as_ref()
            .filter(|p| p.meeting_id == id && (p.is_playing() || p.position_ms() > 0))
            .map(|p| (p.position_ms(), p.is_playing()));
        let mut jump: Option<i64> = None;
        let mut current_seg: Option<usize> = None;
        let last_seg = self.last_play_seg;
        let Some(m) = self.meetings.iter().find(|m| m.id == id) else { return };
        let count = m.transcript.len();
        ui.horizontal(|ui| {
            let words: usize = m.transcript.iter().map(|s| s.text.split_whitespace().count()).sum();
            ui.label(RichText::new(format!("{words} words")).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if count > 0 && ui.button("Copy").clicked() {
                    ui.ctx().copy_text(m.transcript_text());
                    copied = true;
                }
            });
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(recording_here)
            .show(ui, |ui| {
                if count == 0 && !recording_here {
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Nothing here yet").size(17.0).color(TEXT));
                        ui.add_space(4.0);
                        ui.label(RichText::new("Press Record. What people say appears here as they talk.").color(MUTED));
                    });
                    return;
                }
                for (i, s) in m.transcript.iter().enumerate() {
                    let next_start = m.transcript.get(i + 1).map(|x| x.start_ms).unwrap_or(i64::MAX);
                    let current = play_ms.map(|(ms, _)| ms >= s.start_ms && ms < next_start).unwrap_or(false);
                    if current {
                        current_seg = Some(i);
                    }
                    let row = Frame::new()
                        .fill(if current { ACCENT_SOFT } else { Color32::TRANSPARENT })
                        .corner_radius(8)
                        .inner_margin(Margin::symmetric(4, 3))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal_top(|ui| {
                                let time = RichText::new(fmt_clock(s.start_ms)).monospace().size(12.5);
                                if can_play {
                                    let r = ui.add_sized(
                                        [52.0, 20.0],
                                        egui::Label::new(time.color(ACCENT)).sense(Sense::click()),
                                    );
                                    if r.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if r.on_hover_text("Play from here").clicked() {
                                        jump = Some(s.start_ms);
                                    }
                                } else {
                                    ui.add_sized([52.0, 20.0], egui::Label::new(time.color(MUTED)));
                                }
                                ui.add(
                                    egui::Label::new(RichText::new(&s.text).size(15.0).color(if current { TEXT } else { BODY }))
                                        .wrap(),
                                );
                            });
                        })
                        .response;
                    // Keep the line being played in view.
                    if current && last_seg != Some(i) && play_ms.map(|(_, playing)| playing).unwrap_or(false) {
                        row.scroll_to_me(Some(Align::Center));
                    }
                    ui.add_space(2.0);
                }
                if recording_here {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(56.0);
                        ui.spinner();
                        ui.label(RichText::new("Listening… new text appears about every 12 seconds while people talk.").color(MUTED).italics());
                    });
                }
            });
        if copied {
            self.flash("Transcript copied");
        }
        self.last_play_seg = current_seg;
        if let Some(ms) = jump {
            let ctx = ui.ctx().clone();
            self.play_from(&ctx, id, Some(ms));
        }
    }

    /// Play / pause, the time and a slider to move through the meeting's recording.
    fn player_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, id: &str) {
        let here = self.player.as_ref().filter(|p| p.meeting_id == id);
        let playing = here.map(|p| p.is_playing()).unwrap_or(false);
        let total = here.map(|p| p.duration_ms()).unwrap_or_else(|| self.meeting(id).map(|m| m.duration_ms).unwrap_or(0));
        let pos = here.map(|p| p.position_ms()).unwrap_or(0);

        // round play / pause button
        let (r, resp) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
        let p = ui.painter();
        p.circle_filled(r.center(), 18.0, if resp.hovered() { ACCENT.gamma_multiply(0.85) } else { ACCENT });
        if playing {
            for dx in [-4.0, 4.0] {
                p.rect_filled(egui::Rect::from_center_size(r.center() + Vec2::new(dx, 0.0), Vec2::new(4.5, 14.0)), 1, Color32::WHITE);
            }
        } else {
            let c = r.center() + Vec2::new(2.0, 0.0);
            p.add(egui::Shape::convex_polygon(
                vec![c + Vec2::new(-6.0, -8.0), c + Vec2::new(-6.0, 8.0), c + Vec2::new(8.0, 0.0)],
                Color32::WHITE,
                Stroke::NONE,
            ));
        }
        let resp = resp.on_hover_text(if playing { "Pause" } else { "Play the recording" });
        if resp.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        ui.add_space(4.0);
        ui.label(RichText::new(format!("{} / {}", fmt_clock(pos), fmt_clock(total))).monospace().size(13.5).color(TEXT));
        ui.add_space(6.0);

        let mut secs = pos as f64 / 1000.0;
        ui.spacing_mut().slider_width = (ui.available_width() - 12.0).max(80.0);
        let slider = ui
            .scope(|ui| {
                ui.visuals_mut().selection.bg_fill = ACCENT; // played part of the track
                ui.add(
                    egui::Slider::new(&mut secs, 0.0..=(total.max(1) as f64 / 1000.0))
                        .show_value(false)
                        .trailing_fill(true),
                )
            })
            .inner;

        if resp.clicked() {
            if playing {
                if let Some(p) = &self.player {
                    p.pause();
                }
            } else {
                self.play_from(ctx, id, None);
            }
        }
        if slider.changed() && self.ensure_player(id) {
            if let Some(p) = &self.player {
                p.seek_ms((secs * 1000.0) as i64);
            }
        }
        if playing {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }

    fn notes_tab(&mut self, ui: &mut egui::Ui, id: &str) {
        ui.label(
            RichText::new("Your own notes. The AI reads them together with the transcript when it writes the summary.")
                .size(12.5)
                .color(MUTED),
        );
        ui.add_space(6.0);
        let Some(m) = self.meetings.iter_mut().find(|m| m.id == id) else { return };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let r = ui.add_sized(
                [ui.available_width(), ui.available_height().max(300.0)],
                egui::TextEdit::multiline(&mut m.notes)
                    .hint_text("Type notes here…")
                    .font(egui::FontId::proportional(15.5))
                    .margin(Margin::same(12)),
            );
            if r.changed() {
                self.dirty.insert(id.to_string());
            }
        });
    }

    fn ask_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, id: &str) {
        let busy = self.generating.as_ref().map(|g| g.id == id && g.kind == LlmKind::Chat).unwrap_or(false);
        let mut send: Option<String> = None;

        egui::Panel::bottom("ask_input")
            .frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(0, 10)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let w = ui.available_width() - 90.0;
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.ask_input)
                            .hint_text("Ask anything about this meeting…")
                            .desired_width(w)
                            .margin(Margin::symmetric(12, 9)),
                    );
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let b = egui::Button::new(RichText::new("Ask").color(Color32::WHITE)).fill(ACCENT).min_size(Vec2::new(80.0, 36.0));
                    let can = !busy && self.generating.is_none() && !self.ask_input.trim().is_empty();
                    if (ui.add_enabled(can, b).clicked() || (enter && can)) && can {
                        send = Some(self.ask_input.trim().to_string());
                    }
                });
            });

        if let Some((eid, e)) = &self.llm_error {
            if eid == id {
                ui.label(RichText::new(format!("{e}")).color(RED));
            }
        }

        let Some(m) = self.meeting(id) else { return };
        let chat = m.chat.clone();
        egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            if chat.is_empty() {
                ui.add_space(20.0);
                ui.label(RichText::new("Ask the AI about this meeting").size(17.0).color(TEXT));
                ui.label(RichText::new(format!("It only uses this meeting's transcript and notes, and it runs on {}.", crate::platform::THIS)).color(MUTED));
                ui.add_space(10.0);
                for q in ["What did we decide?", "What are my action items?", "Write a short follow-up email"] {
                    if ui.add(egui::Button::new(RichText::new(q).color(ACCENT)).fill(ACCENT_SOFT)).clicked() && self.generating.is_none() {
                        send = Some(q.to_string());
                    }
                }
            }
            for (i, c) in chat.iter().enumerate() {
                let mine = c.role == "user";
                let layout = if mine { Layout::right_to_left(Align::TOP) } else { Layout::left_to_right(Align::TOP) };
                ui.with_layout(layout, |ui| {
                    let maxw = ui.available_width() * 0.78;
                    Frame::new()
                        .fill(if mine { ACCENT } else { CARD })
                        .stroke(if mine { Stroke::NONE } else { Stroke::new(1.0, BORDER) })
                        .corner_radius(12)
                        .inner_margin(Margin::symmetric(12, 9))
                        .show(ui, |ui| {
                            ui.set_max_width(maxw);
                            if mine {
                                ui.add(egui::Label::new(RichText::new(&c.content).color(Color32::WHITE)).wrap());
                            } else {
                                let text = llm::strip_think(&c.content);
                                if text.trim().is_empty() && busy && i == chat.len() - 1 {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(RichText::new("Thinking…").color(MUTED));
                                    });
                                } else {
                                    markdown::show(ui, text.trim());
                                }
                            }
                        });
                });
                ui.add_space(8.0);
            }
        });

        if let Some(q) = send {
            self.ask_input.clear();
            if let Some(m) = self.meeting_mut(id) {
                m.chat.push(ChatMsg { role: "user".into(), content: q });
            }
            self.touch(id);
            self.start_llm(ctx, id, LlmKind::Chat);
        }
    }

    // ---------- windows ----------

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut open = true;
        let mut recheck = false;
        let mut download = false;
        let mut changed = false;
        let mut test_mic: Option<bool> = None;
        egui::Window::new("Settings")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(Frame::new().fill(BG).stroke(Stroke::new(1.0, BORDER)).corner_radius(14).inner_margin(20).shadow(ctx.global_style().visuals.window_shadow))
            .show(ctx, |ui| {
                ui.set_width(440.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Settings").size(19.0).strong().color(TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Done").clicked() {
                            open = false;
                        }
                    });
                });
                ui.add_space(10.0);
                ui.label(RichText::new(format!("AI model (Ollama on {})", crate::platform::THIS)).strong().color(TEXT));
                ui.add_space(2.0);
                match &self.ollama {
                    Ollama::Ready if self.models.is_empty() => {
                        ui.label(RichText::new("Ollama is running but has no models. In Terminal, run:  ollama pull qwen3:8b").color(AMBER));
                    }
                    Ollama::Ready => {
                        let mut model = self.settings.model.clone();
                        egui::ComboBox::from_id_salt("model")
                            .selected_text(&model)
                            .width(300.0)
                            .show_ui(ui, |ui| {
                                for m in &self.models {
                                    ui.selectable_value(&mut model, m.clone(), m);
                                }
                            });
                        if model != self.settings.model {
                            self.settings.model = model;
                            changed = true;
                        }
                        ui.label(RichText::new("Smaller models are faster; larger ones write better notes.").size(12.0).color(MUTED));
                    }
                    Ollama::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Connecting to Ollama…");
                        });
                    }
                    Ollama::Offline(e) => {
                        ui.label(RichText::new(format!("{e}")).color(RED).size(12.5));
                        ui.label(RichText::new("Open the Ollama app, then press “Check again”.").color(MUTED).size(12.5));
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Address").color(MUTED));
                    if ui.add(egui::TextEdit::singleline(&mut self.settings.ollama_url).desired_width(220.0)).changed() {
                        changed = true;
                    }
                    if ui.button("Check again").clicked() {
                        recheck = true;
                        if matches!(self.ollama, Ollama::Offline(_)) {
                            launch_ollama();
                        }
                    }
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(RichText::new("Microphone").strong().color(TEXT));
                let mut mic = self.settings.mic.clone();
                let default_label = format!("System default ({})", self.mic_default.clone().unwrap_or_else(|| "none".into()));
                let shown = if mic.is_empty() || !self.mic_names.contains(&mic) { default_label.clone() } else { mic.clone() };
                egui::ComboBox::from_id_salt("mic").selected_text(shown).width(300.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut mic, String::new(), default_label.clone());
                    for n in &self.mic_names {
                        ui.selectable_value(&mut mic, n.clone(), n);
                    }
                });
                if mic != self.settings.mic {
                    self.settings.mic = mic;
                    changed = true;
                    test_mic = Some(false);
                }
                if let Some(t) = &self.mic_test {
                    ui.horizontal(|ui| sound_wave(ui, t, false));
                    if !t.heard_anything() && t.started.elapsed().as_secs_f32() > 2.0 {
                        status_chip(ui, RED, &format!("No sound: check the microphone in {}", crate::platform::MIC_SETTINGS));
                    }
                    ctx.request_repaint_after(Duration::from_millis(50));
                }
                ui.horizontal(|ui| {
                    if self.mic_test.is_some() {
                        if ui.button("Stop test").clicked() {
                            test_mic = Some(false);
                        }
                    } else if self.recording.is_none() {
                        if ui.button("Test microphone").clicked() {
                            test_mic = Some(true);
                        }
                        ui.label(RichText::new("Speak and watch the bars move.").size(12.0).color(MUTED));
                    }
                });
                if self.mic_perm == crate::mic::Permission::Denied {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} is blocking the microphone for this app.", crate::platform::OS)).size(12.5).color(RED));
                        if crate::platform::CAN_OPEN_MIC_SETTINGS && ui.small_button("Open microphone settings").clicked() {
                            crate::platform::open_mic_settings();
                        }
                    });
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(RichText::new(format!("Speech to text (on {})", crate::platform::THIS)).strong().color(TEXT));
                let mut wm = self.settings.whisper_model.clone();
                egui::ComboBox::from_id_salt("whisper")
                    .selected_text(transcribe::model_label(&wm))
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        for m in transcribe::MODELS {
                            ui.selectable_value(&mut wm, m.id.to_string(), format!("{}  ({} MB)", m.label, m.size_mb));
                        }
                    });
                if wm != self.settings.whisper_model && self.downloading.is_none() {
                    self.settings.whisper_model = wm;
                    changed = true;
                }
                if !self.settings.whisper_model.ends_with(".en") {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Language").color(MUTED));
                        let mut lang = self.settings.language.clone();
                        egui::ComboBox::from_id_salt("lang").selected_text(lang_name(&lang)).show_ui(ui, |ui| {
                            for (code, name) in LANGS {
                                ui.selectable_value(&mut lang, code.to_string(), *name);
                            }
                        });
                        if lang != self.settings.language {
                            self.settings.language = lang;
                            changed = true;
                        }
                    });
                }
                let have = self.whisper_path().exists();
                ui.horizontal(|ui| {
                    if let Some(p) = self.downloading {
                        ui.add(egui::ProgressBar::new(p).desired_width(220.0).show_percentage());
                    } else if have {
                        ui.label(RichText::new("Downloaded and ready").color(GREEN));
                    } else {
                        ui.label(RichText::new("Not downloaded yet (needed once)").color(AMBER));
                        if ui.button("Download now").clicked() {
                            download = true;
                        }
                    }
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(RichText::new("Privacy").strong().color(TEXT));
                ui.label(
                    RichText::new(format!("Recordings, transcripts and notes stay on {}. Nothing is sent to the internet — the only download is the speech model.", crate::platform::THIS))
                        .size(12.5)
                        .color(BODY),
                );
                if ui.button("Show my meeting files").clicked() {
                    crate::platform::open_path(&self.store.root);
                }
            });
        if changed {
            self.store.save_settings(&self.settings);
        }
        if recheck {
            self.check_ollama(ctx);
        }
        if download {
            self.start_download(ctx);
        }
        match test_mic {
            Some(true) => {
                self.refresh_mics();
                self.ask_mic_permission(ctx);
                match Recorder::start(ctx.clone(), &self.settings.mic) {
                    Ok(r) => self.mic_test = Some(r),
                    Err(e) => self.mic_error = Some(e),
                }
            }
            Some(false) => self.mic_test = None,
            None => {}
        }
        if !open {
            self.show_settings = false;
            self.mic_test = None;
        }
    }

    fn delete_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.confirm_delete.clone() else { return };
        let title = self.meeting(&id).map(|m| m.title.clone()).unwrap_or_default();
        let mut close = false;
        let mut delete = false;
        let resp = egui::Modal::new(egui::Id::unique("delete")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.label(RichText::new("Delete this meeting?").size(17.0).strong().color(TEXT));
            ui.add_space(4.0);
            ui.label(RichText::new(format!("“{title}” and its recording will be removed. This can't be undone.")).color(BODY));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("Delete").color(Color32::WHITE)).fill(RED)).clicked() {
                    delete = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if resp.should_close() {
            close = true;
        }
        if delete {
            if self.recording.as_ref().map(|r| r.id == id).unwrap_or(false) {
                if let Some(mut r) = self.recording.take() {
                    r.rec.stop();
                }
            }
            if let Some(pos) = self.meetings.iter().position(|m| m.id == id) {
                if self.player.as_ref().map(|p| p.meeting_id == id).unwrap_or(false) {
                    self.player = None;
                }
                let m = self.meetings.remove(pos);
                self.store.delete_meeting(&m);
            }
            self.dirty.remove(&id);
            self.selected = self.meetings.first().map(|m| m.id.clone());
            close = true;
        }
        if close {
            self.confirm_delete = None;
        }
    }

    fn toast(&mut self, ctx: &egui::Context) {
        if let Some((msg, t)) = &self.toast {
            if t.elapsed() > Duration::from_millis(2200) {
                self.toast = None;
                return;
            }
            egui::Area::new(egui::Id::unique("toast"))
                .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -24.0))
                .show(ctx, |ui| {
                    Frame::new().fill(TEXT).corner_radius(10).inner_margin(Margin::symmetric(16, 10)).show(ui, |ui| {
                        ui.label(RichText::new(msg).color(Color32::WHITE));
                    });
                });
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some(plan) = self.shot.as_mut() else { return };
        ctx.request_repaint();
        self.frame_no += 1;
        if plan.done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.frame_no >= plan.frames && !plan.requested {
            plan.requested = true;
            let path = plan.path.clone();
            let done = plan.done.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                let [w, h] = img.size;
                let mut bytes = Vec::with_capacity(w * h * 4);
                for p in &img.pixels {
                    bytes.extend_from_slice(&p.to_array());
                }
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = image::save_buffer(&path, &bytes, w as u32, h as u32, image::ExtendedColorType::Rgba8) {
                    eprintln!("could not save screenshot: {e}");
                }
                done.store(true, Ordering::SeqCst);
            })));
        }
        if self.frame_no > plan.frames + 300 {
            std::process::exit(1);
        }
    }
}

const LANGS: &[(&str, &str)] = &[
    ("auto", "Detect automatically"),
    ("en", "English"),
    ("zh", "Chinese"),
    ("ms", "Malay"),
    ("ta", "Tamil"),
    ("id", "Indonesian"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("hi", "Hindi"),
];

fn lang_name(code: &str) -> &'static str {
    LANGS.iter().find(|(c, _)| *c == code).map(|(_, n)| *n).unwrap_or("Detect automatically")
}

use crate::platform::launch_ollama;

fn status_chip(ui: &mut egui::Ui, color: Color32, text: &str) {
    Frame::new()
        .fill(color.gamma_multiply(0.12))
        .corner_radius(20)
        .inner_margin(Margin::symmetric(10, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (d, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter().circle_filled(d.center(), 3.5, color);
                ui.label(RichText::new(text).size(12.5).color(color));
            });
        });
}

/// Map loudness (RMS) to 0..1 on a decibel scale, which matches how loud it sounds.
fn loudness(rms: f32) -> f32 {
    let db = 20.0 * rms.max(1e-6).log10();
    ((db + 55.0) / 47.0).clamp(0.0, 1.0)
}

/// A live, scrolling sound wave of the last few seconds, coloured by strength.
fn sound_wave(ui: &mut egui::Ui, rec: &Recorder, paused: bool) {
    const BARS: usize = 64;
    let levels = if paused { vec![0.0; BARS] } else { rec.levels(6.0, BARS) };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(BARS as f32 * 5.0 + 16.0, 40.0), Sense::hover());
    let p = ui.painter();
    let inner = rect.shrink2(Vec2::new(8.0, 5.0));
    let mid = inner.center().y;
    let quiet_c = Color32::from_rgb(0xC9, 0xCB, 0xD3);
    for (i, &rms) in levels.iter().enumerate() {
        let v = loudness(rms);
        let h = (v * inner.height()).max(2.0);
        let x = inner.left() + i as f32 * 5.0 + 1.5;
        let c = if v < 0.25 {
            quiet_c
        } else if v < 0.88 {
            GREEN
        } else {
            RED
        };
        // the newest bars are fully opaque, older ones fade a little
        let fade = 0.45 + 0.55 * (i as f32 / BARS as f32);
        p.rect_filled(
            egui::Rect::from_center_size(egui::pos2(x, mid), Vec2::new(3.0, h)),
            2,
            c.gamma_multiply(fade),
        );
    }
}


impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_messages(&ctx);

        // Menus and keyboard shortcuts
        let st = self.menu_state();
        #[cfg_attr(target_os = "macos", allow(unused_mut))]
        let mut actions: Vec<crate::menu::Action> = self.menu_rx.try_iter().collect();
        #[cfg(target_os = "macos")]
        if let Some(nm) = self.native_menu.as_mut() {
            nm.update(&st);
        }
        #[cfg(not(target_os = "macos"))]
        {
            actions.extend(crate::menu::shortcuts(&ctx, &st));
            egui::Panel::top("menubar")
                .frame(Frame::new().fill(SIDEBAR).inner_margin(Margin::symmetric(8, 3)).stroke(Stroke::new(1.0, BORDER)))
                .show(ui, |ui| {
                    if let Some(a) = crate::menu::window_menu_bar(ui, &st) {
                        actions.push(a);
                    }
                });
        }
        for a in actions {
            self.run_action(&ctx, a);
        }

        if let Some(r) = &self.recording {
            if let Some(e) = r.rec.failed.lock().ok().and_then(|f| f.clone()) {
                self.mic_error = Some(e);
                self.stop_recording();
            }
        }
        if self.recording.is_some() {
            self.send_chunk(false);
            if let Some(r) = &self.recording {
                let ms = r.base_ms + r.rec.len() as i64 * 1000 / RATE as i64;
                let id = r.id.clone();
                if let Some(m) = self.meeting_mut(&id) {
                    m.duration_ms = ms;
                }
            }
        }

        egui::Panel::left("sidebar")
            .resizable(false)
            .exact_size(264.0)
            .frame(Frame::new().fill(SIDEBAR).inner_margin(Margin::symmetric(14, 14)).stroke(Stroke::new(1.0, BORDER)))
            .show(ui, |ui| self.sidebar(ui));
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(Margin { left: 32, right: 32, top: 22, bottom: 12 }))
            .show(ui, |ui| self.main_area(ui));

        self.settings_window(&ctx);
        self.guide_window(&ctx);
        self.delete_dialog(&ctx);
        self.toast(&ctx);
        self.save_dirty(false);
        if !self.dirty.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(900));
        }
        self.screenshot_tick(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.recording.is_some() {
            self.stop_recording();
        }
        self.save_dirty(true);
    }
}

/// The user guide (docs/user-guide.md) tidied for the in-app viewer: no pictures, tables as lines,
/// links as plain words.
fn guide_text() -> String {
    let raw = include_str!("../docs/user-guide.md");
    let mut lines: Vec<String> = Vec::new();
    // whether the last line is ordinary text that a wrapped next line should continue
    let mut joinable = false;
    let mut in_code = false;
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_code = !in_code;
            joinable = false;
            continue;
        }
        if in_code {
            if !t.is_empty() {
                lines.push(format!("    `{}`", t));
            }
            continue;
        }
        if t.starts_with("<img") || t == "---" || t.starts_with("|---") || t.starts_with("| ---") {
            joinable = false;
            continue;
        }
        // skip the table of contents at the top
        if t.starts_with("- [") && t.contains("](#") {
            continue;
        }
        let mut l = t.to_string();
        if t.starts_with('|') {
            let cells: Vec<&str> = t.trim_matches('|').split('|').map(|c| c.trim()).collect();
            l = format!("- {}", cells.join(" — "));
        }
        // [text](link) -> text
        while let (Some(a), Some(b)) = (l.find("]("), l.find('[')) {
            if b > a {
                break;
            }
            let Some(close) = l[a..].find(')') else { break };
            let text = l[b + 1..a].to_string();
            l.replace_range(b..a + close + 1, &text);
        }
        l = l.trim_start_matches("> ").replace("<br>", " ");
        if l.starts_with('#') {
            l = l.replace('`', "");
        }
        let starts_block = l.is_empty()
            || l.starts_with('#')
            || l.starts_with("- ")
            || l.starts_with("* ")
            || l.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && l.contains(". ");
        if !starts_block && joinable {
            // a wrapped line: continue the previous paragraph or list item
            if let Some(last) = lines.last_mut() {
                last.push(' ');
                last.push_str(&l);
                continue;
            }
        }
        joinable = !l.is_empty() && !l.starts_with('#');
        lines.push(l);
    }
    lines.join("\n")
}
