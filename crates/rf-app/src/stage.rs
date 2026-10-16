//! The congregation stage surface — used by both the live stage window and
//! the operator's preview card. Pure black, serif text, gold reference badge,
//! gentle fade-in so nothing flashes on screen.

use crate::state::{StageContent, StageKind};
use crate::theme;
use egui::{Align2, Color32, FontId};
use rf_service::media::{dim_alpha, fit_rect, FitMode, DEFAULT_DIM_PCT};

pub const SERIF: &str = "stage_serif";
/// Stage zoom range: 60% .. 400% (v0.811.0; was 60..220).
pub const ZOOM_MIN: f32 = 0.6;
pub const ZOOM_MAX: f32 = 4.0;

/// How solid a new verse or picture is `age_ms` after it arrived: a cut (fade 0) is solid at once; a fade starts
/// at 15% (never fully invisible) and reaches 100% after `fade_ms`.
pub fn arrival_alpha(age_ms: u64, fade_ms: u32) -> f32 {
    if fade_ms == 0 {
        1.0
    } else {
        (age_ms as f32 / fade_ms as f32).clamp(0.15, 1.0)
    }
}

/// Look of the stage: style and blackout (v0.812.0).
#[derive(Clone, Copy, Debug)]
pub struct StageLook {
    pub style: crate::state::StageStyle,
    pub blackout: bool,
    /// background and text colour (v0.814.0)
    pub bg: [u8; 3],
    pub fg: [u8; 3],
    /// how long a new verse or picture takes to arrive: 0 = cut (instant)
    pub fade_ms: u32,
}

impl Default for StageLook {
    fn default() -> Self {
        StageLook { style: crate::state::StageStyle::Classic, blackout: false, bg: [0, 0, 0], fg: [255, 255, 255], fade_ms: 350 }
    }
}

/// True when a background is light enough that dark text/accents are the readable choice.
pub fn readable_on(bg: [u8; 3]) -> bool {
    // perceived brightness (ITU-R BT.601)
    (0.299 * bg[0] as f32 + 0.587 * bg[1] as f32 + 0.114 * bg[2] as f32) > 150.0
}

/// Ready-made colour pairs (name, background, text).
pub const COLOR_PRESETS: &[(&str, [u8; 3], [u8; 3])] = &[
    ("Black / white", [0, 0, 0], [255, 255, 255]),
    ("Deep navy", [10, 22, 52], [255, 255, 255]),
    ("Dark green", [8, 40, 24], [240, 250, 240]),
    ("Maroon", [64, 10, 20], [255, 240, 235]),
    ("Charcoal", [34, 36, 42], [245, 245, 245]),
    ("Royal purple", [38, 16, 70], [250, 245, 255]),
    ("Warm paper", [250, 244, 230], [30, 24, 16]),
    ("White / black", [255, 255, 255], [10, 10, 10]),
    ("Black / yellow", [0, 0, 0], [255, 230, 80]),
];

/// Text colour that stays readable if the operator changes only the background.
pub fn auto_text_for(bg: [u8; 3]) -> [u8; 3] {
    if readable_on(bg) { [20, 16, 10] } else { [255, 255, 255] }
}

/// Highlight colours for the document reading line (v0.842.0): the operator picks one, and 1, 2 or
/// 3 lines at a time, next to the Reading line switch.
pub const DOC_GUIDE_COLORS: &[(&str, [u8; 3])] = &[
    ("Yellow", [255, 232, 40]),
    ("Green", [110, 240, 120]),
    ("Blue", [90, 200, 255]),
    ("Pink", [255, 120, 190]),
    ("Orange", [255, 165, 50]),
    ("White", [255, 255, 255]),
];

/// A texture the stage may paint (picture, video frame, background).
#[derive(Clone, Copy, Debug)]
pub struct ArtImage {
    pub texture: egui::TextureId,
    pub size: [f32; 2],
}

/// The pictures behind and in the stage (v0.815.0). Textures are shared by every window of the
/// app, so the operator preview and the congregation window both get the same one.
#[derive(Clone, Debug)]
pub struct StageArt {
    /// the picture or current video frame for `StageKind::Media`
    pub media: Option<ArtImage>,
    pub fit: FitMode,
    /// behind slides and countdowns only (never behind scripture)
    pub background: Option<ArtImage>,
    pub dim_pct: u8,
    /// a sound file is playing: the preview says so; the audience screen stays plain
    pub sound_only: bool,
    /// a PDF / HTML page (v0.840.0): drawn instead of `media`, with zoom and the reading line
    pub doc: Option<DocArt>,
}

impl Default for StageArt {
    fn default() -> Self {
        StageArt { media: None, fit: FitMode::Fit, background: None, dim_pct: DEFAULT_DIM_PCT, sound_only: false, doc: None }
    }
}

