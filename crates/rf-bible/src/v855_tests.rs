//! v0.855.0 regression tests — the two operator screenshots plus the new
//! matching machinery they needed (span alignment, plural fold, tail gate,
//! OT/NT name aliases). If any of these fails, a screenshot regression is
//! back on stage.

use super::*;

#[test]
fn screenshot_1_named_verse_beats_look_alike_in_the_ranked_list() {
    let b = Bible::embedded();
    // what the preacher actually said mid-verse (singular "father")
    let hits = b.match_quotations_ranked("turn the heart of the father to the children", 8);
    let labels: Vec<String> = hits
        .iter()
        .map(|h| format!("{} {}:{}", b.book_name(h.book), h.chapter, h.verse))
        .collect();
    // the verse the operator NAMED must be among the candidates at all
    // (it used to be missing entirely — Malachi 4:6 carded alone)
    assert!(
        labels.iter().any(|l| l == "Luke 1:17"),
        "Luke 1:17 must be offered for its own first half: {labels:?}"
    );
    // and the KJV wording still wins outright
    let hits = b.match_quotations_ranked("turn the hearts of the fathers to the children", 8);
    assert_eq!(b.book_name(hits[0].book), "Luke");
    assert_eq!(hits[0].chapter, 1);
    assert_eq!(hits[0].verse, 17);
}

#[test]
fn screenshot_1_tail_gate_silences_generic_short_enders() {
    let b = Bible::embedded();
    // "also before him" — the last three words of Acts 23:33 — used to card
    // (and with auto-approve on, TAKE THE STAGE at 79%).
    let hits = b.match_quotations_ranked("also before him", 8);
    assert!(hits.is_empty(), "generic 3-word verse-tail must stay silent: {hits:?}");
    // the same class of claim on another verse tail
    let hits = b.match_quotations_ranked("but have eternal life", 8);
    assert!(hits.is_empty(), "{said:?} tail must stay silent, got {hits:?}", said = "but have eternal life");
    // "in the morning" genuinely OPENS four verses (Deut 28:67, Josh 7:14,
    // …) so it still cardS — but never as an unopposed single candidate:
    // the operator always sees the alternatives, auto-approve cannot fire.
    let hits = b.match_quotations_ranked("in the morning", 8);
    assert!(hits.len() > 1, "opening claims must come with alternatives");
    // distinctive short openings keep working (the v0.853 guarantees)
    let hits = b.match_quotations_ranked("behold i come", 8);
    assert!(
        hits.iter().any(|h| b.book_name(h.book) == "Revelation" && h.chapter == 22 && h.verse == 12),
        "behold I come must still reach Revelation 22:12: {hits:?}"
    );
}

#[test]
fn screenshot_1_span_fit_prefers_the_real_verse_and_its_own_span() {
    let b = Bible::embedded();
    let said = "and the disobedient to the gods";
    // fits Luke 1:17 mid-verse, including the garbled tail
    let a = b.span_align(said, 41, 1, 17).expect("alignment");
    assert!(a.pct >= 60, "Luke 1:17 span fit too low: {}", a.pct);
    // a line that is NOT the verse scores far below the lock floor
    let a2 = b.span_align("we are going to have a barbecue after the service", 41, 1, 17).unwrap_or(SpanAlign { pct: 0, pairs: vec![] });
    assert!(a2.pct < 50, "chatter must not fit Luke 1:17: {}", a2.pct);
}

#[test]
fn ot_nt_name_aliases_match_across_testaments() {
    let b = Bible::embedded();
    // spoken "Elijah" must reach the verse that reads "Elias" (Luke 1:17)
    let hits = b.match_quotations_ranked("in the spirit and power of elijah", 8);
    assert!(
        hits.iter().any(|h| b.book_name(h.book) == "Luke" && h.chapter == 1 && h.verse == 17),
        "Elijah must find Elias in Luke 1:17: {:?}",
        hits.iter().map(|h| format!("{} {}:{}", b.book_name(h.book), h.chapter, h.verse)).collect::<Vec<_>>()
    );
    // and the OT verse reading "Elijah" is found when the engine writes "Elias"
    let hits = b.match_quotations_ranked("behold I will send you elias the prophet", 8);
    assert!(
        hits.iter().any(|h| b.book_name(h.book) == "Malachi" && h.chapter == 4 && h.verse == 5),
        "Elias must find Elijah in Malachi 4:5: {:?}",
        hits.iter().map(|h| format!("{} {}:{}", b.book_name(h.book), h.chapter, h.verse)).collect::<Vec<_>>()
    );
}

#[test]
fn plural_folded_needles_reach_both_directions() {
    let b = Bible::embedded();
    // heard plural, verse singular (Malachi 4:6 reads "turn the heart")
    let hits = b.match_quotations_ranked("and he shall turn the hearts of the fathers", 8);
    assert!(
        hits.iter().any(|h| b.book_name(h.book) == "Malachi" && h.chapter == 4 && h.verse == 6),
        "plural probe must find Malachi's singular: {:?}",
        hits.iter().map(|h| format!("{} {}:{}", b.book_name(h.book), h.chapter, h.verse)).collect::<Vec<_>>()
    );
}
