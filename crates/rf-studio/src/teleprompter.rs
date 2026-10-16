//! Feature 14: sermon notes that scroll at the speaker's pace. Words-per-minute sets the speed, the operator can nudge it,
//! jump to a paragraph, and mirror the text. Pure maths: positions in pixels, time passed in.

use crate::fit;

pub const MIN_WPM: f32 = 40.0;
pub const MAX_WPM: f32 = 300.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    pub paragraphs: Vec<String>,
}

pub fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

impl Script {
    /// Paragraphs are separated by blank lines.
    pub fn from_text(text: &str) -> Script {
        let mut paragraphs = Vec::new();
        let mut cur: Vec<&str> = Vec::new();
        let norm = text.replace("\r\n", "\n");
        for l in norm.lines() {
            if l.trim().is_empty() {
                if !cur.is_empty() {
                    paragraphs.push(cur.join(" "));
                    cur.clear();
                }
            } else {
                cur.push(l.trim());
            }
        }
        if !cur.is_empty() {
            paragraphs.push(cur.join(" "));
        }
        Script { paragraphs }
    }
    pub fn words(&self) -> usize {
        self.paragraphs.iter().map(|p| word_count(p)).sum()
    }
    /// How long it takes to read at `wpm`.
    pub fn duration_ms(&self, wpm: f32) -> u64 {
        (self.words() as f32 / wpm.clamp(MIN_WPM, MAX_WPM) * 60_000.0) as u64
    }
    /// The script as lines of at most `chars` letters, with a blank line between paragraphs; and the first line of each paragraph.
    pub fn layout(&self, chars: usize) -> (Vec<String>, Vec<usize>) {
        let mut lines = Vec::new();
        let mut starts = Vec::new();
        for (i, p) in self.paragraphs.iter().enumerate() {
            if i > 0 {
                lines.push(String::new());
            }
            starts.push(lines.len());
            lines.extend(fit::wrap(p, chars));
        }
        (lines, starts)
    }
}

/// Pixels per second that match a reading pace: `wpm` words a minute, `words_per_line` words on a line, `line_px` tall lines.
pub fn speed_px_s(wpm: f32, words_per_line: f32, line_px: f32) -> f32 {
    wpm.clamp(MIN_WPM, MAX_WPM) / 60.0 / words_per_line.max(1.0) * line_px
}

#[derive(Clone, Debug)]
pub struct Scroller {
    pub wpm: f32,
    pub mirror: bool,
    pub running: bool,
    pos_px: f32,
    last_ms: Option<u64>,
}

impl Scroller {
    pub fn new(wpm: f32) -> Scroller {
        Scroller { wpm: wpm.clamp(MIN_WPM, MAX_WPM), mirror: false, running: false, pos_px: 0.0, last_ms: None }
    }
    pub fn pos(&self) -> f32 {
        self.pos_px
    }
    pub fn start(&mut self, now_ms: u64) {
        self.running = true;
        self.last_ms = Some(now_ms);
    }
    pub fn stop(&mut self) {
        self.running = false;
        self.last_ms = None;
    }
    pub fn toggle(&mut self, now_ms: u64) {
        if self.running {
            self.stop()
        } else {
            self.start(now_ms)
        }
    }
    /// Faster or slower by `delta` words a minute (never outside 40 to 300).
    pub fn nudge(&mut self, delta: f32) {
        self.wpm = (self.wpm + delta).clamp(MIN_WPM, MAX_WPM);
    }
    pub fn jump_to(&mut self, px: f32) {
        self.pos_px = px.max(0.0);
    }
    /// Move on with the clock. Stops by itself at the end of the text (`total_px` is the height of all lines).
    pub fn tick(&mut self, now_ms: u64, words_per_line: f32, line_px: f32, total_px: f32) {
        if !self.running {
            return;
        }
        let dt = now_ms.saturating_sub(self.last_ms.unwrap_or(now_ms)).min(2000) as f32 / 1000.0; // a freeze never throws the text far
        self.last_ms = Some(now_ms);
        self.pos_px += speed_px_s(self.wpm, words_per_line, line_px) * dt;
        if self.pos_px >= total_px {
            self.pos_px = total_px.max(0.0);
            self.stop();
        }
    }
}

