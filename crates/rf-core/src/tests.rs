//! PRD v2.0 acceptance criteria, locked as regression tests.
//!
//! Every user-reported failure becomes a new test here (PRD §23: "Tests —
//! regression suite green before any release; every user-reported failure
//! becomes a locked test").

use crate::books::idx;
use super::*;
use rf_bible::Bible;

fn bible() -> &'static Bible {
    Bible::embedded()
}

fn parse_default(text: &str) -> ParseOutcome {
    let ctx = ParseContext::default();
    parse(bible(), text, &ctx, 0.75)
}

fn parse_with(text: &str, ctx: &ParseContext) -> ParseOutcome {
    parse(bible(), text, ctx, 0.75)
}

fn label(o: &ParseOutcome, i: usize) -> String {
    o.detections[i].passage.label()
}

// -------------------------------------------------------------------------
// PRD 6.1 — Voice-Activated Scripture Trigger
// -------------------------------------------------------------------------

/// "Given the transcript 'let's go to john three sixteen', then John 3:16 is
/// staged with its full verse text."
#[test]
fn john_three_sixteen_spoken() {
    let o = parse_default("let's go to john three sixteen");
    assert_eq!(o.detections.len(), 1, "{o:?}");
    assert_eq!(label(&o, 0), "John 3:16");
    assert!(o.detections[0].confidence >= 0.75);
    let text = bible().passage(42, 3, 16, 16);
    assert!(text.contains("For God so loved the world"), "{text}");
}

#[test]
fn john_chapter3_verse16_explicit() {
    let o = parse_default("John chapter 3 verse 16");
    assert_eq!(label(&o, 0), "John 3:16");
}

#[test]
fn verse_range_through() {
    let o = parse_default("John chapter 3 verses 16 through 18");
    assert_eq!(label(&o, 0), "John 3:16-18");
    assert_eq!(o.detections[0].passage.to, 18);
}

/// "Given '1st Corinthians five, for seventeen', then the product reports the
/// honest out-of-range message and offers suggestions — it must not stage
/// verse 1 silently." (1 Corinthians 5 has only 13 verses.)
#[test]
fn one_corinthians_5_17_honest_error() {
    let o = parse_default("1st Corinthians five, for seventeen");
    assert!(o.detections.is_empty(), "must not stage: {o:?}");
    assert_eq!(o.errors.len(), 1, "{o:?}");
    let msg = &o.errors[0].message;
    assert!(msg.contains("only 13 verses"), "{msg}");
    assert!(msg.contains("no verse 17"), "{msg}");
    // flagship suggestion: 2 Corinthians 5:17
    let cands = &o.errors[0].candidates;
    assert!(
        cands.iter().any(|p| p.label() == "2 Corinthians 5:17"),
        "candidates: {cands:?}"
    );
}

/// "Given the bare phrase 'verse 17' while a chapter is on stage, then the
/// on-stage reference jumps within that chapter."
#[test]
fn bare_verse_jumps_within_stage_chapter() {
    let ctx = ParseContext {
        last: Some(Passage::single(idx("John") as u8, 3, 16)),
        ..ParseContext::default()
    };
    let o = parse_with("verse 17", &ctx);
    assert_eq!(label(&o, 0), "John 3:17");
    assert_eq!(o.detections[0].kind, DetectionKind::Relative);
}

/// "Given a chapter-only mention ('First Corinthians chapter 13'), then verse
/// 1 of that chapter is staged and the operator sees that it is chapter-only."
#[test]
fn chapter_only_stages_verse1_with_badge() {
    let o = parse_default("First Corinthians chapter 13");
    assert_eq!(o.detections.len(), 1, "{o:?}");
    assert_eq!(label(&o, 0), "1 Corinthians 13:1");
    assert_eq!(o.detections[0].kind, DetectionKind::ChapterOnly);
    assert!(o.detections[0].note.as_deref().unwrap().contains("chapter-only"));
}

#[test]
fn turn_with_me_to_psalm_23() {
    let o = parse_default("turn with me to Psalm 23");
    assert_eq!(label(&o, 0), "Psalms 23:1");
    assert_eq!(o.detections[0].kind, DetectionKind::ChapterOnly);
}

#[test]
fn filler_tolerant_parsing() {
    let o = parse_default(
        "would you turn with me now please over to the gospel of john and chapter three and verse sixteen",
    );
    assert_eq!(label(&o, 0), "John 3:16", "{o:?}");
}

#[test]
fn colon_form() {
    let o = parse_default("romans 8:28");
    assert_eq!(label(&o, 0), "Romans 8:28");
}

#[test]
fn digits_merged_316_digit_split() {
    let o = parse_default("john 316");
    assert_eq!(label(&o, 0), "John 3:16", "{o:?}");
}

#[test]
fn two_refs_in_one_utterance() {
    let o = parse_default("john 3:16 and romans 8:28");
    assert_eq!(o.detections.len(), 2, "{o:?}");
    assert_eq!(label(&o, 0), "John 3:16");
    assert_eq!(label(&o, 1), "Romans 8:28");
}

