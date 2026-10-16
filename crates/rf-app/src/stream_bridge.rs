//! Bridge between a streaming-ASR service (Confucius4-R2T2 protocol) and the
//! app's existing parse pipeline.
//!
//! The service sends append-only text. The bridge keeps the current
//! utterance's text for the live transcript, and hands text to the parser
//! (as a finished "Transcript") in two cases:
//!   * the text is a complete reference/command and the service has been
//!     quiet for the time `rf_core::end_hint` advises (typically ~350 ms) —
//!     this is what lets a verse reach the stage before the speaker finishes
//!     breathing, and is safe *because* the text can never be revised;
//!   * the service reports the utterance finished (reset / EOS / connection
//!     lost), so nothing said is ever dropped.

use rf_asr::stream_client::StreamEvent;
use rf_core::{CommitTracker, Flush, ParseContext};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    /// live text of the utterance in progress (replaces the previous partial)
    Partial(String),
    /// text for the parser; `wait_ms` = how long after the last word it was released
    Transcript { text: String, wait_ms: u64, flush: Flush },
    Notice(String),
}

pub struct Bridge {
    tracker: CommitTracker,
    last_delta_ms: u64,
    floor: f32,
}

impl Bridge {
    pub fn new(floor: f32) -> Self {
        Bridge { tracker: CommitTracker::new(), last_delta_ms: 0, floor }
    }

    pub fn on_event(&mut self, ev: StreamEvent, now_ms: u64) -> Vec<Out> {
        match ev {
            StreamEvent::Text(t) => {
                self.tracker.push(&t, now_ms);
                self.last_delta_ms = now_ms;
                vec![Out::Partial(self.tracker.utterance_text())]
            }
            StreamEvent::Reset => self.finish(now_ms),
            StreamEvent::Connected => vec![Out::Notice("streaming speech engine connected".into())],
            StreamEvent::Disconnected(why) => {
                let mut out = self.finish(now_ms);
                out.push(Out::Notice(format!("streaming speech engine unavailable ({why}) — retrying")));
                out
            }
            StreamEvent::Cost(_) => vec![],
        }
    }

    /// Call every ~20 ms.
    pub fn tick(&mut self, now_ms: u64) -> Vec<Out> {
        match self.tracker.poll(rf_bible::Bible::embedded(), &ParseContext::default(), self.floor, now_ms) {
            Some((text, flush)) => vec![Out::Transcript { text, wait_ms: now_ms.saturating_sub(self.last_delta_ms), flush }],
            None => vec![],
        }
    }

    fn finish(&mut self, now_ms: u64) -> Vec<Out> {
        match self.tracker.end_utterance() {
            Some((text, flush)) => vec![Out::Transcript { text, wait_ms: now_ms.saturating_sub(self.last_delta_ms), flush }],
            None => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(b: &mut Bridge, words: &[&str], start: u64, step: u64) -> u64 {
        let mut t = start;
        for w in words {
            b.on_event(StreamEvent::Text(format!("{w} ")), t);
            t += step;
        }
        t - step
    }

    #[test]
    fn a_reference_reaches_the_parser_about_350ms_after_the_last_word() {
        let mut b = Bridge::new(0.75);
        let last = feed(&mut b, &["turn", "to", "john", "chapter", "three", "verse", "sixteen"], 0, 200);
        let mut released = None;
        for now in (last..last + 1000).step_by(20) {
            if let Some(Out::Transcript { text, wait_ms, flush }) = b.tick(now).into_iter().next() {
                released = Some((text, wait_ms, flush));
                break;
            }
        }
        let (text, wait, flush) = released.expect("released");
        assert_eq!(text, "turn to john chapter three verse sixteen");
        assert_eq!(flush, Flush::Settled);
        assert!((340..=420).contains(&wait), "wait {wait}");
    }

    #[test]
    fn nothing_is_dropped_when_the_service_resets_or_disconnects() {
        let mut b = Bridge::new(0.75);
        feed(&mut b, &["good", "morning", "church"], 0, 150);
        let out = b.on_event(StreamEvent::Reset, 700);
        assert!(matches!(&out[0], Out::Transcript { text, flush: Flush::UtteranceEnd, .. } if text == "good morning church"), "{out:?}");

        feed(&mut b, &["we", "worship", "you"], 1000, 150);
        let out = b.on_event(StreamEvent::Disconnected("socket closed".into()), 1600);
        assert!(matches!(&out[0], Out::Transcript { text, .. } if text == "we worship you"));
        assert!(matches!(&out[1], Out::Notice(n) if n.contains("retrying")));
    }

    #[test]
    fn the_live_partial_shows_the_whole_utterance_including_already_released_text() {
        let mut b = Bridge::new(0.75);
        feed(&mut b, &["john", "three", "sixteen"], 0, 100);
        for now in (200..1500).step_by(20) {
            if !b.tick(now).is_empty() {
                break;
            }
        }
        let out = b.on_event(StreamEvent::Text("and ".into()), 1600);
        assert_eq!(out, vec![Out::Partial("john three sixteen and".into())]);
    }

    #[test]
    fn a_silent_connection_produces_no_output_and_no_panic() {
        let mut b = Bridge::new(0.75);
        for now in (0..5000).step_by(20) {
            assert!(b.tick(now).is_empty());
        }
        assert!(b.on_event(StreamEvent::Reset, 5000).is_empty());
        assert!(b.on_event(StreamEvent::Cost(30.0), 5001).is_empty());
    }
}
