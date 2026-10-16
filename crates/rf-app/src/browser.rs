//! Scripture browser (v0.812.0): books | chapters | verses in three columns, in the
//! "Scripture" tab.
//!
//! * FOLLOWS the stage: whatever reaches the stage by voice, quotation, card or
//!   next/previous, the browser jumps to that book, chapter and verse and scrolls
//!   it into view.
//! * WORKS BY HAND: click a book, a chapter, then a verse (Shift+click for a range)
//!   to put it on the stage - the fallback when the voice does not catch it.
//! * A typed-reference box ("rev 1:6", "john 3:16-18", "1 cor 5:17") for when you
//!   know exactly where you want to go.

use crate::app::{save_settings, RhemaFlowApp};
use crate::state::StageKind;
use crate::theme::*;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, RichText, ScrollArea, Sense, TextEdit};
use rf_core::Passage;

pub struct BrowserState {
    pub visible: bool,
    /// canonical book index (0 = Genesis)
    pub book: usize,
    /// 1-based
    pub chapter: usize,
    /// last verse clicked (start of a Shift+click range)
    pub anchor: Option<u16>,
    /// scroll the selected book / chapter / verse into view on the next frame
    pub scroll_pending: bool,
    /// which stage change the browser has already followed
    pub last_seq: u64,
    /// the one search box: book names, references, or words from a verse
    pub query: String,
    pub results: Vec<SearchResult>,
    pub results_for: String,
    pub focus_search: bool,
}

impl BrowserState {
    pub fn new(visible: bool) -> Self {
        BrowserState { visible, book: 0, chapter: 1, anchor: None, scroll_pending: true, last_seq: 0, query: String::new(), results: Vec::new(), results_for: String::new(), focus_search: false }
    }

    /// Jump to what is on stage when the stage has changed since last time.
    /// Returns true if the selection moved.
    pub fn follow(&mut self, stage_seq: u64, staged: Option<Passage>, stage_is_scripture: bool) -> bool {
        if stage_seq == self.last_seq {
            return false;
        }
        self.last_seq = stage_seq;
        match staged {
            Some(p) if stage_is_scripture && p.chapter > 0 => {
                self.book = p.book as usize;
                self.chapter = p.chapter as usize;
                self.anchor = Some(p.from);
                self.scroll_pending = true;
                true
            }
            _ => false,
        }
    }

    /// Choose a book by hand: starts at chapter 1.
    pub fn pick_book(&mut self, book: usize) {
        if book != self.book {
            self.book = book;
            self.chapter = 1;
            self.anchor = None;
        }
    }
}

/// The verse range a click means: a plain click is that one verse; Shift+click
/// extends from the previous click (either direction).
pub fn click_range(anchor: Option<u16>, clicked: u16, shift: bool) -> (u16, u16) {
    match (shift, anchor) {
        (true, Some(a)) => (a.min(clicked), a.max(clicked)),
        _ => (clicked, clicked),
    }
}

/// Is this verse part of the passage on stage?
pub fn verse_on_stage(p: &Passage, book: usize, chapter: usize, verse: u16) -> bool {
    p.book as usize == book && p.chapter as usize == chapter && verse >= p.from && verse <= p.to
}

/// Turn typed text ("rev 1:6", "John 3:16-18", "first corinthians 5 17") into a
/// passage that really exists in this Bible, or say why not.
pub fn reference_from_text(bible: &rf_bible::Bible, text: &str) -> Result<Passage, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("type a reference such as John 3:16".into());
    }
    // the speech parser already understands every book spelling; give it the digits as words it likes
    let spoken = t.replace(':', " verse ").replace('.', " ").replace(',', " ");
    let spoken = {
        // "3 16-18" -> "chapter 3 verse 16 to 18" only when no ':' was typed
        if t.contains(':') {
            let (book_and_ch, vs) = spoken.split_once(" verse ").unwrap_or((spoken.as_str(), ""));
            format!("{} verse {}", book_and_ch.trim(), vs.replace('-', " to ").trim())
        } else {
            spoken.replace('-', " to ")
        }
    };
    let o = rf_core::parse(bible, &spoken, &rf_core::ParseContext::default(), 0.0);
    let found = o
        .detections
        .last()
        .or_else(|| o.confirms.iter().rev().find(|d| d.passage.chapter > 0));
    // "john 3" is a chapter, not John 3:1: the search offers to open the chapter
    let best = found.map(|d| {
        if d.kind == rf_core::DetectionKind::ChapterOnly {
            Passage { from: 0, to: 0, ..d.passage }
        } else {
            d.passage
        }
    });
    if best.is_none() {
        if let Some(e) = o.errors.first() {
            return Err(e.message.clone()); // honest: "1 Corinthians 5 has only 13 verses ..."
        }
    }
    match best {
        Some(p) if p.from == 0 && p.chapter > 0 && (p.chapter as usize) <= bible.chapter_count(p.book as usize) => Ok(p),
        Some(p) if bible.exists(p.book as usize, p.chapter as usize, p.from as usize) => Ok(p),
        Some(_) => Err("that chapter or verse does not exist".into()),
        None => match o.confirms.iter().find(|d| d.passage.chapter == 0) {
            Some(d) => Err(format!("{} \u{2014} add a chapter, e.g. {} 1", bible.book_name(d.passage.book as usize), bible.book_name(d.passage.book as usize))),
            None => Err("could not read that reference".into()),
        },
    }
}