/// Which line is at the reading marker, when the text has scrolled `pos_px` and the marker is `marker_px` below the top.
pub fn line_at_marker(pos_px: f32, marker_px: f32, line_px: f32, lines: usize) -> usize {
    if lines == 0 {
        return 0;
    }
    (((pos_px + marker_px) / line_px.max(1.0)).floor().max(0.0) as usize).min(lines - 1)
}

/// The paragraph that contains a line.
pub fn paragraph_of_line(starts: &[usize], line: usize) -> usize {
    starts.iter().rposition(|s| *s <= line).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "First point here.\nit continues.\n\n\nSecond point, short.\n\nThird.";

    #[test]
    fn paragraphs_and_words() {
        let s = Script::from_text(TEXT);
        assert_eq!(s.paragraphs, vec!["First point here. it continues.", "Second point, short.", "Third."]);
        assert_eq!(s.words(), 9);
        assert_eq!(Script::from_text("  \n\n").paragraphs.len(), 0);
        assert_eq!(Script::from_text("a\r\nb\r\n\r\nc").paragraphs, vec!["a b", "c"]);
    }

    #[test]
    fn reading_time_follows_the_pace() {
        let s = Script::from_text(&"word ".repeat(1500));
        assert_eq!(s.duration_ms(150.0), 600_000); // 10 minutes
        assert_eq!(s.duration_ms(1.0), s.duration_ms(MIN_WPM)); // limited
    }

    #[test]
    fn speed_matches_words_and_lines() {
        // 120 wpm = 2 words a second; 8 words a line = a line every 4 seconds; lines 50 px tall = 12.5 px/s
        assert!((speed_px_s(120.0, 8.0, 50.0) - 12.5).abs() < 1e-5);
        assert!(speed_px_s(0.0, 0.0, 50.0).is_finite());
    }

    #[test]
    fn scrolling_follows_the_clock_pauses_and_stops_at_the_end() {
        let mut s = Scroller::new(120.0);
        s.tick(1000, 8.0, 50.0, 1000.0);
        assert_eq!(s.pos(), 0.0); // not started
        s.start(1000);
        s.tick(3000, 8.0, 50.0, 1000.0);
        s.tick(5000, 8.0, 50.0, 1000.0); // 4 s in two steps
        assert!((s.pos() - 50.0).abs() < 1e-3);
        s.stop();
        s.tick(9000, 8.0, 50.0, 1000.0);
        assert!((s.pos() - 50.0).abs() < 1e-3);
        s.start(10_000);
        s.tick(10_000 + 1_000_000, 8.0, 50.0, 60.0); // a huge gap is limited to 2 s
        assert!((s.pos() - 50.0 - 25.0).abs() < 1e-3 || s.pos() == 60.0);
        s.start(20_000);
        for t in 1..200u64 {
            s.tick(20_000 + t * 1000, 8.0, 50.0, 60.0);
        }
        assert_eq!(s.pos(), 60.0);
        assert!(!s.running);
    }

    #[test]
    fn the_speed_can_be_nudged_within_limits() {
        let mut s = Scroller::new(150.0);
        s.nudge(25.0);
        assert_eq!(s.wpm, 175.0);
        s.nudge(-1000.0);
        assert_eq!(s.wpm, MIN_WPM);
        s.nudge(1e6);
        assert_eq!(s.wpm, MAX_WPM);
        assert_eq!(Scroller::new(9999.0).wpm, MAX_WPM);
        s.jump_to(-50.0);
        assert_eq!(s.pos(), 0.0);
    }

    #[test]
    fn layout_gives_lines_and_paragraph_starts_and_jumps_work() {
        let s = Script::from_text(TEXT);
        let (lines, starts) = s.layout(20);
        assert_eq!(starts[0], 0);
        assert!(starts[1] > starts[0] && starts[2] > starts[1]);
        assert_eq!(lines[starts[1] - 1], ""); // blank line between paragraphs
        assert_eq!(paragraph_of_line(&starts, starts[1]), 1);
        assert_eq!(paragraph_of_line(&starts, starts[1] - 1), 0);
        assert_eq!(paragraph_of_line(&starts, 9999), 2);
        assert_eq!(line_at_marker(100.0, 50.0, 50.0, lines.len()), 3);
        assert_eq!(line_at_marker(1e9, 0.0, 50.0, 4), 3);
        assert_eq!(line_at_marker(0.0, 0.0, 50.0, 0), 0);
    }
}
