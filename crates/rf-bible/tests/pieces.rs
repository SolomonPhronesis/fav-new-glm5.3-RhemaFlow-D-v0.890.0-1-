//! v0.816.0: a verse quoted in short, garbled pieces (the speech engine splits at every breath).
//! The pieces below are EXACTLY what the operator's Mac heard on 2026-10-08 for 2 Corinthians 5:17.
use rf_bible::{words_alike, Bible};

const USER_PIECES: [&str; 9] = [
    "If any man being Christ.", "He is.", "A new.", "Creation", "Old things.", "I'm passed away.", "Behold.", "All things.", "I'll become new.",
];

fn kjv() -> &'static Bible {
    Bible::embedded()
}
fn name(b: &Bible, hit: &rf_bible::QuoteHit) -> String {
    format!("{} {}:{}", b.book_name(hit.book), hit.chapter, hit.verse)
}

#[test]
fn the_operators_nine_pieces_find_2_corinthians_5_17() {
    let b = kjv();
    let m = b.match_pieces(&USER_PIECES, 8);
    let top = m.hits.first().expect("a match is found");
    assert_eq!(name(b, top), "2 Corinthians 5:17");
    assert!(top.pct >= 70, "confident enough for auto-approve, got {}%", top.pct);
    assert!(m.hits.get(1).map(|h| top.pct.saturating_sub(h.pct) >= 12).unwrap_or(true), "a clear winner: {:?}", m.hits);
    assert!(m.pieces_used >= 8, "it used the pieces together: {}", m.pieces_used);
}

#[test]
fn old_unrelated_pieces_in_front_do_not_spoil_it() {
    let b = kjv();
    let mut pieces = vec!["Next.", "Read.", "previous.", "Full screen.", "That's it for screen."];
    pieces.extend(USER_PIECES);
    let m = b.match_pieces(&pieces, 8);
    assert_eq!(name(b, &m.hits[0]), "2 Corinthians 5:17");
    assert!(m.pieces_used <= 10, "the junk at the front is left out: {}", m.pieces_used);
}

#[test]
fn it_also_works_when_only_the_second_half_has_been_said_so_far() {
    let b = kjv();
    let m = b.match_pieces(&USER_PIECES[3..], 8);
    assert_eq!(name(b, &m.hits[0]), "2 Corinthians 5:17");
    assert!(m.hits[0].pct >= 60, "{}", m.hits[0].pct);
}

#[test]
fn a_verse_said_from_the_middle_in_pieces_is_found() {
    let b = kjv();
    let m = b.match_pieces(&["for by grace are ye saved", "through faith", "and that not of yourselves", "it is the gift of God"], 8);
    assert_eq!(name(b, &m.hits[0]), "Ephesians 2:8");
}

#[test]
fn ordinary_preaching_and_notices_match_nothing_with_confidence() {
    let b = kjv();
    let talk: [&[&str]; 6] = [
        &["So today I want to talk to you about", "what it means to follow Jesus", "in everyday life"],
        &["The music team will lead us in worship", "and then we will take the offering", "please be generous"],
        &["Please turn to your neighbour", "and tell them that God loves them", "and that you are glad they came"],
        &["I went to the shop yesterday", "and bought some milk and bread", "for the family"],
        &["We have a youth camp coming up", "in the summer holidays", "and we need volunteers to help in the kitchen"],
        &["Let us stand together", "and pray for the nation", "and for our leaders and our city"],
    ];
    for pieces in talk {
        let m = b.match_pieces(pieces, 8);
        if let Some(top) = m.hits.first() {
            assert!(top.pct < 60, "{pieces:?} wrongly matched {} at {}%", name(b, top), top.pct);
        }
    }
}

#[test]
fn a_very_common_opening_is_never_a_confident_pick() {
    let b = kjv();
    let m = b.match_pieces(&["and the lord spake unto moses saying"], 8);
    if let Some(top) = m.hits.first() {
        let second = m.hits.get(1).map(|h| h.pct).unwrap_or(0);
        assert!(!(top.pct >= 75 && top.pct.saturating_sub(second) >= 12), "{:?}", m.hits);
    }
}

#[test]
fn empty_and_tiny_input_is_safe() {
    let b = kjv();
    assert!(b.match_pieces(&[], 8).hits.is_empty());
    assert!(b.match_pieces(&["hello there"], 8).hits.is_empty());
    assert!(b.match_pieces(&["", "  ", "a"], 8).hits.is_empty());
    assert!(b.match_pieces(&USER_PIECES, 0).hits.is_empty());
}

#[test]
fn similar_sounding_words_count_as_the_same_word() {
    assert!(words_alike("creation", "creature"));
    assert!(words_alike("lord", "lord"));
    assert!(words_alike("saviour", "savior"));
    assert!(!words_alike("being", "be"));
    assert!(!words_alike("man", "men"), "short words must be exact");
    assert!(!words_alike("god", "good"));
    assert!(!words_alike("love", "lost"));
}

#[test]
fn matching_pieces_is_fast_after_the_first_time() {
    let b = kjv();
    let _ = b.match_pieces(&USER_PIECES, 8); // builds the index once
    let t = std::time::Instant::now();
    for _ in 0..5 {
        let _ = b.match_pieces(&USER_PIECES, 8);
    }
    let per = t.elapsed().as_millis() / 5;
    assert!(per < 400, "{per} ms per call even in a slow debug build");
}

#[test]
fn five_words_are_too_few_for_the_piece_matcher_to_believe() {
    // (the older exact-opening matcher, which is stricter in a different way, still handles short openings)
    let b = kjv();
    assert!(b.match_pieces(&["for God so loved the"], 8).hits.is_empty());
    assert!(b.match_pieces(&["for God", "so loved", "the"], 8).hits.is_empty());
}

#[test]
fn a_quote_buried_in_long_chatter_is_not_enough_on_its_own() {
    let b = kjv();
    let m = b.match_pieces(&["so I told him that for God so loved the world", "and then we went home and ate dinner", "with all of the family and the friends from church"], 8);
    if let Some(top) = m.hits.first() {
        assert!(top.pct < 60, "{} {}:{} at {}%", b.book_name(top.book), top.chapter, top.verse, top.pct);
    }
    assert!(m.hits.is_empty(), "{:?}", m.hits);
}

#[test]
fn a_clear_quotation_still_wins_with_chatter_in_front() {
    let b = kjv();
    let m = b.match_pieces(&["okay so", "the Lord is my shepherd", "I shall not want", "he maketh me to lie down in green pastures"], 8);
    let top = &m.hits[0];
    assert_eq!(top.book, m.hits[0].book);
    assert!(name(b, top).starts_with("Psalms 23:"), "{}", name(b, top));
    assert!(top.pct >= 70, "{}%", top.pct);
}
