//! The Greek & Hebrew (Strong's) panel (v0.816.0).
//!
//! Three things in one window, matching the "three ways" in docs/STRONGS.md:
//!  * type or say a word, number or sound ("love", "G3056", "agape") and get entries   (way 2: always works, offline)
//!  * the words of the verse on screen as buttons; with a Strong's-tagged KJV file installed each
//!    button knows its exact original word (way 1), without one it searches by the English word
//!  * "Show on stage" puts the chosen word on the congregation screen as a slide

use crate::state::{StageContent, StageKind};
use crate::theme;
use rf_strongs::{Entry, Lexicon, TaggedBible};
use std::path::PathBuf;
use std::time::Instant;

/// One button for a word of the verse on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip {
    pub word: String,
    /// exact numbers from a tagged Bible; empty when untagged
    pub numbers: Vec<String>,
}

pub struct StrongsState {
    pub open: bool,
    dir: PathBuf,
    lexicon: Lexicon,
    tagged: Option<TaggedBible>,
    tagged_tried: bool,
    pub query: String,
    pub lang: char,
    pub results: Vec<String>,
    pub selected: Option<String>,
    pub msg: Option<String>,
}

impl StrongsState {
    /// `dir` = the app's data folder; optional files live in `<dir>/strongs/`:
    /// `hebrew.json` (made by tools/fetch_strongs.py) and `kjv-strongs.xml` (a Strong's-tagged OSIS Bible).
    pub fn new(dir: PathBuf) -> StrongsState {
        let mut lexicon = Lexicon::greek();
        let mut msg = None;
        if let Ok(text) = std::fs::read_to_string(dir.join("strongs").join("hebrew.json")) {
            match Lexicon::from_json(&text) {
                Ok(h) => lexicon.merge(h),
                Err(e) => msg = Some(format!("hebrew.json was not used: {e}")),
            }
        }
        StrongsState { open: false, dir, lexicon, tagged: None, tagged_tried: false, query: String::new(), lang: '?', results: Vec::new(), selected: None, msg }
    }

    fn tagged_path(&self) -> PathBuf {
        self.dir.join("strongs").join("kjv-strongs.xml")
    }

    /// Read the tagged Bible the first time the window is opened (about a fifth of a second).
    pub fn ensure_tagged(&mut self) {
        if self.tagged_tried {
            return;
        }
        self.tagged_tried = true;
        let path = self.tagged_path();
        if let Ok(xml) = std::fs::read_to_string(&path) {
            match TaggedBible::from_osis(&xml) {
                Ok(t) => self.tagged = Some(t),
                Err(e) => self.msg = Some(format!("{} was not used: {e}", path.display())),
            }
        }
    }

    /// For tests and for a file the operator just installed.
    pub fn set_tagged(&mut self, t: Option<TaggedBible>) {
        self.tagged = t;
        self.tagged_tried = true;
    }

    pub fn has_tagged(&self) -> bool {
        self.tagged.is_some()
    }
    /// "What is the Greek for love": fills the results and selects the best one.
    pub fn lookup_word(&mut self, lang: char, word: &str) {
        self.open = true;
        self.lang = lang;
        self.query = word.to_string();
        self.results = self.lexicon.search_english(word, lang, 12).iter().map(|e| e.number.clone()).collect();
        self.selected = self.results.first().cloned();
        self.msg = if self.results.is_empty() {
            Some(match lang {
                'H' if !self.lexicon.has_language('H') => "No Hebrew dictionary is installed yet (see docs/STRONGS.md). Greek is built in.".to_string(),
                _ => format!("No {} entry lists \"{}\" as a King James word.", lang_name(lang), word.trim()),
            })
        } else {
            None
        };
    }

    /// Whatever is in the search box.
    pub fn run_query(&mut self) {
        let q = self.query.clone();
        let hits: Vec<String> = match self.lang {
            'G' | 'H' => {
                let mut v: Vec<String> = self.lexicon.search(&q, 30).iter().filter(|e| e.number.starts_with(self.lang)).map(|e| e.number.clone()).collect();
                if v.is_empty() {
                    v = self.lexicon.search_english(&q, self.lang, 12).iter().map(|e| e.number.clone()).collect();
                }
                v
            }
            _ => self.lexicon.search(&q, 12).iter().map(|e| e.number.clone()).collect(),
        };
        self.results = hits;
        self.selected = self.results.first().cloned();
        self.msg = if self.results.is_empty() && !q.trim().is_empty() { Some(format!("Nothing found for \"{}\".", q.trim())) } else { None };
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.selected.as_deref().and_then(|n| self.lexicon.get(n))
    }