/// One document page as the stage should draw it. Positions are fractions of the page (0..1), so the same
/// description draws correctly in the small preview and on the big screen.
#[derive(Clone, Copy, Debug)]
pub struct DocArt {
    pub page: ArtImage,
    /// 1.0 = the whole page fits (or the page fills the width, with `fit_width`); bigger = zoomed in
    pub zoom: f32,
    pub fit_width: bool,
    /// the point of the page kept in the middle of the screen (clamped so the page never leaves a gap)
    pub cx: f32,
    pub cy: f32,
    /// the reading line: (top, bottom, left, right). Everything else on the screen is dimmed.
    pub guide: Option<(f32, f32, f32, f32)>,
    /// the highlight colour of the reading line (v0.842.0)
    pub guide_color: [u8; 3],
}

/// Where the page lands on the screen: (top-left corner, width, height). Pure maths, tested below.
pub fn doc_placement(rect: egui::Rect, page: [f32; 2], zoom: f32, fit_width: bool, cx: f32, cy: f32) -> (egui::Pos2, f32, f32) {
    let (pw, ph) = (page[0].max(1.0), page[1].max(1.0));
    let fit_page = (rect.width() / pw).min(rect.height() / ph);
    let base = if fit_width { rect.width() / pw } else { fit_page };
    let scale = base * zoom.clamp(0.5, 6.0);
    let (w, h) = (pw * scale, ph * scale);
    let clamp_c = |c: f32, view: f32, whole: f32| -> f32 {
        if whole <= view {
            0.5
        } else {
            let half = view / whole / 2.0;
            c.clamp(half, 1.0 - half)
        }
    };
    let (cx, cy) = (clamp_c(cx, rect.width(), w), clamp_c(cy, rect.height(), h));
    (egui::pos2(rect.center().x - cx * w, rect.center().y - cy * h), w, h)
}

/// The page, its soft shadow, and (if asked) the reading line with the rest of the screen dimmed.
fn draw_doc(ui: &egui::Ui, rect: egui::Rect, d: DocArt) {
    let (origin, w, h) = doc_placement(rect, d.page.size, d.zoom, d.fit_width, d.cx, d.cy);
    let dst = egui::Rect::from_min_size(origin, egui::vec2(w, h));
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(dst.translate(egui::vec2(5.0, 7.0)), 3.0, Color32::from_black_alpha(70));
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(d.page.texture, dst, uv, Color32::WHITE);
    if let Some((top, bottom, x0, x1)) = d.guide {
        let band = egui::Rect::from_min_max(egui::pos2(dst.left() + x0 * w, dst.top() + top * h), egui::pos2(dst.left() + x1 * w, dst.top() + bottom * h));
        let dim = Color32::from_black_alpha(165);
        let pieces = [
            egui::Rect::from_min_max(rect.left_top(), egui::pos2(rect.right(), band.top())),
            egui::Rect::from_min_max(egui::pos2(rect.left(), band.bottom()), rect.right_bottom()),
            egui::Rect::from_min_max(egui::pos2(rect.left(), band.top()), egui::pos2(band.left(), band.bottom())),
            egui::Rect::from_min_max(egui::pos2(band.right(), band.top()), egui::pos2(rect.right(), band.bottom())),
        ];
        for r in pieces {
            let r = r.intersect(rect);
            if r.width() > 0.0 && r.height() > 0.0 {
                painter.rect_filled(r, 0.0, dim);
            }
        }
        // v0.845.0 "marker pen": the page picture is painted again inside the band, tinted with the colour. A tint
        // MULTIPLIES, so the white paper turns solid highlighter colour while the black words stay black (before,
        // a see-through yellow washed the words out). The line then stands out from the dimmed page on any screen.
        let tint = Color32::from_rgb(d.guide_color[0], d.guide_color[1], d.guide_color[2]);
        painter.with_clip_rect(band.intersect(rect)).image(d.page.texture, dst, uv, tint);
        painter.rect_stroke(band, 2.0, egui::Stroke::new(2.0, tint));
        let bar = (band.height() * 0.12).clamp(3.0, 10.0);
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(band.left() - bar * 1.6, band.top()), egui::pos2(band.left() - bar * 0.6, band.bottom())), 2.0, Color32::from_rgb(d.guide_color[0], d.guide_color[1], d.guide_color[2]));
    }
}

/// Paints `img` placed by `fit_rect` inside `rect`, clipped to `rect`.
fn draw_image(ui: &egui::Ui, rect: egui::Rect, img: ArtImage, mode: FitMode) {
    let r = fit_rect(img.size[0], img.size[1], rect.width(), rect.height(), mode);
    let dst = egui::Rect::from_min_size(rect.min + egui::vec2(r.x, r.y), egui::vec2(r.w, r.h));
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    ui.painter().with_clip_rect(rect).image(img.texture, dst, uv, Color32::WHITE);
}

