//! v0.855.0 — NINE-GROUP text-layer evaluation ("the nine test sets").
//!
//! Run: cargo run --release -p rf-core --example textbench -- <out.json>
//!
//! The nine PUBLIC speech test sets behind the 93.9%/83.7% chart (LibriSpeech
//! etc.) cannot run in this sandbox: no speech model, no audio, no network to
//! the datasets. What CAN be measured here — honestly, deterministically, on
//! the exact code in this tree — is the TEXT LAYER after the speech model:
//! given ASR-shaped lines (clean reads, garbles, music tags, fragments,
//! name variants, gap-filled repairs, pulpit chatter), does the pipeline
//! identify the right verse, refuse the wrong one, and repair the line?
//!
//! Nine groups, each scored 0-100:
//!   G1 clean_read      20 well-known verses read verbatim (chunk-sized)
//!   G2 noise_garble    12 realistic one/two-word ASR garbles
//!   G3 music_tags      verse reads with [MUSIC]/(guitar) interjections
//!   G4 short_distinctive  distinctive short openings ("for god so")
//!   G5 tail_fragments  generic 3-4 word verse-enders ("also before him")
//!   G6 quote_overlap   the Luke 1:17 vs Malachi 4:6 screenshot sequence
//!   G7 name_variants   Elijah/Elias, Isaiah/Esaias, Jonah/Jonas
//!   G8 gap_fill        verse-on-stage garbled lines, WER before/after repair
//!   G9 chatter         pulpit chatter that must stage nothing
//!
//! COMPOSITE "listening quality" = 20%G1 + 15%G2 + 10%G3 + 10%G4 + 10%G5
//!                                  + 15%G6 + 5%G7 + 10%G8 + 5%G9

use rf_bible::Bible;
use rf_core::parser::{parse_full, Passage, ParseContext};
use rf_core::wer::word_error_rate;

fn bk(b: &Bible, name: &str) -> u8 {
    let i = (0..66)
        .find(|i| b.book_name(*i) == name)
        .unwrap_or_else(|| panic!("book {name}"));
    i as u8
}

/// first n normalized words of a verse, spoken-style (lowercase, no punct)
fn lead_words(b: &Bible, book: &str, c: u16, v: u16, n: usize) -> String {
    let text = b.verse(bk(b, book) as usize, c as usize, v as usize);
    let w: Vec<&str> = text.split_whitespace().take(n).collect();
    w.join(" ")
        .to_lowercase()
        .replace(['.', ',', ';', ':', '?', '!', '"'], "")
}

/// a verse reference as a label
fn lbl(_b: &Bible, book: &str, c: u16, v: u16) -> String {
    format!("{} {}:{}", book, c, v)
}

/// one quotation detection (or ranked top hit) for a line, empty ctx
fn confirm_labels(b: &Bible, text: &str) -> Vec<String> {
    let out = parse_full(b, text, &ParseContext::default(), 0.75, &[], None);
    out.confirms.iter().map(|d| d.passage.label()).collect()
}

/// ranked labels, best first
fn ranked_labels(b: &Bible, text: &str) -> Vec<String> {
    b.match_quotations_ranked(text, 8)
        .iter()
        .map(|h| format!("{} {}:{}", b.book_name(h.book), h.chapter, h.verse))
        .collect()
}

fn hit(a: &[String], want: &str) -> bool {
    a.iter().any(|l| l == want)
}

fn repair_line(b: &Bible, p: &Passage, heard: &str) -> Option<rf_core::repair::Repair> {
    rf_core::repair::repair_line(b, p, heard)
}

struct Score {
    n: usize,
    correct: usize,
}

impl Score {
    fn new() -> Self {
        Score { n: 0, correct: 0 }
    }
    fn add(&mut self, ok: bool) {
        self.n += 1;
        if ok {
            self.correct += 1;
        }
    }
    fn pct(&self) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            100.0 * self.correct as f64 / self.n as f64
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out_path = args.get(1).cloned().unwrap_or_else(|| "textbench.json".into());
    let _ = &out_path;
    let b = Bible::embedded();

