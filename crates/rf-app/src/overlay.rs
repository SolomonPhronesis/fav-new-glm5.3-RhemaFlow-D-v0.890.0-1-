//! Things drawn ON TOP of whatever is on the congregation screen (v0.835.0): lower thirds (a name, a song title,
//! a scripture reference), full-screen cards (giving QR code, handout QR, announcements) and the read-along
//! chapter view. The rules (timing, QR code, layout choice) are plain functions so they are tested without a window.

use crate::stage::SERIF;
use egui::{Align2, Color32, FontId};

// ---------------------------------------------------------------- QR codes

/// The dark/light squares of a QR code for `data` (row by row). `None` when the text is too long for a QR code.
pub fn qr_modules(data: &str) -> Option<Vec<Vec<bool>>> {
    let code = qrcode::QrCode::new(data.as_bytes()).ok()?;
    let w = code.width();
    let colors = code.to_colors();
    Some((0..w).map(|y| (0..w).map(|x| colors[y * w + x] == qrcode::Color::Dark).collect()).collect())
}

/// The same QR code as an SVG picture (for the printed / emailed handout). Four squares of quiet space round it.
pub fn qr_svg(data: &str, pixel: u32) -> Option<String> {
    let m = qr_modules(data)?;
    let n = m.len() as u32;
    let total = n + 8;
    let mut s = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {total} {total}\" width=\"{}\" height=\"{}\" shape-rendering=\"crispEdges\"><rect width=\"{total}\" height=\"{total}\" fill=\"#fff\"/>", pixel, pixel);
    for (y, row) in m.iter().enumerate() {
        for (x, dark) in row.iter().enumerate() {
            if *dark {
                s.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"1\" height=\"1\" fill=\"#000\"/>", x + 4, y + 4));
            }
        }
    }
    s.push_str("</svg>");
    Some(s)
}

/// A web address a person typed ("give.mychurch.org") made safe to put in a QR code: gets https:// if it has no scheme.
pub fn tidy_url(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() || t.contains("://") || t.starts_with("mailto:") || t.starts_with("tel:") {
        t.to_string()
    } else {
        format!("https://{t}")
    }
}

// ---------------------------------------------------------------- lower thirds

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum L3Style {
    /// a dark band across the screen with a coloured stripe
    Bar,
    /// a box just big enough for the words
    Box,
    /// words with a shadow, nothing behind them
    Minimal,
}

impl L3Style {
    pub const ALL: [L3Style; 3] = [L3Style::Bar, L3Style::Box, L3Style::Minimal];
    pub fn label(self) -> &'static str {
        match self {
            L3Style::Bar => "Bar",
            L3Style::Box => "Box",
            L3Style::Minimal => "Words only",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            L3Style::Bar => "bar",
            L3Style::Box => "box",
            L3Style::Minimal => "minimal",
        }
    }
    pub fn from_key(k: &str) -> L3Style {
        Self::ALL.into_iter().find(|s| s.key() == k).unwrap_or(L3Style::Bar)
    }
}

pub const L3_FADE_MS: u64 = 350;

/// A name / title strip. Times are milliseconds on the app's own clock.
#[derive(Clone, Debug, PartialEq)]
pub struct LowerThird {
    pub line1: String,
    pub line2: String,
    pub style: L3Style,
    pub left: bool,
    pub accent: [u8; 3],
    /// leaves by itself after this long (0 = stays until hidden)
    pub hold_ms: u64,
    shown_at: Option<u64>,
    hidden_at: Option<u64>,
}

impl Default for LowerThird {
    fn default() -> Self {
        LowerThird { line1: String::new(), line2: String::new(), style: L3Style::Bar, left: true, accent: [224, 178, 90], hold_ms: 8_000, shown_at: None, hidden_at: None }
    }
}

