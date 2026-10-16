//! Voice-command recognition (v0.801.2).
//!
//! Why this module exists: v0.801.1 required multi-word phrases ("next
//! verse") — a preacher saying just "next", "previous" or "read it" was
//! ignored, which was the single most-reported demo failure. This matcher
//! is two-tiered:
//!
//! 1. **Phrase tier** — robust multi-word patterns anywhere in the
//!    utterance ("next verse", "zoom in", "read it aloud", ...).
//! 2. **Bare tier** — a *short* utterance (exactly one content word after
//!    filler removal, no numbers, no book names) may be a single command:
//!    "next", "previous", "again", "read", "clear", "bigger", "smaller",
//!    "fullscreen". Fuzzy matching tolerates the classic whisper mishears
//!    ("text" for "next").
//!
//! Safety rails (why it is good): the bare tier refuses to fire when the
//! utterance looks like Scripture (any word fuzzy-matches a book name) or
//! contains a number — so "next is John chapter 3" stages Scripture instead
//! of blindly advancing, and bare "stop" intentionally does nothing (voice
//! must never be able to stop recording or listening; PRD §6 safety rule).

use crate::books::{similarity, BOOKS};
use crate::parser::{Command, ParseOutcome};

/// Words ignored when deciding whether an utterance is "bare".
const COMMAND_FILLER: &[&str] = &[
    "please", "now", "the", "a", "an", "to", "go", "let", "us", "me",
    "can", "we", "could", "would", "you", "ok", "okay", "and", "then",
];

/// Number words — their presence vetoes the bare tier (a bare "three" must
/// never be read as a command).
const NUMBER_WORDS: &[&str] = &[
    "zero", "one", "won", "two", "too", "three", "tree", "four", "for",
    "five", "six", "sex", "seven", "eight", "ate", "nine", "ten", "eleven",
    "twelve", "twenty", "thirty", "forty", "fifty", "sixty", "seventy",
    "eighty", "ninety", "hundred", "first", "second", "third", "fourth",
    "fifth", "once", "twice",
];

