//! A PDF or HTML document on the stage (v0.840.0): one page at a time, zoom, and a reading line.
//!
//! The pages are pictures made by `rf_media::document` on a helper thread (see there). This file keeps the
//! operator's place: which page, how far zoomed, which line the reading line is on. Only the page being shown
//! and its two neighbours are kept in memory as textures, so a 300-page PDF costs about three pictures.

use crate::stage::{ArtImage, DocArt};
use rf_media::document::{self, DocJob, LoadedPage, PageLines};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct PageTex {
    pub index: usize,
    pub tex: egui::TextureHandle,
    pub size: [f32; 2],
    pub lines: PageLines,
}

/// What a key or a button asks of the document on the stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocAction {
    NextPage,
    PrevPage,
    LineDown,
    LineUp,
    ZoomIn,
    ZoomOut,
    ToggleGuide,
}

/// The same keys that move verses and slides move a document, so a hand on the keyboard needs no new habits.
pub fn action_for_command(name: &str) -> Option<DocAction> {
    Some(match name {
        "next_verse" | "next_slide" => DocAction::NextPage,
        "prev_verse" | "prev_slide" => DocAction::PrevPage,
        "next_chapter" => DocAction::LineDown,
        "prev_chapter" => DocAction::LineUp,
        "zoom_in" => DocAction::ZoomIn,
        "zoom_out" => DocAction::ZoomOut,
        "media_toggle" => DocAction::ToggleGuide,
        _ => return None,
    })
}

/// Where the cached pages of `path` live: one folder per file (name, size and date decide which).
pub fn cache_dir(root: &Path, path: &str) -> PathBuf {
    let meta = std::fs::metadata(path).ok();
    let len = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let secs = meta.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let mut h = 0xcbf29ce484222325u64;
    for b in path.bytes().chain(len.to_le_bytes()).chain(secs.to_le_bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    root.join(format!("{h:016x}"))
}

/// Keep the newest `keep` cache folders; the rest (old documents) are deleted. Best effort.
pub fn prune_cache(root: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(root) else { return };
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), e.path()))
        .collect();
    dirs.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in dirs.into_iter().skip(keep) {
        let _ = std::fs::remove_dir_all(p);
    }
}

/// Step the reading line by one, moving to the neighbouring page at either end. Pure, so it can be tested.
/// `lines_here` = lines on the current page. Returns true if the page changed.
pub fn step_line(index: &mut usize, line: &mut usize, want_last: &mut bool, dir: i32, lines_here: usize, pages: usize) -> bool {
    if dir > 0 {
        if *line + 1 < lines_here {
            *line += 1;
            false
        } else if *index + 1 < pages {
            *index += 1;
            *line = 0;
            *want_last = false;
            true
        } else {
            false
        }
    } else if *line > 0 {
        *line = (*line).min(lines_here.saturating_sub(1)).saturating_sub(1);
        false
    } else if *index > 0 {
        *index -= 1;
        *line = 0;
        *want_last = true;
        true
    } else {
        false
    }
}

pub struct DocState {
    pub path: String,
    pub name: String,
    job: DocJob,
    pages: Vec<PathBuf>,
    pub index: usize,
    cache: Vec<PageTex>,
    asked: HashSet<usize>,
    tx: Sender<(usize, Result<LoadedPage, String>)>,
    rx: Receiver<(usize, Result<LoadedPage, String>)>,
    pub zoom: f32,
    pub fit_width: bool,
    pub guide_on: bool,
    pub line: usize,
    /// how many lines the reading line covers at once: 1, 2 or 3 (v0.842.0)
    pub guide_lines: usize,
    /// the highlight colour of the reading line, as rgb (v0.842.0)
    pub guide_color: [u8; 3],
    /// vertical position (0 = top) when the reading line is off
    pub scroll: f32,
    want_last_line: bool,
    pub error: Option<String>,
    /// the error has been told to the operator once (so it is not told again every frame)
    pub error_told: bool,
    /// search inside a PDF (v0.843.0)
    pub search: String,
    pub hits: Vec<rf_media::docsearch::TextHit>,
    pub search_note: String,
    texts: std::sync::Arc<std::sync::Mutex<Option<Result<Vec<String>, String>>>>,
    texts_started: bool,
    texts_have: Option<Vec<String>>,
    searched: String,
    /// a strip of small page pictures (v0.843.0)
    pub show_thumbs: bool,
    thumbs: std::collections::HashMap<usize, egui::TextureHandle>,
    thumb_asked: HashSet<usize>,
    thumb_tx: Sender<(usize, Option<rf_media::Rgba>)>,
    thumb_rx: Receiver<(usize, Option<rf_media::Rgba>)>,
}

