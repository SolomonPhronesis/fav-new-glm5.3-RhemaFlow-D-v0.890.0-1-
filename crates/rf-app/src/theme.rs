//! "Cathedral Dark" design system — senior UI/UX decisions encoded.
//!
//! Dark, calm, high-contrast for a dimmed media booth; a single gold accent
//! reserved for the live/staged state so the operator's eye always lands on
//! what matters. The stage surface is pure black with warm-white serif text:
//! maximum legibility on a projector, reverent in tone.

use egui::{Color32, Rounding, Style, Vec2};

pub const BG: Color32 = Color32::from_rgb(0x0E, 0x11, 0x16); // app background
pub const PANEL: Color32 = Color32::from_rgb(0x16, 0x1B, 0x22); // panels
pub const PANEL_2: Color32 = Color32::from_rgb(0x1C, 0x22, 0x2B); // cards
pub const BORDER: Color32 = Color32::from_rgb(0x2A, 0x30, 0x38);
pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE9, 0xEF);
pub const MUTED: Color32 = Color32::from_rgb(0xA4, 0xAC, 0xBA); // lifted from 8B93A1: readable at arm's length in a dim booth
pub const GOLD: Color32 = Color32::from_rgb(0xD4, 0xA2, 0x4C);
pub const GREEN: Color32 = Color32::from_rgb(0x3F, 0xB6, 0x8B);
pub const RED: Color32 = Color32::from_rgb(0xE5, 0x53, 0x4B);
pub const BLUE: Color32 = Color32::from_rgb(0x5B, 0x9D, 0xD9);
pub const STAGE_TEXT: Color32 = Color32::from_rgb(0xF5, 0xF1, 0xE8); // warm white

// transcript blue/green panel (v0.831.0): green = detected verse, blue = voice command.
// The deep blue-to-green wash says "this is the listening column" without a single word.
pub const TR_TOP: Color32 = Color32::from_rgb(0x0C, 0x22, 0x30); // panel gradient top (deep blue)
pub const TR_BOT: Color32 = Color32::from_rgb(0x0B, 0x2A, 0x24); // panel gradient bottom (deep green)
pub const TR_BORDER: Color32 = Color32::from_rgb(0x1F, 0x4A, 0x52);
pub const TR_TEAL: Color32 = Color32::from_rgb(0x7F, 0xD6, 0xC4); // panel heading
pub const TR_GREEN: Color32 = Color32::from_rgb(0x3E, 0xCF, 0x8E); // verse lines
pub const TR_BLUE: Color32 = Color32::from_rgb(0x4F, 0x9D, 0xFF); // command lines
pub const TR_BLUE_TEXT: Color32 = Color32::from_rgb(0xA9, 0xCE, 0xFF); // command line text
pub const TR_EDGE: Color32 = Color32::from_rgb(0x2B, 0x6A, 0x73); // plain lines

pub const NAME: &str = "RhemaFlow";
pub const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));
/// What people see in the window, the header and the menu bar: just the name (the version lives in About).
pub const TITLE: &str = "RhemaFlow";

pub fn apply(ctx: &egui::Context) {
    let mut style = Style::default();
    let v = &mut style.visuals;
    v.dark_mode = true;
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = Color32::from_rgb(0x0A, 0x0D, 0x11);
    v.faint_bg_color = PANEL_2;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = GOLD.gamma_multiply(0.35);
    v.selection.stroke = egui::Stroke::new(1.0_f32, GOLD);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    v.widgets.inactive.bg_fill = PANEL_2;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, MUTED);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x24, 0x2C, 0x37);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.2_f32, TEXT);
    v.widgets.active.bg_fill = Color32::from_rgb(0x2E, 0x38, 0x44);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.2_f32, GOLD);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    let rounding = Rounding::same(6.0);
    v.widgets.noninteractive.rounding = rounding;
    v.widgets.inactive.rounding = rounding;
    v.widgets.hovered.rounding = rounding;
    v.widgets.active.rounding = rounding;
    v.widgets.open.rounding = rounding;
    v.menu_rounding = rounding;
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    // a clear size scale (egui's default small text is 9 px, too small to read in a dim booth)
    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(14.0, FontFamily::Monospace)),
    ]
    .into();
    ctx.set_style(style);
}

/// Stage font auto-fit: bigger for short passages, always readable.
pub fn stage_font_size(text_len: usize, viewport_height: f32) -> f32 {
    let base = if text_len < 120 {
        68.0
    } else if text_len < 240 {
        54.0
    } else if text_len < 420 {
        44.0
    } else if text_len < 700 {
        34.0
    } else {
        27.0
    };
    // scale with viewport height (design baseline: 540 px)
    (base * (viewport_height / 540.0)).clamp(14.0, 140.0)
}

#[cfg(test)]
mod version_tests {
    #[test]
    fn name_and_version_come_from_the_build() {
        assert_eq!(super::VERSION, format!("v{}", env!("CARGO_PKG_VERSION")));
        assert_eq!(super::TITLE, super::NAME);
        assert_eq!(super::TITLE, "RhemaFlow", "the window and header show only the name");
    }
}