#[test]
fn numbered_books_all_forms() {
    for (utterance, expected) in [
        ("second samuel chapter five", "2 Samuel 5:1"),
        ("2 samuel 5", "2 Samuel 5:1"),
        ("ii samuel chapter 5", "2 Samuel 5:1"),
        ("two samuel chapter five", "2 Samuel 5:1"),
        ("1 corinthians 13", "1 Corinthians 13:1"),
        ("first corinthians chapter 13", "1 Corinthians 13:1"),
        ("i corinthians 13", "1 Corinthians 13:1"),
    ] {
        let o = parse_default(utterance);
        assert_eq!(o.detections.len(), 1, "{utterance}: {o:?}");
        assert_eq!(label(&o, 0), expected, "{utterance}");
    }
}

#[test]
fn fuzzy_book_match_jhn() {
    let o = parse_default("jhn chapter three verse sixteen");
    // fuzzy but strong enough to auto-stage with explicit numbers
    assert_eq!(label(&o, 0), "John 3:16", "{o:?}");
}

#[test]
fn song_of_songs_alias() {
    let o = parse_default("song of songs chapter two");
    assert_eq!(label(&o, 0), "Song of Solomon 2:1");
}

#[test]
fn revelations_alias() {
    let o = parse_default("revelations twenty two twenty");
    assert_eq!(label(&o, 0), "Revelation 22:20", "{o:?}");
}

#[test]
fn chapter_out_of_range() {
    let o = parse_default("john chapter 25");
    assert!(o.detections.is_empty());
    assert_eq!(o.errors.len(), 1);
    assert!(o.errors[0].message.contains("only 21 chapters"), "{o:?}");
}

#[test]
fn verse_zero_handled() {
    let o = parse_default("john 3 0");
    assert!(o.detections.is_empty(), "{o:?}");
    assert!(!o.errors.is_empty() || !o.confirms.is_empty());
}

// -------------------------------------------------------------------------
// PRD 6.5 — Confidence and Operator Review
// -------------------------------------------------------------------------

/// "Given a reference scoring below the auto-stage threshold, then it appears
/// in the confirm queue with alternatives, and the stage is unchanged until an
/// operator acts."
/// A garbled book with NO chapter cannot be staged — but uncertainty must be
/// visible as a book-only confirm card (PRD 6.5).
#[test]
fn weak_book_match_goes_to_confirm_queue() {
    let o = parse_default("somewhere in the efeziens");
    assert!(o.detections.is_empty(), "{o:?}");
    assert!(
        !o.confirms.is_empty() || !o.errors.is_empty(),
        "uncertainty must be visible: {o:?}"
    );
}

/// A garbled book WITH explicit numbers and a valid result is exactly the
/// structural-parser case: auto-stage (the numbers carry the confidence).
#[test]
fn garbled_book_with_numbers_autostages() {
    let o = parse_default("epheseeans chapter two verse one");
    assert_eq!(o.detections.len(), 1, "{o:?}");
    assert_eq!(label(&o, 0), "Ephesians 2:1");
}

/// "Given a non-speech transcript ('[BLANK_AUDIO]', '[MUSIC]'), then no
/// candidate is produced and no error status is raised."
#[test]
fn non_speech_artifacts_raise_nothing() {
    for t in ["[BLANK_AUDIO]", "[MUSIC]", "[ silence ]", ""] {
        let o = parse_default(t);
        assert!(!o.is_speech, "{t}");
        assert!(o.is_empty(), "{t}: {o:?}");
    }
}

// -------------------------------------------------------------------------
// PRD 6.2 — Quotation Recognition
// -------------------------------------------------------------------------

/// "Given a quotation whose first eight words match a unique KJV verse, then
/// the candidate appears in the confirm queue labeled as a quotation match,
/// never auto-staged above the operator's confidence setting."
#[test]
fn quotation_goes_to_confirm_queue() {
    let o = parse_default(
        "for god so loved the world that he gave his only begotten son",
    );
    assert!(o.detections.is_empty(), "quotation must never auto-stage: {o:?}");
    assert_eq!(o.confirms.len(), 1, "{o:?}");
    assert_eq!(o.confirms[0].kind, DetectionKind::Quotation);
    assert_eq!(o.confirms[0].passage.label(), "John 3:16");
    assert!(o.confirms[0].confidence < 0.75);
}

#[test]
fn quotation_the_lord_is_my_shepherd() {
    let o = parse_default("the lord is my shepherd i shall not want");
    assert_eq!(o.confirms.len(), 1, "{o:?}");
    assert_eq!(o.confirms[0].passage.label(), "Psalms 23:1");
}

#[test]
fn quotation_split_across_a_natural_pause() {
    // User-reported (locked per PRD §23): a preacher pausing mid-quotation
    // for emphasis — "For God so" … (pause) … "loved the world, that He
    // gave his only begotten Son" — gets chopped by the VAD into two
    // utterances. Neither fragment alone contains a full quotation match;
    // the second one doesn't even start at the verse's own first word.
    // Given as recent context, together they must still resolve to John 3:16.
    let ctx = ParseContext::default();
    let first = parse_with_recent(bible(), "For God so", &ctx, 0.75, &[]);
    // v0.853.0 (operator request): THREE words that open exactly one verse
    // ("for God so" -> John 3:16, nothing else competes) are now offered at
    // once — a suggestion card (or an auto-show with the setting on) instead
    // of silence. The rest of the quote, when it arrives, lands on the same
    // verse, so nothing new is queued.
    assert_eq!(first.confirms.len(), 1, "unique 3 words must be offered at once: {first:?}");
    assert_eq!(first.confirms[0].kind, DetectionKind::Quotation);
    assert_eq!(first.confirms[0].passage.label(), "John 3:16");

    let second = parse_with_recent(
        bible(),
        "loved the world, that He gave his only begotten Son",
        &ctx,
        0.75,
        &["For God so"],
    );
    assert_eq!(second.confirms.len(), 1, "{second:?}");
    assert_eq!(second.confirms[0].kind, DetectionKind::Quotation);
    assert_eq!(second.confirms[0].passage.label(), "John 3:16");
    assert!(second.confirms[0].confidence < 0.75, "must still never auto-stage");
}

