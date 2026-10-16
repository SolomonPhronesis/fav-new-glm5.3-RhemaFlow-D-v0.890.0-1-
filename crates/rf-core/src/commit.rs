//! Append-only transcript consumer.
//!
//! A true streaming engine like Confucius4-R2T2 only ever *appends* text: a
//! word that has been emitted is never revised. That single property is what
//! makes it safe to act on a spoken reference before the speaker stops — the
//! thing the old interim-decode design could never allow, because a re-decoded
//! "verse six" could turn into "verse sixteen".
//!
//! `CommitTracker` turns the stream of deltas into two things:
//!   * the text of the utterance so far (for the live transcript), and
//!   * a *settled* signal: the committed text is a finished reference/command
//!     AND no new word has arrived for as long as `end_hint` says is safe.
//!
//! Safety rules:
//!   * a delta that ends mid-word (no trailing space and no later delta) is
//!     not trusted until the settle window passes;
//!   * once settled text has been handed out, only text *after* it is new —
//!     nothing is ever delivered twice;
//!   * a hallucination loop at the tail ("okay okay okay okay okay") is cut.

use crate::parser::ParseContext;
use crate::stream::end_hint;
use rf_bible::Bible;

/// Why a chunk of text was handed to the parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flush {
    /// Complete reference/command, quiet for the advised time. Act now.
    Settled,
    /// The engine reported the end of the utterance (VAD reset / EOS).
    UtteranceEnd,
}

#[derive(Default)]
pub struct CommitTracker {
    /// text of the current utterance not yet handed out
    pending: String,
    /// milliseconds (caller's clock) of the most recent delta
    last_delta_ms: u64,
    /// text already handed out in this utterance (kept for display only)
    flushed: String,
}

impl CommitTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a delta from the engine. `now_ms` is the caller's clock.
    pub fn push(&mut self, delta: &str, now_ms: u64) {
        if delta.is_empty() {
            return;
        }
        self.pending.push_str(delta);
        self.last_delta_ms = now_ms;
    }

    /// Everything said in this utterance so far, for display.
    pub fn utterance_text(&self) -> String {
        let (a, b) = (self.flushed.trim(), self.pending.trim());
        match (a.is_empty(), b.is_empty()) {
            (true, _) => b.to_string(),
            (_, true) => a.to_string(),
            _ => format!("{a} {b}"),
        }
    }

    /// Text waiting to be parsed.
    pub fn pending_text(&self) -> &str {
        self.pending.trim()
    }

    /// Call on every tick. If the pending text is a finished reference or
    /// command and the engine has been quiet long enough, hand it out.
    pub fn poll(&mut self, bible: &Bible, ctx: &ParseContext, floor: f32, now_ms: u64) -> Option<(String, Flush)> {
        let text = clean_tail(self.pending.trim());
        if text.is_empty() {
            return None;
        }
        let hint = end_hint(bible, &text, ctx, floor)?;
        // a delta that stopped mid-word gets the longer of (hint, 400 ms)
        let mid_word = !self.pending.ends_with(' ') && !self.pending.ends_with(|c: char| c.is_ascii_punctuation());
        let wait = if mid_word { hint.silence_ms.max(400) } else { hint.silence_ms } as u64;
        // hints that mean "wait, more is coming" are long (>= 650); never act on them early
        if hint.silence_ms >= 650 || now_ms.saturating_sub(self.last_delta_ms) < wait {
            return None;
        }
        Some((self.take(text), Flush::Settled))
    }

    /// The engine said the utterance is over (or EOS): hand out the rest.
    pub fn end_utterance(&mut self) -> Option<(String, Flush)> {
        let text = clean_tail(self.pending.trim());
        self.pending.clear();
        self.flushed.clear();
        if text.is_empty() {
            None
        } else {
            Some((text, Flush::UtteranceEnd))
        }
    }

    fn take(&mut self, text: String) -> String {
        if !self.flushed.is_empty() {
            self.flushed.push(' ');
        }
        self.flushed.push_str(&text);
        self.pending.clear();
        text
    }
}

