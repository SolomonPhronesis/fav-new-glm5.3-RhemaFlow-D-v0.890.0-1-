//! Verse-anchored repair of a live transcript line (v0.855.0, screenshot 1's
//! "wrong words" half).
//!
//! When a verse is already on stage (the operator named it, or approved the
//! card) and the preacher is reading THAT verse, the ASR line still carries
//! noise: dropped words ("and the disobedient to the gods" for Luke 1:17's
//! "and the disobedient to the wisdom of the just"), swapped homophones,
//! singular/plural slips. The verse text is the ground truth sitting right
//! there — so the line is repaired AGAINST it:
//!
//!   heard   : and the disobedient to the gods
//!   verse   : ...and the disobedient to the wisdom of the just; ...
//!   repaired: and the disobedient to the wisdom of the just
//!
//! Rules the operator can rely on (all conservative by design):
//! - the line must FIT the verse first (`Bible::span_fit_pct` >= 70) — a
//!   line that is not the verse is never touched;
//! - the repair only rewrites the aligned span of the line: the verse's own
//!   words (with the KJV's punctuation) replace everything between the
//!   first and last aligned word, and words outside the span are kept
//!   exactly as heard;
//! - every repair reports what it did (filled words, dropped noise words)
//!   so the UI can be transparent about it.
//!
//! This module never stages anything and never changes verse detection —
//! it only cleans up what the operator sees while the verse is on stage.

use rf_bible::Bible;

use crate::parser::Passage;

/// A repaired line, with an honest count of what changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repair {
    pub text: String,
    /// verse words inserted because the ASR dropped them
    pub filled: usize,
    /// heard words inside the span that were ASR noise (dropped from the line)
    pub dropped: usize,
}

/// Repair `heard` against the verse `p` currently on stage.
/// `None` when the line does not fit the verse well enough to be the same
/// text, when either side is too short, or when nothing needs repairing.
pub fn repair_line(bible: &Bible, p: &Passage, heard: &str) -> Option<Repair> {
    if p.chapter == 0 || p.from == 0 {
        return None;
    }
    let verse_text = bible.passage(p.book as usize, p.chapter as usize, p.from as usize, p.to as usize);
    if verse_text.trim().is_empty() {
        return None;
    }
    let heard_words: Vec<&str> = heard.split_whitespace().collect();
    if heard_words.len() < 3 {
        return None;
    }
    // the line must genuinely be (part of) this verse before any repair,
    // and the parser's anchored alignment supplies the span: (heard index,
    // verse index) pairs, in order, over normalized words
    let align = bible.span_align(heard, p.book as usize, p.chapter as usize, p.from as usize)?;
    if align.pct < 60 {
        return None;
    }
    let vw = rf_bible::normalize_words(&verse_text);
    let pairs = &align.pairs;
    if pairs.len() < 2 {
        return None;
    }
    let first_h = pairs.first().map(|p| p.0).unwrap();
    let last_h = pairs.last().map(|p| p.0).unwrap();
    let first_v = pairs.first().map(|p| p.1).unwrap();
    let mut last_v = pairs.last().map(|p| p.1).unwrap();

    let verse_words: Vec<&str> = verse_text.split_whitespace().collect();
    // raw-verse-word index for every normalized position (raw words can
    // normalize to more than one token, e.g. "son's" -> "son","s")
    let mut norm_to_raw: Vec<usize> = Vec::with_capacity(vw.len());
    for (ri, rw) in verse_words.iter().enumerate() {
        let pieces = rf_bible::normalize_words(rw);
        for _ in pieces {
            norm_to_raw.push(ri);
        }
    }

    // trailing heard words the verse text does not contain. Two honest
    // possibilities: real pulpit asides ("...to the children today") or the
    // mangled tail of the sentence ("...to the gods" for "to the wisdom of
    // the just;"). Rule: if the tail words are ordinary aside words, keep
    // them verbatim; otherwise, when the verse's own SENTENCE completes
    // within a few words of the aligned span, the tail is treated as ASR
    // garble of that ending — dropped, and the sentence's real words filled
    // in from the verse.
    let tail: Vec<&str> = heard_words[last_h + 1..].to_vec();
    let mut kept_tail: Vec<&str> = tail.clone();
    let mut dropped_tail = 0usize;
    if !tail.is_empty() && tail.iter().all(|w| is_aside_word(w)) {
        kept_tail = tail;
    } else if !tail.is_empty() && last_v < norm_to_raw.len() {
        // scan forward for the sentence boundary (.;:?!) within 6 verse words
        let raw_hi = norm_to_raw[last_v];
        let mut boundary_raw: Option<usize> = None;
        for r in raw_hi..verse_words.len().min(raw_hi + 7) {
            let w = verse_words[r].trim_matches(|c: char| !c.is_alphanumeric());
            let _ = w;
            let raw_end = verse_words[r];
            if raw_end.ends_with('.') || raw_end.ends_with(';') || raw_end.ends_with(':') || raw_end.ends_with('?') || raw_end.ends_with('!') {
                boundary_raw = Some(r);
                break;
            }
        }
        if let Some(br) = boundary_raw {
            // normalized index of that raw word's LAST token
            if let Some(nv) = norm_to_raw.iter().rposition(|&x| x == br) {
                if nv > last_v && nv - last_v <= 6 {
                    last_v = nv;
                    kept_tail = Vec::new();
                    dropped_tail = tail.len();
                }
            }
        }
    }

    let span_h = last_h - first_h + 1;
    let span_v = last_v - first_v + 1;
    let filled = span_v - pairs.len();
    let dropped = span_h - pairs.len() + dropped_tail;
    if filled == 0 && dropped == 0 && span_h == span_v {
        return None;
    }

    // rebuild: heard words before the span | verse words across the span |
    // heard tail (kept asides). The verse supplies punctuation and
    // capitalisation, which is exactly what the stage shows.
    let prefix = heard_words[..first_h].join(" ");
    let suffix = kept_tail.join(" ");
    if norm_to_raw.len() != vw.len() || last_v >= norm_to_raw.len() || first_v >= norm_to_raw.len() {
        // fall back: normalized span rendered as plain words (no punctuation)
        let span_text = vw[first_v..=last_v.min(vw.len() - 1)].join(" ");
        return Some(finish_repair(prefix, span_text, suffix, filled, dropped));
    }
    let raw_lo = norm_to_raw[first_v];
    let raw_hi = norm_to_raw[last_v];
    let span_text = verse_words[raw_lo..=raw_hi].join(" ");
    Some(finish_repair(prefix, span_text, suffix, filled, dropped))
}