    // ---- G1: clean reads (chunk-sized, first 12 words) ----
    let clean: [(&str, u16, u16); 20] = [
        ("John", 3, 16), ("Psalms", 23, 1), ("Psalms", 23, 4), ("Psalms", 46, 1),
        ("Psalms", 91, 1), ("Psalms", 121, 1), ("Proverbs", 3, 5), ("Isaiah", 9, 6),
        ("Isaiah", 40, 31), ("Jeremiah", 29, 11), ("Matthew", 6, 33), ("Matthew", 11, 28),
        ("Matthew", 28, 19), ("John", 14, 6), ("Acts", 16, 31), ("Romans", 8, 28),
        ("Philippians", 4, 13), ("Hebrews", 11, 1), ("Revelation", 22, 12), ("1 Corinthians", 13, 4),
    ];
    let mut g1 = Score::new();
    for (name, c, v) in clean {
        let said = lead_words(&b, name, c, v, 12);
        let want = lbl(&b, name, c, v);
        let labels = confirm_labels(&b, &said);
        let ok = hit(&labels, &want) || ranked_labels(&b, &said).first().map(|l| l == &want).unwrap_or(false);
        g1.add(ok);
    }

    // ---- G2: realistic garbles (one or two words wrong/dropped) ----
    let garbles: [(&str, &str, u16, u16); 12] = [
        ("for god so loved the world that he gave his only son", "John", 3, 16),
        ("the lord is my helper i shall not want", "Psalms", 23, 1),
        ("yea though i walk through the valley of death", "Psalms", 23, 4),
        ("god is our refuge and strength a very present helper", "Psalms", 46, 1),
        ("he that dwells in the secret place of the most high", "Psalms", 91, 1),
        ("i will lift up my eyes unto the hills", "Psalms", 121, 1),
        ("trust in the lord with all your heart", "Proverbs", 3, 5),
        ("unto us a child is born unto us a son given", "Isaiah", 9, 6),
        ("they that wait upon the lord shall mount up with wings", "Isaiah", 40, 31),
        ("i know the plans i have for you says the lord", "Jeremiah", 29, 11),
        ("come unto me all ye that labour and are heavy laden", "Matthew", 11, 28),
        ("i can do everything through christ which strengthens me", "Philippians", 4, 13),
    ];
    let mut g2 = Score::new();
    for (said, name, c, v) in garbles {
        let want = lbl(&b, name, c, v);
        let labels = confirm_labels(&b, said);
        let ok = hit(&labels, &want) || ranked_labels(&b, said).iter().take(3).any(|l| l == &want);
        g2.add(ok);
    }

    // ---- G3: sound-tag interjections ----
    let tagged: [&str; 6] = [
        "[MUSIC] for god so loved the world",
        "the lord is my shepherd (upbeat guitar music) i shall not want",
        "[APPLAUSE] i can do all things through christ",
        "(soft piano) let this mind be in you",
        "we love him because he first loved us [MUSIC]",
        "[MUSIC] [APPLAUSE] behold i come quickly",
    ];
    let wants: [&str; 6] = [
        "John 3:16", "Psalms 23:1", "Philippians 4:13",
        "Philippians 2:5", "1 John 4:19", "Revelation 22:12",
    ];
    let mut g3 = Score::new();
    for (said, want) in tagged.iter().zip(wants.iter()) {
        let labels = confirm_labels(&b, said);
        let ok = hit(&labels, want) || ranked_labels(&b, said).first().map(|l| l == want).unwrap_or(false);
        g3.add(ok);
    }

