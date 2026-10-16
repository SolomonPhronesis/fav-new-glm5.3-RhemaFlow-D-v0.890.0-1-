//! Data model. Every persisted struct is plain serde so the on-disk format is
//! an open, documented JSON file (roadmap item 7: no lock-in).

use serde::{Deserialize, Serialize};

/// Bump when the on-disk shape changes, and add a migration step in `store`.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Theme v1: the minimum a church needs to make slides look like theirs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub font_family: String,
    pub font_size_pt: f32,
    pub text_rgb: [u8; 3],
    pub background_rgb: [u8; 3],
    pub align: Align,
    /// Safe-area margin as a percentage of the output's shorter side
    /// (projectors overscan; roadmap item 18).
    pub margin_pct: f32,
}

impl Default for Theme {
    /// "Cathedral Dark": pure black, warm-white serif — matches the existing stage.
    fn default() -> Self {
        Theme {
            name: "Cathedral Dark".into(),
            font_family: "serif".into(),
            font_size_pt: 54.0,
            text_rgb: [244, 239, 228],
            background_rgb: [0, 0, 0],
            align: Align::Center,
            margin_pct: 5.0,
        }
    }
}

impl Theme {
    /// Reject values that would render as an invisible or broken slide.
    pub fn validate(&self) -> Result<(), String> {
        if !(8.0..=400.0).contains(&self.font_size_pt) {
            return Err(format!("font size {} outside 8–400 pt", self.font_size_pt));
        }
        if !(0.0..=25.0).contains(&self.margin_pct) {
            return Err(format!("margin {}% outside 0–25%", self.margin_pct));
        }
        if self.text_rgb == self.background_rgb {
            return Err("text and background colour are identical".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    pub id: u64,
    pub text: String,
    #[serde(default)]
    pub notes: String,
    /// Name of a theme override; `None` = the service theme.
    #[serde(default)]
    pub theme: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ItemKind {
    /// A Bible passage. Text is resolved by rf-bible at show time, so the
    /// service file never embeds copyrighted translation text.
    Scripture { reference: String, translation: String },
    Slides { slides: Vec<Slide> },
    /// Songs arrive in Phase 2; the slot exists so service files written
    /// today stay loadable tomorrow. `arrangement` lists section labels.
    Song { song_id: String, arrangement: Vec<String> },
    Media { path: String },
    Timer { seconds: u32, label: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: u64,
    pub title: String,
    pub kind: ItemKind,
    /// Private note for the operator ("start the song after the prayer"). Never drawn on the stage.
    #[serde(default)]
    pub notes: String,
    /// How long this item should take, in minutes (for the run-time total and the finish time).
    #[serde(default)]
    pub minutes: Option<u32>,
}

impl Item {
    /// How many "show" positions this item has. Zero means navigation skips it.
    pub fn slide_count(&self) -> usize {
        match &self.kind {
            ItemKind::Slides { slides } => slides.len(),
            ItemKind::Song { arrangement, .. } => arrangement.len().max(1),
            ItemKind::Scripture { .. } | ItemKind::Media { .. } | ItemKind::Timer { .. } => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Service {
    pub schema_version: u32,
    pub title: String,
    #[serde(default)]
    pub theme: Theme,
    pub items: Vec<Item>,
    /// Monotonic id source so ids are never reused after a delete.
    pub next_id: u64,
    /// Last live position, so a relaunch can offer "resume". Never auto-shown.
    #[serde(default)]
    pub last_live: Option<crate::runner::Output>,
}

impl Service {
    pub fn new(title: &str) -> Self {
        Service {
            schema_version: SCHEMA_VERSION,
            title: title.to_string(),
            theme: Theme::default(),
            items: Vec::new(),
            next_id: 1,
            last_live: None,
        }
    }

    fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_item(&mut self, title: &str, kind: ItemKind) -> u64 {
        let id = self.take_id();
        self.items.push(Item { id, title: title.to_string(), kind, notes: String::new(), minutes: None });
        id
    }

    /// Convenience: a text item from blank-line-separated paragraphs
    /// (one slide per paragraph) — the announcements/prayer workflow.
    pub fn add_text_item(&mut self, title: &str, body: &str) -> u64 {
        let slides: Vec<Slide> = body
            .split("\n\n")
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(|p| Slide { id: self.take_id(), text: p.to_string(), notes: String::new(), theme: None })
            .collect();
        self.add_item(title, ItemKind::Slides { slides })
    }

    pub fn index_of(&self, item_id: u64) -> Option<usize> {
        self.items.iter().position(|i| i.id == item_id)
    }

    pub fn item(&self, item_id: u64) -> Option<&Item> {
        self.items.iter().find(|i| i.id == item_id)
    }
}
