//! biblegrid — v0.856.0 whole-Bible false-show grid for the auto-approve rule.
//!
//! WHY: the operator reported Proverbs 19:21 falsely showing. Root cause
//! (measured with `prov1921` example): "there are many devices" — FOUR
//! ordinary English words, the verse's own opening — scores 89% and the
//! clear-winner auto-approve rule (>=75, 12 points ahead) puts it straight
//! on the stage. The 3-word tier is NOT the culprit (3-word probes on this
//! verse stay silent). So the grid sweeps the two knobs that govern the
//! false-show class:
//!   W = minimum words an utterance needs before it may AUTO-STAGE
//!   A = the accuracy bar an auto-stage must clear
//! over EVERY book, chapter and verse of the embedded KJV.
//!
//! Negative surface (false shows): every contiguous 4/5/6-word window of
//! every verse, deduplicated — words a preacher can say IN PASSING without
//! quoting. Anything that auto-stages here is a false show by construction.
//!
//! Positive surface (must keep working): every verse read from its opening
//! (first <=12 words) and every short verse read whole — 31,102 verses.
//!
//! Run:  cargo run --release -p rf-core --example biblegrid -- out.json

use rf_bible::Bible;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Res {
    top: Option<u8>,
    second: Option<u8>,
    /// ranked was empty but the unique-window tier matched (pct==0 path)
    unique: bool,
}

#[derive(Clone)]
struct ConfigRow {
    w: u8,
    a: u8,
    shipped_before: bool,
    /// negative surface: unique windows scanned
    negs: usize,
    /// windows that would AUTO-STAGE (a false show by construction)
    fp: usize,
    fp_per_1k: f64,
    /// positives: verses whose own opening still AUTO-STAGES
    tp_staged: usize,
    /// positives: right verse identified but waits as a card
    tp_carded: usize,
    /// positives: right verse NOT identified (top hit elsewhere or nothing)
    tp_missed: usize,
    /// positives: the true quote AUTO-STAGED the WRONG verse
    tp_misfire: usize,
    accuracy_score: f64,
    identify_score: f64,
    composite: f64,
    rank: usize,
    /// Proverbs 19:21 acceptance: false fragments that still auto-stage (must be 0)
    prov_false_stage: usize,
    /// Proverbs 19:21 acceptance: true reads that no longer stage OR card (must be 0)
    prov_true_lost: usize,
}

/// (text, words) of a false fragment that must never auto-stage again,
/// and true reads that must still stage or card.
const PROV_FALSE: &[(&str, usize)] = &[
    ("there are many devices", 4),
    ("there are many devices here today", 6),
    ("there are many devices in this room", 7),
    ("there are many devices on my desk", 7),
];
const PROV_WATCH: &[(&str, usize)] = &[
    // mash-ups / allusions: the person decides, but they must not stage
    // unless the evidence really is clear-winner grade
    ("the counsel of the lord shall stand", 7),
    ("many plans in a man's heart nevertheless the counsel of the lord", 12),
];
const PROV_TRUE: &[&str] = &[
    "there are many devices in a man's heart",
    "nevertheless the counsel of the lord that shall stand",
    "there are many devices in a man's heart nevertheless the counsel of the lord that shall stand",
];