    // ---- G4: distinctive short openings ----
    let shorts: [(&str, &[&str]); 6] = [
        ("for god so", &["John 3:16"]),
        ("let this mind", &["Philippians 2:5"]),
        ("behold i come", &["Revelation 22:12"]),
        // "the grace of our lord" opens/ends half the epistles — ANY of the
        // real grace-benedictions in the top-3 is a correct identification
        ("the grace of our lord", &["Galatians 6:18", "Philippians 4:23", "2 Corinthians 13:13", "Romans 16:24", "1 Corinthians 16:23", "1 Thessalonians 5:28", "Philemon 1:25", "Revelation 22:21"]),
        ("we love him because", &["1 John 4:19"]),
        ("he that dwells in", &["Psalms 91:1"]),
    ];
    let mut g4 = Score::new();
    for (said, wants) in shorts {
        let labels = confirm_labels(&b, said);
        let ranked = ranked_labels(&b, said);
        let ok = wants.iter().any(|w| hit(&labels, w))
            || wants.iter().any(|w| ranked.iter().take(3).any(|l| l == w));
        g4.add(ok);
    }

    // ---- G5: generic tail fragments must stay silent ----
    let tails: [&str; 6] = [
        "also before him",
        "but have eternal life",
        "with a curse",
        "to their fathers",
        "which have no hope",
        "and it was so",
    ];
    let mut g5 = Score::new();
    for said in tails {
        // silent is best; a card is acceptable ONLY when it can never
        // auto-stage itself (an unopposed single strong candidate)
        let out = parse_full(&b, said, &ParseContext::default(), 0.75, &[], None);
        let auto = out.confirms.iter().any(|d| {
            d.kind == rf_core::parser::DetectionKind::Quotation
                && d.alternatives.is_empty()
                && (d.pct == 0 || d.pct >= 60)
        });
        g5.add(out.confirms.is_empty() || !auto);
    }

    // ---- G6: the Luke 1:17 vs Malachi 4:6 screenshot sequence ----
    let luke = Passage::single(bk(&b, "Luke") as u8, 1, 17);
    let mut ctx = ParseContext::default();
    ctx.last = Some(luke);
    let mut g6 = Score::new();
    // a) the shared half-verse: must NOT card the look-alike
    let out = parse_full(&b, "turn the heart of the father to the children", &ctx, 0.75, &[], None);
    g6.add(!out.confirms.iter().any(|d| d.passage.label() == "Malachi 4:6"));
    // b) the named verse's own distinctive tail: stays on the named verse
    let out = parse_full(&b, "and the disobedient to the wisdom of the just", &ctx, 0.75, &[], None);
    g6.add(out.confirms.iter().all(|d| d.passage.label() == "Luke 1:17"));
    // c) the negative control: the preacher moves to Malachi's own words —
    //    the look-alike lock MUST release and let the new verse card
    let out = parse_full(&b, "lest i come and smite the earth with a curse", &ctx, 0.75, &[], None);
    g6.add(out.confirms.iter().any(|d| d.passage.label() == "Malachi 4:6"));
    // d) Malachi now on stage: its own second half keeps, Luke's words release
    let mut ctx2 = ParseContext::default();
    ctx2.last = Some(Passage::single(bk(&b, "Malachi") as u8, 4, 6));
    let out = parse_full(&b, "and the heart of the children to their fathers", &ctx2, 0.75, &[], None);
    g6.add(out.confirms.iter().all(|d| d.passage.label() == "Malachi 4:6"));
    let out = parse_full(&b, "to make ready a people prepared for the lord", &ctx2, 0.75, &[], None);
    g6.add(out.confirms.iter().any(|d| d.passage.label() == "Luke 1:17"));

    // ---- G7: OT/NT name variants ----
    let names: [(&str, &str, u16, u16); 4] = [
        ("in the spirit and power of elijah", "Luke", 1, 17),
        ("behold i will send you elias the prophet", "Malachi", 4, 5),
        ("spoken of by the prophet isaiah", "Matthew", 3, 3),
        ("as jonah was three days and three nights", "Matthew", 12, 40),
    ];
    let mut g7 = Score::new();
    for (said, name, c, v) in names {
        let want = lbl(&b, name, c, v);
        let ok = hit(&confirm_labels(&b, said), &want) || ranked_labels(&b, said).iter().take(3).any(|l| l == &want);
        g7.add(ok);
    }