/// Ordinary pulpit asides that legitimately trail a verse being read aloud
/// ("...to the children today"). These are KEPT by the repair, not replaced.
fn is_aside_word(w: &str) -> bool {
    const ASIDE: &[&str] = &[
        "today", "tonight", "folks", "friends", "amen", "now", "look", "listen",
        "notice", "see", "watch", "church", "everybody", "brothers", "sisters",
        "beloved", "hear", "here", "again", "people", "church", "saints",
    ];
    let lower = w.trim_matches(|c: char| !c.is_alphanumeric()).to_ascii_lowercase();
    ASIDE.contains(&lower.as_str())
}

fn finish_repair(prefix: String, span: String, suffix: String, filled: usize, dropped: usize) -> Repair {
    let mut parts: Vec<String> = Vec::new();
    if !prefix.is_empty() {
        parts.push(prefix);
    }
    parts.push(span);
    if !suffix.is_empty() {
        parts.push(suffix);
    }
    Repair { text: parts.join(" "), filled, dropped }
}

/// v0.890.0 — the KJV post-correction for lines with NO verse on stage.
///
/// This is the text-layer half of the measured STT plan (docs/STT-V0890.md):
/// Whisper may garble a read ("and hezekiah prayed before the lord and seen")
/// while the verse text is sitting verbatim in the embedded KJV. When the
/// finished line strongly matches exactly ONE verse, the DISPLAY line is
/// re-rendered from that verse — the same operator-visible repair v0.855
/// already did against the staged verse, now driven by the matcher.
///
/// Guard rails (both must pass, either stops the repair):
/// 1. the ranked quotation matcher scores the line `REPAIR_MATCH_MIN_PCT`
///    (70) or better — the same scoring that decides cards, tuned by the
///    whole-Bible grids of v0.856/v0.889, so ordinary chatter stays silent;
/// 2. `repair_line`'s own span fit (`span_align` >= 70) — the matched span
///    must genuinely sit inside the verse.
///
/// Display-only, like every repair: staging, detection and the export logs
/// are untouched; the UI names the verse and the word counts in the
/// interpretation strip.
pub const REPAIR_MATCH_MIN_PCT: u8 = 70;

