//! Low-latency speech plumbing that needs Scripture knowledge.
//!
//! Two ideas from streaming-ASR practice, adapted to a pulpit:
//!
//! 1. **Agreement** (the "LocalAgreement-2" rule from the whisper-streaming
//!    line of work): while someone is still talking, every interim decode
//!    re-transcribes a longer slice of the same speech. Words that two
//!    *consecutive* decodes agree on are stable; words that keep changing
//!    are not. Acting only on stable words removes most "verse six → verse
//!    sixteen" style mistakes that come from trusting a cut-off guess.
//!
//! 2. **Semantic endpointing**: a fixed "wait 800 ms of silence" is wrong
//!    both ways. "…John chapter three verse sixteen" is finished — waiting
//!    longer only adds lag. "…John chapter three verse" is not — cutting it
//!    off loses the verse. `end_hint` reads the interim text with the real
//!    reference parser and recommends how much silence to wait for.

use crate::parser::{parse, Command, DetectionKind, ParseContext};
use crate::wer::normalize_words;
use rf_bible::Bible;

fn norm_token(t: &str) -> String {
    t.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// Result of feeding one interim decode to [`Agreement`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgreementView {
    /// Number of leading words both of the last two decodes agree on.
    pub stable_words: usize,
    /// The stable prefix, in the decoder's own spelling.
    pub stable: String,
    /// The rest (still changing) — show it greyed out.
    pub unstable: String,
}

#[derive(Default)]
pub struct Agreement {
    prev: Vec<String>,
}

impl Agreement {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed the newest interim decode of the current utterance.
    pub fn update(&mut self, hypothesis: &str) -> AgreementView {
        let shown: Vec<&str> = hypothesis.split_whitespace().collect();
        let cur: Vec<String> = shown.iter().map(|w| norm_token(w)).collect();
        let stable_words = self
            .prev
            .iter()
            .zip(cur.iter())
            .take_while(|(a, b)| a == b && !a.is_empty())
            .count();
        self.prev = cur;
        AgreementView {
            stable_words,
            stable: shown[..stable_words].join(" "),
            unstable: shown[stable_words..].join(" "),
        }
    }

    /// Call when the utterance ends: the next one starts from nothing.
    pub fn reset(&mut self) {
        self.prev.clear();
    }
}

/// How long the audio layer should wait in silence before calling the
/// utterance finished, based on what has been heard so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndHint {
    pub silence_ms: u32,
    pub reason: &'static str,
}

/// Words that mean "a reference is still being spoken".
const DANGLING: &[&str] = &["chapter", "chapters", "verse", "verses", "through", "thru", "to", "and", "from", "versus"];
const TENS: &[&str] = &["twenty", "thirty", "forty", "fourty", "fifty", "sixty", "seventy", "eighty", "ninety", "hundred"];
const DIGIT_WORDS: &[&str] = &["one", "two", "three", "four", "five", "six", "seven", "eight", "nine"];