    // ---- G8: verse-anchored repair (WER before vs after) ----
    let luke_pass = Passage::single(bk(&b, "Luke") as u8, 1, 17);
    let ps23 = Passage::single(bk(&b, "Psalms") as u8, 23, 1);
    let john316 = Passage::single(bk(&b, "John") as u8, 3, 16);
    let repairs: [(&str, &Passage); 5] = [
        ("and the disobedient to the gods", &luke_pass),
        ("turn the heart of the father to the children", &luke_pass),
        ("the lord is my helper i shall not want", &ps23),
        ("for god so loved the world that he gave his son", &john316),
        ("behold i come quickly and my reward is with me", &Passage::single(bk(&b, "Revelation") as u8, 22, 12)),
    ];
    let mut g8 = Score::new();
    let mut wer_before_sum = 0.0f64;
    let mut wer_after_sum = 0.0f64;
    let mut g8_n = 0usize;
    // WER against the verse's own SPAN the line sits in (a segment line can
    // never match a whole 37-word verse; the span reference is the fair one)
    let span_reference = |p: &Passage, line: &str| -> String {
        let vt = b.passage(p.book as usize, p.chapter as usize, p.from as usize, p.to as usize);
        let vw = rf_bible::normalize_words(&vt);
        let hw = rf_bible::normalize_words(line);
        // local greedy-earliest span (identical code in both trees, so the
        // before/after reference is the same measurement)
        if let Some((first, last)) = local_span(&vw, &hw) {
            vw[first..=last].join(" ")
        } else {
            vw.join(" ")
        }
    };
    for (heard, p) in repairs {
        let wer_raw = word_error_rate(heard, &span_reference(p, heard)).rate();
        let repaired = repair_line(&b, p, heard);
        let (wer_rep, made_worse) = match &repaired {
            Some(r) => {
                let w = word_error_rate(&r.text, &span_reference(p, &r.text)).rate();
                (w, w > wer_raw + 1e-9)
            }
            // no repair applied: the line goes to screen as heard
            None => (wer_raw, false),
        };
        wer_before_sum += wer_raw;
        wer_after_sum += wer_rep;
        g8_n += 1;
        g8.add(!made_worse && wer_rep < wer_raw + 1e-9);
    }
    let wer_before = if g8_n > 0 { wer_before_sum / g8_n as f64 } else { 0.0 };
    let wer_after = if g8_n > 0 { wer_after_sum / g8_n as f64 } else { 0.0 };
    // repair group score = 100 * (1 - after-WER), floored at 0
    let g8_score = (100.0 * (1.0 - wer_after)).max(0.0);

    // ---- G9: pulpit chatter must never take the stage ----
    let chatter: [&str; 10] = [
        "we are going to have a barbecue after the service",
        "and he told them the story about the fisherman and his net again",
        "and he gave his only horse away last spring",
        "the wisdom of the elders is worth remembering every single day",
        "i think we should pray before we eat",
        "turn the page back for me please",
        "at nine in the morning we started the drive",
        "we have a meeting after church today",
        "thank you all for coming out this evening",
        "let us bow our heads and pray together",
    ];
    let mut g9 = Score::new();
    for said in chatter {
        // the line must stage nothing on its own: silent, or a card that
        // cannot auto-approve (alternatives present, or evidence below 60)
        let out = parse_full(&b, said, &ParseContext::default(), 0.75, &[], None);
        let auto = out.confirms.iter().any(|d| {
            d.kind == rf_core::parser::DetectionKind::Quotation
                && d.alternatives.is_empty()
                && (d.pct == 0 || d.pct >= 60)
        });
        g9.add(out.confirms.is_empty() || !auto);
    }

    // ---- composite ----
    let composite = 0.20 * g1.pct()
        + 0.15 * g2.pct()
        + 0.10 * g3.pct()
        + 0.10 * g4.pct()
        + 0.10 * g5.pct()
        + 0.15 * g6.pct()
        + 0.05 * g7.pct()
        + 0.10 * g8_score
        + 0.05 * g9.pct();

