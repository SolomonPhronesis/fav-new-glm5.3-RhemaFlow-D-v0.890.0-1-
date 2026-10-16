//! Feature 6: slide layouts with several boxes (words, picture, clock, timer, QR code, shape), the rules an editor needs
//! (move, resize, snap, align, order, hit-test, duplicate, validate), and a set of ready-made templates.
//!
//! Positions are fractions of the canvas (0 to 1), so one layout fits any screen shape.

use rf_service::Align;
use serde::{Deserialize, Serialize};

pub const MIN_SIZE: f32 = 0.03;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }
    /// Make it at least [`MIN_SIZE`] big and keep it inside the canvas.
    pub fn clamped(self) -> Rect {
        let w = if self.w.is_finite() { self.w.clamp(MIN_SIZE, 1.0) } else { MIN_SIZE };
        let h = if self.h.is_finite() { self.h.clamp(MIN_SIZE, 1.0) } else { MIN_SIZE };
        let x = if self.x.is_finite() { self.x.clamp(0.0, 1.0 - w) } else { 0.0 };
        let y = if self.y.is_finite() { self.y.clamp(0.0, 1.0 - h) } else { 0.0 };
        Rect { x, y, w, h }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VAlign {
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transform {
    None,
    Upper,
    Lower,
    Title,
}

pub fn apply_transform(text: &str, t: Transform) -> String {
    match t {
        Transform::None => text.to_string(),
        Transform::Upper => text.to_uppercase(),
        Transform::Lower => text.to_lowercase(),
        Transform::Title => {
            let mut out = String::with_capacity(text.len());
            let mut start = true;
            for c in text.chars() {
                if c.is_alphabetic() {
                    if start {
                        out.extend(c.to_uppercase());
                    } else {
                        out.extend(c.to_lowercase());
                    }
                    start = false;
                } else {
                    out.push(c);
                    start = !(c == '\'' || c == '\u{2019}');
                }
            }
            out
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Outline {
    pub width: f32,
    pub rgb: [u8; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub dx: f32,
    pub dy: f32,
    pub blur: f32,
    pub rgb: [u8; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    pub font: String,
    pub size_pt: f32,
    pub rgb: [u8; 3],
    pub bold: bool,
    pub italic: bool,
    pub align: Align,
    pub valign: VAlign,
    pub line_spacing: f32,
    pub transform: Transform,
    pub outline: Option<Outline>,
    pub shadow: Option<Shadow>,
    /// Shrink to fit the box instead of overflowing.
    pub auto_fit: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle { font: "serif".into(), size_pt: 48.0, rgb: [244, 239, 228], bold: false, italic: false, align: Align::Center, valign: VAlign::Middle, line_spacing: 1.2, transform: Transform::None, outline: None, shadow: None, auto_fit: true }
    }
}

/// What a box shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Source {
    /// Words typed into the layout. May contain `{date}` and friends.
    Text(String),
    /// The slide on the screen now / the one coming next / the operator's private notes.
    Slide,
    NextSlide,
    Notes,
    /// "John 3:16", the song or item title, the alert message, the chords for the slide.
    Reference,
    Title,
    Message,
    Chords,
    Clock,
    Timer(String),
    Image(String),
    Video(String),
    Qr(String),
    /// A coloured rectangle (red, green, blue, alpha).
    Shape([u8; 4]),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutBox {
    pub id: u32,
    pub name: String,
    pub source: Source,
    pub rect: Rect,
    pub style: TextStyle,
    pub opacity: f32,
    pub z: i32,
    pub locked: bool,
    pub visible: bool,
    /// Leave the box out when it has nothing to show (a note, a message).
    pub hide_when_empty: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    pub aspect_w: u32,
    pub aspect_h: u32,
    pub background: [u8; 3],
    pub boxes: Vec<LayoutBox>,
    next_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignOp {
    Left,
    Right,
    Top,
    Bottom,
    CenterH,
    CenterV,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub id: Option<u32>,
    pub message: String,
}

/// WCAG contrast ratio between two colours, 1 (none) to 21 (black on white).
pub fn contrast_ratio(a: [u8; 3], b: [u8; 3]) -> f32 {
    fn lum(c: [u8; 3]) -> f32 {
        let f = |v: u8| {
            let s = v as f32 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
    }
    let (la, lb) = (lum(a), lum(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

impl Layout {
    pub fn new(name: &str) -> Layout {
        Layout { name: name.to_string(), aspect_w: 16, aspect_h: 9, background: [0, 0, 0], boxes: Vec::new(), next_id: 1 }
    }

    pub fn add(&mut self, name: &str, source: Source, rect: Rect) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let z = self.boxes.iter().map(|b| b.z).max().map_or(0, |m| m + 1);
        let hide = matches!(source, Source::Notes | Source::Message);
        self.boxes.push(LayoutBox { id, name: name.to_string(), source, rect: rect.clamped(), style: TextStyle::default(), opacity: 1.0, z, locked: false, visible: true, hide_when_empty: hide });
        id
    }

    pub fn get(&self, id: u32) -> Option<&LayoutBox> {
        self.boxes.iter().find(|b| b.id == id)
    }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut LayoutBox> {
        self.boxes.iter_mut().find(|b| b.id == id)
    }
    pub fn remove(&mut self, id: u32) -> bool {
        let n = self.boxes.len();
        self.boxes.retain(|b| b.id != id);
        self.boxes.len() != n
    }

    /// Move a box. Locked boxes stay put. The box never leaves the canvas.
    pub fn move_by(&mut self, id: u32, dx: f32, dy: f32) -> bool {
        match self.get_mut(id) {
            Some(b) if !b.locked => {
                b.rect = Rect { x: b.rect.x + dx, y: b.rect.y + dy, ..b.rect }.clamped();
                true
            }
            _ => false,
        }
    }

    /// Drag one handle of a box by (dx, dy). The opposite edges stay where they are, the size never drops below the minimum.
    pub fn resize(&mut self, id: u32, handle: Handle, dx: f32, dy: f32) -> bool {
        let Some(b) = self.get_mut(id) else { return false };
        if b.locked {
            return false;
        }
        let (mut l, mut t, mut r, mut bt) = (b.rect.x, b.rect.y, b.rect.right(), b.rect.bottom());
        let west = matches!(handle, Handle::W | Handle::NW | Handle::SW);
        let east = matches!(handle, Handle::E | Handle::NE | Handle::SE);
        let north = matches!(handle, Handle::N | Handle::NE | Handle::NW);
        let south = matches!(handle, Handle::S | Handle::SE | Handle::SW);
        if west {
            l = (l + dx).clamp(0.0, r - MIN_SIZE);
        }
        if east {
            r = (r + dx).clamp(l + MIN_SIZE, 1.0);
        }
        if north {
            t = (t + dy).clamp(0.0, bt - MIN_SIZE);
        }
        if south {
            bt = (bt + dy).clamp(t + MIN_SIZE, 1.0);
        }
        b.rect = Rect { x: l, y: t, w: r - l, h: bt - t };
        true
    }

    /// Positions other boxes and the canvas offer for snapping: (vertical lines at x, horizontal lines at y).
    pub fn guides(&self, except: u32) -> (Vec<f32>, Vec<f32>) {
        let mut xs = vec![0.0, 0.5, 1.0];
        let mut ys = vec![0.0, 0.5, 1.0];
        for b in self.boxes.iter().filter(|b| b.id != except && b.visible) {
            xs.extend([b.rect.x, b.rect.x + b.rect.w / 2.0, b.rect.right()]);
            ys.extend([b.rect.y, b.rect.y + b.rect.h / 2.0, b.rect.bottom()]);
        }
        (xs, ys)
    }

    /// Nudge `r` so its left / centre / right (and top / middle / bottom) sit on a guide that is closer than `threshold`.
    pub fn snap(r: Rect, guides: &(Vec<f32>, Vec<f32>), threshold: f32) -> Rect {
        fn best(points: [f32; 3], lines: &[f32], thr: f32) -> f32 {
            let mut best: Option<f32> = None;
            for p in points {
                for l in lines {
                    let d = l - p;
                    if d.abs() <= thr && best.map_or(true, |b| d.abs() < b.abs()) {
                        best = Some(d);
                    }
                }
            }
            best.unwrap_or(0.0)
        }
        let dx = best([r.x, r.x + r.w / 2.0, r.right()], &guides.0, threshold);
        let dy = best([r.y, r.y + r.h / 2.0, r.bottom()], &guides.1, threshold);
        Rect { x: r.x + dx, y: r.y + dy, ..r }.clamped()
    }

    /// Round to the nearest grid line (grid of 0.0 means no grid).
    pub fn snap_to_grid(v: f32, grid: f32) -> f32 {
        if grid <= 0.0 {
            v
        } else {
            (v / grid).round() * grid
        }
    }

    /// Box ids from back to front.
    pub fn paint_order(&self) -> Vec<u32> {
        let mut v: Vec<(i32, usize, u32)> = self.boxes.iter().enumerate().map(|(i, b)| (b.z, i, b.id)).collect();
        v.sort();
        v.into_iter().map(|t| t.2).collect()
    }

    /// The front-most box at a point (visible and not locked): what a click selects.
    pub fn hit(&self, x: f32, y: f32) -> Option<u32> {
        self.paint_order().into_iter().rev().find(|id| self.get(*id).map_or(false, |b| b.visible && !b.locked && b.rect.contains(x, y)))
    }

    fn set_order(&mut self, order: Vec<u32>) {
        for (z, id) in order.into_iter().enumerate() {
            if let Some(b) = self.get_mut(id) {
                b.z = z as i32;
            }
        }
    }

    pub fn bring_to_front(&mut self, id: u32) {
        let mut o = self.paint_order();
        if let Some(p) = o.iter().position(|x| *x == id) {
            let v = o.remove(p);
            o.push(v);
            self.set_order(o);
        }
    }
    pub fn send_to_back(&mut self, id: u32) {
        let mut o = self.paint_order();
        if let Some(p) = o.iter().position(|x| *x == id) {
            let v = o.remove(p);
            o.insert(0, v);
            self.set_order(o);
        }
    }
    pub fn bring_forward(&mut self, id: u32) {
        let mut o = self.paint_order();
        if let Some(p) = o.iter().position(|x| *x == id) {
            if p + 1 < o.len() {
                o.swap(p, p + 1);
                self.set_order(o);
            }
        }
    }
    pub fn send_backward(&mut self, id: u32) {
        let mut o = self.paint_order();
        if let Some(p) = o.iter().position(|x| *x == id) {
            if p > 0 {
                o.swap(p, p - 1);
                self.set_order(o);
            }
        }
    }

    /// Line up the chosen boxes. With one box it lines up to the canvas; with several, to the group's outer edge (or centre).
    pub fn align(&mut self, ids: &[u32], op: AlignOp) {
        let rects: Vec<(u32, Rect)> = ids.iter().filter_map(|id| self.get(*id).filter(|b| !b.locked).map(|b| (*id, b.rect))).collect();
        if rects.is_empty() {
            return;
        }
        let (l, t, r, b) = if rects.len() == 1 {
            (0.0, 0.0, 1.0, 1.0)
        } else {
            (rects.iter().map(|x| x.1.x).fold(f32::MAX, f32::min), rects.iter().map(|x| x.1.y).fold(f32::MAX, f32::min), rects.iter().map(|x| x.1.right()).fold(f32::MIN, f32::max), rects.iter().map(|x| x.1.bottom()).fold(f32::MIN, f32::max))
        };
        for (id, rc) in rects {
            let mut n = rc;
            match op {
                AlignOp::Left => n.x = l,
                AlignOp::Right => n.x = r - rc.w,
                AlignOp::Top => n.y = t,
                AlignOp::Bottom => n.y = b - rc.h,
                AlignOp::CenterH => n.x = (l + r) / 2.0 - rc.w / 2.0,
                AlignOp::CenterV => n.y = (t + b) / 2.0 - rc.h / 2.0,
            }
            if let Some(bx) = self.get_mut(id) {
                bx.rect = n.clamped();
            }
        }
    }

    /// Equal gaps between three or more boxes, keeping the outer two where they are.
    pub fn distribute(&mut self, ids: &[u32], horizontal: bool) {
        let mut items: Vec<(u32, Rect)> = ids.iter().filter_map(|id| self.get(*id).filter(|b| !b.locked).map(|b| (*id, b.rect))).collect();
        if items.len() < 3 {
            return;
        }
        items.sort_by(|a, b| {
            let (ka, kb) = if horizontal { (a.1.x, b.1.x) } else { (a.1.y, b.1.y) };
            ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
        });
        let first = items[0].1;
        let last = items[items.len() - 1].1;
        let (start, end) = if horizontal { (first.right(), last.x) } else { (first.bottom(), last.y) };
        let inner: f32 = items[1..items.len() - 1].iter().map(|(_, r)| if horizontal { r.w } else { r.h }).sum();
        let gap = (end - start - inner) / (items.len() as f32 - 1.0);
        let mut cursor = start + gap;
        for (id, r) in &items[1..items.len() - 1] {
            if let Some(b) = self.get_mut(*id) {
                if horizontal {
                    b.rect.x = cursor;
                } else {
                    b.rect.y = cursor;
                }
                b.rect = b.rect.clamped();
            }
            cursor += gap + if horizontal { r.w } else { r.h };
        }
    }

    pub fn duplicate(&mut self, id: u32) -> Option<u32> {
        let src = self.get(id)?.clone();
        let new = self.next_id;
        self.next_id += 1;
        let z = self.boxes.iter().map(|b| b.z).max().unwrap_or(0) + 1;
        let mut copy = src;
        copy.id = new;
        copy.name = format!("{} copy", copy.name);
        copy.rect = Rect { x: copy.rect.x + 0.02, y: copy.rect.y + 0.02, ..copy.rect }.clamped();
        copy.z = z;
        copy.locked = false;
        self.boxes.push(copy);
        Some(new)
    }

    /// Things an operator should fix before using the layout.
    pub fn validate(&self) -> Vec<Problem> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if self.aspect_w == 0 || self.aspect_h == 0 {
            out.push(Problem { id: None, message: "The screen shape (aspect ratio) has a zero in it.".into() });
        }
        for b in &self.boxes {
            if !seen.insert(b.id) {
                out.push(Problem { id: Some(b.id), message: format!("Two boxes share the number {}.", b.id) });
            }
            let r = b.rect;
            if r.w < MIN_SIZE - 1e-6 || r.h < MIN_SIZE - 1e-6 {
                out.push(Problem { id: Some(b.id), message: format!("\"{}\" is too small to see.", b.name) });
            }
            if r.x < -1e-6 || r.y < -1e-6 || r.right() > 1.0 + 1e-4 || r.bottom() > 1.0 + 1e-4 {
                out.push(Problem { id: Some(b.id), message: format!("\"{}\" sticks out past the edge of the screen.", b.name) });
            }
            if b.visible && b.opacity <= 0.0 {
                out.push(Problem { id: Some(b.id), message: format!("\"{}\" is on but fully see-through.", b.name) });
            }
            let texty = matches!(b.source, Source::Text(_) | Source::Slide | Source::NextSlide | Source::Notes | Source::Reference | Source::Title | Source::Message | Source::Chords | Source::Clock | Source::Timer(_));
            if texty && b.visible {
                if b.style.size_pt < 8.0 {
                    out.push(Problem { id: Some(b.id), message: format!("\"{}\" has tiny letters ({} pt).", b.name, b.style.size_pt) });
                }
                // contrast against a shape behind it, else against the layout background
                let behind = self
                    .paint_order()
                    .into_iter()
                    .filter_map(|i| self.get(i))
                    .filter(|o| o.id != b.id && o.z < b.z && o.visible && matches!(o.source, Source::Shape(c) if c[3] >= 128))
                    .filter(|o| o.rect.x <= b.rect.x + 1e-4 && o.rect.y <= b.rect.y + 1e-4 && o.rect.right() >= b.rect.right() - 1e-4 && o.rect.bottom() >= b.rect.bottom() - 1e-4)
                    .last()
                    .and_then(|o| if let Source::Shape(c) = o.source { Some([c[0], c[1], c[2]]) } else { None })
                    .unwrap_or(self.background);
                if contrast_ratio(b.style.rgb, behind) < 3.0 {
                    out.push(Problem { id: Some(b.id), message: format!("The words in \"{}\" are hard to read against what is behind them.", b.name) });
                }
            }
        }
        out
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Read a saved layout. Boxes that stick out are pulled back in, so a damaged file cannot draw off-screen.
    pub fn from_json(s: &str) -> Result<Layout, String> {
        let mut l: Layout = serde_json::from_str(s).map_err(|e| format!("not a layout file: {e}"))?;
        if l.aspect_w == 0 || l.aspect_h == 0 {
            l.aspect_w = 16;
            l.aspect_h = 9;
        }
        let max_id = l.boxes.iter().map(|b| b.id).max().unwrap_or(0);
        l.next_id = l.next_id.max(max_id + 1);
        for b in &mut l.boxes {
            b.rect = b.rect.clamped();
            b.opacity = if b.opacity.is_finite() { b.opacity.clamp(0.0, 1.0) } else { 1.0 };
            b.style.size_pt = if b.style.size_pt.is_finite() { b.style.size_pt.clamp(1.0, 600.0) } else { 48.0 };
        }
        Ok(l)
    }
}

// ---------------------------------------------------------------- templates

fn styled(l: &mut Layout, name: &str, src: Source, r: Rect, f: impl FnOnce(&mut TextStyle)) -> u32 {
    let id = l.add(name, src, r);
    if let Some(b) = l.get_mut(id) {
        f(&mut b.style);
    }
    id
}

/// The built-in templates, by name.
pub fn templates() -> Vec<Layout> {
    let mut all = Vec::new();

    let mut l = Layout::new("Song lyrics");
    styled(&mut l, "Words", Source::Slide, Rect::new(0.05, 0.06, 0.9, 0.8), |s| s.size_pt = 60.0);
    styled(&mut l, "Copyright line", Source::Text("{copyright}".into()), Rect::new(0.05, 0.9, 0.9, 0.06), |s| {
        s.size_pt = 16.0;
        s.rgb = [190, 190, 190];
    });
    if let Some(b) = l.boxes.last_mut() {
        b.hide_when_empty = true;
    }
    all.push(l);

    let mut l = Layout::new("Scripture");
    styled(&mut l, "Verse", Source::Slide, Rect::new(0.08, 0.08, 0.84, 0.64), |s| s.size_pt = 56.0);
    styled(&mut l, "Reference", Source::Reference, Rect::new(0.08, 0.76, 0.84, 0.14), |s| {
        s.size_pt = 36.0;
        s.bold = true;
        s.align = Align::Right;
    });
    all.push(l);

    let mut l = Layout::new("Title and subtitle");
    styled(&mut l, "Title", Source::Title, Rect::new(0.1, 0.28, 0.8, 0.22), |s| {
        s.size_pt = 80.0;
        s.bold = true;
    });
    styled(&mut l, "Subtitle", Source::Text("{date}".into()), Rect::new(0.1, 0.54, 0.8, 0.1), |s| s.size_pt = 32.0);
    all.push(l);

    let mut l = Layout::new("Two columns");
    styled(&mut l, "Left", Source::Text("Left side".into()), Rect::new(0.05, 0.1, 0.43, 0.8), |s| {
        s.size_pt = 40.0;
        s.align = Align::Left;
        s.valign = VAlign::Top;
    });
    styled(&mut l, "Right", Source::Text("Right side".into()), Rect::new(0.52, 0.1, 0.43, 0.8), |s| {
        s.size_pt = 40.0;
        s.align = Align::Left;
        s.valign = VAlign::Top;
    });
    all.push(l);

    let mut l = Layout::new("Announcement with picture");
    l.add("Picture", Source::Image(String::new()), Rect::new(0.04, 0.08, 0.44, 0.84));
    styled(&mut l, "Words", Source::Slide, Rect::new(0.52, 0.08, 0.44, 0.84), |s| {
        s.size_pt = 44.0;
        s.align = Align::Left;
    });
    all.push(l);

    let mut l = Layout::new("Lower third");
    l.add("Bar", Source::Shape([0, 0, 0, 190]), Rect::new(0.0, 0.74, 1.0, 0.26));
    styled(&mut l, "Words", Source::Slide, Rect::new(0.04, 0.76, 0.92, 0.2), |s| {
        s.size_pt = 40.0;
        s.align = Align::Left;
        s.rgb = [255, 255, 255];
    });
    all.push(l);

    let mut l = Layout::new("Stage: preacher");
    styled(&mut l, "Now", Source::Slide, Rect::new(0.04, 0.06, 0.62, 0.5), |s| s.size_pt = 48.0);
    styled(&mut l, "Next", Source::NextSlide, Rect::new(0.04, 0.6, 0.62, 0.22), |s| {
        s.size_pt = 28.0;
        s.rgb = [160, 160, 160];
    });
    styled(&mut l, "Clock", Source::Clock, Rect::new(0.7, 0.06, 0.26, 0.14), |s| s.size_pt = 44.0);
    styled(&mut l, "Sermon timer", Source::Timer("Sermon".into()), Rect::new(0.7, 0.24, 0.26, 0.16), |s| {
        s.size_pt = 52.0;
        s.bold = true;
    });
    styled(&mut l, "Notes", Source::Notes, Rect::new(0.7, 0.44, 0.26, 0.4), |s| {
        s.size_pt = 22.0;
        s.align = Align::Left;
        s.valign = VAlign::Top;
    });
    styled(&mut l, "Message", Source::Message, Rect::new(0.04, 0.86, 0.92, 0.1), |s| {
        s.size_pt = 34.0;
        s.rgb = [255, 200, 0];
        s.bold = true;
    });
    all.push(l);

    let mut l = Layout::new("Stage: worship leader");
    styled(&mut l, "Chords", Source::Chords, Rect::new(0.04, 0.06, 0.92, 0.52), |s| {
        s.font = "mono".into();
        s.size_pt = 34.0;
        s.align = Align::Left;
        s.valign = VAlign::Top;
    });
    styled(&mut l, "Next", Source::NextSlide, Rect::new(0.04, 0.62, 0.92, 0.26), |s| {
        s.size_pt = 28.0;
        s.rgb = [160, 160, 160];
    });
    styled(&mut l, "Clock", Source::Clock, Rect::new(0.8, 0.9, 0.16, 0.08), |s| s.size_pt = 24.0);
    styled(&mut l, "Message", Source::Message, Rect::new(0.04, 0.9, 0.74, 0.08), |s| {
        s.size_pt = 28.0;
        s.rgb = [255, 200, 0];
    });
    all.push(l);

    all
}

pub fn template(name: &str) -> Option<Layout> {
    templates().into_iter().find(|l| l.name.eq_ignore_ascii_case(name.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two() -> (Layout, u32, u32) {
        let mut l = Layout::new("t");
        let a = l.add("a", Source::Slide, Rect::new(0.1, 0.1, 0.3, 0.3));
        let b = l.add("b", Source::Notes, Rect::new(0.5, 0.5, 0.3, 0.3));
        (l, a, b)
    }

    #[test]
    fn boxes_start_inside_the_canvas_whatever_was_asked() {
        let mut l = Layout::new("t");
        let id = l.add("x", Source::Slide, Rect::new(0.9, 0.9, 0.5, 0.5));
        let r = l.get(id).unwrap().rect;
        assert!(r.right() <= 1.0 + 1e-6 && r.bottom() <= 1.0 + 1e-6 && r.x >= 0.0);
        let nan = l.add("n", Source::Slide, Rect::new(f32::NAN, f32::INFINITY, f32::NAN, -5.0));
        let r = l.get(nan).unwrap().rect;
        assert!(r.w >= MIN_SIZE && r.h >= MIN_SIZE && r.x >= 0.0 && r.y >= 0.0);
    }

    #[test]
    fn moving_is_clamped_and_locked_boxes_stay() {
        let (mut l, a, b) = two();
        assert!(l.move_by(a, -5.0, 5.0));
        let r = l.get(a).unwrap().rect;
        assert_eq!((r.x, r.bottom()), (0.0, 1.0));
        l.get_mut(b).unwrap().locked = true;
        assert!(!l.move_by(b, 0.1, 0.1));
        assert_eq!(l.get(b).unwrap().rect.x, 0.5);
        assert!(!l.move_by(999, 0.1, 0.1));
    }

    #[test]
    fn resizing_keeps_the_far_edge_still_and_a_minimum_size() {
        let (mut l, a, _) = two();
        l.resize(a, Handle::SE, 0.2, 0.1);
        let r = l.get(a).unwrap().rect;
        assert!((r.x - 0.1).abs() < 1e-6 && (r.w - 0.5).abs() < 1e-6 && (r.h - 0.4).abs() < 1e-6);
        l.resize(a, Handle::W, 5.0, 0.0); // drag the left edge far right
        let r = l.get(a).unwrap().rect;
        assert!(r.w >= MIN_SIZE - 1e-6);
        assert!((r.right() - 0.6).abs() < 1e-6);
        l.resize(a, Handle::NW, -5.0, -5.0);
        let r = l.get(a).unwrap().rect;
        assert!(r.x >= 0.0 && r.y >= 0.0);
        l.get_mut(a).unwrap().locked = true;
        assert!(!l.resize(a, Handle::E, 0.1, 0.0));
    }

    #[test]
    fn snapping_pulls_edges_to_guides_only_when_close() {
        let (l, a, _) = two();
        let g = l.guides(a);
        let near = Layout::snap(Rect::new(0.49, 0.2, 0.2, 0.1), &g, 0.02); // left edge is 0.01 from box b's left edge
        assert!((near.x - 0.5).abs() < 1e-6);
        let far = Layout::snap(Rect::new(0.33, 0.2, 0.1, 0.1), &g, 0.005);
        assert_eq!(far.x, 0.33);
        let centred = Layout::snap(Rect::new(0.4, 0.4, 0.195, 0.2), &g, 0.02); // centre 0.4975 -> 0.5
        assert!((centred.x + centred.w / 2.0 - 0.5).abs() < 1e-5);
        assert_eq!(Layout::snap_to_grid(0.134, 0.05), 0.15000001);
        assert_eq!(Layout::snap_to_grid(0.134, 0.0), 0.134);
    }

    #[test]
    fn clicking_selects_the_front_box_and_skips_locked_or_hidden_ones() {
        let mut l = Layout::new("t");
        let back = l.add("back", Source::Slide, Rect::new(0.0, 0.0, 1.0, 1.0));
        let front = l.add("front", Source::Notes, Rect::new(0.2, 0.2, 0.3, 0.3));
        assert_eq!(l.hit(0.3, 0.3), Some(front));
        assert_eq!(l.hit(0.9, 0.9), Some(back));
        l.get_mut(front).unwrap().locked = true;
        assert_eq!(l.hit(0.3, 0.3), Some(back));
        l.get_mut(back).unwrap().visible = false;
        assert_eq!(l.hit(0.3, 0.3), None);
    }

    #[test]
    fn stacking_order_changes_who_is_in_front() {
        let mut l = Layout::new("t");
        let a = l.add("a", Source::Slide, Rect::new(0.0, 0.0, 0.5, 0.5));
        let b = l.add("b", Source::Slide, Rect::new(0.0, 0.0, 0.5, 0.5));
        let c = l.add("c", Source::Slide, Rect::new(0.0, 0.0, 0.5, 0.5));
        assert_eq!(l.paint_order(), vec![a, b, c]);
        l.send_to_back(c);
        assert_eq!(l.paint_order(), vec![c, a, b]);
        l.bring_to_front(c);
        assert_eq!(l.paint_order(), vec![a, b, c]);
        l.send_backward(c);
        assert_eq!(l.paint_order(), vec![a, c, b]);
        l.bring_forward(a);
        assert_eq!(l.paint_order(), vec![c, a, b]);
        l.bring_forward(b); // already in front: nothing changes
        assert_eq!(l.paint_order(), vec![c, a, b]);
        assert_eq!(l.hit(0.1, 0.1), Some(b));
    }

    #[test]
    fn aligning_one_box_goes_to_the_canvas_and_many_to_each_other() {
        let (mut l, a, b) = two();
        l.align(&[a], AlignOp::Right);
        assert!((l.get(a).unwrap().rect.right() - 1.0).abs() < 1e-6);
        l.align(&[a], AlignOp::CenterH);
        let r = l.get(a).unwrap().rect;
        assert!((r.x + r.w / 2.0 - 0.5).abs() < 1e-6);
        l.align(&[a, b], AlignOp::Top);
        assert_eq!(l.get(a).unwrap().rect.y, l.get(b).unwrap().rect.y);
        l.align(&[], AlignOp::Top); // nothing chosen: nothing happens
    }

    #[test]
    fn distributing_makes_equal_gaps() {
        let mut l = Layout::new("t");
        let a = l.add("a", Source::Slide, Rect::new(0.0, 0.0, 0.1, 0.1));
        let b = l.add("b", Source::Slide, Rect::new(0.2, 0.0, 0.1, 0.1));
        let c = l.add("c", Source::Slide, Rect::new(0.9, 0.0, 0.1, 0.1));
        l.distribute(&[a, b, c], true);
        let (ra, rb, rc) = (l.get(a).unwrap().rect, l.get(b).unwrap().rect, l.get(c).unwrap().rect);
        assert!(((rb.x - ra.right()) - (rc.x - rb.right())).abs() < 1e-5);
        assert_eq!(rc.x, 0.9);
        l.distribute(&[a, b], true); // two boxes: nothing to do
    }

    #[test]
    fn duplicates_are_new_unlocked_and_in_front() {
        let (mut l, a, _) = two();
        l.get_mut(a).unwrap().locked = true;
        let d = l.duplicate(a).unwrap();
        assert_ne!(d, a);
        let c = l.get(d).unwrap();
        assert!(!c.locked && c.name == "a copy");
        assert_eq!(l.paint_order().last(), Some(&d));
        assert!(l.duplicate(999).is_none());
    }

    #[test]
    fn validation_finds_the_usual_mistakes() {
        let mut l = Layout::new("t");
        let a = l.add("words", Source::Text("hi".into()), Rect::new(0.1, 0.1, 0.5, 0.2));
        l.get_mut(a).unwrap().style.rgb = [5, 5, 5]; // dark on black
        let b = l.add("tiny", Source::Text("x".into()), Rect::new(0.5, 0.5, 0.2, 0.2));
        l.get_mut(b).unwrap().style.size_pt = 5.0;
        let c = l.add("ghost", Source::Slide, Rect::new(0.0, 0.0, 0.2, 0.2));
        l.get_mut(c).unwrap().opacity = 0.0;
        l.get_mut(c).unwrap().rect = Rect::new(0.9, 0.9, 0.3, 0.3); // forced past the edge
        let msgs: Vec<String> = l.validate().into_iter().map(|p| p.message).collect();
        assert!(msgs.iter().any(|m| m.contains("hard to read") && m.contains("words")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("tiny letters")));
        assert!(msgs.iter().any(|m| m.contains("past the edge")));
        assert!(msgs.iter().any(|m| m.contains("see-through")));
        assert!(Layout::new("empty").validate().is_empty());
    }

    #[test]
    fn contrast_is_measured_against_a_bar_behind_the_words() {
        let mut l = Layout::new("t");
        l.background = [255, 255, 255];
        l.add("bar", Source::Shape([0, 0, 0, 255]), Rect::new(0.0, 0.6, 1.0, 0.4));
        let w = l.add("w", Source::Text("hi".into()), Rect::new(0.1, 0.7, 0.5, 0.2));
        l.get_mut(w).unwrap().style.rgb = [255, 255, 255]; // white on the black bar: fine even though the page is white
        assert!(l.validate().is_empty(), "{:?}", l.validate());
        assert!((contrast_ratio([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 0.01);
        assert!((contrast_ratio([9, 9, 9], [9, 9, 9]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn text_transforms() {
        assert_eq!(apply_transform("Hello World", Transform::Upper), "HELLO WORLD");
        assert_eq!(apply_transform("Hello World", Transform::Lower), "hello world");
        assert_eq!(apply_transform("the LORD's own-people", Transform::Title), "The Lord's Own-People");
        assert_eq!(apply_transform("señor ñandú", Transform::Upper), "SEÑOR ÑANDÚ");
        assert_eq!(apply_transform("keep", Transform::None), "keep");
    }

    #[test]
    fn saving_and_loading_keeps_everything_and_repairs_damage() {
        let t = template("scripture").unwrap();
        let back = Layout::from_json(&t.to_json()).unwrap();
        assert_eq!(back, t);
        let mut bad = t.clone();
        bad.boxes[0].rect = Rect { x: 5.0, y: -3.0, w: 9.0, h: 0.0 };
        bad.boxes[0].opacity = 7.0;
        bad.aspect_w = 0;
        let fixed = Layout::from_json(&bad.to_json()).unwrap();
        assert_eq!(fixed.boxes[0].opacity, 1.0);
        let r = fixed.boxes[0].rect;
        assert!(r.x >= 0.0 && r.right() <= 1.0 + 1e-5 && r.h >= MIN_SIZE);
        assert_eq!(fixed.aspect_w, 16);
        assert!(Layout::from_json("not json").is_err());
        let mut l = fixed.clone();
        let id = l.add("new", Source::Slide, Rect::new(0.1, 0.1, 0.1, 0.1));
        assert!(fixed.boxes.iter().all(|b| b.id != id), "a new box must never reuse an old id");
    }

    #[test]
    fn every_built_in_template_is_clean() {
        let all = templates();
        assert!(all.len() >= 8);
        for l in &all {
            assert!(l.validate().is_empty(), "{}: {:?}", l.name, l.validate());
            assert!(!l.boxes.is_empty());
        }
        let names: Vec<&str> = all.iter().map(|l| l.name.as_str()).collect();
        assert!(names.contains(&"Stage: preacher") && names.contains(&"Lower third"));
        assert!(template("nope").is_none());
    }
}