pub fn extract_commands(text: &str, out: &mut ParseOutcome) {
    let words: Vec<&str> = text.split_whitespace().collect();

    // ---- explicit translation request (existing behaviour) ----
    // (v0.816.0: also "K J V", "k.j.v.", "King James Version", "Basic English", ...)
    let tokens0 = crate::cmdtable::tokenize(text);
    if let Some(tr) = crate::cmdtable::translation_in(&tokens0, tokens0.len() <= crate::cmdtable::SHORT_MAX_WORDS) {
        out.commands.push(Command::Translation(tr));
    }

    // ---- phrase tier: the data table in cmdtable.rs ------------------------
    // (longest phrase first, words consumed so overlapping commands cannot
    // both fire; "short" entries only fire in short utterances)
    let tokens = crate::cmdtable::tokenize(text);
    for c in crate::cmdtable::match_table(&tokens) {
        out.commands.push(c);
    }

    // ---- bare tier: a single spoken word is a command ---------------------
    // Example: "next." or "previous!" — the demo-day utterances.
    if out.commands.is_empty() {
        let core: Vec<&str> = words
            .iter()
            .map(|w| clean(w))
            .filter(|w| !w.is_empty() && !COMMAND_FILLER.contains(w))
            .collect();
        if core.len() == 1 {
            let w = &core[0];
            let numeric = w.parse::<u16>().is_ok() || NUMBER_WORDS.contains(&w);
            if !numeric && !looks_like_scripture(&words) {
                let cmd = bare_command(w);
                if let Some(c) = cmd {
                    out.commands.push(c);
                }
            }
        }
    }

    // dedupe (e.g. "zoom in ... zoom in" said twice in one breath)
    let mut seen: Vec<Command> = Vec::new();
    out.commands.retain(|c| {
        // Translation carries a payload; compare by discriminant + payload
        let key = c.clone();
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
}

/// True when the whole utterance is just "stop" (any case, with filler such
/// as "please"/"now"). On its own "stop" never controls recording or listening
/// (operator-only), but while a verse is being read aloud the app treats it as
/// "stop reading".
pub fn is_bare_stop(text: &str) -> bool {
    let core: Vec<String> = crate::cmdtable::tokenize(text)
        .into_iter()
        .filter(|w| !COMMAND_FILLER.contains(&w.as_str()))
        .collect();
    core.len() == 1 && (core[0] == "stop" || core[0] == "cancel" || core[0] == "pause")
}

/// Bare single-word commands. Fuzzy thresholds are tuned per word: short
/// words need tighter matches, and known mishears are whitelisted.
fn bare_command(w: &str) -> Option<Command> {
    // "next" — accept the classic "text"/"nex" mishears
    if w == "next" || similarity(w, "next") >= 0.7 {
        return Some(Command::NextVerse);
    }
    if w == "previous" || w == "prior" || w == "back" || w == "reverse" || w == "last"
        || similarity(w, "previous") >= 0.72
    {
        return Some(Command::PrevVerse);
    }
    if w == "again" || w == "repeat" || similarity(w, "again") >= 0.72 {
        return Some(Command::LastScripture);
    }
    if w == "read" || similarity(w, "read") >= 0.75 {
        return Some(Command::ReadAloud);
    }
    if w == "clear" || similarity(w, "clear") >= 0.75 {
        return Some(Command::ClearStage);
    }
    if w == "bigger" || w == "larger" || w == "zoom" {
        return Some(Command::ZoomIn);
    }
    if w == "smaller" {
        return Some(Command::ZoomOut);
    }
    if w == "fullscreen" {
        return Some(Command::Fullscreen);
    }
    if let Some(c) = crate::cmdtable::bare_word_command(w) {
        return Some(c);
    }
    // deliberate omissions: "stop", "record", "start" — voice must never be
    // able to control the service lifecycle (operator-only, PRD §6).
    None
}

/// True when any word in the utterance fuzzy-matches a Bible book name —
/// the bare tier must stay silent so Scripture wins.
fn looks_like_scripture(words: &[&str]) -> bool {
    for w in words {
        let w = clean(w);
        if w.len() < 3 {
            continue;
        }
        for b in BOOKS.iter() {
            for alias in b.aliases.iter() {
                // single-word aliases only (multi-word aliases cannot match
                // one token) — but split aliases on spaces for robustness
                for part in alias.split_whitespace() {
                    if part.len() >= 4 && similarity(&w, part) >= 0.86 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn clean(w: &str) -> &str {
    w.trim_matches(|c: char| !c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use rf_bible::Bible;

    fn cmds(text: &str) -> Vec<Command> {
        let mut out = ParseOutcome::default();
        extract_commands(&crate::parser::normalize_for_commands(text), &mut out);
        out.commands
    }

    fn parse_all(text: &str) -> crate::parser::ParseOutcome {
        let ctx = crate::parser::ParseContext::default();
        parse(Bible::embedded(), text, &ctx, 0.75)
    }

    #[test]
    fn bare_next_works() {
        assert!(cmds("next.").contains(&Command::NextVerse));
        assert!(cmds("next").contains(&Command::NextVerse));
        assert!(cmds("next please").contains(&Command::NextVerse));
    }

    #[test]
    fn bare_previous_and_back_work() {
        assert!(cmds("previous").contains(&Command::PrevVerse));
        assert!(cmds("go back").contains(&Command::PrevVerse));
        assert!(cmds("last.").contains(&Command::PrevVerse));
    }

    #[test]
    fn next_verse_phrase_still_works() {
        assert!(cmds("next verse please").contains(&Command::NextVerse));
        assert!(cmds("previous verse").contains(&Command::PrevVerse));
        assert!(cmds("next chapter").contains(&Command::NextChapter));
    }

    #[test]
    fn read_aloud_phrases() {
        assert!(cmds("read it.").contains(&Command::ReadAloud));
        assert!(cmds("rhemaflow read it").contains(&Command::ReadAloud));
        assert!(cmds("read that again").contains(&Command::ReadAloud));
        assert!(cmds("stop reading").contains(&Command::StopReading));
    }

    #[test]
    fn again_and_last_scripture() {
        assert!(cmds("again").contains(&Command::LastScripture));
        assert!(cmds("say that again").contains(&Command::LastScripture));
        assert!(cmds("last scripture").contains(&Command::LastScripture));
    }

    #[test]
    fn zoom_and_fullscreen() {
        assert!(cmds("zoom in").contains(&Command::ZoomIn));
        assert!(cmds("zoom out").contains(&Command::ZoomOut));
        assert!(cmds("bigger").contains(&Command::ZoomIn));
        assert!(cmds("smaller.").contains(&Command::ZoomOut));
        assert!(cmds("full screen").contains(&Command::Fullscreen));
        assert!(cmds("exit fullscreen").contains(&Command::ExitFullscreen));
    }

    #[test]
    fn asr_mishear_text_means_next() {
        assert!(cmds("text").contains(&Command::NextVerse));
    }

    #[test]
    fn clear_stage_survives_a_whisper_insertion() {
        // Regression: real Whisper output for a spoken "Clear the screen."
        // came back as "Clear in the screen." — an inserted "in" broke the
        // old exact-substring match ("clear the") entirely.
        assert!(cmds("clear in the screen.").contains(&Command::ClearStage), "{:?}", cmds("clear in the screen."));
        assert!(cmds("clear the screen").contains(&Command::ClearStage));
        assert!(cmds("clear the stage").contains(&Command::ClearStage));
        assert!(cmds("close the screen please").contains(&Command::ClearStage));
    }

    #[test]
    fn clear_stage_and_exit_fullscreen_do_not_collide() {
        // "close full screen" must mean exit-fullscreen, not clear-stage,
        // even though "close" + "screen" both appear within 3 words.
        let c = cmds("close full screen");
        assert!(c.contains(&Command::ExitFullscreen), "{c:?}");
        assert!(!c.contains(&Command::ClearStage), "{c:?}");
    }

    #[test]
    fn deictic_phrases_resolve_to_last_scripture() {
        // PRD 6.3: "that verse", "the same passage" resolve against stage
        // history, the same as "again" already does.
        assert!(cmds("let's look at that verse again").contains(&Command::LastScripture));
        assert!(cmds("turn to that verse").contains(&Command::LastScripture), "{:?}", cmds("turn to that verse"));
        assert!(cmds("show me the same passage").contains(&Command::LastScripture));
        assert!(cmds("the same verse please").contains(&Command::LastScripture));
    }

    #[test]
    fn go_back_to_that_verse_means_last_scripture_not_previous_verse() {
        // "go back" alone means PrevVerse, but "go back to THAT verse" means
        // return to what was on stage before — a different, more specific
        // instruction. Must fire exactly one of the two, not both.
        let c = cmds("let's go back to that verse");
        assert!(c.contains(&Command::LastScripture), "{c:?}");
        assert!(!c.contains(&Command::PrevVerse), "{c:?}");

        // plain "go back" (no "that"/"same"/"this") must still mean PrevVerse
        let c2 = cmds("go back please");
        assert!(c2.contains(&Command::PrevVerse), "{c2:?}");
        assert!(!c2.contains(&Command::LastScripture), "{c2:?}");
    }

    #[test]
    fn zoom_and_last_scripture_survive_small_insertions() {
        assert!(cmds("please zoom right in now").contains(&Command::ZoomIn), "{:?}", cmds("please zoom right in now"));
        assert!(cmds("can you zoom back out").contains(&Command::ZoomOut));
        assert!(cmds("read that one aloud").contains(&Command::ReadAloud));
        assert!(cmds("please show it again").contains(&Command::LastScripture), "{:?}", cmds("please show it again"));
    }

    #[test]
    fn scripture_utterances_do_not_fire_commands() {
        let o = parse_all("let's go to john three sixteen");
        assert!(o.commands.is_empty(), "{:?}", o.commands);
        assert_eq!(o.detections.len(), 1);
        let o = parse_all("next is john chapter 3");
        assert!(
            !o.commands.contains(&Command::NextVerse),
            "bare tier must not fire when a book name is present: {:?}",
            o.commands
        );
    }

    #[test]
    fn numbers_veto_bare_tier() {
        assert!(cmds("three").is_empty());
        assert!(cmds("16").is_empty());
    }

    #[test]
    fn stop_is_deliberately_not_a_command() {
        assert!(cmds("stop").is_empty());
        assert!(cmds("record").is_empty());
    }

    #[test]
    fn no_duplicate_commands() {
        let c = cmds("next verse, next verse, next verse");
        assert_eq!(c.iter().filter(|x| **x == Command::NextVerse).count(), 1);
    }
}