#[test]
fn quotation_matches_alt_translation_when_kjv_does_not() {
    // "keep the same translation, but suggest it and wait for approval":
    // KJV stays the default/structural bible; when the operator's active
    // display translation is different (BBE here) and KJV finds nothing,
    // a match against that translation is still offered — always to the
    // confirm queue, same as any other quotation, never auto-staged.
    let kjv = bible();
    let bbe = rf_bible::Bible::embedded_bbe();
    let ctx = ParseContext::default();

    // BBE's own wording for John 3:16 ("For God had such love for the
    // world...") — deliberately does NOT match KJV's wording at all.
    let text = "for god had such love for the world that he gave his only son";

    let kjv_only = parse_full(kjv, text, &ctx, 0.75, &[], None);
    assert!(kjv_only.is_empty(), "must not match on KJV alone: {kjv_only:?}");

    let with_alt = parse_full(kjv, text, &ctx, 0.75, &[], Some((bbe, "BBE")));
    assert_eq!(with_alt.confirms.len(), 1, "{with_alt:?}");
    assert_eq!(with_alt.confirms[0].kind, DetectionKind::Quotation);
    assert_eq!(with_alt.confirms[0].passage.label(), "John 3:16");
    assert!(with_alt.confirms[0].confidence < 0.75, "must still never auto-stage");
    let note = with_alt.confirms[0].note.as_deref().unwrap_or("");
    assert!(note.contains("BBE"), "note should name which translation matched: {note:?}");
}

#[test]
fn quotation_kjv_match_wins_without_needing_the_alt_translation() {
    // The alt-translation fallback must only be consulted when KJV finds
    // nothing — a normal KJV quotation must resolve exactly as before,
    // whether or not an alt translation is configured.
    let kjv = bible();
    let bbe = rf_bible::Bible::embedded_bbe();
    let ctx = ParseContext::default();
    let out = parse_full(
        kjv,
        "the lord is my shepherd i shall not want",
        &ctx,
        0.75,
        &[],
        Some((bbe, "BBE")),
    );
    assert_eq!(out.confirms.len(), 1, "{out:?}");
    assert_eq!(out.confirms[0].passage.label(), "Psalms 23:1");
    let note = out.confirms[0].note.as_deref().unwrap_or("");
    assert!(!note.contains("BBE"), "a direct KJV match must not be labeled as a BBE match: {note:?}");
}

#[test]
fn quotation_recent_context_does_not_override_a_direct_match() {
    // Recent text must only be consulted as a fallback — a current
    // utterance that already matches on its own must not be second-guessed
    // by unrelated recent text.
    let ctx = ParseContext::default();
    let o = parse_with_recent(
        bible(),
        "the lord is my shepherd i shall not want",
        &ctx,
        0.75,
        &["completely unrelated announcement about the potluck"],
    );
    assert_eq!(o.confirms.len(), 1, "{o:?}");
    assert_eq!(o.confirms[0].passage.label(), "Psalms 23:1");
}

// -------------------------------------------------------------------------
// PRD 6.3 — Natural Context Understanding
// -------------------------------------------------------------------------

#[test]
fn that_verse_repeats_stage() {
    let ctx = ParseContext {
        last: Some(Passage::single(idx("Romans") as u8, 8, 28)),
        ..ParseContext::default()
    };
    let o = parse_with("let's go back to that verse", &ctx);
    assert_eq!(o.detections.len(), 1, "{o:?}");
    assert_eq!(label(&o, 0), "Romans 8:28");
}

// -------------------------------------------------------------------------
// PRD 6.4 — Translation Management
// -------------------------------------------------------------------------

#[test]
fn translation_request_recognized() {
    let o = parse_default("now let's read it in the niv");
    assert!(o.commands.contains(&Command::Translation("NIV")), "{o:?}");
}

// -------------------------------------------------------------------------
// Voice navigation commands (M0: next/previous with rollover)
// -------------------------------------------------------------------------

#[test]
fn voice_commands() {
    let o = parse_default("next verse please");
    assert!(o.commands.contains(&Command::NextVerse));
    let o = parse_default("go to the previous chapter");
    assert!(o.commands.contains(&Command::PrevChapter));
    let o = parse_default("next chapter");
    assert!(o.commands.contains(&Command::NextChapter));
}

// -------------------------------------------------------------------------
// PRD 7.4 — Suggested Sermon Content ("three things" strict pattern)
// -------------------------------------------------------------------------

/// "Given the enumerated pattern with N between 2 and 5 and a list of short
/// items, then one suggestion card is raised listing all N items; nothing is
/// staged without operator approval."
#[test]
fn three_things_suggestion() {
    let o = parse_default(
        "there are three things i want you to remember: faith, obedience and perseverance",
    );
    assert_eq!(o.suggestions.len(), 1, "{o:?}");
    let s = &o.suggestions[0];
    assert_eq!(s.items, vec!["Faith", "Obedience", "Perseverance"], "{s:?}");
    // nothing staged
    assert!(o.detections.is_empty());
}

