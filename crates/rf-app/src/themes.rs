//! Sermon-series looks (v0.835.0): a name, colours, style, size and an optional background picture, saved once and
//! applied with one click for the whole series. Pure data; the app applies it.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeriesTheme {
    pub name: String,
    /// the series title, shown by the "Series" lower third
    #[serde(default)]
    pub series_title: String,
    pub bg: [u8; 3],
    pub fg: [u8; 3],
    #[serde(default = "default_accent")]
    pub accent: [u8; 3],
    /// `StageStyle` key: classic | lower | chroma | large
    #[serde(default = "default_style")]
    pub style: String,
    #[serde(default = "one")]
    pub zoom: f32,
    /// a picture file used as the background while this series runs
    #[serde(default)]
    pub background: Option<String>,
    /// the church name line on the screen, if different for this series
    #[serde(default)]
    pub church_line: Option<String>,
}

fn default_accent() -> [u8; 3] {
    [224, 178, 90]
}
fn default_style() -> String {
    "classic".into()
}
fn one() -> f32 {
    1.0
}

/// A handful to start from. Every one is readable from the back of a room (contrast checked in the tests).
pub fn presets() -> Vec<SeriesTheme> {
    let t = |name: &str, bg: [u8; 3], fg: [u8; 3], accent: [u8; 3]| SeriesTheme { name: name.into(), series_title: String::new(), bg, fg, accent, style: "classic".into(), zoom: 1.0, background: None, church_line: None };
    vec![
        t("Midnight & gold", [10, 22, 52], [255, 255, 255], [224, 178, 90]),
        t("Advent purple", [44, 20, 70], [250, 246, 255], [230, 190, 255]),
        t("Easter morning", [250, 246, 232], [30, 24, 12], [150, 100, 0]),
        t("Harvest", [60, 30, 10], [255, 244, 224], [240, 170, 60]),
        t("Ordinary time", [18, 54, 40], [244, 255, 248], [150, 230, 190]),
        t("Plain black", [0, 0, 0], [255, 255, 255], [224, 178, 90]),
    ]
}

fn lum(c: [u8; 3]) -> f32 {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

/// Contrast ratio between text and background (WCAG): 7 or more reads from the back of a room.
pub fn contrast(a: [u8; 3], b: [u8; 3]) -> f32 {
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

impl SeriesTheme {
    /// A warning when the colours would be hard to read, or None if all is well.
    pub fn problem(&self) -> Option<String> {
        if self.name.trim().is_empty() {
            return Some("Give the look a name.".into());
        }
        let c = contrast(self.fg, self.bg);
        if c < 4.5 {
            return Some(format!("The text and background are too close in colour ({c:.1}:1). Aim for 7:1 or more."));
        }
        None
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct ThemeBook {
    pub themes: Vec<SeriesTheme>,
    #[serde(default)]
    pub active: Option<String>,
}

impl ThemeBook {
    pub fn with_presets() -> ThemeBook {
        ThemeBook { themes: presets(), active: None }
    }

    /// Add or replace by name (names are the identity, ignoring case).
    pub fn save(&mut self, t: SeriesTheme) {
        match self.themes.iter_mut().find(|x| x.name.eq_ignore_ascii_case(&t.name)) {
            Some(x) => *x = t,
            None => self.themes.push(t),
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.themes.retain(|t| !t.name.eq_ignore_ascii_case(name));
        if self.active.as_deref().map_or(false, |a| a.eq_ignore_ascii_case(name)) {
            self.active = None;
        }
    }

    pub fn get(&self, name: &str) -> Option<&SeriesTheme> {
        self.themes.iter().find(|t| t.name.eq_ignore_ascii_case(name))
    }

    pub fn load(path: &Path) -> ThemeBook {
        match std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<ThemeBook>(&s).ok()) {
            Some(b) if !b.themes.is_empty() => b,
            _ => ThemeBook::with_presets(),
        }
    }

    pub fn store(&self, path: &Path) -> std::io::Result<()> {
        crate::fsutil::write_atomic(path, serde_json::to_string_pretty(self).unwrap_or_default().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ready_made_look_is_readable() {
        for t in presets() {
            assert!(contrast(t.fg, t.bg) >= 7.0, "{} is too faint: {:.1}", t.name, contrast(t.fg, t.bg));
            assert!(t.problem().is_none());
        }
    }

    #[test]
    fn faint_colours_are_flagged_and_names_are_required() {
        let mut t = presets().remove(0);
        t.fg = [30, 40, 70];
        assert!(t.problem().unwrap().contains("too close"));
        t.name = " ".into();
        assert!(t.problem().unwrap().contains("name"));
    }

    #[test]
    fn saving_replaces_by_name_and_removing_clears_the_active_one() {
        let mut b = ThemeBook::with_presets();
        let n = b.themes.len();
        let mut t = b.get("advent purple").unwrap().clone();
        t.series_title = "Waiting".into();
        b.save(t);
        assert_eq!(b.themes.len(), n, "same name: replaced, not added");
        assert_eq!(b.get("Advent Purple").unwrap().series_title, "Waiting");
        b.active = Some("Advent purple".into());
        b.remove("ADVENT PURPLE");
        assert!(b.get("advent purple").is_none() && b.active.is_none());
    }

    #[test]
    fn the_book_survives_a_round_trip_and_a_broken_file_falls_back() {
        let d = std::env::temp_dir().join(format!("rf-themes-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("themes.json");
        let mut b = ThemeBook::with_presets();
        b.save(SeriesTheme { name: "Romans".into(), series_title: "Romans 8".into(), bg: [0, 0, 0], fg: [255, 255, 255], accent: [1, 2, 3], style: "large".into(), zoom: 1.2, background: Some("/x/y.jpg".into()), church_line: None });
        b.store(&p).unwrap();
        let back = ThemeBook::load(&p);
        assert_eq!(back.get("romans").unwrap().zoom, 1.2);
        std::fs::write(&p, "{ not json").unwrap();
        assert_eq!(ThemeBook::load(&p).themes.len(), presets().len());
        assert_eq!(ThemeBook::load(&d.join("missing.json")).themes.len(), presets().len());
    }
}