    /// A word button was pressed. With exact numbers it selects them; otherwise it searches the English word.
    pub fn press_chip(&mut self, chip: &Chip) {
        if !chip.numbers.is_empty() {
            self.open = true;
            self.query = chip.numbers.join(", ");
            self.results = chip.numbers.iter().filter(|n| self.lexicon.get(n).is_some()).cloned().collect();
            self.selected = self.results.first().cloned();
            self.msg = if self.results.is_empty() { Some(format!("{} is tagged {} but that number is not in the installed dictionary (Hebrew missing?).", chip.word, chip.numbers.join(", "))) } else { None };
        } else {
            self.lookup_word('?', &chip.word.to_lowercase());
        }
    }

    /// The buttons for the verse(s) on screen. `book` is 0-based, `chapter`/`from`/`to` 1-based;
    /// `plain_text` (the KJV words of the passage) is used for any verse the tagged Bible does not have.
    pub fn chips(&self, book: usize, chapter: usize, from: usize, to: usize, plain_text: &str) -> Vec<Chip> {
        let mut out = Vec::new();
        if let Some(t) = &self.tagged {
            let mut all = true;
            for v in from..=to.max(from) {
                match t.verse(book, chapter, v) {
                    Some(words) => out.extend(words.iter().map(|w| Chip { word: w.text.clone(), numbers: w.strongs.clone() })),
                    None => all = false,
                }
            }
            if all && !out.is_empty() {
                return out;
            }
            out.clear();
        }
        for w in plain_text.split_whitespace() {
            let t: String = w.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
            // verse numbers in a range ("16 For God...") are not words
            if !t.is_empty() && !t.chars().all(|c| c.is_ascii_digit()) {
                out.push(Chip { word: t, numbers: Vec::new() });
            }
        }
        out
    }

    /// What goes on the congregation screen for the chosen entry: (small label, the words).
    pub fn stage_text(e: &Entry) -> (String, String) {
        let mut short: String = e.kjv.split([',', ';']).take(6).map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
        if short.chars().count() > 90 {
            short = short.chars().take(90).collect::<String>() + "...";
        }
        let first_line = if e.translit.is_empty() { e.lemma.clone() } else { format!("{}  ({})", e.lemma, e.translit) };
        (e.number.clone(), format!("{first_line}\n{short}"))
    }

    fn stage_content(&self) -> Option<StageContent> {
        let e = self.selected_entry()?;
        let (label, text) = Self::stage_text(e);
        Some(StageContent { kind: StageKind::Slide, label, text, translation: String::new(), chapter_only: false, updated: Some(Instant::now()) })
    }

    /// The panel body, drawn inside the right dock. `verse` = what is on the stage if it is a scripture:
    /// (book 0-based, chapter, from, to, its text).
    /// Kept for tests/callers: the old floating window is gone; the dock draws `body`.
    pub fn ui(&mut self, _ctx: &egui::Context, _verse: Option<(usize, usize, usize, usize, String)>) -> Option<StageContent> {
        None
    }

    pub fn body(&mut self, ui: &mut egui::Ui, verse: Option<(usize, usize, usize, usize, String)>) -> Option<StageContent> {
        self.ensure_tagged();
        let mut request: Option<StageContent> = None;
        {
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut self.query).desired_width(150.0).hint_text("love, G3056, agape, λόγος"));
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Search").clicked() || enter {
                    self.run_query();
                }
                for (c, n) in [('?', "Either"), ('G', "Greek"), ('H', "Hebrew")] {
                    ui.radio_value(&mut self.lang, c, n);
                }
            });
            let heb = if self.lexicon.has_language('H') { "Hebrew installed" } else { "Hebrew not installed" };
            let tag = if self.tagged.is_some() { "word-by-word Bible installed" } else { "no word-by-word Bible (searching by English word)" };
            ui.label(egui::RichText::new(format!("Greek built in ({} entries) - {heb} - {tag}", self.lexicon.len())).weak().small());
            if let Some(m) = &self.msg {
                ui.label(egui::RichText::new(m).color(theme::GOLD));
            }
            if let Some((b, c, f, t, text)) = &verse {
                ui.separator();
                ui.label("Words of the verse on screen (press one):");
                let chips = self.chips(*b, *c, *f, *t, text);
                let mut pressed: Option<Chip> = None;
                ui.horizontal_wrapped(|ui| {
                    for chip in &chips {
                        let label = if chip.numbers.is_empty() { chip.word.clone() } else { format!("{} {}", chip.word, chip.numbers.join("/")) };
                        let b = ui.small_button(label);
                        if b.clicked() {
                            pressed = Some(chip.clone());
                        }
                    }
                });
                if let Some(c) = pressed {
                    self.press_chip(&c);
                }
            }
            ui.separator();
            let results = self.results.clone();
            egui::ScrollArea::vertical().max_height(150.0).id_source("strongs_results").show(ui, |ui| {
                for n in &results {
                    if let Some(e) = self.lexicon.get(n) {
                        let short: String = e.kjv.chars().take(50).collect();
                        let label = format!("{}   {}  {}   {}", e.number, e.lemma, e.translit, short);
                        if ui.selectable_label(self.selected.as_deref() == Some(n.as_str()), label).clicked() {
                            self.selected = Some(n.clone());
                        }
                    }
                }
            });
            if let Some(e) = self.selected_entry().cloned() {
                ui.separator();
                ui.label(egui::RichText::new(format!("{}   {}", e.lemma, e.translit)).size(26.0).color(theme::GOLD));
                ui.label(format!("{}   pronounced {}", e.number, e.pron));
                if !e.definition.is_empty() {
                    ui.label(&e.definition);
                }
                if !e.derivation.is_empty() {
                    ui.label(egui::RichText::new(format!("from: {}", e.derivation)).weak());
                }
                if !e.kjv.is_empty() {
                    ui.label(egui::RichText::new(format!("King James renders it: {}", e.kjv)).weak());
                }
                if ui.button("Show on stage").clicked() {
                    request = self.stage_content();
                }
            }
            ui.separator();
            ui.collapsing("Where this comes from", |ui| {
                for l in self.lexicon.licenses() {
                    ui.label(egui::RichText::new(l).weak().small());
                }
                ui.label(egui::RichText::new("Word-by-word lookup needs a Strong's-tagged KJV file: see docs/STRONGS.md.").weak().small());
            });
        }
        request
    }
}