/// Background picture (filling the screen) plus the darkening layer (S4.1, S4.3).
fn draw_background(ui: &egui::Ui, rect: egui::Rect, art: &StageArt) -> bool {
    let Some(bg) = art.background else { return false };
    draw_image(ui, rect, bg, FitMode::Fill);
    let a = dim_alpha(art.dim_pct);
    if a > 0 {
        ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(a));
    }
    true
}

/// Source lines kept as lines (lyrics), each wrapped only if it is too wide.
fn wrap_source_lines(ui: &egui::Ui, text: &str, max_w: f32, size: f32) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.split('\n') {
        if line.trim().is_empty() {
            out.push(String::new());
        } else {
            out.extend(wrap_lines(ui, line, max_w, size));
        }
    }
    out
}

pub fn draw_stage(ui: &mut egui::Ui, stage: &StageContent, full: bool, zoom: f32, look: StageLook, art: &StageArt) {
    use crate::state::StageStyle;
    let zoom = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let bg = if look.style == StageStyle::ChromaGreen { Color32::from_rgb(0, 177, 64) } else { Color32::from_rgb(look.bg[0], look.bg[1], look.bg[2]) };
    ui.painter().rect_filled(rect, 0.0, bg);
    if look.blackout {
        // blackout hides everything but keeps the verse for "un-blank"
        if !full {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, "BLACKOUT \u{2014} press B to bring it back", FontId::proportional(14.0), Color32::from_rgb(0x9A, 0xA0, 0xAC));
        }
        return;
    }

    // fade-in from the moment content changed (look.fade_ms; 0 = a plain cut)
    let alpha = match stage.updated {
        Some(t) => arrival_alpha(t.elapsed().as_millis() as u64, look.fade_ms),
        None => 1.0,
    };
    let with_alpha = |c: Color32| {
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (255.0 * alpha) as u8)
    };
    let text_col = with_alpha(Color32::from_rgb(look.fg[0], look.fg[1], look.fg[2]));
    let light_bg = readable_on(look.bg);
    let gold_col = if light_bg { with_alpha(Color32::from_rgb(0x8A, 0x5A, 0x00)) } else { with_alpha(theme::GOLD) };
    let muted_col = Color32::from_rgba_unmultiplied(120, 126, 138, (180.0 * alpha) as u8);

    match stage.kind {
        StageKind::Blank => {
            if full {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "RhemaFlow is ready",
                    FontId::proportional(22.0),
                    muted_col,
                );
            }
        }
        StageKind::Scripture => {
            // region the text may use
            let (area, big, show_ref) = match look.style {
                StageStyle::LowerThird => {
                    let h = rect.height() * 0.34;
                    let band = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - h), rect.max);
                    ui.painter().rect_filled(band, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 205));
                    (band, 0.55, true)
                }
                StageStyle::LargePrint => (rect, 1.35, false),
                _ => (rect, 1.0, true),
            };
            let inner_w = area.width() * 0.84;
            let mut size = if full {
                theme::stage_font_size(stage.text.len(), area.height())
            } else {
                theme::stage_font_size(stage.text.len(), 300.0).min(24.0)
            } * zoom * big;
            // Never let big zoom push text off the screen: shrink until the whole
            // verse and its reference fit (at most a few passes).
            let mut lines = wrap_lines(ui, &stage.text, inner_w, size);
            for _ in 0..8 {
                let badge_s = (size * 0.42).clamp(14.0, 30.0);
                let h = lines.len() as f32 * size * 1.32 + if show_ref { badge_s * 2.2 } else { 0.0 };
                if h <= area.height() * 0.94 || size <= 12.0 {
                    break;
                }
                size *= (area.height() * 0.94 / h).clamp(0.6, 0.95);
                lines = wrap_lines(ui, &stage.text, inner_w, size);
            }
            let font = FontId::new(size, egui::FontFamily::Name(SERIF.into()));
            let line_h = size * 1.32;
            let badge_size = (size * 0.42).clamp(14.0, 30.0);
            let block_h = lines.len() as f32 * line_h + if show_ref { badge_size * 2.2 } else { 0.0 };
            let rect = area; // everything below centres inside the chosen area

            let mut y = rect.center().y - block_h / 2.0;
            for line in &lines {
                y += line_h;
                ui.painter().text(
                    egui::pos2(rect.center().x, y - line_h * 0.35),
                    Align2::CENTER_CENTER,
                    line,
                    font.clone(),
                    text_col,
                );
            }
            y += badge_size * 1.1;
            if show_ref {
                let badge = format!("{} — {}", stage.label, stage.translation);
                // on a green screen gold is hard to read: use white
                let ref_col = if look.style == StageStyle::ChromaGreen { Color32::WHITE } else { gold_col };
                ui.painter().text(
                    egui::pos2(rect.center().x, y),
                    Align2::CENTER_CENTER,
                    &badge,
                    FontId::new(badge_size, egui::FontFamily::Name(SERIF.into())),
                    ref_col,
                );
            }

            if stage.chapter_only {
                ui.painter().text(
                    egui::pos2(rect.right() - 30.0, rect.bottom() - 26.0),
                    Align2::RIGHT_BOTTOM,
                    "chapter · verse 1",
                    FontId::proportional(13.0),
                    muted_col,
                );
            }
        }
        StageKind::Points => {
            let items: Vec<&str> = stage.text.split('\n').collect();
            let size = if full { 54.0 * zoom } else { 18.0 * zoom };
            let line_h = size * 1.35;
            let title_size = size * 0.7;
            let block_h = items.len() as f32 * line_h + title_size * 1.6;
            let mut y = rect.center().y - block_h / 2.0;

            ui.painter().text(
                egui::pos2(rect.center().x, y + title_size),
                Align2::CENTER_CENTER,
                &stage.label,
                FontId::new(title_size, egui::FontFamily::Name(SERIF.into())),
                gold_col,
            );
            y += title_size * 1.9;
            for item in &items {
                ui.painter().text(
                    egui::pos2(rect.center().x, y + line_h / 2.0),
                    Align2::CENTER_CENTER,
                    item,
                    FontId::new(size, egui::FontFamily::Name(SERIF.into())),
                    text_col,
                );
                y += line_h;
            }
        }
        StageKind::Slide => {
            let has_bg = draw_background(ui, rect, art);
            let inner_w = rect.width() * 0.84;
            let len = stage.text.len();
            let mut size = if full { theme::stage_font_size(len, rect.height()) } else { theme::stage_font_size(len, 300.0).min(24.0) } * zoom;
            let mut lines = wrap_source_lines(ui, &stage.text, inner_w, size);
            for _ in 0..10 {
                let h = lines.len() as f32 * size * 1.32;
                if h <= rect.height() * 0.92 || size <= 10.0 {
                    break;
                }
                size *= (rect.height() * 0.92 / h).clamp(0.5, 0.95);
                lines = wrap_source_lines(ui, &stage.text, inner_w, size);
            }
            let font = FontId::new(size, egui::FontFamily::Name(SERIF.into()));
            let line_h = size * 1.32;
            let mut y = rect.center().y - lines.len() as f32 * line_h / 2.0;
            for line in &lines {
                y += line_h;
                let pos = egui::pos2(rect.center().x, y - line_h * 0.5);
                if has_bg && !line.is_empty() {
                    // soft shadow so words stay readable on a bright picture (S4.4)
                    let off = (size * 0.045).max(1.5);
                    ui.painter().text(pos + egui::vec2(off, off), Align2::CENTER_CENTER, line, font.clone(), Color32::from_black_alpha((190.0 * alpha) as u8));
                }
                ui.painter().text(pos, Align2::CENTER_CENTER, line, font.clone(), text_col);
            }
            if !full && !stage.label.is_empty() {
                // the section name is for the operator only (S1.2)
                ui.painter().text(rect.left_top() + egui::vec2(8.0, 6.0), Align2::LEFT_TOP, &stage.label, FontId::proportional(12.0), gold_col);
            }
        }
        StageKind::Countdown => {
            let has_bg = draw_background(ui, rect, art);
            let mut size = (if full { rect.height() * 0.30 } else { 46.0 }) * zoom;
            size = size.min(rect.height() * 0.5);
            let mono = |s: f32| FontId::new(s, egui::FontFamily::Monospace);
            let width_of = |s: f32| ui.ctx().fonts(|f| f.layout_no_wrap(stage.text.clone(), mono(s), Color32::WHITE).rect.width());
            let w = width_of(size);
            if w > rect.width() * 0.9 && w > 0.0 {
                size *= rect.width() * 0.9 / w;
            }
            let title_size = (size * 0.30).clamp(14.0, 90.0);
            let block_h = size * 1.1 + if stage.label.is_empty() { 0.0 } else { title_size * 1.8 };
            let top = rect.center().y - block_h / 2.0;
            if !stage.label.is_empty() {
                let tp = egui::pos2(rect.center().x, top + title_size * 0.9);
                let tf = FontId::new(title_size, egui::FontFamily::Name(SERIF.into()));
                if has_bg {
                    ui.painter().text(tp + egui::vec2(2.0, 2.0), Align2::CENTER_CENTER, &stage.label, tf.clone(), Color32::from_black_alpha((190.0 * alpha) as u8));
                }
                ui.painter().text(tp, Align2::CENTER_CENTER, &stage.label, tf, gold_col);
            }
            let np = egui::pos2(rect.center().x, top + block_h - size * 0.55);
            if has_bg {
                let off = (size * 0.03).max(1.5);
                ui.painter().text(np + egui::vec2(off, off), Align2::CENTER_CENTER, &stage.text, mono(size), Color32::from_black_alpha((190.0 * alpha) as u8));
            }
            ui.painter().text(np, Align2::CENTER_CENTER, &stage.text, mono(size), text_col);
        }
        StageKind::Media if art.doc.is_some() => {
            if let Some(d) = art.doc {
                draw_doc(ui, rect, d);
            }
        }
        StageKind::Media => match art.media {
            Some(img) => draw_image(ui, rect, img, art.fit),
            None if !full && art.sound_only => {
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, format!("\u{266a}  playing sound only\n{}", stage.label), FontId::proportional(15.0), muted_col);
            }
            None if !full => {
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, "opening picture, video or document...", FontId::proportional(14.0), muted_col);
            }
            None => {}
        },
    }
}

