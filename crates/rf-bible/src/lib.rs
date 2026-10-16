//! rf-bible — the embedded, build-validated King James Version (31,102 verses).
//!
//! The full KJV text ships inside the binary (`assets/kjv.json`, include_str!).
//! The dataset is validated at build time by `scripts/build_kjv.py` against
//! canonical per-book chapter/verse counts; `Bible::load()` re-checks the
//! totals at startup so a corrupted asset can never ship silently.
//!
//! Public-domain text. No network access is ever required.

use serde::Deserialize;

pub const BOOK_COUNT: usize = 66;
pub const EXPECTED_TOTAL_VERSES: usize = 31_102;

/// Lowest accuracy a candidate from the normal tiers may be offered at
/// (the parser applies its own floors on top).
pub const MIN_TIER_PCT: u8 = 50;
/// v0.853.0: candidates from the weakest tier (opening-trigram probes on a
/// fragment shorter than anything else) must clear a higher bar before they
/// are offered at all — a 3-word probe is a weak claim.
pub const SHORT_TIER_PCT: u8 = 70;

// ---------------------------------------------------------------------------
// v0.890.0 — filler tails (ported from v0.889, floor adapted to the
// adjustable word settings)
// ---------------------------------------------------------------------------

/// Spoken tail words a speaker adds AFTER a quotation ("...able to make you
/// wise. So." / "...Amen." / "...okay"). They are never the evidence for a
/// verse, and one of them used to be the difference between a card that
/// clears the bar and one that sits under it.
const FILLER_TAIL: &[&str] = &["so", "amen", "okay", "ok", "alright", "right", "yeah", "yes", "hallelujah", "praise"];

/// Never strip below this many words: the exact-window tier's own floor
/// (v0.856) — a shorter line has nothing left to score once the filler is
/// gone. (v0.889 used 5 because its whole matcher stood on 5 words; v0.890's
/// word floor is a setting, so the strip keeps the structural 4-word floor.)
const FILLER_FLOOR_WORDS: usize = 4;

/// `text` without trailing filler words (at most three removed, and never
/// below [`FILLER_FLOOR_WORDS`] words). Returns the original text when
/// nothing qualifies. Matching only — display text is never rewritten by
/// this: the transcript keeps exactly what the engine heard.
pub fn strip_filler_tail(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let norm = |w: &str| -> String { w.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect() };
    let mut end = words.len();
    let mut cut = 0;
    while end > FILLER_FLOOR_WORDS && cut < 3 && FILLER_TAIL.contains(&norm(words[end - 1]).as_str()) {
        end -= 1;
        cut += 1;
    }
    if cut == 0 {
        text.to_string()
    } else {
        words[..end].join(" ")
    }
}

/// How much a WELL_KNOWN verse gains in the ranked scorer (v0.853.0 late
/// fix). When several verses tie on text evidence, the one congregations
/// actually mean most often must lead the card: "the Lord is" opens Exodus
/// 15:2, Numbers 14:18 and a dozen Psalms at the same 86%, but Psalm 23:1 is
/// what the operator means; "behold I come quickly" textually opens
/// Revelation 22:7, but Revelation 22:12 ("and, behold, I come quickly; and
/// my reward is with me") is the verse being quoted. The bonus is applied
/// only on top of real text evidence — it breaks ties and near-ties, it
/// never rescues a weak match past the floors.
pub const FAME_BONUS: u8 = 12;

/// Best-loved verses of the congregation, (book name, chapter, verse).
/// Names must match the BOOKS table exactly.
const WELL_KNOWN: &[(&str, u16, u16)] = &[
    ("Genesis", 1, 1),
    ("Psalms", 23, 1), ("Psalms", 23, 4), ("Psalms", 27, 1), ("Psalms", 46, 1),
    ("Psalms", 91, 1), ("Psalms", 118, 24), ("Psalms", 121, 1),
    ("Proverbs", 3, 5), ("Isaiah", 9, 6), ("Isaiah", 40, 31), ("Jeremiah", 29, 11),
    ("Matthew", 6, 33), ("Matthew", 11, 28), ("Matthew", 28, 19),
    ("John", 3, 16), ("John", 14, 6),
    ("Acts", 16, 31), ("Romans", 3, 23), ("Romans", 8, 28), ("Romans", 10, 9),
    ("1 Corinthians", 13, 4), ("Philippians", 2, 5), ("Philippians", 4, 13),
    ("Hebrews", 11, 1), ("Revelation", 3, 20), ("Revelation", 22, 12),
];

/// v0.853.0: a 3-word probe may only found a quotation when at least one of
/// its words is a distinctive (non stop-word, 3+ letter) word. "for God so"
/// (god) and "let this mind" (mind) qualify; "we have a" — which happens to
/// open Song of Solomon 8:8 and nothing else — is ordinary English and must
/// not raise a card or auto-stage a verse the speaker never quoted.
pub fn trigram_distinctive(w: &[String]) -> bool {
    const STOP: &[&str] = &[
        "a","an","the","and","or","but","if","of","to","in","on","at","by","for",
        "with","from","as","is","are","was","were","be","been","being","have",
        "has","had","do","does","did","will","would","shall","should","may",
        "might","must","can","could","that","this","these","those","i","you",
        "he","she","it","we","they","them","his","her","its","our","their",
        "my","your","not","no","yes","so","up","down","out","over","under",
        "again","then","than","when","what","which","who","whom","all","any",
        "both","each","more","most","some","such","only","own","same","too",
        "very","just","also","now","here","there","unto","ye","thou","us","me",
        "him",
    ];
    w.iter().any(|x| x.len() >= 3 && !STOP.contains(&x.as_str()))
}

#[derive(Deserialize)]
struct RawBible {
    books: Vec<RawBook>,
}

mod pieces;
pub use pieces::{words_alike, PieceMatch, MIN_PIECES_PCT};

#[derive(Deserialize)]
struct RawBook {
    name: String,
    chapters: Vec<Vec<String>>,
}

/// A loaded, validated KJV Bible.
pub struct Bible {
    books: Vec<BookData>,
    /// Quotation index: sorted (hash of a verse's own leading 4..=8 words, verse id).
    /// Sorted pairs rather than a HashMap<String, Vec<_>>: same lookups, a
    /// fraction of the memory (this used to cost ~70 MB per translation
    /// against the PRD's 350 MB live-service budget).
    quote_index: Vec<(u64, u32)>,
    /// Half-window index for the fuzzy quotation tier (see
    /// `match_quotation_fuzzy`): sorted (hash of one fixed segment of a
    /// verse's opening, verse id). Lets a one-word-off lookup touch a handful
    /// of candidate verses instead of scanning all 31,102.
    seg_index: Vec<(u64, u32)>,
    /// Lazily-built normalized text of every verse (space-joined words), used by
    /// the ranked quotation search to find a phrase said from the MIDDLE of a verse.
    flat: std::sync::OnceLock<Vec<(u32, String)>>,
    /// Lazily-built (hash of three words in a row anywhere in a verse, verse id): see `pieces.rs`.
    tri: std::sync::OnceLock<Vec<(u64, u32)>>,
    /// Lazily-built (hash of a verse's own FIRST three words, verse id), v0.853.0:
    /// lets a unique 3-word fragment ("for God so" -> John 3:16) and short ASR-damaged
    /// fragments ("let this mind being you") still find their verse. Verse openings are
    /// distinctive; every entry is still verified word-for-word before use.
    open_tri: std::sync::OnceLock<Vec<(u64, u32)>>,
}

/// The fixed segments of a verse's opening that the fuzzy index is keyed on:
/// for a 6/7/8-word window split into halves (0..h) and (h..n) with h = n/2.
const FUZZY_SEGMENTS: [(usize, usize); 5] = [(0, 3), (3, 3), (3, 4), (0, 4), (4, 4)];

/// FNV-1a over the words (space-separated), salted with the segment's start
/// and length so different segments/lengths never share a key.
fn key_hash(start: usize, words: &[String]) -> u64 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ ((start as u64) << 48) ^ ((words.len() as u64) << 56);
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            h = (h ^ b' ' as u64).wrapping_mul(PRIME);
        }
        for b in w.bytes() {
            h = (h ^ b as u64).wrapping_mul(PRIME);
        }
    }
    h
}

/// All ids stored under `key` in a sorted (key, id) index.
fn index_range(index: &[(u64, u32)], key: u64) -> &[(u64, u32)] {
    let lo = index.partition_point(|e| e.0 < key);
    let hi = index.partition_point(|e| e.0 <= key);
    &index[lo..hi]
}

struct BookData {
    name: String,
    chapters: Vec<Vec<String>>,
}

