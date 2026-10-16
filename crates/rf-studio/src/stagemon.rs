//! Feature 12: confidence-monitor layouts. A layout (feature 6) plus what is happening now gives the words to draw
//! on the preacher's, the band's and the tech desk's screens. Hidden when empty, timer colours, role presets.

use crate::layout::{apply_transform, template, Layout, LayoutBox, Rect, Source, TextStyle};
use crate::vars::{self, Ctx};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Preacher,
    Worship,
    Tech,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Preacher, Role::Worship, Role::Tech];
    pub fn label(self) -> &'static str {
        match self {
            Role::Preacher => "Preacher",
            Role::Worship => "Worship leader / band",
            Role::Tech => "Tech desk",
        }
    }
}

/// The layout a role starts with. The tech desk gets the preacher's screen plus the title and the reference.
pub fn layout_for(role: Role) -> Layout {
    match role {
        Role::Preacher => template("Stage: preacher").unwrap_or_else(|| Layout::new("Stage: preacher")),
        Role::Worship => template("Stage: worship leader").unwrap_or_else(|| Layout::new("Stage: worship leader")),
        Role::Tech => {
            let mut l = template("Stage: preacher").unwrap_or_else(|| Layout::new("Stage: tech"));
            l.name = "Stage: tech".into();
            let t = l.add("Item", Source::Title, Rect::new(0.04, 0.0, 0.5, 0.06));
            if let Some(b) = l.get_mut(t) {
                b.style.size_pt = 22.0;
                b.style.rgb = [170, 170, 170];
            }
            let r = l.add("Reference", Source::Reference, Rect::new(0.56, 0.0, 0.4, 0.06));
            if let Some(b) = l.get_mut(r) {
                b.style.size_pt = 22.0;
                b.style.rgb = [170, 170, 170];
            }
            l
        }
    }
}

/// Everything a stage screen can show.
#[derive(Clone, Debug, Default)]
pub struct StageCtx {
    pub slide: String,
    pub next: String,
    pub notes: String,
    pub reference: String,
    pub title: String,
    pub message: String,
    pub chords: String,
    pub vars: Ctx,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    pub id: u32,
    pub rect: Rect,
    /// `None` for pictures, video, QR codes and shapes (the painter draws those from `source`).
    pub text: Option<String>,
    pub source: Source,
    pub style: TextStyle,
    pub opacity: f32,
}

/// Timer colour: green while there is time, amber in the last two minutes, red when over.
pub fn timer_color(secs_left: i64) -> [u8; 3] {
    if secs_left < 0 {
        [235, 60, 60]
    } else if secs_left <= 120 {
        [255, 180, 0]
    } else {
        [80, 200, 120]
    }
}

fn text_of(b: &LayoutBox, c: &StageCtx) -> Option<String> {
    let raw = match &b.source {
        Source::Text(t) => vars::render(t, &c.vars).text,
        Source::Slide => c.slide.clone(),
        Source::NextSlide => c.next.clone(),
        Source::Notes => c.notes.clone(),
        Source::Reference => c.reference.clone(),
        Source::Title => c.title.clone(),
        Source::Message => c.message.clone(),
        Source::Chords => c.chords.clone(),
        Source::Clock => vars::render("{time}", &c.vars).text,
        Source::Timer(name) => {
            let r = vars::render(&format!("{{countdown:{name}}}"), &c.vars);
            if r.unknown.is_empty() {
                r.text
            } else {
                "--:--".to_string()
            }
        }
        Source::Image(_) | Source::Video(_) | Source::Qr(_) | Source::Shape(_) => return None,
    };
    Some(apply_transform(&raw, b.style.transform))
}