fn wrap_lines(ui: &egui::Ui, text: &str, max_w: f32, size: f32) -> Vec<String> {
    let font = FontId::new(size, egui::FontFamily::Name(SERIF.into()));
    let measure = |s: &str| -> f32 {
        ui.ctx().fonts(|f| {
            f.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE)
                .rect
                .width()
        })
    };
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let cand = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if measure(&cand) > max_w && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = cand;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

#[cfg(test)]
mod doc_tests {
    use super::*;

    fn r() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1600.0, 900.0))
    }

    #[test]
    fn a_whole_page_is_centred_and_fits() {
        let (o, w, h) = doc_placement(r(), [1000.0, 1400.0], 1.0, false, 0.5, 0.0);
        assert!((h - 900.0).abs() < 0.01 && (w - 900.0 * 1000.0 / 1400.0).abs() < 0.01);
        assert!((o.y - 0.0).abs() < 0.01 && (o.x - (800.0 - w / 2.0)).abs() < 0.01, "centred sideways, whole page seen");
    }

    #[test]
    fn fit_width_scrolls_and_never_leaves_a_gap() {
        let (o, w, h) = doc_placement(r(), [1000.0, 1400.0], 1.0, true, 0.5, 0.0);
        assert!((w - 1600.0).abs() < 0.01 && (h - 2240.0).abs() < 0.01);
        assert!(o.y <= 0.0 && o.y + h >= 900.0, "page covers the window");
        assert!((o.y - 0.0).abs() < 0.01, "asked for the top: the top edge sits at the window's top");
        let (o2, _, h2) = doc_placement(r(), [1000.0, 1400.0], 1.0, true, 0.5, 1.0);
        assert!((o2.y + h2 - 900.0).abs() < 0.01, "asked for the bottom: the bottom edge sits at the window's bottom");
        let (o3, _, _) = doc_placement(r(), [1000.0, 1400.0], 1.0, true, 0.5, 0.5);
        assert!(o3.y < 0.0 && o3.y > -(h - 900.0));
    }

    #[test]
    fn zooming_in_on_a_point_keeps_it_in_the_middle() {
        let (o, w, h) = doc_placement(r(), [1000.0, 1000.0], 3.0, false, 0.5, 0.5);
        assert!((o.x + 0.5 * w - 800.0).abs() < 0.01 && (o.y + 0.5 * h - 450.0).abs() < 0.01);
    }
}

