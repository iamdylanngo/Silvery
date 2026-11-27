//! Meetings, settings and where they are kept on disk.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub created: DateTime<Local>,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub transcript: Vec<Segment>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub template: usize,
    #[serde(default)]
    pub chat: Vec<ChatMsg>,
    #[serde(default)]
    pub recordings: Vec<String>,
    /// Words transcribed per spoken language (e.g. {"vi": 120, "en": 8}).
    #[serde(default)]
    pub spoken: std::collections::BTreeMap<String, u32>,
}

pub const UNTITLED: &str = "Untitled meeting";

impl Meeting {
    pub fn new() -> Self {
        let now = Local::now();
        Self {
            id: format!("{}", now.format("%Y%m%d-%H%M%S-%3f")),
            title: UNTITLED.to_string(),
            created: now,
            duration_ms: 0,
            notes: String::new(),
            transcript: Vec::new(),
            summary: String::new(),
            template: 0,
            chat: Vec::new(),
            recordings: Vec::new(),
            spoken: Default::default(),
        }
    }

    /// The language spoken most in this meeting (a code like "vi"), if known.
    pub fn main_language(&self) -> Option<&str> {
        self.spoken.iter().max_by_key(|(_, n)| **n).map(|(c, _)| c.as_str())
    }

    pub fn transcript_text(&self) -> String {
        let mut out = String::new();
        for s in &self.transcript {
            out.push_str(&format!("[{}] {}\n", fmt_clock(s.start_ms), s.text.trim()));
        }
        out
    }

    pub fn to_markdown(&self) -> String {
        let mut md = format!(
            "# {}\n_{}_\n\n",
            self.title,
            self.created.format("%A %e %B %Y, %H:%M")
        );
        if !self.summary.trim().is_empty() {
            md.push_str("## Summary\n\n");
            md.push_str(crate::llm::strip_think(&self.summary).trim());
            md.push_str("\n\n");
        }
        if !self.notes.trim().is_empty() {
            md.push_str("## My notes\n\n");
            md.push_str(self.notes.trim());
            md.push_str("\n\n");
        }
        if !self.transcript.is_empty() {
            md.push_str("## Transcript\n\n");
            for s in &self.transcript {
                md.push_str(&format!("**{}** {}\n\n", fmt_clock(s.start_ms), s.text.trim()));
            }
        }
        md
    }
}

pub fn fmt_clock(ms: i64) -> String {
    let s = (ms / 1000).max(0);
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub ollama_url: String,
    pub model: String,
    pub whisper_model: String,
    pub language: String,
    #[serde(default)]
    pub default_template: usize,
    #[serde(default)]
    pub seen_sample: bool,
    /// Chosen microphone; empty = system default.
    #[serde(default)]
    pub mic: String,
    /// Language the AI writes summaries and answers in ("auto" = same as the meeting).
    #[serde(default = "auto_lang")]
    pub notes_language: String,
}

fn auto_lang() -> String {
    "auto".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ollama_url: "http://localhost:11434".into(),
            model: String::new(),
            whisper_model: "base.en".into(),
            language: "auto".into(),
            default_template: 0,
            seen_sample: false,
            mic: String::new(),
            notes_language: auto_lang(),
        }
    }
}

pub struct Store {
    pub root: PathBuf,
    pub readonly: bool,
}