impl LowerThird {
    pub fn show(&mut self, now: u64) {
        self.shown_at = Some(now);
        self.hidden_at = None;
    }
    pub fn hide(&mut self, now: u64) {
        if self.shown_at.is_some() && self.hidden_at.is_none() {
            self.hidden_at = Some(now);
        }
    }
    /// 0.0 (gone) to 1.0 (fully in), with a short fade in and out.
    pub fn alpha(&self, now: u64) -> f32 {
        let Some(s) = self.shown_at else { return 0.0 };
        if now < s || (self.line1.trim().is_empty() && self.line2.trim().is_empty()) {
            return 0.0;
        }
        let fade_in = ((now - s) as f32 / L3_FADE_MS as f32).min(1.0);
        let end = self.hidden_at.or(if self.hold_ms > 0 { Some(s + self.hold_ms) } else { None });
        let fade_out = match end {
            Some(e) if now >= e => 1.0 - ((now - e) as f32 / L3_FADE_MS as f32),
            _ => 1.0,
        };
        fade_in.min(fade_out).clamp(0.0, 1.0)
    }
    pub fn is_showing(&self, now: u64) -> bool {
        self.alpha(now) > 0.0
    }
}

/// Ready-made wording for the usual lower thirds.
pub fn speaker_lines(name: &str, role: &str) -> (String, String) {
    (name.trim().to_string(), role.trim().to_string())
}
pub fn scripture_lines(label: &str, translation: &str) -> (String, String) {
    (label.trim().to_string(), translation.trim().to_string())
}
pub fn song_lines(title: &str, author: &str, ccli: &str) -> (String, String) {
    let mut sub = Vec::new();
    if !author.trim().is_empty() {
        sub.push(author.trim().to_string());
    }
    if !ccli.trim().is_empty() {
        sub.push(format!("CCLI {}", ccli.trim()));
    }
    (title.trim().to_string(), sub.join("  \u{b7}  "))
}

fn col(c: [u8; 3], a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn draw_lower_third(ui: &egui::Ui, rect: egui::Rect, lt: &LowerThird, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let h = rect.height();
    let big = (h * 0.052).clamp(20.0, 72.0);
    let small = big * 0.58;
    let font1 = FontId::new(big, egui::FontFamily::Name(SERIF.into()));
    let font2 = FontId::proportional(small);
    let p = ui.painter();
    let g1 = p.layout_no_wrap(lt.line1.clone(), font1, Color32::WHITE);
    let g2 = if lt.line2.trim().is_empty() { None } else { Some(p.layout_no_wrap(lt.line2.clone(), font2, Color32::from_gray(215))) };
    let text_w = g1.size().x.max(g2.as_ref().map_or(0.0, |g| g.size().x));
    let text_h = g1.size().y + g2.as_ref().map_or(0.0, |g| g.size().y + 4.0);
    let pad = big * 0.45;
    let margin = rect.width() * 0.05;
    let bottom = rect.bottom() - h * 0.085;
    let (x0, w) = match lt.style {
        L3Style::Bar => (rect.left(), rect.width()),
        _ => {
            let w = (text_w + pad * 2.0 + big * 0.35).min(rect.width() - margin * 2.0);
            let x = if lt.left { rect.left() + margin } else { rect.center().x - w / 2.0 };
            (x, w)
        }
    };
    let box_rect = egui::Rect::from_min_size(egui::pos2(x0, bottom - text_h - pad * 2.0), egui::vec2(w, text_h + pad * 2.0));
    let text_x = match lt.style {
        L3Style::Bar => {
            if lt.left { rect.left() + margin + big * 0.3 } else { rect.center().x - text_w / 2.0 }
        }
        _ => {
            if lt.left { box_rect.left() + pad + big * 0.25 } else { box_rect.center().x - text_w / 2.0 }
        }
    };
    match lt.style {
        L3Style::Bar => {
            p.rect_filled(box_rect, 0.0, Color32::from_rgba_unmultiplied(8, 10, 16, (215.0 * alpha) as u8));
            p.rect_filled(egui::Rect::from_min_size(box_rect.min, egui::vec2(big * 0.14, box_rect.height())), 0.0, col(lt.accent, alpha));
        }
        L3Style::Box => {
            p.rect_filled(box_rect, 6.0, Color32::from_rgba_unmultiplied(8, 10, 16, (220.0 * alpha) as u8));
            p.rect_filled(egui::Rect::from_min_size(box_rect.min, egui::vec2(big * 0.14, box_rect.height())), 6.0, col(lt.accent, alpha));
        }
        L3Style::Minimal => {}
    }
    let mut y = box_rect.top() + pad;
    let shadow = Color32::from_black_alpha((200.0 * alpha) as u8);
    let tint = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (255.0 * alpha) as u8);
    if lt.style == L3Style::Minimal {
        p.galley_with_override_text_color(egui::pos2(text_x + 2.0, y + 2.0), g1.clone(), shadow);
    }
    p.galley_with_override_text_color(egui::pos2(text_x, y), g1.clone(), tint(Color32::WHITE));
    y += g1.size().y + 4.0;
    if let Some(g) = g2 {
        if lt.style == L3Style::Minimal {
            p.galley_with_override_text_color(egui::pos2(text_x + 1.5, y + 1.5), g.clone(), shadow);
        }
        let accent = if lt.style == L3Style::Minimal { Color32::from_gray(225) } else { Color32::from_rgb(lt.accent[0], lt.accent[1], lt.accent[2]) };
        p.galley_with_override_text_color(egui::pos2(text_x, y), g, tint(accent));
    }
}

