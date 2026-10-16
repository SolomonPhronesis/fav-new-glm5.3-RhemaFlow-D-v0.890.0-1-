//! Feature 5: how one slide gives way to the next. Pure per-frame maths: the painter asks "at 120 ms, what do I draw?"
//! and gets opacities, offsets, scale and a reveal window back.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Cut,
    Fade,
    DipToBlack,
    PushLeft,
    PushRight,
    PushUp,
    PushDown,
    WipeLeft,
    WipeRight,
    ZoomIn,
}

impl Kind {
    pub const ALL: [Kind; 10] = [Kind::Cut, Kind::Fade, Kind::DipToBlack, Kind::PushLeft, Kind::PushRight, Kind::PushUp, Kind::PushDown, Kind::WipeLeft, Kind::WipeRight, Kind::ZoomIn];
    pub fn label(self) -> &'static str {
        match self {
            Kind::Cut => "Cut",
            Kind::Fade => "Fade",
            Kind::DipToBlack => "Dip to black",
            Kind::PushLeft => "Push left",
            Kind::PushRight => "Push right",
            Kind::PushUp => "Push up",
            Kind::PushDown => "Push down",
            Kind::WipeLeft => "Wipe left",
            Kind::WipeRight => "Wipe right",
            Kind::ZoomIn => "Zoom in",
        }
    }
    pub fn from_name(s: &str) -> Option<Kind> {
        let k = s.trim().to_lowercase().replace([' ', '-', '_'], "");
        Kind::ALL.iter().copied().find(|x| x.label().to_lowercase().replace(' ', "") == k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl Easing {
    pub fn apply(self, p: f32) -> f32 {
        let p = p.clamp(0.0, 1.0);
        match self {
            Easing::Linear => p,
            Easing::EaseIn => p * p,
            Easing::EaseOut => 1.0 - (1.0 - p) * (1.0 - p),
            Easing::EaseInOut => p * p * (3.0 - 2.0 * p),
        }
    }
}

pub const MAX_MS: u32 = 5000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    pub kind: Kind,
    pub ms: u32,
    pub easing: Easing,
}

impl Default for Transition {
    fn default() -> Self {
        Transition { kind: Kind::Fade, ms: 400, easing: Easing::EaseInOut }
    }
}

/// What to draw. Offsets are fractions of the screen (x right, y down). `reveal` is the part of the NEW picture shown
/// (x0, y0, x1, y1 as fractions). `black` is a black layer on top, 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub old_alpha: f32,
    pub new_alpha: f32,
    pub old_off: [f32; 2],
    pub new_off: [f32; 2],
    pub new_scale: f32,
    pub reveal: [f32; 4],
    pub black: f32,
    pub done: bool,
}

impl Transition {
    pub fn new(kind: Kind, ms: u32) -> Transition {
        Transition { kind, ms: ms.min(MAX_MS), easing: Easing::EaseInOut }
    }
    pub fn duration_ms(&self) -> u32 {
        if self.kind == Kind::Cut {
            0
        } else {
            self.ms.min(MAX_MS)
        }
    }
    /// People who are sensitive to motion: anything that moves or zooms becomes a short fade.
    pub fn for_reduced_motion(self, reduce: bool) -> Transition {
        if !reduce {
            return self;
        }
        match self.kind {
            Kind::Cut | Kind::Fade | Kind::DipToBlack => self,
            _ => Transition { kind: Kind::Fade, ms: self.ms.min(200), easing: Easing::Linear },
        }
    }

