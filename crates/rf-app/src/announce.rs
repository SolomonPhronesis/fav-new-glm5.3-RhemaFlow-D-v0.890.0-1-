//! The announcement loop (v0.835.0): notices that cycle by themselves on the congregation screen before the
//! service starts, in the gaps, or after it. Pure timing and parsing; the app puts each item on the stage.

#[derive(Clone, Debug, PartialEq)]
pub struct AnnounceItem {
    pub title: String,
    pub body: String,
    pub secs: u32,
}

pub const MIN_SECS: u32 = 3;
pub const MAX_SECS: u32 = 120;

impl AnnounceItem {
    /// The words shown on the screen: title, a gap, then the details.
    pub fn screen_text(&self) -> String {
        match (self.title.trim().is_empty(), self.body.trim().is_empty()) {
            (false, false) => format!("{}\n\n{}", self.title.trim(), self.body.trim()),
            (false, true) => self.title.trim().to_string(),
            _ => self.body.trim().to_string(),
        }
    }
}

/// "[12s] Youth night" / "(12s) Youth night" / "12s: Youth night" -> (Some(12), "Youth night").
fn take_seconds(line: &str) -> (Option<u32>, String) {
    let t = line.trim();
    let try_len = |inner: &str| -> Option<u32> {
        let i = inner.trim().trim_end_matches(|c| c == 's' || c == 'S').trim();
        if inner.trim().to_lowercase().ends_with('s') {
            i.parse::<u32>().ok()
        } else {
            None
        }
    };
    for (open, close) in [('[', ']'), ('(', ')')] {
        if t.starts_with(open) {
            if let Some(end) = t.find(close) {
                if let Some(n) = try_len(&t[1..end]) {
                    return (Some(n), t[end + 1..].trim().to_string());
                }
            }
        }
    }
    if let Some((head, rest)) = t.split_once(':') {
        if head.len() <= 5 {
            if let Some(n) = try_len(head) {
                return (Some(n), rest.trim().to_string());
            }
        }
    }
    (None, t.to_string())
}

/// Notices typed or pasted: blocks separated by a blank line. The first line of a block is its heading
/// (a block of one line is all heading), the rest is the detail. `[15s]` in front of the heading sets how long it stays.
pub fn parse(text: &str, default_secs: u32) -> Vec<AnnounceItem> {
    let default_secs = default_secs.clamp(MIN_SECS, MAX_SECS);
    let mut out = Vec::new();
    for block in text.replace("\r\n", "\n").split("\n\n") {
        let lines: Vec<&str> = block.lines().map(|l| l.trim_end()).filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            continue;
        }
        let (secs, title) = take_seconds(lines[0]);
        let body = lines[1..].iter().map(|l| l.trim()).collect::<Vec<_>>().join("\n");
        out.push(AnnounceItem { title, body, secs: secs.unwrap_or(default_secs).clamp(MIN_SECS, MAX_SECS) });
    }
    out
}

/// The loop's clock. Times are milliseconds on the app's own clock.
#[derive(Clone, Debug, Default)]
pub struct Loop {
    pub items: Vec<AnnounceItem>,
    pub running: bool,
    /// which item is up now
    pub index: usize,
    next_at: u64,
    /// the stage counter value right after the loop last put something up; a different value means someone else took the screen
    pub last_seq: u64,
}

impl Loop {
    /// Start from the first notice. Returns it (the caller puts it on screen) or None if there are none.
    pub fn start(&mut self, now: u64) -> Option<usize> {
        if self.items.is_empty() {
            self.running = false;
            return None;
        }
        self.running = true;
        self.index = 0;
        self.next_at = now + self.items[0].secs as u64 * 1000;
        Some(0)
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Call often. `Some(i)` = time to show item i now.
    pub fn tick(&mut self, now: u64) -> Option<usize> {
        if !self.running || self.items.is_empty() {
            return None;
        }
        if now < self.next_at {
            return None;
        }
        self.index = (self.index + 1) % self.items.len();
        self.next_at = now + self.items[self.index].secs as u64 * 1000;
        Some(self.index)
    }

    /// Jump by hand (next / previous notice), restarting its time.
    pub fn step(&mut self, delta: i32, now: u64) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let n = self.items.len() as i32;
        self.index = ((self.index as i32 + delta).rem_euclid(n)) as usize;
        self.next_at = now + self.items[self.index].secs as u64 * 1000;
        Some(self.index)
    }