#[cfg(test)]
mod colour_tests {
    use super::*;

    #[test]
    fn readable_choice_flips_on_light_backgrounds() {
        assert!(!readable_on([0, 0, 0]));
        assert!(!readable_on([10, 22, 52]));
        assert!(readable_on([255, 255, 255]));
        assert!(readable_on([250, 244, 230]));
        assert_eq!(auto_text_for([0, 0, 40]), [255, 255, 255]);
        assert_eq!(auto_text_for([240, 240, 200]), [20, 16, 10]);
    }

    #[test]
    fn every_preset_has_enough_contrast() {
        fn lum(c: [u8; 3]) -> f32 {
            let f = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
        }
        for (name, bg, fg) in COLOR_PRESETS {
            let (a, b) = (lum(*bg), lum(*fg));
            let ratio = (a.max(b) + 0.05) / (a.min(b) + 0.05);
            assert!(ratio >= 7.0, "{name}: contrast {ratio:.1}:1 is too low to read from the back of a room");
        }
    }
}

/// Stage 2 drawing tests (spec sections 1, 3, 4, 5): paint the stage into a bare egui context and
/// look at what was painted. No window, no Mac needed.
#[cfg(test)]
mod stage2_tests {
    use super::*;
    use egui::epaint::Shape;

    const W: f32 = 1280.0;
    const H: f32 = 720.0;