fn main() {
    let out_path = std::env::args().nth(1).unwrap_or_else(|| "grid.json".into());
    let t0 = std::time::Instant::now();
    let bib = Bible::embedded();

    // ------------------------------------------------------------------
    // 1. negative surface: every 4/5/6-word window of every verse, deduped.
    //    Keep the first source book of each window for the per-book report.
    // ------------------------------------------------------------------
    let mut neg_map: std::collections::HashMap<String, (u8, u8)> = HashMap::new(); // text -> (n_words, first_book)
    let total_windows = &AtomicUsize::new(0);
    for b in 0..rf_bible::BOOK_COUNT {
        for c in 1..=bib.chapter_count(b) {
            for v in 1..=bib.verse_count(b, c) {
                let words = rf_bible::normalize_words(bib.verse(b, c, v));
                if words.len() < 4 {
                    continue;
                }
                for n in [4usize, 5, 6] {
                    if words.len() < n {
                        break;
                    }
                    total_windows.fetch_add(1, Ordering::Relaxed);
                    for i in 0..=words.len() - n {
                        let text = words[i..i + n].join(" ");
                        neg_map.entry(text).or_insert((n as u8, b as u8));
                    }
                }
            }
        }
    }
    let negatives_raw_windows = total_windows.load(Ordering::Relaxed);
    let mut negs: Vec<(String, u8, u8)> = neg_map.into_iter().map(|(t, (n, b))| (t, n, b)).collect();
    negs.sort();
    eprintln!(
        "[{}] negatives: {} unique windows (from {} raw) in {:?}",
        t0.elapsed().as_secs_f32(),
        negs.len(),
        total_windows.load(Ordering::Relaxed),
        t0.elapsed()
    );

    // ------------------------------------------------------------------
    // 2. positive surface: every verse read from its opening (<=12 words);
    //    verses of <=14 words also read whole.
    // ------------------------------------------------------------------
    struct Pos {
        book: u8,
        chapter: u16,
        verse: u16,
        text: String,
        n: u8,
    }
    let mut poss: Vec<Pos> = Vec::with_capacity(70_000);
    for b in 0..rf_bible::BOOK_COUNT {
        for c in 1..=bib.chapter_count(b) {
            for v in 1..=bib.verse_count(b, c) {
                let words = rf_bible::normalize_words(bib.verse(b, c, v));
                if words.is_empty() {
                    continue;
                }
                let take = words.len().min(12);
                let push = |w: &[String], poss: &mut Vec<Pos>| {
                    if w.len() < 3 {
                        return; // too short to be a quotation utterance at all
                    }
                    poss.push(Pos {
                        book: b as u8,
                        chapter: c as u16,
                        verse: v as u16,
                        text: w.join(" "),
                        n: w.len() as u8,
                    });
                };
                push(&words[..take], &mut poss);
                if words.len() <= 14 && words.len() != take {
                    push(&words, &mut poss);
                }
            }
        }
    }
    eprintln!("[{}] positives: {} verse-read utterances", t0.elapsed().as_secs_f32(), poss.len());

    // ------------------------------------------------------------------
    // 3. one threaded pass: ranked results cached for every utterance.
    //    (The config gates are applied to this cache below — the matcher's
    //    own output does not depend on W/A, so one pass serves all configs.)
    // ------------------------------------------------------------------
    fn run_chunk(bib: &'static Bible, items: &[String]) -> Vec<Res> {
        items
            .iter()
            .map(|t| {
                let hits = bib.match_quotations_ranked(t, 8);
                if hits.is_empty() {
                    Res { top: None, second: None, unique: bib.match_quotation(t).is_some() }
                } else {
                    Res { top: Some(hits[0].pct), second: hits.get(1).map(|h| h.pct), unique: false }
                }
            })
            .collect()
    }

    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let neg_texts: Vec<String> = negs.iter().map(|(t, _, _)| t.clone()).collect();
    let neg_res: Vec<Res> = parallel_map(&neg_texts, threads, |chunk| run_chunk(bib, chunk));

    let pos_texts: Vec<String> = poss.iter().map(|p| p.text.clone()).collect();
    let pos_res: Vec<Res> = parallel_map(&pos_texts, threads, |chunk| run_chunk(bib, chunk));
    eprintln!("[{}] matcher pass done ({})", t0.elapsed().as_secs_f32(), threads);

    // own-verse flag, computed ONCE per positive (top ranked hit is the verse
    // being read, or the unique-window tier lands on it)
    let pos_own: Vec<bool> = poss
        .iter()
        .zip(pos_res.iter())
        .map(|(p, r)| {
            if r.unique {
                unique_is_own(bib, &p.text, p.book, p.chapter, p.verse)
            } else if r.top.is_some() {
                bib_top_is_own(bib, &p.text, p.book, p.chapter, p.verse)
            } else {
                false
            }
        })
        .collect();
    eprintln!("[{}] own-verse pass done", t0.elapsed().as_secs_f32());

    // ------------------------------------------------------------------
    // 4. the grid. auto() mirrors rf-app/state.rs::should_auto_approve with
    //    knobs W (min words to stage) and A (single- and multi-candidate bar;
    //    the multi case keeps its 12-point clear-winner lead).
    // ------------------------------------------------------------------
    #[inline]
    fn auto_stage(r: &Res, n: u8, w: u8, a: u8, single_bar: u8, multi_bar: u8) -> bool {
        if n < w {
            return false;
        }
        match (r.top, r.second) {
            (None, _) => r.unique, // exact-text unique tier: pct==0 -> passes any bar
            (Some(t), None) => t >= single_bar,
            (Some(t), Some(s)) => t >= multi_bar && t.saturating_sub(s) >= 12,
        }
    }

    let mut rows: Vec<(u8, u8, bool, u8, u8)> = Vec::new();
    // the shipped v0.855 rule first (the BEFORE row)
    rows.push((3, 0u8, true, 60u8, 75u8));
    for w in [3u8, 4, 5] {
        for a in [75u8, 80, 85, 90] {
            rows.push((w, a, false, a, a));
        }
    }

    let mut table: Vec<ConfigRow> = Vec::with_capacity(rows.len());
    for &(w, a, shipped, single_bar, multi_bar) in &rows {
        let mut fp = 0usize;
        for (i, (_, n, _book)) in negs.iter().enumerate() {
            if auto_stage(&neg_res[i], *n, w, a, single_bar, multi_bar) {
                fp += 1;
            }
        }
        let (mut staged, mut carded, mut missed, mut misfire) = (0usize, 0, 0, 0);
        for (i, p) in poss.iter().enumerate() {
            let would = auto_stage(&pos_res[i], p.n, w, a, single_bar, multi_bar);
            if pos_own[i] {
                if would {
                    staged += 1;
                } else {
                    carded += 1;
                }
            } else {
                missed += 1;
                if would {
                    misfire += 1;
                }
            }
        }

        // Proverbs 19:21 acceptance
        let mut prov_false_stage = 0usize;
        for (text, n) in PROV_FALSE {
            let r = match_quot_one(bib, text);
            if auto_stage(&r, *n as u8, w, a, single_bar, multi_bar) {
                prov_false_stage += 1;
            }
        }
        let mut prov_true_lost = 0usize;
        for text in PROV_TRUE {
            let n = text.split_whitespace().count() as u8;
            let r = match_quot_one(bib, text);
            let identified = r.top.is_some() || r.unique;
            if !identified {
                prov_true_lost += 1;
            }
        }

        table.push(ConfigRow {
            w,
            a,
            shipped_before: shipped,
            negs: negs.len(),
            fp,
            fp_per_1k: fp as f64 * 1000.0 / negs.len() as f64,
            tp_staged: staged,
            tp_carded: carded,
            tp_missed: missed,
            tp_misfire: misfire,
            accuracy_score: 0.0,
            identify_score: 0.0,
            composite: 0.0,
            rank: 0,
            prov_false_stage,
            prov_true_lost,
        });
    }
    // ---- scores: accuracy first (70%), identification second (30%) ----
    let worst_fp = table.iter().map(|r| r.fp_per_1k).fold(0.0_f64, f64::max);
    for r in &mut table {
        r.accuracy_score = 100.0 * (1.0 - r.fp_per_1k / worst_fp);
        r.identify_score = 100.0 * (r.tp_staged + r.tp_carded) as f64 / poss.len() as f64;
        r.composite = 0.7 * r.accuracy_score + 0.3 * r.identify_score;
    }
    let mut order: Vec<usize> = (0..table.len()).collect();
    order.sort_by(|&i, &j| {
        table[j]
            .composite
            .partial_cmp(&table[i].composite)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (pos, &i) in order.iter().enumerate() {
        table[i].rank = pos + 1;
    }

    // ---- per-book false-show counts for the BEFORE row and the winner ----
    let before = &table[0];
    let best = &table[order[0]];
    let mut per_book_before = vec![0usize; rf_bible::BOOK_COUNT];
    let mut per_book_best = vec![0usize; rf_bible::BOOK_COUNT];
    for (i, (_, n, b)) in negs.iter().enumerate() {
        if auto_stage(&neg_res[i], *n, before.w, 0, 60, 75) {
            per_book_before[*b as usize] += 1;
        }
        if auto_stage(&neg_res[i], *n, best.w, best.a, best.a, best.a) {
            per_book_best[*b as usize] += 1;
        }
    }

    // ---- JSON out ----
    fn crow(r: &ConfigRow) -> serde_json::Value {
        serde_json::json!({
            "w": r.w, "a": r.a, "shipped_before": r.shipped_before,
            "fp": r.fp, "fp_per_1k": r.fp_per_1k,
            "tp_staged": r.tp_staged, "tp_carded": r.tp_carded,
            "tp_missed": r.tp_missed, "tp_misfire": r.tp_misfire,
            "accuracy_score": r.accuracy_score, "identify_score": r.identify_score,
            "composite": r.composite, "rank": r.rank,
            "prov_false_stage": r.prov_false_stage, "prov_true_lost": r.prov_true_lost,
        })
    }
    let prov_watch_out: Vec<(String, usize, bool)> = PROV_WATCH
        .iter()
        .map(|(t, n)| {
            let r = match_quot_one(bib, t);
            (t.to_string(), *n, auto_stage(&r, *n as u8, best.w, best.a, best.a, best.a))
        })
        .collect();
    let out = serde_json::json!({
        "version": "0.856.0",
        "negatives_unique": negs.len(),
        "negatives_raw_windows": negatives_raw_windows,
        "positives": poss.len(),
        "elapsed_secs": t0.elapsed().as_secs_f64(),
        "rows": table.iter().map(crow).collect::<Vec<_>>(),
        "prov_false_watch": prov_watch_out,
        "per_book_before": per_book_before.iter().enumerate().map(|(b, n)| (bib.book_name(b), *n)).collect::<Vec<_>>(),
        "per_book_best": per_book_best.iter().enumerate().map(|(b, n)| (bib.book_name(b), *n)).collect::<Vec<_>>(),
        "winner": crow(best),
    });
    std::fs::write(&out_path, serde_json::to_string_pretty(&out).unwrap()).unwrap();

    // ---- console table ----
    println!(
        "negatives {} unique windows (raw {}) | positives {} verse reads | {:?}\n",
        negs.len(),
        negatives_raw_windows,
        poss.len(),
        t0.elapsed()
    );
    println!(
        "{:>4} {:>3} {:>3} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>7} {:>5} {:>5}",
        "rank", "W", "A", "FP", "FP/1k", "acc", "staged", "carded", "missed", "misfire", "ident", "pF", "pT"
    );
    for r in &table {
        println!(
            "{:>4} {:>3} {:>3} {:>8} {:>8.2} {:>7.1}% {:>8} {:>8} {:>8} {:>8} {:>6.1}% {:>5} {:>5} {}",
            r.rank,
            r.w,
            if r.shipped_before { 75 } else { r.a },
            r.fp,
            r.fp_per_1k,
            r.accuracy_score,
            r.tp_staged,
            r.tp_carded,
            r.tp_missed,
            r.tp_misfire,
            r.identify_score,
            r.prov_false_stage,
            r.prov_true_lost,
            if r.shipped_before { "<= SHIPPED v0.855" } else { "" },
        );
    }
    println!(
        "\nWINNER: rank {} = W={} A={} (FP {} -> {}, staged {} carded {})",
        best.rank, best.w, if best.shipped_before { 75 } else { best.a }, before.fp, best.fp, best.tp_staged, best.tp_carded
    );
}

fn parallel_map<T, R, F>(items: &[T], threads: usize, f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&[T]) -> Vec<R> + Sync + Send + Clone + 'static,
{
    if items.len() < 4000 || threads <= 1 {
        return f(items);
    }
    let chunk = items.len().div_ceil(threads);
    std::thread::scope(|s| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|c| {
                let f = f.clone();
                s.spawn(move || f(c))
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

#[allow(dead_code)]
fn bib_top_is_own(bib: &'static Bible, text: &str, b: u8, c: u16, v: u16) -> bool {
    let hits = bib.match_quotations_ranked(text, 1);
    hits.first().map_or(false, |h| h.book == b as usize && h.chapter == c as usize && h.verse == v as usize)
}

#[allow(dead_code)]
fn unique_is_own(bib: &'static Bible, text: &str, b: u8, c: u16, v: u16) -> bool {
    bib.match_quotation(text)
        .map_or(false, |(hb, hc, hv)| hb == b as usize && hc == c as usize && hv == v as usize)
}

fn match_quot_one(bib: &'static Bible, text: &str) -> Res {
    let hits = bib.match_quotations_ranked(text, 8);
    if hits.is_empty() {
        Res { top: None, second: None, unique: bib.match_quotation(text).is_some() }
    } else {
        Res { top: Some(hits[0].pct), second: hits.get(1).map(|h| h.pct), unique: false }
    }
}

use std::collections::HashMap;