    let json = format!(
        "{{\n  \"groups\": {{\n    \"G1_clean_read\": {{\"n\": {}, \"correct\": {}, \"pct\": {:.1}}},\n    \"G2_noise_garble\": {{\"n\": {}, \"correct\": {}, \"pct\": {:.1}}},\n    \"G3_music_tags\": {{\"n\": {}, \"correct\": {}, \"pct\": {:.1}}},\n    \"G4_short_distinctive\": {{\"n\": {}, \"correct\": {}, \"pct\": {:.1}}},\n    \"G5_tail_fragments_silent\": {{\"n\": {}, \"correct\": {}, \"pct\": {:.1}}},\n    \"G6_quote_overlap_lock\": {{\"n\": {}, \"correct\": {}, \"pct\": {:.1}}},\n    \"G7_name_variants\": {{\"n\": {}, \"correct\": {}, \"pct\": {:.1}}},\n    \"G8_gap_fill\": {{\"n\": {}, \"repairs_not_worse\": {}, \"wer_before\": {:.3}, \"wer_after\": {:.3}, \"pct\": {:.1}}},\n    \"G9_chatter_silent\": {{\"n\": {}, \"correct\": {}, \"pct\": {:.1}}}\n  }},\n  \"listening_quality_composite\": {:.1}\n}}\n",
        g1.n, g1.correct, g1.pct(),
        g2.n, g2.correct, g2.pct(),
        g3.n, g3.correct, g3.pct(),
        g4.n, g4.correct, g4.pct(),
        g5.n, g5.correct, g5.pct(),
        g6.n, g6.correct, g6.pct(),
        g7.n, g7.correct, g7.pct(),
        g8.n, g8.correct, wer_before, wer_after, g8_score,
        g9.n, g9.correct, g9.pct(),
        composite
    );
    println!("{json}");
    if let Some(path) = args.get(1) {
        let _ = std::fs::write(path, &json);
    }
}

/// Anchored best-span alignment of hw inside vw — a self-contained port of
/// Bible::span_align's core (every matching (heard, verse) position pair
/// seeds one greedy in-order extension; the span with the best
/// matched-words x F1 wins). Defined LOCALLY so the before/after trees
/// measure with byte-identical reference extraction.
fn local_span(vw: &[String], hw: &[String]) -> Option<(usize, usize)> {
    let min_pairs = ((hw.len() * 34 + 99) / 100).max(2);
    let mut best: Option<(usize, usize, usize, usize)> = None; // (score, lcs, first, last)
    for i0 in 0..hw.len() {
        for v0 in 0..vw.len() {
            if hw[i0] != vw[v0] {
                continue;
            }
            let mut pairs: Vec<usize> = Vec::new();
            let mut j = v0;
            let mut last_v = v0;
            for h in &hw[i0..] {
                let mut k = j;
                while k < vw.len() && &vw[k] != h {
                    k += 1;
                }
                if k < vw.len() {
                    pairs.push(k);
                    last_v = k;
                    j = k + 1;
                }
            }
            if pairs.len() < min_pairs {
                continue;
            }
            let lcs = pairs.len();
            let span = (last_v - v0 + 1) as f32;
            let p = lcs as f32 / (hw.len() - i0) as f32;
            let r = lcs as f32 / span;
            let mut f1 = if p + r > 0.0 { 2.0 * p * r / (p + r) } else { 0.0 };
            let run = {
                let mut br = 1usize;
                let mut cur = 1usize;
                for w in pairs.windows(2) {
                    if w[1] == w[0] + 1 {
                        cur += 1;
                        br = br.max(cur);
                    } else {
                        cur = 1;
                    }
                }
                br
            };
            f1 *= 0.7 + 0.3 * run as f32 / lcs as f32;
            let pct = (f1 * 100.0).round() as usize;
            let score = lcs * pct;
            if best.map_or(true, |(bs, _, _, _)| score > bs) {
                best = Some((score, lcs, v0, last_v));
            }
        }
    }
    best.map(|(_, lcs, first, last)| (first, last)).filter(|_| best.unwrap().1 >= 2)
}