    fn content(kind: StageKind, label: &str, text: &str) -> StageContent {
        StageContent { kind, label: label.into(), text: text.into(), ..Default::default() }
    }

    fn render(stage: &StageContent, full: bool, zoom: f32, look: StageLook, art: &StageArt) -> Vec<Shape> {
        let ctx = egui::Context::default();
        crate::app::load_stage_fonts(&ctx);
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(W, H)));
        let mut shapes = Vec::new();
        for _ in 0..3 {
            let out = ctx.run(raw.clone(), |ctx| {
                egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| draw_stage(ui, stage, full, zoom, look, art));
            });
            shapes = out.shapes.into_iter().map(|c| c.shape).collect();
        }
        shapes
    }

    fn flat(shapes: &[Shape], out: &mut Vec<Shape>) {
        for s in shapes {
            match s {
                Shape::Vec(v) => flat(v, out),
                other => out.push(other.clone()),
            }
        }
    }

    fn painted(shapes: &[Shape]) -> (Vec<(String, egui::Rect)>, Vec<(egui::TextureId, egui::Rect)>, Vec<(egui::Rect, Color32)>) {
        let mut all = Vec::new();
        flat(shapes, &mut all);
        let (mut texts, mut meshes, mut rects) = (Vec::new(), Vec::new(), Vec::new());
        for s in all {
            match s {
                Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    texts.push((t.galley.text().to_string(), r));
                }
                Shape::Mesh(m) => meshes.push((m.texture_id, m.calc_bounds())),
                Shape::Rect(r) => rects.push((r.rect, r.fill)),
                _ => {}
            }
        }
        (texts, meshes, rects)
    }

    fn look() -> StageLook {
        StageLook::default()
    }
    fn screen() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(W, H))
    }
    fn inside(r: egui::Rect) -> bool {
        r.min.x >= -1.0 && r.min.y >= -1.0 && r.max.x <= W + 1.0 && r.max.y <= H + 1.0
    }
    fn img(id: u64, w: f32, h: f32) -> ArtImage {
        ArtImage { texture: egui::TextureId::User(id), size: [w, h] }
    }

    #[test]
    fn s1_6_slide_lines_stay_lines_and_the_label_is_for_the_operator_only() {
        let st = content(StageKind::Slide, "Chorus", "Amazing grace\nhow sweet the sound");
        let (texts, _, _) = painted(&render(&st, true, 1.0, look(), &StageArt::default()));
        let words: Vec<&str> = texts.iter().map(|t| t.0.as_str()).collect();
        assert!(words.contains(&"Amazing grace") && words.contains(&"how sweet the sound"), "{words:?}");
        assert!(!words.contains(&"Chorus"), "the congregation must not see the label: {words:?}");
        // the operator preview does show it
        let (texts, _, _) = painted(&render(&st, false, 1.0, look(), &StageArt::default()));
        assert!(texts.iter().any(|t| t.0 == "Chorus"));
    }

    #[test]
    fn s1_6_slide_text_never_leaves_the_screen_even_when_long_and_zoomed() {
        let long: String = (1..=14).map(|i| format!("This is a rather long line of a song number {i} that keeps on going and going")).collect::<Vec<_>>().join("\n");
        for zoom in [0.6, 1.0, 2.0, 4.0] {
            let st = content(StageKind::Slide, "", &long);
            let (texts, _, _) = painted(&render(&st, true, zoom, look(), &StageArt::default()));
            assert!(!texts.is_empty());
            for (t, r) in &texts {
                assert!(inside(*r), "zoom {zoom}: {t:?} at {r:?} is off screen");
            }
        }
    }

    #[test]
    fn s1_6_lines_are_centred() {
        let st = content(StageKind::Slide, "", "Short\nA much longer second line of words");
        let (texts, _, _) = painted(&render(&st, true, 1.0, look(), &StageArt::default()));
        for (_, r) in &texts {
            assert!((r.center().x - W / 2.0).abs() < 2.0, "not centred: {r:?}");
        }
    }

    #[test]
    fn s4_1_background_covers_the_screen_then_gets_darkened_then_the_words() {
        let st = content(StageKind::Slide, "", "Words");
        let art = StageArt { background: Some(img(5, 1920.0, 1080.0)), dim_pct: 45, ..Default::default() };
        let shapes = render(&st, true, 1.0, look(), &art);
        let (texts, meshes, rects) = painted(&shapes);
        let bg = meshes.iter().find(|m| m.0 == egui::TextureId::User(5)).expect("background painted");
        assert!(bg.1.contains_rect(screen().shrink(1.0)), "background must cover the whole screen: {:?}", bg.1);
        let dim = rect_alpha(&rects, dim_alpha(45));
        assert!(dim, "a black layer of {} alpha should sit over the background: {rects:?}", dim_alpha(45));
        // order: background, dim, words
        let mut all = Vec::new();
        flat(&shapes, &mut all);
        let pos = |f: &dyn Fn(&Shape) -> bool| all.iter().position(|s| f(s)).unwrap();
        let i_bg = pos(&|s| matches!(s, Shape::Mesh(m) if m.texture_id == egui::TextureId::User(5)));
        let i_dim = pos(&|s| matches!(s, Shape::Rect(r) if r.fill.a() == dim_alpha(45)));
        let i_txt = pos(&|s| matches!(s, Shape::Text(t) if t.galley.text() == "Words"));
        assert!(i_bg < i_dim && i_dim < i_txt, "bg {i_bg}, dim {i_dim}, text {i_txt}");
        // S4.4 shadow: the same words painted twice (shadow then real)
        assert!(texts.iter().filter(|t| t.0 == "Words").count() >= 2, "words need a soft shadow on a picture");
    }

    fn rect_alpha(rects: &[(egui::Rect, Color32)], alpha: u8) -> bool {
        rects.iter().any(|(r, c)| r.contains_rect(screen().shrink(1.0)) && c.a() == alpha && c.r() == 0 && c.g() == 0 && c.b() == 0)
    }

    #[test]
    fn s4_3_zero_darken_paints_no_layer_and_no_background_means_no_shadow_or_layer() {
        let st = content(StageKind::Slide, "", "Words");
        let art = StageArt { background: Some(img(5, 100.0, 100.0)), dim_pct: 0, ..Default::default() };
        let (_, _, rects) = painted(&render(&st, true, 1.0, look(), &art));
        assert!(!rects.iter().any(|(r, c)| r.contains_rect(screen().shrink(1.0)) && c.a() > 0 && c.a() < 255), "{rects:?}");
        let (texts, meshes, _) = painted(&render(&st, true, 1.0, look(), &StageArt::default()));
        assert!(meshes.is_empty());
        assert_eq!(texts.iter().filter(|t| t.0 == "Words").count(), 1, "no shadow without a background");
    }

    #[test]
    fn s4_2_scripture_never_gets_a_background() {
        let st = StageContent { kind: StageKind::Scripture, label: "John 3:16".into(), text: "For God so loved the world".into(), ..Default::default() };
        let art = StageArt { background: Some(img(5, 1920.0, 1080.0)), ..Default::default() };
        let (texts, meshes, _) = painted(&render(&st, true, 1.0, look(), &art));
        assert!(meshes.is_empty(), "scripture stays plain");
        assert!(texts.iter().any(|t| t.0.contains("For God")));
    }

    #[test]
    fn s3_3_a_picture_is_fitted_in_the_screen_by_the_chosen_mode() {
        let st = content(StageKind::Media, "photo.png", "");
        // Fit: a 2:1 picture on a 16:9 screen keeps its shape and sits inside
        let art = StageArt { media: Some(img(9, 2000.0, 1000.0)), fit: FitMode::Fit, ..Default::default() };
        let (_, meshes, _) = painted(&render(&st, true, 1.0, look(), &art));
        let m = meshes.iter().find(|m| m.0 == egui::TextureId::User(9)).expect("picture painted");
        assert!(inside(m.1), "{:?}", m.1);
        assert!((m.1.width() / m.1.height() - 2.0).abs() < 0.02);
        assert!((m.1.center().x - W / 2.0).abs() < 1.0 && (m.1.center().y - H / 2.0).abs() < 1.0);
        // Fill: covers the screen (and is cropped by the clip, which we check via the shape's clip rect elsewhere)
        let art = StageArt { media: Some(img(9, 2000.0, 1000.0)), fit: FitMode::Fill, ..Default::default() };
        let (_, meshes, _) = painted(&render(&st, true, 1.0, look(), &art));
        let m = meshes.iter().find(|m| m.0 == egui::TextureId::User(9)).unwrap();
        assert!(m.1.contains_rect(screen().shrink(1.0)));
        // Stretch: exactly the screen
        let art = StageArt { media: Some(img(9, 2000.0, 1000.0)), fit: FitMode::Stretch, ..Default::default() };
        let (_, meshes, _) = painted(&render(&st, true, 1.0, look(), &art));
        let m = meshes.iter().find(|m| m.0 == egui::TextureId::User(9)).unwrap();
        assert!((m.1.width() - W).abs() < 1.0 && (m.1.height() - H).abs() < 1.0);
    }

    #[test]
    fn s3_3_fill_is_clipped_to_its_box() {
        let st = content(StageKind::Media, "x", "");
        let art = StageArt { media: Some(img(9, 2000.0, 1000.0)), fit: FitMode::Fill, ..Default::default() };
        let ctx = egui::Context::default();
        crate::app::load_stage_fonts(&ctx);
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(screen());
        let mut clipped = Vec::new();
        for _ in 0..3 {
            let out = ctx.run(raw.clone(), |ctx| {
                // a small box inside the window, like the operator's preview card
                egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
                    ui.allocate_ui(egui::vec2(400.0, 225.0), |ui| draw_stage(ui, &st, false, 1.0, look(), &art));
                });
            });
            clipped = out.shapes;
        }
        let c = clipped.iter().find(|c| matches!(&c.shape, Shape::Mesh(m) if m.texture_id == egui::TextureId::User(9))).expect("mesh");
        let boxed = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 225.0)).expand(1.0);
        assert!(boxed.contains_rect(c.clip_rect), "the cropped part of a Fill picture must not spill out of its box: {:?}", c.clip_rect);
    }

    #[test]
    fn s0_5_media_without_pixels_yet_paints_nothing_on_the_stage_and_a_hint_in_the_preview() {
        let st = content(StageKind::Media, "film.mp4", "");
        let (t, m, _) = painted(&render(&st, true, 1.0, look(), &StageArt::default()));
        assert!(t.is_empty() && m.is_empty(), "congregation sees plain stage colour: {t:?}");
        let (t, _, _) = painted(&render(&st, false, 1.0, look(), &StageArt::default()));
        assert!(t.iter().any(|x| x.0.contains("opening")));
    }

    #[test]
    fn s0_1_blackout_hides_slides_pictures_and_countdowns() {
        let mut l = look();
        l.blackout = true;
        let art = StageArt { media: Some(img(9, 100.0, 100.0)), background: Some(img(5, 100.0, 100.0)), ..Default::default() };
        for (kind, text) in [(StageKind::Slide, "Words"), (StageKind::Media, ""), (StageKind::Countdown, "05:00")] {
            let (t, m, _) = painted(&render(&content(kind.clone(), "T", text), true, 1.0, l, &art));
            assert!(t.is_empty() && m.is_empty(), "{kind:?} leaked through blackout: {t:?}");
        }
    }

    #[test]
    fn s5_7_countdown_shows_title_above_big_numbers_and_always_fits() {
        for zoom in [0.6, 1.0, 4.0] {
            let st = content(StageKind::Countdown, "Service starts in", "1:15:00");
            let (texts, _, _) = painted(&render(&st, true, zoom, look(), &StageArt::default()));
            let title = texts.iter().find(|t| t.0 == "Service starts in").expect("title");
            let digits = texts.iter().find(|t| t.0 == "1:15:00").expect("digits");
            assert!(title.1.center().y < digits.1.center().y, "title is above the numbers");
            assert!(digits.1.height() > title.1.height() * 2.0, "numbers are much bigger than the title");
            assert!(inside(title.1) && inside(digits.1), "zoom {zoom}: {:?} {:?}", title.1, digits.1);
            assert!((digits.1.center().x - W / 2.0).abs() < 2.0);
        }
    }

    #[test]
    fn s5_7_countdown_without_a_title_is_just_the_numbers() {
        let st = content(StageKind::Countdown, "", "04:59");
        let (texts, _, _) = painted(&render(&st, true, 1.0, look(), &StageArt::default()));
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].0, "04:59");
    }

    #[test]
    fn s4_1_a_countdown_can_sit_on_a_background_too() {
        let st = content(StageKind::Countdown, "Welcome", "05:00");
        let art = StageArt { background: Some(img(5, 1920.0, 1080.0)), ..Default::default() };
        let (_, meshes, _) = painted(&render(&st, true, 1.0, look(), &art));
        assert!(meshes.iter().any(|m| m.0 == egui::TextureId::User(5)));
    }
}

#[cfg(test)]
mod fade_tests {
    use super::arrival_alpha;

    #[test]
    fn cut_is_instant_and_fades_take_their_time() {
        assert_eq!(arrival_alpha(0, 0), 1.0, "a cut is solid at once");
        assert_eq!(arrival_alpha(0, 350), 0.15, "a fade starts faint, never invisible");
        assert!((arrival_alpha(175, 350) - 0.5).abs() < 0.01);
        assert_eq!(arrival_alpha(350, 350), 1.0);
        assert_eq!(arrival_alpha(5000, 900), 1.0);
        assert!(arrival_alpha(300, 900) < arrival_alpha(300, 350), "slow fade is behind a normal fade at the same moment");
    }
}
