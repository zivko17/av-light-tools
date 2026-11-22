//! Dark theme, after `NetMaint/Theme.cs`.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

pub const BG: Color32 = Color32::from_rgb(18, 18, 23);
pub const PANEL: Color32 = Color32::from_rgb(28, 28, 35);
pub const CARD: Color32 = Color32::from_rgb(36, 36, 45);
pub const BUTTON: Color32 = Color32::from_rgb(46, 46, 57);
pub const BORDER: Color32 = Color32::from_rgb(62, 62, 75);
pub const TEXT: Color32 = Color32::from_rgb(220, 220, 220);
pub const TEXT_DIM: Color32 = Color32::from_rgb(140, 140, 152);
pub const ACCENT: Color32 = Color32::from_rgb(86, 214, 232);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0, 150, 170);
pub const GOOD: Color32 = Color32::from_rgb(120, 210, 115);
pub const WARN: Color32 = Color32::from_rgb(229, 192, 80);
pub const BAD: Color32 = Color32::from_rgb(240, 95, 95);
/// Text boxes and graph backgrounds.
pub const INPUT_BG: Color32 = Color32::from_rgb(14, 14, 18);
/// Graph grid lines.
pub const GRID: Color32 = Color32::from_rgb(38, 38, 48);

/// Applies the dark theme to every style of the context.
pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = PANEL;
        v.window_fill = CARD;
        v.window_stroke = Stroke::new(1.0, BORDER);
        v.extreme_bg_color = INPUT_BG;
        v.faint_bg_color = CARD;
        v.code_bg_color = INPUT_BG;
        v.hyperlink_color = ACCENT;
        v.warn_fg_color = WARN;
        v.error_fg_color = BAD;
        v.selection.bg_fill = Color32::from_rgb(20, 90, 100);
        v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        let radius = CornerRadius::same(3);
        for w in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = radius;
        }
        v.widgets.noninteractive.bg_fill = PANEL;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.inactive.bg_fill = BUTTON;
        v.widgets.inactive.weak_bg_fill = BUTTON;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.hovered.bg_fill = Color32::from_rgb(58, 58, 70);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(58, 58, 70);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_DIM);
        v.widgets.active.bg_fill = Color32::from_rgb(0, 115, 130);
        v.widgets.active.weak_bg_fill = Color32::from_rgb(0, 115, 130);

        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        );
    });
}

/// A section heading in the accent colour.
pub fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).heading().color(ACCENT));
}

/// A sub-heading in the warning colour (`Theme.SectionLabel`).
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).strong().color(WARN));
}

/// Dimmed help text.
pub fn dim(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(TEXT_DIM));
}

/// A primary action button (`Theme.AccentButton`).
pub fn accent_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(Color32::WHITE))
            .fill(Color32::from_rgb(20, 90, 100))
            .stroke(Stroke::new(1.0, ACCENT_DIM)),
    )
}
