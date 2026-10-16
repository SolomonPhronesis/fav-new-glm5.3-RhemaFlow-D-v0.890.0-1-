//! Documents on the stage (v0.840.0): a PDF or an HTML file becomes a stack of PAGE PICTURES that the
//! stage shows one at a time (with zoom, and a reading line that moves down the page).
//!
//! * PDF: the pages are drawn by a helper program that is already on the Mac or easy to get: `pdftoppm`
//!   (the free "poppler" tools), `mutool`, the Mac's own Swift + CoreGraphics (nothing to install), Ghostscript,
//!   or, as the last resort, Quick Look (first page only). RhemaFlow only STARTS these programs; it does not link
//!   to them, so their licences never mix with ours.
//! * HTML: RhemaFlow reads the text, headings, lists and bold of the page itself and sets it into A4 pages with
//!   its own serif font (pictures and style sheets inside the HTML are not drawn: this is for notes and
//!   bulletins, not for web sites).
//! * `detect_lines` finds the lines of text on any page picture, so the "reading line" can step from line to line
//!   on a PDF, a scan or an HTML page alike.
//!
//! No UI here, and nothing in this file needs a screen.

use crate::Rgba;
use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const PDF_INSTALL_HINT: &str = "Only the first page could be drawn. For every page, install the free poppler tools once (in Terminal: brew install poppler), then show the file again.";

/// Width in pixels of a page picture (about 1900 is sharp on a 1080p screen even when zoomed to fit the width).
pub const PAGE_PX: u32 = 1900;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocKind {
    Pdf,
    Html,
}

/// By file extension.
pub fn doc_kind(path: &str) -> Option<DocKind> {
    let ext = match path.rsplit_once('.') {
        Some((_, e)) if !e.contains('/') && !e.contains('\\') => e.to_ascii_lowercase(),
        _ => return None,
    };
    match ext.as_str() {
        "pdf" => Some(DocKind::Pdf),
        "html" | "htm" | "xhtml" => Some(DocKind::Html),
        _ => None,
    }
}

// ------------------------------------------------------------------------------------------------
// lines of text on a page picture
// ------------------------------------------------------------------------------------------------

/// One line of text on a page, as fractions of the page height (0.0 = top edge, 1.0 = bottom edge).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineBand {
    pub top: f32,
    pub bottom: f32,
}

/// The lines found on a page, top to bottom, and where the text starts and ends across the page (fractions of the width).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageLines {
    pub lines: Vec<LineBand>,
    pub x0: f32,
    pub x1: f32,
}

