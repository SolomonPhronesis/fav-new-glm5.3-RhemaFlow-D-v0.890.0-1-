//! Structural spoken-Scripture parser.
//!
//! Design rules (PRD v2.0 §6.1):
//! - Structural, not a word whitelist: any number of filler words is tolerated
//!   between the book, the chapter and the verse, because no word list can
//!   cover natural speech.
//! - Digit-split: ASR merges ("chapter three sixteen" -> "316") and inserts
//!   spurious artifacts ("five, for seventeen"); the parser tolerates both.
//! - Fuzzy book match with a preaching-frequency tie-break.
//! - Verse-count validation against the embedded KJV: a verse that does not
//!   exist is NEVER staged silently — the operator gets an honest message
//!   with the closest real candidates (silent failure is a product defect).

use crate::books::{self, BOOKS};
use rf_bible::Bible;

// ---------------------------------------------------------------------------
// v0.890.0 — the operator's suggestion gates, carried in ParseContext.
//
// v0.856 shipped two fixed bars for quotation CARDS (the suggestive
// scriptures the operator approves with one click): a fragment shorter than
// 4 words needed 70%, anything else 50%. v0.889 went the other way and
// removed the short suggestions entirely (a hard 5-word floor), which the
// operator experienced as "v0.889 stopped suggesting". v0.890 makes both
// bars SETTINGS instead of constants, so the same build can behave like
// v0.856 (3 words from 50%), like v0.889 (5 words from 90%) or anywhere in
// between — without recompiling.
//
// They ride on ParseContext (never a global): every parse carries its own
// gates, so the settings window can change them between utterances and no
// shared state can leak between decodes or between tests.
// ---------------------------------------------------------------------------

/// The card (suggestion) gates. `Default` is v0.856 exactly — the version
/// whose suggestive scriptures the operator asked to keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteGates {
    /// Shortest utterance (in words) that may raise a quotation card.
    /// 3 = v0.856 (the trigram tier and everything above), 4 = no 3-word cards,
    /// 5 = v0.889. The settings UI offers 3/4/5.
    pub min_words: u8,
    /// Lowest accuracy a quotation candidate may have to be offered as a card.
    /// 50 = v0.856's firehose, 75-90 = the operator's stated Sunday range.
    pub min_pct: u8,
}

impl Default for QuoteGates {
    fn default() -> Self {
        QuoteGates { min_words: 3, min_pct: MIN_QUOTE_PCT }
    }
}

impl QuoteGates {
    /// Clamp user input into the safe range.
    pub fn clamped(min_words: u8, min_pct: u8) -> Self {
        QuoteGates { min_words: min_words.clamp(2, 6), min_pct: min_pct.clamp(40, 95) }
    }

    /// The bar a quote card needs for THIS utterance: the operator's slider,
    /// but a fragment shorter than 4 words is a weaker claim and must clear
    /// the higher of the slider and the short-utterance floor (v0.853.0 rule:
    /// "for God so" scores 86; ordinary chatter rarely clears 70).
    pub fn card_min_pct(&self, word_count: usize) -> u8 {
        if word_count < 4 {
            SHORT_QUOTE_FLOOR_PCT.max(self.min_pct)
        } else {
            self.min_pct
        }
    }
}

/// The floor for fragments SHORTER than 4 words, applied on top of the
/// operator's slider (a 3-word claim stays a weak claim).
pub const SHORT_QUOTE_FLOOR_PCT: u8 = 70;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// 1-based Scripture location. `to >= from`. `chapter == 0` means book-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Passage {
    pub book: u8,
    pub chapter: u16,
    pub from: u16,
    pub to: u16,
}

