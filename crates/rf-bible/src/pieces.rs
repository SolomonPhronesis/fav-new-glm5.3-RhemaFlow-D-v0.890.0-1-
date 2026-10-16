//! Quotations said in short, garbled pieces (v0.816.0).
//!
//! A speech engine that cuts at every breath hands us "If any man being Christ." / "He is." /
//! "A new." / "Creation" ... for 2 Corinthians 5:17. No single piece is a quotation, the pieces
//! have wrong words ("being" for "be in", "creation" for "creature"), and the older matcher only
//! recognised a verse by its OPENING words. This one:
//!
//!  1. joins the newest pieces together (every "last k pieces" window is tried, so junk spoken
//!     earlier cannot spoil it),
//!  2. finds candidate verses by word triples ANYWHERE in a verse (an index built once, lazily),
//!  3. scores each candidate by the longest in-order run of alike words (forgiving about
//!     near-spellings), counting both words missing and words extra, and
//!  4. refuses anything that is not clearly a quotation (at least 6 words in order, a run of 3
//!     together, and a good score), so ordinary talk is left alone.

use super::{index_range, key_hash, normalize_words, pack, unpack, Bible, QuoteHit};

/// What `match_pieces` found.
#[derive(Clone, Debug, Default)]
pub struct PieceMatch {
    /// Best first, 0-100 accuracy each.
    pub hits: Vec<QuoteHit>,
    /// How many of the newest pieces the best match was built from.
    pub pieces_used: usize,
}

/// Fewest in-order matching words before we believe anything.
const MIN_MATCHED_WORDS: usize = 6;
/// ...and at least this many of them side by side.
const MIN_RUN: usize = 3;
/// Lowest accuracy offered at all.
pub const MIN_PIECES_PCT: u8 = 60;
/// A trigram that occurs in more verses than this carries no information ("and the lord said").
const COMMON_TRIGRAM: usize = 60;
const MAX_CANDIDATES: usize = 16;
/// Never look at more than this many words of what was heard.
const MAX_HEARD: usize = 60;

/// Two spoken/written words that probably mean the same word: equal, or close in spelling.
/// Short words must be exactly equal (man / men, god / good are different words).
pub fn words_alike(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (la, lb) = (a.chars().count(), b.chars().count());
    if la < 5 || lb < 5 {
        return false;
    }
    let prefix = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    if prefix >= 5 && la.abs_diff(lb) <= 3 {
        return true; // creation / creature
    }
    edit_distance_at_most(a, b, if la.min(lb) >= 7 { 2 } else { 1 })
}

fn edit_distance_at_most(a: &str, b: &str, limit: usize) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > limit {
        return false;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, y) in b.iter().enumerate() {
            cur.push((prev[j] + usize::from(x != y)).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()] <= limit
}

/// Words with the leftovers of apostrophes taken out ("I'm" is "i" + "m").
fn core_words(text: &str) -> Vec<String> {
    normalize_words(text).into_iter().filter(|w| !matches!(w.as_str(), "m" | "ll" | "re" | "ve" | "d" | "s" | "t")).collect()
}

struct Alignment {
    matched: usize,
    first: usize,
    last: usize,
    longest_run: usize,
}