// ---------------------------------------------------------------- cards

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    /// a title, some lines and (optionally) a QR code: giving, handout, welcome
    Info,
    /// the chapter being read, one verse picked out
    ReadAlong,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub kind: CardKind,
    pub title: String,
    pub lines: Vec<String>,
    pub footer: String,
    pub qr: Option<Vec<Vec<bool>>>,
    pub qr_caption: String,
    /// read-along: which of `lines` is being read now
    pub highlight: Option<usize>,
}

impl Card {
    pub fn info(title: &str, lines: &[&str]) -> Card {
        Card { kind: CardKind::Info, title: title.to_string(), lines: lines.iter().map(|s| s.to_string()).collect(), footer: String::new(), qr: None, qr_caption: String::new(), highlight: None }
    }
    /// The giving slide: a headline, a few words, the QR code and the plain web address under it.
    pub fn giving(headline: &str, message: &str, url: &str) -> Card {
        let url = tidy_url(url);
        let mut c = Card::info(headline, &[message]);
        c.lines.retain(|l| !l.trim().is_empty());
        c.qr = qr_modules(&url);
        c.qr_caption = url.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_string();
        c
    }
}

/// Which row of the chapter to show first so the highlighted one sits near the middle of a window of `fit` rows.
pub fn first_row_for(highlight: usize, total: usize, fit: usize) -> usize {
    if fit == 0 || total <= fit {
        return 0;
    }
    highlight.saturating_sub(fit / 3).min(total - fit)
}