/// Find the lines of text on a page picture. The page background is whatever colour covers most of the page
/// (white paper, or a dark page), and anything clearly different from it counts as ink.
pub fn detect_lines(img: &Rgba) -> PageLines {
    let (w, h) = (img.width as usize, img.height as usize);
    if w < 8 || h < 8 || img.pixels.len() < w * h * 4 {
        return PageLines::default();
    }
    let lum = |i: usize| -> i32 {
        let p = &img.pixels[i * 4..i * 4 + 4];
        (p[0] as i32 * 299 + p[1] as i32 * 587 + p[2] as i32 * 114) / 1000
    };
    // the background: the most common brightness (32 coarse steps)
    let mut hist = [0u32; 32];
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            hist[(lum(y * w + x) >> 3).clamp(0, 31) as usize] += 1;
        }
    }
    let mut best = 31usize;
    let mut best_n = 0u32;
    for (i, n) in hist.iter().enumerate() {
        if *n > best_n {
            best_n = *n;
            best = i;
        }
    }
    let bg = best as i32 * 8 + 4;
    let thr = 70;
    let min_ink = (w / 600).max(2) as u32;
    let (mut col_min, mut col_max) = (w, 0usize);
    let mut inked = vec![false; h];
    let mut row_ink = vec![0u32; h]; // how many ink pixels each row has (for the paragraph split)
    for y in 0..h {
        let mut n = 0u32;
        for x in 0..w {
            if (lum(y * w + x) - bg).abs() > thr {
                n += 1;
                if x < col_min {
                    col_min = x;
                }
                if x > col_max {
                    col_max = x;
                }
            }
        }
        row_ink[y] = n;
        inked[y] = n >= min_ink;
    }
    // runs of inked rows; small gaps (the dot of an i, a thin letter) do not split a line
    let gap_tol = (h / 400).max(1);
    let mut bands: Vec<(usize, usize)> = Vec::new(); // inclusive rows
    let mut y = 0;
    while y < h {
        if !inked[y] {
            y += 1;
            continue;
        }
        let start = y;
        let mut end = y;
        let mut gap = 0;
        while y < h {
            if inked[y] {
                end = y;
                gap = 0;
            } else {
                gap += 1;
                if gap > gap_tol {
                    break;
                }
            }
            y += 1;
        }
        bands.push((start, end));
    }
    if bands.is_empty() {
        return PageLines::default();
    }
    // drop hair-lines and specks. First an absolute floor that no real text line falls beneath (a
    // page number rule, an underline), then the "much thinner than the typical line" test. The
    // typical line is the quarter-line instead of the median, so one merged paragraph cannot make
    // every real line look like a speck (that is exactly what went wrong when a paragraph came out
    // as one band: the median grew, and the real lines around it were dropped).
    let floor = (h / 200).max(3);
    bands.retain(|b| b.1 - b.0 + 1 >= floor);
    if bands.is_empty() {
        return PageLines::default();
    }
    let mut heights: Vec<usize> = bands.iter().map(|b| b.1 - b.0 + 1).collect();
    heights.sort_unstable();
    let line_h = heights[heights.len() / 4].max(floor);
    bands.retain(|b| {
        let bh = b.1 - b.0 + 1;
        bh * 3 >= line_h
    });
    if bands.is_empty() {
        return PageLines::default();
    }
    // Wrapped lines inside a paragraph can sit so close that no empty row separates them (their
    // descenders and the next line's ascenders share rows), so a whole paragraph comes out as one
    // band. Such a band is cut at its lightest rows — the thin gaps between wrapped lines have far
    // fewer ink pixels than the lines themselves. A very tall band with no light rows at all (a
    // block of solid ink) is cut at an even pitch instead. The light rows at a band's own top and
    // bottom belong to the first or last line (their ascenders and descenders), so they are trimmed
    // away first: only the light runs BETWEEN lines of text can cut, and a single line stays whole.
    let mut parts: Vec<(usize, usize)> = Vec::new();
    for (t, b) in bands {
        let bh = b - t + 1;
        let peak = (t..=b).map(|y| row_ink[y]).max().unwrap_or(1).max(1);
        let light = |y: usize| row_ink[y] * 100 < peak * 30;
        let (mut t2, mut b2) = (t, b);
        while t2 < b2 && light(t2) {
            t2 += 1;
        }
        while b2 > t2 && light(b2) {
            b2 -= 1;
        }
        let mut valleys: Vec<usize> = Vec::new(); // centres of runs of rows with little ink
        let mut y = t2;
        while y <= b2 {
            if light(y) {
                let run = y;
                while y <= b2 && light(y) {
                    y += 1;
                }
                valleys.push(run + (y - run) / 2);
            } else {
                y += 1;
            }
        }
        let tall = bh * 2 >= line_h * 3; // could hold more than one wrapped line
        let cuts: Vec<usize> = if valleys.len() >= 2 || (valleys.len() == 1 && tall) {
            valleys.into_iter().filter(|c| *c > t && *c < b).collect()
        } else if tall && bh >= line_h * 5 {
            let k = ((bh as f32) / (line_h as f32 * 1.5)).round().max(2.0) as usize;
            (1..k).map(|i| t + i * bh / k).collect()
        } else {
            Vec::new()
        };
        if cuts.is_empty() {
            parts.push((t, b));
        } else {
            let mut prev = t;
            for c in cuts {
                if c > prev && c < b {
                    parts.push((prev, c - 1));
                    prev = c;
                }
            }
            if prev <= b {
                parts.push((prev, b));
            }
        }
    }
    if parts.is_empty() {
        return PageLines::default();
    }
    let n = parts.len();
    let mut lines = Vec::with_capacity(n);
    for i in 0..n {
        let (t, b) = (parts[i].0 as f32, parts[i].1 as f32 + 1.0);
        let prev_b = if i > 0 { parts[i - 1].1 as f32 + 1.0 } else { 0.0 };
        let next_t = if i + 1 < n { parts[i + 1].0 as f32 } else { h as f32 };
        let pad = (b - t) * 0.25;
        let top = t - pad.min((t - prev_b) / 2.0).max(0.0);
        let bottom = b + pad.min((next_t - b) / 2.0).max(0.0);
        lines.push(LineBand { top: top / h as f32, bottom: bottom / h as f32 });
    }
    let margin = w as f32 * 0.012;
    let x0 = ((col_min as f32 - margin).max(0.0)) / w as f32;
    let x1 = ((col_max as f32 + 1.0 + margin).min(w as f32)) / w as f32;
    PageLines { lines, x0, x1 }
}

// ------------------------------------------------------------------------------------------------
// HTML -> blocks
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    H1,
    H2,
    H3,
    Para,
    Item,
    Rule,
}