    /// Seconds until the next notice (for the little countdown in the window).
    pub fn seconds_left(&self, now: u64) -> u64 {
        if !self.running {
            return 0;
        }
        self.next_at.saturating_sub(now).div_ceil(1000)
    }

    /// The loop must not fight the operator: if anything else has been put on the screen since the loop last did, it stops.
    pub fn yield_if_taken(&mut self, stage_seq_now: u64) -> bool {
        if self.running && stage_seq_now != self.last_seq {
            self.running = false;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_are_split_on_blank_lines_with_optional_times() {
        let t = "Youth night\nFriday 7pm in the hall\n\n[20s] Coffee morning\nTuesday 10am\nAll welcome\n\n\nCar park closed";
        let v = parse(t, 10);
        assert_eq!(v.len(), 3);
        assert_eq!((v[0].title.as_str(), v[0].body.as_str(), v[0].secs), ("Youth night", "Friday 7pm in the hall", 10));
        assert_eq!((v[1].title.as_str(), v[1].secs), ("Coffee morning", 20));
        assert_eq!(v[1].body, "Tuesday 10am\nAll welcome");
        assert_eq!((v[2].title.as_str(), v[2].body.as_str()), ("Car park closed", ""));
        assert_eq!(v[0].screen_text(), "Youth night\n\nFriday 7pm in the hall");
        assert!(parse("   \n\n  ", 10).is_empty());
    }

    #[test]
    fn times_are_kept_in_a_sensible_range_and_other_brackets_are_left_alone() {
        assert_eq!(parse("[1s] Quick", 10)[0].secs, MIN_SECS);
        assert_eq!(parse("(999s) Long", 10)[0].secs, MAX_SECS);
        assert_eq!(parse("15s: Prayer meeting", 10)[0].title, "Prayer meeting");
        let v = parse("[Important] Fire drill", 10);
        assert_eq!(v[0].title, "[Important] Fire drill");
        assert_eq!(v[0].secs, 10);
        assert_eq!(parse("Service at 10:30\nAll welcome", 8)[0].title, "Service at 10:30");
    }

    #[test]
    fn the_loop_cycles_on_time_and_wraps() {
        let mut l = Loop { items: parse("A\n\n[5s] B\n\nC", 3), ..Default::default() };
        assert_eq!(l.start(1_000), Some(0));
        assert_eq!(l.tick(3_999), None);
        assert_eq!(l.tick(4_000), Some(1));
        assert_eq!(l.seconds_left(4_000), 5);
        assert_eq!(l.tick(8_999), None);
        assert_eq!(l.tick(9_000), Some(2));
        assert_eq!(l.tick(12_000), Some(0), "wraps round to the first");
        l.stop();
        assert_eq!(l.tick(99_000), None);
    }

    #[test]
    fn stepping_by_hand_restarts_the_clock_and_an_empty_loop_never_runs() {
        let mut l = Loop { items: parse("A\n\nB\n\nC", 10), ..Default::default() };
        l.start(0);
        assert_eq!(l.step(-1, 2_000), Some(2));
        assert_eq!(l.tick(11_999), None);
        assert_eq!(l.tick(12_000), Some(0));
        let mut e = Loop::default();
        assert_eq!(e.start(0), None);
        assert!(!e.running);
        assert_eq!(e.step(1, 0), None);
    }

    #[test]
    fn the_loop_gives_way_when_the_operator_puts_something_else_up() {
        let mut l = Loop { items: parse("A\n\nB", 5), ..Default::default() };
        l.start(0);
        l.last_seq = 7;
        assert!(!l.yield_if_taken(7));
        assert!(l.yield_if_taken(8));
        assert!(!l.running);
        assert!(!l.yield_if_taken(9));
    }
}
