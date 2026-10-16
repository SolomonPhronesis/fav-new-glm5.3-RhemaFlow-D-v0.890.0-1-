//! Timeline simulation of the speech pipeline: how long after the speaker
//! stops does the reference reach the stage, and is a mid-reference breath
//! kept in one utterance?
//!
//! This is a MODEL, not a measurement. Audio is synthetic, the "engine" is a
//! scripted decoder, and decode time is an assumed constant. What it does
//! exercise for real is the production policy code: `Vad`, `PartialPacer`,
//! `EndHintCell` semantics and `rf_core::end_hint`. Real-hardware numbers
//! come from `rhemaflow-d calibrate`.

use rf_asr::vad::{PartialPacer, Vad};
use rf_core::{end_hint, ParseContext};

const WORDS: [&str; 9] = ["turn", "with", "me", "to", "john", "chapter", "three", "verse", "sixteen"];
/// frame (10 ms) at which each word has been fully spoken, within the speech only
const WORD_END: [u32; 9] = [30, 60, 90, 120, 150, 190, 220, 240, 280];

fn transcript(speech_frames: u32) -> String {
    let n = WORD_END.iter().filter(|e| **e <= speech_frames).count();
    WORDS[..n].join(" ")
}

#[derive(Clone, Copy)]
enum Policy {
    /// v0.804: fixed 800 ms tail, decode only after the utterance ends.
    Old,
    /// v0.806: pause-time speculative decode + semantic endpointing + reuse.
    New,
}

struct Run {
    /// ms from the LAST spoken frame until the finished text is available
    ready_after_ms: u64,
    utterances: usize,
    final_text: String,
}

/// `segments`: (speech_frames, silence_frames_after) pairs fed in order.
fn simulate(policy: Policy, decode_ms: u64, segments: &[(u32, u32)]) -> Run {
    let bible = rf_bible::Bible::embedded();
    let ctx = ParseContext::default();
    let mut vad = Vad::new();
    let mut pacer = PartialPacer::new();
    let sine = |n: usize| -> Vec<f32> { (0..n).map(|i| (i as f32 * 0.05).sin() * 0.2).collect() };

    let mut now: u64 = 0;
    let mut busy_until: u64 = 0;
    let mut pending: Option<(u64, u32, u32, String)> = None; // (done_at, seq, speech_frames, text)
    let mut cache: Option<(u32, u32, String)> = None;
    let mut last_speech_ms: u64 = 0;
    let mut ready_at: Option<u64> = None;
    let mut utterances = 0;
    let mut final_text = String::new();

    vad.feed(&vec![0.0; 16_000]); // 1 s lead-in silence
    now += 1000;

    let mut step = |vad: &mut Vad, samples: Vec<f32>, speech: bool, now: &mut u64| {
        vad.feed_with_meta(&[]); // no-op, keeps API symmetrical
        let finals = vad.feed_with_meta(&samples);
        *now += 10;
        if speech {
            last_speech_ms = *now;
        }
        // worker completes a speculative decode
        if let Some((done, seq, sf, text)) = pending.clone() {
            if *now >= done {
                pending = None;
                if let Some(h) = end_hint(bible, &text, &ctx, 0.75) {
                    vad.set_end_hint(h.silence_ms, seq, sf);
                }
                cache = Some((seq, sf, text));
            }
        }
        for (_, meta) in finals {
            utterances += 1;
            let text = transcript(meta.speech_frames);
            let hit = matches!(&cache, Some((s, f, _)) if *s == meta.seq && *f == meta.speech_frames);
            // The real worker is single-threaded: a final queued behind an
            // in-flight pause decode of the SAME speech is served from that
            // decode's result the moment it completes — no second decode.
            let inflight = matches!(&pending, Some((_, s, f, _)) if *s == meta.seq && *f == meta.speech_frames);
            let t = if hit {
                *now
            } else if inflight {
                pending.as_ref().map(|p| p.0).unwrap_or(*now)
            } else {
                (*now).max(busy_until) + decode_ms
            };
            final_text = text;
            ready_at = Some(t);
            cache = None;
            pacer.reset();
        }
        if matches!(policy, Policy::New) && ready_at.is_none() {
            if let Some((_, sf, seq)) = vad.pause_snapshot() {
                if pending.is_none() && pacer.should_emit_pause(sf as usize, *now < busy_until, 0) {
                    busy_until = *now + decode_ms;
                    pending = Some((busy_until, seq, sf, transcript(sf)));
                }
            }
        }
    };

    for &(speech, silence) in segments {
        for _ in 0..speech {
            step(&mut vad, sine(160), true, &mut now);
        }
        for _ in 0..silence {
            step(&mut vad, vec![0.0; 160], false, &mut now);
        }
    }
    Run { ready_after_ms: ready_at.map(|t| t.saturating_sub(last_speech_ms)).unwrap_or(u64::MAX), utterances, final_text }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHOLE_REFERENCE: [(u32, u32); 1] = [(280, 400 / 10 * 4)]; // speak 2.8 s, then quiet

    #[test]
    fn finished_reference_is_ready_much_sooner() {
        println!("\n  decode    old (ms after last word)    new");
        for decode in [100u64, 250, 500, 800] {
            let old = simulate(Policy::Old, decode, &WHOLE_REFERENCE);
            let new = simulate(Policy::New, decode, &WHOLE_REFERENCE);
            println!("  {decode:>4} ms   {:>10}                 {:>6}", old.ready_after_ms, new.ready_after_ms);
            assert_eq!(old.final_text, new.final_text);
            assert_eq!(new.final_text, WORDS.join(" "));
            assert!(new.ready_after_ms + 250 <= old.ready_after_ms, "decode {decode}: old {} new {}", old.ready_after_ms, new.ready_after_ms);
            if decode <= 500 {
                assert!(new.ready_after_ms <= 650, "decode {decode}: new {}", new.ready_after_ms);
            }
        }
        // the headline case: a decode that takes a quarter second
        assert!(simulate(Policy::New, 250, &WHOLE_REFERENCE).ready_after_ms <= 400);
        assert!(simulate(Policy::Old, 250, &WHOLE_REFERENCE).ready_after_ms >= 1000);
    }

    #[test]
    fn a_breath_mid_reference_no_longer_splits_the_utterance() {
        // "…chapter three verse" <0.9 s pause> "sixteen"
        let segs = [(240u32, 90u32), (40, 120)];
        let old = simulate(Policy::Old, 250, &segs);
        let new = simulate(Policy::New, 250, &segs);
        assert_eq!(old.utterances, 2, "old policy cuts at 800 ms and loses the verse number");
        assert_eq!(new.utterances, 1, "new policy sees an unfinished reference and waits");
        assert_eq!(new.final_text, WORDS.join(" "));
    }

    #[test]
    fn ordinary_speech_is_never_made_slower_than_the_old_default() {
        // nothing the parser recognises → no hint → identical to the old tail
        let segs = [(280u32, 120u32)];
        let hint = end_hint(rf_bible::Bible::embedded(), "the lord is my shepherd i shall not want", &ParseContext::default(), 0.75);
        assert!(hint.is_none());
        let old = simulate(Policy::Old, 250, &segs);
        let new = simulate(Policy::New, 250, &segs);
        assert!(new.ready_after_ms <= old.ready_after_ms + 20);
    }
}