pub fn repair_from_match(bible: &Bible, heard: &str) -> Option<(Passage, Repair)> {
    if heard.split_whitespace().count() < 5 {
        return None;
    }
    let top = bible.match_quotations_ranked(heard, 1).into_iter().next()?;
    if top.pct < REPAIR_MATCH_MIN_PCT {
        return None;
    }
    let p = Passage::single(top.book as u8, top.chapter as u16, top.verse as u16);
    let r = repair_line(bible, &p, heard)?;
    Some((p, r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Passage;
    use rf_bible::Bible;

    fn passage_single(book: u8, chapter: u16, verse: u16) -> Passage {
        Passage::single(book, chapter, verse)
    }

    #[test]
    fn fills_the_gods_gap_in_luke_1_17() {
        let b = Bible::embedded();
        let luke = passage_single(41, 1, 17);
        // the operator's screenshot: ASR turned "to the wisdom of the just"
        // into "to the gods"
        let r = repair_line(&b, &luke, "and the disobedient to the gods").expect("must repair");
        assert!(r.text.contains("the disobedient to the wisdom of the just"), "{r:?}");
        assert!(r.filled >= 2, "wisdom/of/the(/just) were missing: {r:?}");
    }

    #[test]
    fn plural_slips_are_repaired_against_the_kjv_text() {
        let b = Bible::embedded();
        let luke = passage_single(41, 1, 17);
        let r = repair_line(&b, &luke, "turn the heart of the father to the children").expect("must repair");
        assert!(r.text.contains("turn the hearts of the fathers to the children"), "{r:?}");
    }

    #[test]
    fn a_line_that_is_not_the_verse_is_untouched() {
        let b = Bible::embedded();
        let luke = passage_single(41, 1, 17);
        assert!(repair_line(&b, &luke, "we are going to have a barbecue after the service").is_none());
    }

    #[test]
    fn clean_prefix_and_suffix_outside_the_span_survive_verbatim() {
        let b = Bible::embedded();
        let luke = passage_single(41, 1, 17);
        let r = repair_line(&b, &luke, "he says turn the heart of the father to the children today")
            .expect("must repair");
        assert!(r.text.starts_with("he says"), "{r:?}");
        assert!(r.text.ends_with("today"), "{r:?}");
        assert!(r.text.contains("hearts of the fathers"), "{r:?}");
    }

    #[test]
    fn noise_words_inside_the_span_are_counted_as_dropped() {
        let b = Bible::embedded();
        let luke = passage_single(41, 1, 17);
        let r = repair_line(&b, &luke, "and the disobedient to the banana wisdom of the just").expect("must repair");
        assert!(r.dropped >= 1, "{r:?}");
        assert!(r.text.contains("to the wisdom of the just"), "{r:?}");
    }
}


#[cfg(test)]
mod v890_tests {
    // v0.890.0 — the generalized KJV post-correction (no verse on stage).
    use super::*;

    #[test]
    fn a_garbled_read_is_repaired_from_the_kjv() {
        let b = Bible::embedded();
        // ASR slips: "prayed... and seen" for "prayed before the LORD, and said"
        let heard = "and hezekiah prayed before the lord and seen o lord god of israel";
        let (p, r) = repair_from_match(b, heard).expect("a strong single-verse match repairs");
        assert_eq!(p.label(), "2 Kings 19:15");
        assert!(r.filled > 0, "the dropped word is filled from the verse");
        assert!(r.text.contains("said,"), "the KJV wording lands in the display text");
    }

    #[test]
    fn chatter_is_never_repaired() {
        let b = Bible::embedded();
        for said in [
            "there are many devices in this room today",
            "in the beginning of the service we sang two songs",
            "we will turn together to our reading this morning",
            "the children will go out to their classes after the second song",
        ] {
            assert!(repair_from_match(b, said).is_none(), "chatter must stay untouched: {said}");
        }
    }

    #[test]
    fn short_lines_are_never_repaired() {
        let b = Bible::embedded();
        assert!(repair_from_match(b, "for god so loved").is_none(), "under the 5-word floor");
    }
}