/// One paragraph, heading or list item: runs of (text, bold).
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub kind: BlockKind,
    pub runs: Vec<(String, bool)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedHtml {
    pub title: String,
    pub blocks: Vec<Block>,
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let semi = tail.char_indices().take(12).find(|(_, c)| *c == ';').map(|(i, _)| i);
        let mut done = false;
        if let Some(sc) = semi {
            let name = &tail[1..sc];
            let rep: Option<String> = match name {
                "amp" => Some("&".into()),
                "lt" => Some("<".into()),
                "gt" => Some(">".into()),
                "quot" => Some("\"".into()),
                "apos" => Some("'".into()),
                "nbsp" => Some(" ".into()),
                "ndash" => Some("\u{2013}".into()),
                "mdash" => Some("\u{2014}".into()),
                "hellip" => Some("\u{2026}".into()),
                "rsquo" => Some("\u{2019}".into()),
                "lsquo" => Some("\u{2018}".into()),
                "rdquo" => Some("\u{201d}".into()),
                "ldquo" => Some("\u{201c}".into()),
                "copy" => Some("\u{a9}".into()),
                "reg" => Some("\u{ae}".into()),
                "trade" => Some("\u{2122}".into()),
                "bull" => Some("\u{2022}".into()),
                "middot" => Some("\u{b7}".into()),
                "laquo" => Some("\u{ab}".into()),
                "raquo" => Some("\u{bb}".into()),
                _ => {
                    if let Some(num) = name.strip_prefix('#') {
                        let code = if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) { u32::from_str_radix(hex, 16).ok() } else { num.parse::<u32>().ok() };
                        code.and_then(char::from_u32).map(|c| c.to_string())
                    } else {
                        None
                    }
                }
            };
            if let Some(r) = rep {
                out.push_str(&r);
                rest = &tail[sc + 1..];
                done = true;
            }
        }
        if !done {
            out.push('&');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

struct Parser {
    blocks: Vec<Block>,
    runs: Vec<(String, bool)>,
    kind: BlockKind,
    has_text: bool,
    bold: u32,
    skip: Option<String>,
    lists: Vec<(bool, u32)>,
    title: String,
    in_title: bool,
}

impl Parser {
    fn flush(&mut self) {
        let mut runs = std::mem::take(&mut self.runs);
        if let Some(f) = runs.first_mut() {
            f.0 = f.0.trim_start().to_string();
        }
        if let Some(l) = runs.last_mut() {
            l.0 = l.0.trim_end().to_string();
        }
        runs.retain(|r| !r.0.is_empty());
        if self.has_text && !runs.is_empty() {
            self.blocks.push(Block { kind: self.kind, runs });
        }
        self.has_text = false;
        self.kind = BlockKind::Para;
    }

    fn open(&mut self, kind: BlockKind) {
        self.flush();
        self.kind = kind;
    }

    fn text(&mut self, raw: &str) {
        if self.skip.is_some() {
            return;
        }
        let t = decode_entities(raw);
        if self.in_title {
            self.title.push_str(t.trim());
            return;
        }
        let mut last_space = self.runs.last().map_or(true, |r| r.0.ends_with(' ') || r.0.ends_with('\n'));
        let mut s = String::new();
        for ch in t.chars() {
            if ch.is_whitespace() {
                if !last_space {
                    s.push(' ');
                    last_space = true;
                }
            } else {
                s.push(ch);
                last_space = false;
            }
        }
        if s.is_empty() {
            return;
        }
        if s.chars().any(|c| !c.is_whitespace()) {
            self.has_text = true;
        }
        let bold = self.bold > 0;
        match self.runs.last_mut() {
            Some(l) if l.1 == bold && l.0 != "\n" => l.0.push_str(&s),
            _ => self.runs.push((s, bold)),
        }
    }

    fn tag(&mut self, inner: &str) {
        let inner = inner.trim();
        let closing = inner.starts_with('/');
        let body = inner.trim_start_matches('/');
        let name: String = body.chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
        if name.is_empty() {
            return;
        }
        if let Some(s) = &self.skip {
            if closing && *s == name {
                self.skip = None;
            }
            return;
        }
        if !closing && matches!(name.as_str(), "script" | "style" | "noscript" | "template") && !inner.ends_with('/') {
            self.skip = Some(name);
            return;
        }
        match (name.as_str(), closing) {
            ("title", c) => self.in_title = !c,
            ("h1", false) => self.open(BlockKind::H1),
            ("h2", false) => self.open(BlockKind::H2),
            ("h3" | "h4" | "h5" | "h6", false) => self.open(BlockKind::H3),
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", true) => self.flush(),
            ("p" | "div" | "section" | "article" | "header" | "footer" | "main" | "nav" | "aside" | "blockquote" | "pre" | "figure" | "figcaption" | "table" | "tr" | "form" | "fieldset" | "dd" | "dt", false) => self.open(BlockKind::Para),
            ("p" | "div" | "section" | "article" | "header" | "footer" | "main" | "nav" | "aside" | "blockquote" | "pre" | "figure" | "figcaption" | "table" | "tr" | "form" | "fieldset" | "dd" | "dt", true) => self.flush(),
            ("ul", false) => {
                self.flush();
                self.lists.push((false, 0));
            }
            ("ol", false) => {
                self.flush();
                self.lists.push((true, 0));
            }
            ("ul" | "ol", true) => {
                self.flush();
                self.lists.pop();
            }
            ("li", false) => {
                self.flush();
                self.kind = BlockKind::Item;
                let mark = match self.lists.last_mut() {
                    Some((true, n)) => {
                        *n += 1;
                        format!("{}.  ", n)
                    }
                    _ => "\u{2022}  ".to_string(),
                };
                self.runs.push((mark, false));
            }
            ("li", true) => self.flush(),
            ("br", _) => self.runs.push(("\n".into(), false)),
            ("hr", _) => {
                self.flush();
                self.blocks.push(Block { kind: BlockKind::Rule, runs: Vec::new() });
            }
            ("b" | "strong", false) => self.bold += 1,
            ("b" | "strong", true) => self.bold = self.bold.saturating_sub(1),
            ("td" | "th", true) => self.runs.push(("   ".into(), false)),
            _ => {}
        }
    }
}

/// Read the text structure of an HTML page: headings, paragraphs, list items, rules, bold.
pub fn parse_html(src: &str) -> ParsedHtml {
    let mut p = Parser { blocks: Vec::new(), runs: Vec::new(), kind: BlockKind::Para, has_text: false, bold: 0, skip: None, lists: Vec::new(), title: String::new(), in_title: false };
    let mut i = 0usize;
    while i < src.len() {
        if src.as_bytes()[i] == b'<' {
            if src[i..].starts_with("<!--") {
                i = src[i..].find("-->").map(|k| i + k + 3).unwrap_or(src.len());
                continue;
            }
            let end = match src[i..].find('>') {
                Some(k) => i + k,
                None => break,
            };
            p.tag(&src[i + 1..end]);
            i = end + 1;
        } else {
            let next = src[i..].find('<').map(|k| i + k).unwrap_or(src.len());
            p.text(&src[i..next]);
            i = next;
        }
    }
    p.flush();
    ParsedHtml { title: p.title, blocks: p.blocks }
}

// ------------------------------------------------------------------------------------------------
// blocks -> A4 page pictures
// ------------------------------------------------------------------------------------------------

const PAGE_W: u32 = 1240;
const PAGE_H: u32 = 1754;
const MARGIN: f32 = 120.0;
const REGULAR: &[u8] = include_bytes!("../../rf-app/assets/LiberationSerif-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../../rf-app/assets/LiberationSerif-Bold.ttf");
const INK: [u8; 3] = [0x1c, 0x1a, 0x17];

struct Canvas {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new() -> Canvas {
        Canvas { w: PAGE_W, h: PAGE_H, px: vec![255u8; (PAGE_W * PAGE_H * 4) as usize] }
    }

    fn blend(&mut self, x: i32, y: i32, c: f32, col: [u8; 3]) {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        for k in 0..3 {
            let d = self.px[i + k] as f32;
            self.px[i + k] = (d * (1.0 - c) + col[k] as f32 * c).round().clamp(0.0, 255.0) as u8;
        }
    }

    fn text(&mut self, font: &FontRef<'static>, x: f32, baseline: f32, size: f32, text: &str) -> f32 {
        let scale = PxScale::from(size);
        let sc = font.as_scaled(scale);
        let mut cx = x;
        for ch in text.chars() {
            let id = sc.glyph_id(ch);
            let glyph = id.with_scale_and_position(scale, point(cx, baseline));
            if let Some(og) = font.outline_glyph(glyph) {
                let bb = og.px_bounds();
                let (bx, by) = (bb.min.x as i32, bb.min.y as i32);
                og.draw(|gx, gy, cov| {
                    if cov > 0.01 {
                        self.blend(bx + gx as i32, by + gy as i32, cov, INK);
                    }
                });
            }
            cx += sc.h_advance(id);
        }
        cx
    }

    fn rule(&mut self, y: i32) {
        for dy in 0..2 {
            for x in (MARGIN as i32)..(self.w as i32 - MARGIN as i32) {
                self.blend(x, y + dy, 1.0, [0x9a, 0x96, 0x8e]);
            }
        }
    }

    fn into_rgba(self) -> Rgba {
        Rgba { width: self.w, height: self.h, pixels: self.px }
    }
}

struct Look {
    size: f32,
    bold: bool,
    before: f32,
    after: f32,
    indent: f32,
}

fn look_of(kind: BlockKind) -> Look {
    match kind {
        BlockKind::H1 => Look { size: 64.0, bold: true, before: 30.0, after: 20.0, indent: 0.0 },
        BlockKind::H2 => Look { size: 50.0, bold: true, before: 24.0, after: 14.0, indent: 0.0 },
        BlockKind::H3 => Look { size: 42.0, bold: true, before: 18.0, after: 10.0, indent: 0.0 },
        BlockKind::Item => Look { size: 38.0, bold: false, before: 0.0, after: 9.0, indent: 30.0 },
        _ => Look { size: 38.0, bold: false, before: 0.0, after: 20.0, indent: 0.0 },
    }
}

/// Set parsed HTML into A4 pages (always at least one).
pub fn typeset(parsed: &ParsedHtml) -> Vec<Rgba> {
    let regular = FontRef::try_from_slice(REGULAR).expect("bundled font");
    let bold = FontRef::try_from_slice(BOLD).expect("bundled font");
    let width_of = |text: &str, is_bold: bool, size: f32| -> f32 {
        let f = if is_bold { &bold } else { &regular };
        let sc = f.as_scaled(PxScale::from(size));
        text.chars().map(|c| sc.h_advance(sc.glyph_id(c))).sum()
    };
    let bottom = PAGE_H as f32 - MARGIN;
    let mut pages: Vec<Rgba> = Vec::new();
    let mut cv = Canvas::new();
    let mut y = MARGIN;
    let mut at_top = true;
    macro_rules! new_page {
        () => {{
            let old = std::mem::replace(&mut cv, Canvas::new());
            pages.push(old.into_rgba());
            y = MARGIN;
            at_top = true;
        }};
    }
    let content_w = PAGE_W as f32 - 2.0 * MARGIN;
    for block in &parsed.blocks {
        if block.kind == BlockKind::Rule {
            if y + 34.0 > bottom {
                new_page!();
            }
            cv.rule((y + 16.0) as i32);
            y += 34.0;
            at_top = false;
            continue;
        }
        let look = look_of(block.kind);
        // words, with a forced break where the HTML had a <br>
        let mut words: Vec<(String, bool)> = Vec::new();
        for (text, b) in &block.runs {
            if text == "\n" {
                words.push(("\n".into(), false));
                continue;
            }
            for w in text.split(' ').filter(|w| !w.is_empty()) {
                words.push((w.to_string(), *b || look.bold));
            }
        }
        if words.is_empty() {
            continue;
        }
        let max_w = content_w - look.indent;
        let space = width_of(" ", false, look.size);
        let mut lines: Vec<Vec<(String, bool)>> = Vec::new();
        let mut cur: Vec<(String, bool)> = Vec::new();
        let mut cur_w = 0.0f32;
        for (w, b) in words {
            if w == "\n" {
                lines.push(std::mem::take(&mut cur));
                cur_w = 0.0;
                continue;
            }
            let ww = width_of(&w, b, look.size);
            let need = if cur.is_empty() { ww } else { cur_w + space + ww };
            if need > max_w && !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
                cur_w = ww;
                cur.push((w, b));
            } else {
                cur_w = need;
                cur.push((w, b));
            }
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
        let lh = look.size * 1.38;
        if !at_top {
            y += look.before;
        }
        // a heading is never left alone at the bottom of a page
        let keep = if matches!(block.kind, BlockKind::H1 | BlockKind::H2 | BlockKind::H3) { lh * 3.0 } else { lh };
        if y + keep > bottom {
            new_page!();
        }
        for line in &lines {
            if y + lh > bottom {
                new_page!();
            }
            let sc = regular.as_scaled(PxScale::from(look.size));
            let baseline = y + sc.ascent() + (lh - look.size) * 0.5;
            let mut x = MARGIN + look.indent;
            if block.kind == BlockKind::Item {
                x -= look.indent * 0.0; // the bullet is the first word, already part of the line
            }
            for (i, (w, b)) in line.iter().enumerate() {
                if i > 0 {
                    x += space;
                }
                let f = if *b { &bold } else { &regular };
                x = cv.text(f, x, baseline, look.size, w);
            }
            y += lh;
            at_top = false;
        }
        y += look.after;
    }
    pages.push(cv.into_rgba());
    if pages.len() > 1 {
        // drop a last page that holds nothing (only possible when a page break came right at the end)
        let last = pages.last().map(|p| p.pixels.chunks(4).all(|px| px[0] == 255 && px[1] == 255 && px[2] == 255)).unwrap_or(false);
        if last {
            pages.pop();
        }
    }
    pages
}

// ------------------------------------------------------------------------------------------------
// the page files and the background job
// ------------------------------------------------------------------------------------------------

/// `page-0001.png`, `page-1.png`, `page-01.png` ... in page order (numbers compared as numbers).
pub fn list_pages(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<(u32, PathBuf)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if let Some(rest) = name.strip_prefix("page-").and_then(|r| r.strip_suffix(".png")) {
                if let Ok(n) = rest.parse::<u32>() {
                    v.push((n, p));
                }
            }
        }
    }
    v.sort_by_key(|(n, _)| *n);
    v.into_iter().map(|(_, p)| p).collect()
}