    pub fn frame(&self, elapsed_ms: u32) -> Frame {
        let dur = self.duration_ms();
        let finished = dur == 0 || elapsed_ms >= dur;
        let raw = if dur == 0 { 1.0 } else { (elapsed_ms as f32 / dur as f32).min(1.0) };
        let p = self.easing.apply(raw);
        let mut f = Frame { old_alpha: 1.0 - p, new_alpha: 1.0, old_off: [0.0, 0.0], new_off: [0.0, 0.0], new_scale: 1.0, reveal: [0.0, 0.0, 1.0, 1.0], black: 0.0, done: finished };
        if finished {
            f.old_alpha = 0.0;
            return f;
        }
        match self.kind {
            Kind::Cut => {}
            Kind::Fade => f.new_alpha = p,
            Kind::DipToBlack => {
                if p < 0.5 {
                    f.old_alpha = 1.0;
                    f.new_alpha = 0.0;
                    f.black = p * 2.0;
                } else {
                    f.old_alpha = 0.0;
                    f.new_alpha = 1.0;
                    f.black = (1.0 - p) * 2.0;
                }
            }
            Kind::PushLeft => {
                f.old_alpha = 1.0;
                f.old_off = [-p, 0.0];
                f.new_off = [1.0 - p, 0.0];
            }
            Kind::PushRight => {
                f.old_alpha = 1.0;
                f.old_off = [p, 0.0];
                f.new_off = [-(1.0 - p), 0.0];
            }
            Kind::PushUp => {
                f.old_alpha = 1.0;
                f.old_off = [0.0, -p];
                f.new_off = [0.0, 1.0 - p];
            }
            Kind::PushDown => {
                f.old_alpha = 1.0;
                f.old_off = [0.0, p];
                f.new_off = [0.0, -(1.0 - p)];
            }
            Kind::WipeRight => {
                f.old_alpha = 1.0;
                f.reveal = [0.0, 0.0, p, 1.0];
            }
            Kind::WipeLeft => {
                f.old_alpha = 1.0;
                f.reveal = [1.0 - p, 0.0, 1.0, 1.0];
            }
            Kind::ZoomIn => {
                f.new_alpha = p;
                f.new_scale = 0.85 + 0.15 * p;
            }
        }
        f
    }
}

/// A transition that is running: start it with the clock, ask for the frame with the clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct Running {
    started_ms: Option<u64>,
    t: Option<Transition>,
}

