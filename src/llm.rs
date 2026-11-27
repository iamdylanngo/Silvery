//! Talking to the local LLM through Ollama's HTTP API.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::time::Duration;

pub struct Template {
    pub name: &'static str,
    pub instructions: &'static str,
}

pub const TEMPLATES: &[Template] = &[
    Template {
        name: "General meeting",
        instructions: "Write these sections:\n## Overview\n(2-3 sentences)\n## Key points\n(bullets)\n## Decisions\n(bullets, or 'None recorded')\n## Action items\n(checkbox bullets like '- [ ] Task — *Owner*', or 'None recorded')",
    },
    Template {
        name: "One-on-one",
        instructions: "Write these sections:\n## Overview\n## Updates shared\n(bullets)\n## Challenges & support needed\n(bullets)\n## Feedback\n(bullets)\n## Next steps\n(checkbox bullets like '- [ ] Task — *Owner*')",
    },
    Template {
        name: "Stand-up",
        instructions: "Write these sections:\n## Done since last time\n(bullets, grouped by person when known)\n## Today\n(bullets)\n## Blockers\n(bullets, or 'None')\n## Follow-ups\n(checkbox bullets like '- [ ] Task — *Owner*')",
    },
    Template {
        name: "Customer call",
        instructions: "Write these sections:\n## Overview\n## Customer needs & pain points\n(bullets)\n## Questions & objections\n(bullets)\n## Agreed next steps\n(checkbox bullets like '- [ ] Task — *Owner*')",
    },
    Template {
        name: "Interview",
        instructions: "Write these sections:\n## Overview\n## Background of the person\n(bullets)\n## Strengths\n(bullets)\n## Concerns\n(bullets)\n## Notable quotes\n(bullets with short quotes)\n## Next steps\n(checkbox bullets)",
    },
];

fn agent(timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(timeout)
            .build(),
    )
}

/// The models Ollama has installed, excluding embedding-only ones.
pub fn list_models(base: &str) -> Result<Vec<String>, String> {
    let url = format!("{}/api/tags", base.trim_end_matches('/'));
    let v: Value = agent(Some(Duration::from_secs(4)))
        .get(&url)
        .call()
        .map_err(|_| format!("Ollama isn't running at {base}."))?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    let mut names: Vec<String> = v["models"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .filter(|n| !n.contains("embed"))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    Ok(names)
}

/// Pick a sensible default from what is installed.
pub fn pick_default(models: &[String]) -> Option<String> {
    let prefs = ["qwen3.5", "qwen3:8b", "qwen3", "gpt-oss", "llama3", "mistral", "gemma"];
    for p in prefs {
        if let Some(m) = models.iter().find(|m| m.starts_with(p)) {
            return Some(m.clone());
        }
    }
    models.first().cloned()
}

/// Stream a chat completion. `on_delta` gets each piece of visible text.
pub fn chat_stream(
    base: &str,
    model: &str,
    messages: &[Value],
    mut on_delta: impl FnMut(&str) -> bool,
) -> Result<(), String> {
    let url = format!("{}/api/chat", base.trim_end_matches('/'));
    let lower = model.to_lowercase();
    // Reasoning models can skip their "thinking" phase, which makes notes much faster.
    let can_skip_thinking = lower.starts_with("qwen3") || lower.starts_with("deepseek-r1");
    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "options": { "temperature": 0.3, "num_ctx": 16384 },
    });
    if can_skip_thinking {
        body["think"] = json!(false);
    }
    let a = agent(None);
    let resp = match a.post(&url).send_json(&body) {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(_)) if can_skip_thinking => {
            body.as_object_mut().unwrap().remove("think");
            a.post(&url)
                .send_json(&body)
                .map_err(|e| format!("The AI model returned an error: {e}"))?
        }
        Err(ureq::Error::StatusCode(404)) => {
            return Err(format!("The model \"{model}\" isn't installed in Ollama. Pick another one in Settings."))
        }
        Err(e) => return Err(format!("Can't reach Ollama at {base}. Is it running? ({e})")),
    };
    let reader = BufReader::new(resp.into_body().into_reader());
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(err) = v["error"].as_str() {
            return Err(err.to_string());
        }
        if let Some(t) = v["message"]["content"].as_str() {
            if !t.is_empty() && !on_delta(t) {
                break;
            }
        }
        if v["done"].as_bool() == Some(true) {
            break;
        }
    }
    Ok(())
}

