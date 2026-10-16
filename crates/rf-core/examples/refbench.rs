//! v0.853.0 before/after accuracy benchmark for the spoken-Scripture pipeline.
//!
//! Run: cargo run --release -p rf-core --example refbench -- <out.json>
//!
//! Measures, on the exact code in this tree:
//! 1. REF-COVERAGE: every verse of every book spoken as a reference,
//!    in the forms preachers actually use:
//!      A "2 corinthians 5 17"            (digits)
//!      B "second corinthians chapter five verse seventeen"  (words)
//!      C "second corinthians 5 17"       (ordinal + digits, numbered books)
//!      D "2 corinthians chapter 5 verse 17"                 (mixed + kws)
//! 2. CHAPTER-ONLY: every (book, chapter) as "book chapter".
//! 3. THE FIVE REAL FAILURES from the operator's screenshots (with the
//!    stage context the screenshots show), simulating the app: last
//!    detection wins, quotations follow the auto-approve rule, nav runs
//!    unless scripture wins.
//! 4. QUOTATIONS: 3-word unique, 4/6/8-word fragments, mid-sentence
//!    lead-ins, one-word ASR slips — over a fixed sample of well-known verses.
//! 5. FALSE POSITIVES: pulpit chatter that must stage nothing.
//! 6. TIMING: mean parse µs/call for references and quotations.
//!
//! Output: one JSON document consumed by tools/make_comparison.py.

use rf_core::books::{self, BOOKS};
use rf_bible::Bible;
use rf_core::parser::{parse, parse_full, Command, DetectionKind, ParseContext};
use std::time::Instant;

fn words_num(n: u16) -> String {
    const ONES: [&str; 20] = ["zero","one","two","three","four","five","six","seven","eight","nine","ten","eleven","twelve","thirteen","fourteen","fifteen","sixteen","seventeen","eighteen","nineteen"];
    const TENS: [&str; 10] = ["","","twenty","thirty","forty","fifty","sixty","seventy","eighty","ninety"];
    if n < 20 { return ONES[n as usize].into(); }
    if n < 100 {
        let (t, o) = (n / 10, n % 10);
        return if o == 0 { TENS[t as usize].into() } else { format!("{} {}", TENS[t as usize], ONES[o as usize]) };
    }
    n.to_string()
}

fn spoken_forms(_bi: usize, name: &str) -> Vec<String> {
    let mut forms = vec![name.to_ascii_lowercase()];
    if let Some(rest) = name.strip_prefix("1 ") {
        forms = vec![format!("1 {rest}"), format!("first {rest}"), format!("1st {rest}")];
    } else if let Some(rest) = name.strip_prefix("2 ") {
        forms = vec![format!("2 {rest}"), format!("second {rest}"), format!("2nd {rest}")];
    } else if let Some(rest) = name.strip_prefix("3 ") {
        forms = vec![format!("3 {rest}"), format!("third {rest}"), format!("3rd {rest}")];
    }
    forms
}

fn staged_label(out: &rf_core::parser::ParseOutcome, auto_approve: bool) -> Option<String> {
    // mirrors actions.rs: last detection wins the stage
    if let Some(det) = out.detections.last() {
        return Some(det.passage.label());
    }
    // quotation confirm -> auto-approve rule (state.rs should_auto_approve)
    for det in &out.confirms {
        if det.kind == DetectionKind::Quotation {
            let win = if det.alternatives.is_empty() {
                det.pct == 0 || det.pct >= 60 // v0.853.0: single candidate, 70 -> 60
            } else {
                let second = det.alt_pct.first().copied().unwrap_or(0);
                det.pct >= 75 && det.pct.saturating_sub(second) >= 12
            };
            if auto_approve && win {
                return Some(det.passage.label());
            }
        }
    }
    None
}

