//! A calm, light look with one accent colour.

use egui::{Color32, CornerRadius, Stroke, Visuals};

pub const BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const SIDEBAR: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF7);
pub const CARD: Color32 = Color32::from_rgb(0xF8, 0xF8, 0xFA);
pub const BORDER: Color32 = Color32::from_rgb(0xE4, 0xE4, 0xE9);
pub const TEXT: Color32 = Color32::from_rgb(0x16, 0x17, 0x1C);
pub const BODY: Color32 = Color32::from_rgb(0x33, 0x36, 0x3F);
pub const MUTED: Color32 = Color32::from_rgb(0x82, 0x86, 0x92);
pub const ACCENT: Color32 = Color32::from_rgb(0x4F, 0x5B, 0xE8);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0xEC, 0xEE, 0xFD);
pub const RED: Color32 = Color32::from_rgb(0xE5, 0x48, 0x4D);
pub const RED_SOFT: Color32 = Color32::from_rgb(0xFD, 0xEC, 0xEC);
pub const GREEN: Color32 = Color32::from_rgb(0x2F, 0xA8, 0x5A);
pub const AMBER: Color32 = Color32::from_rgb(0xE0, 0x9B, 0x1A);
pub const CODE_BG: Color32 = Color32::from_rgb(0xF0, 0xF1, 0xF5);

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::light();
    v.panel_fill = BG;
    v.window_fill = BG;
    v.extreme_bg_color = Color32::WHITE;
    v.faint_bg_color = CARD;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(12);
    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.override_text_color = None;
    let r = CornerRadius::same(8);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, BODY);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.bg_fill = Color32::from_rgb(0xF1, 0xF1, 0xF4);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xF1, 0xF1, 0xF4);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = Color32::from_rgb(0xE8, 0xE9, 0xEE);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xE8, 0xE9, 0xEE);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.bg_fill = ACCENT_SOFT;
    v.widgets.active.weak_bg_fill = ACCENT_SOFT;
    v.widgets.active.fg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.corner_radius = r;
    v.widgets.open.corner_radius = r;
    ctx.set_theme(egui::Theme::Light);
    ctx.style_mut_of(egui::Theme::Light, |s| {
        s.visuals = v.clone();
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = 30.0;
        use egui::{FontFamily, FontId, TextStyle};
        s.text_styles.insert(TextStyle::Body, FontId::new(14.5, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(14.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Small, FontId::new(12.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Heading, FontId::new(24.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace));
    });
}