/// The boxes to draw, back to front. Empty boxes marked "hide when empty" are left out, and so are hidden boxes.
pub fn resolve(layout: &Layout, ctx: &StageCtx) -> Vec<Drawn> {
    let mut out = Vec::new();
    for id in layout.paint_order() {
        let Some(b) = layout.get(id) else { continue };
        if !b.visible {
            continue;
        }
        let text = text_of(b, ctx);
        if b.hide_when_empty && text.as_deref().map_or(false, |t| t.trim().is_empty()) {
            continue;
        }
        let mut style = b.style.clone();
        if let Source::Timer(name) = &b.source {
            if let Some(secs) = ctx.vars.timer(name) {
                style.rgb = timer_color(secs);
            }
        }
        out.push(Drawn { id: b.id, rect: b.rect, text, source: b.source.clone(), style, opacity: b.opacity });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> StageCtx {
        let mut c = StageCtx { slide: "Amazing grace".into(), next: "How sweet".into(), reference: "John 3:16".into(), title: "Sunday".into(), ..Default::default() };
        c.vars = Ctx::new(vars::from_local(2026, 10, 14, 10, 29, 0), 0);
        c.vars.set_timer("Sermon", 1500);
        c
    }

    #[test]
    fn the_preacher_sees_now_next_clock_and_timer() {
        let drawn = resolve(&layout_for(Role::Preacher), &ctx());
        let by = |n: &str| drawn.iter().find(|d| d.text.as_deref().map_or(false, |t| t == n)).is_some();
        assert!(by("Amazing grace") && by("How sweet"));
        assert!(by("10:29"));
        assert!(by("25:00"));
    }

    #[test]
    fn empty_notes_and_messages_are_left_out_but_a_message_appears_when_there_is_one() {
        let l = layout_for(Role::Preacher);
        let n_quiet = resolve(&l, &ctx()).len();
        let mut c = ctx();
        c.message = "Wrap up in 5".into();
        c.notes = "Mention the picnic".into();
        let loud = resolve(&l, &c);
        assert_eq!(loud.len(), n_quiet + 2);
        assert!(loud.iter().any(|d| d.text.as_deref() == Some("Wrap up in 5")));
    }

    #[test]
    fn the_timer_changes_colour_as_time_runs_out() {
        assert_eq!(timer_color(600), [80, 200, 120]);
        assert_eq!(timer_color(120), [255, 180, 0]);
        assert_eq!(timer_color(0), [255, 180, 0]);
        assert_eq!(timer_color(-1), [235, 60, 60]);
        let mut c = ctx();
        c.vars.set_timer("Sermon", -30);
        let d = resolve(&layout_for(Role::Preacher), &c);
        let t = d.iter().find(|d| matches!(d.source, Source::Timer(_))).unwrap();
        assert_eq!(t.style.rgb, [235, 60, 60]);
        assert_eq!(t.text.as_deref(), Some("-00:30"));
    }

    #[test]
    fn a_timer_that_does_not_exist_shows_dashes_not_a_crash() {
        let mut c = ctx();
        c.vars = Ctx::new(0, 0);
        let d = resolve(&layout_for(Role::Preacher), &c);
        assert!(d.iter().any(|x| x.text.as_deref() == Some("--:--")));
    }

    #[test]
    fn the_band_sees_chords_and_the_tech_desk_sees_the_reference() {
        let mut c = ctx();
        c.chords = "G    C\nAmazing grace".into();
        assert!(resolve(&layout_for(Role::Worship), &c).iter().any(|d| d.text.as_deref() == Some("G    C\nAmazing grace")));
        assert!(resolve(&layout_for(Role::Tech), &ctx()).iter().any(|d| d.text.as_deref() == Some("John 3:16")));
        for r in Role::ALL {
            assert!(layout_for(r).validate().is_empty(), "{:?}: {:?}", r, layout_for(r).validate());
        }
    }

    #[test]
    fn hidden_boxes_and_transforms_are_respected() {
        let mut l = layout_for(Role::Preacher);
        let id = l.boxes[0].id;
        l.get_mut(id).unwrap().style.transform = crate::layout::Transform::Upper;
        let d = resolve(&l, &ctx());
        assert!(d.iter().any(|x| x.text.as_deref() == Some("AMAZING GRACE")));
        l.get_mut(id).unwrap().visible = false;
        assert!(!resolve(&l, &ctx()).iter().any(|x| x.id == id));
    }
}