pub fn draw_card(ui: &egui::Ui, rect: egui::Rect, card: &Card, fg: [u8; 3], bg: [u8; 3], alpha: f32) {
    let p = ui.painter();
    p.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], (255.0 * alpha) as u8));
    let gold = Color32::from_rgba_unmultiplied(224, 178, 90, (255.0 * alpha) as u8);
    let text = col(fg, alpha);
    let serif = |s: f32| FontId::new(s, egui::FontFamily::Name(SERIF.into()));
    match card.kind {
        CardKind::Info => {
            let h = rect.height();
            let title_size = (h * 0.085).clamp(24.0, 110.0);
            let body_size = (h * 0.048).clamp(16.0, 64.0);
            let has_qr = card.qr.is_some();
            let text_w = if has_qr { rect.width() * 0.46 } else { rect.width() * 0.8 };
            let left_x = if has_qr { rect.left() + rect.width() * 0.07 } else { rect.center().x - text_w / 2.0 };
            let mut galleys = Vec::new();
            galleys.push(p.layout(card.title.clone(), serif(title_size), gold, text_w));
            for l in &card.lines {
                galleys.push(p.layout(l.clone(), serif(body_size), text, text_w));
            }
            let total_h: f32 = galleys.iter().map(|g| g.size().y + 14.0).sum();
            let mut y = rect.center().y - total_h / 2.0;
            for g in galleys {
                let gh = g.size().y;
                let x = if has_qr { left_x } else { rect.center().x - g.size().x / 2.0 };
                p.galley(egui::pos2(x, y), g, text);
                y += gh + 14.0;
            }
            if let Some(m) = &card.qr {
                let n = m.len() as f32 + 8.0;
                let side = (h * 0.62).min(rect.width() * 0.38);
                let cell = side / n;
                let origin = egui::pos2(rect.right() - rect.width() * 0.07 - side, rect.center().y - side / 2.0 - body_size * 0.5);
                p.rect_filled(egui::Rect::from_min_size(origin, egui::vec2(side, side)), 8.0, Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8));
                for (yy, row) in m.iter().enumerate() {
                    for (xx, dark) in row.iter().enumerate() {
                        if *dark {
                            let tl = origin + egui::vec2((xx as f32 + 4.0) * cell, (yy as f32 + 4.0) * cell);
                            p.rect_filled(egui::Rect::from_min_size(tl, egui::vec2(cell + 0.5, cell + 0.5)), 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, (255.0 * alpha) as u8));
                        }
                    }
                }
                if !card.qr_caption.is_empty() {
                    p.text(egui::pos2(origin.x + side / 2.0, origin.y + side + body_size * 0.9), Align2::CENTER_CENTER, &card.qr_caption, FontId::proportional(body_size * 0.8), text);
                }
            }
            if !card.footer.is_empty() {
                p.text(egui::pos2(rect.center().x, rect.bottom() - h * 0.06), Align2::CENTER_CENTER, &card.footer, FontId::proportional(body_size * 0.7), Color32::from_rgba_unmultiplied(160, 166, 178, (200.0 * alpha) as u8));
            }
        }
        CardKind::ReadAlong => {
            let size = (rect.height() * 0.05).clamp(18.0, 60.0);
            let w = rect.width() * 0.78;
            let hl = card.highlight.unwrap_or(0).min(card.lines.len().saturating_sub(1));
            // lay out rows until the window is full, keeping the highlighted row near the upper-middle
            let galley = |i: usize, bright: bool| {
                let c = if bright { text } else { Color32::from_rgba_unmultiplied(fg[0], fg[1], fg[2], (90.0 * alpha) as u8) };
                p.layout(card.lines[i].clone(), serif(size), c, w)
            };
            if card.lines.is_empty() {
                return;
            }
            let title_h = size * 1.6;
            p.text(egui::pos2(rect.center().x, rect.top() + title_h * 0.7), Align2::CENTER_CENTER, &card.title, serif(size * 0.8), gold);
            let top = rect.top() + title_h * 1.4;
            let avail = rect.bottom() - top - rect.height() * 0.04;
            // measure rows from the start index that keeps the highlight in view
            let mut start = hl.saturating_sub(2);
            loop {
                let mut used = 0.0;
                let mut fits_hl = false;
                for i in start..card.lines.len() {
                    used += galley(i, i == hl).size().y + size * 0.4;
                    if used > avail {
                        break;
                    }
                    if i == hl {
                        fits_hl = true;
                        break;
                    }
                }
                if fits_hl || start >= hl {
                    break;
                }
                start += 1;
            }
            let mut y = top;
            for i in start..card.lines.len() {
                let g = galley(i, i == hl);
                if y + g.size().y > rect.bottom() {
                    break;
                }
                let gh = g.size().y;
                if i == hl {
                    p.rect_filled(egui::Rect::from_min_size(egui::pos2(rect.center().x - w / 2.0 - 16.0, y - 6.0), egui::vec2(w + 32.0, gh + 12.0)), 10.0, Color32::from_rgba_unmultiplied(255, 255, 255, (22.0 * alpha) as u8));
                }
                p.galley(egui::pos2(rect.center().x - w / 2.0, y), g, text);
                y += gh + size * 0.4;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_web_address_becomes_a_qr_code_and_a_picture() {
        let m = qr_modules("https://give.example.org/sunday").unwrap();
        assert!(m.len() >= 21 && m.len() == m[0].len(), "square, at least version 1");
        // the three finder squares sit in the corners: the top-left module is dark
        assert!(m[0][0] && m[0][m.len() - 1] && m[m.len() - 1][0]);
        let svg = qr_svg("https://give.example.org/sunday", 300).unwrap();
        assert!(svg.starts_with("<svg") && svg.contains("fill=\"#000\"") && svg.ends_with("</svg>"));
        assert!(qr_modules(&"x".repeat(5000)).is_none(), "too long for a QR code is an answer, not a crash");
    }

    #[test]
    fn typed_addresses_get_https_once() {
        assert_eq!(tidy_url("give.church.org"), "https://give.church.org");
        assert_eq!(tidy_url(" https://a.b/c "), "https://a.b/c");
        assert_eq!(tidy_url("http://a.b"), "http://a.b");
        assert_eq!(tidy_url(""), "");
    }

    #[test]
    fn a_lower_third_fades_in_holds_then_fades_out_by_itself() {
        let mut lt = LowerThird { line1: "Pastor Sam".into(), hold_ms: 4_000, ..Default::default() };
        assert_eq!(lt.alpha(0), 0.0, "never shown");
        lt.show(1_000);
        assert_eq!(lt.alpha(1_000), 0.0);
        assert!((lt.alpha(1_000 + L3_FADE_MS / 2) - 0.5).abs() < 0.01);
        assert_eq!(lt.alpha(2_000), 1.0);
        assert_eq!(lt.alpha(5_000), 1.0);
        assert!(lt.alpha(5_000 + L3_FADE_MS / 2) < 0.6);
        assert_eq!(lt.alpha(5_000 + L3_FADE_MS + 1), 0.0);
        assert!(!lt.is_showing(9_000));
    }

    #[test]
    fn hold_zero_stays_until_hidden_and_empty_text_never_shows() {
        let mut lt = LowerThird { line1: "Welcome".into(), hold_ms: 0, ..Default::default() };
        lt.show(0);
        assert_eq!(lt.alpha(600_000), 1.0);
        lt.hide(600_000);
        assert_eq!(lt.alpha(600_000 + L3_FADE_MS + 5), 0.0);
        let mut empty = LowerThird::default();
        empty.show(0);
        assert_eq!(empty.alpha(5_000), 0.0);
    }

    #[test]
    fn the_usual_lower_thirds_are_worded_sensibly() {
        assert_eq!(song_lines("Great Is Thy Faithfulness", "T. Chisholm", "18723"), ("Great Is Thy Faithfulness".to_string(), "T. Chisholm  \u{b7}  CCLI 18723".to_string()));
        assert_eq!(song_lines("Song", "", "").1, "");
        assert_eq!(scripture_lines("John 3:16", "KJV"), ("John 3:16".into(), "KJV".into()));
        assert_eq!(speaker_lines(" Sam Lee ", "Senior Pastor").0, "Sam Lee");
    }

    #[test]
    fn the_giving_card_has_the_qr_and_a_clean_address() {
        let c = Card::giving("Give", "Thank you for your generosity", "give.example.org/sunday/");
        assert!(c.qr.is_some());
        assert_eq!(c.qr_caption, "give.example.org/sunday");
    }

    #[test]
    fn the_read_along_window_keeps_the_verse_in_view() {
        assert_eq!(first_row_for(0, 30, 8), 0);
        assert_eq!(first_row_for(10, 30, 9), 7);
        assert_eq!(first_row_for(29, 30, 8), 22, "never scrolls past the end");
        assert_eq!(first_row_for(3, 5, 8), 0, "short chapter: all of it");
    }
}