/// A page picture and the lines found on it.
pub struct LoadedPage {
    pub image: Rgba,
    pub lines: PageLines,
}

pub fn load_page(path: &Path) -> Result<LoadedPage, String> {
    let image = crate::load_image(path, crate::MAX_IMAGE_SIDE)?;
    let lines = detect_lines(&image);
    Ok(LoadedPage { image, lines })
}

/// Where a helper program might be. A Finder-launched app does not see the shell's PATH, so Homebrew's folders are tried by name.
pub fn find_tool(name: &str) -> Option<PathBuf> {
    let mut places: Vec<PathBuf> = vec![PathBuf::from("/opt/homebrew/bin").join(name), PathBuf::from("/usr/local/bin").join(name), PathBuf::from("/usr/bin").join(name)];
    if let Ok(path) = std::env::var("PATH") {
        for d in std::env::split_paths(&path) {
            places.push(d.join(name));
        }
    }
    places.into_iter().find(|p| p.is_file())
}

struct JobState {
    ready: AtomicUsize,
    done: AtomicBool,
    error: Mutex<Option<String>>,
    note: Mutex<String>,
}

/// A document being turned into pages on a helper thread. Cheap to clone; every clone sees the same progress.
#[derive(Clone)]
pub struct DocJob {
    pub dir: PathBuf,
    state: Arc<JobState>,
}