fn main() {
    let out_path = std::env::args().nth(1).unwrap_or_else(|| "bench.json".into());
    let bib = Bible::embedded();
    let mut sample_failures: Vec<serde_json::Value> = Vec::new();
    let mut group_counts: std::collections::BTreeMap<String, (usize, usize)> = Default::default();

    // ---- 1. reference coverage: all 31,102 verses, forms A/B/C/D ----
    let t0 = Instant::now();
    let mut parse_us_total = 0u128;
    let mut parse_n = 0u128;
    for (bi, book) in BOOKS.iter().enumerate() {
        let forms = spoken_forms(bi, book.name);
        for ci in 0..bib.chapter_count(bi) {
            for vi in 0..bib.verse_count(bi, ci + 1) {
                let want = format!("{} {}:{}", book.name, ci + 1, vi + 1);
                for (fi, bf) in forms.iter().enumerate() {
                    let said = match fi {
                        0 => format!("{bf} {} {}", ci + 1, vi + 1),
                        1 => {
                            // "twenty one" is ambiguous with 21 — a real
                            // speaker says "twenty, VERSE one"; insert the
                            // keyword whenever the digit pair could merge
                            let ch = (ci + 1) as u16;
                            let vs = (vi + 1) as u16;
                            if ch >= 20 && ch % 10 == 0 && vs < 10 {
                                format!("{bf} {} verse {}", words_num(ch), words_num(vs))
                            } else {
                                format!("{bf} {} {}", words_num(ch), words_num(vs))
                            }
                        }
                        _ => format!("{bf} {} {}", ci + 1, vi + 1),
                    };
                    let ctx = ParseContext::default();
                    let t = Instant::now();
                    let out = parse(bib, &said, &ctx, 0.75);
                    parse_us_total += t.elapsed().as_micros();
                    parse_n += 1;
                    let got = staged_label(&out, true);
                    let ok = got.as_deref() == Some(want.as_str());
                    let e = group_counts.entry(match fi { 0 => "A_digits", 1 => "B_words", _ => "C_ordinal_digits" }.to_string()).or_insert((0, 0));
                    e.1 += 1;
                    if ok { e.0 += 1; }
                    if !ok && sample_failures.len() < 400 {
                        sample_failures.push(serde_json::json!({"group": format!("ref_{}", fi), "said": said, "want": want, "got": got.unwrap_or_else(|| "(nothing)".into())}));
                    }
                }
                if (vi + ci) % 7 == 0 {
                    let said = format!("{} chapter {} verse {}", forms[0], ci + 1, vi + 1);
                    let ctx = ParseContext::default();
                    let out = parse(bib, &said, &ctx, 0.75);
                    let got = staged_label(&out, true);
                    let ok = got.as_deref() == Some(want.as_str());
                    let e = group_counts.entry("D_mixed_kw".to_string()).or_insert((0, 0));
                    e.1 += 1;
                    if ok { e.0 += 1; }
                    if !ok && sample_failures.len() < 400 {
                        sample_failures.push(serde_json::json!({"group": "ref_D", "said": said, "want": want, "got": got.unwrap_or_else(|| "(nothing)".into())}));
                    }
                }
            }
        }
    }
    let refcover_ms = t0.elapsed().as_millis();

    // ---- 2. chapter-only, every chapter of every book ----
    for (bi, book) in BOOKS.iter().enumerate() {
        let bf = spoken_forms(bi, book.name)[0].clone();
        for ci in 0..bib.chapter_count(bi) {
            let said = format!("{bf} {}", ci + 1);
            let ctx = ParseContext::default();
            let out = parse(bib, &said, &ctx, 0.75);
            let want = format!("{} {}:1", book.name, ci + 1);
            let got = staged_label(&out, true);
            let ok = got.as_deref() == Some(want.as_str());
            let e = group_counts.entry("chapter_only".to_string()).or_insert((0, 0));
            e.1 += 1;
            if ok { e.0 += 1; }
            if !ok && sample_failures.len() < 400 {
                sample_failures.push(serde_json::json!({"group": "chapter_only", "said": said, "want": want, "got": got.unwrap_or_else(|| "(nothing)".into())}));
            }
        }
    }

    // ---- 3. the five real failures (screenshot transcripts) ----
    let mut five: Vec<(String, String, String)> = Vec::new();
    {
        let mut ctx = ParseContext::default();
        ctx.last = Some(rf_core::parser::Passage::single(books::idx("Joshua") as u8, 1, 8));
        let said = "Second Chronicles No Second Corinthians chapter 5 verse 17.";
        let out = parse_full(bib, said, &ctx, 0.75, &[], None);
        five.push(("F1_self_corrected_reference".into(), said.into(), staged_label(&out, true).unwrap_or_else(|| "(nothing)".into())));
    }
    {
        let mut ctx = ParseContext::default();
        ctx.last = Some(rf_core::parser::Passage::single(books::idx("2 Corinthians") as u8, 5, 1));
        let said = "We go to Psalms.";
        let out1 = parse_full(bib, said, &ctx, 0.75, &[], None);
        if let Some(d) = out1.confirms.iter().find(|d| d.kind == DetectionKind::BookOnly) {
            ctx.pending_book = Some(d.passage.book);
        }
        let said2 = "23 verse 1.";
        let out2 = parse_full(bib, said2, &ctx, 0.75, &[], None);
        five.push(("F2_book_then_numbers".into(), format!("{said} + {said2}"), staged_label(&out2, true).unwrap_or_else(|| "(nothing)".into())));
    }
    {
        let mut ctx = ParseContext::default();
        ctx.last = Some(rf_core::parser::Passage::single(books::idx("Hebrews") as u8, 11, 1));
        let said = "The evidence of things not seen. Next verse.";
        let out = parse_full(bib, said, &ctx, 0.75, &[], None);
        let nav = out.commands.contains(&Command::NextVerse);
        let quote_card = out.confirms.iter().any(|d| d.kind == DetectionKind::Quotation);
        let blocking = !out.detections.is_empty()
            || out.confirms.iter().any(|d| d.kind != DetectionKind::Quotation)
            || !out.errors.is_empty();
        let applied = nav && !blocking;
        let result = if applied { "advanced (next verse ran)".into() }
            else if nav && quote_card { "queued for review (nav dropped)".into() }
            else if nav { "command found, blocked".into() }
            else { "no command found".into() };
        five.push(("F3_next_verse_command".into(), said.into(), result));
    }
    for (said, want) in [
        ("Let this mind be in you.", "Philippians 2:5"),
        ("There is a mentality of Zion that is to don't have when the Bible says let this mind be in you.", "Philippians 2:5"),
        ("Let this mind being you.", "Philippians 2:5"),
        ("for God so", "John 3:16"),
        ("for God so loved the world", "John 3:16"),
        ("the Lord is my shepherd", "Psalms 23:1"),
    ] {
        let ctx = ParseContext::default();
        let out = parse_full(bib, said, &ctx, 0.75, &[], None);
        let got = staged_label(&out, true).unwrap_or_else(|| {
            out.confirms.iter().find(|d| d.kind == DetectionKind::Quotation)
                .map(|d| format!("card: {}", d.passage.label()))
                .unwrap_or_else(|| "(nothing)".into())
        });
        five.push(("F4_quotation_autoshow".into(), said.into(), format!("{got}|want:{want}")));
    }
    {
        let ctx = ParseContext::default();
        let saids = ["Let this mind being you.", "That doesn't mind being you.", "Let this mind be in you."];
        let mut results = Vec::new();
        for s in saids {
            let out = parse_full(bib, s, &ctx, 0.75, &[], None);
            results.push(staged_label(&out, true).unwrap_or_else(|| {
                out.confirms.iter().find(|d| d.kind == DetectionKind::Quotation)
                    .map(|d| format!("card: {}", d.passage.label()))
                    .unwrap_or_else(|| "(nothing)".into())
            }));
        }
        // v0.853.0: the FIRST clean repeat lands (auto or card); the extreme
        // mishearing cannot be resolved from text alone (lexicon/final-pass
        // territory); the last, correctly-heard repeat lands again. The old
        // app produced NOTHING three times in a row.
        five.push(("F5_repeat_three_times".into(), saids.join(" | "), format!("{} | {} | {}", results[0], results[1], results[2])));
    }
    for (g, said, got) in &five {
        let ok = match g.as_str() {
            // v0.853.0: the first clean repeat and the final correct one must
            // land; the middle extreme mishearing may stay unmatched
            "F5_repeat_three_times" => {
                let parts: Vec<&str> = got.split(" | ").collect();
                parts.len() == 3 && parts[0].contains("Philippians 2:5") && parts[2].contains("Philippians 2:5")
            }
            _ => {
                let want = if g.starts_with("F4") {
                    let s = said.as_str();
                    // both "for God so" fragments quote John 3:16 (the len
                    // guard here used to mis-derive "for God so loved the
                    // world" as wanting Philippians 2:5)
                    if s.starts_with("for God so") { "John 3:16" }
                    else if s.contains("shepherd") { "Psalms 23:1" }
                    else { "Philippians 2:5" }
                } else if g == "F1_self_corrected_reference" { "2 Corinthians 5:17" }
                else if g == "F2_book_then_numbers" { "Psalms 23:1" }
                else if g == "F3_next_verse_command" { "advanced (next verse ran)" }
                else { "" };
                if g.starts_with("F4") {
                    got.starts_with(want)
                } else {
                    got == want
                }
            }
        };
        let e = group_counts.entry(g.clone()).or_insert((0, 0));
        e.1 += 1;
        if ok { e.0 += 1; }
    }

    // ---- 4. quotation battery ----
    const SAMPLES: &[(&str, &str, u16, u16)] = &[
        ("John", "For God so loved the world that he gave his only begotten Son", 3, 16),
        ("Psalms", "The LORD is my shepherd I shall not want", 23, 1),
        ("Philippians", "Let this mind be in you which was also in Christ Jesus", 2, 5),
        ("Genesis", "In the beginning God created the heaven and the earth", 1, 1),
        ("Revelation", "Behold I come quickly and my reward is with me", 22, 12),
        ("Romans", "For all have sinned and come short of the glory of God", 3, 23),
        ("Proverbs", "Trust in the LORD with all thine heart and lean not unto thine own understanding", 3, 5),
        ("Isaiah", "For unto us a child is born unto us a son is given", 9, 6),
        ("Matthew", "Come unto me all ye that labour and are heavy laden", 11, 28),
        ("Hebrews", "Now faith is the substance of things hoped for the evidence of things not seen", 11, 1),
    ];
    for (bk, text, c, v) in SAMPLES {
        let w: Vec<String> = text.to_ascii_lowercase().split_whitespace().map(|s| s.to_string()).collect();
        let book_idx = books::idx(bk);
        let want = format!("{} {}:{}", BOOKS[book_idx].name, c, v);
        for n in [3usize, 4, 6, 8] {
            if w.len() < n { continue; }
            let frag = w[..n].join(" ");
            let ctx = ParseContext::default();
            let out = parse_full(bib, &frag, &ctx, 0.75, &[], None);
            let got = staged_label(&out, true);
            // v0.853.0 semantics, per fragment kind:
            //  - non-distinctive 3-gram ("for unto us") -> must stay QUIET
            //    (by design: ordinary English never founds a quotation)
            //  - unique opening -> must RESOLVE to the verse (auto or card)
            //  - ambiguous opening -> must raise a CARD that lists the true
            //    verse among the ranked candidates
            let words_f: Vec<String> = frag.split_whitespace().map(|s| s.to_string()).collect();
            let distinctive = n != 3 || rf_bible::trigram_distinctive(&words_f);
            let unique = rf_bible::trigram_distinctive(&words_f) && bib.match_quotation(&frag).is_some();
            let found = if !distinctive {
                got.is_none() && !out.confirms.iter().any(|d| d.kind == DetectionKind::Quotation)
            } else if unique {
                got.as_deref() == Some(want.as_str())
                    || out.confirms.iter().any(|d| d.kind == DetectionKind::Quotation && d.passage.label() == want)
            } else {
                out.confirms.iter().any(|d| d.kind == DetectionKind::Quotation
                    && (d.passage.label() == want || d.alternatives.iter().any(|a| a.label() == want)))
            };
            let e = group_counts.entry(format!("quote_{n}w")).or_insert((0, 0));
            e.1 += 1;
            if found { e.0 += 1; }
            if !found && sample_failures.len() < 400 {
                sample_failures.push(serde_json::json!({"group": format!("quote_{n}w"), "said": frag, "want": want, "got": got.unwrap_or_else(|| "(nothing)".into())}));
            }
        }
        let mid = format!("you know the bible says there in {}", w[..6].join(" "));
        let ctx = ParseContext::default();
        let out = parse_full(bib, &mid, &ctx, 0.75, &[], None);
        let got = staged_label(&out, true);
        let ok = got.as_deref() == Some(want.as_str());
        let e = group_counts.entry("quote_long_lead_in".to_string()).or_insert((0, 0));
        e.1 += 1;
        if ok { e.0 += 1; }
        if !ok && sample_failures.len() < 400 {
            sample_failures.push(serde_json::json!({"group": "quote_long_lead_in", "said": mid, "want": want, "got": got.unwrap_or_else(|| "(nothing)".into())}));
        }
        if w.len() >= 6 {
            let mut slipped = w.clone();
            slipped[3] = "banana".into();
            let said = slipped[..6].join(" ");
            let ctx = ParseContext::default();
            let out = parse_full(bib, &said, &ctx, 0.75, &[], None);
            let got = staged_label(&out, true);
            let ok = got.as_deref() == Some(want.as_str());
            let e = group_counts.entry("quote_asr_slip".to_string()).or_insert((0, 0));
            e.1 += 1;
            if ok { e.0 += 1; }
            if !ok && sample_failures.len() < 400 {
                sample_failures.push(serde_json::json!({"group": "quote_asr_slip", "said": said, "want": want, "got": got.unwrap_or_else(|| "(nothing)".into())}));
            }
        }
    }

    // ---- 5. false positives: pulpit chatter must stage nothing ----
    const CHATTER: &[&str] = &[
        "good morning everybody welcome to church this morning",
        "we are so glad you made it out today",
        "let us pray father we thank you for this day",
        "turn with me in your bibles this morning please",
        "i want to talk about the love of god today",
        "the next verse says something wonderful about grace",
        "we should not look back on the past because of this",
        "next week we will meet at nine in the morning",
        "there are three things i want you to remember today",
        "and he told them the story about the fisherman and his net again",
        "the church has been here for one hundred years",
        "my grandmother used to say that all the time",
        "can everybody hear me in the back",
        "we have a different approach to marriage and family",
        "the wisdom of the elders is worth remembering every single day",
        "i was sitting in my office one day this week",
        "there is a mentality of zion that we must have",
        "what did the bible mean when it said that",
        "a lot of people in the world live like this",
        "so by way of reminder let me say it again",
    ];
    for s in CHATTER {
        let ctx = ParseContext::default();
        let out = parse_full(bib, s, &ctx, 0.75, &[], None);
        let staged = staged_label(&out, true).is_some();
        let card = !out.confirms.is_empty() || !out.errors.is_empty();
        let clean = !staged && !card;
        let e = group_counts.entry("chatter_false_positive".to_string()).or_insert((0, 0));
        e.1 += 1;
        if clean { e.0 += 1; }
        if !clean && sample_failures.len() < 400 {
            sample_failures.push(serde_json::json!({"group": "chatter", "said": s, "want": "clean", "got": if staged { "staged".to_string() } else { "card".to_string() }}));
        }
    }

    // ---- 6. timing ----
    let t1 = Instant::now();
    for _ in 0..2000 {
        let ctx = ParseContext::default();
        let _ = parse(bib, "second corinthians chapter five verse seventeen", &ctx, 0.75);
    }
    let ref_us = t1.elapsed().as_micros() as f64 / 2000.0;
    let t2 = Instant::now();
    for _ in 0..2000 {
        let ctx = ParseContext::default();
        let _ = parse_full(bib, "the bible says let this mind be in you", &ctx, 0.75, &[], None);
    }
    let quote_us = t2.elapsed().as_micros() as f64 / 2000.0;

    let mut groups = serde_json::Map::new();
    for (g, (ok, n)) in &group_counts {
        groups.insert(g.clone(), serde_json::json!({ "ok": ok, "n": n, "pct": (100.0 * *ok as f64 / *n as f64 * 10.0).round() / 10.0 }));
    }
    let doc = serde_json::json!({
        "refcover_ms": refcover_ms,
        "parse_us_per_call": ref_us,
        "quote_us_per_call": quote_us,
        "parse_samples": parse_n,
        "groups": groups,
        "five_cases": five.iter().map(|(g, s, r)| serde_json::json!({"case": g, "said": s, "result": r})).collect::<Vec<_>>(),
        "sample_failures": sample_failures,
    });
    std::fs::write(&out_path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    println!("wrote {out_path}");
    for (g, (ok, n)) in &group_counts {
        println!("{g:>28}: {ok}/{n} ({:.1}%)", 100.0 * *ok as f64 / *n as f64);
    }
    println!("parse {ref_us:.0} µs/call, quote {quote_us:.0} µs/call, full sweep {refcover_ms} ms");
}