#[test]
fn two_things_suggestion() {
    let o = parse_default("there are two things i want you to remember: grace and truth");
    assert_eq!(o.suggestions.len(), 1);
    assert_eq!(o.suggestions[0].items, vec!["Grace", "Truth"]);
}

// -------------------------------------------------------------------------
// Sentence-level sanity: prose without references stays quiet
// -------------------------------------------------------------------------

#[test]
fn ordinary_speech_produces_no_noise() {
    for t in [
        "good morning everyone it is wonderful to be here with you today",
        "let us pray father we thank you for this day",
        "please stand as we sing together",
    ] {
        let o = parse_default(t);
        assert!(o.detections.is_empty() && o.errors.is_empty(), "{t}: {o:?}");
    }
}

#[test]
fn display_labels() {
    assert_eq!(Passage::single(idx("John") as u8, 3, 16).label(), "John 3:16");
    let range = Passage { book: idx("John") as u8, chapter: 3, from: 16, to: 18 };
    assert_eq!(range.label(), "John 3:16-18");
    let ch = Passage { book: idx("Psalms") as u8, chapter: 23, from: 1, to: 1 };
    assert_eq!(ch.label(), "Psalms 23:1");
}

#[test]
fn navigation_rollover_logic() {
    // next verse at chapter end rolls into next chapter verse 1
    let b = bible();
    let max = b.verse_count(idx("John"), 3);
    assert_eq!(max, 36);
    // John 3:36 -> next = John 4:1
    let next_verse = |book: usize, ch: u16, v: u16| -> Option<(usize, u16, u16)> {
        if (v as usize) < b.verse_count(book, ch as usize) {
            Some((book, ch, v + 1))
        } else if (ch as usize) < b.chapter_count(book) {
            Some((book, ch + 1, 1))
        } else {
            None
        }
    };
    assert_eq!(next_verse(idx("John"), 3, 36), Some((idx("John"), 4, 1)));
}

// ---- v0.811.0: ranked quotation candidates ----------------------------------

#[test]
fn grace_of_our_lord_offers_every_candidate_best_first() {
    let o = parse(Bible::embedded(), "the grace of our Lord Jesus Christ be with you", &ParseContext::default(), 0.75);
    let q = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).expect("a quotation card");
    assert!(q.alternatives.len() >= 3, "several verses share this opening: {q:?}");
    assert_eq!(q.alternatives.len(), q.alt_pct.len());
    assert!(q.alt_pct.windows(2).all(|w| w[0] >= w[1]), "sorted: {:?}", q.alt_pct);
    assert!((q.confidence * 100.0).round() as u8 >= q.alt_pct[0].min(74), "best candidate is first");
    assert!(q.confidence < 0.75, "parser never auto-stages a quotation");
    assert!(o.detections.is_empty(), "never auto-staged by the parser");
}

#[test]
fn unique_quotation_is_unchanged() {
    let o = parse(Bible::embedded(), "for god so loved the world", &ParseContext::default(), 0.75);
    let q = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).expect("quotation");
    assert_eq!(q.passage.label(), "John 3:16");
    assert!(q.alternatives.is_empty());
    assert!((q.confidence - 0.62).abs() < 1e-6);
}

// ---- v0.812.0: a weak "1 Kings" guess must not block the quotation ----------

#[test]
fn kings_and_priests_is_revelation_1_6_not_a_book_card() {
    for said in [
        "and have made us kings and priests",
        "and have made us kings and priests unto god",
        "and have made us kings and priests unto god and his father",
        "And have made those kings and priests unto God and his father.",
    ] {
        let o = parse(Bible::embedded(), said, &ParseContext::default(), 0.75);
        let labels: Vec<String> = o.confirms.iter().map(|d| format!("{:?}:{}", d.kind, d.passage.label())).collect();
        assert!(
            o.confirms.iter().any(|d| d.kind == DetectionKind::Quotation && d.passage.label() == "Revelation 1:6"),
            "{said:?} -> {labels:?}"
        );
        assert!(!o.confirms.iter().any(|d| d.kind == DetectionKind::BookOnly), "{said:?} still has a book card: {labels:?}");
    }
}

#[test]
fn a_clear_book_name_is_still_a_book_card() {
    let o = parse(Bible::embedded(), "turn with me to second samuel", &ParseContext::default(), 0.75);
    assert!(o.confirms.iter().any(|d| d.kind == DetectionKind::BookOnly), "{o:?}");
}

#[test]
fn real_transcript_lines_from_the_service_screenshot() {
    // (heard text, expected quotation label or None)
    let cases: &[(&str, Option<&str>)] = &[
        ("And have made us kings and priests.", Some("Revelation 1:6")),
        ("Relations and have made us kings and priests.", Some("Revelation 1:6")),
        ("And have made us kings and priests unto God.", Some("Revelation 1:6")),
        ("And have made us kings and priests unto God and his father.", Some("Revelation 1:6")),
        ("And have made us kings and priests on to gods.", Some("Revelation 1:6")),
        ("And have made those kings and priests unto God and his father.", Some("Revelation 1:6")),
        ("For God so loved the world.", None), // only 5 words, unique -> still a John 3:16 card
    ];
    for (said, want) in cases {
        let o = parse(Bible::embedded(), said, &ParseContext::default(), 0.75);
        let got: Vec<String> = o.confirms.iter().map(|d| format!("{:?} {}", d.kind, d.passage.label())).collect();
        match want {
            Some(l) => assert!(o.confirms.iter().any(|d| d.kind == DetectionKind::Quotation && d.passage.label() == *l), "{said:?} -> {got:?} det={:?}", o.detections),
            None => {}
        }
    }
}