/// Recommend a trailing-silence length for the text heard so far, or `None`
/// to keep the audio layer's default behaviour (ordinary speech).
pub fn end_hint(bible: &Bible, text: &str, ctx: &ParseContext, floor: f32) -> Option<EndHint> {
    let words = normalize_words(text);
    let last = words.last()?.as_str();
    let hint = |silence_ms, reason| Some(EndHint { silence_ms, reason });

    let outcome = parse(bible, text, ctx, floor);
    if !outcome.is_speech {
        return None;
    }

    // A complete bare command ("next verse", "zoom in"): quick. Lifecycle
    // commands are operator-only and never shortened.
    if !outcome.commands.is_empty() && outcome.detections.is_empty() && outcome.errors.is_empty() {
        let harmless = outcome.commands.iter().all(|c| !matches!(c, Command::StopListening | Command::StartRecording | Command::StopRecording));
        return if harmless { hint(300, "command complete") } else { None };
    }

    // An unfinished number or joiner: the speaker is mid-reference. Wait.
    if TENS.contains(&last) {
        return hint(900, "number may continue");
    }
    let mentions_reference = words.iter().any(|w| {
        ["chapter", "chapters", "verse", "verses"].contains(&w.as_str())
            || DIGIT_WORDS.contains(&w.as_str())
            || (!w.is_empty() && w.chars().all(|c| c.is_ascii_digit()))
    });
    if DANGLING.contains(&last) && mentions_reference {
        return hint(1100, "reference unfinished");
    }
    let kinds: Vec<DetectionKind> = outcome.detections.iter().map(|d| d.kind).collect();
    if kinds.iter().any(|k| matches!(k, DetectionKind::Explicit | DetectionKind::Relative)) {
        // "…verse one" may still become "one hundred"/"one through three"
        return if DIGIT_WORDS.contains(&last) { hint(450, "reference complete (short verse number)") } else { hint(350, "reference complete") };
    }
    if !outcome.errors.is_empty() {
        return hint(450, "reference out of range");
    }
    if kinds.contains(&DetectionKind::ChapterOnly) {
        return hint(650, "chapter spoken, verse may follow");
    }
    if outcome.confirms.iter().any(|d| d.kind == DetectionKind::BookOnly) {
        return hint(900, "book spoken, chapter may follow");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(text: &str) -> Option<EndHint> {
        end_hint(Bible::embedded(), text, &ParseContext::default(), 0.75)
    }

    #[test]
    fn agreement_marks_only_repeated_prefix_as_stable() {
        let mut a = Agreement::new();
        let v = a.update("turn to john");
        assert_eq!(v.stable_words, 0, "first decode has nothing to agree with");
        let v = a.update("turn to John chapter three");
        assert_eq!(v.stable, "turn to John");
        assert_eq!(v.unstable, "chapter three");
        let v = a.update("Turn to John, chapter three verse six");
        assert_eq!(v.stable, "Turn to John, chapter three", "case/punctuation don't break agreement");
        assert_eq!(v.unstable, "verse six");
    }

    #[test]
    fn agreement_drops_words_the_decoder_changed_its_mind_about() {
        let mut a = Agreement::new();
        a.update("john three verse six");
        let v = a.update("john three verse sixteen");
        assert_eq!(v.stable, "john three verse", "six→sixteen is not stable yet");
        a.reset();
        assert_eq!(a.update("fresh start").stable_words, 0);
    }

    #[test]
    fn finished_references_end_fast() {
        for t in ["turn with me to john chapter three verse sixteen", "Romans 8:28", "john three sixteen"] {
            let e = h(t).unwrap_or_else(|| panic!("no hint for {t}"));
            assert!(e.silence_ms <= 450, "{t}: {e:?}");
        }
    }

    #[test]
    fn unfinished_references_wait_longer_than_the_default() {
        for t in ["turn with me to john chapter three verse", "john chapter", "first corinthians thirteen verses four through", "psalm one nineteen verse twenty"] {
            let e = h(t).unwrap_or_else(|| panic!("no hint for {t}"));
            assert!(e.silence_ms >= 900, "{t}: {e:?}");
        }
    }

    #[test]
    fn chapter_only_waits_for_a_possible_verse() {
        let e = h("turn to john chapter three").unwrap();
        assert!(e.silence_ms > 350 && e.silence_ms < 900, "{e:?}");
    }

    #[test]
    fn bare_commands_are_quick_but_lifecycle_commands_are_never_shortened() {
        assert_eq!(h("next verse").unwrap().silence_ms, 300);
        assert_eq!(h("zoom in").unwrap().silence_ms, 300);
        assert!(h("stop listening").is_none());
    }

    #[test]
    fn ordinary_sermon_speech_keeps_the_default() {
        assert!(h("good morning everybody welcome to church").is_none());
        assert!(h("and so the lord is good all the time").is_none());
        assert!(h("").is_none());
        assert!(h("[BLANK_AUDIO]").is_none());
    }
}
