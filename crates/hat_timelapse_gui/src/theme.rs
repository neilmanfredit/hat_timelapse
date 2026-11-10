//! Navy and gold, to match the Fredian Shield / "guy in the hat" brand.

use egui::{Color32, CornerRadius, Stroke, Visuals};

pub const NAVY: Color32 = Color32::from_rgb(0x0d, 0x1b, 0x3e);
pub const NAVY_DEEP: Color32 = Color32::from_rgb(0x08, 0x12, 0x2b);
pub const NAVY_RAISED: Color32 = Color32::from_rgb(0x15, 0x27, 0x52);
pub const NAVY_LINE: Color32 = Color32::from_rgb(0x26, 0x3a, 0x6b);
pub const GOLD: Color32 = Color32::from_rgb(0xc8, 0xa8, 0x2c);
pub const GOLD_SOFT: Color32 = Color32::from_rgb(0xe0, 0xc8, 0x6a);
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xec, 0xf5);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x9a, 0xa6, 0xc4);
pub const SKY: Color32 = Color32::from_rgb(0x7f, 0xb6, 0xff);
pub const GOOD: Color32 = Color32::from_rgb(0x6c, 0xcf, 0x8e);
pub const WARN: Color32 = Color32::from_rgb(0xf0, 0x8a, 0x5d);

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = NAVY;
    v.window_fill = NAVY_RAISED;
    v.extreme_bg_color = NAVY_DEEP;
    v.faint_bg_color = Color32::from_rgb(0x11, 0x21, 0x48);
    v.code_bg_color = NAVY_DEEP;
    v.hyperlink_color = GOLD_SOFT;
    v.override_text_color = Some(TEXT);
    v.window_stroke = Stroke::new(1.0, NAVY_LINE);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0xc8, 0xa8, 0x2c, 90);
    v.selection.stroke = Stroke::new(1.0, GOLD);

    let r = CornerRadius::same(6);
    v.widgets.noninteractive.bg_fill = NAVY;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, NAVY_LINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    // Slider rails and check boxes: a step lighter than windows so they stand out.
    v.widgets.inactive.bg_fill = NAVY_LINE;
    v.slider_trailing_fill = true;
    v.widgets.inactive.weak_bg_fill = NAVY_RAISED;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, NAVY_LINE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x1e, 0x33, 0x66);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x1e, 0x33, 0x66);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, GOLD);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, GOLD_SOFT);
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.bg_fill = GOLD;
    v.widgets.active.weak_bg_fill = GOLD;
    v.widgets.active.bg_stroke = Stroke::new(1.0, GOLD_SOFT);
    v.widgets.active.fg_stroke = Stroke::new(1.5, NAVY);
    v.widgets.active.corner_radius = r;
    v.widgets.open.bg_fill = NAVY_LINE;
    v.widgets.open.weak_bg_fill = NAVY_LINE;
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

/// A solid gold call-to-action button.
pub fn primary(text: &str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.to_string()).color(NAVY).strong())
        .fill(GOLD)
        .corner_radius(CornerRadius::same(6))
}

/// A gold all-caps eyebrow label.
pub fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .color(GOLD)
            .small()
            .strong(),
    );
}