/// Longest in-order common words (alike words count), and where in the verse they sit.
fn align(heard: &[String], verse: &[String]) -> Alignment {
    let (m, n) = (heard.len(), verse.len());
    let mut dp = vec![vec![0u16; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if words_alike(&heard[i - 1], &verse[j - 1]) { dp[i - 1][j - 1] + 1 } else { dp[i - 1][j].max(dp[i][j - 1]) };
        }
    }
    let (mut i, mut j) = (m, n);
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    while i > 0 && j > 0 {
        if dp[i][j] == dp[i - 1][j - 1] + 1 && words_alike(&heard[i - 1], &verse[j - 1]) {
            pairs.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    pairs.reverse();
    let mut longest = usize::from(!pairs.is_empty());
    let mut run = 1;
    for w in pairs.windows(2) {
        run = if w[1].0 == w[0].0 + 1 && w[1].1 == w[0].1 + 1 { run + 1 } else { 1 };
        longest = longest.max(run);
    }
    Alignment { matched: pairs.len(), first: pairs.first().map(|p| p.1).unwrap_or(0), last: pairs.last().map(|p| p.1).unwrap_or(0), longest_run: longest }
}

impl Bible {
    /// Build the lazily-made indexes now (call from a background thread at start-up so the first
    /// garbled quotation does not pause the screen).
    pub fn warm(&self) {
        let _ = self.tri_index();
        let _ = self.flat_verses();
    }

    /// Sorted (hash of three words in a row, verse id), one entry per verse per distinct triple.
    fn tri_index(&self) -> &Vec<(u64, u32)> {
        self.tri.get_or_init(|| {
            let mut out: Vec<(u64, u32)> = Vec::with_capacity(self.total_verses() * 20);
            for (bi, book) in self.books.iter().enumerate() {
                for (ci, chapter) in book.chapters.iter().enumerate() {
                    for (vi, text) in chapter.iter().enumerate() {
                        let id = pack(bi as u32, ci as u32, vi as u32);
                        let w = core_words(text);
                        let mut seen: Vec<u64> = Vec::new();
                        for t in w.windows(3) {
                            let h = key_hash(100, t);
                            if !seen.contains(&h) {
                                seen.push(h);
                                out.push((h, id));
                            }
                        }
                    }
                }
            }
            out.sort_unstable();
            out.shrink_to_fit();
            out
        })
    }

    /// Best verses for ONE run of heard words (already cleaned), or empty.
    fn match_heard(&self, heard: &[String], max_results: usize) -> Vec<QuoteHit> {
        if heard.len() < MIN_MATCHED_WORDS {
            return Vec::new();
        }
        let idx = self.tri_index();
        // 1. candidates: verses sharing word triples with what was heard
        let mut votes: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for t in heard.windows(3) {
            let r = index_range(idx, key_hash(100, t));
            if r.is_empty() || r.len() > COMMON_TRIGRAM {
                continue;
            }
            for e in r {
                *votes.entry(e.1).or_insert(0) += 1;
            }
        }
        let mut cands: Vec<(u32, u32)> = votes.into_iter().filter(|(_, v)| *v >= 2).collect();
        cands.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        cands.truncate(MAX_CANDIDATES);
        // 2. score each one against what was heard
        let mut hits: Vec<QuoteHit> = Vec::new();
        for (id, _) in cands {
            let (b, c, v) = unpack(id);
            let Some(text) = self.books.get(b).and_then(|bk| bk.chapters.get(c)).and_then(|ch| ch.get(v)) else { continue };
            let verse = core_words(text);
            if verse.is_empty() {
                continue;
            }
            let a = align(heard, &verse);
            if a.matched < MIN_MATCHED_WORDS || a.longest_run < MIN_RUN {
                continue;
            }
            let span = a.last - a.first + 1;
            let p = a.matched as f32 / heard.len() as f32;
            let r = a.matched as f32 / span as f32;
            let f1 = 2.0 * p * r / (p + r);
            // a verse that is mostly unsaid is a weaker claim than one that was quoted in full
            let cover = span as f32 / verse.len() as f32;
            let factor = if cover >= 0.6 { 1.0 } else { 0.88 + 0.12 * (cover / 0.6) };
            let pct = ((f1 * factor * 100.0).round() as u8).min(99);
            if pct >= MIN_PIECES_PCT {
                hits.push(QuoteHit { book: b, chapter: c + 1, verse: v + 1, pct });
            }
        }
        hits.sort_by(|a, b| b.pct.cmp(&a.pct).then(a.book.cmp(&b.book)).then(a.chapter.cmp(&b.chapter)).then(a.verse.cmp(&b.verse)));
        hits.truncate(max_results);
        hits
    }

    /// `pieces` = what was heard, oldest first; the LAST one is the newest. Tries the newest piece
    /// alone, then the newest two, three... and keeps the window that explains most of a verse.
    pub fn match_pieces(&self, pieces: &[&str], max_results: usize) -> PieceMatch {
        if pieces.is_empty() || max_results == 0 {
            return PieceMatch::default();
        }
        let cleaned: Vec<Vec<String>> = pieces.iter().map(|p| core_words(p)).collect();
        let mut best = PieceMatch::default();
        for k in 1..=cleaned.len() {
            let mut heard: Vec<String> = cleaned[cleaned.len() - k..].iter().flatten().cloned().collect();
            if heard.len() > MAX_HEARD {
                heard.drain(..heard.len() - MAX_HEARD);
            }
            let hits = self.match_heard(&heard, max_results);
            let better = match (hits.first(), best.hits.first()) {
                (Some(h), Some(b)) => h.pct > b.pct, // a tie keeps the window with fewer pieces
                (Some(_), None) => true,
                _ => false,
            };
            if better {
                best = PieceMatch { hits, pieces_used: k };
            }
        }
        best
    }
}
