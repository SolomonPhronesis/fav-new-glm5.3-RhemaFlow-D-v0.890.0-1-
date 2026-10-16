//! v0.856.0 investigation: why does Proverbs 19:21 falsely show?
//!
//! Proverbs 19:21 KJV: "There are many devices in a man's heart;
//! nevertheless the counsel of the LORD, that shall stand."
//!
//! Feeds every plausible spoken fragment that touches this verse through the
//! REAL pipeline (`parse_full` + the auto-approve rule) and prints whether
//! the verse would card or AUTO-STAGE. Also prints the ranked runner-ups so
//! the guilty gate (3-word min tier / short-quote bar / auto-approve bar)
//! can be named exactly.
//!
//! Run: cargo run -p rf-core --example prov1921

use rf_bible::Bible;
use rf_core::parser::{parse_full, Passage, ParseContext};

fn label(bib: &Bible, p: &Passage) -> String {
    format!(
        "{} {}:{}",
        bib.book_name(p.book as usize),
        p.chapter,
        p.from
    )
}

/// the app's rule EXACTLY, from the live constants (rf-app/src/state.rs
/// mirrors this — kept in sync by the rf-app tests). v0.856.0: W=5/A=90 from
/// the whole-Bible grid (docs/GRID-PROV19-21-v0856.md).
fn would_auto(d: &rf_core::parser::Detection) -> bool {
    use rf_core::parser::{AUTO_MIN_WORDS, AUTO_STAGE_PCT};
    if d.words > 0 && d.words < AUTO_MIN_WORDS {
        return false;
    }
    if d.alternatives.is_empty() {
        return d.pct == 0 || d.pct >= AUTO_STAGE_PCT;
    }
    let second = d.alt_pct.first().copied().unwrap_or(0);
    d.pct >= AUTO_STAGE_PCT && d.pct.saturating_sub(second) >= 12
}

fn main() {
    let bib = Bible::embedded();

    // book indices: Proverbs = 19 (0-based), Psalms = 18, Isaiah = 22, Jeremiah = 23
    println!("Target      : {}", bib.verse(19, 19, 21));
    println!("Lookalike A : {}", bib.verse(18, 33, 11));
    println!("Lookalike B : {}", bib.verse(22, 46, 10));
    println!("Lookalike C : {}", bib.verse(23, 23, 18));
    println!();

    let phrases: &[&str] = &[
        // sub-phrases of Proverbs 19:21 itself, shortest first
        "there are many devices",
        "many devices",
        "devices in a man's heart",
        "many devices in a man's heart",
        "there are many devices in a man's heart",
        "nevertheless the counsel of the lord",
        "the counsel of the lord",
        "the counsel of the lord that shall stand",
        "counsel of the lord that shall stand",
        "nevertheless the counsel of the lord that shall stand",
        "the counsel of the lord shall stand",
        "that shall stand",
        "shall stand",
        // the look-alike verses read VERBATIM (would showing Prov 19:21 be false?)
        "the counsel of the lord standeth for ever",
        "the thoughts of his heart to all generations",
        "my counsel shall stand",
        "my counsel shall stand and i will do all my pleasure",
        "who hath stood in the counsel of the lord",
        // pulpit chatter that merely touches the words
        "the counsel of the lord this morning",
        "we need the counsel of the lord",
        // MODERN chatter: the verse's own opening said about phones/gadgets
        "there are many devices in this room",
        "there are many devices here today",
        "there are many devices on my desk",
        "there are many devices",
        "many plans in a man's heart",
        "many plans in a man's heart nevertheless the counsel of the lord",
    ];

    for said in phrases {
        let out = parse_full(bib, said, &ParseContext::default(), 0.75, &[], None);
        let ranked: Vec<String> = bib
            .match_quotations_ranked(said, 6)
            .iter()
            .map(|h| format!("{} {}:{} {}%", bib.book_name(h.book), h.chapter, h.verse, h.pct))
            .collect();
        let confirm_strs: Vec<String> = out
            .confirms
            .iter()
            .map(|d| {
                format!(
                    "{}{} [{}]",
                    if would_auto(d) { "AUTO-STAGE " } else { "card " },
                    label(bib, &d.passage),
                    d.pct
                )
            })
            .collect();
        println!("{:?} ({}w)", said, said.split_whitespace().count());
        println!("  pipeline: {}", if confirm_strs.is_empty() { "nothing".into() } else { confirm_strs.join(" | ") });
        println!("  ranked  : {}", ranked.join(", "));
        println!();
    }

    // the operator-named-verse case: something else was on stage
    for (staged, said) in [
        (Passage::single(18, 33, 11), "the counsel of the lord"),
        (Passage::single(22, 46, 10), "my counsel shall stand"),
        (Passage::single(19, 19, 20), "the counsel of the lord that shall stand"),
    ] {
        let mut ctx = ParseContext::default();
        ctx.last = Some(staged);
        let out = parse_full(bib, said, &ctx, 0.75, &[], None);
        let cs: Vec<String> = out
            .confirms
            .iter()
            .map(|d| format!("{}{} [{}]", if would_auto(d) { "AUTO-STAGE " } else { "card " }, label(bib, &d.passage), d.pct))
            .collect();
        println!(
            "with {} on stage, {:?} -> {}",
            label(bib, &staged),
            said,
            if cs.is_empty() { "nothing".into() } else { cs.join(" | ") }
        );
    }
}