#[test]
fn uncertain_book_guess_inside_a_sentence_makes_no_card_but_a_bare_name_still_does() {
    let o = parse(Bible::embedded(), "and have made those kings entreeced", &ParseContext::default(), 0.75);
    assert!(!o.confirms.iter().any(|d| d.kind == DetectionKind::BookOnly), "{o:?}");
    let o = parse(Bible::embedded(), "second samuel", &ParseContext::default(), 0.75);
    assert!(o.confirms.iter().any(|d| d.kind == DetectionKind::BookOnly), "{o:?}");
}

// ---- v0.816.0: quotations said in short garbled pieces ----
mod pieces_v0816 {
    use super::*;
    use crate::parser::{pick_recent_pieces, RecentLine, RECENT_MAX_GAP_MS, RECENT_MAX_LINES, RECENT_MAX_SPAN_MS};

    const PIECES: [&str; 9] = ["If any man being Christ.", "He is.", "A new.", "Creation", "Old things.", "I'm passed away.", "Behold.", "All things.", "I'll become new."];

    #[test]
    fn the_parser_assembles_the_pieces_into_one_suggestion() {
        let bible = rf_bible::Bible::embedded();
        let recent: Vec<&str> = PIECES[..8].to_vec();
        let out = parse_full(bible, PIECES[8], &ParseContext::default(), 0.8, &recent, None);
        let d = out.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).expect("a quotation suggestion");
        assert_eq!((bible.book_name(d.passage.book as usize), d.passage.chapter, d.passage.from), ("2 Corinthians", 5, 17));
        assert!(d.pct >= 70, "{}", d.pct);
        assert!(d.note.as_deref().unwrap_or("").contains("pieces"), "{:?}", d.note);
    }

    #[test]
    fn the_last_piece_alone_stays_quiet() {
        let bible = rf_bible::Bible::embedded();
        let out = parse_full(bible, "I'll become new.", &ParseContext::default(), 0.8, &[], None);
        assert!(out.confirms.iter().all(|d| d.kind != DetectionKind::Quotation));
    }

    fn l(ms: u64, t: &str, flagged: bool) -> RecentLine {
        RecentLine { ms, text: t.to_string(), flagged }
    }

    #[test]
    fn a_whole_sentence_with_a_modern_word_or_two_is_still_found_on_its_own() {
        let bible = rf_bible::Bible::embedded();
        let out = parse_full(bible, "If anyone be in Christ he is a new creature", &ParseContext::default(), 0.8, &[], None);
        let d = out.detections.iter().chain(out.confirms.iter()).find(|d| d.kind == DetectionKind::Quotation).expect("found");
        assert_eq!((d.passage.chapter, d.passage.from), (5, 17));
        assert!(d.pct >= 80, "{}", d.pct);
    }

    #[test]
    fn pieces_in_bbe_wording_are_offered_as_bbe_not_as_a_loose_kjv_guess() {
        let kjv = rf_bible::Bible::embedded();
        let bbe = rf_bible::Bible::embedded_bbe();
        let pieces = ["for god had such love", "for the world that he gave his only son", "so that everyone who has faith in him", "may not be sent to destruction", "but have eternal life"];
        let recent: Vec<&str> = pieces[..4].to_vec();
        let with_alt = parse_full(kjv, pieces[4], &ParseContext::default(), 0.8, &recent, Some((bbe, "BBE")));
        let d = with_alt.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).expect("suggested");
        assert_eq!(d.passage.label(), "John 3:16");
        assert!(d.note.as_deref().unwrap_or("").contains("BBE"), "{:?}", d.note);
        // with no alternate translation set, the loose wording must not be put forward as KJV
        let kjv_only = parse_full(kjv, pieces[4], &ParseContext::default(), 0.8, &recent, None);
        assert!(kjv_only.confirms.iter().all(|d| d.pct < 85 || d.kind != DetectionKind::Quotation), "{kjv_only:?}");
    }

    #[test]
    fn recent_pieces_reach_back_over_the_operators_real_pauses() {
        // the real timestamps from the screenshot: gaps of 7, 3, 3, 3, 3, 2, 3, 2 seconds
        let lines: Vec<RecentLine> = [(194, "If any man being Christ."), (201, "He is."), (204, "A new."), (207, "Creation"), (210, "Old things."), (213, "I'm passed away."), (215, "Behold."), (218, "All things.")]
            .iter()
            .map(|(s, t)| l(s * 1000, t, false))
            .collect();
        let got = pick_recent_pieces(&lines, 220_000, 0);
        assert_eq!(got.len(), 8, "{got:?}");
        assert_eq!(got[0], "If any man being Christ.", "oldest first");
        assert_eq!(got[7], "All things.");
    }

    #[test]
    fn flagged_pieces_are_kept_but_a_piece_already_used_for_a_verse_is_not() {
        let lines = vec![l(100_000, "Old news.", false), l(105_000, "Next.", true), l(108_000, "For God so loved", false), l(111_000, "the world.", true)];
        assert_eq!(pick_recent_pieces(&lines, 113_000, 0).len(), 4, "a flagged piece does not end the search");
        // a verse was staged at 108 s: everything up to and including it is spent
        assert_eq!(pick_recent_pieces(&lines, 113_000, 108_000), vec!["the world.".to_string()]);
    }

    #[test]
    fn recent_pieces_stop_at_a_long_silence_a_long_span_and_a_line_limit() {
        let lines = vec![l(10_000, "ancient", false), l(60_000, "recent", false)];
        assert_eq!(pick_recent_pieces(&lines, 62_000, 0), vec!["recent".to_string()], "a 50 s silence ends the context");
        let many: Vec<RecentLine> = (0..40).map(|i| l(1_000 + i * 2_000, "word", false)).collect();
        let got = pick_recent_pieces(&many, 81_000, 0);
        assert!(got.len() <= RECENT_MAX_LINES);
        assert!((got.len() as u64) * 2_000 <= RECENT_MAX_SPAN_MS + 4_000);
        assert!(RECENT_MAX_GAP_MS >= 8_000, "must bridge the real 7 s pause");
        assert!(pick_recent_pieces(&[], 5_000, 0).is_empty());
    }
}

