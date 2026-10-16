//! Feature 9: slides that move on by themselves. Each slide can have its own time (or "wait for the operator").
//! Once, loop, or ping-pong. Pause and resume. Pure: the caller passes the clock.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Once,
    Loop,
    PingPong,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Show this slide now.
    Go(usize),
    /// The last slide of a one-time run has had its time.
    Finished,
}

/// The shortest time a slide may stay (stops a zero from spinning through the whole list in one tick).
pub const MIN_MS: u64 = 100;

#[derive(Clone, Debug)]
pub struct AutoPlay {
    /// One entry per slide. `None` = stay until the operator moves on (the run pauses there).
    pub durations_ms: Vec<Option<u64>>,
    pub mode: Mode,
    idx: usize,
    due_at: Option<u64>,
    paused_left: Option<u64>,
    dir: i32,
    running: bool,
    engaged: bool,
}

/// "8, 8, manual, 12.5" -> seconds per slide. Missing entries repeat the last number; words like manual/stay/- mean wait.
pub fn parse_durations(text: &str, slides: usize) -> Vec<Option<u64>> {
    let mut out: Vec<Option<u64>> = Vec::new();
    for tok in text.split(|c| c == ',' || c == ';' || c == '\n') {
        let t = tok.trim().to_lowercase();
        if t.is_empty() {
            continue;
        }
        let v = if ["manual", "stay", "hold", "-", "none", "m"].contains(&t.as_str()) { None } else { t.trim_end_matches('s').trim().parse::<f64>().ok().filter(|s| s.is_finite() && *s > 0.0).map(|s| ((s * 1000.0) as u64).max(MIN_MS)) };
        out.push(v);
    }
    let fill = out.last().copied().unwrap_or(None);
    out.resize(slides, fill);
    out.truncate(slides);
    out
}

impl AutoPlay {
    pub fn new(durations_ms: Vec<Option<u64>>, mode: Mode) -> AutoPlay {
        AutoPlay { durations_ms, mode, idx: 0, due_at: None, paused_left: None, dir: 1, running: false, engaged: false }
    }
    pub fn uniform(slides: usize, secs: f64, mode: Mode) -> AutoPlay {
        AutoPlay::new(vec![Some(((secs * 1000.0) as u64).max(MIN_MS)); slides], mode)
    }
    pub fn current(&self) -> usize {
        self.idx
    }
    pub fn is_running(&self) -> bool {
        self.running
    }
    fn dur(&self, i: usize) -> Option<u64> {
        self.durations_ms.get(i).copied().flatten().map(|d| d.max(MIN_MS))
    }

    /// Begin at slide `idx`. Returns the slide to show.
    pub fn start(&mut self, now_ms: u64, idx: usize) -> Option<Event> {
        if self.durations_ms.is_empty() {
            return None;
        }
        self.idx = idx.min(self.durations_ms.len() - 1);
        self.dir = 1;
        self.engaged = true;
        self.paused_left = None;
        self.arm(now_ms);
        Some(Event::Go(self.idx))
    }

    fn arm(&mut self, now_ms: u64) {
        match self.dur(self.idx) {
            Some(d) => {
                self.due_at = Some(now_ms + d);
                self.running = true;
            }
            None => {
                self.due_at = None;
                self.running = false; // a "wait for the operator" slide
            }
        }
    }

    /// The operator moved to a slide by hand: keep running from there (or wait, if that slide waits).
    pub fn jumped_to(&mut self, now_ms: u64, idx: usize) {
        if self.durations_ms.is_empty() || !self.engaged {
            return;
        }
        self.idx = idx.min(self.durations_ms.len() - 1);
        self.paused_left = None;
        self.arm(now_ms);
    }

    pub fn stop(&mut self) {
        self.engaged = false;
        self.running = false;
        self.due_at = None;
        self.paused_left = None;
    }
    pub fn pause(&mut self, now_ms: u64) {
        if let (true, Some(due)) = (self.running, self.due_at) {
            self.paused_left = Some(due.saturating_sub(now_ms));
            self.running = false;
            self.due_at = None;
        }
    }
    pub fn resume(&mut self, now_ms: u64) {
        if let Some(left) = self.paused_left.take() {
            self.due_at = Some(now_ms + left);
            self.running = true;
        }
    }
    pub fn is_paused(&self) -> bool {
        self.paused_left.is_some()
    }
    pub fn time_left(&self, now_ms: u64) -> Option<u64> {
        match (self.running, self.due_at, self.paused_left) {
            (true, Some(d), _) => Some(d.saturating_sub(now_ms)),
            (_, _, Some(p)) => Some(p),
            _ => None,
        }
    }

    fn step(&mut self) -> Option<usize> {
        let n = self.durations_ms.len();
        match self.mode {
            Mode::Once => (self.idx + 1 < n).then(|| self.idx + 1),
            Mode::Loop => Some((self.idx + 1) % n),
            Mode::PingPong => {
                if n == 1 {
                    return Some(0);
                }
                if self.dir > 0 && self.idx + 1 >= n {
                    self.dir = -1;
                } else if self.dir < 0 && self.idx == 0 {
                    self.dir = 1;
                }
                Some((self.idx as i32 + self.dir) as usize)
            }
        }
    }

