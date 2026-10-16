//! Accessibility mode (v0.835.0): large print, high contrast and a dyslexia-friendly font, for the congregation
//! screen and (scale only) for the operator window. Pure choices; the app applies them.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageFont {
    #[default]
    Serif,
    /// OpenDyslexic (SIL Open Font License)
    Dyslexic,
    /// Atkinson Hyperlegible, made for low vision (SIL Open Font License)
    Hyperlegible,
}

impl StageFont {
    pub const ALL: [StageFont; 3] = [StageFont::Serif, StageFont::Dyslexic, StageFont::Hyperlegible];
    pub fn label(self) -> &'static str {
        match self {
            StageFont::Serif => "Standard (serif)",
            StageFont::Dyslexic => "Dyslexia-friendly (OpenDyslexic)",
            StageFont::Hyperlegible => "High legibility (Atkinson Hyperlegible)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Contrast {
    #[default]
    Off,
    WhiteOnBlack,
    YellowOnBlack,
    BlackOnWhite,
}

impl Contrast {
    pub const ALL: [Contrast; 4] = [Contrast::Off, Contrast::WhiteOnBlack, Contrast::YellowOnBlack, Contrast::BlackOnWhite];
    pub fn label(self) -> &'static str {
        match self {
            Contrast::Off => "Normal colours",
            Contrast::WhiteOnBlack => "White on black",
            Contrast::YellowOnBlack => "Yellow on black",
            Contrast::BlackOnWhite => "Black on white",
        }
    }
    /// (background, text)
    pub fn colors(self) -> Option<([u8; 3], [u8; 3])> {
        match self {
            Contrast::Off => None,
            Contrast::WhiteOnBlack => Some(([0, 0, 0], [255, 255, 255])),
            Contrast::YellowOnBlack => Some(([0, 0, 0], [255, 235, 59])),
            Contrast::BlackOnWhite => Some(([255, 255, 255], [0, 0, 0])),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Access {
    pub large_print: bool,
    pub contrast: Contrast,
    pub font: StageFont,
    /// the operator window's size (1.0 = normal)
    pub ui_scale: f32,
}

impl Default for Access {
    fn default() -> Self {
        Access { large_print: false, contrast: Contrast::Off, font: StageFont::Serif, ui_scale: 1.0 }
    }
}

pub const LARGE_PRINT_FACTOR: f32 = 1.35;

impl Access {
    pub fn is_on(&self) -> bool {
        *self != Access::default()
    }
    /// Multiplier on the stage text size.
    pub fn zoom_factor(&self) -> f32 {
        if self.large_print { LARGE_PRINT_FACTOR } else { 1.0 }
    }
    /// The colours the stage should use given the normal ones.
    pub fn stage_colors(&self, bg: [u8; 3], fg: [u8; 3]) -> ([u8; 3], [u8; 3]) {
        self.contrast.colors().unwrap_or((bg, fg))
    }
    pub fn ui_zoom(&self) -> f32 {
        self.ui_scale.clamp(0.8, 1.8)
    }
    /// One tap: everything helpful on at once.
    pub fn everything() -> Access {
        Access { large_print: true, contrast: Contrast::WhiteOnBlack, font: StageFont::Hyperlegible, ui_scale: 1.25 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_changes_until_something_is_switched_on() {
        let a = Access::default();
        assert!(!a.is_on());
        assert_eq!(a.zoom_factor(), 1.0);
        assert_eq!(a.stage_colors([1, 2, 3], [4, 5, 6]), ([1, 2, 3], [4, 5, 6]));
        assert_eq!(a.ui_zoom(), 1.0);
    }

    #[test]
    fn large_print_and_contrast_change_the_stage() {
        let a = Access { large_print: true, contrast: Contrast::YellowOnBlack, ..Default::default() };
        assert!(a.is_on());
        assert_eq!(a.zoom_factor(), LARGE_PRINT_FACTOR);
        assert_eq!(a.stage_colors([200, 0, 0], [0, 0, 200]), ([0, 0, 0], [255, 235, 59]));
        assert_eq!(Access { ui_scale: 5.0, ..Default::default() }.ui_zoom(), 1.8);
        assert_eq!(Access { ui_scale: 0.1, ..Default::default() }.ui_zoom(), 0.8);
    }

    #[test]
    fn every_high_contrast_choice_is_very_readable() {
        for c in Contrast::ALL {
            if let Some((bg, fg)) = c.colors() {
                assert!(crate::themes::contrast(fg, bg) >= 12.0, "{:?}", c);
            }
        }
        assert!(Access::everything().is_on());
    }

    #[test]
    fn choices_survive_saving() {
        let j = serde_json::to_string(&Access::everything()).unwrap();
        assert_eq!(serde_json::from_str::<Access>(&j).unwrap(), Access::everything());
        assert!(j.contains("hyperlegible") && j.contains("white_on_black"));
    }
}