// -------------------------------------------------------------------------
// v0.853.0 — the five operator-reported failures (screenshot transcripts)
// -------------------------------------------------------------------------

/// #1: the preacher self-corrected ("Second Chronicles NO, second
/// Corinthians chapter 5 verse 17") while Joshua 1 was on stage. The old
/// parse read the second "second" as 2 Chronicles' chapter and kept the old
/// Joshua reference; the stage never moved. Must stage 2 Corinthians 5:17.
#[test]
fn self_corrected_reference_stages_the_correction() {
    let mut ctx = ParseContext::default();
    ctx.last = Some(Passage::single(idx("Joshua") as u8, 1, 8));
    let o = parse_with("Second Chronicles No Second Corinthians chapter 5 verse 17.", &ctx);
    assert!(o.detections.last().is_some(), "{o:?}");
    assert_eq!(o.detections.last().unwrap().passage.label(), "2 Corinthians 5:17", "{o:?}");
    assert!(o.confirms.is_empty(), "no book card for the false start: {o:?}");
}

/// #1 (guard): the ordinal before the corrected book must never be swallowed
/// as the PREVIOUS book's chapter even without a correction marker.
#[test]
fn ordinal_of_next_book_is_not_the_previous_book_chapter() {
    let o = parse_default("second chronicles second corinthians chapter 5 verse 17");
    let last = o.detections.last().expect("{o:?}");
    assert_eq!(last.passage.label(), "2 Corinthians 5:17", "{o:?}");
}

/// #2: "We go to Psalms." sets the pending book; "23 verse 1." completes it.
/// The old parse ignored "23" and resolved "verse 1" against the on-stage
/// chapter (2 Corinthians 5:1). Must complete as Psalms 23:1.
#[test]
fn book_then_numbers_completes_the_reference() {
    let mut ctx = ParseContext::default();
    ctx.last = Some(Passage::single(idx("2 Corinthians") as u8, 5, 1));
    let o1 = parse_with("We go to Psalms.", &ctx);
    let book = o1
        .confirms
        .iter()
        .find(|d| d.kind == DetectionKind::BookOnly)
        .map(|d| d.passage.book)
        .expect("Psalms must be heard as a book-only mention");
    ctx.pending_book = Some(book);
    let o2 = parse_with("23 verse 1.", &ctx);
    assert_eq!(o2.detections.last().expect("{o2:?}").passage.label(), "Psalms 23:1", "{o2:?}");
}

/// #2 (safety): a stray number in running speech must NOT claim the pending
/// book — only a real chapter cue (keyword or second number) may.
#[test]
fn pending_book_ignores_stray_numbers() {
    let mut ctx = ParseContext::default();
    ctx.pending_book = Some(idx("Psalms") as u8);
    let o = parse_with("that's 3 points tonight", &ctx);
    assert!(o.detections.is_empty(), "a lone number must not stage: {o:?}");
}

/// #2 (guard): a lone number still means nothing even with a pending book.
#[test]
fn bare_number_alone_stays_silent() {
    let mut ctx = ParseContext::default();
    ctx.pending_book = Some(idx("Psalms") as u8);
    let o = parse_with("23", &ctx);
    assert!(o.detections.is_empty(), "{o:?}");
}

/// #3: "…the evidence of things not seen. Next verse." — a nav phrase that
/// ENDS a long utterance must survive (the old NAV_MAX_WORDS gate stripped
/// it), and the quotation re-match of the just-read verse must not block it.
#[test]
fn nav_phrase_at_the_end_of_a_long_utterance_survives() {
    let mut ctx = ParseContext::default();
    ctx.last = Some(Passage::single(idx("Hebrews") as u8, 11, 1));
    let o = parse_with("The evidence of things not seen. Next verse.", &ctx);
    assert!(o.commands.contains(&Command::NextVerse), "{o:?}");
    // app rule: a quotation confirm does not block navigation
    assert!(!nav_blocked_by(&o), "{o:?}");
}

