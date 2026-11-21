//! Astryx neutral dark design language.

use egui::{Color32, CornerRadius, Stroke, Visuals};

// Astryx background scale
pub const NAVY: Color32 = Color32::from_rgb(0x11, 0x11, 0x12);        // body / panel fill
pub const NAVY_DEEP: Color32 = Color32::from_rgb(0x0d, 0x0d, 0x0e);   // deepest inset
pub const NAVY_RAISED: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x22); // card / window fill
pub const NAVY_LINE: Color32 = Color32::from_rgba_premultiplied(0x10, 0x11, 0x11, 26); // border ~10% white
// Astryx accent (replaces gold)
pub const GOLD: Color32 = Color32::from_rgb(0x3e, 0x9e, 0xfb);        // accent blue
pub const GOLD_SOFT: Color32 = Color32::from_rgb(0x7d, 0xbf, 0xfd);   // accent blue soft
// Astryx text
pub const TEXT: Color32 = Color32::from_rgb(0xdf, 0xe2, 0xe5);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xaa, 0xaf, 0xb5);
// Astryx semantic colours
pub const SKY: Color32 = Color32::from_rgb(0x7d, 0xbf, 0xfd);         // keyframe-changed indicator
pub const GOOD: Color32 = Color32::from_rgb(0xa5, 0xf6, 0x90);
pub const WARN: Color32 = Color32::from_rgb(0xfb, 0xce, 0x03);

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = NAVY;
    v.window_fill = NAVY_RAISED;
    v.extreme_bg_color = NAVY_DEEP;
    v.faint_bg_color = Color32::from_rgb(0x17, 0x17, 0x19);
    v.code_bg_color = NAVY_DEEP;
    v.hyperlink_color = GOLD_SOFT;
    v.override_text_color = Some(TEXT);
    v.window_stroke = Stroke::new(1.0, NAVY_LINE);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0x3e, 0x9e, 0xfb, 60);
    v.selection.stroke = Stroke::new(1.0, GOLD);

    let r = CornerRadius::same(6);
    v.widgets.noninteractive.bg_fill = NAVY;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, NAVY_LINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x2a, 0x2a, 0x2e);
    v.slider_trailing_fill = true;
    v.widgets.inactive.weak_bg_fill = NAVY_RAISED;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, NAVY_LINE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x2d, 0x2d, 0x32);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2d, 0x2d, 0x32);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, GOLD);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, GOLD_SOFT);
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.bg_fill = GOLD;
    v.widgets.active.weak_bg_fill = GOLD;
    v.widgets.active.bg_stroke = Stroke::new(1.0, GOLD_SOFT);
    v.widgets.active.fg_stroke = Stroke::new(1.5, NAVY);
    v.widgets.active.corner_radius = r;
    v.widgets.open.bg_fill = Color32::from_rgb(0x2a, 0x2a, 0x2e);
    v.widgets.open.weak_bg_fill = Color32::from_rgb(0x2a, 0x2a, 0x2e);
    v.widgets.open.bg_stroke = Stroke::new(1.0, GOLD);
    v.widgets.open.corner_radius = r;
    ctx.set_visuals(v);

    // Symbols such as ◆ ▾ ↑ ↓ live in the monospace font; let ordinary text use it as a fallback.
    let mut fonts = egui::FontDefinitions::default();
    if let Some(mono) = fonts
        .families
        .get(&egui::FontFamily::Monospace)
        .and_then(|f| f.first().cloned())
    {
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push(mono);
    }
    ctx.set_fonts(fonts);

    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.interact_size.y = 26.0;
    });
}

/// A solid accent call-to-action button.
pub fn primary(text: &str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.to_string()).color(NAVY).strong())
        .fill(GOLD)
        .corner_radius(CornerRadius::same(6))
}

/// An accent all-caps eyebrow label.
pub fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .color(GOLD)
            .small()
            .strong(),
    );
}
