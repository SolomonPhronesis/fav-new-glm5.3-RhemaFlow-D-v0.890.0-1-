//! Timers and clocks for the stage (Stage 2, spec section 5).
//!
//! Pure: every function is told the time (`now_ms` = a steady clock in milliseconds, `wall_ms` =
//! milliseconds since local midnight). Tests can run a whole service in microseconds.

/// Warning starts when this much time is left.
pub const WARNING_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerKind {
    /// S5.1: count down a length of time.
    CountDown { total_ms: u64 },
    /// S5.2: count down to a time of day (seconds after midnight). Never wraps to tomorrow.
    UntilClock { target_secs: u32 },
    /// S5.3: count up.
    StopWatch,
    /// S5.4: the time of day.
    Clock { twelve_hour: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Running,
    /// S5.6: last minute (operator panel only).
    Warning,
    Paused,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timer {
    kind: TimerKind,
    title: String,
    started: bool,
    /// milliseconds counted before the current run began
    before_ms: u64,
    running_since: Option<u64>,
}

impl Timer {
    pub fn new(kind: TimerKind, title: &str) -> Timer {
        Timer { kind, title: title.to_string(), started: false, before_ms: 0, running_since: None }
    }
    pub fn kind(&self) -> TimerKind {
        self.kind
    }
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn start(&mut self, now_ms: u64, _wall_ms: u64) {
        self.started = true;
        self.before_ms = 0;
        self.running_since = Some(now_ms);
    }
    pub fn pause(&mut self, now_ms: u64) {
        if let Some(s) = self.running_since.take() {
            self.before_ms += now_ms.saturating_sub(s);
        }
    }
    pub fn resume(&mut self, now_ms: u64) {
        if self.started && self.running_since.is_none() {
            self.running_since = Some(now_ms);
        }
    }
    pub fn reset(&mut self) {
        self.started = false;
        self.before_ms = 0;
        self.running_since = None;
    }

    fn elapsed_ms(&self, now_ms: u64) -> u64 {
        self.before_ms + self.running_since.map(|s| now_ms.saturating_sub(s)).unwrap_or(0)
    }

    /// Milliseconds left, for kinds that count down.
    pub fn remaining_ms(&self, now_ms: u64, wall_ms: u64) -> Option<u64> {
        match self.kind {
            TimerKind::CountDown { total_ms } => Some(if self.started { total_ms.saturating_sub(self.elapsed_ms(now_ms)) } else { total_ms }),
            TimerKind::UntilClock { target_secs } => Some((target_secs as u64 * 1000).saturating_sub(wall_ms % 86_400_000)),
            _ => None,
        }
    }

    pub fn phase(&self, now_ms: u64, wall_ms: u64) -> Phase {
        if !self.started {
            return match self.kind {
                TimerKind::Clock { .. } => Phase::Running,
                _ => Phase::Idle,
            };
        }
        let follows_wall_clock = matches!(self.kind, TimerKind::UntilClock { .. } | TimerKind::Clock { .. });
        if self.running_since.is_none() && !follows_wall_clock {
            return Phase::Paused;
        }
        match self.remaining_ms(now_ms, wall_ms) {
            Some(0) => Phase::Done,
            Some(r) if r <= WARNING_MS => Phase::Warning,
            _ => Phase::Running,
        }
    }

    /// The text for the stage.
    pub fn display(&self, now_ms: u64, wall_ms: u64) -> String {
        match self.kind {
            TimerKind::CountDown { .. } | TimerKind::UntilClock { .. } => format_duration(self.remaining_ms(now_ms, wall_ms).unwrap_or(0), true),
            TimerKind::StopWatch => format_duration(if self.started { self.elapsed_ms(now_ms) } else { 0 }, false),
            TimerKind::Clock { twelve_hour } => format_time_of_day(((wall_ms / 1000) % 86_400) as u32, twelve_hour),
        }
    }
}

/// `mm:ss`, or `h:mm:ss` from one hour. Countdowns round up, stopwatches round down.
pub fn format_duration(ms: u64, round_up: bool) -> String {
    let s = if round_up { (ms + 999) / 1000 } else { ms / 1000 };
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

/// `10:30`, or `10:30 AM`.
pub fn format_time_of_day(secs: u32, twelve_hour: bool) -> String {
    let (h, m) = ((secs / 3600) % 24, (secs % 3600) / 60);
    if !twelve_hour {
        return format!("{h:02}:{m:02}");
    }
    let (h12, ampm) = match h {
        0 => (12, "AM"),
        1..=11 => (h, "AM"),
        12 => (12, "PM"),
        _ => (h - 12, "PM"),
    };
    format!("{h12}:{m:02} {ampm}")
}

/// S5.8: `10:30`, `9:05`, `0930`, `9.05`, `9:05pm`, `12am`. Seconds after midnight, or None.
pub fn parse_time_of_day(text: &str) -> Option<u32> {
    let t = text.trim().to_lowercase();
    let (body, ampm) = if let Some(b) = t.strip_suffix("am") {
        (b.trim(), Some(false))
    } else if let Some(b) = t.strip_suffix("pm") {
        (b.trim(), Some(true))
    } else {
        (t.as_str(), None)
    };
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_digit() || c == ':' || c == '.') {
        return None;
    }
    let (h, m): (u32, u32) = if let Some((a, b)) = body.split_once(|c| c == ':' || c == '.') {
        let digits = |s: &str| !s.is_empty() && s.len() <= 2 && s.chars().all(|c| c.is_ascii_digit());
        if !digits(a) || !digits(b) {
            return None;
        }
        (a.parse().ok()?, b.parse().ok()?)
    } else {
        match body.len() {
            1 | 2 => (body.parse().ok()?, 0),
            3 | 4 => (body[..body.len() - 2].parse().ok()?, body[body.len() - 2..].parse().ok()?),
            _ => return None,
        }
    };
    if m > 59 {
        return None;
    }
    let h24 = match ampm {
        None => {
            if h > 23 {
                return None;
            }
            h
        }
        Some(pm) => {
            if !(1..=12).contains(&h) {
                return None;
            }
            match (h, pm) {
                (12, false) => 0,
                (12, true) => 12,
                (h, true) => h + 12,
                (h, false) => h,
            }
        }
    };
    Some(h24 * 3600 + m * 60)
}