/// One line in the search results.
#[derive(Clone, Debug, PartialEq)]
pub enum SearchResult {
    /// open this book in the browser
    Book(usize),
    /// open this chapter in the browser
    Chapter(usize, usize),
    /// a verse (or range) to show; `pct` = relevance (100 = typed reference), `text` = snippet
    Verse { passage: Passage, pct: u8, text: String },
}

fn squash(s: &str) -> String {
    s.to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// Book numbers written as words or digits so "1 sam", "1sam", "first sam", "i sam" all find 1 Samuel.
fn book_query_forms(q: &str) -> Vec<String> {
    let t = q.trim().to_ascii_lowercase();
    let mut forms = vec![squash(&t)];
    for (word, digit) in [("first ", "1"), ("second ", "2"), ("third ", "3"), ("1st ", "1"), ("2nd ", "2"), ("3rd ", "3"), ("iii ", "3"), ("ii ", "2"), ("i ", "1")] {
        if let Some(rest) = t.strip_prefix(word) {
            forms.push(squash(&format!("{digit}{rest}")));
        }
    }
    forms.retain(|f| !f.is_empty());
    forms
}

/// What the search box means (v0.812.0). No error messages: whatever is typed gives the best list it can.
///
///  * letters only ("1 sam", "rev", "john")       -> books that start that way, then verses containing the words
///  * book + chapter ("john 3")                   -> that chapter
///  * a reference ("rev 1:6", "john 3:16-18")     -> that verse, first
///  * words from a verse ("god so loved")         -> verses containing them, best first
pub fn search(bible: &rf_bible::Bible, query: &str, max_verses: usize) -> Vec<SearchResult> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<SearchResult> = Vec::new();
    let has_digit = q.chars().any(|c| c.is_ascii_digit());
    let forms = book_query_forms(q);

    // 1. a full reference, or book + chapter
    if has_digit {
        match reference_from_text(bible, q) {
            Ok(p) if p.from > 0 => {
                let text = bible.passage(p.book as usize, p.chapter as usize, p.from as usize, p.to as usize);
                out.push(SearchResult::Verse { passage: p, pct: 100, text });
            }
            Ok(p) => out.push(SearchResult::Chapter(p.book as usize, p.chapter as usize)),
            Err(_) => {}
        }
    }
    // 2. books whose name starts with what was typed ("1 sam" -> 1 Samuel); only when the whole
    //    query is a book-name prefix, so "god so" never lists books
    let only_book_like = !forms.is_empty() && forms.iter().any(|f| (0..66).any(|b| squash(bible.book_name(b)).starts_with(f.as_str())));
    if only_book_like {
        for b in 0..66 {
            let name = squash(bible.book_name(b));
            if forms.iter().any(|f| name.starts_with(f.as_str())) {
                out.push(SearchResult::Book(b));
            }
        }
    }
    // 3. fuzzy book spelling ("efeziens", "revelations") through the speech parser's matcher
    if out.is_empty() && !has_digit {
        let o = rf_core::parse(bible, q, &rf_core::ParseContext::default(), 0.0);
        for d in o.confirms.iter().chain(o.detections.iter()) {
            if d.kind == rf_core::DetectionKind::BookOnly {
                let b = d.passage.book as usize;
                if !out.contains(&SearchResult::Book(b)) {
                    out.push(SearchResult::Book(b));
                }
            }
        }
    }
    // 4. words from a verse
    let letters = q.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if letters >= 3 && !(has_digit && out.iter().any(|r| matches!(r, SearchResult::Verse { .. }))) {
        for h in bible.search_text(q, max_verses) {
            let p = Passage { book: h.book as u8, chapter: h.chapter as u16, from: h.verse as u16, to: h.verse as u16 };
            if out.iter().any(|r| matches!(r, SearchResult::Verse { passage, .. } if *passage == p)) {
                continue;
            }
            out.push(SearchResult::Verse { passage: p, pct: h.pct, text: bible.verse(h.book, h.chapter, h.verse).to_string() });
        }
    }
    out
}

