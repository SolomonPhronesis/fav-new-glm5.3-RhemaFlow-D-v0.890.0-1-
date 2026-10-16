//! Interim ("partial") results: what to do with text decoded while the
//! speaker is still talking.
//!
//! Deliberately narrow. A partial is cut off wherever the audio happened to
//! end, so it can be wrong in ways a finished utterance isn't — "verse six"
//! heard while the speaker is halfway through "sixteen". So partials only
//! ever act on **quotation** matches, which are approval-only anyway (never
//! auto-staged) and identify a verse from four or more distinctive words;
//! spoken references, commands and errors all wait for the finished
//! utterance.

use rf_core::{Detection, DetectionKind, ParseOutcome};

use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// v0.890.0 — the stability hold ("wait 0.5-1 s like R2T2") and the card
// upgrade, ported from v0.889 as an OPTIONAL setting. v0.889 always held
// every partial for 650 ms, which removed flicker but delayed the suggestive
// scriptures the operator liked in v0.856. In v0.890 the hold is OFF by
// default (hold_ms == 0, v0.856 snappiness) and the operator can switch it
// on in the settings window (500-1000 ms).
// ---------------------------------------------------------------------------

/// How long a scripture candidate must stay the SAME candidate across live
/// decodes before a partial may raise it. R2T2 never revises a word because
/// it waits; Whisper re-decodes the whole utterance every 0.4-1 s and changes
/// its mind, so we make the scripture decision wait instead.
pub const QUOTE_HOLD_MS_MIN: u64 = 500;
pub const QUOTE_HOLD_MS_MAX: u64 = 1000;
pub const QUOTE_HOLD_MS_DEFAULT: u64 = 650;
/// 0 = the hold is OFF (v0.856 behaviour: a partial may raise a card at once).
pub const QUOTE_HOLD_MS_OFF: u64 = 0;

pub fn clamp_hold_ms(ms: u64) -> u64 {
    if ms == QUOTE_HOLD_MS_OFF {
        return QUOTE_HOLD_MS_OFF;
    }
    ms.clamp(QUOTE_HOLD_MS_MIN, QUOTE_HOLD_MS_MAX)
}

/// Hold time from the `RHEMAFLOW_HOLD_MS` environment variable (clamped,
/// 0 disables), else OFF — the setting in the app always wins after startup.
pub fn hold_ms_from_env() -> u64 {
    std::env::var("RHEMAFLOW_HOLD_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(clamp_hold_ms)
        .unwrap_or(QUOTE_HOLD_MS_OFF)
}

/// Tracks, per scripture label, since when it has been present in EVERY live
/// decode of the utterance in progress. A label that drops out of one decode
/// starts its clock again — that is the "the words moved" signal.
#[derive(Default)]
pub struct QuoteHold {
    seen: Vec<(String, Instant)>,
}

impl QuoteHold {
    /// Feed the candidate labels of ONE live decode. Returns the labels that
    /// have now been continuously present for at least `hold`. `hold == 0`
    /// (the v0.856 setting) passes everything through immediately.
    pub fn observe(&mut self, labels: &[String], now: Instant, hold: Duration) -> Vec<String> {
        if hold.is_zero() {
            return labels.to_vec();
        }
        self.seen.retain(|(l, _)| labels.contains(l));
        for l in labels {
            if !self.seen.iter().any(|(x, _)| x == l) {
                self.seen.push((l.clone(), now));
            }
        }
        self.seen.iter().filter(|(_, t)| now.saturating_duration_since(*t) >= hold).map(|(l, _)| l.clone()).collect()
    }

    /// The utterance finished (or a new one began): forget everything.
    pub fn reset(&mut self) {
        self.seen.clear();
    }
}

/// What to do with a quotation detection for a verse that may already have a
/// card waiting (ported from v0.889 — the 2 Timothy 3:15 bug).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Raise {
    /// nothing new to say: the verse is already raised (and not upgradable)
    Skip,
    /// queue it (a card, or straight to the stage when auto-approve allows)
    Queue,
    /// a card for this verse is waiting from a weaker, earlier decode, and this
    /// detection is good enough to stage on its own: drop the card, stage now
    Upgrade,
}

/// The v0.856 rule was "already raised or pending -> skip", so the live
/// partial "from a child you have known the Holy Scriptures" (87%, a card)
/// blocked the FINISHED line (93%) from ever auto-staging 2 Timothy 3:15.
/// v0.890 keeps the v0.889 fix: a better finished line upgrades the card.
pub fn decide_raise(pending_card: bool, raised_earlier: bool, would_auto_stage: bool) -> Raise {
    if pending_card {
        if would_auto_stage { Raise::Upgrade } else { Raise::Skip }
    } else if raised_earlier {
        Raise::Skip
    } else {
        Raise::Queue
    }
}

/// The quotation candidates in a partial parse that should be queued for
/// approval right now: quotation matches only, and not ones already pending
/// or already raised earlier in this same utterance (`already` = passage labels).
pub fn early_quote_confirms<'a>(outcome: &'a ParseOutcome, already: &[String]) -> Vec<&'a Detection> {
    outcome
        .confirms
        .iter()
        .filter(|d| d.kind == DetectionKind::Quotation)
        .filter(|d| !already.iter().any(|l| *l == d.passage.label()))
        .collect()
}