/// Cut a runaway repetition off the end ("okay okay okay okay okay ...").
/// Same idea as the guard in R2T2's server: a tail pattern repeated
/// `REPEATS`+ times is a decoder loop, not speech.
pub fn clean_tail(text: &str) -> String {
    const REPEATS: usize = 5;
    let words: Vec<&str> = text.split_whitespace().collect();
    let norm: Vec<String> = words.iter().map(|w| w.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()).collect();
    for k in 1..=8usize {
        if words.len() < k * REPEATS {
            continue;
        }
        let tail = &norm[norm.len() - k..];
        if tail.iter().all(|w| w.is_empty()) {
            continue;
        }
        let mut reps = 1;
        while words.len() >= (reps + 1) * k && norm[norm.len() - (reps + 1) * k..norm.len() - reps * k] == *tail {
            reps += 1;
        }
        if reps >= REPEATS {
            let keep = words.len() - (reps - 1) * k;
            return words[..keep].join(" ");
        }
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poll(t: &mut CommitTracker, now: u64) -> Option<(String, Flush)> {
        t.poll(Bible::embedded(), &ParseContext::default(), 0.75, now)
    }

    #[test]
    fn a_finished_reference_settles_after_the_advised_quiet_not_before() {
        let mut t = CommitTracker::new();
        for (i, w) in ["turn ", "with ", "me ", "to ", "john ", "chapter ", "three ", "verse ", "sixteen "].iter().enumerate() {
            t.push(w, 100 * i as u64);
        }
        let last = 800;
        assert_eq!(poll(&mut t, last + 100), None, "100 ms of quiet is too soon");
        let (text, why) = poll(&mut t, last + 400).expect("settled");
        assert_eq!(why, Flush::Settled);
        assert_eq!(text, "turn with me to john chapter three verse sixteen");
        assert_eq!(poll(&mut t, last + 2000), None, "never delivered twice");
    }

    #[test]
    fn an_unfinished_reference_is_never_acted_on_early() {
        let mut t = CommitTracker::new();
        t.push("turn to john chapter three verse ", 0);
        assert_eq!(poll(&mut t, 5_000), None, "waiting for the verse number, however long");
        t.push("sixteen ", 5_100);
        assert!(poll(&mut t, 5_100 + 500).is_some());
    }

    #[test]
    fn append_only_means_a_late_continuation_extends_not_replaces() {
        // "verse one" ... then "sixteen"?  Only the pending text matters; it grows.
        let mut t = CommitTracker::new();
        t.push("john three verse ", 0);
        t.push("one ", 100);
        // too soon to act (350/450 ms rule) ...
        assert_eq!(poll(&mut t, 200), None);
        // ... and if more arrives, the longer text is what gets parsed
        t.push("through three ", 300);
        let (text, _) = poll(&mut t, 300 + 600).expect("settled");
        assert_eq!(text, "john three verse one through three");
    }

    #[test]
    fn a_half_word_waits_longer() {
        let mut t = CommitTracker::new();
        t.push("john three sixteen", 0); // no trailing space: could still become "sixteenth"/more
        assert_eq!(poll(&mut t, 360), None);
        assert!(poll(&mut t, 420).is_some());
    }

    #[test]
    fn utterance_end_flushes_the_remainder_once() {
        let mut t = CommitTracker::new();
        t.push("good morning church ", 0);
        assert_eq!(poll(&mut t, 5_000), None, "ordinary speech never settles early");
        let (text, why) = t.end_utterance().unwrap();
        assert_eq!((text.as_str(), why), ("good morning church", Flush::UtteranceEnd));
        assert!(t.end_utterance().is_none());
        assert_eq!(t.utterance_text(), "");
    }

    #[test]
    fn display_text_includes_what_was_already_handed_out() {
        let mut t = CommitTracker::new();
        t.push("john three sixteen ", 0);
        poll(&mut t, 1_000).unwrap();
        t.push("and for god so loved ", 1_100);
        assert_eq!(t.utterance_text(), "john three sixteen and for god so loved");
        assert_eq!(t.pending_text(), "and for god so loved");
    }

    #[test]
    fn decoder_loops_are_cut_from_the_tail() {
        assert_eq!(clean_tail("amen okay okay okay okay okay okay okay"), "amen okay");
        assert_eq!(clean_tail("praise the lord praise the lord"), "praise the lord praise the lord");
        assert_eq!(clean_tail("we love you we love you we love you we love you we love you we love you"), "we love you");
        assert_eq!(clean_tail("one two three"), "one two three");
    }

    #[test]
    fn commands_settle_fast_but_stop_listening_never_does() {
        let mut t = CommitTracker::new();
        t.push("next verse ", 0);
        assert!(poll(&mut t, 350).is_some());
        let mut t = CommitTracker::new();
        t.push("stop listening ", 0);
        assert_eq!(poll(&mut t, 10_000), None);
    }
}