fn lang_name(l: char) -> &'static str {
    match l {
        'G' => "Greek",
        'H' => "Hebrew",
        _ => "Greek or Hebrew",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rf_strongs::ascii_fold;

    const OSIS: &str = r#"<osis><osisText>
<div type="book" osisID="John"><chapter osisID="John.1">
<verse osisID="John.1.1">In the <w lemma="strong:G746">beginning</w> was the <w lemma="strong:G3056">Word</w>, and the <w lemma="strong:G3056">Word</w> was with <w lemma="strong:G2316">God</w>.</verse>
</chapter></div></osisText></osis>"#;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rf-strongs-ui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn what_is_the_greek_for_word_selects_logos() {
        let mut s = StrongsState::new(tmp("word"));
        s.lookup_word('G', "word");
        assert!(s.open, "asking opens the panel");
        assert_eq!(s.selected.as_deref(), Some("G3056"));
        assert_eq!(&s.results[..2], ["G3056", "G4487"]);
        let e = s.selected_entry().unwrap();
        assert_eq!(e.lemma, "λόγος");
    }

    #[test]
    fn a_word_with_no_entry_says_so_in_plain_words() {
        let mut s = StrongsState::new(tmp("none"));
        s.lookup_word('G', "xylophone");
        assert!(s.results.is_empty() && s.msg.as_deref().unwrap().contains("xylophone"));
        s.lookup_word('H', "peace");
        assert!(s.msg.as_deref().unwrap().contains("No Hebrew dictionary"), "{:?}", s.msg);
    }

    #[test]
    fn the_search_box_takes_numbers_sounds_greek_letters_and_english() {
        let mut s = StrongsState::new(tmp("box"));
        for (q, want) in [("G3056", "G3056"), ("3056", "G3056"), ("agape", "G26"), ("ἀγάπη", "G26"), ("love", "G26")] {
            s.query = q.to_string();
            s.lang = '?';
            s.run_query();
            assert_eq!(s.selected.as_deref(), Some(want), "{q}");
        }
        s.query = "nothingatall".into();
        s.run_query();
        assert!(s.results.is_empty() && s.msg.is_some());
        s.query = "  ".into();
        s.run_query();
        assert!(s.msg.is_none(), "an empty box is not an error");
    }

    #[test]
    fn without_a_tagged_bible_the_verse_words_search_by_english() {
        let mut s = StrongsState::new(tmp("plain"));
        let chips = s.chips(42, 1, 1, 1, "In the beginning was the Word, and the Word was with God.");
        assert_eq!(chips.len(), 12);
        assert!(chips.iter().all(|c| c.numbers.is_empty()));
        assert_eq!(chips[5].word, "Word");
        let word = chips[5].clone();
        s.press_chip(&word);
        assert_eq!(s.selected.as_deref(), Some("G3056"), "falls back to the English word search");
        // verse numbers inside a range are not words
        let r = s.chips(42, 3, 16, 17, "16 For God so loved the world  17 For God sent not");
        assert!(r.iter().all(|c| c.word != "16" && c.word != "17"));
    }

    #[test]
    fn with_a_tagged_bible_each_word_knows_its_exact_number() {
        let mut s = StrongsState::new(tmp("tagged"));
        s.set_tagged(Some(TaggedBible::from_osis(OSIS).unwrap()));
        assert!(s.has_tagged());
        let chips = s.chips(42, 1, 1, 1, "unused because the tagged Bible has this verse");
        let tagged: Vec<(&str, &str)> = chips.iter().filter(|c| !c.numbers.is_empty()).map(|c| (c.word.as_str(), c.numbers[0].as_str())).collect();
        assert_eq!(tagged, vec![("beginning", "G746"), ("Word", "G3056"), ("Word", "G3056"), ("God", "G2316")]);
        let beginning = chips.iter().find(|c| c.word == "beginning").unwrap().clone();
        s.press_chip(&beginning);
        assert_eq!(s.selected.as_deref(), Some("G746"));
        assert_eq!(ascii_fold(&s.selected_entry().unwrap().translit), "arche");
        // a verse the tagged Bible does not have falls back to plain words
        let other = s.chips(42, 3, 16, 16, "For God so loved the world");
        assert_eq!(other.len(), 6);
        assert!(other.iter().all(|c| c.numbers.is_empty()));
    }

    #[test]
    fn a_tagged_number_missing_from_the_dictionary_says_why() {
        let mut s = StrongsState::new(tmp("hebmissing"));
        s.press_chip(&Chip { word: "beginning".into(), numbers: vec!["H7225".into()] });
        assert!(s.results.is_empty());
        assert!(s.msg.as_deref().unwrap().contains("H7225"));
    }

    #[test]
    fn what_goes_on_the_stage_is_short_and_readable() {
        let mut s = StrongsState::new(tmp("stage"));
        s.lookup_word('G', "word");
        let c = s.stage_content().unwrap();
        assert_eq!(c.kind, StageKind::Slide);
        assert_eq!(c.label, "G3056");
        let lines: Vec<&str> = c.text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("λόγος") && lines[0].contains("lógos"), "{}", lines[0]);
        assert!(lines[1].chars().count() <= 100, "{}", lines[1]);
    }

    #[test]
    fn optional_files_in_the_data_folder_are_picked_up_and_bad_ones_are_reported() {
        let d = tmp("files");
        std::fs::create_dir_all(d.join("strongs")).unwrap();
        std::fs::write(d.join("strongs").join("kjv-strongs.xml"), OSIS).unwrap();
        let mut s = StrongsState::new(d.clone());
        assert!(!s.has_tagged(), "not read until the panel is opened");
        s.ensure_tagged();
        assert!(s.has_tagged());
        let d2 = tmp("badfiles");
        std::fs::create_dir_all(d2.join("strongs")).unwrap();
        std::fs::write(d2.join("strongs").join("hebrew.json"), "nope").unwrap();
        std::fs::write(d2.join("strongs").join("kjv-strongs.xml"), "<osis><osisText>no verses</osisText></osis>").unwrap();
        let mut s2 = StrongsState::new(d2);
        assert!(s2.msg.as_deref().unwrap().contains("hebrew.json"));
        s2.ensure_tagged();
        assert!(!s2.has_tagged() && s2.msg.as_deref().unwrap().contains("kjv-strongs.xml"));
    }

    #[test]
    fn the_window_draws_with_and_without_a_verse() {
        let c = egui::Context::default();
        crate::app::load_stage_fonts(&c);
        let mut s = StrongsState::new(tmp("ui"));
        s.lookup_word('G', "love");
        let mut texts: Vec<String> = Vec::new();
        for verse in [None, Some((42usize, 3usize, 16usize, 16usize, "For God so loved the world".to_string()))] {
            let mut raw = egui::RawInput::default();
            raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 800.0)));
            for _ in 0..3 {
                let v = verse.clone();
                let out = c.run(raw.clone(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        s.body(ui, v.clone());
                    });
                });
                for cs in out.shapes {
                    collect(&cs.shape, &mut texts);
                }
            }
        }
        let all = texts.join("\n");
        for want in ["Search", "Greek built in", "G26", "ἀγάπη", "Show on stage", "loved"] {
            assert!(all.contains(want), "the panel never showed {want:?}");
        }
        assert!(s.open);
    }

    fn collect(s: &egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| collect(x, out)),
            _ => {}
        }
    }

    #[test]
    fn a_closed_panel_draws_nothing() {
        let c = egui::Context::default();
        let mut s = StrongsState::new(tmp("closed"));
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)));
        let out = c.run(raw, |ctx| {
            assert!(s.ui(ctx, None).is_none());
        });
        assert!(out.shapes.is_empty());
    }
}