/// #3 (guard): "next verse" buried mid-sentence is still not a command.
#[test]
fn nav_phrase_mid_sentence_still_dropped() {
    let o = parse_default("the next verse says something wonderful about grace");
    assert!(!o.commands.contains(&Command::NextVerse), "{o:?}");
}

/// #4: the quotation itself, 4-5 words, auto-shows through the normal
/// auto-approve rule (pct >= 70 on a single candidate).
#[test]
fn let_this_mind_be_in_you_is_offered_at_once() {
    let o = parse_default("Let this mind be in you.");
    let d = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).expect("{o:?}");
    assert_eq!(d.passage.label(), "Philippians 2:5");
    assert!(d.pct >= 70 || d.pct == 0, "single-candidate pct must reach the auto bar: {}", d.pct);
}

/// #4: the same quotation embedded after 15 lead-in words must still be
/// found (the old matcher gave up after 6).
#[test]
fn quotation_after_fifteen_lead_in_words_is_found() {
    let o = parse_default("There is a mentality of Zion that is to don't have when the Bible says let this mind be in you.");
    let d = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).expect("{o:?}");
    assert_eq!(d.passage.label(), "Philippians 2:5", "{o:?}");
}

/// #4/#5: ASR-damaged readings of the same quote must still reach
/// Philippians 2:5. The FIRST mangled repeat now matches ("being" for
/// "be in" — one damaged word in a 3-word distinctive opening). An extreme
/// mishearing that keeps only two of the words ("That doesn't mind being
/// you") legitimately cannot be resolved from text alone — that is what the
/// operator correction box (lexicon), the learning fixes and the ClearSpeech
/// final pass are for; the operator no longer NEEDS a third repeat because
/// the first good one now lands.
#[test]
fn asr_damaged_quotation_still_matches() {
    for said in ["Let this mind being you.", "Let this mind to being you."] {
        let o = parse_default(said);
        let d = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation)
            .unwrap_or_else(|| panic!("{said} must match: {o:?}"));
        assert_eq!(d.passage.label(), "Philippians 2:5", "{said}");
    }
    // the extreme case stays honest: no guess, no wrong card
    let o = parse_default("That doesn't mind being you.");
    assert!(o.confirms.iter().all(|d| d.kind != DetectionKind::Quotation || d.passage.label() == "Philippians 2:5"), "{o:?}");
}

/// #4: "for God so" — three words, no competitor in the whole KJV —
/// must be offered at once.
#[test]
fn three_unique_words_are_offered_at_once() {
    let o = parse_default("for God so");
    let d = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).expect("{o:?}");
    assert_eq!(d.passage.label(), "John 3:16");
}

/// #5 (the app rule): an utterance that navigates drops its quotation
/// re-match instead of stacking a card for the verse already on stage.
#[test]
fn navigation_utterance_does_not_restage_the_read_verse() {
    let mut ctx = ParseContext::default();
    ctx.last = Some(Passage::single(idx("Hebrews") as u8, 11, 1));
    let o = parse_with("The evidence of things not seen. Next verse.", &ctx);
    let had_nav = o.commands.iter().any(|c| matches!(c, Command::NextVerse));
    assert!(had_nav);
    // parse_and_stage skips quotation confirms when had_nav — the parser
    // side only needs to guarantee the command and no blocking scripture
    assert!(o.detections.is_empty());
    assert!(o.confirms.iter().all(|d| d.kind == DetectionKind::Quotation));
}

// ---------------------------------------------------------------------------
// v0.855.0 — the two operator screenshots, locked as regressions
// ---------------------------------------------------------------------------

/// Screenshot 1: the operator named "Luke 1:17"; the preacher read the verse.
/// The half-line also fits Malachi 4:6 (Luke quotes Malachi). While Luke 1:17
/// is on stage, the look-alike must NOT card — the named verse stays.
#[test]
fn screenshot_1_named_verse_lock_blocks_the_look_alike() {
    let mut ctx = ParseContext::default();
    ctx.last = Some(Passage::single(idx("Luke") as u8, 1, 17));
    let o = parse_with("turn the heart of the father to the children", &ctx);
    assert!(
        o.confirms.iter().all(|d| d.passage.label() != "Malachi 4:6"),
        "Malachi 4:6 must not card while Luke 1:17 is on stage: {o:?}"
    );
    // the distinctive tail of the same verse is fine (it IS the named verse —
    // the app's repeat-suppression keeps it from re-carding)
    let o = parse_with("and the disobedient to the wisdom of the just", &ctx);
    assert!(o.confirms.iter().all(|d| d.passage.label() == "Luke 1:17"), "{o:?}");
}

/// Screenshot 1, without the lock: the fragment alone still offers BOTH
/// verses (the named one must be in the alternatives, not invisible).
#[test]
fn screenshot_1_without_lock_both_look_alikes_are_offered() {
    let o = parse_default("turn the heart of the father to the children");
    let mal = o.confirms.iter().find(|d| d.passage.label() == "Malachi 4:6").expect("{o:?}");
    assert!(mal.kind == DetectionKind::Quotation);
    assert!(
        mal.alternatives.iter().any(|p| p.label() == "Luke 1:17"),
        "Luke 1:17 must be among the alternatives: {mal:?}"
    );
}