impl Store {
    pub fn new(readonly: bool) -> Self {
        // SILVERY_DATA_DIR points Silvery at another folder (e.g. for demos or tests).
        let root = std::env::var_os("SILVERY_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(crate::APP_NAME)
            });
        if !readonly {
            let _ = std::fs::create_dir_all(root.join("meetings"));
            let _ = std::fs::create_dir_all(root.join("recordings"));
            let _ = std::fs::create_dir_all(root.join("models"));
        }
        Self { root, readonly }
    }

    pub fn meetings_dir(&self) -> PathBuf {
        self.root.join("meetings")
    }
    pub fn recordings_dir(&self) -> PathBuf {
        self.root.join("recordings")
    }
    pub fn models_dir(&self) -> PathBuf {
        self.root.join("models")
    }

    pub fn load_meetings(&self) -> Vec<Meeting> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(self.meetings_dir()) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) != Some("json") {
                    continue;
                }
                if let Ok(s) = std::fs::read_to_string(&p) {
                    if let Ok(m) = serde_json::from_str::<Meeting>(&s) {
                        out.push(m);
                    }
                }
            }
        }
        out.sort_by(|a, b| b.created.cmp(&a.created));
        out
    }

    pub fn save_meeting(&self, m: &Meeting) {
        if self.readonly {
            return;
        }
        let p = self.meetings_dir().join(format!("{}.json", m.id));
        if let Ok(s) = serde_json::to_string_pretty(m) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }

    pub fn delete_meeting(&self, m: &Meeting) {
        if self.readonly {
            return;
        }
        let _ = std::fs::remove_file(self.meetings_dir().join(format!("{}.json", m.id)));
        for r in &m.recordings {
            let _ = std::fs::remove_file(self.recordings_dir().join(r));
        }
    }

    pub fn load_settings(&self) -> Settings {
        std::fs::read_to_string(self.root.join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, s: &Settings) {
        if self.readonly {
            return;
        }
        if let Ok(j) = serde_json::to_string_pretty(s) {
            let _ = std::fs::write(self.root.join("settings.json"), j);
        }
    }

    pub fn whisper_path(&self, name: &str) -> PathBuf {
        self.models_dir().join(format!("ggml-{name}.bin"))
    }
}

/// A sample meeting shown the first time, so the app doesn't open empty.
pub fn sample_meeting() -> Meeting {
    let mut m = Meeting::new();
    m.id = "sample-welcome".into();
    m.title = "Weekly product sync (example)".into();
    m.created = Local::now() - chrono::Duration::minutes(95);
    m.duration_ms = 14 * 60_000 + 32_000;
    m.notes = "- Launch date still 9 Dec?\n- Ask Priya about the onboarding copy\n- Budget for user interviews".into();
    let lines = [
        (0, "Okay, let's get started. Main thing today is whether we're still on track for the ninth."),
        (9_000, "Engineering is on track. The sync bug is fixed, we're just waiting on the final review."),
        (21_000, "Design has the new onboarding screens ready, but the copy still needs a pass."),
        (34_000, "Priya said she can do the copy by Thursday if we send her the screens today."),
        (47_000, "Good. Then the other open question is the budget for user interviews next month."),
        (61_000, "I'd suggest eight interviews, roughly forty minutes each, and we record them all."),
        (78_000, "Agreed. Sam, can you draft the interview plan and share it by Monday?"),
        (86_000, "Sure, I'll have it ready by Monday morning."),
        (95_000, "Great, so we keep the ninth of December as launch date. Thanks everyone."),
    ];
    m.transcript = lines
        .iter()
        .enumerate()
        .map(|(i, (t, s))| Segment {
            start_ms: *t,
            end_ms: lines.get(i + 1).map(|n| n.0).unwrap_or(t + 8000),
            text: s.to_string(),
        })
        .collect();
    m.summary = "## Overview\nThe team confirmed the product launch stays on **9 December**. Engineering is on track, onboarding design is ready and only the copy is outstanding.\n\n## Key points\n- The sync bug is fixed; the release is waiting on a final review.\n- New onboarding screens are done; copy needs one more pass.\n- Plan for **8 user interviews** of about 40 minutes next month.\n\n## Decisions\n- Launch date stays on 9 December.\n- All user interviews will be recorded.\n\n## Action items\n- [ ] Send the onboarding screens to Priya today — *Design*\n- [ ] Finish onboarding copy by Thursday — *Priya*\n- [ ] Draft and share the interview plan by Monday — *Sam*\n".into();
    m.chat = vec![
        ChatMsg {
            role: "user".into(),
            content: "Who is doing the interview plan?".into(),
        },
        ChatMsg {
            role: "assistant".into(),
            content: "Sam is drafting the user-interview plan and will share it by Monday morning.".into(),
        },
    ];
    m
}