/// Whether a partial's text is worth showing on screen: Whisper's non-speech
/// markers ("[BLANK_AUDIO]", "[MUSIC]", "(silence)") are not (PRD 6.5: "no
/// candidate is produced and no error status is raised").
pub fn worth_showing(outcome: &ParseOutcome) -> bool {
    outcome.is_speech
}

#[cfg(test)]
mod hold_tests {
    // v0.890.0 — the optional stability hold and the raise decision.
    use super::*;

    #[test]
    fn hold_off_passes_everything_through_at_once() {
        let mut h = QuoteHold::default();
        let now = Instant::now();
        let labels = vec!["John 3:16".to_string()];
        // hold == 0: no waiting, the label is ready the first time it appears
        assert_eq!(h.observe(&labels, now, Duration::ZERO), labels);
        assert_eq!(h.observe(&labels, now, Duration::ZERO), labels);
    }

    #[test]
    fn hold_waits_for_stability_and_resets_on_dropouts() {
        let mut h = QuoteHold::default();
        let now = Instant::now();
        let john = "John 3:16".to_string();
        let luke = "Luke 1:17".to_string();
        // first sight: not ready yet
        assert!(h.observe(&[john.clone()], now, Duration::from_millis(650)).is_empty());
        // still present 500 ms later: not ready
        assert!(h.observe(&[john.clone()], now + Duration::from_millis(500), Duration::from_millis(650)).is_empty());
        // 700 ms: ready
        assert_eq!(h.observe(&[john.clone()], now + Duration::from_millis(700), Duration::from_millis(650)), vec![john.clone()]);
        // the candidate MOVES to another verse ("whisper changed its mind"): clocks restart
        assert!(h.observe(&[luke.clone()], now + Duration::from_millis(800), Duration::from_millis(650)).is_empty());
        assert!(h.observe(&[john.clone()], now + Duration::from_millis(900), Duration::from_millis(650)).is_empty());
        // utterance finished: forget everything
        h.reset();
        assert!(h.observe(&[john.clone()], now + Duration::from_millis(950), Duration::from_millis(650)).is_empty());
    }

    #[test]
    fn clamp_holds_the_setting_range_and_the_off_value() {
        assert_eq!(clamp_hold_ms(0), 0);
        assert_eq!(clamp_hold_ms(100), QUOTE_HOLD_MS_MIN);
        assert_eq!(clamp_hold_ms(650), 650);
        assert_eq!(clamp_hold_ms(5_000), QUOTE_HOLD_MS_MAX);
    }

    #[test]
    fn decide_raise_upgrades_a_weaker_pending_card() {
        // the 2 Timothy 3:15 story: a partial carded 87%, the finished line is 93%
        assert_eq!(decide_raise(true, false, true), Raise::Upgrade);
        // pending and still weak: keep the card, queue nothing new
        assert_eq!(decide_raise(true, false, false), Raise::Skip);
        // already raised from an earlier partial: never twice
        assert_eq!(decide_raise(false, true, true), Raise::Skip);
        assert_eq!(decide_raise(false, true, false), Raise::Skip);
        // fresh candidate: queue (a card, or an auto-stage when allowed)
        assert_eq!(decide_raise(false, false, false), Raise::Queue);
        assert_eq!(decide_raise(false, false, true), Raise::Queue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rf_core::{parse, ParseContext};

    fn outcome(text: &str) -> ParseOutcome {
        parse(rf_bible::Bible::embedded(), text, &ParseContext::default(), 0.75)
    }

    #[test]
    fn a_quotation_is_raised_early() {
        let o = outcome("for god so loved the world");
        let got = early_quote_confirms(&o, &[]);
        assert_eq!(got.len(), 1, "{o:?}");
        assert_eq!(got[0].passage.label(), "John 3:16");
    }

    #[test]
    fn an_already_pending_or_already_raised_quotation_is_not_raised_twice() {
        let o = outcome("for god so loved the world");
        assert!(early_quote_confirms(&o, &["John 3:16".to_string()]).is_empty());
    }

    #[test]
    fn spoken_references_never_act_early() {
        // a partial can be cut mid-number ("verse six" of "sixteen"), so a
        // spoken reference must wait for the finished utterance
        let o = outcome("turn with me to john chapter three verse sixteen");
        assert!(!o.detections.is_empty(), "sanity: the parser does detect it");
        assert!(early_quote_confirms(&o, &[]).is_empty());
    }

    #[test]
    fn commands_and_chatter_never_act_early() {
        assert!(early_quote_confirms(&outcome("next verse"), &[]).is_empty());
        assert!(early_quote_confirms(&outcome("good morning everybody welcome to church"), &[]).is_empty());
    }

    #[test]
    fn non_speech_markers_are_not_shown_live() {
        for marker in ["[BLANK_AUDIO]", "[MUSIC]", "(silence)"] {
            assert!(!worth_showing(&outcome(marker)), "{marker} must not appear as live text");
        }
        assert!(worth_showing(&outcome("and so we turn to the word this morning")));
    }
}
