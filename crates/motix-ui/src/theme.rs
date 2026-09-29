//! MOTIX visual style: a calm dark theme with a violet accent and a pink playhead.

use egui::{Color32, CornerRadius, Stroke, Visuals};

/// Window background.
pub const BG: Color32 = Color32::from_rgb(0x0F, 0x11, 0x15);
/// Panel background.
pub const PANEL: Color32 = Color32::from_rgb(0x16, 0x19, 0x20);
/// Raised surfaces (track headers, inputs).
pub const RAISED: Color32 = Color32::from_rgb(0x1E, 0x22, 0x2B);
/// Hairlines and borders.
pub const LINE: Color32 = Color32::from_rgb(0x2A, 0x2F, 0x3A);
/// Primary text.
pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE8, 0xEE);
/// Secondary text.
pub const TEXT_WEAK: Color32 = Color32::from_rgb(0x9A, 0xA1, 0xAE);
/// Brand accent (selection, primary buttons).
pub const ACCENT: Color32 = Color32::from_rgb(0x7C, 0x5C, 0xFF);
/// Playhead and "live" indicators.
pub const PLAYHEAD: Color32 = Color32::from_rgb(0xFF, 0x4D, 0x6D);
/// Video items.
pub const VIDEO: Color32 = Color32::from_rgb(0x4F, 0x8C, 0xFF);
/// Audio items.
pub const AUDIO: Color32 = Color32::from_rgb(0x2E, 0xC4, 0x8B);
/// Image items.
pub const IMAGE: Color32 = Color32::from_rgb(0xF5, 0xA6, 0x23);

/// Timeline: video clip body (DaVinci Resolve-style blue).
pub const CLIP_VIDEO: Color32 = Color32::from_rgb(0x2C, 0x55, 0x80);
/// Timeline: video clip name band.
pub const CLIP_VIDEO_BAR: Color32 = Color32::from_rgb(0x3E, 0x72, 0xA8);
/// Timeline: audio clip body (Resolve-style green).
pub const CLIP_AUDIO: Color32 = Color32::from_rgb(0x25, 0x5E, 0x40);
/// Timeline: audio clip name band.
pub const CLIP_AUDIO_BAR: Color32 = Color32::from_rgb(0x33, 0x80, 0x57);
/// Timeline: still-image clip body.
pub const CLIP_IMAGE: Color32 = Color32::from_rgb(0x7A, 0x55, 0x1A);
/// Timeline: still-image clip name band.
pub const CLIP_IMAGE_BAR: Color32 = Color32::from_rgb(0xA3, 0x74, 0x26);
/// Selected clip outline (Resolve uses orange).
pub const SELECTED: Color32 = Color32::from_rgb(0xF2, 0x8C, 0x28);
/// Timeline lane background.
pub const LANE: Color32 = Color32::from_rgb(0x1A, 0x1D, 0x23);

/// Marker colour for display.
#[must_use]
pub fn marker(color: motix_app::MarkerColor) -> Color32 {
    use motix_app::MarkerColor as M;
    match color {
        M::Blue => Color32::from_rgb(0x3D, 0x8B, 0xFF),
        M::Cyan => Color32::from_rgb(0x2E, 0xD3, 0xE0),
        M::Green => Color32::from_rgb(0x3C, 0xC8, 0x5A),
        M::Yellow => Color32::from_rgb(0xF2, 0xD0, 0x2E),
        M::Red => Color32::from_rgb(0xF0, 0x44, 0x44),
        M::Pink => Color32::from_rgb(0xF0, 0x6E, 0xC8),
        M::Purple => Color32::from_rgb(0x9A, 0x6B, 0xFF),
    }
}

/// Applies the MOTIX theme to a context.
pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG;
    v.faint_bg_color = RAISED;
    v.override_text_color = Some(TEXT);
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = ACCENT.linear_multiply(0.55);
    v.selection.stroke = Stroke::new(1.0, TEXT);
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(6);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.inactive.weak_bg_fill = RAISED;
    v.widgets.inactive.bg_fill = RAISED;
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2A, 0x2F, 0x3C);
    v.widgets.active.weak_bg_fill = ACCENT.linear_multiply(0.8);
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
    });
}