impl DocJob {
    /// Pages that are finished and safe to open.
    pub fn ready(&self) -> usize {
        self.state.ready.load(Ordering::Relaxed)
    }
    pub fn is_done(&self) -> bool {
        self.state.done.load(Ordering::Relaxed)
    }
    pub fn error(&self) -> Option<String> {
        self.state.error.lock().ok().and_then(|e| e.clone())
    }
    /// A plain-words remark for the operator (for example "only the first page could be drawn").
    pub fn note(&self) -> String {
        self.state.note.lock().map(|n| n.clone()).unwrap_or_default()
    }
}

/// Start (or pick up) the page job for `src`; the pages go in `dir`. If `dir` already holds a finished job it is reused.
pub fn start_job(src: &Path, dir: &Path) -> DocJob {
    let state = Arc::new(JobState { ready: AtomicUsize::new(0), done: AtomicBool::new(false), error: Mutex::new(None), note: Mutex::new(String::new()) });
    let job = DocJob { dir: dir.to_path_buf(), state: state.clone() };
    let (src, dir) = (src.to_path_buf(), dir.to_path_buf());
    std::thread::spawn(move || {
        let finish = |res: Result<String, String>| {
            match res {
                Ok(note) => {
                    let n = list_pages(&dir).len();
                    state.ready.store(n, Ordering::Relaxed);
                    if let Ok(mut s) = state.note.lock() {
                        *s = note;
                    }
                    if n == 0 {
                        if let Ok(mut e) = state.error.lock() {
                            *e = Some("No pages could be made from this file.".into());
                        }
                    } else if note_is_clean(&state) {
                        let _ = std::fs::write(dir.join("done.txt"), n.to_string());
                    }
                }
                Err(e) => {
                    if let Ok(mut s) = state.error.lock() {
                        *s = Some(e);
                    }
                }
            }
            state.done.store(true, Ordering::Relaxed);
        };
        if let Ok(txt) = std::fs::read_to_string(dir.join("done.txt")) {
            if let Ok(n) = txt.trim().parse::<usize>() {
                if n > 0 && list_pages(&dir).len() == n {
                    finish(Ok(String::new()));
                    return;
                }
            }
        }
        let _ = std::fs::create_dir_all(&dir);
        clear_pages(&dir);
        let res = match doc_kind(&src.to_string_lossy()) {
            Some(DocKind::Html) => render_html(&src, &dir).map(|_| String::new()),
            Some(DocKind::Pdf) => render_pdf(&src, &dir, &state.ready),
            None => Err("That is not a PDF or an HTML file.".to_string()),
        };
        finish(res);
    });
    job
}