impl DocState {
    pub fn open(path: &str, name: &str, job: DocJob, page: usize, fit_width: bool) -> DocState {
        let (tx, rx) = channel();
        let (thumb_tx, thumb_rx) = channel();
        DocState {
            path: path.to_string(),
            name: name.to_string(),
            job,
            pages: Vec::new(),
            index: page,
            cache: Vec::new(),
            asked: HashSet::new(),
            tx,
            rx,
            zoom: 1.0,
            fit_width,
            guide_on: false,
            line: 0,
            guide_lines: 1,
            guide_color: [255, 232, 40],
            scroll: 0.0,
            want_last_line: false,
            error: None,
            error_told: false,
            search: String::new(),
            hits: Vec::new(),
            search_note: String::new(),
            texts: std::sync::Arc::new(std::sync::Mutex::new(None)),
            texts_started: false,
            texts_have: None,
            searched: String::new(),
            show_thumbs: false,
            thumbs: std::collections::HashMap::new(),
            thumb_asked: HashSet::new(),
            thumb_tx,
            thumb_rx,
        }
    }

    /// Search the PDF for `self.search`. The words of the pages are read once, in the background.
    pub fn search_tick(&mut self, ctx: &egui::Context) {
        if self.search.trim().is_empty() {
            self.hits.clear();
            self.search_note.clear();
            self.searched.clear();
            return;
        }
        if !self.path.to_lowercase().ends_with(".pdf") {
            self.search_note = "Searching works for PDFs.".into();
            return;
        }
        if self.texts_have.is_none() {
            if !self.texts_started {
                self.texts_started = true;
                let (slot, path, c2) = (self.texts.clone(), self.path.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let r = rf_media::docsearch::pdf_page_texts(std::path::Path::new(&path));
                    if let Ok(mut g) = slot.lock() {
                        *g = Some(r);
                    }
                    c2.request_repaint();
                });
            }
            let done = self.texts.lock().ok().and_then(|mut g| g.take());
            match done {
                Some(Ok(t)) => self.texts_have = Some(t),
                Some(Err(e)) => {
                    self.search_note = e;
                    self.texts_started = false;
                    return;
                }
                None => {
                    self.search_note = "reading the words of the PDF\u{2026}".into();
                    ctx.request_repaint_after(std::time::Duration::from_millis(150));
                    return;
                }
            }
        }
        if self.searched != self.search {
            self.searched = self.search.clone();
            let t = self.texts_have.as_deref().unwrap_or(&[]);
            self.hits = rf_media::docsearch::search_pages(t, &self.search, 50);
            self.search_note = if self.hits.is_empty() { "No page has those words.".into() } else { format!("{} page(s)", self.hits.len()) };
        }
    }

    /// A small picture of page `i` for the strip. Asks for it in the background the first time.
    pub fn thumb(&mut self, ctx: &egui::Context, i: usize) -> Option<egui::TextureHandle> {
        while let Ok((k, img)) = self.thumb_rx.try_recv() {
            self.thumb_asked.remove(&k);
            if let Some(im) = img {
                let ci = egui::ColorImage::from_rgba_unmultiplied([im.width as usize, im.height as usize], &im.pixels);
                self.thumbs.insert(k, ctx.load_texture(format!("doc-thumb-{k}"), ci, egui::TextureOptions::LINEAR));
            }
        }
        if let Some(t) = self.thumbs.get(&i) {
            return Some(t.clone());
        }
        if i < self.pages.len() && !self.thumb_asked.contains(&i) {
            self.thumb_asked.insert(i);
            let (tx, path, c2) = (self.thumb_tx.clone(), self.pages[i].clone(), ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send((i, rf_media::load_image(&path, 160).ok()));
                c2.request_repaint();
            });
        }
        None
    }

    /// Pages that can be shown now. While a big PDF is still being drawn this grows.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// True while pages are still being made.
    pub fn preparing(&self) -> bool {
        !self.job.is_done()
    }

    pub fn note(&self) -> String {
        self.job.note()
    }

    fn page_tex(&self, i: usize) -> Option<&PageTex> {
        self.cache.iter().find(|p| p.index == i)
    }

    /// The page to draw: the wanted one, or (while it loads) the nearest one already in memory, so the screen never flashes empty.
    fn shown(&self) -> Option<&PageTex> {
        self.page_tex(self.index).or_else(|| self.cache.iter().min_by_key(|p| (p.index as i64 - self.index as i64).abs()))
    }

    pub fn lines_here(&self) -> usize {
        self.page_tex(self.index).map(|p| p.lines.lines.len()).unwrap_or(0)
    }

    /// Call every frame: learn about new pages, collect loaded ones, ask for the ones that are needed.
    pub fn tick(&mut self, ctx: &egui::Context) {
        let ready = self.job.ready();
        if ready != self.pages.len() {
            let mut all = document::list_pages(&self.job.dir);
            all.truncate(ready);
            self.pages = all;
        }
        if self.pages.is_empty() && self.job.is_done() && self.error.is_none() && !self.error_told {
            self.error = Some(self.job.error().unwrap_or_else(|| "No pages could be made from this file.".into()));
        }
        if !self.pages.is_empty() && self.index >= self.pages.len() && self.job.is_done() {
            self.index = self.pages.len() - 1;
        }
        while let Ok((idx, res)) = self.rx.try_recv() {
            self.asked.remove(&idx);
            match res {
                Ok(lp) => {
                    let size = [lp.image.width as f32, lp.image.height as f32];
                    let ci = egui::ColorImage::from_rgba_unmultiplied([lp.image.width as usize, lp.image.height as usize], &lp.image.pixels);
                    let tex = ctx.load_texture(format!("doc-page-{idx}"), ci, egui::TextureOptions::LINEAR);
                    if idx == self.index && self.want_last_line {
                        self.line = lp.lines.lines.len().saturating_sub(1);
                        self.want_last_line = false;
                    }
                    self.cache.retain(|p| p.index != idx);
                    self.cache.push(PageTex { index: idx, tex, size, lines: lp.lines });
                }
                Err(e) => {
                    if idx == self.index && !self.error_told {
                        self.error = Some(e);
                    }
                }
            }
        }
        let keep_from = self.index.saturating_sub(1);
        let keep_to = self.index + 1;
        self.cache.retain(|p| p.index >= keep_from && p.index <= keep_to);
        let mut want = vec![self.index, self.index + 1];
        if self.index > 0 {
            want.push(self.index - 1);
        }
        for k in want {
            if k < self.pages.len() && self.page_tex(k).is_none() && !self.asked.contains(&k) {
                self.asked.insert(k);
                let (tx, path, c2) = (self.tx.clone(), self.pages[k].clone(), ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send((k, document::load_page(&path)));
                    c2.request_repaint();
                });
            }
        }
        if self.preparing() || !self.asked.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
    }

    pub fn goto(&mut self, i: usize) {
        let last = self.pages.len().saturating_sub(1);
        self.index = i.min(last);
        self.line = 0;
        self.scroll = 0.0;
        self.want_last_line = false;
    }

    pub fn act(&mut self, a: DocAction) {
        match a {
            DocAction::NextPage => {
                if self.index + 1 < self.pages.len() {
                    self.goto(self.index + 1);
                }
            }
            DocAction::PrevPage => {
                if self.index > 0 {
                    self.goto(self.index - 1);
                }
            }
            DocAction::LineDown | DocAction::LineUp => {
                if !self.guide_on {
                    self.guide_on = true; // the first press switches the reading line on
                    return;
                }
                let dir = if a == DocAction::LineDown { 1 } else { -1 };
                let (lines_here, pages) = (self.lines_here(), self.pages.len());
                let changed = step_line(&mut self.index, &mut self.line, &mut self.want_last_line, dir, lines_here, pages);
                if changed {
                    self.scroll = 0.0;
                    if self.want_last_line {
                        if let Some(p) = self.page_tex(self.index) {
                            self.line = p.lines.lines.len().saturating_sub(1);
                            self.want_last_line = false;
                        }
                    }
                }
            }
            DocAction::ZoomIn => self.zoom = (self.zoom * 1.25).clamp(0.5, 5.0),
            DocAction::ZoomOut => self.zoom = (self.zoom / 1.25).clamp(0.5, 5.0),
            DocAction::ToggleGuide => self.guide_on = !self.guide_on,
        }
    }

    /// Scroll a zoomed page up or down by a fraction of its height (when the reading line is off).
    pub fn scroll_by(&mut self, d: f32) {
        self.scroll = (self.scroll + d).clamp(0.0, 1.0);
    }

    /// What the stage should draw (None until the first page is in memory). The reading line covers
    /// `guide_lines` lines at once (1 = one line), so the band is the first line's top to the last
    /// line's bottom (v0.842.0).
    pub fn art(&self) -> Option<DocArt> {
        let p = self.shown()?;
        let guide = if self.guide_on && p.index == self.index && !p.lines.lines.is_empty() {
            let n = self.guide_lines.max(1);
            let i = self.line.min(p.lines.lines.len() - 1);
            let j = (i + n - 1).min(p.lines.lines.len() - 1);
            let (a, b) = (p.lines.lines[i], p.lines.lines[j]);
            Some((a.top, b.bottom, p.lines.x0, p.lines.x1))
        } else {
            None
        };
        let cy = guide.map(|g| (g.0 + g.1) / 2.0).unwrap_or(self.scroll);
        Some(DocArt { page: ArtImage { texture: p.tex.id(), size: p.size }, zoom: self.zoom, fit_width: self.fit_width, cx: 0.5, cy, guide, guide_color: self.guide_color })
    }
}

