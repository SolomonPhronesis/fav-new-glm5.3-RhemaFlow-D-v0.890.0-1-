//! Song library (Stage 2, spec section 2): sections, arrangement, search, safe saving.
//!
//! No song words are shipped. The church types or imports its own (and holds its own licence).

use crate::slides::{slides_from_text, ParsedSlide};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const LIBRARY_SCHEMA: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    pub label: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Song {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub author: String,
    /// CCLI song number and the copyright line, shown under the words (v0.833.0)
    #[serde(default)]
    pub ccli_song: String,
    #[serde(default)]
    pub copyright: String,
    pub sections: Vec<Section>,
    /// Section labels in singing order. Empty = the sections in the order written.
    #[serde(default)]
    pub arrangement: Vec<String>,
}

/// "  Verse   1 " and "verse 1" are the same label.
fn key(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

impl Song {
    /// S2.2: parse typed words. Unlabelled blocks are called `Verse 1`, `Verse 2`... (only they are counted).
    pub fn from_text(title: &str, text: &str) -> Song {
        let mut unlabelled = 0;
        let sections = slides_from_text(text)
            .into_iter()
            .map(|s| {
                let label = if s.label.is_empty() {
                    unlabelled += 1;
                    format!("Verse {unlabelled}")
                } else {
                    s.label
                };
                Section { label, text: s.text }
            })
            .collect();
        Song { id: 0, title: title.trim().to_string(), author: String::new(), ccli_song: String::new(), copyright: String::new(), sections, arrangement: Vec::new() }
    }

    /// The words as typed text again (what the editor shows).
    pub fn to_text(&self) -> String {
        self.sections.iter().map(|s| format!("{}:\n{}", s.label, s.text)).collect::<Vec<_>>().join("\n\n")
    }

    /// S2.1 / S2.3: `Verse 1, Chorus, Verse 2, Chorus` -> the list (blank entries dropped, inner spaces collapsed).
    pub fn set_arrangement_text(&mut self, text: &str) {
        self.arrangement = text
            .split(',')
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|p| !p.is_empty())
            .collect();
    }

    pub fn arrangement_text(&self) -> String {
        self.arrangement.join(", ")
    }

    /// Sections in singing order, plus the arrangement entries that matched no section.
    pub fn ordered(&self) -> (Vec<&Section>, Vec<String>) {
        if self.arrangement.is_empty() {
            return (self.sections.iter().collect(), Vec::new());
        }
        let mut out = Vec::new();
        let mut unknown = Vec::new();
        for want in &self.arrangement {
            match self.sections.iter().find(|s| key(&s.label) == key(want)) {
                Some(s) => out.push(s),
                None => unknown.push(want.clone()),
            }
        }
        (out, unknown)
    }

    /// S2.4: one slide per section in singing order; long sections are split to `max_lines` (0 = never).
    pub fn slides(&self, max_lines: usize) -> (Vec<ParsedSlide>, Vec<String>) {
        let (secs, unknown) = self.ordered();
        let mut out = Vec::new();
        for s in secs {
            let lines: Vec<&str> = s.text.lines().collect();
            if max_lines == 0 || lines.len() <= max_lines {
                out.push(ParsedSlide { label: s.label.clone(), text: s.text.clone() });
                continue;
            }
            for (n, chunk) in lines.chunks(max_lines).enumerate() {
                let label = if n == 0 { s.label.clone() } else { format!("{} ({})", s.label, n + 1) };
                out.push(ParsedSlide { label, text: chunk.join("\n") });
            }
        }
        (out, unknown)
    }
}

/// Lowercase words with punctuation turned into spaces.
fn words(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Library {
    pub schema_version: u32,
    songs: Vec<Song>,
    next_id: u64,
}

impl Default for Library {
    fn default() -> Self {
        Library::new()
    }
}

impl Library {
    pub fn new() -> Library {
        Library { schema_version: LIBRARY_SCHEMA, songs: Vec::new(), next_id: 1 }
    }
    pub fn songs(&self) -> &[Song] {
        &self.songs
    }
    /// S2.7: ids only go up.
    pub fn add(&mut self, mut song: Song) -> u64 {
        song.id = self.next_id;
        self.next_id += 1;
        let id = song.id;
        self.songs.push(song);
        id
    }
    pub fn get(&self, id: u64) -> Option<&Song> {
        self.songs.iter().find(|s| s.id == id)
    }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Song> {
        self.songs.iter_mut().find(|s| s.id == id)
    }
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.songs.len();
        self.songs.retain(|s| s.id != id);
        self.songs.len() != before
    }

    /// S2.5: every search word must be somewhere in title, author or lyrics.
    pub fn search(&self, query: &str) -> Vec<&Song> {
        let q = words(query);
        let mut hits: Vec<(u8, String, u64, &Song)> = Vec::new();
        for s in &self.songs {
            let title = words(&s.title);
            let rank = if q.is_empty() {
                0
            } else {
                let lyrics = s.sections.iter().map(|x| words(&x.text)).collect::<Vec<_>>().join(" ");
                let hay = format!("{} {} {}", title, words(&s.author), lyrics);
                if !q.split(' ').all(|w| hay.contains(w)) {
                    continue;
                }
                if title.starts_with(&q) {
                    0
                } else if title.contains(&q) {
                    1
                } else {
                    2
                }
            };
            hits.push((rank, title, s.id, s));
        }
        hits.sort_by(|a, b| (a.0, &a.1, a.2).cmp(&(b.0, &b.1, b.2)));
        hits.into_iter().map(|h| h.3).collect()
    }
}

pub struct LoadedLibrary {
    pub library: Library,
    /// Something the operator should be told (damaged file kept aside...).
    pub warning: Option<String>,
}

/// S2.6: write the library safely (temporary file, then rename).
pub fn save_library(path: &Path, lib: &Library) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(lib).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    crate::store::atomic_write(path, &json)
}

/// S2.6: a missing file is an empty library; a damaged one is moved aside and reported.
pub fn load_library(path: &Path) -> LoadedLibrary {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LoadedLibrary { library: Library::new(), warning: None },
        Err(e) => return LoadedLibrary { library: Library::new(), warning: Some(format!("Could not read the song library: {e}")) },
    };
    match serde_json::from_slice::<Library>(&bytes) {
        Ok(lib) => LoadedLibrary { library: lib, warning: None },
        Err(e) => {
            let aside = free_name(path);
            let kept = std::fs::rename(path, &aside).is_ok() || std::fs::write(&aside, &bytes).is_ok();
            let name = aside.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let warning = if kept {
                format!("The song library file was damaged ({e}). It was kept as {name} next to it, and a new empty library started.")
            } else {
                format!("The song library file was damaged ({e}) and could not be copied aside.")
            };
            LoadedLibrary { library: Library::new(), warning: Some(warning) }
        }
    }
}

/// `songs.json.damaged`, or `.damaged.2`, `.damaged.3`... so an older damaged copy is never overwritten.
fn free_name(path: &Path) -> PathBuf {
    let base = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "songs.json".into());
    let first = path.with_file_name(format!("{base}.damaged"));
    if !first.exists() {
        return first;
    }
    (2..).map(|n| path.with_file_name(format!("{base}.damaged.{n}"))).find(|p| !p.exists()).unwrap_or(first)
}