fn note_is_clean(state: &JobState) -> bool {
    state.note.lock().map(|n| n.is_empty()).unwrap_or(true)
}

fn clear_pages(dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("page-") || n.ends_with(".png") || n == "done.txt" {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

fn render_html(src: &Path, dir: &Path) -> Result<usize, String> {
    let meta = std::fs::metadata(src).map_err(|e| format!("Cannot open this file ({e})."))?;
    if meta.len() > 8 * 1024 * 1024 {
        return Err("This HTML file is over 8 MB. Please use a smaller one.".into());
    }
    let bytes = std::fs::read(src).map_err(|e| format!("Cannot read this file ({e})."))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let parsed = parse_html(&text);
    let pages = typeset(&parsed);
    for (i, p) in pages.iter().enumerate() {
        let rgb: Vec<u8> = p.pixels.chunks(4).flat_map(|px| [px[0], px[1], px[2]]).collect();
        let path = dir.join(format!("page-{:04}.png", i + 1));
        image::save_buffer(&path, &rgb, p.width, p.height, image::ColorType::Rgb8).map_err(|e| format!("Could not save a page ({e})."))?;
    }
    Ok(pages.len())
}

/// Run a helper that writes `page-*.png` into `dir`; `ready` follows its progress (a page counts once the next one has begun).
fn run_polling(cmd: &mut Command, dir: &Path, ready: &AtomicUsize) -> Result<(), String> {
    let log = std::fs::File::create(dir.join("helper.log")).map_err(|e| e.to_string())?;
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::from(log)).spawn().map_err(|e| format!("could not start ({e})"))?;
    let t0 = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                ready.store(list_pages(dir).len(), Ordering::Relaxed);
                if status.success() {
                    return Ok(());
                }
                let tail = std::fs::read_to_string(dir.join("helper.log")).unwrap_or_default();
                let tail: String = tail.lines().rev().take(2).collect::<Vec<_>>().join(" ");
                return Err(format!("stopped with {status} {tail}"));
            }
            Ok(None) => {
                let n = list_pages(dir).len();
                ready.store(n.saturating_sub(1), Ordering::Relaxed);
                if t0.elapsed() > Duration::from_secs(900) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("took too long".into());
                }
                std::thread::sleep(Duration::from_millis(80));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

const SWIFT_PDF: &str = r#"import Foundation
import CoreGraphics
import ImageIO

let args = CommandLine.arguments
if args.count < 4 { exit(2) }
let inURL = URL(fileURLWithPath: args[1]) as CFURL
let outDir = args[2]
let targetW = CGFloat(Double(args[3]) ?? 1900.0)
guard let doc = CGPDFDocument(inURL) else { exit(3) }
let n = doc.numberOfPages
if n < 1 { exit(4) }
for i in 1...n {
    guard let page = doc.page(at: i) else { continue }
    let box = page.getBoxRect(.cropBox)
    var pw = box.width
    var ph = box.height
    let rot = ((Int(page.rotationAngle) % 360) + 360) % 360
    if rot == 90 || rot == 270 { let t = pw; pw = ph; ph = t }
    if pw < 1 || ph < 1 { continue }
    let scale = targetW / pw
    let w = Int((pw * scale).rounded())
    let h = Int((ph * scale).rounded())
    guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { continue }
    ctx.setFillColor(red: 1, green: 1, blue: 1, alpha: 1)
    ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
    let t = page.getDrawingTransform(.cropBox, rect: CGRect(x: 0, y: 0, width: w, height: h), rotate: 0, preserveAspectRatio: true)
    ctx.concatenate(t)
    ctx.drawPDFPage(page)
    guard let img = ctx.makeImage() else { continue }
    let name = outDir + "/page-" + String(format: "%04d", i) + ".png"
    guard let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: name) as CFURL, "public.png" as CFString, 1, nil) else { continue }
    CGImageDestinationAddImage(dest, img, nil)
    CGImageDestinationFinalize(dest)
}
"#;