impl Passage {
    pub fn single(book: u8, chapter: u16, verse: u16) -> Self {
        Passage { book, chapter, from: verse, to: verse }
    }
    pub fn is_range(&self) -> bool {
        self.to > self.from
    }
    pub fn label(&self) -> String {
        let name = BOOKS[self.book as usize].name;
        if self.chapter == 0 {
            return name.to_string();
        }
        let c = self.chapter;
        if self.from == 0 {
            return format!("{name} {c}");
        }
        if self.to > self.from {
            format!("{name} {c}:{}-{}", self.from, self.to)
        } else {
            format!("{name} {c}:{}", self.from)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectionKind {
    /// Full spoken reference: book + chapter + verse.
    Explicit,
    /// Book + chapter, no verse — stages verse 1 with a visible badge.
    ChapterOnly,
    /// Bare "verse 7" / "that verse" — resolved against the on-stage chapter.
    Relative,
    /// Leading-words match against the KJV text — always confirm-queued.
    Quotation,
    /// Book mentioned without chapter or verse.
    BookOnly,
}

#[derive(Clone, Debug)]
pub struct Detection {
    pub kind: DetectionKind,
    pub passage: Passage,
    pub confidence: f32,
    pub alternatives: Vec<Passage>,
    /// 0-100 accuracy of each entry in `alternatives` (same order); empty when not ranked.
    pub alt_pct: Vec<u8>,
    /// 0-100 accuracy of the best candidate for a ranked quotation; 0 when not ranked.
    pub pct: u8,
    /// v0.856.0: how many words the utterance (or assembled pieces) carried.
    /// The auto-approve rule in rf-app gates on this — the whole-Bible grid
    /// showed a FOUR-word fragment said in passing could clear the old bars
    /// ("there are many devices" staged Proverbs 19:21 at 89%). 0 = unknown
    /// (detections that are not quotations; the gate then ignores it).
    pub words: u8,
    pub note: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RefError {
    /// Operator-readable, honest message ("1 Corinthians 5 has only 13 verses
    /// (no verse 17) — did you mean 2 Corinthians 5:17?").
    pub message: String,
    /// Stageable alternatives; the operator can promote any of them.
    pub candidates: Vec<Passage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    NextVerse,
    PrevVerse,
    NextChapter,
    PrevChapter,
    /// Re-stage the most recent Scripture ("last scripture", "again").
    LastScripture,
    /// Read the on-stage passage aloud ("read it").
    ReadAloud,
    /// Stop the current read-aloud ("stop reading").
    StopReading,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ClearStage,
    Fullscreen,
    ExitFullscreen,
    StopListening,
    StartRecording,
    StopRecording,
    /// Preacher explicitly asked for another translation.
    Translation(&'static str),
    // ---- v0.811.0 ----
    /// Open the congregation stage window ("open stage").
    OpenStage,
    /// Close the congregation stage window ("close stage").
    CloseStage,
    /// Jump to verse 1 of the chapter on stage ("first verse").
    FirstVerse,
    /// Set the stage text zoom to this percent ("zoom 300", "max zoom" = 400).
    ZoomTo(u16),
    /// Approve the first card waiting in the review queue ("approve it").
    ApproveQuote,
    /// Dismiss the first card waiting in the review queue ("dismiss that").
    DismissQuote,
    /// Stage the Nth candidate (1-based) of the first review card ("option two").
    PickMatch(u8),
    /// Auto-approve scripture quotations on/off.
    AutoApprove(bool),
    /// Show every possible matching scripture (true) or only the best one (false).
    ShowAllMatches(bool),
    /// Read each newly staged verse aloud automatically on/off.
    AutoRead(bool),
    ReadFaster,
    ReadSlower,
    /// Show (true) or hide (false) the voice-command list.
    Help(bool),
    // ---- v0.814.0: walk the list of scriptures shown so far ----
    /// Go back one scripture in the history ("previous scripture"); repeat to keep going back.
    PrevScripture,
    /// Go forward in the history after going back; with nothing to go forward to, next verse.
    NextScripture,
    // ---- v0.815.0: slides and video ----
    /// Next slide of the slides/song on the stage.
    NextSlide,
    /// Previous slide.
    PrevSlide,
    /// Play (or resume) the video on the stage.
    MediaPlay,
    /// Pause the video on the stage (the picture stays).
    MediaPause,
    /// Video back to 0:00 and play.
    MediaRestart,
    // ---- v0.816.0: the next 20 ----
    /// Screen black (true) or back on (false). The verse stays; only the congregation screen goes dark.
    Blackout(bool),
    /// Next (+1) or previous (-1) translation in the menu.
    SwitchTranslation(i8),
    /// Jump to the last verse of the chapter on stage ("final verse").
    LastVerse,
    /// Read aloud from the verse on stage up to this verse of the same chapter (0 = to the end of the chapter).
    ReadUntil(u16),
    /// Show / hide the list of scriptures shown so far.
    History(bool),
    /// Show / hide the Bible browser (books, chapters, verses).
    Browser(bool),
    /// Open / close the Media & slides window.
    MediaWindow(bool),
    /// Dismiss every card waiting for review.
    DismissAll,
    /// How the congregation stage looks: "classic", "lower_third", "chroma", "large_print".
    Look(&'static str),
    /// Put the congregation stage on screen N (1-based).
    StageScreen(u8),
    /// Countdown of N minutes, shown on the stage.
    TimerStart(u16),
    TimerPause,
    TimerResume,
    TimerStop,
    /// Put the timer / the clock on the stage.
    ShowTimer,
    ShowClock,
    /// Show / hide the Greek and Hebrew (Strong's) panel.
    Strongs(bool),
    /// "What is the Greek for love": `lang` is 'G', 'H' or '?' (either), `word` the English word.
    OriginalFor { lang: char, word: String },
}

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub title: String,
    pub items: Vec<String>,
}

#[derive(Default, Clone, Debug)]
pub struct ParseOutcome {
    /// Confidence >= floor: auto-stage candidates.
    pub detections: Vec<Detection>,
    /// Below floor / quotations / book-only: operator must decide.
    pub confirms: Vec<Detection>,
    /// Honest out-of-range reports (never staged silently).
    pub errors: Vec<RefError>,
    pub commands: Vec<Command>,
    /// "Three things" enumerated-list suggestion cards.
    pub suggestions: Vec<Suggestion>,
    /// False for non-speech ASR artifacts ("[BLANK_AUDIO]") — no error status.
    pub is_speech: bool,
}

impl ParseOutcome {
    pub fn is_empty(&self) -> bool {
        self.detections.is_empty()
            && self.confirms.is_empty()
            && self.errors.is_empty()
            && self.commands.is_empty()
            && self.suggestions.is_empty()
    }
}

/// v0.853.0: should a navigation command in this utterance be suppressed
/// because real scripture content is present? An explicit reference, an
/// out-of-range error or a non-quotation confirm all block ("next is John 3"
/// stages John 3, it does not advance first). A QUOTATION confirm does not:
/// "The evidence of things not seen. Next verse." is the preacher finishing
/// the verse and asking for the next one — the advance must still run.
pub fn nav_blocked_by(out: &ParseOutcome) -> bool {
    !out.detections.is_empty()
        || out.confirms.iter().any(|d| d.kind != DetectionKind::Quotation)
        || !out.errors.is_empty()
}

/// What the parser knows about the live service so far.
#[derive(Default, Clone, Debug)]
pub struct ParseContext {
    /// The passage currently on stage (for bare "verse N" and "that verse").
    pub last: Option<Passage>,
    /// A book the preacher mentioned without a chapter ("we go to Psalms")
    /// and that is still waiting for its chapter/verse (v0.853.0).
    /// "23 verse 1" after "We go to Psalms." completes as Psalms 23:1.
    pub pending_book: Option<u8>,
    /// v0.890.0 — the operator's card gates (words / min %). Default is
    /// v0.856 exactly; the settings window edits `self.parse_ctx.quote_gates`.
    pub quote_gates: QuoteGates,
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kw {
    None,
    Verse,
    Chapter,
    Range,
    Colon,
}

#[derive(Clone, Debug)]
struct Tok {
    text: String,
    num: Option<u16>,
    kw: Kw,
}

impl Tok {
    fn number(text: String, n: u16) -> Self {
        Tok { text, num: Some(n), kw: Kw::None }
    }
}

/// Words that map to numbers when spoken ("three sixteen" -> 3, 16).
const NUM_WORDS: &[(&str, u16)] = &[
    ("zero", 0), ("one", 1), ("won", 1), ("two", 2), ("too", 2),
    ("three", 3), ("tree", 3), ("four", 4), ("for", 4), ("five", 5),
    ("six", 6), ("sex", 6), ("seven", 7), ("eight", 8), ("ate", 8),
    ("nine", 9), ("ten", 10), ("eleven", 11), ("twelve", 12), ("thirteen", 13),
    ("fourteen", 14), ("fifteen", 15), ("sixteen", 16), ("seventeen", 17),
    ("eighteen", 18), ("nineteen", 19), ("twenty", 20), ("thirty", 30),
    ("forty", 40), ("fifty", 50), ("sixty", 60), ("seventy", 70),
    ("eighty", 80), ("ninety", 90), ("hundred", 100),
    ("first", 1), ("second", 2), ("third", 3), ("fourth", 4), ("fifth", 5),
    ("sixth", 6), ("seventh", 7), ("eighth", 8), ("ninth", 9), ("tenth", 10),
    ("eleventh", 11), ("twelfth", 12), ("thirteenth", 13), ("fourteenth", 14),
    ("fifteenth", 15), ("sixteenth", 16), ("seventeenth", 17),
    ("eighteenth", 18), ("nineteenth", 19), ("twentieth", 20),
    ("thirtieth", 30), ("fortieth", 40), ("fiftieth", 50),
];

const VERSE_KW: &[&str] = &["verse", "verses", "verse's"];
const CHAPTER_KW: &[&str] = &["chapter", "chapters", "chapters'", "chapter's"];
const RANGE_KW: &[&str] = &["through", "thru", "to", "till", "until"];
// "to" is both a range word and the number 2; disambiguated by neighbours.

/// Tokenize a transcript into words with optional numeric values.
fn tokenize(text: &str) -> Vec<Tok> {
    let lower = text.to_ascii_lowercase();
    let mut toks: Vec<Tok> = Vec::new();
    let mut cur = String::new();

    let push_word = |w: &str, toks: &mut Vec<Tok>| {
        if w.is_empty() {
            return;
        }
        // bare digits ("23", "316")
        if let Ok(n) = w.parse::<u16>() {
            toks.push(Tok::number(w.to_string(), n));
            return;
        }
        // number words
        if let Some(&(_, n)) = NUM_WORDS.iter().find(|(k, _)| *k == w) {
            // combine tens+units ("twenty five" -> 25) and hundreds
            if let Some(last) = toks.last_mut() {
                if let Some(l) = last.num {
                    // "twenty five" -> 25, "one hundred" -> 100, "hundred fifty" -> 150
                    if (l % 10 == 0 && l >= 20 && n < 10)
                        || (l < 10 && n == 100)
                        || (l == 100 && n < 100)
                    {
                        last.num = Some(l + n);
                        last.text = format!("{}+{}", last.text, w);
                        return;
                    }
                }
            }
            toks.push(Tok::number(w.to_string(), n));
            return;
        }
        // keywords
        let kw = if VERSE_KW.contains(&w) {
            Kw::Verse
        } else if CHAPTER_KW.contains(&w) {
            Kw::Chapter
        } else if RANGE_KW.contains(&w) {
            Kw::Range
        } else {
            Kw::None
        };
        toks.push(Tok { text: w.to_string(), num: None, kw });
    };

    let chars: Vec<char> = lower.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphanumeric() {
            cur.push(c);
            i += 1;
            continue;
        }
        // flush current word
        if !cur.is_empty() {
            push_word(&cur.clone(), &mut toks);
            cur.clear();
        }
        if c == ':' {
            toks.push(Tok { text: ":".into(), num: None, kw: Kw::Colon });
        } else if c == '-' || c == '\u{2013}' || c == '\u{2014}' {
            toks.push(Tok { text: "-".into(), num: None, kw: Kw::Range });
        }
        i += 1;
    }
    if !cur.is_empty() {
        push_word(&cur, &mut toks);
    }
    toks
}

// ---------------------------------------------------------------------------
// Book matching
// ---------------------------------------------------------------------------

pub struct BookMatch {
    pub index: u8,
    pub score: f32,
    /// number of tokens consumed
    pub len: usize,
}

/// Best fuzzy book match for a window of tokens starting at `start`.
/// Windows of 1..=4 word-tokens are compared against every alias.
fn match_book_at(toks: &[Tok], start: usize) -> Option<BookMatch> {
    let mut best: Option<BookMatch> = None;
    let mut joined = String::new();
    for len in 1..=4 {
        let i = start + len - 1;
        if i >= toks.len() {
            break;
        }
        let t = &toks[i];
        if t.kw == Kw::Colon {
            break;
        }
        if !joined.is_empty() {
            joined.push(' ');
        }
        joined.push_str(&t.text);

        // v0.853.0: compare against a precomputed LOWERCASE alias table.
        // The static aliases keep their canonical capitals ("Exodus"), but
        // the tokenized speech is lowercase — comparing them directly cost
        // every capitalized alias one edit of distance, which pushed e.g.
        // "exodus 5" (conf 0.7499) just under the 0.75 auto-stage floor and
        // silently turned dozens of chapters into confirm cards.
        for (bi, alias) in books::aliases_lower() {
            let s = books::similarity(&joined, alias);
                // small structural boost for exact alias hits
                let score = if s > 0.999 { 1.0 } else { s };
                if score >= 0.68 {
                    let cand = BookMatch { index: *bi, score, len };
                    let better = match &best {
                        None => true,
                        Some(b) => {
                            let sdiff = (cand.score - b.score) > 0.02;
                            let same = (cand.score - b.score).abs() <= 0.02;
                            (sdiff) || (same && cand.len > b.len)
                                || (same && cand.len == b.len
                                    && BOOKS[cand.index as usize].freq
                                        > BOOKS[b.index as usize].freq)
                        }
                    };
                    if better {
                        best = Some(cand);
                    }
                }
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Suggestion engine for out-of-range references
// ---------------------------------------------------------------------------

/// Closest real candidates for an invalid (book, chapter, verse):
/// 1. same chapter's last verse, 2. same verse in adjacent chapters,
/// 3. same numbers in the adjacent books ("1 Corinthians 5:17" ->
///    "2 Corinthians 5:17" — the PRD's flagship suggestion).
pub fn suggest_alternatives(bible: &Bible, book: u8, chapter: u16, verse: u16) -> Vec<Passage> {
    let mut out: Vec<Passage> = Vec::new();
    let mut push = |p: Passage| {
        if !out.contains(&p) && out.len() < 3 {
            out.push(p);
        }
    };
    // 1. the classic preaching confusion: adjacent book, same numbers
    //    ("1 Corinthians 5:17" -> "2 Corinthians 5:17" — PRD's flagship fix)
    if (book as usize + 1) < BOOKS.len()
        && bible.exists(book as usize + 1, chapter as usize, verse as usize)
    {
        push(Passage::single(book + 1, chapter, verse));
    }
    if book > 0 && bible.exists(book as usize - 1, chapter as usize, verse as usize) {
        push(Passage::single(book - 1, chapter, verse));
    }
    // 2. closest real verses in the same neighborhood
    let vc = bible.verse_count(book as usize, chapter as usize);
    if vc > 0 {
        push(Passage::single(book, chapter, vc as u16));
    }
    if chapter > 1 && bible.exists(book as usize, (chapter - 1) as usize, verse as usize) {
        push(Passage::single(book, chapter - 1, verse));
    }
    if chapter as usize + 1 < bible.chapter_count(book as usize)
        && bible.exists(book as usize, chapter as usize + 1, verse as usize)
    {
        push(Passage::single(book, chapter + 1, verse));
    }
    out
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

const NON_SPEECH: &[&str] = &[
    "blank audio", "music", "[music]", "silence", "inaudible", "crosstalk",
    "[blank_audio]", "[silence]", "[inaudible]", "[crosstalk]", "applause",
    "[applause]", "[noise]", "noise",
];

pub fn parse(bible: &Bible, text: &str, ctx: &ParseContext, floor: f32) -> ParseOutcome {
    parse_with_recent(bible, text, ctx, floor, &[])
}

/// Same as [`parse`], but also given the text of a few recent utterances
/// (oldest first) to fall back on for quotation recognition, in case the
/// current utterance alone is a fragment of a quotation that a natural
/// pause split across two VAD segments — e.g. "For God so" … (pause) …
/// "loved the world, that he gave his only begotten Son". Neither fragment
/// contains a full match on its own; concatenated, it does. Callers decide
/// what counts as "recent enough" (a short, live time window) — this
/// function only tries what it's given.
pub fn parse_with_recent(bible: &Bible, text: &str, ctx: &ParseContext, floor: f32, recent: &[&str]) -> ParseOutcome {
    parse_full(bible, text, ctx, floor, recent, None)
}

/// Full form of [`parse`]. `alt_translation`, when given, is a second Bible
/// (with a short label such as "BBE") to also try for quotation recognition
/// — ONLY for quotation matching, never for reference parsing or the
/// structural chapter/verse checks, which stay on `bible` (kept KJV-
/// canonical on purpose: a different translation's versification can differ
/// by a verse or two, and reference navigation needs one steady, canonical
/// source of truth). A match against `alt_translation` is surfaced exactly
/// like any other quotation — always to the confirm queue, never
/// auto-staged — with a note naming which translation it matched, so the
/// operator approves it knowingly rather than the app silently switching
/// translations underneath them.
pub fn parse_full(
    bible: &Bible,
    text: &str,
    ctx: &ParseContext,
    floor: f32,
    recent: &[&str],
    alt_translation: Option<(&Bible, &str)>,
) -> ParseOutcome {
    // v0.855.0: bracketed sound tags ("[MUSIC]", "(upbeat guitar music)",
    // "[APPLAUSE]") are stripped BEFORE anything parses — their words used to
    // dilute the quotation match and could steer it to a wrong verse. Real
    // asides ("Romans (chapter 8)") contain non-audio words and stay intact;
    // display text elsewhere keeps exactly what the engine heard.
    let trimmed = crate::soundtag::strip_sound_tags(text.trim()).trim().to_string();
    let lower_trim = trimmed
        .to_ascii_lowercase()
        .replace(['[', ']', '(', ')', '_'], " ");
    let stripped = lower_trim.split_whitespace().collect::<Vec<_>>().join(" ");

    // ASR artifacts: "[BLANK_AUDIO]" etc. — no candidate, no error (PRD 6.5).
    if stripped.is_empty() || NON_SPEECH.iter().any(|n| stripped == *n) {
        return ParseOutcome { is_speech: false, ..Default::default() };
    }

    let mut out = ParseOutcome { is_speech: true, ..Default::default() };

    crate::commands::extract_commands(&stripped, &mut out);
    extract_suggestions(&stripped, &mut out);
    // "read till verse 20" is a command about verse 20, not a reference to stage it
    let only_a_read_command = out.commands.iter().any(|c| matches!(c, Command::ReadUntil(_) | Command::StageScreen(_) | Command::TimerStart(_)));
    if !only_a_read_command {
        extract_references(bible, &trimmed, ctx, floor, &mut out);
    }

    // Quotation recognition (PRD 6.2): only when nothing else was understood.
    //
    // v0.812.0: an UNCERTAIN book-only guess ("kings" in "...made us kings and
    // priests" looks like 1 Kings, 54%) must not block a real quotation match.
    // That blocked Revelation 1:6 completely: four "1 Kings" cards, no verse.
    // A quotation that fits wins over weak book-only guesses, which are dropped
    // only when a quotation is actually found.
    let only_weak_books = !out.confirms.is_empty()
        && out
            .confirms
            .iter()
            .all(|d| d.kind == DetectionKind::BookOnly && d.confidence < 0.6);
    if out.detections.is_empty()
        && (out.confirms.is_empty() || only_weak_books)
        && out.errors.is_empty()
        && out.is_speech
    {
        let mut hit = quote_detection(bible, &stripped, "quotation match", ctx.quote_gates);
        if hit.is_none() && !recent.is_empty() {
            let mut combined = crate::soundtag::strip_sound_tags(&recent.join(" "));
            if !combined.is_empty() {
                combined.push(' ');
            }
            combined.push_str(&stripped);
            hit = quote_detection(bible, &combined, "quotation match across recent utterances", ctx.quote_gates);
        }

        // v0.853.0: the widened quote matcher surfaces MORE weak single-line
        // hits than v0.852 did (a 55% match on a paraphrase, a stray verse
        // whose opening words appear late in the sentence). A weak hit must
        // not END the search: the operator's active translation and the
        // pieces assembler (v0.816.0) may both know better, and they only
        // used to get a chance when the single-line search found NOTHING.
        let weak = |h: &Option<Detection>| h.as_ref().map_or(true, |d| d.pct < SINGLE_PIECE_MIN_PCT);
        if weak(&hit) {
            if let Some((alt, label)) = alt_translation {
                let note = format!("quotation match ({label} translation)");
                let mut alt_hit = quote_detection(alt, &stripped, &note, ctx.quote_gates);
                if alt_hit.is_none() && !recent.is_empty() {
                    let mut combined = crate::soundtag::strip_sound_tags(&recent.join(" "));
                    if !combined.is_empty() {
                        combined.push(' ');
                    }
                    combined.push_str(&stripped);
                    alt_hit = quote_detection(alt, &combined, &note, ctx.quote_gates);
                }
                if alt_hit.as_ref().map_or(false, |a| hit.as_ref().map_or(true, |h| a.pct > h.pct)) {
                    hit = alt_hit;
                }
            }
            if weak(&hit) {
                let in_kjv = pieces_detection(bible, recent, &stripped, "quotation match", ctx.quote_gates);
                let in_alt = alt_translation.and_then(|(alt, label)| pieces_detection(alt, recent, &stripped, &format!("quotation match ({label} translation)"), ctx.quote_gates));
                let best = match (in_kjv, in_alt) {
                    (Some(k), Some(a)) => Some(if a.pct > k.pct { a } else { k }),
                    (k, a) => k.or(a),
                };
                if let Some(p) = best {
                    if hit.as_ref().map_or(true, |h| p.pct > h.pct) {
                        hit = Some(p);
                    }
                }
            }
        }
        if let Some(d) = hit {
            // v0.855.0 — the NAMED-VERSE LOCK (screenshot 1). The operator
            // put "Luke 1:17" on stage; the preacher then read the verse.
            // The half-line "turn the heart of the father to the children"
            // also fits Malachi 4:6 (Luke quotes Malachi), which used to
            // card — and with auto-approve on, DISPLACE the named verse.
            // Rule: while a verse is on stage and the incoming fragment is
            // still explained by THAT verse's own text (span_fit clears the
            // same floor the fragment would have needed), no other verse —
            // however similar — may card or auto-stage. When the preacher
            // genuinely moves on, the fragment stops fitting the staged
            // verse and detection resumes normally.
            let locked = ctx.last.filter(|p| p.chapter > 0);
            let mut suppressed_by_lock = false;
            if let Some(l) = locked {
                if d.passage != l {
                    let fit = bible
                        .span_fit_pct(&stripped, l.book as usize, l.chapter as usize, l.from as usize)
                        .unwrap_or(0);
                    let heard_words = stripped.split_whitespace().count();
                    // v0.890.0: the named-verse lock now uses the operator's card
                    // bar too — the fragment stays suppressed while it fits the
                    // staged verse at least as well as the slider demands.
                    let floor = ctx.quote_gates.card_min_pct(heard_words);
                    suppressed_by_lock = fit >= floor;
                }
            }
            if !suppressed_by_lock {
                if only_weak_books {
                    out.confirms.clear();
                }
                out.confirms.push(d);
            }
        }
    }
    out
}

/// v0.890.0: the v0.856 constants became operator settings (see
/// `set_suggest_gates` above). These keep their names for compatibility and
/// hold the v0.856 VALUES — the documented defaults the settings window
/// starts from. Nothing reads them at runtime any more.
pub const MIN_QUOTE_PCT: u8 = 50;
pub const SHORT_QUOTE_MIN_PCT: u8 = 70;

/// v0.856.0 — the auto-approve bars, chosen by the whole-Bible grid
/// (`cargo run --release -p rf-core --example biblegrid`, results and method
/// in docs/GRID-PROV19-21-v0856.md):
///
/// 1,698,315 deduplicated 4-6-word windows of the entire KJV (the words a
/// preacher can say IN PASSING) were fed through the matcher: the shipped
/// v0.855 rule auto-staged 653 of every 1,000 of them — that is exactly how
/// "there are many devices" put Proverbs 19:21 on the big screen. Sweeping
/// W (minimum words to auto-stage) × A (accuracy bar) showed the ACCURACY
/// BAR is the lever, not the 3-word tier: W=5/A=90 kills 91.2% of the
/// false-show surface (653 -> 57.25 per 1,000) while 98.2% of real verse
/// reads are still identified (95.7% still auto-stage) and the Proverbs
/// 19:21 false fragments all go silent. The losing trade-off, measured:
/// true quotes scoring below the bar (the 73% Revelation 1:6/5:10 tie, damaged
/// one-word-off readings) now wait as a card instead of self-staging — one
/// operator click, never a wrong verse.
pub const AUTO_MIN_WORDS: u8 = 5;
pub const AUTO_STAGE_PCT: u8 = 90;
/// How many ranked candidates the parser hands to the UI (the UI decides how many to show).
pub const MAX_QUOTE_CANDIDATES: usize = 8;

/// Turn spoken words into a quotation detection, or None.
///
/// A fragment that identifies exactly one verse behaves as it always has
/// (confidence 0.62, never auto-staged by the parser). A fragment that fits
/// SEVERAL verses ("the grace of our Lord Jesus Christ ...") used to produce
/// nothing; it now yields the best candidate with every other real candidate
/// in `alternatives`, each with its own accuracy percentage in `alt_pct`.
fn quote_detection(bible: &Bible, text: &str, note: &str, gates: QuoteGates) -> Option<Detection> {
    // v0.890.0: a trailing "So." / "Amen." is not evidence. Score the line
    // both ways (with and without the filler tail) and keep the better
    // reading — ported from v0.889, where it let the finished
    // "...able to make you wise. So." line reach the bar.
    let trimmed = rf_bible::strip_filler_tail(text);
    let plain = quote_detection_raw(bible, text, note, gates);
    if trimmed == text {
        return plain;
    }
    let clean = quote_detection_raw(bible, &trimmed, note, gates);
    match (plain, clean) {
        (Some(p), Some(c)) => Some(if c.pct > p.pct { c } else { p }),
        (p, c) => p.or(c),
    }
}

fn quote_detection_raw(bible: &Bible, text: &str, note: &str, gates: QuoteGates) -> Option<Detection> {
    let word_count = text.split_whitespace().count();
    // v0.890.0: the operator's word floor. An utterance shorter than the
    // setting may not found a quotation card at all (3 = v0.856, 5 = v0.889).
    if word_count < gates.min_words as usize {
        return None;
    }
    let min_pct = gates.card_min_pct(word_count);
    let ranked: Vec<_> = bible
        .match_quotations_ranked(text, MAX_QUOTE_CANDIDATES)
        .into_iter()
        .filter(|h| h.pct >= min_pct)
        .collect();
    if ranked.is_empty() {
        // nothing ranks well enough: the older unique-window tier may still know
        let (b, c, v) = bible.match_quotation(text)?;
        // v0.853.0 late fix: a unique-tier hit must still look like the WHOLE
        // utterance. "and he told" uniquely opens Genesis 37:10, but "and he
        // told them the story about the fisherman and his net again" is
        // pulpit chatter that happens to BEGIN like the verse — at pct=0 it
        // used to raise a card and, with auto-show on, even stage itself.
        // The same F1 math the ranked scorer uses gates it: chatter stays
        // silent, real verses ("let this mind being you" -> Philippians
        // 2:5) still card.
        let q = bible.quote_pct(text, b, c, v).unwrap_or(0);
        if q < min_pct {
            return None;
        }
        return Some(Detection {
            kind: DetectionKind::Quotation,
            passage: Passage::single(b as u8, c as u16, v as u16),
            confidence: 0.62,
            alternatives: vec![],
            alt_pct: vec![],
            pct: 0,
            words: word_count as u8,
            note: Some(format!("{note} \u{2014} never auto-staged")),
        });
    }
    let top = ranked[0];
    let rest = &ranked[1..];
    Some(Detection {
        kind: DetectionKind::Quotation,
        passage: Passage::single(top.book as u8, top.chapter as u16, top.verse as u16),
        // a lone candidate keeps the long-standing 0.62; several candidates carry their own accuracy.
        // Always below the auto-stage floor: a quotation is never auto-staged by the parser.
        confidence: if rest.is_empty() { 0.62 } else { (top.pct as f32 / 100.0).min(0.74) },
        alternatives: rest.iter().map(|h| Passage::single(h.book as u8, h.chapter as u16, h.verse as u16)).collect(),
        alt_pct: rest.iter().map(|h| h.pct).collect(),
        pct: top.pct,
        words: word_count as u8,
        note: Some(if rest.is_empty() {
            format!("{note} \u{2014} never auto-staged")
        } else {
            format!("{note} \u{2014} {} possible verses, best first", ranked.len())
        }),
    })
}

/// Lowest accuracy for a match built from a single piece.
const SINGLE_PIECE_MIN_PCT: u8 = 80;

/// Several short pieces said one after another, assembled into one quotation suggestion (v0.816.0).
/// Always a suggestion for the operator (or the auto-approve rule, which needs a clear winner).
/// v0.890.0: the assembled line also answers to the operator's card word floor
/// ("for god so" + a pause + "loved the world" is 8 combined words and still
/// suggests; a lone 3-word piece respects the same floor as every other path).
fn pieces_detection(bible: &Bible, recent: &[&str], newest: &str, note: &str, gates: QuoteGates) -> Option<Detection> {
    let mut pieces: Vec<&str> = recent.to_vec();
    pieces.push(newest);
    let combined_words: usize = pieces.iter().map(|p| p.split_whitespace().count()).sum();
    if combined_words < gates.min_words as usize {
        return None;
    }
    let m = bible.match_pieces(&pieces, MAX_QUOTE_CANDIDATES);
    let top = *m.hits.first()?;
    // one piece on its own must be a near-exact match: looser readings of a single sentence are
    // paraphrases (another translation's wording), which are offered per translation instead
    if m.pieces_used <= 1 && top.pct < SINGLE_PIECE_MIN_PCT {
        return None;
    }
    let rest = &m.hits[1..];
    let how = if m.pieces_used > 1 { format!("assembled from {} spoken pieces", m.pieces_used) } else { "heard with a few words wrong".to_string() };
    let words = pieces.iter().map(|p| p.split_whitespace().count()).sum::<usize>() as u8;
    Some(Detection {
        kind: DetectionKind::Quotation,
        passage: Passage::single(top.book as u8, top.chapter as u16, top.verse as u16),
        confidence: if rest.is_empty() { 0.62 } else { (top.pct as f32 / 100.0).min(0.74) },
        alternatives: rest.iter().map(|h| Passage::single(h.book as u8, h.chapter as u16, h.verse as u16)).collect(),
        alt_pct: rest.iter().map(|h| h.pct).collect(),
        pct: top.pct,
        words,
        note: Some(format!("{note} {how}")),
    })
}

/// How far back "recent pieces" may reach (v0.816.0). The gap is wider than a breath on purpose:
/// the operator's real pieces were 2 to 7 seconds apart.
pub const RECENT_MAX_LINES: usize = 14;
pub const RECENT_MAX_GAP_MS: u64 = 9_000;
pub const RECENT_MAX_SPAN_MS: u64 = 45_000;

/// One finished piece of speech (what the app keeps in its transcript).
#[derive(Clone, Debug)]
pub struct RecentLine {
    pub ms: u64,
    pub text: String,
    /// Something was recognised in it. Kept: it can still be part of a quotation ("Next." is simply
    /// ignored by the matcher). What ends the search is `after_ms`, below.
    pub flagged: bool,
}

/// The newest pieces worth joining together, oldest first. Stops at a long silence, at a piece
/// older than `RECENT_MAX_SPAN_MS`, after `RECENT_MAX_LINES`, and at anything at or before
/// `after_ms` (the moment a quotation was last put forward: those words are spent).
pub fn pick_recent_pieces(lines: &[RecentLine], now_ms: u64, after_ms: u64) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cursor = now_ms;
    for line in lines.iter().rev() {
        if out.len() >= RECENT_MAX_LINES
            || (after_ms > 0 && line.ms <= after_ms)
            || cursor.saturating_sub(line.ms) > RECENT_MAX_GAP_MS
            || now_ms.saturating_sub(line.ms) > RECENT_MAX_SPAN_MS
        {
            break;
        }
        out.push(line.text.clone());
        cursor = line.ms;
    }
    out.reverse();
    out
}

/// Lowercase + whitespace-normalize a raw utterance the same way `parse`
/// does, so command tests and callers can inspect command matching without
/// going through the full parser.
pub fn normalize_for_commands(text: &str) -> String {
    let lower = text
        .to_ascii_lowercase()
        .replace(['[', ']', '(', ')', '_'], " ");
    lower.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub static TRANSLATIONS: &[(&str, &str)] = &[
    ("kjv", "KJV"),
    ("bbe", "BBE"),
    ("niv", "NIV"),
    ("esv", "ESV"),
    ("nkjv", "NKJV"),
    ("nlt", "NLT"),
    ("nasb", "NASB"),
    ("amp", "AMP"),
    ("msg", "MSG"),
    ("asv", "ASV"),
    ("ylt", "YLT"),
    ("akjv", "AKJV"),
];

pub fn translation_for(word: &str) -> Option<&'static str> {
    TRANSLATIONS
        .iter()
        .find(|(k, _)| *k == word)
        .map(|(_, v)| *v)
}

fn extract_suggestions(text: &str, out: &mut ParseOutcome) {
    // Strict enumerated pattern first (PRD 7.4): "there are N things ... : a, b and c"
    let t = text;
    let Some(pos) = t.find("there are") else { return };
    let rest = &t[pos + "there are".len()..];
    let words: Vec<&str> = rest.split_whitespace().collect();
    let Some(first) = words.first() else { return };
    let n: u16 = match *first {
        "two" | "2" => 2,
        "three" | "3" => 3,
        "four" | "4" => 4,
        "five" | "5" => 5,
        _ => return,
    };
    if !words.get(1).map(|w| w.starts_with("thing")).unwrap_or(false) {
        return;
    }
    // Items live after the colon (or after "remember" as a fallback).
    let after_things = rest.split_once("things").map(|(_, r)| r).unwrap_or("");
    let items_raw = match after_things.split_once(':') {
        Some((_, r)) => r,
        None => match after_things.find("remember") {
            Some(i) => &after_things[i + "remember".len()..],
            None => return,
        },
    };
    // Split on commas and "and".
    let mut items: Vec<String> = Vec::new();
    for chunk in items_raw.split(',') {
        for piece in chunk.split(" and ") {
            let p = piece.trim().trim_end_matches(['.', '!', '?']).trim();
            if p.is_empty() || p == "and" {
                continue;
            }
            if p.split_whitespace().count() <= 4 {
                items.push(title_case(p));
            }
        }
    }
    items.retain(|i| i.split_whitespace().count() >= 1);
    if items.len().clamp(2, 5) == items.len() && items.len() as u16 == n {
        out.suggestions.push(Suggestion {
            title: format!("{n} sermon points suggested"),
            items,
        });
    }
}

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Reference extraction (the structural core)
// ---------------------------------------------------------------------------

fn extract_references(bible: &Bible, text: &str, ctx: &ParseContext, floor: f32, out: &mut ParseOutcome) {
    let mut toks = tokenize(text);
    // v0.853.0 self-correction: "Second Chronicles no second Corinthians
    // chapter 5 verse 17" — the preacher changed their mind mid-sentence.
    // Everything before the LAST correction marker is a false start and must
    // not stage (the old parse staged neither book, or worse, read "second"
    // as the previous book's chapter). Command and quotation recognition
    // still see the whole line; only references respect the correction.
    if let Some(start) = find_correction_split(&toks) {
        toks.drain(..start);
    }
    let n = toks.len();
    let mut i = 0;

    while i < n {
        // Book anchor?
        let Some(bm) = match_book_at(&toks, i) else {
            // Bare "verse N" relative to the stage (PRD 6.3, shipped M0)
            if toks[i].kw == Kw::Verse {
                if let Some(v) = next_number(&toks, i + 1) {
                    if let Some(last) = ctx.last {
                        if last.chapter > 0 {
                            out.detections.push(Detection {
                                kind: DetectionKind::Relative,
                                passage: Passage::single(last.book, last.chapter, v),
                                confidence: 0.95,
                                alternatives: vec![],
                                alt_pct: vec![],
                                pct: 0,
                                words: 0,
                                note: Some("relative to on-stage chapter".into()),
                            });
                            i += 2;
                            continue;
                        }
                    }
                }
            }
            // "that verse" / "the same verse" — repeat the on-stage passage
            if (toks[i].text == "that" || toks[i].text == "same")
                && i + 1 < n
                && toks[i + 1].kw == Kw::Verse
            {
                if let Some(last) = ctx.last {
                    out.detections.push(Detection {
                        kind: DetectionKind::Relative,
                        passage: last,
                        confidence: 0.9,
                        alternatives: vec![],
                        alt_pct: vec![],
                        pct: 0,
                        words: 0,
                        note: Some("repeated on-stage reference".into()),
                    });
                    i += 2;
                    continue;
                }
            }
            // v0.853.0: a book the preacher mentioned earlier without its
            // chapter ("we go to Psalms") is still waiting: "23 verse 1."
            // now completes as Psalms 23:1 instead of falling back to the
            // on-stage chapter. Needs a real chapter cue (a number that is
            // followed by verse/chapter words or a second number, or the
            // word "chapter") so stray numbers in speech stay harmless.
            if let Some(pb) = ctx.pending_book {
                if pending_cue_ahead(&toks, i) {
                    let bm = BookMatch { index: pb, score: 0.999, len: 0 };
                    // fall through to the shared chapter/verse machinery
                    // with a zero-length book anchor at i
                    let ni = finish_book_match(bible, &toks, i, bm, ctx, floor, out, true);
                    i = ni;
                    continue;
                }
            }
            i += 1;
            continue;
        };

        // Peek-ahead: a fuzzy anchor ("to john" ~ "two john") must never
        // outrank a nearby exact anchor ("john"). If an exact match exists
        // within a few tokens, skip the fuzzy one.
        if bm.score < 0.999 {
            let mut exact_ahead = false;
            for k in 1..=3 {
                if i + k >= n {
                    break;
                }
                if let Some(m2) = match_book_at(&toks, i + k) {
                    if m2.score >= 0.999 {
                        exact_ahead = true;
                        break;
                    }
                }
            }
            if exact_ahead {
                i += 1;
                continue;
            }
        }

        let next = finish_book_match(bible, &toks, i, bm, ctx, floor, out, false);
        i = next;
    }
}

/// The shared chapter/verse collector (v0.853.0): everything that happens
/// after a book anchor is found — find the chapter, collect the verses,
/// validate, emit. Returns the next scan position. Used by the normal path
/// AND by the pending-book completion ("we go to Psalms." + "23 verse 1."),
/// which enters with a zero-length book anchor at `i`.
fn finish_book_match(
    bible: &Bible,
    toks: &[Tok],
    i: usize,
    bm: BookMatch,
    ctx: &ParseContext,
    floor: f32,
    out: &mut ParseOutcome,
    pending: bool,
) -> usize {
    let n = toks.len();
    let book = bm.index;
    let mut j = i + bm.len;

    // ---- find chapter ----
    let mut chapter: Option<u16> = None;
    let mut chapter_kw = false;
    let mut verses_from_split: Option<u16> = None;
    while j < n {
        let t = &toks[j];
        if t.kw == Kw::Chapter {
            chapter_kw = true;
            j += 1;
            continue;
        }
        if t.kw == Kw::Colon {
            j += 1;
            continue;
        }
        if let Some(num) = t.num {
            // v0.853.0: an ordinal that opens the NEXT numbered book ("second
            // chronicles ... second corinthians chapter 5 verse 17") is not
            // this book's chapter. A strong book anchor starting right here
            // ends this reference before the number is swallowed as a chapter.
            if let Some(other) = match_book_at(toks, j) {
                if other.score >= 0.86 {
                    break;
                }
            }
            if num <= 150 {
                chapter = Some(num);
                j += 1;
            } else if let Some((c, v)) = try_split(num) {
                // digit-split: ASR merged "three sixteen" -> "316"
                chapter = Some(c);
                verses_from_split = Some(v);
                j += 1;
            } else {
                // impossible chapter — emit as-is, validation reports honestly
                chapter = Some(num);
                j += 1;
            }
            break;
        }
        if t.kw == Kw::Verse {
            break; // "John verse 3"? — malformed, stop cleanly
        }
        // another strong book anchor ahead? then this ref has no chapter
        if let Some(other) = match_book_at(toks, j) {
            if other.score >= 0.86 {
                break;
            }
        }
        j += 1;
    }

    let Some(chapter) = chapter else {
        // Book-only mention (e.g. "second samuel") — operator decides.
        let conf = if bm.score >= 0.86 { bm.score * 0.8 } else { bm.score * 0.7 };
        // v0.812.0: an UNCERTAIN book guess ("kings" inside "...made those kings
        // entreeced", "Thessalonians" -> Esther) is only worth a card when the
        // speaker said little more than the name (5 words or fewer). In a sentence it is just a
        // word that happens to look like a book, and the cards piled up.
        if bm.score < 0.86 && toks.len() > 5 {
            return i + bm.len.max(1);
        }
        out.confirms.push(Detection {
            kind: DetectionKind::BookOnly,
            passage: Passage { book, chapter: 0, from: 0, to: 0 },
            confidence: conf,
            alternatives: vec![],
            alt_pct: vec![],
            pct: 0,
            words: 0,
            note: Some(
                if bm.score >= 0.86 {
                    "book only — no chapter heard"
                } else {
                    "uncertain book — no chapter heard"
                }
                .into(),
            ),
        });
        return i + bm.len.max(1);
    };

    // ---- collect trailing verse numbers / ranges ----
    let mut verses: Vec<u16> = Vec::new();
    let mut verse_kw = false;
    let mut range: Option<(u16, u16)> = None;
    let mut ended = false;

    while j < n {
        let t = &toks[j];
        match t.kw {
            Kw::Verse => {
                verse_kw = true;
                j += 1;
            }
            Kw::Colon => {
                j += 1;
            }
            Kw::Chapter => {
                // a new "chapter" mention ends this reference
                break;
            }
            Kw::Range => {
                // "16 through 18" / "16-18" / "16 to 18"
                if let Some(a) = verses.last().copied() {
                    if let Some(pos) = find_number_pos(toks, j + 1, 4) {
                        let b = toks[pos].num.unwrap_or(0);
                        if a > 0 && b >= a {
                            range = Some((a, b));
                            verses.clear();
                            j = pos + 1;
                            continue;
                        }
                    }
                }
                j += 1;
            }
            Kw::None => {
                if let Some(num) = t.num {
                    // stop if a new book anchor begins here
                    if let Some(other) = match_book_at(toks, j) {
                        if other.score >= 0.86 && other.len >= 1 {
                            // "1 corinthians 5 17" — a number can also be
                            // the start of a numbered book ("1 corinthians").
                            // Only treat as book if the NEXT token is a book word.
                            let is_book_start = toks
                                .get(j + 1)
                                .map(|nx| {
                                    matches!(nx.text.as_str(), "samuel" | "kings" | "chronicles" | "corinthians" | "thessalonians" | "timothy" | "peter" | "john")
                                })
                                .unwrap_or(false);
                            if is_book_start {
                                ended = true;
                                break;
                            }
                        }
                    }
                    if verses.len() < 3 {
                        verses.push(num);
                    }
                    j += 1;
                } else {
                    if let Some(other) = match_book_at(toks, j) {
                        if other.score >= 0.86 {
                            ended = true;
                            break;
                        }
                    }
                    j += 1; // filler word — tolerated
                }
            }
        }
        if ended {
            break;
        }
    }

    let _ = ended;
    if let Some(v) = verses_from_split {
        verses.push(v);
    }

    // ---- validate & emit ----
    emit_reference(bible, out, ctx, floor, EmitRef {
        book,
        book_score: bm.score,
        chapter,
        chapter_kw,
        verses,
        verse_kw,
        range,
        pending,
    });
    j.max(i + 1)
}

struct EmitRef {
    book: u8,
    book_score: f32,
    chapter: u16,
    chapter_kw: bool,
    verses: Vec<u16>,
    verse_kw: bool,
    range: Option<(u16, u16)>,
    /// v0.853.0: chapter/verse completed a book named in an earlier utterance
    pending: bool,
}

fn emit_reference(bible: &Bible, out: &mut ParseOutcome, _ctx: &ParseContext, floor: f32, r: EmitRef) {
    let name = BOOKS[r.book as usize].name;

    // chapter out of range — honest failure
    if r.chapter as usize > bible.chapter_count(r.book as usize) {
        let chapters = bible.chapter_count(r.book as usize);
        out.errors.push(RefError {
            message: format!(
                "{} has only {} chapters (no chapter {}).",
                name, chapters, r.chapter
            ),
            candidates: suggest_alternatives(bible, r.book, r.chapter, 1),
        });
        return;
    }

    // ---- chapter-only mention: stage verse 1 with a visible badge (PRD 6.1) ----
    if r.verses.is_empty() && r.range.is_none() {
        let confidence = (r.book_score * 0.9 + if r.chapter_kw { 0.05 } else { 0.0 }).clamp(0.0, 1.0);
        let det = Detection {
            kind: DetectionKind::ChapterOnly,
            passage: Passage::single(r.book, r.chapter, 1),
            confidence,
            alternatives: vec![],
                alt_pct: vec![],
                pct: 0,
                words: 0,
            note: Some("chapter-only — showing verse 1".into()),
        };
        if confidence >= floor {
            out.detections.push(det);
        } else {
            out.confirms.push(det);
        }
        return;
    }

    // ---- full reference assembly ----
    let passage = if let Some((a, b)) = r.range {
        Passage { book: r.book, chapter: r.chapter, from: a, to: b }
    } else {
        match r.verses.len() {
            1 => Passage::single(r.book, r.chapter, r.verses[0]),
            // Two trailing numbers: ASR often inserts an artifact between the
            // real ones ("five, for seventeen"); the LAST number is the verse,
            // the earlier one is kept as an alternative.
            _ => Passage::single(r.book, r.chapter, *r.verses.last().unwrap()),
        }
    };

    // verse-range sanity + validation
    if passage.from == 0 {
        out.errors.push(RefError {
            message: format!("Heard \"{} {}:0\" — verses start at 1.", name, passage.chapter),
            candidates: vec![Passage::single(r.book, r.chapter, 1)],
        });
        return;
    }
    let max_v = bible.verse_count(r.book as usize, r.chapter as usize);
    if passage.to as usize > max_v {
        let extra = if max_v == 1 { "" } else { "s" };
        let first_cand = suggest_alternatives(bible, r.book, r.chapter, passage.to);
        let did_you = first_cand
            .first()
            .map(|p| format!(" — did you mean {}?", p.label()))
            .unwrap_or_default();
        out.errors.push(RefError {
            message: format!(
                "{} {} has only {} verse{} (no verse {}){}",
                name, passage.chapter, max_v, extra, passage.to, did_you
            ),
            candidates: first_cand,
        });
        return;
    }
    if passage.from as usize > max_v {
        out.errors.push(RefError {
            message: format!(
                "{} {} has only {} verses (no verse {})",
                name, passage.chapter, max_v, passage.from
            ),
            candidates: suggest_alternatives(bible, r.book, r.chapter, passage.from),
        });
        return;
    }

    // build detection
    let mut confidence = 0.55 + 0.45 * r.book_score;
    if r.verses.len() > 1 && !r.verse_kw && r.range.is_none() {
        confidence -= 0.06; // artifact-tolerated assembly: slightly less sure
    }
    if r.verse_kw {
        confidence += 0.02;
    }
    confidence = confidence.clamp(0.0, 1.0);

    let mut alternatives = Vec::new();
    if r.verses.len() > 1 && r.range.is_none() {
        for v in &r.verses {
            if bible.exists(r.book as usize, r.chapter as usize, *v as usize) {
                let p = Passage::single(r.book, r.chapter, *v);
                if p != passage {
                    alternatives.push(p);
                }
            }
        }
    }

    let det = Detection {
        kind: DetectionKind::Explicit,
        passage,
        confidence,
        alternatives,
        alt_pct: vec![],
                pct: 0,
                words: 0,
        note: r.pending.then(|| "completed from the book mentioned earlier".to_string()),
    };
    if confidence >= floor {
        out.detections.push(det);
    } else {
        out.confirms.push(det);
    }
}

fn find_number_pos(toks: &[Tok], from: usize, max_fillers: usize) -> Option<usize> {
    let mut fillers = 0;
    let mut i = from;
    while i < toks.len() {
        match toks[i].kw {
            Kw::None => {
                if toks[i].num.is_some() {
                    return Some(i);
                }
                fillers += 1;
                if fillers > max_fillers {
                    return None;
                }
            }
            _ => return None,
        }
        i += 1;
    }
    None
}

fn next_number(toks: &[Tok], from: usize) -> Option<u16> {
    find_number_pos(toks, from, 4).map(|i| toks[i].num.unwrap_or(0))
}

// ---------------------------------------------------------------------------
// v0.853.0 helpers: self-correction and the pending-book completion
// ---------------------------------------------------------------------------

/// Spoken self-correction markers. Everything before the LAST one is a false
/// start: "second chronicles NO second corinthians chapter 5 verse 17".
/// Only what follows may stage a reference, so the correction wins.
const CORRECTION_MARKERS: &[&[&str]] = &[
    &["no"],
    &["nope"],
    &["sorry"],
    &["scratch", "that"],
    &["i", "mean"],
    &["correction"],
];

/// Where references should start scanning, if a genuine self-correction was
/// heard. A marker only counts when something reference-like follows within a
/// few tokens (a number — "second" IS the number 2 — or a book anchor), so a
/// plain "no" in running speech ("there is no condemnation") never splits the
/// line and "john 3 16 no more" keeps its reference.
fn find_correction_split(toks: &[Tok]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for m in CORRECTION_MARKERS {
        if m.len() >= toks.len() {
            continue;
        }
        for i in 0..=(toks.len() - m.len() - 1) {
            if !toks[i..i + m.len()].iter().zip(m.iter()).all(|(t, w)| t.text == **w) {
                continue;
            }
            let after = i + m.len();
            let window_end = (after + 3).min(toks.len());
            let corroborated = toks[after..window_end].iter().any(|t| t.num.is_some())
                || match_book_at(toks, after).is_some();
            if corroborated {
                best = Some(after); // the LAST marker wins
            }
        }
    }
    best
}

/// Does a real chapter cue sit at `i`? Required before the pending book may
/// claim a bare number. "23 verse 1", "23 1" and "chapter 23" all cue; a lone
/// number in running speech ("that's 3 points", "in 2019 we") does not.
fn pending_cue_ahead(toks: &[Tok], i: usize) -> bool {
    if toks[i].kw == Kw::Chapter {
        return true;
    }
    if toks[i].num.is_none() {
        return false;
    }
    let mut saw_num = false;
    for t in &toks[i..] {
        if t.kw == Kw::Verse || t.kw == Kw::Chapter || t.kw == Kw::Colon {
            return true;
        }
        if t.num.is_some() {
            if saw_num {
                return true; // two numbers: "23 1"
            }
            saw_num = true;
        }
    }
    false
}

/// Digit-split (PRD 6.1): ASR merges "three sixteen" into "316" or "1517".
/// Try first digits as chapter (<= 150), remaining digits as verse (<= 176).
fn try_split(n: u16) -> Option<(u16, u16)> {
    if n < 100 {
        return None;
    }
    let s = n.to_string();
    for k in 1..s.len() {
        let (Ok(a), Ok(b)) = (s[..k].parse::<u16>(), s[k..].parse::<u16>()) else {
            continue;
        };
        if a > 0 && a <= 150 && b > 0 && b <= 176 {
            return Some((a, b));
        }
    }
    None
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