    /// Call often. Returns what to do now, if anything. After a long freeze it catches up and reports only where it landed.
    pub fn tick(&mut self, now_ms: u64) -> Option<Event> {
        if !self.running || self.durations_ms.is_empty() {
            return None;
        }
        let mut result = None;
        let mut guard = 0;
        while let (true, Some(due)) = (self.running, self.due_at) {
            if now_ms < due || guard > self.durations_ms.len() * 2 + 2 {
                break;
            }
            guard += 1;
            match self.step() {
                None => {
                    self.running = false;
                    self.due_at = None;
                    return Some(Event::Finished);
                }
                Some(next) => {
                    self.idx = next;
                    result = Some(Event::Go(next));
                    match self.dur(next) {
                        Some(d) => self.due_at = Some(due + d),
                        None => {
                            self.running = false;
                            self.due_at = None;
                        }
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn once_runs_through_then_finishes() {
        let mut a = AutoPlay::uniform(3, 1.0, Mode::Once);
        assert_eq!(a.start(0, 0), Some(Event::Go(0)));
        assert_eq!(a.tick(999), None);
        assert_eq!(a.tick(1000), Some(Event::Go(1)));
        assert_eq!(a.tick(2000), Some(Event::Go(2)));
        assert_eq!(a.tick(3000), Some(Event::Finished));
        assert!(!a.is_running());
        assert_eq!(a.tick(9000), None);
        assert_eq!(a.current(), 2);
    }

    #[test]
    fn loop_wraps_and_ping_pong_bounces() {
        let mut l = AutoPlay::uniform(3, 1.0, Mode::Loop);
        l.start(0, 0);
        let seq: Vec<usize> = (1..=5).filter_map(|i| if let Some(Event::Go(n)) = l.tick(i * 1000) { Some(n) } else { None }).collect();
        assert_eq!(seq, vec![1, 2, 0, 1, 2]);
        let mut p = AutoPlay::uniform(3, 1.0, Mode::PingPong);
        p.start(0, 0);
        let seq: Vec<usize> = (1..=6).filter_map(|i| if let Some(Event::Go(n)) = p.tick(i * 1000) { Some(n) } else { None }).collect();
        assert_eq!(seq, vec![1, 2, 1, 0, 1, 2]);
        let mut one = AutoPlay::uniform(1, 1.0, Mode::PingPong);
        one.start(0, 0);
        assert_eq!(one.tick(1000), Some(Event::Go(0)));
    }

    #[test]
    fn each_slide_has_its_own_time_and_a_manual_slide_waits_for_the_operator() {
        let mut a = AutoPlay::new(vec![Some(2000), None, Some(1000)], Mode::Once);
        a.start(0, 0);
        assert_eq!(a.tick(1999), None);
        assert_eq!(a.tick(2000), Some(Event::Go(1)));
        assert!(!a.is_running(), "slide 2 waits");
        assert_eq!(a.tick(60_000), None);
        // the operator goes on by hand
        a.jumped_to(61_000, 2);
        assert!(a.is_running());
        assert_eq!(a.tick(62_000), Some(Event::Finished));
    }

    #[test]
    fn pausing_keeps_the_time_that_was_left() {
        let mut a = AutoPlay::uniform(3, 4.0, Mode::Once);
        a.start(0, 0);
        a.pause(1000);
        assert!(a.is_paused());
        assert_eq!(a.time_left(50_000), Some(3000));
        assert_eq!(a.tick(50_000), None);
        a.resume(50_000);
        assert_eq!(a.tick(52_999), None);
        assert_eq!(a.tick(53_000), Some(Event::Go(1)));
    }

    #[test]
    fn a_long_freeze_catches_up_and_reports_where_it_landed() {
        let mut a = AutoPlay::uniform(4, 1.0, Mode::Loop);
        a.start(0, 0);
        assert_eq!(a.tick(2500), Some(Event::Go(2)));
        assert_eq!(a.time_left(2500), Some(500));
        let mut o = AutoPlay::uniform(4, 1.0, Mode::Once);
        o.start(0, 0);
        assert_eq!(o.tick(1_000_000), Some(Event::Finished));
    }

    #[test]
    fn zero_or_tiny_times_cannot_spin() {
        let mut a = AutoPlay::new(vec![Some(0), Some(0)], Mode::Loop);
        a.start(0, 0);
        assert!(a.tick(0).is_none());
        assert!(a.tick(150).is_some());
        assert!(AutoPlay::new(vec![], Mode::Loop).start(0, 0).is_none());
    }

    #[test]
    fn typed_times_are_understood() {
        assert_eq!(parse_durations("8, 8, manual, 12.5s", 5), vec![Some(8000), Some(8000), None, Some(12_500), Some(12_500)]);
        assert_eq!(parse_durations("", 2), vec![None, None]);
        assert_eq!(parse_durations("5", 3), vec![Some(5000); 3]);
        assert_eq!(parse_durations("1,2,3,4", 2), vec![Some(1000), Some(2000)]);
        assert_eq!(parse_durations("abc, -3", 2), vec![None, None]);
    }

    #[test]
    fn stopping_ends_everything() {
        let mut a = AutoPlay::uniform(2, 1.0, Mode::Loop);
        a.start(0, 0);
        a.stop();
        a.jumped_to(0, 1); // not started: a hand jump must not start it
        assert!(!a.is_running());
        assert_eq!(a.tick(5000), None);
        assert_eq!(a.time_left(0), None);
    }
}