/// A small picture of the first page for the shelf, if its pages have been made.
pub fn first_page_thumb(dir: &Path, max_side: u32) -> Option<rf_media::Rgba> {
    let first = document::list_pages(dir).into_iter().next()?;
    rf_media::load_image(&first, max_side).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reading_line_walks_down_a_page_and_over_to_the_next() {
        let (mut i, mut l, mut w) = (0usize, 0usize, false);
        assert!(!step_line(&mut i, &mut l, &mut w, 1, 3, 2));
        assert!(!step_line(&mut i, &mut l, &mut w, 1, 3, 2));
        assert_eq!((i, l), (0, 2));
        assert!(step_line(&mut i, &mut l, &mut w, 1, 3, 2), "past the last line: next page");
        assert_eq!((i, l), (1, 0));
        assert!(!step_line(&mut i, &mut l, &mut w, 1, 1, 2) || (i, l) == (1, 0));
        assert_eq!((i, l), (1, 0), "the very last line stays put");
    }

    #[test]
    fn the_reading_line_walks_back_up_to_the_last_line_of_the_page_before() {
        let (mut i, mut l, mut w) = (1usize, 1usize, false);
        assert!(!step_line(&mut i, &mut l, &mut w, -1, 4, 3));
        assert_eq!((i, l), (1, 0));
        assert!(step_line(&mut i, &mut l, &mut w, -1, 4, 3));
        assert_eq!(i, 0);
        assert!(w, "the page before has to say how many lines it has");
        let (mut i, mut l, mut w) = (0usize, 0usize, false);
        assert!(!step_line(&mut i, &mut l, &mut w, -1, 4, 3), "the very first line stays put");
    }

    #[test]
    fn the_same_keys_that_move_verses_move_a_document() {
        assert_eq!(action_for_command("next_verse"), Some(DocAction::NextPage));
        assert_eq!(action_for_command("prev_slide"), Some(DocAction::PrevPage));
        assert_eq!(action_for_command("next_chapter"), Some(DocAction::LineDown));
        assert_eq!(action_for_command("prev_chapter"), Some(DocAction::LineUp));
        assert_eq!(action_for_command("media_toggle"), Some(DocAction::ToggleGuide));
        assert_eq!(action_for_command("blackout"), None);
    }

    #[test]
    fn each_file_gets_its_own_cache_folder() {
        let root = std::path::Path::new("/tmp/x");
        assert_ne!(cache_dir(root, "/a/b.pdf"), cache_dir(root, "/a/c.pdf"));
        assert_eq!(cache_dir(root, "/a/b.pdf"), cache_dir(root, "/a/b.pdf"));
    }
}
