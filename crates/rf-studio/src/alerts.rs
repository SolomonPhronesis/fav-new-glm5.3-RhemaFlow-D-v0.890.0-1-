//! Feature 13: alerts and nursery calls. A short message over the picture (or a scrolling strip), with a life span,
//! a priority, and a log of what was sent. Pure: the caller passes the clock and draws the result.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Kind {
    /// Shown first.
    Urgent,
    Nursery,
    Info,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Urgent => "Urgent",
            Kind::Nursery => "Nursery",
            Kind::Info => "Info",
        }
    }
    /// How long it stays unless told otherwise. `None` = until someone dismisses it.
    pub fn default_ttl_ms(self) -> Option<u64> {
        match self {
            Kind::Urgent => None,
            Kind::Nursery => Some(60_000),
            Kind::Info => Some(15_000),
        }
    }
    pub fn color(self) -> [u8; 3] {
        match self {
            Kind::Urgent => [220, 50, 50],
            Kind::Nursery => [240, 170, 0],
            Kind::Info => [60, 120, 220],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alert {
    pub id: u64,
    pub kind: Kind,
    pub text: String,
    pub created_ms: u64,
    pub expires_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sent {
    pub text: String,
    pub kind: Kind,
    pub at_ms: u64,
}

pub const MAX_TEXT: usize = 160;

#[derive(Clone, Debug, Default)]
pub struct Queue {
    items: Vec<Alert>,
    next_id: u64,
    pub log: Vec<Sent>,
}

/// "214" or "A-12": 1 to 8 letters, digits or dashes. Stops a typo from putting nonsense on the screen.
pub fn valid_code(code: &str) -> bool {
    let c = code.trim();
    !c.is_empty() && c.chars().count() <= 8 && c.chars().all(|ch| ch.is_alphanumeric() || ch == '-')
}

pub const NURSERY_TEMPLATE: &str = "Nursery: parents of {code}, please come to the nursery";

/// The sentence for a nursery call. `template` may contain `{code}`; `None` uses the standard one.
pub fn nursery_message(code: &str, template: Option<&str>) -> Result<String, String> {
    if !valid_code(code) {
        return Err("The code must be 1 to 8 letters, numbers or dashes (for example 214 or A-12).".into());
    }
    Ok(template.unwrap_or(NURSERY_TEMPLATE).replace("{code}", code.trim().to_uppercase().as_str()))
}

impl Queue {
    pub fn new() -> Queue {
        Queue { items: Vec::new(), next_id: 1, log: Vec::new() }
    }

    /// Put up a message. The same words already showing are refreshed instead of shown twice. Returns its number.
    /// `ttl_ms` of `None` uses the kind's usual time.
    pub fn push(&mut self, kind: Kind, text: &str, now_ms: u64, ttl_ms: Option<u64>) -> Result<u64, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("There is nothing to show: the message is empty.".into());
        }
        let text: String = text.chars().take(MAX_TEXT).collect();
        let ttl = ttl_ms.or_else(|| kind.default_ttl_ms());
        let exp = ttl.map(|t| now_ms + t);
        self.expire(now_ms);
        if let Some(a) = self.items.iter_mut().find(|a| a.text == text && a.kind == kind) {
            a.created_ms = now_ms;
            a.expires_ms = exp;
            return Ok(a.id);
        }
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        self.items.push(Alert { id, kind, text: text.clone(), created_ms: now_ms, expires_ms: exp });
        self.log.push(Sent { text, kind, at_ms: now_ms });
        Ok(id)
    }

    pub fn expire(&mut self, now_ms: u64) {
        self.items.retain(|a| a.expires_ms.map_or(true, |e| now_ms < e));
    }
    pub fn dismiss(&mut self, id: u64) -> bool {
        let n = self.items.len();
        self.items.retain(|a| a.id != id);
        self.items.len() != n
    }
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// What should be on the screen now: most urgent first, then oldest first; at most `limit`.
    pub fn active(&self, now_ms: u64, limit: usize) -> Vec<&Alert> {
        let mut v: Vec<&Alert> = self.items.iter().filter(|a| a.expires_ms.map_or(true, |e| now_ms < e)).collect();
        v.sort_by_key(|a| (a.kind, a.created_ms, a.id));
        v.truncate(limit);
        v
    }
    pub fn is_empty(&self, now_ms: u64) -> bool {
        self.active(now_ms, 1).is_empty()
    }
    /// Seconds left on a message, if it has an end.
    pub fn seconds_left(&self, id: u64, now_ms: u64) -> Option<u64> {
        self.items.iter().find(|a| a.id == id).and_then(|a| a.expires_ms).map(|e| e.saturating_sub(now_ms).div_ceil(1000))
    }
}

/// Horizontal position of a scrolling strip: the text enters at the right edge, crosses, leaves on the left,
/// then waits one half-screen before coming again. `elapsed_ms` counts from when it started.
pub fn ticker_x(elapsed_ms: u64, speed_px_s: f32, text_w: f32, view_w: f32) -> f32 {
    let gap = view_w * 0.5;
    let period = (view_w + text_w + gap).max(1.0);
    let travelled = (elapsed_ms as f32 / 1000.0 * speed_px_s.max(1.0)) % period;
    view_w - travelled
}

/// How long one pass takes.
pub fn ticker_period_ms(speed_px_s: f32, text_w: f32, view_w: f32) -> u64 {
    (((view_w + text_w + view_w * 0.5) / speed_px_s.max(1.0)) * 1000.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_checked_and_the_sentence_is_made() {
        assert!(valid_code("214") && valid_code("A-12") && valid_code(" 7 "));
        for bad in ["", "   ", "123456789", "a b", "<b>", "12;3"] {
            assert!(!valid_code(bad), "{bad:?}");
        }
        assert_eq!(nursery_message("a-12", None).unwrap(), "Nursery: parents of A-12, please come to the nursery");
        assert_eq!(nursery_message("214", Some("Parents of {code} please")).unwrap(), "Parents of 214 please");
        assert!(nursery_message("", None).is_err());
    }

    #[test]
    fn messages_live_for_their_time_then_go() {
        let mut q = Queue::new();
        let id = q.push(Kind::Info, "Car lights on", 1000, None).unwrap();
        assert_eq!(q.active(1000, 5).len(), 1);
        assert_eq!(q.seconds_left(id, 1000), Some(15));
        assert_eq!(q.active(15_999, 5).len(), 1);
        assert!(q.active(16_000, 5).is_empty());
        assert!(q.is_empty(16_000));
    }

    #[test]
    fn an_urgent_message_stays_until_dismissed_and_comes_first() {
        let mut q = Queue::new();
        q.push(Kind::Info, "Coffee after", 0, None).unwrap();
        q.push(Kind::Nursery, "Nursery: 214", 10, None).unwrap();
        let u = q.push(Kind::Urgent, "Fire drill", 20, None).unwrap();
        let order: Vec<Kind> = q.active(30, 5).iter().map(|a| a.kind).collect();
        assert_eq!(order, vec![Kind::Urgent, Kind::Nursery, Kind::Info]);
        assert_eq!(q.seconds_left(u, 30), None);
        assert_eq!(q.active(10_000_000, 5).len(), 1);
        assert!(q.dismiss(u));
        assert!(!q.dismiss(u));
    }

    #[test]
    fn the_same_words_are_refreshed_not_doubled() {
        let mut q = Queue::new();
        let a = q.push(Kind::Nursery, "Nursery: 214", 0, None).unwrap();
        let b = q.push(Kind::Nursery, "  Nursery: 214 ", 50_000, None).unwrap();
        assert_eq!(a, b);
        assert_eq!(q.active(100_000, 5).len(), 1); // refreshed: would have ended at 60 000
        assert_eq!(q.log.len(), 1);
    }

    #[test]
    fn the_limit_and_empty_and_long_messages() {
        let mut q = Queue::new();
        for i in 0..5 {
            q.push(Kind::Info, &format!("m{i}"), i, Some(10_000)).unwrap();
        }
        assert_eq!(q.active(10, 2).len(), 2);
        assert!(q.push(Kind::Info, "   ", 0, None).is_err());
        let id = q.push(Kind::Info, &"x".repeat(1000), 0, None).unwrap();
        assert_eq!(q.active(1, 10).iter().find(|a| a.id == id).unwrap().text.chars().count(), MAX_TEXT);
        q.clear();
        assert!(q.is_empty(0));
    }

    #[test]
    fn the_log_remembers_what_was_sent() {
        let mut q = Queue::new();
        q.push(Kind::Info, "a", 5, None).unwrap();
        q.push(Kind::Urgent, "b", 6, None).unwrap();
        q.dismiss(2);
        assert_eq!(q.log.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn the_ticker_crosses_the_screen_and_comes_round_again() {
        let (view, text, speed) = (1000.0, 400.0, 200.0);
        assert_eq!(ticker_x(0, speed, text, view), 1000.0); // just off the right edge
        assert_eq!(ticker_x(1000, speed, text, view), 800.0);
        let left_edge_gone = ticker_x(7000, speed, text, view); // 1400 px travelled: fully off the left
        assert!(left_edge_gone <= -text);
        let period = ticker_period_ms(speed, text, view);
        assert_eq!(period, 9500);
        assert_eq!(ticker_x(period + 1000, speed, text, view), ticker_x(1000, speed, text, view));
        assert!(ticker_x(5000, 0.0, text, view).is_finite()); // a zero speed is made safe
    }
}