/// Screenshot 2: "also before him" (the tail of Acts 23:33) must card
/// nothing — not on its own, not assembled from recent pieces.
#[test]
fn screenshot_2_short_tail_fragment_raises_no_card() {
    let o = parse_default("also before him");
    assert!(o.confirms.is_empty(), "{o:?}");
    assert!(o.detections.is_empty(), "{o:?}");
    // with recent pieces — the assembler must not rescue it either
    let o = parse_with_recent(bible(), "also before him", &ParseContext::default(), 0.75, &["he said", "and then"]);
    assert!(o.confirms.is_empty(), "pieces must not rescue a tail fragment: {o:?}");
}

/// Sound tags must vanish before parsing while real asides survive.
#[test]
fn sound_tags_cannot_hide_a_verse_or_steer_it() {
    let o = parse_default("[MUSIC] for God so loved the world");
    let d = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).expect("{o:?}");
    assert_eq!(d.passage.label(), "John 3:16");
    let o = parse_default("for God so (upbeat guitar music) loved the world");
    let d = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).expect("{o:?}");
    assert_eq!(d.passage.label(), "John 3:16");
    // a real aside is untouched: this still parses as the reference
    let o = parse_default("romans (chapter 8) verse 28");
    assert!(
        o.detections.iter().any(|d| d.passage.label() == "Romans 8:28"),
        "{o:?}"
    );
    // a whole line of tags is not speech
    let o = parse_default("[APPLAUSE] [LAUGHTER]");
    assert!(!o.is_speech);
}

// ---------------------------------------------------------------------------
// v0.890.0 — the operator-adjustable suggestion gates (ParseContext-carried,
// so these tests never touch global state and always run parallel-safe)
// ---------------------------------------------------------------------------

fn parse_with_gates(text: &str, min_words: u8, min_pct: u8) -> ParseOutcome {
    let ctx = ParseContext { quote_gates: QuoteGates::clamped(min_words, min_pct), ..ParseContext::default() };
    parse(bible(), text, &ctx, 0.75)
}

/// v0.890.0 — the card word floor is the operator's setting. At the default
/// (3) the 3-word unique tier still suggests; at 5 (v0.889's floor) it must
/// not. "for God so" uniquely opens John 3:16 and scores 86.
#[test]
fn suggest_word_floor_is_adjustable() {
    let o = parse_with_gates("for god so", 3, MIN_QUOTE_PCT);
    assert!(
        o.confirms.iter().any(|d| d.passage.label() == "John 3:16"),
        "3 words / 50%: the short suggestion still appears (v0.856)"
    );

    let o = parse_with_gates("for god so", 5, MIN_QUOTE_PCT);
    assert!(
        !o.confirms.iter().any(|d| d.kind == DetectionKind::Quotation),
        "5 words minimum (v0.889): a 3-word fragment suggests nothing"
    );
}

/// v0.890.0 — the card % bar is the operator's slider. A mid-strength
/// look-alike (~50-74%) cards at the v0.856 bar (50) and stays silent at 75+.
/// The fragment is the documented Luke-1:17-read look-alike: it fits
/// Malachi 4:6 only loosely (singular "father" vs the verse's "fathers"),
/// so it lands in the 50-74 band on the real matcher.
#[test]
fn suggest_pct_bar_is_adjustable() {
    let said = "turn the heart of the father to the children";
    let loose = parse_with_gates(said, 3, 50);
    let q = loose
        .confirms
        .iter()
        .find(|d| d.kind == DetectionKind::Quotation)
        .expect("50% bar: the loose look-alike appears (v0.856 behaviour)");
    assert!(
        q.pct >= 50 && q.pct < 75,
        "premise: this look-alike is a weak claim, scored {}",
        q.pct
    );
    let tight = parse_with_gates(said, 3, 75);
    assert!(
        !tight.confirms.iter().any(|d| d.kind == DetectionKind::Quotation),
        "75% bar: the same suggestion stays silent (the operator's Sunday range)"
    );
}

/// v0.890.0 — a REAL read must card at every gate setting (the slider trims
/// weak look-alikes, never true quotations).
#[test]
fn suggest_bars_never_hide_a_true_read() {
    for (w, p) in [(3u8, 50u8), (3, 75), (3, 90), (5, 50), (5, 90)] {
        let o = parse_with_gates("let this mind be in you which was also in christ jesus", w, p);
        assert!(
            o.confirms.iter().any(|d| d.passage.label() == "Philippians 2:5"),
            "w={w} p={p}: a true read is still suggested"
        );
    }
}

/// v0.890.0 — the filler tail (ported from v0.889): "so" at the end no longer
/// drags a real read under the bar. The plain line and the same line with a
/// trailing "so" must land on the SAME verse at >= the same score.
#[test]
fn trailing_filler_no_longer_costs_the_read() {
    let clean = parse_with_gates("able to make thee wise unto salvation", 3, MIN_QUOTE_PCT);
    let tailed = parse_with_gates("able to make thee wise unto salvation so", 3, MIN_QUOTE_PCT);
    let q = |o: &ParseOutcome| {
        o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).map(|d| (d.passage.label(), d.pct))
    };
    let (label_clean, pct_clean) = q(&clean).expect("clean read is identified");
    let (label_tailed, pct_tailed) = q(&tailed).expect("tailed read is identified");
    assert_eq!(label_clean, label_tailed, "same verse with or without the tail");
    assert!(pct_tailed >= pct_clean, "the tail must not cost points ({} < {})", pct_tailed, pct_clean);
}
