//! The run-sheet (v0.835.0): the service as a list of timed steps that move forward by themselves.
//!
//! Each step has a length. With Auto on, when a step's time is up the next one is put on the screen. The
//! operator can always step by hand, pause, or go back: a hand move restarts that step's time. Pure timing
//! (milliseconds on the app's clock) so it is tested without a window; the app does what a step says.

use serde::{Deserialize, Serialize};

/// What a step puts on the congregation screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum StepAction {
    /// nothing changes on the screen (a reminder for the operator: "offering", "prayer")
    Note,
    /// a scripture, by its label ("John 3:16-18")
    Scripture(String),
    /// a plain slide of words
    Slide(String),
    /// an item of the service order (by id)
    Order(u64),
    /// a picture / video / sound on the media shelf (by id)
    Media(u64),
    /// start the announcement loop
    Announcements,
    /// the giving card
    Giving,
    /// clear the screen
    Clear,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunStep {
    pub title: String,
    /// how long this step is planned to last, in seconds (0 = no time: only moves when you do)
    pub secs: u32,
    pub action: StepAction,
    #[serde(default)]
    pub note: String,
}

impl RunStep {
    pub fn new(title: &str, secs: u32, action: StepAction) -> RunStep {
        RunStep { title: title.to_string(), secs, action, note: String::new() }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunSheet {
    pub steps: Vec<RunStep>,
    pub current: Option<usize>,
    pub auto: bool,
    #[serde(skip)]
    pub(crate) started_at: u64,
    #[serde(skip)]
    pub(crate) paused_at: Option<u64>,
}

impl RunSheet {
    pub fn total_secs(&self) -> u32 {
        self.steps.iter().map(|s| s.secs).sum()
    }

    /// Planned start of step `i`, in seconds from the start of the service.
    pub fn planned_start(&self, i: usize) -> u32 {
        self.steps.iter().take(i).map(|s| s.secs).sum()
    }

    pub fn is_running(&self) -> bool {
        self.current.is_some()
    }

    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    /// Begin at step `i` (default the first). Returns the step to show.
    pub fn start(&mut self, now: u64) -> Option<usize> {
        if self.steps.is_empty() {
            self.current = None;
            return None;
        }
        self.go(0, now)
    }

    pub fn go(&mut self, i: usize, now: u64) -> Option<usize> {
        if i >= self.steps.len() {
            return None;
        }
        self.current = Some(i);
        self.started_at = now;
        self.paused_at = None;
        Some(i)
    }

    pub fn next(&mut self, now: u64) -> Option<usize> {
        match self.current {
            None => self.start(now),
            Some(i) if i + 1 < self.steps.len() => self.go(i + 1, now),
            Some(_) => None,
        }
    }

    pub fn prev(&mut self, now: u64) -> Option<usize> {
        match self.current {
            Some(i) if i > 0 => self.go(i - 1, now),
            _ => None,
        }
    }

    pub fn stop(&mut self) {
        self.current = None;
        self.paused_at = None;
    }

    pub fn pause(&mut self, now: u64) {
        if self.current.is_some() && self.paused_at.is_none() {
            self.paused_at = Some(now);
        }
    }

    pub fn resume(&mut self, now: u64) {
        if let Some(p) = self.paused_at.take() {
            self.started_at += now.saturating_sub(p);
        }
    }

    /// Milliseconds left in the current step (negative = over time). None when no step is running or it has no time.
    pub fn remaining_ms(&self, now: u64) -> Option<i64> {
        let i = self.current?;
        let secs = self.steps.get(i)?.secs;
        if secs == 0 {
            return None;
        }
        let clock = self.paused_at.unwrap_or(now);
        Some(secs as i64 * 1000 - clock.saturating_sub(self.started_at) as i64)
    }

    /// Call often. With Auto on and a step's time up, moves to the next and returns it.
    /// At the last step it stays (running over) rather than stopping the service by itself.
    pub fn tick(&mut self, now: u64) -> Option<usize> {
        if !self.auto || self.paused_at.is_some() {
            return None;
        }
        let i = self.current?;
        if self.remaining_ms(now)? <= 0 && i + 1 < self.steps.len() {
            return self.go(i + 1, now);
        }
        None
    }

    /// How far the service is ahead (negative) or behind (positive) the plan, in seconds, judged by when the
    /// current step began against when it was planned to begin. `service_start` is the clock time of step 0.
    pub fn behind_secs(&self, now: u64, service_start: u64) -> Option<i64> {
        let i = self.current?;
        let planned = service_start + self.planned_start(i) as u64 * 1000;
        Some((self.started_at as i64 - planned as i64) / 1000)
    }

    pub fn move_step(&mut self, i: usize, delta: i32) {
        let j = i as i32 + delta;
        if i < self.steps.len() && j >= 0 && (j as usize) < self.steps.len() {
            self.steps.swap(i, j as usize);
            if self.current == Some(i) {
                self.current = Some(j as usize);
            } else if self.current == Some(j as usize) {
                self.current = Some(i);
            }
        }
    }

    pub fn remove_step(&mut self, i: usize) {
        if i < self.steps.len() {
            self.steps.remove(i);
            self.current = match self.current {
                Some(c) if c == i => None,
                Some(c) if c > i => Some(c - 1),
                other => other,
            };
        }
    }
}

/// "12:05" for 725 s; "1:02:05" past an hour; "-0:30" for negatives.
pub fn clock_text(secs: i64) -> String {
    let neg = secs < 0;
    let s = secs.unsigned_abs();
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    let body = if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m}:{sec:02}") };
    if neg { format!("-{body}") } else { body }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> RunSheet {
        RunSheet {
            steps: vec![
                RunStep::new("Welcome", 60, StepAction::Slide("Welcome".into())),
                RunStep::new("Reading", 120, StepAction::Scripture("John 3:16".into())),
                RunStep::new("Prayer", 0, StepAction::Note),
                RunStep::new("Close", 30, StepAction::Clear),
            ],
            auto: true,
            ..Default::default()
        }
    }

    #[test]
    fn auto_moves_on_when_a_steps_time_is_up_and_not_before() {
        let mut r = sheet();
        assert_eq!(r.start(1_000), Some(0));
        assert_eq!(r.tick(60_999), None);
        assert_eq!(r.tick(61_000), Some(1));
        assert_eq!(r.remaining_ms(61_000), Some(120_000));
        assert_eq!(r.tick(181_000), Some(2));
        // a step with no time waits for the operator
        assert_eq!(r.remaining_ms(999_999), None);
        assert_eq!(r.tick(999_999), None);
        assert_eq!(r.next(1_000_000), Some(3));
        // the last step runs over rather than ending the service by itself
        assert_eq!(r.tick(2_000_000), None);
        assert!(r.remaining_ms(2_000_000).unwrap() < 0);
        assert_eq!(r.current, Some(3));
    }

    #[test]
    fn auto_off_never_moves_by_itself() {
        let mut r = sheet();
        r.auto = false;
        r.start(0);
        assert_eq!(r.tick(10_000_000), None);
        assert_eq!(r.current, Some(0));
    }

    #[test]
    fn pausing_stops_the_clock_and_resuming_gives_back_the_time() {
        let mut r = sheet();
        r.start(0);
        r.pause(30_000);
        assert!(r.is_paused());
        assert_eq!(r.tick(500_000), None, "paused: no auto move");
        assert_eq!(r.remaining_ms(500_000), Some(30_000));
        r.resume(500_000);
        assert_eq!(r.remaining_ms(500_000), Some(30_000));
        assert_eq!(r.tick(529_999), None);
        assert_eq!(r.tick(530_000), Some(1));
    }

    #[test]
    fn stepping_by_hand_restarts_the_time_and_stays_inside_the_list() {
        let mut r = sheet();
        assert_eq!(r.prev(0), None);
        assert_eq!(r.next(0), Some(0), "next before starting begins");
        assert_eq!(r.next(10_000), Some(1));
        assert_eq!(r.remaining_ms(10_000), Some(120_000));
        assert_eq!(r.prev(20_000), Some(0));
        assert_eq!(r.go(99, 0), None);
        r.stop();
        assert!(!r.is_running());
        assert_eq!(RunSheet::default().start(0), None);
    }

    #[test]
    fn planned_times_and_the_behind_or_ahead_figure() {
        let mut r = sheet();
        assert_eq!(r.total_secs(), 210);
        assert_eq!(r.planned_start(2), 180);
        r.start(1_000_000);
        r.go(1, 1_075_000); // reached step 2 at +75 s but it was planned at +60 s
        assert_eq!(r.behind_secs(1_075_000, 1_000_000), Some(15));
        r.go(1, 1_050_000);
        assert_eq!(r.behind_secs(1_050_000, 1_000_000), Some(-10), "ahead of plan");
    }

    #[test]
    fn reordering_and_deleting_keep_the_running_step_straight() {
        let mut r = sheet();
        r.start(0);
        r.go(1, 0);
        r.move_step(1, -1);
        assert_eq!(r.current, Some(0));
        assert_eq!(r.steps[0].title, "Reading");
        r.remove_step(0);
        assert_eq!(r.current, None, "the running step was removed");
        r.go(1, 0);
        r.remove_step(0);
        assert_eq!(r.current, Some(0));
    }

    #[test]
    fn clocks_read_naturally_and_the_sheet_saves_and_loads() {
        assert_eq!(clock_text(725), "12:05");
        assert_eq!(clock_text(3725), "1:02:05");
        assert_eq!(clock_text(-30), "-0:30");
        let r = sheet();
        let j = serde_json::to_string(&r).unwrap();
        let back: RunSheet = serde_json::from_str(&j).unwrap();
        assert_eq!(back.steps, r.steps);
        assert!(j.contains("scripture"));
    }
}