impl Running {
    pub fn start(&mut self, t: Transition, now_ms: u64) {
        self.t = Some(t);
        self.started_ms = Some(now_ms);
    }
    pub fn active(&self, now_ms: u64) -> bool {
        match (self.t, self.started_ms) {
            (Some(t), Some(s)) => now_ms.saturating_sub(s) < t.duration_ms() as u64,
            _ => false,
        }
    }
    pub fn frame(&self, now_ms: u64) -> Option<Frame> {
        let (t, s) = (self.t?, self.started_ms?);
        let f = t.frame(now_ms.saturating_sub(s).min(u32::MAX as u64) as u32);
        Some(f)
    }
    /// Ask the painter to keep redrawing while this is true.
    pub fn needs_repaint(&self, now_ms: u64) -> bool {
        self.active(now_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_starts_on_the_old_slide_and_ends_on_the_new() {
        for k in Kind::ALL {
            let t = Transition::new(k, 500);
            let end = t.frame(500);
            assert!(end.done, "{k:?}");
            assert_eq!((end.new_alpha, end.old_alpha, end.black), (1.0, 0.0, 0.0), "{k:?}");
            assert_eq!(end.new_off, [0.0, 0.0]);
            assert_eq!(end.new_scale, 1.0);
            assert_eq!(end.reveal, [0.0, 0.0, 1.0, 1.0]);
            if k != Kind::Cut {
                let start = t.frame(0);
                assert!(!start.done, "{k:?}");
                // at the very start the new picture is not visible yet (hidden, off screen, covered, or revealed 0 wide)
                let hidden = start.new_alpha == 0.0 || start.new_off != [0.0, 0.0] || start.reveal[2] - start.reveal[0] == 0.0 || start.black >= 1.0 - 1e-6 || start.new_alpha < 1.0;
                assert!(hidden || k == Kind::DipToBlack, "{k:?} {start:?}");
            }
        }
    }

    #[test]
    fn a_cut_is_instant() {
        let t = Transition::new(Kind::Cut, 1000);
        assert_eq!(t.duration_ms(), 0);
        assert!(t.frame(0).done);
    }

    #[test]
    fn fade_is_halfway_at_the_middle() {
        let t = Transition { kind: Kind::Fade, ms: 400, easing: Easing::Linear };
        let f = t.frame(200);
        assert!((f.new_alpha - 0.5).abs() < 1e-6 && (f.old_alpha - 0.5).abs() < 1e-6);
    }

    #[test]
    fn dip_goes_fully_black_in_the_middle_and_the_picture_changes_there() {
        let t = Transition { kind: Kind::DipToBlack, ms: 400, easing: Easing::Linear };
        let before = t.frame(199);
        let mid = t.frame(200);
        let after = t.frame(201);
        assert!(before.old_alpha == 1.0 && before.new_alpha == 0.0);
        assert!((mid.black - 1.0).abs() < 0.01);
        assert!(after.old_alpha == 0.0 && after.new_alpha == 1.0);
    }

    #[test]
    fn push_moves_both_pictures_together() {
        let t = Transition { kind: Kind::PushLeft, ms: 100, easing: Easing::Linear };
        let f = t.frame(25);
        assert!((f.old_off[0] + 0.25).abs() < 1e-6);
        assert!((f.new_off[0] - 0.75).abs() < 1e-6);
        assert!((f.new_off[0] - f.old_off[0] - 1.0).abs() < 1e-6); // always exactly one screen apart
        let u = Transition { kind: Kind::PushDown, ms: 100, easing: Easing::Linear }.frame(50);
        assert!((u.new_off[1] - u.old_off[1] + 1.0).abs() < 1e-6);
    }

    #[test]
    fn wipe_reveals_from_the_right_side() {
        let r = Transition { kind: Kind::WipeRight, ms: 100, easing: Easing::Linear }.frame(40).reveal;
        assert_eq!(r, [0.0, 0.0, 0.4, 1.0]);
        let l = Transition { kind: Kind::WipeLeft, ms: 100, easing: Easing::Linear }.frame(40).reveal;
        assert!((l[0] - 0.6).abs() < 1e-6 && l[2] == 1.0);
    }

    #[test]
    fn progress_only_ever_goes_forward() {
        for k in [Kind::Fade, Kind::PushLeft, Kind::WipeRight, Kind::ZoomIn] {
            let t = Transition::new(k, 600);
            let mut last = -1.0f32;
            for ms in (0..=600).step_by(20) {
                let f = t.frame(ms);
                let progress = match k {
                    Kind::Fade | Kind::ZoomIn => f.new_alpha,
                    Kind::PushLeft => 1.0 - f.new_off[0],
                    _ => f.reveal[2],
                };
                assert!(progress >= last - 1e-6, "{k:?} at {ms}");
                last = progress;
            }
        }
    }

    #[test]
    fn easing_stays_in_range_and_meets_the_ends() {
        for e in [Easing::Linear, Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut] {
            assert_eq!(e.apply(0.0), 0.0);
            assert_eq!(e.apply(1.0), 1.0);
            assert_eq!(e.apply(-3.0), 0.0);
            assert_eq!(e.apply(9.0), 1.0);
        }
    }

    #[test]
    fn long_durations_are_capped_and_reduced_motion_calms_movement() {
        assert_eq!(Transition::new(Kind::Fade, 60_000).ms, MAX_MS);
        let calm = Transition::new(Kind::PushLeft, 900).for_reduced_motion(true);
        assert_eq!(calm.kind, Kind::Fade);
        assert!(calm.ms <= 200);
        assert_eq!(Transition::new(Kind::Fade, 900).for_reduced_motion(true).ms, 900);
        assert_eq!(Transition::new(Kind::PushLeft, 900).for_reduced_motion(false).kind, Kind::PushLeft);
    }

    #[test]
    fn names_are_understood() {
        assert_eq!(Kind::from_name("push left"), Some(Kind::PushLeft));
        assert_eq!(Kind::from_name("Dip-to-Black"), Some(Kind::DipToBlack));
        assert_eq!(Kind::from_name("spin"), None);
    }

    #[test]
    fn the_running_clock_says_when_to_keep_painting() {
        let mut r = Running::default();
        assert!(!r.active(0));
        r.start(Transition::new(Kind::Fade, 300), 1000);
        assert!(r.needs_repaint(1100));
        assert!(!r.needs_repaint(1300));
        assert!(r.frame(1300).unwrap().done);
        assert!(r.frame(1000).unwrap().new_alpha < 0.01);
    }
}