/// Remove <think>…</think> blocks (also an unfinished one at the end).
pub fn strip_think(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        match rest.find("<think>") {
            Some(i) => {
                out.push_str(&rest[..i]);
                match rest[i..].find("</think>") {
                    Some(j) => rest = &rest[i + j + "</think>".len()..],
                    None => return out,
                }
            }
            None => {
                out.push_str(rest);
                return out;
            }
        }
    }
}

fn meeting_context(m: &crate::model::Meeting) -> String {
    let mut ctx = String::new();
    if !m.notes.trim().is_empty() {
        ctx.push_str("MY NOTES:\n");
        ctx.push_str(m.notes.trim());
        ctx.push_str("\n\n");
    }
    if !m.transcript.is_empty() {
        ctx.push_str("TRANSCRIPT:\n");
        ctx.push_str(&m.transcript_text());
    }
    ctx
}

/// Which language the AI should write a meeting's notes in: the language chosen in Settings, or else
/// the language spoken most in the meeting. None means English or unknown (headings stay as given).
pub fn notes_language_for(notes_setting: &str, meeting: &crate::model::Meeting) -> Option<String> {
    if notes_setting != "auto" {
        return Some(crate::transcribe::language_name(notes_setting));
    }
    match meeting.main_language() {
        Some(code) if code != "en" => Some(crate::transcribe::language_name(code)),
        _ => None,
    }
}

/// The instruction about which language the AI writes in.
/// `notes_language` is a language name like "Vietnamese", or None for "same as the meeting".
fn language_rule(notes_language: Option<&str>) -> String {
    match notes_language {
        Some(lang) => format!(
            "LANGUAGE: write the whole answer in {lang}, even if the meeting was in another language. \
             This includes the title and every section heading: translate the English heading names \
             above into {lang} (for example '## Overview' becomes the {lang} word for Overview). \
             Keep people's names unchanged."
        ),
        None => "LANGUAGE: use the same language as the TRANSCRIPT below (if it mixes languages, the one \
                 spoken most). If the transcript is in English, write in English and use the headings \
                 exactly as given. If it is in another language, write the whole answer in that language, \
                 including the title, and translate the heading names into it. Keep people's names unchanged."
            .to_string(),
    }
}

pub fn summary_messages(m: &crate::model::Meeting, template: usize, notes_language: Option<&str>) -> Vec<Value> {
    let t = &TEMPLATES[template.min(TEMPLATES.len() - 1)];
    let system = format!(
        "You are a meeting assistant. You write clear, short, accurate meeting notes in Markdown \
         from a transcript and the user's own notes. Only use facts that are in the material; \
         never invent names, numbers or dates.\n\n\
         Start with one line '# <short meeting title, max 6 words>'. Then {}\n\n{}",
        t.instructions,
        language_rule(notes_language),
    );
    vec![
        json!({"role": "system", "content": system}),
        json!({"role": "user", "content": format!("Meeting date: {}\n\n{}", m.created.format("%A %e %B %Y"), meeting_context(m))}),
    ]
}

pub fn ask_messages(m: &crate::model::Meeting, notes_language: Option<&str>) -> Vec<Value> {
    let lang = match notes_language {
        Some(l) => format!("Always answer in {l}."),
        None => "Answer in the language the question is asked in.".to_string(),
    };
    let mut msgs = vec![json!({"role": "system", "content": format!(
        "You answer questions about one meeting. Use only the material below. If the answer isn't there, say so. Keep answers short. {lang}\n\n{}{}",
        meeting_context(m),
        if m.summary.trim().is_empty() { String::new() } else { format!("\nSUMMARY:\n{}", strip_think(&m.summary)) }
    )})];
    for c in &m.chat {
        msgs.push(json!({"role": c.role, "content": strip_think(&c.content)}));
    }
    msgs
}