fn mac_command_line_tools() -> bool {
    cfg!(target_os = "macos") && Command::new("xcode-select").arg("-p").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn render_pdf(src: &Path, dir: &Path, ready: &AtomicUsize) -> Result<String, String> {
    let s = src.to_string_lossy().to_string();
    let d = dir.to_string_lossy().to_string();
    let width = PAGE_PX.to_string();
    let mut tried: Vec<String> = Vec::new();

    if let Some(bin) = find_tool("pdftoppm") {
        let mut c = Command::new(bin);
        c.args(["-png", "-scale-to-x", &width, "-scale-to-y", "-1", &s, &format!("{d}/page")]);
        match run_polling(&mut c, dir, ready) {
            Ok(()) if !list_pages(dir).is_empty() => return Ok(String::new()),
            Ok(()) => tried.push("pdftoppm made no pages".into()),
            Err(e) => tried.push(format!("pdftoppm {e}")),
        }
        clear_pages(dir);
    }
    if let Some(bin) = find_tool("mutool") {
        let mut c = Command::new(bin);
        c.args(["draw", "-q", "-F", "png", "-w", &width, "-o", &format!("{d}/page-%04d.png"), &s, "1-N"]);
        match run_polling(&mut c, dir, ready) {
            Ok(()) if !list_pages(dir).is_empty() => return Ok(String::new()),
            Ok(()) => tried.push("mutool made no pages".into()),
            Err(e) => tried.push(format!("mutool {e}")),
        }
        clear_pages(dir);
    }
    if mac_command_line_tools() {
        let script = dir.join("pdf2png.swift");
        if std::fs::write(&script, SWIFT_PDF).is_ok() {
            let mut c = Command::new("/usr/bin/swift");
            c.arg(&script).arg(&s).arg(&d).arg(&width);
            match run_polling(&mut c, dir, ready) {
                Ok(()) if !list_pages(dir).is_empty() => return Ok(String::new()),
                Ok(()) => tried.push("the Mac's own renderer made no pages".into()),
                Err(e) => tried.push(format!("the Mac's own renderer {e}")),
            }
            clear_pages(dir);
        }
    }
    if let Some(bin) = find_tool("gs") {
        let mut c = Command::new(bin);
        c.args(["-q", "-dNOPAUSE", "-dBATCH", "-dSAFER", "-sDEVICE=png16m", "-r160", "-dTextAlphaBits=4", "-dGraphicsAlphaBits=4", &format!("-sOutputFile={d}/page-%04d.png"), &s]);
        match run_polling(&mut c, dir, ready) {
            Ok(()) if !list_pages(dir).is_empty() => return Ok(String::new()),
            Ok(()) => tried.push("Ghostscript made no pages".into()),
            Err(e) => tried.push(format!("Ghostscript {e}")),
        }
        clear_pages(dir);
    }
    // last resort on a Mac: Quick Look draws the first page
    if cfg!(target_os = "macos") {
        let mut c = Command::new("/usr/bin/qlmanage");
        c.args(["-t", "-s", &width, "-o", &d, &s]);
        let _ = run_polling(&mut c, dir, ready);
        let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let made = dir.join(format!("{name}.png"));
        if made.is_file() && std::fs::rename(&made, dir.join("page-0001.png")).is_ok() {
            return Ok(PDF_INSTALL_HINT.to_string());
        }
    }
    Err(format!("RhemaFlow could not draw this PDF ({}). Install the free poppler tools (Terminal: brew install poppler) and try again.", if tried.is_empty() { "no PDF helper found".to_string() } else { tried.join("; ") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white(w: u32, h: u32) -> Rgba {
        Rgba { width: w, height: h, pixels: vec![255u8; (w * h * 4) as usize] }
    }
    fn ink(img: &mut Rgba, x0: u32, x1: u32, y0: u32, y1: u32) {
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * img.width + x) * 4) as usize;
                img.pixels[i] = 10;
                img.pixels[i + 1] = 10;
                img.pixels[i + 2] = 10;
            }
        }
    }

    #[test]
    fn kinds_by_extension() {
        assert_eq!(doc_kind("/a/b/Notes.PDF"), Some(DocKind::Pdf));
        assert_eq!(doc_kind("x.html"), Some(DocKind::Html));
        assert_eq!(doc_kind("x.htm"), Some(DocKind::Html));
        assert_eq!(doc_kind("x.png"), None);
        assert_eq!(doc_kind("noext"), None);
    }

    #[test]
    fn finds_three_lines_and_pads_them() {
        let mut img = white(600, 800);
        for k in 0..3u32 {
            ink(&mut img, 60, 540, 100 + k * 60, 120 + k * 60);
        }
        let p = detect_lines(&img);
        assert_eq!(p.lines.len(), 3, "{:?}", p.lines);
        assert!(p.lines[0].top < 100.0 / 800.0 && p.lines[0].bottom > 120.0 / 800.0);
        assert!(p.lines[0].bottom <= p.lines[1].top + 1e-3, "lines never overlap");
        assert!(p.x0 < 60.0 / 600.0 && p.x1 > 540.0 / 600.0 && p.x0 >= 0.0 && p.x1 <= 1.0);
    }

    #[test]
    fn a_dotted_letter_stays_one_line_and_a_hairline_is_ignored() {
        let mut img = white(600, 800);
        ink(&mut img, 60, 300, 100, 104); // the dot of an i
        ink(&mut img, 60, 300, 106, 124); // the body (2 rows of gap)
        ink(&mut img, 60, 300, 200, 218);
        ink(&mut img, 0, 600, 300, 303); // a thin rule
        let p = detect_lines(&img);
        assert_eq!(p.lines.len(), 2, "{:?}", p.lines);
    }

    #[test]
    fn a_justified_paragraph_is_cut_line_by_line() {
        // Six tight lines whose gaps are bridged by descender-like ink, so no empty row separates
        // them and they all come out as one band (the reported bug: the reading line lit the whole
        // paragraph). A cut at each light gap must give the six lines back, plus the plain line.
        let mut img = white(600, 1600);
        for k in 0..6u32 {
            let y0 = 100 + k * 40;
            ink(&mut img, 60, 540, y0, y0 + 26);
            if k + 1 < 6 {
                ink(&mut img, 200, 210, y0 + 26, y0 + 40); // a few descenders bridge the gap
            }
        }
        ink(&mut img, 60, 540, 400, 426); // one normal line further down
        let p = detect_lines(&img);
        assert_eq!(p.lines.len(), 7, "{:?}", p.lines);
        for w in p.lines.windows(2) {
            assert!(w[0].bottom <= w[1].top + 1e-3, "lines never overlap: {:?}", w);
        }
    }

    #[test]
    fn a_solid_block_far_taller_than_a_line_is_still_split() {
        // No light rows at all inside the tall band, so the even-pitch fallback must do the cutting.
        let mut img = white(600, 1600);
        ink(&mut img, 60, 540, 100, 420);
        ink(&mut img, 60, 540, 500, 526);
        let p = detect_lines(&img);
        assert!(p.lines.len() >= 4, "{:?}", p.lines);
    }

    #[test]
    fn a_big_heading_stays_one_line() {
        // A heading about 1.8x a body line has no gaps inside: it must not be cut in half.
        let mut img = white(600, 1600);
        ink(&mut img, 60, 540, 100, 145);
        ink(&mut img, 60, 540, 200, 226);
        ink(&mut img, 60, 540, 260, 286);
        let p = detect_lines(&img);
        assert_eq!(p.lines.len(), 3, "{:?}", p.lines);
    }

    #[test]
    fn a_dark_page_is_read_by_its_own_background() {
        let mut img = Rgba { width: 600, height: 400, pixels: vec![20u8; 600 * 400 * 4] };
        for y in 100..120u32 {
            for x in 50..500u32 {
                let i = ((y * 600 + x) * 4) as usize;
                img.pixels[i] = 240;
                img.pixels[i + 1] = 240;
                img.pixels[i + 2] = 240;
            }
        }
        assert_eq!(detect_lines(&img).lines.len(), 1);
    }

    #[test]
    fn an_empty_page_has_no_lines() {
        assert!(detect_lines(&white(300, 300)).lines.is_empty());
        assert!(detect_lines(&Rgba { width: 2, height: 2, pixels: vec![0; 16] }).lines.is_empty());
    }

    #[test]
    fn html_structure_entities_and_skipped_parts() {
        let html = "<html><head><title>Sunday &amp; Notes</title><style>p{color:red}</style></head><body>\
            <h1>Welcome</h1><p>Hello <b>big</b> world &mdash; it&#39;s &lt;great&gt;.</p>\
            <script>var x = '<p>no</p>';</script>\
            <ul><li>One</li><li>Two</li></ul><ol><li>First</li><li>Second</li></ol><hr><div>Last<br>line</div></body></html>";
        let p = parse_html(html);
        assert_eq!(p.title, "Sunday & Notes");
        let kinds: Vec<BlockKind> = p.blocks.iter().map(|b| b.kind).collect();
        assert_eq!(kinds, vec![BlockKind::H1, BlockKind::Para, BlockKind::Item, BlockKind::Item, BlockKind::Item, BlockKind::Item, BlockKind::Rule, BlockKind::Para]);
        let joined = |b: &Block| b.runs.iter().map(|r| r.0.clone()).collect::<String>();
        assert_eq!(joined(&p.blocks[1]), "Hello big world \u{2014} it's <great>.");
        assert!(p.blocks[1].runs.iter().any(|r| r.1 && r.0 == "big"), "bold kept");
        assert_eq!(joined(&p.blocks[2]), "\u{2022}  One");
        assert_eq!(joined(&p.blocks[4]), "1.  First");
        assert_eq!(joined(&p.blocks[5]), "2.  Second");
        assert_eq!(joined(&p.blocks[7]), "Last\nline");
        assert!(!p.blocks.iter().any(|b| joined(b).contains("no")), "script text is never shown");
    }

    #[test]
    fn broken_html_never_panics() {
        for s in ["<", "<p", "<p>unclosed <b>bold", "&", "&#xZZ;&;&amp", "<!-- never closed", "<<>>", "\u{1F600}<p>\u{1F600}</p>"] {
            let _ = parse_html(s);
        }
    }

    #[test]
    fn typesetting_makes_pages_and_the_lines_can_be_found_again() {
        let one = parse_html("<h1>Title</h1><p>Short paragraph.</p>");
        let pages = typeset(&one);
        assert_eq!(pages.len(), 1);
        assert_eq!((pages[0].width, pages[0].height), (PAGE_W, PAGE_H));
        let found = detect_lines(&pages[0]);
        assert_eq!(found.lines.len(), 2, "a heading line and a paragraph line: {:?}", found.lines);

        let long: String = (0..220).map(|i| format!("<p>Paragraph number {i} has a few words in it so that it needs a line.</p>")).collect();
        let many = typeset(&parse_html(&long));
        assert!(many.len() >= 3, "pages: {}", many.len());
        for p in &many {
            assert!(!detect_lines(p).lines.is_empty());
        }
    }

    #[test]
    fn an_empty_html_file_still_gives_one_blank_page() {
        let pages = typeset(&parse_html(""));
        assert_eq!(pages.len(), 1);
    }

    #[test]
    fn page_files_sort_by_number_not_by_text() {
        let d = std::env::temp_dir().join(format!("rf-doc-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for n in ["page-10.png", "page-2.png", "page-1.png", "page-0003.png", "notes.txt", "page-x.png"] {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        let names: Vec<String> = list_pages(&d).iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["page-1.png", "page-2.png", "page-0003.png", "page-10.png"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_html_job_makes_page_files_and_is_reused() {
        let d = std::env::temp_dir().join(format!("rf-doc-job-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let src = d.join("notes.html");
        std::fs::write(&src, "<h1>Hi</h1><p>One two three.</p>").unwrap();
        let out = d.join("cache");
        let job = start_job(&src, &out);
        let t0 = Instant::now();
        while !job.is_done() && t0.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(job.is_done() && job.error().is_none(), "{:?}", job.error());
        assert_eq!(job.ready(), 1);
        let page = load_page(&list_pages(&out)[0]).unwrap();
        assert_eq!(page.lines.lines.len(), 2);
        assert!(out.join("done.txt").exists());
        // a second job finds the finished pages and does not make them again
        let again = start_job(&src, &out);
        let t0 = Instant::now();
        while !again.is_done() && t0.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(again.ready(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }
}
