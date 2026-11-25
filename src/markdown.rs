//! A small Markdown viewer for AI-written notes (headings, bullets, checkboxes, bold, italics).

use crate::theme;
use egui::text::LayoutJob;
use egui::{FontId, RichText, TextFormat};

/// Draw `md`. Returns the line number of a checkbox the person clicked, if any.
pub fn show(ui: &mut egui::Ui, md: &str) -> Option<usize> {
    let mut toggled = None;
    let mut first = true;
    for (idx, raw) in md.lines().enumerate() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        let indent = (line.len() - trimmed.len()) as f32 * 6.0;
        if trimmed.is_empty() {
            ui.add_space(6.0);
            continue;
        }
        if let Some(h) = trimmed.strip_prefix("# ") {
            ui.label(RichText::new(h).size(22.0).strong().color(theme::TEXT));
            ui.add_space(4.0);
        } else if let Some(h) = trimmed.strip_prefix("## ") {
            if !first {
                ui.add_space(10.0);
            }
            ui.label(RichText::new(h).size(17.0).strong().color(theme::TEXT));
            ui.add_space(2.0);
        } else if let Some(h) = trimmed.strip_prefix("### ") {
            ui.add_space(6.0);
            ui.label(RichText::new(h).size(15.0).strong().color(theme::TEXT));
        } else if let Some((checked, rest)) = checkbox(trimmed) {
            ui.horizontal_top(|ui| {
                ui.add_space(indent);
                let (r, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                let b = egui::Rect::from_center_size(r.center() + egui::vec2(0.0, 1.0), egui::vec2(15.0, 15.0));
                let hover = resp.hovered();
                if checked {
                    ui.painter().rect_filled(b, 4, theme::ACCENT);
                    let p = ui.painter();
                    let st = egui::Stroke::new(2.0, egui::Color32::WHITE);
                    p.line_segment([b.left_center() + egui::vec2(3.5, 0.0), b.center_bottom() + egui::vec2(-1.0, -4.0)], st);
                    p.line_segment([b.center_bottom() + egui::vec2(-1.0, -4.0), b.right_top() + egui::vec2(-3.5, 4.0)], st);
                } else {
                    ui.painter().rect_stroke(b, 4, egui::Stroke::new(1.5, if hover { theme::ACCENT } else { theme::MUTED }), egui::StrokeKind::Inside);
                }
                if resp.clicked() {
                    toggled = Some(idx);
                }
                let mut job = inline(rest, 15.0);
                if checked {
                    for s in job.sections.iter_mut() {
                        s.format.strikethrough = egui::Stroke::new(1.0, theme::MUTED);
                        s.format.color = theme::MUTED;
                    }
                }
                ui.add(egui::Label::new(job).wrap());
            });
        } else if let Some(rest) = bullet(trimmed) {
            ui.horizontal_top(|ui| {
                ui.add_space(indent);
                let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center() + egui::vec2(0.0, 1.0), 2.8, theme::ACCENT);
                ui.add(egui::Label::new(inline(rest, 15.0)).wrap());
            });
        } else {
            ui.horizontal_top(|ui| {
                ui.add_space(indent);
                ui.add(egui::Label::new(inline(trimmed, 15.0)).wrap());
            });
        }
        first = false;
    }
    toggled
}

fn checkbox(s: &str) -> Option<(bool, &str)> {
    let b = bullet(s)?;
    if let Some(r) = b.strip_prefix("[ ] ") {
        Some((false, r))
    } else if let Some(r) = b.strip_prefix("[x] ").or_else(|| b.strip_prefix("[X] ")) {
        Some((true, r))
    } else {
        None
    }
}

fn bullet(s: &str) -> Option<&str> {
    if let Some(r) = s.strip_prefix("- ").or_else(|| s.strip_prefix("* ")).or_else(|| s.strip_prefix("• ")) {
        return Some(r);
    }
    // numbered: "1. "
    let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && s[digits..].starts_with(". ") {
        return Some(&s[digits + 2..]);
    }
    None
}

/// Flip the checkbox on line `idx`.
pub fn toggle_line(md: &str, idx: usize) -> String {
    let mut out: Vec<String> = Vec::new();
    for (i, l) in md.lines().enumerate() {
        if i == idx {
            if l.contains("[ ]") {
                out.push(l.replacen("[ ]", "[x]", 1));
            } else {
                out.push(l.replacen("[x]", "[ ]", 1).replacen("[X]", "[ ]", 1));
            }
        } else {
            out.push(l.to_string());
        }
    }
    let mut s = out.join("\n");
    if md.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// Bold (**x**), italics (*x* / _x_) and `code`.
fn inline(s: &str, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    let mut bold = false;
    let mut italic = false;
    let mut code = false;
    let mut buf = String::new();
    let flush = |job: &mut LayoutJob, buf: &mut String, bold: bool, italic: bool, code: bool| {
        if buf.is_empty() {
            return;
        }
        let fmt = TextFormat {
            font_id: if code { FontId::monospace(size - 1.0) } else { FontId::proportional(size) },
            color: if bold { theme::TEXT } else if code { theme::ACCENT } else { theme::BODY },
            italics: italic,
            background: if code { theme::CODE_BG } else { egui::Color32::TRANSPARENT },
            ..Default::default()
        };
        job.append(buf, 0.0, fmt);
        buf.clear();
    };
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            flush(&mut job, &mut buf, bold, italic, code);
            code = !code;
        } else if !code && c == '*' && chars.get(i + 1) == Some(&'*') {
            flush(&mut job, &mut buf, bold, italic, code);
            bold = !bold;
            i += 1;
        } else if !code
            && (c == '*' || c == '_')
            && (italic || chars.get(i + 1).map(|n| !n.is_whitespace()).unwrap_or(false))
            && (c == '*' || i == 0 || !chars[i - 1].is_alphanumeric())
        {
            flush(&mut job, &mut buf, bold, italic, code);
            italic = !italic;
        } else {
            buf.push(c);
        }
        i += 1;
    }
    flush(&mut job, &mut buf, bold, italic, code);
    job
}
