//! v0.855.0 pre-fix reproduction of the two operator screenshots (issue 1 & 2).
//!
//! Issue 1 (screenshot A): the operator named "Luke 1:17"; the preacher then
//! read the verse. The half-line "turn the heart of the father to the
//! children" fits BOTH Luke 1:17 and Malachi 4:6 (Luke quotes Malachi), and
//! the app auto-staged MALACHI 4:6 over the named verse.
//!
//! Issue 2 (screenshot B): the fragment "also before him" (3 generic words,
//! the tail of Acts 23:33) raised false cards — Acts 23:33 and Daniel 6:22.
//!
//! Run: cargo run -p rf-core --example repro855

use rf_bible::Bible;
use rf_core::parser::{parse_full, Passage, ParseContext};

fn label(bib: &Bible, p: &Passage) -> String {
    format!("{} {}:{} ({})", bib.book_name(p.book as usize), p.chapter, p.from, p.to)
}

fn luke_1_17() -> Passage {
    // canonical book index: Luke = 41 (0-based: Matthew 39, Mark 40, Luke 41)
    Passage::single(41, 1, 17)
}

fn malachi_4_6() -> Passage {
    Passage::single(38, 4, 6)
}

fn main() {
    let bib = Bible::embedded();
    println!("Luke 1:17 KJV: {}", bib.verse(41, 1, 17));
    println!("Malachi 4:6  : {}", bib.verse(38, 4, 6));
    println!("Acts 23:33   : {}", bib.verse(43, 23, 33));
    println!("Dan 6:22     : {}", bib.verse(26, 6, 22));
    println!();

    for said in [
        // issue 1: the preacher mid-verse
        "turn the heart of the father to the children",
        // issue 1 tail: the distinctive part that should settle it
        "and the disobedient to the wisdom of the just",
        // issue 2: the fragment that must not card
        "also before him",
    ] {
        let out = parse_full(bib, said, &ParseContext::default(), 0.75, &[], None);
        let r: Vec<String> = bib
            .match_quotations_ranked(said, 8)
            .iter()
            .map(|h| format!("{} {}:{} {}%", bib.book_name(h.book), h.chapter, h.verse, h.pct))
            .collect();
        println!(
            "{said:?}\n  confirms={:?}\n  ranked=[{}]",
            out.confirms
                .iter()
                .map(|d| format!("{} pct={} alts={}", d.passage.label(), d.pct, d.alternatives.len()))
                .collect::<Vec<_>>(),
            r.join(", ")
        );
        for d in &out.confirms {
            let auto = d.alternatives.is_empty() && (d.pct == 0 || d.pct >= 60);
            println!("  -> would {} : {}", if auto { "AUTO-STAGE" } else { "card" }, label(bib, &d.passage));
        }
        println!();
    }

    // with Luke 1:17 already on stage (the operator named it)
    let mut ctx = ParseContext::default();
    ctx.last = Some(luke_1_17());
    let out = parse_full(bib, "turn the heart of the father to the children", &ctx, 0.75, &[], None);
    println!("with Luke 1:17 on stage, 'turn the heart of the father to the children' ->");
    for d in &out.confirms {
        let auto = d.alternatives.is_empty() && (d.pct == 0 || d.pct >= 60);
        println!(
            "  {} {} (pct={} alts={}) — displaces the named verse: {}",
            if auto { "AUTO-STAGE" } else { "card" },
            label(bib, &d.passage),
            d.pct,
            d.alternatives.len(),
            d.passage != malachi_4_6()
        );
    }
    if out.confirms.is_empty() {
        println!("  nothing (verse stays)");
    }
}