impl Bible {
    /// Load and validate the embedded KJV. Panics if the asset fails
    /// validation — that is a build defect, not a runtime condition.
    pub fn embedded() -> &'static Bible {
        use std::sync::OnceLock;
        static BIBLE: OnceLock<Bible> = OnceLock::new();
        BIBLE.get_or_init(|| {
            let raw = include_str!("../assets/kjv.json");
            Bible::from_str(raw).expect("embedded KJV asset must be valid")
        })
    }

    /// Load and validate the embedded Basic English Edition (public domain,
    /// v0.801.2 multi-translation engine). Its versification differs from
    /// the KJV by a couple of verses, so validation is structural rather
    /// than an exact verse count.
    pub fn embedded_bbe() -> &'static Bible {
        use std::sync::OnceLock;
        static BIBLE: OnceLock<Bible> = OnceLock::new();
        BIBLE.get_or_init(|| {
            let raw = include_str!("../assets/bbe.json");
            Bible::from_translation(raw).expect("embedded BBE asset must be valid")
        })
    }

    /// Structurally-validated load for any public-domain translation:
    /// exactly 66 canonical books, every chapter non-empty, total in a
    /// sane range. Licensed translations (NIV/AMPC) plug in here once the
    /// publisher agreement provides the JSON — same schema, same API.
    pub fn from_translation(json: &str) -> Result<Bible, String> {
        Self::from_raw(json, 30_000..=32_500)
    }

    fn from_raw(json: &str, total_range: std::ops::RangeInclusive<usize>) -> Result<Bible, String> {
        let raw: RawBible =
            serde_json::from_str(json).map_err(|e| format!("Bible parse error: {e}"))?;
        if raw.books.len() != BOOK_COUNT {
            return Err(format!(
                "Bible validation failed: {} books, expected {BOOK_COUNT}",
                raw.books.len()
            ));
        }
        let mut total = 0usize;
        for b in &raw.books {
            for (ci, c) in b.chapters.iter().enumerate() {
                if c.is_empty() {
                    return Err(format!(
                        "Bible validation failed: {} chapter {} is empty",
                        b.name,
                        ci + 1
                    ));
                }
            }
            total += b.chapters.iter().map(|c| c.len()).sum::<usize>();
        }
        if !total_range.contains(&total) {
            return Err(format!(
                "Bible validation failed: {total} verses, expected in {:?}",
                total_range
            ));
        }
        let mut bible = Bible {
            books: raw
                .books
                .into_iter()
                .map(|b| BookData {
                    name: b.name,
                    chapters: b.chapters,
                })
                .collect(),
            quote_index: Vec::new(),
            seg_index: Vec::new(),
            flat: std::sync::OnceLock::new(),
            tri: std::sync::OnceLock::new(),
            open_tri: std::sync::OnceLock::new(),
        };
        bible.build_quote_index();
        Ok(bible)
    }

    /// Build a Bible from ANY of the common JSON/text layouts (v0.812.0), so an
    /// operator can drop a translation file in the Bibles folder:
    ///  1. this project's own `{"books":[{"name","chapters":[["v1","v2"],..]}]}`
    ///  2. `[{"name","chapters":[["v1",..],..]}, ...]` (thiagobodruk/bible; book NAMES
    ///     in that repo are not always English, so only the ORDER is used)
    ///  3. `{"translation":..,"books":[{"name","chapters":[{"chapter":1,"verses":[{"verse":1,"text":".."}]}]}]}` (scrollmapper)
    ///  4. text lines `Book<TAB>Chapter<TAB>Verse<TAB>Text` (or `Book Chapter:Verse<TAB>Text`)
    /// The result must be a complete 66-book Bible; the same validation as every
    /// built-in translation applies. Verse markup (`{..}`, `<..>`) is stripped.
    pub fn from_any(text: &str) -> Result<Bible, String> {
        let t = text.trim_start_matches('\u{feff}').trim();
        let canon = canonical_book_names();
        let books: Vec<Vec<Vec<String>>> = if t.starts_with('{') || t.starts_with('[') {
            let v: serde_json::Value = serde_json::from_str(t).map_err(|e| format!("not valid JSON: {e}"))?;
            let arr = match &v {
                serde_json::Value::Array(a) => a.clone(),
                serde_json::Value::Object(o) => o.get("books").and_then(|b| b.as_array()).cloned().ok_or("JSON has no \"books\" list")?,
                _ => return Err("unrecognised JSON layout".into()),
            };
            let mut out = Vec::new();
            for b in &arr {
                let chs = b.get("chapters").and_then(|c| c.as_array()).ok_or("a book has no \"chapters\"")?;
                let mut chapters = Vec::new();
                for ch in chs {
                    let verses: Vec<String> = match ch {
                        serde_json::Value::Array(vs) => vs.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect(),
                        serde_json::Value::Object(o) => {
                            let vs = o.get("verses").and_then(|x| x.as_array()).ok_or("a chapter has no \"verses\"")?;
                            let mut sorted: Vec<(u64, String)> = vs
                                .iter()
                                .map(|x| (x.get("verse").and_then(|n| n.as_u64()).unwrap_or(0), x.get("text").and_then(|n| n.as_str()).unwrap_or("").to_string()))
                                .collect();
                            sorted.sort_by_key(|x| x.0);
                            sorted.into_iter().map(|x| x.1).collect()
                        }
                        _ => return Err("unrecognised chapter layout".into()),
                    };
                    chapters.push(verses.into_iter().map(|v| clean_verse(&v)).collect());
                }
                out.push(chapters);
            }
            out
        } else {
            Self::books_from_lines(t, &canon)?
        };
        if books.len() != BOOK_COUNT {
            return Err(format!("found {} books, a Bible needs {BOOK_COUNT}", books.len()));
        }
        let raw = serde_json::json!({
            "books": books.iter().enumerate().map(|(i, c)| serde_json::json!({"name": canon[i], "chapters": c})).collect::<Vec<_>>()
        });
        Self::from_translation(&raw.to_string())
    }

    fn books_from_lines(t: &str, canon: &[String]) -> Result<Vec<Vec<Vec<String>>>, String> {
        let mut books: Vec<Vec<Vec<String>>> = vec![Vec::new(); BOOK_COUNT];
        for (n, line) in t.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            let (book_s, ch, vs, text) = match cols.len() {
                4 => (cols[0].trim().to_string(), cols[1].trim().to_string(), cols[2].trim().to_string(), cols[3]),
                2 => {
                    // "Genesis 1:1<TAB>text"
                    let (head, tail) = cols[0].rsplit_once(' ').ok_or(format!("line {}: cannot read reference", n + 1))?;
                    let (c, v) = tail.split_once(':').ok_or(format!("line {}: expected chapter:verse", n + 1))?;
                    (head.trim().to_string(), c.to_string(), v.to_string(), cols[1])
                }
                _ => return Err(format!("line {}: expected 4 tab-separated columns or 'Book C:V<TAB>text'", n + 1)),
            };
            let b = book_index_by_name(&book_s, canon).ok_or(format!("line {}: unknown book {book_s:?}", n + 1))?;
            let c: usize = ch.parse().map_err(|_| format!("line {}: bad chapter", n + 1))?;
            let v: usize = vs.parse().map_err(|_| format!("line {}: bad verse", n + 1))?;
            if c == 0 || v == 0 || c > 200 || v > 200 {
                return Err(format!("line {}: chapter/verse out of range", n + 1));
            }
            let chapters = &mut books[b];
            while chapters.len() < c {
                chapters.push(Vec::new());
            }
            let verses = &mut chapters[c - 1];
            while verses.len() < v {
                verses.push(String::new());
            }
            verses[v - 1] = clean_verse(text);
        }
        Ok(books)
    }

    pub fn from_str(json: &str) -> Result<Bible, String> {
        // KJV keeps the strict build-time verse count (31,102).
        let b = Self::from_raw(json, EXPECTED_TOTAL_VERSES..=EXPECTED_TOTAL_VERSES)?;
        Ok(b)
    }

    /// Build the quotation index: normalized first-4..8 word keys per verse.
    ///
    /// BUG (fixed): this used to store exactly ONE key per verse — its own
    /// leading min(len,8) words — so a spoken 5-word fragment of an 8+ word
    /// verse could never match anything (the verse's only key was its full
    /// 8-word prefix, a different string). It only worked when the spoken
    /// fragment's length happened to equal the verse's own total word count.
    /// Every verse now gets one key for EACH of its own leading 4..=8 word
    /// prefixes, so a genuine 4, 5, 6, 7 or 8-word leading fragment all have
    /// a chance to match, independent of how long the full verse is.
    fn build_quote_index(&mut self) {
        for (bi, book) in self.books.iter().enumerate() {
            for (ci, chapter) in book.chapters.iter().enumerate() {
                for (vi, text) in chapter.iter().enumerate() {
                    let words = normalize_words(text);
                    if words.len() < 4 {
                        continue;
                    }
                    let max_n = words.len().min(8);
                    let id = pack(bi as u32, ci as u32, vi as u32);
                    for n in 4..=max_n {
                        self.quote_index.push((key_hash(0, &words[..n]), id));
                    }
                    if words.len() >= 6 {
                        for (st, ln) in FUZZY_SEGMENTS {
                            if st + ln <= max_n {
                                self.seg_index.push((key_hash(st, &words[st..st + ln]), id));
                            }
                        }
                    }
                }
            }
        }
        self.quote_index.sort_unstable();
        self.quote_index.shrink_to_fit();
        self.seg_index.sort_unstable();
        self.seg_index.shrink_to_fit();
    }

    /// The first `n` normalized words of a verse, re-derived on demand (only
    /// ever for the handful of index hits being verified — storing them for
    /// every verse cost far too much memory).
    fn verse_lead(&self, id: u32, n: usize) -> Option<Vec<String>> {
        let (b, c, v) = unpack(id);
        let text = self.books.get(b)?.chapters.get(c)?.get(v)?;
        let mut w = normalize_words(text);
        (w.len() >= n).then(|| {
            w.truncate(n);
            w
        })
    }

    /// Sorted (hash of a verse's FIRST three words, verse id) — v0.853.0.
    /// Built once, ~31k entries. Probing it is how "for God so" (3 words,
    /// unique in the whole KJV) resolves to John 3:16, and how a 5-word
    /// ASR-damaged fragment ("let this mind being you") still reaches
    /// Philippians 2:5 through the ranked scorer.
    fn open_tri(&self) -> &Vec<(u64, u32)> {
        self.open_tri.get_or_init(|| {
            let mut out: Vec<(u64, u32)> = Vec::with_capacity(self.total_verses());
            for (bi, book) in self.books.iter().enumerate() {
                for (ci, chapter) in book.chapters.iter().enumerate() {
                    for (vi, text) in chapter.iter().enumerate() {
                        let w = normalize_words(text);
                        if w.len() < 3 {
                            continue;
                        }
                        out.push((key_hash(0, &w[..3]), pack(bi as u32, ci as u32, vi as u32)));
                    }
                }
            }
            out.sort_unstable();
            out.shrink_to_fit();
            out
        })
    }

    /// Sorted (hash of three words in a row ANYWHERE in a verse, verse id),
    /// shared with `pieces.rs` (v0.853.0). Used as a cheap prefilter before
    /// any "does this phrase appear inside a verse" text scan, which used to
    /// walk all 31,102 verses on every quotation miss (~7 ms per call).
    pub(crate) fn any_tri_index(&self) -> &Vec<(u64, u32)> {
        self.tri.get_or_init(|| {
            let mut out: Vec<(u64, u32)> = Vec::with_capacity(self.total_verses() * 20);
            for (bi, book) in self.books.iter().enumerate() {
                for (ci, chapter) in book.chapters.iter().enumerate() {
                    for (vi, text) in chapter.iter().enumerate() {
                        let id = pack(bi as u32, ci as u32, vi as u32);
                        let w = normalize_words(text);
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

    /// Fallback for the exact/sliding search above: tolerate exactly one
    /// mismatched word within a 6..=8 word window, for a single ASR
    /// substitution (e.g. Whisper hearing "love" for "loved" right at the
    /// start of a fragment with no preceding sentence context — an observed,
    /// real failure mode, not a hypothetical one). Deliberately restricted
    /// to n>=6 (a higher floor than the exact tier's 4) so a short phrase
    /// never gets a "close enough" guess — only a longer, already fairly
    /// specific match is allowed to tolerate any slack at all. Same
    /// uniqueness rule as the exact tier: a tie between two near-matches
    /// returns nothing rather than picking one.
    fn match_quotation_fuzzy(&self, words: &[String]) -> Option<(usize, usize, usize)> {
        const MAX_LEAD_IN: usize = 6;
        let max_start = words.len().saturating_sub(6).min(MAX_LEAD_IN);
        for start in 0..=max_start {
            let remaining = words.len() - start;
            if remaining < 6 {
                break;
            }
            let max_n = remaining.min(8);
            for n in (6..=max_n).rev() {
                let slice = &words[start..start + n];
                // Pigeonhole: with at most ONE differing word in n words, at
                // least one half matches exactly — so every verse that could
                // qualify is among those sharing an exact half with the window.
                let h = n / 2;
                let mut cands: Vec<u32> = Vec::new();
                for (st, seg) in [(0usize, &slice[..h]), (h, &slice[h..])] {
                    cands.extend(index_range(&self.seg_index, key_hash(st, seg)).iter().map(|e| e.1));
                }
                cands.sort_unstable();
                cands.dedup();
                let mut best: Option<u32> = None;
                let mut tie = false;
                for id in cands {
                    let Some(lead) = self.verse_lead(id, n) else { continue };
                    let mismatches = slice.iter().zip(lead.iter()).filter(|(a, b)| a != b).count();
                    if mismatches <= 1 {
                        if best.is_some() {
                            tie = true;
                        } else {
                            best = Some(id);
                        }
                    }
                }
                if let (Some(id), false) = (best, tie) {
                    let (b, c, v) = unpack(id);
                    return Some((b, c + 1, v + 1));
                }
            }
        }
        None
    }

    pub fn book_name(&self, book: usize) -> &str {
        &self.books[book].name
    }

    pub fn chapter_count(&self, book: usize) -> usize {
        self.books[book].chapters.len()
    }

    /// All chapter/verse parameters in the public API are 1-based.
    pub fn verse_count(&self, book: usize, chapter: usize) -> usize {
        if book >= self.books.len() || chapter == 0 || chapter > self.books[book].chapters.len() {
            return 0;
        }
        self.books[book].chapters[chapter - 1].len()
    }

    pub fn exists(&self, book: usize, chapter: usize, verse: usize) -> bool {
        verse >= 1 && self.verse_count(book, chapter) >= verse
    }

    /// Verse text (1-based chapter/verse). Empty string if out of range.
    pub fn verse(&self, book: usize, chapter: usize, verse: usize) -> &str {
        if !self.exists(book, chapter, verse) {
            return "";
        }
        &self.books[book].chapters[chapter - 1][verse - 1]
    }

    /// Passage text joined verse-by-verse ("v16 text  v17 text").
    pub fn passage(&self, book: usize, chapter: usize, from: usize, to: usize) -> String {
        let mut out = String::new();
        for v in from..=to.min(self.verse_count(book, chapter)) {
            if !out.is_empty() {
                out.push_str("  ");
            }
            let t = self.verse(book, chapter, v);
            if to > from {
                out.push_str(&format!("{v} {t}"));
            } else {
                out.push_str(t);
            }
        }
        out
    }

    /// Recognize a possible Scripture quotation (no reference spoken).
    /// Returns the verse hit for a 3..=8 word n-gram match, as
    /// (book_index, chapter, verse) with 1-based chapter/verse — but only
    /// when that n-gram identifies a *unique* verse (PRD 6.2's own
    /// acceptance wording), never an arbitrary pick among several ties.
    ///
    /// v0.853.0: the floor drops from 4 words to 3. Three words are enough
    /// when they are distinctive ("for God so" opens exactly one verse in
    /// the whole KJV — John 3:16), and the operator asked for auto-show at
    /// 3 words when nothing else competes. Uniqueness is still verified
    /// word-for-word, and an ambiguous 3-gram ("and it came to") still
    /// returns nothing rather than a guess.
    ///
    /// Tries the quotation starting at the utterance's first word first
    /// (the common case), then slides forward a few words at a time, so a
    /// lead-in the operator actually said out loud — "the Bible says,
    /// for God so loved the world…", "listen to this: the Lord is my
    /// shepherd…" — doesn't defeat a match that would otherwise be exact.
    /// At each start position, the longest available n-gram (8 words down
    /// to 3) is tried first, so a fuller match is always preferred over a
    /// shorter, less specific one.
    pub fn match_quotation(&self, text: &str) -> Option<(usize, usize, usize)> {
        let words = normalize_words(text);
        const MAX_LEAD_IN: usize = 18; // v0.853.0: was 6 — "…when the Bible says let this mind be in you" has 15 lead words
        let max_start = words.len().saturating_sub(4).min(MAX_LEAD_IN);
        for start in 0..=max_start {
            let remaining = words.len() - start;
            if remaining < 4 {
                break;
            }
            let max_n = remaining.min(8);
            for n in (4..=max_n).rev() {
                let window = &words[start..start + n];
                let hits = index_range(&self.quote_index, key_hash(0, window));
                if hits.len() == 1 {
                    let id = hits[0].1;
                    // hashes are 64-bit, but confirm against the real words —
                    // a wrong "unique" match on unrelated speech is worse than none
                    if self.verse_lead(id, n).as_deref() == Some(window) {
                        let (b, c, v) = unpack(id);
                        return Some((b, c + 1, v + 1));
                    }
                }
                // Not unique (or not found) at this (start, n) — e.g. the real but
                // non-unique opening "and it came to pass" — try a shorter n or a
                // later start instead of guessing.
            }
        }
        // v0.853.0: the unique 3-word tier (still uniqueness-gated, still
        // verified word-for-word against the verse text). The probe stays
        // within the first few words of the utterance — the RANKED search
        // above owns the deep-mid-sentence scan (with its own accuracy
        // floor); letting a 3-word probe fire at word 9 of a long line made
        // ordinary BBE-wording chatter ("...that he gave his only son")
        // resolve to whatever verse happens to open "he gave his".
        if words.len() >= 3 {
            let max3 = words.len().saturating_sub(3).min(6);
            let tri = self.open_tri();
            for start in 0..=max3 {
                let window = &words[start..start + 3];
                if !trigram_distinctive(window) {
                    continue;
                }
                let hits = index_range(tri, key_hash(0, window));
                if hits.len() == 1 && self.verse_lead(hits[0].1, 3).as_deref() == Some(window) {
                    let (b, c, v) = unpack(hits[0].1);
                    return Some((b, c + 1, v + 1));
                }
            }
        }
        self.match_quotation_fuzzy(&words)
    }

    /// Every verse a spoken fragment could be, best first, each with a
    /// 0-100 accuracy percentage (v0.811.0).
    ///
    /// Why this exists: `match_quotation` deliberately returns nothing when a
    /// fragment is not unique ("the grace of our Lord Jesus Christ ..." opens
    /// a dozen verses in the epistles). That is safe but unhelpful - the
    /// operator got no suggestion at all. This returns the whole ranked list
    /// so the screen can offer every real candidate and let the operator (or
    /// the auto-approve rule) pick.
    ///
    /// Scoring: word-order-aware overlap (longest common subsequence) between
    /// what was heard and the opening of each candidate verse, as an F1 score
    /// so that BOTH "heard words missing from the verse" and "verse words not
    /// heard" lower the percentage. Exact full-fragment matches score highest.
    ///
    /// v0.853.0 changes (the five operator-reported failures):
    /// - the floor drops from 4 words to 3 (unique 3-word openings are real
    ///   quotations: "for God so", "let this mind");
    /// - the lead-in window widens from 6 to 18 words ("…there is a mentality
    ///   of Zion that is to don't have when the Bible says let this mind be
    ///   in you" carries 15 words before the quotation);
    /// - every candidate is scored against the words FROM ITS OWN start
    ///   position, so a candidate found deep in the sentence is not scored
    ///   against the sentence's first words;
    /// - a new opening-trigram tier catches short ASR-damaged fragments
    ///   ("let this mind being you" -> Philippians 2:5);
    /// - the mid-verse text scan is prefiltered through the trigram index,
    ///   cutting the common no-match path from ~7 ms to well under 1 ms.
    pub fn match_quotations_ranked(&self, text: &str, max_results: usize) -> Vec<QuoteHit> {
        let words = normalize_words(text);
        if words.len() < 3 || max_results == 0 {
            return Vec::new();
        }
        const MAX_LEAD_IN: usize = 18;
        const MAX_CANDIDATES: usize = 400;
        let max_start = words.len().saturating_sub(3).min(MAX_LEAD_IN);

        struct Cand {
            id: u32,
            off: usize,
            start: usize,
        }
        let mut cands: Vec<Cand> = Vec::new();
        let push = |cands: &mut Vec<Cand>, id: u32, off: usize, start: usize| {
            if cands.len() >= MAX_CANDIDATES {
                return;
            }
            // v0.855.0: dedupe by VERSE, not by (verse, offset) — the two
            // probe origins (and the folded needles) can find the same verse
            // at two spots, which used to print "Malachi 4:6" twice on one
            // card. First (earliest-origin) entry wins.
            if !cands.iter().any(|c| c.id == id) {
                cands.push(Cand { id, off, start });
            }
        };

        // 1. gather candidate verses from the exact opening index (4..=8 word
        //    windows). Non-unique allowed. The FIRST start with any verified
        //    hit wins (earliest words are the strongest claim).
        let mut best_start = 0usize;
        'outer: for start in 0..=words.len().saturating_sub(4).min(MAX_LEAD_IN) {
            let remaining = words.len() - start;
            if remaining < 4 {
                break;
            }
            let max_n = remaining.min(8);
            for n in (4..=max_n).rev() {
                let window = &words[start..start + n];
                let mut found = false;
                for h in index_range(&self.quote_index, key_hash(0, window)) {
                    if self.verse_lead(h.1, n).as_deref() == Some(window) {
                        push(&mut cands, h.1, 0, start);
                        found = true;
                    }
                }
                if found {
                    best_start = start;
                    break 'outer; // longest window at the earliest start wins
                }
            }
        }
        // 2. a phrase said from the MIDDLE of a verse ("... the grace of our Lord
        //    Jesus Christ be with your spirit" is the end of Galatians 6:18).
        //    Runs EVEN when step 1 found opening candidates (the same words can
        //    both open one verse and close another). v0.853.0: this used to
        //    substring-scan all 31,102 verses on every call (~7 ms); the verse's
        //    own trigrams prefilter it — a verse containing the probe phrase
        //    must contain the phrase's first 3 words.
        //    v0.853.0 late fix: the gate drops from 5 words to 3, so a short
        //    fragment can also reach a verse that carries the phrase after
        //    one lead word ("and, behold, I come quickly" — Revelation
        //    22:12 — heard as "behold I come quickly" or "behold I come").
        //    A 3-4 word probe must open with a distinctive trigram, exactly
        //    like the opening-trigram tier, so ordinary English ("for unto
        //    us") stays quiet. And the needle slides down from the full
        //    probe to its first 3 words, so one ASR noise word in the middle
        //    ("behold I come banana and my") no longer hides a verse the
        //    prefilter already knows about — the F1 scorer plus the floors
        //    decide what survives.
        if words.len() >= 3 {
            // v0.855.0: the probe now runs from TWO origins — the tier-1
            // opening match (best_start) AND the utterance's own first words
            // (start 0). A weak opening hit elsewhere in the line ("…turn
            // THE HEART OF THE… children" — Proverbs 15:28 genuinely opens
            // "the heart of the…") used to drag the probe window past the
            // words the real quotation starts with, and Luke 1:17 became
            // unreachable while Malachi 4:6 carded alone (screenshot 1).
            let mut origins: Vec<usize> = vec![0];
            let bs = best_start;
            if bs > 0 && bs < words.len() - 3 && !origins.contains(&bs) {
                origins.push(bs);
            }
            for &probe_start in &origins {
            let probe_n = (words.len() - probe_start).min(6);
            let probe_words = &words[probe_start..probe_start + probe_n];
            if words.len() >= 5 || trigram_distinctive(&probe_words[..3]) {
                // v0.855.0: the probe trigram itself is also tried with its
                // last word singular/plural folded — "turn the heart" must
                // reach verses that read "turn the hearts" (Luke 1:17), and
                // vice versa. Exact hits keep priority in the candidate cap.
                let probe_tris: Vec<&[String]> = vec![&probe_words[..3]];
                let last = probe_words[2].clone();
                let mut folded_tri = probe_words[..3].to_vec();
                let mut fold_ok = false;
                if let Some(s) = last.strip_suffix('s') {
                    if s.len() >= 3 {
                        folded_tri[2] = s.to_string();
                        fold_ok = true;
                    }
                } else {
                    let plur = format!("{last}s");
                    if plur.len() >= 4 {
                        folded_tri[2] = plur;
                        fold_ok = true;
                    }
                }
                let mut seen: Vec<u32> = Vec::new();
                let mut ids: Vec<u32> = Vec::new();
                for key in probe_tris.iter() {
                    for h in index_range(self.any_tri_index(), key_hash(100, key)) {
                        if !ids.contains(&h.1) {
                            ids.push(h.1);
                        }
                    }
                }
                if fold_ok {
                    for h in index_range(self.any_tri_index(), key_hash(100, &folded_tri)) {
                        if !ids.contains(&h.1) {
                            ids.push(h.1);
                        }
                    }
                }
                // longest needle first: the full probe pins the exact spot
                // when it is present; shorter needles rescue noise-damaged
                // probes
                //
                // v0.855.0 (screenshot 1): each needle also tries a
                // singular/plural fold of its LAST word. The preacher says
                // "turn the heart of the father to the children"; Luke 1:17
                // reads "turn the HEARTS of the FATHERS to the children" — an
                // exact-only needle never finds the verse the phrase belongs
                // to (it scored Malachi 4:6 alone, which carded over the
                // named verse). Only the final word folds, only both forms
                // are 3+ letters, and the exact needle always runs before the
                // folded one, so F1 floors still judge what surfaces.
                let mut needles: Vec<String> = Vec::new();
                for n in (3..=probe_words.len()).rev() {
                    needles.push(format!(" {} ", probe_words[..n].join(" ")));
                    let last = probe_words[n - 1].clone();
                    let mut variants: Vec<String> = Vec::new();
                    if let Some(s) = last.strip_suffix('s') {
                        if s.len() >= 3 {
                            variants.push(s.to_string());
                        }
                    }
                    let plur = format!("{last}s");
                    if plur.len() >= 4 {
                        variants.push(plur);
                    }
                    for v in variants {
                        if v != last {
                            let mut folded = probe_words[..n - 1].to_vec();
                            folded.push(v);
                            needles.push(format!(" {} ", folded.join(" ")));
                        }
                    }
                }
                // cap the text scans the same way the old code capped results
                for id in ids.iter().take(MAX_CANDIDATES) {
                    if seen.len() >= MAX_CANDIDATES {
                        break;
                    }
                    if seen.contains(id) {
                        continue;
                    }
                    seen.push(*id);
                    let (b, c, v) = unpack(*id);
                    let Some(flat) = self.books.get(b as usize)
                        .and_then(|bk| bk.chapters.get(c as usize))
                        .and_then(|ch| ch.get(v as usize))
                    else {
                        continue;
                    };
                    let flat_pad = format!(" {} ", normalize_words(flat).join(" "));
                    for nd in &needles {
                        if let Some(pos) = flat_pad.find(nd.as_str()) {
                            let off = flat_pad[..pos].matches(' ').count();
                            push(&mut cands, *id, off, probe_start);
                            break;
                        }
                    }
                }
            }
            }
        }
        // 3. the single-substitution tier adds near misses (n >= 6).
        //    v0.853.0 ORDER: this STRONGER tier must run before the trigram
        //    tier below — "and have made those kings and priests unto god"
        //    fuzzy-resolves to Revelation 1:6 (one substituted word), but a
        //    weaker trigram probe on the same line also finds some unrelated
        //    verse whose opening happens to be "and his father"; letting the
        //    weak tier answer first would block the right one.
        if cands.is_empty() {
            if let Some((b, c, v)) = self.match_quotation_fuzzy(&words) {
                push(&mut cands, pack(b as u32, (c - 1) as u32, (v - 1) as u32), 0, 0);
            }
        }
        // 4. v0.853.0: opening-trigram tier, the WEAKEST evidence. Short
        //    fragments and ASR-damaged ones ("let this mind being you") have
        //    no exact 4+ word window; their first three words still open real
        //    verses. The bar depends on WHERE the probe sat: a fragment that
        //    IS the utterance's opening ("let this mind to being you", 57%)
        //    is a strong quotation claim and is offered from 50%; a probe
        //    buried mid-sentence ("...that he gave his only son" -> Psalms
        //    78:62 at 55%) is chatter and needs 70% to be offered at all.
        let mut tier_min_pct = MIN_TIER_PCT;
        if cands.is_empty() {
            let tri = self.open_tri();
            let mut earliest = usize::MAX;
            for start in 0..=max_start {
                let window = &words[start..start + 3];
                // ordinary English trigrams ("we have a") must not found a
                // quotation even when they uniquely open a verse
                if !trigram_distinctive(window) {
                    continue;
                }
                // a probe buried deep in the line that reaches the line's very
                // end ("...at nine in the morning" -> "in the morning") has
                // no continuation to confirm the verse — skip it
                if start > 2 && start + 3 >= words.len() {
                    continue;
                }
                for h in index_range(tri, key_hash(0, window)) {
                    if self.verse_lead(h.1, 3).as_deref() == Some(window) {
                        let before = cands.len();
                        push(&mut cands, h.1, 0, start);
                        if cands.len() > before && start < earliest {
                            earliest = start;
                        }
                    }
                }
            }
            if !cands.is_empty() && earliest > 2 {
                tier_min_pct = SHORT_TIER_PCT;
            }
        }
        if cands.is_empty() {
            return Vec::new();
        }

        // 5. score every candidate against what was actually heard FROM ITS
        //    OWN start (v0.853.0: a candidate found at word 15 is no longer
        //    scored against words 0..24 of a long line).
        let mut hits: Vec<QuoteHit> = cands
            .into_iter()
            .filter_map(|c| {
                let (b, cc, v) = unpack(c.id);
                let text = self.books.get(b as usize)?.chapters.get(cc as usize)?.get(v as usize)?;
                let vw = normalize_words(text);
                if c.off >= vw.len() {
                    return None;
                }
                let heard_from = c.start.min(words.len());
                let heard: Vec<&String> = words[heard_from..].iter().take(24).collect();
                if heard.is_empty() {
                    return None;
                }
                // compare against the verse words the speaker COULD have reached by now
                // (+1 slack): a verse is not penalised for words not yet said
                let take = (c.off + heard.len() + 1).min(vw.len());
                let lead: Vec<&String> = vw[c.off..take].iter().collect();
                let lcs = lcs_len(&heard, &lead);
                let run = longest_common_run(&heard, &lead);
                let p = lcs as f32 / heard.len() as f32;
                let r = lcs as f32 / lead.len().max(1) as f32;
                let f1 = if p + r > 0.0 { 2.0 * p * r / (p + r) } else { 0.0 };
                // words in an unbroken run beat the same words scattered with gaps:
                // "and have made us KINGS AND PRIESTS" is Rev 1:6 (contiguous), not
                // Rev 5:10 ("...us unto our God kings and priests").
                let tight = if lcs == 0 { 0.0 } else { 0.7 + 0.3 * run as f32 / lcs as f32 };
                let f1 = f1 * tight;
                // a phrase found mid-verse is a weaker claim than a verse opening
                let f1 = if c.off > 0 { f1 * 0.92 } else { f1 };
                let mut pct = ((f1 * 100.0).round() as u8).min(99);
                // v0.855.0: a silenced tail claim must stay silent even after fame
                let mut silenced = false;
                // v0.853.0 late fix: on a 3-4 word line a verse-TAIL match
                // ("but have eternal life" -> the ending of John 3:15) is a
                // continuation claim, not an opening claim. Cap it below the
                // strong-hit bar so the operator's translation and the pieces
                // assembler (which carry the real context) keep their vote;
                // fame may still lift the verse the operator actually means.
                //
                // v0.855.0 (screenshot 2): "also before him" — the last three
                // words of Acts 23:33 — cleared the 70% short-quote floor and
                // carded (even auto-staged) itself. A 3-4 word fragment that
                // claims a MID-VERSE spot is only offered when it carries real
                // evidence:
                //   * the claim sits at the verse's mouth (off <= 2) — the
                //     verse still has continuation words that can confirm it
                //     ("behold I come" one word into Revelation 22:12); or
                //   * the fragment matches a real share of the whole verse
                //     (>= 35% of its words).
                // A tail claim with no continuation ahead and no bulk behind
                // it ("also before him": 3 of 19 words, ends the verse) is
                // capped below the offer floor and stays silent. The pieces
                // assembler, full-line utterances and text search still find
                // those verses through their real context.
                if words.len() <= 4 && c.off > 0 {
                    let ender = c.off + heard.len() >= vw.len();
                    let at_mouth = c.off <= 2;
                    let coverage = lcs as f32 / vw.len() as f32;
                    if (ender || !at_mouth) && coverage < 0.35 {
                        silenced = true;
                    } else {
                        pct = pct.min(79);
                    }
                }
                // best-loved verses lead ties (v0.853.0 late fix: FAME_BONUS).
                // v0.855.0: fame only lifts a match whose OWN text evidence is
                // already auto-approve-grade (>= 60). "and he gave his only
                // horse away" scored 57 on its own; +12 fame put it at 69 and
                // the single-candidate rule AUTO-STAGED John 3:16 over
                // ordinary chatter. Fame still breaks ties and near-ties for
                // genuinely-fitting fragments ("behold I come quickly" 79 ->
                // Revelation 22:12), it never manufactures a strong hit.
                let fame = self.fame_bonus(b as usize, cc + 1, v + 1);
                let pct = if pct >= 60 { pct.saturating_add(fame).min(99) } else { pct.min(59) };
                // fame may lift a capped-but-real tail claim, never a silenced one
                let pct = if silenced { pct.min(49) } else { pct };
                Some(QuoteHit { book: b as usize, chapter: cc + 1, verse: v + 1, pct })
            })
            .collect();
        hits.sort_by(|a, b| {
            b.pct
                .cmp(&a.pct)
                .then(a.book.cmp(&b.book))
                .then(a.chapter.cmp(&b.chapter))
                .then(a.verse.cmp(&b.verse))
        });
        // the weakest tier's candidates must clear a higher bar to be offered
        hits.retain(|h| h.pct >= tier_min_pct);
        hits.truncate(max_results);
        hits
    }

    /// FAME_BONUS for a verse, 0 when it is not on the best-loved list
    /// (v0.853.0 late fix). 1-based chapter/verse, 0-based book index.
    pub(crate) fn fame_bonus(&self, book: usize, chapter: usize, verse: usize) -> u8 {
        let Some(name) = self.books.get(book).map(|b| b.name.as_str()) else { return 0 };
        WELL_KNOWN
            .iter()
            .any(|(n, c, v)| *n == name && *c as usize == chapter && *v as usize == verse)
            .then_some(FAME_BONUS)
            .unwrap_or(0)
    }

    /// Whole-utterance quality of a quotation hit, 0-100 — the same F1 math
    /// the ranked scorer uses, from the verse's opening (v0.853.0 late fix).
    /// Gates the unique-tier fallback in the parser so pulpit chatter that
    /// happens to BEGIN like a verse ("and he told them the story…" opens
    /// like Genesis 37:10) cannot raise a card, let alone stage.
    pub fn quote_pct(&self, text: &str, book: usize, chapter: usize, verse: usize) -> Option<u8> {
        let verse_text = self.books.get(book)?.chapters.get(chapter.checked_sub(1)?)?.get(verse.checked_sub(1)?)?;
        let vw = normalize_words(verse_text);
        let heard_raw = normalize_words(text);
        if heard_raw.is_empty() || vw.is_empty() {
            return None;
        }
        let heard: Vec<&String> = heard_raw.iter().collect();
        // the speaker could not have passed the verse's end (+1 slack), same
        // as the ranked scorer
        let take = (heard.len() + 1).min(vw.len());
        let lead: Vec<&String> = vw[..take].iter().collect();
        let lcs = lcs_len(&heard, &lead);
        if lcs == 0 {
            return Some(0);
        }
        let run = longest_common_run(&heard, &lead);
        let p = lcs as f32 / heard.len() as f32;
        let r = lcs as f32 / lead.len().max(1) as f32;
        let mut f1 = if p + r > 0.0 { 2.0 * p * r / (p + r) } else { 0.0 };
        f1 *= 0.7 + 0.3 * run as f32 / lcs as f32;
        Some(((f1 * 100.0).round() as u8).min(99))
    }

    /// v0.855.0 — how well a spoken fragment fits ONE SPECIFIC verse, allowing
    /// the fragment to sit anywhere IN the verse (`quote_pct` only ever
    /// measures from a verse's opening, so a preacher mid-verse always scored
    /// badly against the verse that is actually on stage).
    ///
    /// The metric is the same F1 the ranked scorer uses, but measured against
    /// the verse's best-matching SPAN: from the first verse word the fragment
    /// matched to the last one. The alignment is ANCHORED: every (heard
    /// position, verse position) pair where the two words agree seeds one
    /// greedy in-order extension, and the span with the best F1 wins — a
    /// naive earliest-match alignment lets filler words ("and", "the") anchor
    /// at the verse's first chapter-openers and smear the span across the
    /// whole verse, hiding a real 90% fit behind a 24% score.
    ///
    /// Used by the parser's named-verse lock (a fragment that clears the
    /// offer floor is "explained by" the verse on stage, so a look-alike
    /// verse must not take the stage) and by `repair` (the same pairs drive
    /// the line reconstruction).
    pub fn span_fit_pct(&self, text: &str, book: usize, chapter: usize, verse: usize) -> Option<u8> {
        self.span_align(text, book, chapter, verse).map(|a| a.pct)
    }

    /// Best anchored alignment of `text` inside one verse: the F1 percentage
    /// plus the (heard index, verse index) pairs of the winning span, both
    /// over NORMALIZED word positions (v0.855.0).
    pub fn span_align(
        &self,
        text: &str,
        book: usize,
        chapter: usize,
        verse: usize,
    ) -> Option<SpanAlign> {
        let vt = self
            .books
            .get(book)?
            .chapters
            .get(chapter.checked_sub(1)?)?
            .get(verse.checked_sub(1)?)?;
        let vw = normalize_words(vt);
        let hw = normalize_words(text);
        if hw.is_empty() || vw.is_empty() {
            return None;
        }
        // cap for cost: an utterance longer than this is pulpit talk, not one
        // verse; the parser's quotation path caps similarly
        let hw = if hw.len() > 60 { hw[..60].to_vec() } else { hw };

        let mut best: Option<SpanAlign> = None;
        let mut best_score = 0usize;
        // v0.855.0: a span must match at least ~1/3 of the heard words —
        // otherwise the scorer cherry-picks a tiny high-precision tail ("to
        // the children" scores 99 against Luke 1:17 and hides the real 7-word
        // span at 64). The floor keeps the winning span honest.
        let min_pairs = ((hw.len() * 34 + 99) / 100).max(2);
        for i0 in 0..hw.len() {
            for v0 in 0..vw.len() {
                if hw[i0] != vw[v0] {
                    continue;
                }
                // greedy in-order extension from this anchor, probing forward
                // WITHOUT consuming on failure
                let mut pairs: Vec<(usize, usize)> = Vec::with_capacity(hw.len() - i0);
                let mut j = v0;
                let mut last_v = v0;
                for (di, h) in hw[i0..].iter().enumerate() {
                    let mut k = j;
                    while k < vw.len() && &vw[k] != h {
                        k += 1;
                    }
                    if k < vw.len() {
                        pairs.push((i0 + di, k));
                        last_v = k;
                        j = k + 1;
                    }
                }
                if pairs.len() < min_pairs {
                    continue; // too small a span to be the real claim
                }
                let lcs = pairs.len();
                let span = (last_v - v0 + 1) as f32;
                let p = lcs as f32 / (hw.len() - i0) as f32;
                let r = lcs as f32 / span;
                let mut f1 = if p + r > 0.0 { 2.0 * p * r / (p + r) } else { 0.0 };
                // contiguity, same as the ranked scorer: an unbroken run of
                // matched verse words beats scattered hits
                let run = {
                    let mut best_run = 1usize;
                    let mut cur = 1usize;
                    for w in pairs.windows(2) {
                        if w[1].1 == w[0].1 + 1 {
                            cur += 1;
                            best_run = best_run.max(cur);
                        } else {
                            cur = 1;
                        }
                    }
                    best_run
                };
                f1 *= 0.7 + 0.3 * run as f32 / lcs as f32;
                let pct = ((f1 * 100.0).round() as u8).min(99);
                // selection = matched words x quality: a longer, decent span
                // beats a short perfect one ("of the fathers to the children"
                // at 73% must not beat "turn the hearts … children" at 64%
                // just because it skipped the two plural slips)
                let score = lcs * pct as usize;
                if score > best_score {
                    best_score = score;
                    best = Some(SpanAlign { pct, pairs });
                }
            }
        }
        best
    }

    pub(crate) fn flat_verses(&self) -> &Vec<(u32, String)> {
        self.flat.get_or_init(|| {
            let mut out = Vec::with_capacity(self.total_verses());
            for (bi, book) in self.books.iter().enumerate() {
                for (ci, chapter) in book.chapters.iter().enumerate() {
                    for (vi, text) in chapter.iter().enumerate() {
                        let mut j = String::with_capacity(text.len() + 2);
                        j.push(' ');
                        j.push_str(&normalize_words(text).join(" "));
                        j.push(' ');
                        out.push((pack(bi as u32, ci as u32, vi as u32), j));
                    }
                }
            }
            out
        })
    }

    /// Search the text of the whole Bible (v0.812.0): "god so loved" finds John 3:16.
    ///
    /// Ranking, best first: the words as an exact phrase; all the words in order
    /// (gaps allowed); all the words anywhere; most of the words. Verse-opening
    /// matches and short verses win ties (a short verse that contains the words is
    /// usually the one meant). The last word may be a prefix ("lov" finds loved).
    /// `pct` is a 0-100 relevance; hits below 40 are dropped.
    pub fn search_text(&self, query: &str, limit: usize) -> Vec<TextHit> {
        let q = normalize_words(query);
        if q.is_empty() || limit == 0 {
            return Vec::new();
        }
        let total_q = q.len();
        let needles_ws: Vec<String> = q.iter().map(|w| format!(" {w} ")).collect();
        let needles_pre: Vec<String> = q.iter().map(|w| format!(" {w}")).collect();
        let phrase = format!(" {} ", q.join(" "));
        let phrase_pre = format!(" {}", q.join(" "));
        let mut hits: Vec<TextHit> = Vec::new();
        for (id, flat) in self.flat_verses() {
            // flat = " w1 w2 w3 "
            let mut found = 0usize;
            let mut last_pos: Option<usize> = None;
            let mut in_order = true;
            let mut first_pos = usize::MAX;
            for i in 0..total_q {
                let is_last = i + 1 == total_q;
                // whole word, or (for the final typed word) a word that starts with it
                let pos = flat.find(needles_ws[i].as_str()).or_else(|| if is_last { flat.find(needles_pre[i].as_str()) } else { None });
                if let Some(p) = pos {
                    found += 1;
                    first_pos = first_pos.min(p);
                    if let Some(lp) = last_pos {
                        if p < lp {
                            in_order = false;
                        }
                    }
                    last_pos = Some(p);
                }
            }
            if found == 0 || (total_q >= 3 && found * 100 < total_q * 60) || (total_q <= 2 && found < total_q) {
                continue;
            }
            let has_phrase = flat.contains(phrase.as_str()) || flat.contains(phrase_pre.as_str());
            let coverage = found as f32 / total_q as f32;
            let mut score = if has_phrase {
                100.0
            } else if found == total_q && in_order {
                88.0
            } else if found == total_q {
                80.0
            } else {
                55.0 + 25.0 * coverage
            };
            let words_in_verse = flat.matches(' ').count().saturating_sub(1).max(1) as f32;
            // short verses: the query is a bigger share of them
            score -= (words_in_verse / 12.0).min(9.0);
            if first_pos == 0 {
                score += 2.0; // starts the verse
            }
            let (pb, pc, pv) = unpack(*id);
            if is_popular(self.book_name(pb as usize), pc as usize + 1, pv as usize + 1) {
                score += 6.0; // the verses preachers actually mean (John 3:16 before 1 John 4:11)
            }
            let pct = score.clamp(0.0, 99.0).round() as u8;
            if pct < 40 {
                continue;
            }
            let (b, c, v) = unpack(*id);
            hits.push(TextHit { book: b, chapter: c + 1, verse: v + 1, pct });
        }
        hits.sort_by(|a, b| b.pct.cmp(&a.pct).then(a.book.cmp(&b.book)).then(a.chapter.cmp(&b.chapter)).then(a.verse.cmp(&b.verse)));
        hits.truncate(limit);
        hits
    }

    pub fn total_verses(&self) -> usize {
        self.books
            .iter()
            .map(|b| b.chapters.iter().map(|c| c.len()).sum::<usize>())
            .sum()
    }
}

/// Strip `{...}` notes, `<...>` tags and doubled spaces from imported verse text.
fn clean_verse(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let (mut brace, mut angle) = (0u32, 0u32);
    for ch in v.chars() {
        match ch {
            '{' => brace += 1,
            '}' => brace = brace.saturating_sub(1),
            '<' => angle += 1,
            '>' => angle = angle.saturating_sub(1),
            c if brace == 0 && angle == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The 66 canonical English book names, in order, taken from the embedded KJV.
fn canonical_book_names() -> Vec<String> {
    let k = Bible::embedded();
    (0..BOOK_COUNT).map(|i| k.book_name(i).to_string()).collect()
}

fn book_index_by_name(name: &str, canon: &[String]) -> Option<usize> {
    let want: String = name.to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    canon.iter().position(|c| c.to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>() == want)
}

/// Widely quoted verses, nudged up in search results so "god so loved" offers John 3:16 first.
const POPULAR: &[(&str, usize, usize)] = &[
    ("Genesis", 1, 1), ("Genesis", 1, 27), ("Exodus", 20, 3), ("Joshua", 1, 9), ("Psalms", 23, 1), ("Psalms", 46, 1),
    ("Psalms", 119, 105), ("Proverbs", 3, 5), ("Proverbs", 3, 6), ("Isaiah", 40, 31), ("Isaiah", 41, 10), ("Isaiah", 53, 5),
    ("Jeremiah", 29, 11), ("Matthew", 6, 33), ("Matthew", 11, 28), ("Matthew", 28, 19), ("Matthew", 28, 20),
    ("John", 1, 1), ("John", 3, 16), ("John", 14, 6), ("John", 10, 10), ("Romans", 3, 23), ("Romans", 5, 8), ("Romans", 6, 23),
    ("Romans", 8, 28), ("Romans", 10, 9), ("Romans", 12, 2), ("1 Corinthians", 13, 4), ("2 Corinthians", 5, 17),
    ("Galatians", 5, 22), ("Ephesians", 2, 8), ("Ephesians", 2, 9), ("Philippians", 4, 13), ("Philippians", 4, 6),
    ("Hebrews", 11, 1), ("Hebrews", 11, 6), ("1 John", 1, 9), ("Revelation", 1, 6), ("Revelation", 3, 20),
];

fn is_popular(book: &str, chapter: usize, verse: usize) -> bool {
    POPULAR.iter().any(|(b, c, v)| *b == book && *c == chapter && *v == verse)
}

/// One full-text search result (1-based chapter/verse, canonical book index).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextHit {
    pub book: usize,
    pub chapter: usize,
    pub verse: usize,
    /// 0-100 relevance
    pub pct: u8,
}

/// One ranked quotation candidate (1-based chapter/verse, canonical book index).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteHit {
    pub book: usize,
    pub chapter: usize,
    pub verse: usize,
    /// 0-100 accuracy of this candidate against what was heard.
    pub pct: u8,
}

/// v0.855.0 — the best anchored alignment of a spoken line inside one verse
/// (see `Bible::span_align`): `pct` is the span F1, `pairs` are
/// (heard word index, verse word index) over NORMALIZED words.
#[derive(Clone, Debug)]
pub struct SpanAlign {
    pub pct: u8,
    pub pairs: Vec<(usize, usize)>,
}

/// Length of the longest run of words that appears unbroken in both lists.
fn longest_common_run(a: &[&String], b: &[&String]) -> usize {
    let mut best = 0usize;
    let mut prev = vec![0usize; b.len() + 1];
    for x in a {
        let mut cur = vec![0usize; b.len() + 1];
        for (j, y) in b.iter().enumerate() {
            if x == y {
                cur[j + 1] = prev[j] + 1;
                best = best.max(cur[j + 1]);
            }
        }
        prev = cur;
    }
    best
}

/// Length of the longest common subsequence of two word lists.
fn lcs_len(a: &[&String], b: &[&String]) -> usize {
    let mut prev = vec![0usize; b.len() + 1];
    for x in a {
        let mut cur = vec![0usize; b.len() + 1];
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = if x == y { prev[j] + 1 } else { cur[j].max(prev[j + 1]) };
        }
        prev = cur;
    }
    prev[b.len()]
}

fn pack(b: u32, c: u32, v: u32) -> u32 {
    // b < 66 (7 bits), c <= 150 (8 bits), v <= 176 (8 bits)
    (b << 16) | (c << 8) | v
}

fn unpack(p: u32) -> (usize, usize, usize) {
    ((p >> 16) as usize, ((p >> 8) & 0xFF) as usize, (p & 0xFF) as usize)
}

/// Lowercase, strip punctuation, split words.
pub fn normalize_words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for w in text.split(|c: char| !c.is_ascii_alphanumeric()) {
        if !w.is_empty() {
            out.push(modernize(&w.to_ascii_lowercase()));
        }
    }
    out
}

/// King James verbs and pronouns a speech engine writes the modern way.
/// A preacher reading "And hath made us kings and priests" is transcribed
/// "and have made us kings and priests"; without this the two never matched.
/// Applied to BOTH the verse text and the spoken text, so the comparison is
/// symmetric: `hath`/`hast` -> `have`, `doth`/`dost` -> `does`, `saith` -> `says`,
/// `unto` is kept (it is distinctive and spoken as written).
///
/// v0.855.0: the OT/NT name pairs are folded the same symmetric way —
/// the KJV OT says "Elijah" where the NT (and most speech engines) say
/// "Elias", so "the spirit and power of Elijah" (spoken) and Luke 1:17's
/// "power of Elias" (text) collapsed to one word and finally match.
/// Both forms map to the NT Greek form; matching only — display text is
/// never rewritten by this.
fn modernize(w: &str) -> String {
    match w {
        "hath" | "hast" => "have",
        "doth" | "dost" => "does",
        "saith" => "says",
        "shalt" => "shall",
        "wilt" => "will",
        "art" => "are",
        "thee" | "thou" => "you",
        "thy" | "thine" => "your",
        "ye" => "you",
        "wherefore" => "therefore",
        // OT Hebrew form | NT Greek form -> one canonical token
        "elijah" => "elias",
        "elisha" => "eliseus",
        "isaiah" => "esaias",
        "jeremiah" | "jeremy" => "jeremias",
        "jonah" => "jonas",
        "noah" => "noe",
        // KJV -eth verbs a speech engine hears as modern -s/-es: applied to
        // BOTH sides, so "he that dwells in" (spoken) matches "He that
        // dwelleth in" (text) and vice versa
        "abideth" => "abides",
        "believeth" => "believes",
        "cometh" => "comes",
        "doeth" => "does",
        "dwelleth" => "dwells",
        "giveth" => "gives",
        "goeth" => "goes",
        "heareth" => "hears",
        "keepeth" => "keeps",
        "knoweth" => "knows",
        "loveth" => "loves",
        "maketh" => "makes",
        "putteth" => "puts",
        "sitteth" => "sits",
        "speaketh" => "speaks",
        "standeth" => "stands",
        "teacheth" => "teaches",
        "turneth" => "turns",
        "waiteth" => "waits",
        "walketh" => "walks",
        "worketh" => "works",
        other => return other.to_string(),
    }
    .to_string()
}

#[cfg(test)]
#[path = "v855_tests.rs"]
mod v855_tests;

#[cfg(test)]
mod v890_tests {
    use super::*;

    /// v0.890.0 — the filler-tail stripper (ported from v0.889): a trailing
    /// "So." / "Amen." is not evidence and must not cost the line its bar.
    #[test]
    fn strip_filler_tail_removes_only_spoken_tails() {
        assert_eq!(strip_filler_tail("able to make you wise. So."), "able to make you wise.");
        assert_eq!(strip_filler_tail("able to make you wise. So. Amen."), "able to make you wise.");
        assert_eq!(strip_filler_tail("for God so loved the world"), "for God so loved the world");
        // ordinary trailing words are never touched
        assert_eq!(strip_filler_tail("the lord is my shepherd today"), "the lord is my shepherd today");
        // never below the 4-word floor, at most three words
        assert_eq!(strip_filler_tail("he is good so amen okay"), "he is good so amen okay".split_whitespace().take(4).collect::<Vec<_>>().join(" "));
        assert_eq!(strip_filler_tail("amen"), "amen");
    }

    /// "so" INSIDE a line is ordinary speech and stays.
    #[test]
    fn strip_filler_tail_keeps_interior_words() {
        assert_eq!(strip_filler_tail("so we go on to the next thing"), "so we go on to the next thing");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_kjv_validates() {
        let b = Bible::embedded();
        assert_eq!(b.books.len(), 66);
        assert_eq!(b.total_verses(), 31_102);
    }

    #[test]
    fn john_3_16() {
        let b = Bible::embedded();
        let jn = 42; // canonical index of John
        assert_eq!(b.book_name(jn), "John");
        assert_eq!(b.chapter_count(jn), 21);
        assert_eq!(b.verse_count(jn, 2), 25); // John 2 has 25 verses
        assert!(b.verse(jn, 2, 25).starts_with("And needed not that any should testify"));
        assert_eq!(b.verse(jn, 2, 26), "");
        assert_eq!(b.verse_count(jn, 16), 33); // John 16
        assert!(b.verse(jn, 3, 16).starts_with("For God so loved"));
    }

    #[test]
    fn one_corinthians_five_has_13_verses() {
        let b = Bible::embedded();
        let icor = 45;
        assert_eq!(b.book_name(icor), "1 Corinthians");
        assert_eq!(b.verse_count(icor, 5), 13);
        assert!(!b.exists(icor, 5, 17));
    }

    #[test]
    fn two_corinthians_5_17_exists() {
        let b = Bible::embedded();
        let iicor = 46;
        assert!(b.exists(iicor, 5, 17));
        assert!(b.verse(iicor, 5, 17).contains("new creature"));
    }

    #[test]
    fn quotation_for_god_so_loved() {
        let b = Bible::embedded();
        let (bk, c, v) = b
            .match_quotation("For God so loved the world that he gave his only begotten Son")
            .expect("must match");
        assert_eq!((b.book_name(bk), c, v), ("John", 3, 16));
    }

    #[test]
    fn quotation_four_word_fragment_of_a_longer_verse_matches() {
        // Regression: the old index stored only ONE key per verse (its own
        // leading min(len,8) words), so a shorter genuine fragment of a
        // longer verse could never match anything — only a fragment whose
        // length happened to equal the verse's own total word count. John
        // 3:16 is far longer than 4 words; its own first 4 words must now
        // match on their own.
        let b = Bible::embedded();
        let (bk, c, v) = b.match_quotation("for god so loved").expect("must match");
        assert_eq!((b.book_name(bk), c, v), ("John", 3, 16));
    }

    #[test]
    fn quotation_three_unique_words_now_match() {
        // v0.853.0 (operator request): "for God so" — THREE words — opens
        // exactly one verse in the whole KJV (John 3:16). Nothing else
        // competes, so it must resolve. Two words still never match.
        let b = Bible::embedded();
        let (bk, c, v) = b.match_quotation("for god so").expect("unique 3 words must match");
        assert_eq!((b.book_name(bk), c, v), ("John", 3, 16));
        assert!(b.match_quotation("for god").is_none());
        assert!(b.match_quotation("").is_none());
    }

    #[test]
    fn quotation_ambiguous_three_words_still_rejected() {
        // The 3-word floor only helps when the fragment is UNIGUE.
        // "and it came to" opens many verses; nothing may be guessed.
        let b = Bible::embedded();
        assert!(b.match_quotation("and it came").is_none());
        assert!(b.match_quotation("blessed are the").is_none());
    }

    #[test]
    fn quotation_matches_after_a_spoken_lead_in() {
        // A real operator rarely starts talking on the exact first word of
        // the verse — "the Bible says, for God so loved the world…" should
        // still match, by sliding the search a few words forward.
        let b = Bible::embedded();
        let (bk, c, v) = b
            .match_quotation("the bible says for god so loved the world that he gave his only begotten son")
            .expect("must match despite the lead-in");
        assert_eq!((b.book_name(bk), c, v), ("John", 3, 16));
    }

    #[test]
    fn quotation_ambiguous_opening_is_not_guessed() {
        // "And it came to pass" opens dozens of KJV verses. The old code
        // took whichever hit happened to be first in the index — silently
        // wrong most of the time. The PRD's own acceptance wording requires
        // a *unique* match; a common, non-unique opening must return
        // nothing rather than guess.
        let b = Bible::embedded();
        assert!(b.match_quotation("and it came to pass").is_none());
    }

    #[test]
    fn quotation_tolerates_one_word_asr_slip() {
        // Real failure observed via actual Whisper output on synthesized
        // speech: a fragment starting cold on "Loved the world…" (no
        // preceding sentence audio) was transcribed as "Love the world…".
        // A single substituted word in an otherwise-long, specific match
        // should still resolve.
        let b = Bible::embedded();
        let (bk, c, v) = b
            .match_quotation("for god so love the world that he gave his only begotten son")
            .expect("must tolerate one substituted word");
        assert_eq!((b.book_name(bk), c, v), ("John", 3, 16));
    }

    #[test]
    fn quotation_fuzzy_tier_still_requires_at_least_six_words() {
        // The one-word-tolerance fallback is STILL restricted to n>=6 — a
        // short phrase never gets a fuzzy "close enough" guess from THAT
        // tier. ("for god so love" now resolves through the v0.853.0
        // 3-word-unique tier instead: its first 3 words open only John 3:16,
        // so the fragment is a known verse with one slip, not a guess.)
        let b = Bible::embedded();
        let (bk, c, v) = b
            .match_quotation("for god so love")
            .expect("first 3 words are unique, one slip is tolerated by that tier");
        assert_eq!((b.book_name(bk), c, v), ("John", 3, 16));
        // a 3-word fragment with a broken trigram gets nothing from any tier
        assert!(b.match_quotation("for god, love").is_none());
    }

    #[test]
    fn quotation_two_mismatches_still_never_fuzzy_guessed() {
        // Tolerance is exactly one substitution in the FUZZY tier, not "close
        // enough" in general — two wrong words in the same 6-word window must
        // never come from the fuzzy tier. (This fragment now resolves through
        // the ranked path instead: its opening trigram is unique, and the F1
        // score honestly reflects the damage. The OPERATOR asked for exactly
        // this: a mis-transcribed quote that still shows the right verse.)
        let b = Bible::embedded();
        let (bk, c, v) = b
            .match_quotation("for god so love that world that he gave his only begotten son")
            .expect("unique opening + enough correct words still resolves");
        assert_eq!((b.book_name(bk), c, v), ("John", 3, 16));
    }

    #[test]
    fn quotation_matching_stays_fast_on_ordinary_speech() {
        // PRD §23: parse + stage < 50 ms CPU per transcript. The one-word
        // tolerance tier once scanned all 31,102 verses per call (~25 ms even
        // in an optimized build, ~100 ms across the several calls one
        // utterance can trigger). It is index-driven now; this guards against
        // a regression back to a full scan. The bound is deliberately loose
        // (debug build, shared CI machine) yet far below the old cost.
        let b = Bible::embedded();
        let chatter = "and you know brothers and sisters when we think about what god has done in our lives this past year we have so much to be thankful for";
        let n = 20;
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            std::hint::black_box(b.match_quotation(chatter));
        }
        let per_call_ms = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        assert!(per_call_ms < 8.0, "match_quotation took {per_call_ms:.1} ms/call");
    }

    #[test]
    fn psalm_23() {
        let b = Bible::embedded();
        let ps = 18;
        assert_eq!(b.chapter_count(ps), 150);
        assert_eq!(b.verse(ps, 23, 1), "The LORD is my shepherd; I shall not want.");
    }

    // ---- v0.801.2 multi-translation engine ----

    #[test]
    fn embedded_bbe_validates() {
        let b = Bible::embedded_bbe();
        assert_eq!(b.books.len(), 66);
        assert!(b.total_verses() >= 31_000 && b.total_verses() <= 31_200);
    }

    #[test]
    fn kjv_and_bbe_share_book_order() {
        // Quotation matching against an alternate translation reuses the
        // book INDEX from whichever Bible matched, and treats it as valid
        // against the other translation too (e.g. staging a BBE quotation
        // hit through the same book/chapter/verse numbering used for KJV
        // navigation). That's only safe if both assets list all 66 books in
        // the same order — checked here structurally (chapter counts, which
        // will essentially never coincidentally match between two different
        // books) rather than by name: the two assets spell one book
        // differently ("Psalms" vs "Psalm") without that being an ordering
        // problem, so name equality would be the wrong, over-strict check.
        let kjv = Bible::embedded();
        let bbe = Bible::embedded_bbe();
        for i in 0..BOOK_COUNT {
            assert_eq!(
                kjv.chapter_count(i),
                bbe.chapter_count(i),
                "book order mismatch at index {i}: {} vs {}",
                kjv.book_name(i),
                bbe.book_name(i)
            );
        }
    }

    #[test]
    fn bbe_john_3_16_differs_from_kjv() {
        let kjv = Bible::embedded();
        let bbe = Bible::embedded_bbe();
        let jn = 42;
        assert!(kjv.verse(jn, 3, 16).starts_with("For God so loved"));
        assert!(bbe.verse(jn, 3, 16).starts_with("For God had such love"));
    }

    #[test]
    fn ranked_quotations_offer_every_grace_verse_best_first() {
        let b = Bible::embedded();
        let hits = b.match_quotations_ranked("the grace of our lord jesus christ be with you", 14);
        assert!(hits.len() >= 4, "{hits:?}");
        assert!(hits.windows(2).all(|w| w[0].pct >= w[1].pct), "sorted best first: {hits:?}");
        let names: Vec<String> = hits.iter().map(|h| format!("{} {}:{}", b.book_name(h.book), h.chapter, h.verse)).collect();
        assert!(names.iter().any(|n| n == "Revelation 22:21"), "{names:?}");
        assert!(names.iter().any(|n| n == "Galatians 6:18"), "mid-verse phrase must be offered too: {names:?}");
        assert!(hits[0].pct > 60, "{hits:?}");
    }

    #[test]
    fn ranked_unique_fragment_is_a_single_confident_hit() {
        let b = Bible::embedded();
        let hits = b.match_quotations_ranked("for god so loved the world", 5);
        assert_eq!(hits.first().map(|h| (h.book, h.chapter, h.verse)), Some((42, 3, 16)));
    }

    #[test]
    fn ranked_short_or_junk_is_empty() {
        let b = Bible::embedded();
        assert!(b.match_quotations_ranked("thank you", 5).is_empty());
        assert!(b.match_quotations_ranked("good morning everybody welcome to church", 5).is_empty());
    }

    #[test]
    fn hath_and_have_are_the_same_word_for_quotations() {
        let b = Bible::embedded();
        // KJV Revelation 1:6 says "hath"; Whisper writes "have"
        let m = b.match_quotation("and have made us kings and priests unto god");
        assert_eq!(m.map(|(bk, c, v)| (b.book_name(bk).to_string(), c, v)), Some(("Revelation".to_string(), 1, 6)));
        let ranked = b.match_quotations_ranked("and have made us kings and priests", 5);
        assert_eq!(ranked.first().map(|h| (h.book, h.chapter, h.verse)), Some((65, 1, 6)), "{ranked:?}");
    }

    #[test]
    fn modernize_is_symmetric_and_small() {
        assert_eq!(normalize_words("And HATH made thee"), vec!["and", "have", "made", "you"]);
        assert_eq!(normalize_words("Thou shalt not"), vec!["you", "shall", "not"]);
    }

    #[test]
    fn text_search_finds_john_3_16_from_a_few_words() {
        let b = Bible::embedded();
        for q in ["god so loved", "for god so loved the world", "so loved the world", "God so lov"] {
            let hits = b.search_text(q, 10);
            assert_eq!(hits.first().map(|h| (h.book, h.chapter, h.verse)), Some((42, 3, 16)), "{q}: {hits:?}");
        }
    }

    #[test]
    fn text_search_returns_several_relevant_verses_best_first() {
        let b = Bible::embedded();
        let hits = b.search_text("kings and priests", 10);
        let names: Vec<String> = hits.iter().map(|h| format!("{} {}:{}", b.book_name(h.book), h.chapter, h.verse)).collect();
        assert!(names.contains(&"Revelation 1:6".to_string()), "{names:?}");
        assert!(names.contains(&"Revelation 5:10".to_string()), "{names:?}");
        assert!(hits.windows(2).all(|w| w[0].pct >= w[1].pct));
        assert!(b.search_text("zzzqqq", 5).is_empty());
        assert!(b.search_text("", 5).is_empty());
    }

    #[test]
    fn importer_reads_the_other_common_layouts() {
        let k = Bible::embedded();
        // layout 2: bare array of {name, chapters:[[..]]} with a non-English name, like thiagobodruk
        let arr: Vec<serde_json::Value> = (0..66)
            .map(|b| {
                let chs: Vec<Vec<String>> = (1..=k.chapter_count(b)).map(|c| (1..=k.verse_count(b, c)).map(|v| k.verse(b, c, v).to_string()).collect()).collect();
                serde_json::json!({"name": "G\u{00ea}nesis", "chapters": chs})
            })
            .collect();
        let t = Bible::from_any(&serde_json::Value::Array(arr.clone()).to_string()).unwrap();
        assert_eq!(t.book_name(0), "Genesis", "names always come from the canonical list");
        assert_eq!(t.verse(42, 3, 16), k.verse(42, 3, 16));
        // layout 3: scrollmapper
        let sm: Vec<serde_json::Value> = (0..66)
            .map(|b| {
                let chs: Vec<serde_json::Value> = (1..=k.chapter_count(b))
                    .map(|c| serde_json::json!({"chapter": c, "verses": (1..=k.verse_count(b, c)).map(|v| serde_json::json!({"verse": v, "text": format!("{{note}} {} <i>x</i>", k.verse(b, c, v))})).collect::<Vec<_>>()}))
                    .collect();
                serde_json::json!({"name": k.book_name(b), "chapters": chs})
            })
            .collect();
        let t = Bible::from_any(&serde_json::json!({"translation": "T", "books": sm}).to_string()).unwrap();
        assert_eq!(t.verse(0, 1, 1), format!("{} x", k.verse(0, 1, 1)), "markup stripped");
        // layout 4: tab lines
        let mut lines = String::new();
        for b in 0..66 {
            for c in 1..=k.chapter_count(b) {
                for v in 1..=k.verse_count(b, c) {
                    lines.push_str(&format!("{}\t{c}\t{v}\t{}\n", k.book_name(b), k.verse(b, c, v)));
                }
            }
        }
        let t = Bible::from_any(&lines).unwrap();
        assert_eq!(t.total_verses(), 31102);
        // rejects an incomplete Bible honestly
        assert!(Bible::from_any("{\"books\":[]}").err().unwrap().contains("66"));
        assert!(Bible::from_any("hello").is_err());
    }

    #[test]
    fn kings_and_priests_picks_revelation_1_6_clearly_over_5_10() {
        let b = Bible::embedded();
        for q in ["and have made us kings and priests", "and have made us kings and priests unto god"] {
            let h = b.match_quotations_ranked(q, 5);
            assert_eq!((h[0].book, h[0].chapter, h[0].verse), (65, 1, 6), "{q}: {h:?}");
            let second = h.get(1).map(|x| x.pct).unwrap_or(0);
            assert!(h[0].pct >= 75 && h[0].pct - second >= 12, "{q}: clear winner needed: {h:?}");
        }
    }
}