impl RhemaFlowApp {
    /// Put a passage on the stage from the browser or the reference box.
    pub fn stage_by_hand(&mut self, book: usize, chapter: usize, from: u16, to: u16) {
        let p = Passage { book: book as u8, chapter: chapter as u16, from, to: to.max(from) };
        self.do_stage(p, "browser", 1.0, false);
        self.touches += 1;
    }

    pub fn ui_browser(&mut self, ctx: &egui::Context) {
        // follow the stage (voice, quotation, card, next/previous...)
        let staged = self.parse_ctx.last;
        let on_stage = self.stage.kind == StageKind::Scripture;
        self.browser.follow(self.stage_seq, staged, on_stage);
        if !self.browser.visible {
            // a slim bar so the panel can always be brought back (not just hidden)
            let mut show = false;
            let mut open_history = false;
            egui::TopBottomPanel::bottom("scripture_bar")
                .exact_height(30.0)
                .frame(egui::Frame::none().fill(PANEL).inner_margin(egui::Margin::symmetric(10.0, 4.0)))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("\u{25b2} Show scripture").color(GOLD)).on_hover_text("Bring back the books / chapters / verses browser and search").clicked() {
                            show = true;
                        }
                        if ui.small_button(format!("History ({})", self.timeline.len())).clicked() {
                            open_history = true;
                        }
                        ui.label(RichText::new("press / to search").small().color(MUTED));
                    });
                });
            if show {
                self.browser.visible = true;
                save_settings(self);
            }
            if open_history {
                self.history_visible = true;
            }
            return;
        }
        let bible = self.text_bible();
        let n_books = 66usize;
        self.browser.book = self.browser.book.min(n_books - 1);
        let chapters = bible.chapter_count(self.browser.book).max(1);
        self.browser.chapter = self.browser.chapter.clamp(1, chapters);
        let stage_passage = if on_stage { staged } else { None };

        // search results follow the text box (recomputed only when it changes)
        if self.browser.query != self.browser.results_for {
            self.browser.results = search(bible, &self.browser.query, 40);
            self.browser.results_for = self.browser.query.clone();
        }

        let mut click: Option<(usize, usize, u16, u16)> = None;
        let mut pick_book: Option<usize> = None;
        let mut pick_chapter: Option<(usize, usize)> = None;
        let mut clear_query = false;
        let mut hide = false;
        let mut restage: Option<Passage> = None;
        let scroll = self.browser.scroll_pending;
        let searching = !self.browser.query.trim().is_empty();
        let history = self.history.clone();
        let search_id = egui::Id::new("rf-search");

        egui::TopBottomPanel::bottom("scripture_browser")
            .resizable(true)
            .default_height((ctx.screen_rect().height() * 0.17).clamp(110.0, 240.0))
            .height_range(120.0..=640.0)
            .frame(egui::Frame::none().fill(PANEL).inner_margin(egui::Margin::symmetric(10.0, 6.0)))
            .show(ctx, |ui| {
                // ---- one row: search, recent, plan, hide ----
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("SCRIPTURE").small().color(MUTED).strong());
                    let r = ui.add(
                        TextEdit::singleline(&mut self.browser.query)
                            .id(search_id)
                            .hint_text("search: book, John 3:16, or words like \"god so loved\"   ( / )")
                            .desired_width(330.0),
                    );
                    if self.browser.focus_search {
                        r.request_focus();
                        self.browser.focus_search = false;
                    }
                    if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        clear_query = true;
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        match self.browser.results.first().cloned() {
                            Some(SearchResult::Verse { passage, .. }) => click = Some((passage.book as usize, passage.chapter as usize, passage.from, passage.to)),
                            Some(SearchResult::Book(b)) => pick_book = Some(b),
                            Some(SearchResult::Chapter(b, c)) => pick_chapter = Some((b, c)),
                            None => {}
                        }
                    }
                    if searching && ui.small_button("\u{2716}").on_hover_text("clear search").clicked() {
                        clear_query = true;
                    }
                    if ui.small_button(RichText::new(format!("Plan ({})", self.plan.len())).color(if self.plan.is_empty() { MUTED } else { GOLD })).on_hover_text("Sermon plan: paste the notes, then Space steps through the scriptures").clicked() {
                        self.plan_visible = !self.plan_visible;
                    }
                    if ui.small_button(format!("History ({})", self.timeline.len())).on_hover_text("Every scripture shown, in order: previous / next scripture").clicked() {
                        self.history_visible = !self.history_visible;
                    }
                    if ui.small_button("\u{25bc} Hide").on_hover_text("Hide the panel (a Show bar stays at the bottom)").clicked() {
                        hide = true;
                    }
                });
                // recent scriptures: one click to show again
                if !history.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("recent").small().color(MUTED));
                        for (p, label) in history.iter().take(10) {
                            if ui.small_button(RichText::new(label).color(TEXT)).clicked() {
                                restage = Some(*p);
                            }
                        }
                    });
                }
                ui.add_space(3.0);
                let h = ui.available_height().max(90.0);

                if searching {
                    // ---- results ----
                    ScrollArea::vertical().id_source("br_results").max_height(h).auto_shrink([false, false]).show(ui, |ui| {
                        if self.browser.results.is_empty() {
                            ui.label(RichText::new("nothing found \u{2014} try fewer words, or a book name").color(MUTED));
                        }
                        let bible = bible;
                        for res in self.browser.results.clone() {
                            match res {
                                SearchResult::Book(b) => {
                                    if ui.add_sized([ui.available_width(), 24.0], egui::Button::new(RichText::new(format!("\u{1f4d6}  {}  \u{2014} open book", bible.book_name(b))).color(GOLD))).clicked() {
                                        pick_book = Some(b);
                                    }
                                }
                                SearchResult::Chapter(b, c) => {
                                    if ui.add_sized([ui.available_width(), 24.0], egui::Button::new(RichText::new(format!("\u{1f4d6}  {} {}  \u{2014} open chapter", bible.book_name(b), c)).color(GOLD))).clicked() {
                                        pick_chapter = Some((b, c));
                                    }
                                }
                                SearchResult::Verse { passage, pct, text } => {
                                    let mut job = LayoutJob::default();
                                    job.wrap.max_width = (ui.available_width() - 20.0).max(160.0);
                                    job.append(&format!("{}", passage.label()), 0.0, TextFormat { font_id: FontId::proportional(15.0), color: GOLD, ..Default::default() });
                                    job.append(&format!("   {pct}%   "), 0.0, TextFormat { font_id: FontId::proportional(12.0), color: MUTED, ..Default::default() });
                                    let snippet: String = text.chars().take(170).collect();
                                    job.append(&snippet, 0.0, TextFormat { font_id: FontId::proportional(14.0), color: TEXT, ..Default::default() });
                                    let fr = egui::Frame::none().rounding(4.0).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.add(egui::Label::new(job).sense(Sense::click()))
                                    });
                                    if fr.inner.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                        click = Some((passage.book as usize, passage.chapter as usize, passage.from, passage.to));
                                    }
                                }
                            }
                        }
                    });
                } else {
                    // ---- three columns: books | chapters | verses (each a real vertical list) ----
                    let total_w = ui.available_width();
                    let books_w = 170.0_f32.min(total_w * 0.26);
                    let chap_w = 150.0_f32.min(total_w * 0.22);
                    let verses_w = (total_w - books_w - chap_w - 24.0).max(150.0);
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(egui::vec2(books_w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                            ScrollArea::vertical().id_source("br_books").max_height(h).auto_shrink([false, false]).show(ui, |ui| {
                                for b in 0..n_books {
                                    let sel = b == self.browser.book;
                                    let col = if b < 39 { Color32::from_rgb(0xE0, 0xA8, 0x6A) } else { Color32::from_rgb(0x7F, 0xB4, 0xE6) };
                                    let r = ui.add_sized([ui.available_width(), 22.0], egui::SelectableLabel::new(sel, RichText::new(bible.book_name(b)).color(if sel { TEXT } else { col })));
                                    if sel && scroll {
                                        r.scroll_to_me(Some(egui::Align::Center));
                                    }
                                    if r.clicked() {
                                        pick_book = Some(b);
                                    }
                                }
                            });
                        });
                        ui.separator();
                        ui.allocate_ui_with_layout(egui::vec2(chap_w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                            ScrollArea::vertical().id_source("br_chapters").max_height(h).auto_shrink([false, false]).show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                    for c in 1..=chapters {
                                        let sel = c == self.browser.chapter;
                                        let r = ui.add_sized([32.0, 24.0], egui::SelectableLabel::new(sel, RichText::new(c.to_string()).color(if sel { GOLD } else { TEXT })));
                                        if sel && scroll {
                                            r.scroll_to_me(Some(egui::Align::Center));
                                        }
                                        if r.clicked() {
                                            pick_chapter = Some((self.browser.book, c));
                                        }
                                    }
                                });
                            });
                        });
                        ui.separator();
                        ui.allocate_ui_with_layout(egui::vec2(verses_w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                            ScrollArea::vertical().id_source("br_verses").max_height(h).auto_shrink([false, false]).show(ui, |ui| {
                                let count = bible.verse_count(self.browser.book, self.browser.chapter);
                                let shift = ui.input(|i| i.modifiers.shift);
                                for v in 1..=count as u16 {
                                    let text = bible.verse(self.browser.book, self.browser.chapter, v as usize);
                                    let staged_here = stage_passage.map_or(false, |p| verse_on_stage(&p, self.browser.book, self.browser.chapter, v));
                                    let mut job = LayoutJob::default();
                                    job.wrap.max_width = (verses_w - 56.0).max(120.0);
                                    job.append(&format!("{v}   "), 0.0, TextFormat { font_id: FontId::proportional(15.0), color: Color32::from_rgb(0xE8, 0x4E, 0x8E), ..Default::default() });
                                    job.append(text, 0.0, TextFormat { font_id: FontId::proportional(15.0), color: TEXT, ..Default::default() });
                                    let fill = if staged_here { Color32::from_rgba_unmultiplied(0xD4, 0xA2, 0x4C, 0x38) } else { Color32::TRANSPARENT };
                                    let fr = egui::Frame::none().fill(fill).rounding(4.0).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.add(egui::Label::new(job).sense(Sense::click()))
                                    });
                                    let r = fr.inner;
                                    if staged_here && scroll && stage_passage.map_or(false, |p| p.from == v) {
                                        r.scroll_to_me(Some(egui::Align::Center));
                                    }
                                    if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                        let (from, to) = click_range(self.browser.anchor, v, shift);
                                        click = Some((self.browser.book, self.browser.chapter, from, to));
                                    }
                                }
                            });
                        });
                    });
                }
            });

        self.browser.scroll_pending = false;
        if clear_query {
            self.browser.query.clear();
        }
        if let Some(b) = pick_book {
            self.browser.pick_book(b);
            self.browser.scroll_pending = true;
            self.browser.query.clear();
        }
        if let Some((b, c)) = pick_chapter {
            self.browser.pick_book(b);
            self.browser.chapter = c;
            self.browser.anchor = None;
            self.browser.scroll_pending = true;
            self.browser.query.clear();
        }
        if hide {
            self.browser.visible = false;
            save_settings(self);
        }
        if let Some(p) = restage {
            self.do_stage(p, "browser", 1.0, false);
            self.touches += 1;
        }
        if let Some((b, c, from, to)) = click {
            self.browser.anchor = Some(from);
            self.browser.query.clear();
            self.stage_by_hand(b, c, from, to);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bible() -> &'static rf_bible::Bible {
        rf_bible::Bible::embedded()
    }

    #[test]
    fn plain_click_is_one_verse_and_shift_click_is_a_range_either_way() {
        assert_eq!(click_range(None, 6, false), (6, 6));
        assert_eq!(click_range(Some(3), 6, false), (6, 6));
        assert_eq!(click_range(Some(3), 6, true), (3, 6));
        assert_eq!(click_range(Some(9), 6, true), (6, 9));
        assert_eq!(click_range(None, 6, true), (6, 6));
    }

    #[test]
    fn browser_follows_a_new_stage_once_and_then_leaves_manual_browsing_alone() {
        let mut b = BrowserState::new(true);
        let rev = Passage { book: 65, chapter: 1, from: 6, to: 6 };
        assert!(b.follow(1, Some(rev), true));
        assert_eq!((b.book, b.chapter, b.anchor), (65, 1, Some(6)));
        // the operator browses elsewhere by hand; the same stage does not drag them back
        b.pick_book(0);
        assert!(!b.follow(1, Some(rev), true));
        assert_eq!((b.book, b.chapter), (0, 1));
        // a new stage (even of the same verse) snaps the browser to it
        assert!(b.follow(2, Some(rev), true));
        assert_eq!(b.book, 65);
        // clearing the stage or a non-scripture stage never moves it
        assert!(!b.follow(3, Some(rev), false));
    }

    #[test]
    fn picking_a_book_starts_at_chapter_one_but_picking_the_same_book_keeps_the_chapter() {
        let mut b = BrowserState::new(true);
        b.book = 42;
        b.chapter = 3;
        b.pick_book(42);
        assert_eq!(b.chapter, 3);
        b.pick_book(65);
        assert_eq!((b.book, b.chapter), (65, 1));
    }

    #[test]
    fn highlight_covers_exactly_the_staged_verses() {
        let p = Passage { book: 42, chapter: 3, from: 16, to: 18 };
        assert!(verse_on_stage(&p, 42, 3, 16) && verse_on_stage(&p, 42, 3, 18));
        assert!(!verse_on_stage(&p, 42, 3, 15) && !verse_on_stage(&p, 42, 3, 19));
        assert!(!verse_on_stage(&p, 42, 4, 16) && !verse_on_stage(&p, 41, 3, 16));
    }

    #[test]
    fn typed_references_in_many_spellings() {
        let cases = [
            ("Rev 1:6", (65, 1, 6, 6)),
            ("Revelation 1:6", (65, 1, 6, 6)),
            ("john 3:16", (42, 3, 16, 16)),
            ("John 3:16-18", (42, 3, 16, 18)),
            ("2 Corinthians 5:17", (46, 5, 17, 17)),
            ("1 Corinthians 5:13", (45, 5, 13, 13)),
            ("genesis 1 7", (0, 1, 7, 7)),
            ("Psalm 23:1", (18, 23, 1, 1)),
        ];
        for (text, (b, c, f, t)) in cases {
            let got = reference_from_text(bible(), text);
            assert_eq!(got.map(|p| (p.book, p.chapter, p.from, p.to)), Ok((b, c, f, t)), "{text}");
        }
    }

    #[test]
    fn search_1_sam_is_a_book_not_an_error() {
        let r = search(bible(), "1 sam", 10);
        assert_eq!(r.first(), Some(&SearchResult::Book(8)), "{r:?}");
        let r = search(bible(), "1sam", 10);
        assert_eq!(r.first(), Some(&SearchResult::Book(8)));
        let r = search(bible(), "first samuel", 10);
        assert_eq!(r.first(), Some(&SearchResult::Book(8)));
        // "john" opens John first, then 1/2/3 John are not listed above it
        let r = search(bible(), "john", 10);
        assert_eq!(r.first(), Some(&SearchResult::Book(42)));
        let r = search(bible(), "rev", 10);
        assert_eq!(r.first(), Some(&SearchResult::Book(65)));
    }

    #[test]
    fn search_reference_chapter_and_words() {
        let r = search(bible(), "rev 1:6", 10);
        assert!(matches!(r.first(), Some(SearchResult::Verse { passage, pct: 100, .. }) if (passage.book, passage.chapter, passage.from) == (65, 1, 6)), "{r:?}");
        let r = search(bible(), "john 3", 10);
        assert_eq!(r.first(), Some(&SearchResult::Chapter(42, 3)), "{r:?}");
        let r = search(bible(), "god so loved", 10);
        assert!(matches!(r.first(), Some(SearchResult::Verse { passage, .. }) if (passage.book, passage.chapter, passage.from) == (42, 3, 16)), "{r:?}");
        assert!(r.len() > 1, "other relevant verses are offered too");
        assert!(search(bible(), "   ", 10).is_empty());
        assert!(search(bible(), "zzzzqqq", 10).is_empty());
        // no books are suggested for ordinary words
        assert!(!search(bible(), "god so loved", 10).iter().any(|x| matches!(x, SearchResult::Book(_))));
    }

    #[test]
    fn search_forgives_a_misspelt_book() {
        let r = search(bible(), "revelations", 10);
        assert!(r.contains(&SearchResult::Book(65)), "{r:?}");
    }

    #[test]
    fn bad_references_say_why() {
        assert!(reference_from_text(bible(), "").is_err());
        assert!(reference_from_text(bible(), "1 Corinthians 5:17").unwrap_err().contains("13"), "names the real verse count");
        assert!(reference_from_text(bible(), "John 3:99").is_err());
        assert!(reference_from_text(bible(), "hello there").is_err());
    }
}
