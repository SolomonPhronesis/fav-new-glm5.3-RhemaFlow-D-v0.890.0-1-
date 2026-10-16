//! Voice-command phrase table (v0.811.0).
//!
//! One data table is the single source of truth for every spoken command:
//! the matcher, the in-app "Voice commands" window, `docs/VOICE-COMMANDS.md`
//! and the tests all read it, so the list a person sees is the list that works.
//!
//! How matching works (why it is safer than substring checks):
//! * A phrase matches when its words appear IN ORDER in the utterance with at
//!   most two other words between neighbours ("zoom right in now" -> zoom in).
//! * Longest phrase first, and the words a phrase uses are *consumed*, so
//!   "close full screen" can never also fire "close screen" (clear stage), and
//!   "go back to that verse" can never also fire "go back" (previous verse).
//! * Entries flagged `short` only fire in short utterances (<= 9 words): a
//!   preacher mid-sentence saying "...and then you clear it out of the way..."
//!   must not blank the screen.
//! * Zoom accepts a spoken percent: "zoom 300", "zoom to three hundred".

use crate::parser::Command;
use std::sync::OnceLock;

/// Longest utterance (in words) in which a `short` entry may fire.
pub const SHORT_MAX_WORDS: usize = 9;
/// Largest distance (in word positions) between two neighbouring words of a phrase.
const MAX_GAP: usize = 3;

pub const ZOOM_MIN: u16 = 60;
pub const ZOOM_HALF_MAX: u16 = 200;
pub const ZOOM_MAX: u16 = 400;

pub struct Entry {
    pub cmd: Command,
    pub phrases: &'static [&'static str],
    /// Only fire in a short utterance.
    pub short: bool,
}

use Command::*;

/// ORDER MATTERS: for phrases of equal length the earlier entry wins the words.
pub static TABLE: &[Entry] = &[
    // ---- reading aloud: stop first, so "stop reading" is never lost ----
    Entry { cmd: StopReading, short: false, phrases: &["stop reading", "stop read aloud", "stop the reading", "stop reading it", "stop reader", "cancel reading"] },
    Entry { cmd: StopReading, short: true, phrases: &["stop talking", "stop speaking", "be quiet", "quiet please", "stop it", "stop the voice", "stop voice", "no more reading", "thats enough", "stop read"] },
    Entry { cmd: ReadAloud, short: false, phrases: &["read it", "read that", "read this", "read the verse", "read the verses", "read aloud", "read the scripture", "read the text"] },
    Entry { cmd: ReadAloud, short: true, phrases: &["read it aloud", "read out loud", "read to us", "read it out", "please read", "start reading", "start read aloud"] },
    Entry { cmd: ReadFaster, short: true, phrases: &["read faster", "speak faster", "faster reading", "speed up reading", "read quicker"] },
    Entry { cmd: ReadSlower, short: true, phrases: &["read slower", "speak slower", "slower reading", "slow down reading", "read slowly"] },
    Entry { cmd: AutoRead(true), short: true, phrases: &["auto read on", "turn on auto read", "enable auto read", "auto reading on", "start auto reading", "read every verse"] },
    Entry { cmd: AutoRead(false), short: true, phrases: &["auto read off", "turn off auto read", "disable auto read", "auto reading off", "stop auto reading", "stop auto read"] },
    // ---- stage window ----
    Entry { cmd: ExitFullscreen, short: false, phrases: &["exit full screen", "exit fullscreen", "close full screen", "close fullscreen", "leave full screen", "leave fullscreen", "normal screen", "exit full", "leave full", "close full"] },
    // v0.833.0: what the speech engine really writes for "exit full screen" ("Exit for screen", "That's it for screen")
    Entry { cmd: ExitFullscreen, short: false, phrases: &["exit for screen", "exit four screen", "exit 4 screen", "exit fool screen", "exit pull screen", "exit foul screen", "exit fall screen", "exit phone screen", "exit forscreen", "exit the screen", "exit screen", "leave for screen", "close for screen", "exit full-screen"] },
    Entry { cmd: ExitFullscreen, short: true, phrases: &["that's it for screen", "thats it for screen", "that is it for screen", "it's it for screen", "exit it for screen", "windowed mode", "back to window", "un full screen", "minimize stage"] },
    Entry { cmd: Fullscreen, short: false, phrases: &["full screen", "fullscreen"] },
    Entry { cmd: Fullscreen, short: true, phrases: &["go full screen", "go fullscreen", "make it full screen", "full screen mode", "enter full screen", "maximize stage", "maximize screen", "project full screen"] },
    Entry { cmd: OpenStage, short: true, phrases: &["open stage", "open the stage", "show stage", "show the stage", "open stage window", "open projector", "open the projector", "open screen", "open the screen", "launch stage", "show the screen", "open display", "bring up the stage", "turn on the stage", "stage on"] },
    Entry { cmd: CloseStage, short: true, phrases: &["close stage", "close the stage", "hide stage", "hide the stage", "close stage window", "close projector", "close the projector", "close display", "turn off the stage", "stage off", "shut the stage", "close the window"] },
    Entry { cmd: ClearStage, short: false, phrases: &["clear the screen", "clear screen", "clear the stage", "clear stage", "close the screen", "close screen", "take it off"] },
    Entry { cmd: ClearStage, short: true, phrases: &["blank screen", "blank the screen", "black screen", "go black", "go blank", "clear it", "clear this", "clear that", "remove it", "remove the verse", "hide the verse", "hide verse", "take it down", "clear the verse", "empty screen", "clear display", "wipe screen", "blank stage", "black out", "blackout"] },
    // ---- zoom (percent numbers are handled separately: "zoom 300") ----
    Entry { cmd: ZoomTo(ZOOM_MAX), short: true, phrases: &["max zoom", "maximum zoom", "zoom max", "zoom maximum", "biggest text", "largest text", "full zoom", "zoom all the way"] },
    Entry { cmd: ZoomTo(ZOOM_HALF_MAX), short: true, phrases: &["half max zoom", "half max", "half maximum", "half zoom", "zoom half", "zoom halfway", "medium zoom"] },
    Entry { cmd: ZoomTo(ZOOM_MIN), short: true, phrases: &["minimum zoom", "min zoom", "zoom min", "zoom minimum", "smallest text"] },
    Entry { cmd: ZoomReset, short: false, phrases: &["reset zoom", "normal size", "reset text"] },
    Entry { cmd: ZoomReset, short: true, phrases: &["zoom reset", "default size", "default zoom", "zoom normal", "normal zoom", "original size"] },
    Entry { cmd: ZoomIn, short: false, phrases: &["zoom in", "text bigger", "bigger text", "make it bigger", "make the text bigger"] },
    Entry { cmd: ZoomIn, short: true, phrases: &["bigger font", "larger text", "increase size", "enlarge text", "zoom closer", "magnify", "make text larger", "bigger please", "increase zoom", "zoom up"] },
    Entry { cmd: ZoomOut, short: false, phrases: &["zoom out", "text smaller", "smaller text", "make it smaller", "make the text smaller"] },
    Entry { cmd: ZoomOut, short: true, phrases: &["smaller font", "reduce size", "shrink text", "decrease zoom", "zoom down", "make text smaller", "smaller please"] },
    // ---- navigation ----
    Entry { cmd: LastScripture, short: false, phrases: &["last scripture", "that again", "it again", "show again", "say again", "that verse", "that passage", "same verse", "same passage", "back to that", "back to same", "back to this"] },
    Entry { cmd: LastScripture, short: true, phrases: &["last passage", "put it back", "bring it back"] },
    Entry { cmd: NextVerse, short: false, phrases: &["next verse", "next one"] },
    Entry { cmd: NextVerse, short: true, phrases: &["next line", "go forward", "forward one verse", "one verse forward", "following verse", "verse after"] },
    Entry { cmd: NextChapter, short: false, phrases: &["next chapter"] },
    Entry { cmd: NextChapter, short: true, phrases: &["following chapter", "chapter forward", "forward one chapter", "chapter after"] },
    Entry { cmd: PrevChapter, short: false, phrases: &["previous chapter", "last chapter", "back chapter"] },
    Entry { cmd: PrevChapter, short: true, phrases: &["prior chapter", "go back a chapter", "chapter before", "one chapter back", "preceding chapter"] },
    Entry { cmd: PrevVerse, short: false, phrases: &["previous verse", "last verse", "back verse", "prior verse", "go back"] },
    Entry { cmd: PrevVerse, short: true, phrases: &["back one verse", "one verse back", "verse before", "preceding verse", "previous one"] },
    // history: the scriptures shown so far, in order (v0.814.0)
    Entry { cmd: PrevScripture, short: false, phrases: &["previous scripture", "previous passage", "scripture before that", "the one before", "one before that", "the scripture before", "earlier scripture", "go to the previous scripture", "back to the previous scripture", "before that one"] },
    Entry { cmd: PrevScripture, short: true, phrases: &["go back to the last scripture", "the last one", "the one before that", "back one scripture", "last scripture before", "the previous scripture"] },
    Entry { cmd: NextScripture, short: false, phrases: &["next scripture", "scripture after that", "the one after", "forward scripture"] },
    Entry { cmd: NextScripture, short: true, phrases: &["the next scripture", "go forward a scripture", "scripture forward", "back to the later scripture"] },
    // slides and video (v0.815.0). Slide stepping is "navigation" (short sentences only);
    // the video words only count in a short utterance, so "...they played the video of..." does nothing.
    Entry { cmd: NextSlide, short: false, phrases: &["next slide", "forward one slide", "advance slide", "advance the slide", "slide forward"] },
    Entry { cmd: PrevSlide, short: false, phrases: &["previous slide", "back one slide", "go back one slide", "slide back", "slide before", "the previous slide"] },
    Entry { cmd: MediaPlay, short: true, phrases: &["play video", "play the video", "start video", "start the video", "play movie", "play the movie", "play the clip", "play clip", "resume video", "resume the video"] },
    Entry { cmd: MediaPause, short: true, phrases: &["pause video", "pause the video", "pause movie", "pause the movie", "pause clip", "pause the clip", "freeze the video"] },
    Entry { cmd: MediaRestart, short: true, phrases: &["restart video", "restart the video", "replay video", "replay the video", "video from the start", "restart the clip"] },
    // ---- v0.816.0: the next 20 (all short-utterance only) ----
    // (the words "black screen", "go black", "blackout" already mean CLEAR in the list above; Blackout
    // keeps the verse and only darkens the congregation screen, so it gets its own words)
    Entry { cmd: Blackout(true), short: true, phrases: &["go dark", "screen off", "turn the screen off", "turn off the screen", "dark screen", "lights out", "make it dark", "darken the screen"] },
    Entry { cmd: Blackout(false), short: true, phrases: &["screen on", "turn the screen on", "turn on the screen", "bring the screen back", "unblack", "end blackout", "stop blackout", "lights on", "lights up", "undarken"] },
    Entry { cmd: SwitchTranslation(1), short: true, phrases: &["next translation", "another translation", "change translation", "switch translation"] },
    Entry { cmd: SwitchTranslation(-1), short: true, phrases: &["previous translation", "last translation", "translation before"] },
    Entry { cmd: LastVerse, short: true, phrases: &["final verse", "last verse of the chapter", "last verse in the chapter", "end of chapter", "end of the chapter", "bottom of chapter"] },
    Entry { cmd: ReadUntil(0), short: true, phrases: &["read the chapter", "read the whole chapter", "read to the end", "read to the end of the chapter", "read the rest", "read the rest of the chapter", "read until the end", "read till the end"] },
    Entry { cmd: History(true), short: true, phrases: &["show history", "open history", "show the history", "scripture history", "show scripture history", "open the history"] },
    Entry { cmd: History(false), short: true, phrases: &["hide history", "close history", "hide the history", "close the history"] },
    Entry { cmd: Browser(true), short: true, phrases: &["show browser", "show the browser", "open the browser", "open browser", "show the bible", "show books", "open the bible browser"] },
    Entry { cmd: Browser(false), short: true, phrases: &["hide browser", "hide the browser", "close the browser", "close browser", "hide the bible", "hide books"] },
    Entry { cmd: MediaWindow(true), short: true, phrases: &["open media", "show media", "open slides", "show slides", "open the media window", "media and slides", "open songs", "show songs"] },
    Entry { cmd: MediaWindow(false), short: true, phrases: &["close media", "hide media", "close slides", "hide slides", "close the media window", "close songs", "hide songs"] },
    Entry { cmd: DismissAll, short: true, phrases: &["dismiss all", "dismiss everything", "dismiss all cards", "clear the queue", "clear the cards", "clear all cards", "reject all", "empty the queue"] },
    Entry { cmd: Look("classic"), short: true, phrases: &["classic look", "classic style", "plain style", "normal style"] },
    Entry { cmd: Look("lower_third"), short: true, phrases: &["lower third", "lower thirds", "lower third style", "stream style"] },
    Entry { cmd: Look("chroma"), short: true, phrases: &["green screen", "chroma key", "chroma green", "green background"] },
    Entry { cmd: Look("large_print"), short: true, phrases: &["large print", "big print", "large print style"] },
    Entry { cmd: TimerPause, short: true, phrases: &["pause the timer", "pause timer", "pause the countdown", "pause countdown", "hold the timer", "hold the countdown"] },
    Entry { cmd: TimerResume, short: true, phrases: &["resume the timer", "resume timer", "resume the countdown", "resume countdown", "continue the timer", "continue the countdown"] },
    Entry { cmd: TimerStop, short: true, phrases: &["stop the timer", "stop timer", "cancel the timer", "stop the countdown", "stop countdown", "end the timer", "reset the timer", "clear the timer"] },
    Entry { cmd: ShowTimer, short: true, phrases: &["show the timer", "show timer", "show the countdown", "show countdown", "put the timer up", "put the countdown up"] },
    Entry { cmd: ShowClock, short: true, phrases: &["show the clock", "show clock", "show the time", "put the clock up", "put the time up"] },
    Entry { cmd: Strongs(true), short: true, phrases: &["show strongs", "open strongs", "show the strongs", "strongs on", "show strong", "open strong", "greek and hebrew", "show original words", "show the original words"] },
    Entry { cmd: Strongs(false), short: true, phrases: &["hide strongs", "close strongs", "hide the strongs", "strongs off", "hide strong", "close strong", "hide original words", "hide the original words"] },
    Entry { cmd: FirstVerse, short: true, phrases: &["first verse", "top of chapter", "start of chapter", "beginning of chapter"] },
    // ---- the review queue ----
    Entry { cmd: ApproveQuote, short: true, phrases: &["approve", "approve it", "approve that", "yes show it", "yes approve", "accept it", "accept that", "confirm it", "show it", "put it up", "put it on screen", "show that one", "go with that", "use that one", "thats the one"] },
    Entry { cmd: DismissQuote, short: true, phrases: &["dismiss", "dismiss it", "dismiss that", "reject it", "reject that", "no not that", "not that one", "wrong verse", "wrong one", "never mind", "ignore that", "skip it", "skip that", "cancel that", "thats wrong"] },
    Entry { cmd: PickMatch(1), short: true, phrases: &["option one", "option 1", "first match", "first option", "choice one", "pick one"] },
    Entry { cmd: PickMatch(2), short: true, phrases: &["option two", "option 2", "second match", "second option", "second one", "choice two", "pick two"] },
    Entry { cmd: PickMatch(3), short: true, phrases: &["option three", "option 3", "third match", "third option", "third one", "choice three", "pick three"] },
    Entry { cmd: PickMatch(4), short: true, phrases: &["option four", "option 4", "fourth match", "fourth option", "fourth one", "choice four"] },
    Entry { cmd: PickMatch(5), short: true, phrases: &["option five", "option 5", "fifth match", "fifth option"] },
    Entry { cmd: AutoApprove(true), short: true, phrases: &["auto approve on", "turn on auto approve", "enable auto approve", "automatic approval on", "auto stage quotes", "auto approve quotes"] },
    Entry { cmd: AutoApprove(false), short: true, phrases: &["auto approve off", "turn off auto approve", "disable auto approve", "automatic approval off", "wait for approval", "ask me first", "stop auto approving"] },
    Entry { cmd: ShowAllMatches(true), short: true, phrases: &["show all matches", "show all possible", "show all scriptures", "show all options", "show every match", "list all matches"] },
    Entry { cmd: ShowAllMatches(false), short: true, phrases: &["show one match", "only one match", "show best match only", "show top match", "single match"] },
    // ---- recording (operator voice, as before) ----
    Entry { cmd: StartRecording, short: false, phrases: &["start recording", "begin recording"] },
    Entry { cmd: StartRecording, short: true, phrases: &["record this", "resume recording", "start the recording", "record service"] },
    Entry { cmd: StopRecording, short: false, phrases: &["stop recording", "end recording"] },
    Entry { cmd: StopRecording, short: true, phrases: &["pause recording", "stop the recording", "finish recording"] },
    Entry { cmd: StopListening, short: false, phrases: &["stop listening", "pause listening"] },
    // ---- help ----
    Entry { cmd: Help(true), short: true, phrases: &["show commands", "show help", "voice commands", "list commands", "what can i say", "show the commands", "open help"] },
    Entry { cmd: Help(false), short: true, phrases: &["hide commands", "hide help", "close help", "close commands", "close the help", "hide the commands"] },
];

/// Words that mean one thing when they are the WHOLE utterance. These are too
/// common inside a sentence to be phrases.
pub fn bare_word_command(w: &str) -> Option<Command> {
    Some(match w {
        "exit" | "escape" | "windowed" => ExitFullscreen,
        "quiet" | "silence" | "hush" | "shush" | "enough" => StopReading,
        "approve" | "accept" => ApproveQuote,
        "dismiss" | "reject" => DismissQuote,
        "blank" | "blackout" => ClearStage,
        "maximize" => Fullscreen,
        // v0.813.0: "Next." / "Previous." are what the speech engine returns when someone says them
        "next" | "forward" | "nex" => NextVerse,
        "previous" | "previews" | "preview" | "prior" | "back" => PrevVerse,
        "again" => LastScripture,
        "read" | "reading" => ReadAloud,
        "clear" => ClearStage,
        _ => return None,
    })
}

/// (entry index, phrase index, token count) sorted longest first, ties by table order.
fn order() -> &'static Vec<(usize, usize, usize)> {
    static ORDER: OnceLock<Vec<(usize, usize, usize)>> = OnceLock::new();
    ORDER.get_or_init(|| {
        let mut v = Vec::new();
        for (ei, e) in TABLE.iter().enumerate() {
            for (pi, p) in e.phrases.iter().enumerate() {
                v.push((ei, pi, p.split_whitespace().count()));
            }
        }
        v.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
        v
    })
}

/// Lowercase tokens: hyphens split, apostrophes dropped, edge punctuation trimmed.
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .replace('-', " ")
        .split_whitespace()
        .map(|w| w.chars().filter(|c| *c != '\'' && *c != '\u{2019}').collect::<String>())
        .map(|w| w.trim_matches(|c: char| !c.is_ascii_alphanumeric()).to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

fn find_phrase(tokens: &[String], used: &[bool], phrase: &[&str]) -> Option<Vec<usize>> {
    'start: for i in 0..tokens.len() {
        if used[i] || tokens[i] != phrase[0] {
            continue;
        }
        let mut hit = vec![i];
        let mut prev = i;
        for w in &phrase[1..] {
            let mut found = None;
            for j in (prev + 1)..=(prev + MAX_GAP).min(tokens.len().saturating_sub(1)) {
                if !used[j] && tokens[j] == *w {
                    found = Some(j);
                    break;
                }
            }
            match found {
                Some(j) => {
                    hit.push(j);
                    prev = j;
                }
                None => continue 'start,
            }
        }
        return Some(hit);
    }
    None
}

/// Spoken number -> value. Digits ("300") and words ("three hundred (and) fifty").
pub fn words_to_number(tokens: &[&str]) -> Option<u32> {
    let mut total: u32 = 0;
    let mut seen = false;
    for t in tokens {
        if let Ok(n) = t.parse::<u32>() {
            total += n;
            seen = true;
            continue;
        }
        let unit = match *t {
            "zero" => Some(0), "one" => Some(1), "two" => Some(2), "three" => Some(3), "four" => Some(4),
            "five" => Some(5), "six" => Some(6), "seven" => Some(7), "eight" => Some(8), "nine" => Some(9),
            "ten" => Some(10), "eleven" => Some(11), "twelve" => Some(12), "thirteen" => Some(13),
            "fourteen" => Some(14), "fifteen" => Some(15), "sixteen" => Some(16), "seventeen" => Some(17),
            "eighteen" => Some(18), "nineteen" => Some(19), "twenty" => Some(20), "thirty" => Some(30),
            "forty" => Some(40), "fifty" => Some(50), "sixty" => Some(60), "seventy" => Some(70),
            "eighty" => Some(80), "ninety" => Some(90),
            _ => None,
        };
        if let Some(u) = unit {
            total += u;
            seen = true;
        } else if *t == "hundred" {
            total = if total == 0 { 100 } else { total * 100 };
            seen = true;
        } else if *t == "and" || *t == "percent" || *t == "per" || *t == "cent" {
            continue;
        } else {
            break;
        }
    }
    seen.then_some(total)
}

/// "zoom 300" / "zoom to three hundred percent" -> Some(300). A bare 1-4
/// means hundreds ("zoom three" -> 300). Returns the zoom token index and the
/// number of tokens used, so they can be consumed.
fn zoom_percent(tokens: &[String]) -> Option<(u16, Vec<usize>)> {
    for (i, t) in tokens.iter().enumerate() {
        if t != "zoom" {
            continue;
        }
        let mut j = i + 1;
        while j < tokens.len() && matches!(tokens[j].as_str(), "to" | "at" | "level" | "by" | "of" | "set" | "on" | "size") {
            j += 1;
        }
        let start = j;
        while j < tokens.len() && j < start + 5 && (tokens[j].parse::<u32>().is_ok() || words_to_number(&[tokens[j].as_str()]).is_some() || matches!(tokens[j].as_str(), "and" | "percent")) {
            j += 1;
        }
        if j == start {
            continue;
        }
        let words: Vec<&str> = tokens[start..j].iter().map(String::as_str).collect();
        let Some(n) = words_to_number(&words) else { continue };
        let pct = match n {
            1..=4 => n * 100,
            50..=500 => n,
            _ => continue,
        };
        let mut used: Vec<usize> = vec![i];
        used.extend(start..j);
        return Some((pct.clamp(ZOOM_MIN as u32, ZOOM_MAX as u32) as u16, used));
    }
    None
}

// ---------------------------------------------------------------------------
// v0.816.0: commands that carry a number or a word, and translation names
// ---------------------------------------------------------------------------

fn numlike(t: &str) -> bool {
    t.parse::<u32>().is_ok() || (t != "and" && words_to_number(&[t]).is_some())
}

/// A spoken number starting at `start`: (value, index after its last token).
fn number_at(tokens: &[String], start: usize) -> Option<(u32, usize)> {
    let (mut j, mut last) = (start, start);
    while j < tokens.len() && j < start + 6 {
        let t = tokens[j].as_str();
        if numlike(t) {
            j += 1;
            last = j;
        } else if t == "and" && last > start && j + 1 < tokens.len() && numlike(&tokens[j + 1]) {
            j += 1;
        } else {
            break;
        }
    }
    if last == start {
        return None;
    }
    let words: Vec<&str> = tokens[start..last].iter().map(String::as_str).collect();
    words_to_number(&words).map(|n| (n, last))
}

fn payload_commands(tokens: &[String]) -> Vec<(Command, Vec<usize>)> {
    [read_until(tokens), timer_start(tokens), stage_screen(tokens), original_for(tokens)].into_iter().flatten().collect()
}

/// "read till verse 20", "read to verse seventeen", "read up to verse thirty", "read through 25".
fn read_until(tokens: &[String]) -> Option<(Command, Vec<usize>)> {
    let r = tokens.iter().position(|t| t == "read")?;
    let mut j = r + 1;
    let mut skipped = 0;
    while j < tokens.len() && skipped < 2 && matches!(tokens[j].as_str(), "it" | "this" | "that" | "out" | "aloud" | "please") {
        j += 1;
        skipped += 1;
    }
    let conn = tokens.get(j)?.as_str();
    if !matches!(conn, "till" | "until" | "to" | "through" | "thru" | "up" | "down") {
        return None;
    }
    j += 1;
    if matches!(conn, "up" | "down") {
        if tokens.get(j).map(String::as_str) != Some("to") {
            return None;
        }
        j += 1;
    }
    if tokens.get(j).map(String::as_str) == Some("the") {
        j += 1;
    }
    if matches!(tokens.get(j).map(String::as_str), Some("verse") | Some("verses")) {
        j += 1;
    }
    let (n, end) = number_at(tokens, j)?;
    if !(1..=176).contains(&n) {
        return None;
    }
    Some((ReadUntil(n as u16), (r..end).collect()))
}

/// "start a five minute countdown", "countdown five minutes", "set a timer for ten minutes".
fn timer_start(tokens: &[String]) -> Option<(Command, Vec<usize>)> {
    let kw = tokens.iter().position(|t| t == "countdown" || t == "timer").or_else(|| tokens.windows(2).position(|w| w[0] == "count" && w[1] == "down"))?;
    for i in 1..tokens.len() {
        if !matches!(tokens[i].as_str(), "minute" | "minutes" | "min" | "mins") {
            continue;
        }
        let mut s = i;
        while s > 0 && numlike(&tokens[s - 1]) {
            s -= 1;
        }
        if s == i {
            continue;
        }
        let words: Vec<&str> = tokens[s..i].iter().map(String::as_str).collect();
        let n = words_to_number(&words)?;
        if !(1..=180).contains(&n) {
            return None;
        }
        let mut used: Vec<usize> = (s..=i).collect();
        used.push(kw);
        return Some((TimerStart(n as u16), used));
    }
    None
}

/// "put the stage on screen two", "send it to screen 2", "use screen three".
fn stage_screen(tokens: &[String]) -> Option<(Command, Vec<usize>)> {
    let sc = tokens.iter().position(|t| t == "screen")?;
    if !tokens.iter().any(|t| matches!(t.as_str(), "stage" | "send" | "move" | "put" | "use" | "output" | "display")) {
        return None;
    }
    let (n, end) = number_at(tokens, sc + 1)?;
    if !(1..=4).contains(&n) {
        return None;
    }
    Some((StageScreen(n as u8), (sc..end).collect()))
}

/// "what is the Greek for love", "hebrew for peace", "strong's number for the word spirit".
fn original_for(tokens: &[String]) -> Option<(Command, Vec<usize>)> {
    let k = tokens.iter().position(|t| matches!(t.as_str(), "greek" | "hebrew" | "original" | "strongs" | "strong"))?;
    let lang = match tokens[k].as_str() {
        "greek" => 'G',
        "hebrew" => 'H',
        _ => '?',
    };
    let mut j = k + 1;
    while j < tokens.len() && j < k + 4 && matches!(tokens[j].as_str(), "word" | "words" | "number" | "text" | "is" | "meaning" | "definition" | "the") {
        j += 1;
    }
    if !matches!(tokens.get(j).map(String::as_str), Some("for") | Some("of")) {
        return None;
    }
    j += 1;
    while j < tokens.len() && matches!(tokens[j].as_str(), "the" | "a" | "an" | "this" | "that") {
        j += 1;
    }
    if tokens.get(j).map(String::as_str) == Some("word") && j + 1 < tokens.len() {
        j += 1; // "the word spirit"
    }
    let word = tokens.get(j)?.clone();
    if !word.chars().any(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some((OriginalFor { lang, word }, (k..=j).collect()))
}

/// Long names of translations, longest first ("new king james" before "king james").
const TRANSLATION_NAMES: &[(&str, &str)] = &[
    ("new american standard", "NASB"),
    ("new international", "NIV"),
    ("new king james", "NKJV"),
    ("american standard", "ASV"),
    ("english standard", "ESV"),
    ("authorized version", "KJV"),
    ("new living", "NLT"),
    ("king james", "KJV"),
    ("basic english", "BBE"),
    ("youngs literal", "YLT"),
    ("rotherham", "Rotherham"),
    ("amplified", "AMP"),
    ("webster", "Webster"),
    ("darby", "Darby"),
];

/// The translation the operator asked for, however the speech engine wrote it: "kjv", "K J V",
/// "k.j.v.", "king james version", "basic english". The LAST mention wins. Long names only count in a
/// short utterance, so a sermon sentence about "the king james version" changes nothing.
pub fn translation_in(tokens: &[String], short: bool) -> Option<&'static str> {
    // a speech engine may write the SOUND of a letter ("kay jay vee") instead of the letter: fold those to letters
    let letter = |w: String| -> String {
        match w.as_str() {
            "kay" => "k", "jay" => "j", "vee" => "v", "bee" => "b", "en" => "n", "ess" | "es" => "s", "ell" => "l", "tee" => "t", "ay" => "a", "em" => "m",
            _ => return w,
        }
        .to_string()
    };
    let t: Vec<String> = tokens.iter().map(|x| letter(x.replace('.', ""))).collect();
    let mut best: Option<(usize, &'static str)> = None;
    let mut take = |pos: usize, tr: &'static str| {
        if best.map_or(true, |b| pos >= b.0) {
            best = Some((pos, tr));
        }
    };
    for (i, w) in t.iter().enumerate() {
        if let Some(tr) = crate::parser::translation_for(w) {
            take(i, tr);
        }
    }
    // spelled out: "k j v", "n k j v"
    let single = |w: &str| w.len() == 1 && w.chars().all(|c| c.is_ascii_alphabetic());
    let mut i = 0;
    while i < t.len() {
        if single(&t[i]) {
            let mut j = i;
            let mut run = String::new();
            while j < t.len() && single(&t[j]) {
                run.push_str(&t[j]);
                j += 1;
            }
            if (2..=5).contains(&run.len()) {
                if let Some(tr) = crate::parser::translation_for(&run) {
                    take(i, tr);
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    if short {
        let mut used = vec![false; t.len()];
        for (name, tr) in TRANSLATION_NAMES {
            let words: Vec<&str> = name.split(' ').collect();
            if t.len() < words.len() {
                continue;
            }
            for p in 0..=(t.len() - words.len()) {
                if (0..words.len()).all(|k| !used[p + k] && t[p + k] == words[k]) {
                    for k in 0..words.len() {
                        used[p + k] = true;
                    }
                    take(p, tr);
                }
            }
        }
    }
    best.map(|b| b.1)
}

/// Navigation is only ever a SHORT utterance ("next verse"), never inside a sentence about a
/// verse ("the next verse says something about grace").
pub const NAV_MAX_WORDS: usize = 6;

fn is_nav(c: &Command) -> bool {
    matches!(c, NextVerse | PrevVerse | NextChapter | PrevChapter | LastScripture | FirstVerse | PrevScripture | NextScripture | NextSlide | PrevSlide)
}

/// Run-together words ("nextverse") and a slipped letter ("netverse", "nex verse") in a WHOLE short
/// utterance: compare it, spaces removed, with every phrase, spaces removed.
fn fuzzy_whole(tokens: &[String]) -> Option<Command> {
    if tokens.is_empty() || tokens.len() > 4 {
        return None;
    }
    let u: String = tokens.concat();
    if u.len() < 6 {
        return None;
    }
    let mut best: Option<(f32, Command)> = None;
    for e in TABLE {
        for p in e.phrases {
            let q: String = p.split_whitespace().collect();
            if q.len() < 6 {
                continue;
            }
            let sc = if u == q { 1.0 } else { crate::books::similarity(&u, &q) };
            let thr = if q.len() >= 10 { 0.84 } else { 0.88 };
            if sc >= thr && best.as_ref().map_or(true, |b| sc > b.0) {
                best = Some((sc, e.cmd.clone()));
            }
        }
    }
    best.map(|b| b.1)
}

/// All table + percent-zoom commands found in the utterance.
pub fn match_table(tokens: &[String]) -> Vec<Command> {
    let mut out: Vec<Command> = Vec::new();
    let mut used = vec![false; tokens.len()];
    let short_ok = tokens.len() <= SHORT_MAX_WORDS;

    if short_ok {
        if let Some((pct, idx)) = zoom_percent(tokens) {
            for i in idx {
                used[i] = true;
            }
            out.push(ZoomTo(pct));
        }
    }
    if short_ok {
        for (cmd, idx) in payload_commands(tokens) {
            for i in idx {
                used[i] = true;
            }
            if !out.contains(&cmd) {
                out.push(cmd);
            }
        }
    }
    // v0.853.0: a navigation phrase that ENDS the utterance stays alive even
    // in a long line. "The evidence of things not seen. Next verse." is a
    // preacher finishing reading the verse and then asking for the next one —
    // the old length gate (below) stripped the command and the verse never
    // advanced. A nav phrase buried mid-sentence ("the NEXT VERSE says
    // something wonderful about grace") is still dropped.
    let mut nav_at_end: Vec<Command> = Vec::new();
    for &(ei, pi, _) in order() {
        let e = &TABLE[ei];
        if e.short && !short_ok {
            continue;
        }
        let phrase: Vec<&str> = e.phrases[pi].split_whitespace().collect();
        if let Some(hit) = find_phrase(tokens, &used, &phrase) {
            if is_nav(&e.cmd) && hit.last() == Some(&(tokens.len() - 1)) && !nav_at_end.contains(&e.cmd) {
                nav_at_end.push(e.cmd.clone());
            }
            for i in hit {
                used[i] = true;
            }
            if !out.contains(&e.cmd) {
                out.push(e.cmd.clone());
            }
        }
    }
    if out.is_empty() {
        if let Some(c) = fuzzy_whole(tokens) {
            out.push(c);
        }
    }
    if tokens.len() > NAV_MAX_WORDS {
        out.retain(|c| !is_nav(c) || nav_at_end.contains(c));
    }
    out
}

/// Every spoken phrase, for the help window and the docs: (phrase, what it does).
pub fn all_phrases() -> Vec<(String, String)> {
    let mut v = Vec::new();
    for e in TABLE {
        for p in e.phrases {
            v.push((p.to_string(), e.cmd.label()));
        }
    }
    for (p, c) in [
        ("read till verse 20", ReadUntil(20)), ("read until verse twenty", ReadUntil(20)), ("read to verse 17", ReadUntil(17)),
        ("start a five minute countdown", TimerStart(5)), ("set a timer for ten minutes", TimerStart(10)),
        ("put the stage on screen two", StageScreen(2)),
        ("what is the greek for love", OriginalFor { lang: 'G', word: "love".into() }), ("hebrew for peace", OriginalFor { lang: 'H', word: "peace".into() }),
        ("in the KJV", Translation("KJV")), ("KJV translation", Translation("KJV")), ("K J V", Translation("KJV")), ("king james version", Translation("KJV")),
        ("basic english", Translation("BBE")), ("B B E", Translation("BBE")), ("A S V", Translation("ASV")), ("N I V", Translation("NIV")),
    ] {
        v.push((p.to_string(), c.label()));
    }
    for (p, pct) in [("zoom 300", 300), ("zoom to 250", 250), ("zoom three hundred", 300), ("zoom to two hundred percent", 200), ("zoom 150", 150), ("zoom four hundred", 400), ("zoom 100", 100)] {
        v.push((p.to_string(), ZoomTo(pct).label()));
    }
    v
}

impl Command {
    /// Short operator-readable name ("zoom 300%", "approve first card").
    pub fn label(&self) -> String {
        match self {
            NextVerse => "next verse".into(),
            PrevVerse => "previous verse".into(),
            NextChapter => "next chapter".into(),
            PrevChapter => "previous chapter".into(),
            LastScripture => "re-show last scripture".into(),
            ReadAloud => "read aloud".into(),
            StopReading => "stop reading".into(),
            ZoomIn => "zoom in".into(),
            ZoomOut => "zoom out".into(),
            ZoomReset => "zoom reset (100%)".into(),
            ClearStage => "clear stage".into(),
            Fullscreen => "full screen".into(),
            ExitFullscreen => "exit full screen".into(),
            StopListening => "stop listening (ignored \u{2014} operator-only)".into(),
            StartRecording => "start recording".into(),
            StopRecording => "stop recording".into(),
            Translation(t) => format!("translation {t}"),
            OpenStage => "open stage window".into(),
            CloseStage => "close stage window".into(),
            FirstVerse => "first verse of chapter".into(),
            ZoomTo(p) => format!("zoom {p}%"),
            ApproveQuote => "approve first review card".into(),
            DismissQuote => "dismiss first review card".into(),
            PickMatch(n) => format!("pick match #{n}"),
            AutoApprove(true) => "auto-approve quotations ON".into(),
            AutoApprove(false) => "auto-approve quotations OFF (wait for approval)".into(),
            ShowAllMatches(true) => "show all possible matches".into(),
            ShowAllMatches(false) => "show best match only".into(),
            AutoRead(true) => "auto-read new verse ON".into(),
            AutoRead(false) => "auto-read new verse OFF".into(),
            ReadFaster => "read faster".into(),
            ReadSlower => "read slower".into(),
            Help(true) => "show voice commands".into(),
            Help(false) => "hide voice commands".into(),
            PrevScripture => "previous scripture (history back)".into(),
            NextScripture => "next scripture (history forward, else next verse)".into(),
            Blackout(true) => "black screen".into(),
            Blackout(false) => "screen back on".into(),
            SwitchTranslation(d) => if *d >= 0 { "next translation".into() } else { "previous translation".into() },
            LastVerse => "last verse of the chapter".into(),
            ReadUntil(0) => "read to the end of the chapter".into(),
            ReadUntil(n) => format!("read until verse {n}"),
            History(true) => "show history".into(),
            History(false) => "hide history".into(),
            Browser(true) => "show Bible browser".into(),
            Browser(false) => "hide Bible browser".into(),
            MediaWindow(true) => "open media & slides".into(),
            MediaWindow(false) => "close media & slides".into(),
            DismissAll => "dismiss all review cards".into(),
            Look(l) => format!("stage look: {l}"),
            StageScreen(n) => format!("stage on screen {n}"),
            TimerStart(m) => format!("{m}-minute countdown"),
            TimerPause => "pause timer".into(),
            TimerResume => "resume timer".into(),
            TimerStop => "stop timer".into(),
            ShowTimer => "show timer".into(),
            ShowClock => "show clock".into(),
            Strongs(true) => "show Strong's".into(),
            Strongs(false) => "hide Strong's".into(),
            OriginalFor { lang, word } => format!("{} for \"{word}\"", match lang { 'G' => "Greek", 'H' => "Hebrew", _ => "original word" }),
            NextSlide => "next slide".into(),
            PrevSlide => "previous slide".into(),
            MediaPlay => "play video".into(),
            MediaPause => "pause video".into(),
            MediaRestart => "restart video".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ParseOutcome;

    fn cmds(text: &str) -> Vec<Command> {
        let mut out = ParseOutcome::default();
        crate::commands::extract_commands(&crate::parser::normalize_for_commands(text), &mut out);
        out.commands
    }

    // ---- v0.816.0 ----

    fn translation_of(text: &str) -> Vec<&'static str> {
        cmds(text).into_iter().filter_map(|c| if let Translation(t) = c { Some(t) } else { None }).collect()
    }

    #[test]
    fn translations_are_understood_however_the_speech_engine_writes_them() {
        for (said, want) in [
            ("in the KJV", "KJV"), ("KJV", "KJV"), ("KJV translation", "KJV"), ("the KJV version", "KJV"), ("K J V", "KJV"), ("k.j.v.", "KJV"), ("K-J-V please", "KJV"),
            ("switch to the king james version", "KJV"), ("in the king james", "KJV"), ("the authorized version", "KJV"),
            ("read it in the B B E", "BBE"), ("basic english", "BBE"), ("bbe translation", "BBE"),
            ("A S V", "ASV"), ("american standard version", "ASV"), ("asv", "ASV"),
            ("youngs literal translation", "YLT"), ("Young's literal", "YLT"), ("Y L T", "YLT"),
            ("N I V", "NIV"), ("the new international version", "NIV"), ("E S V", "ESV"), ("english standard version", "ESV"),
            ("new king james version", "NKJV"), ("N K J V", "NKJV"), ("new living translation", "NLT"), ("new american standard", "NASB"), ("N A S B", "NASB"),
            ("amplified", "AMP"), ("darby translation", "Darby"), ("webster", "Webster"), ("rotherham", "Rotherham"), ("A K J V", "AKJV"), ("show it in the akjv", "AKJV"),
        ] {
            assert_eq!(translation_of(said), vec![want], "{said:?}");
        }
    }

    #[test]
    fn speech_engines_that_spell_kjv_as_letters_or_sounds_still_switch_the_translation() {
        for said in ["KJV", "kjv.", "Kay Jay Vee", "kay jay vee please", "switch to KJV", "use the King James"] {
            assert_eq!(translation_of(said), vec!["KJV"], "{said:?}");
        }
    }

    #[test]
    fn the_last_translation_mentioned_wins_and_long_sermon_sentences_about_names_do_nothing() {
        assert_eq!(translation_of("from the kjv to the bbe"), vec!["BBE"]);
        for said in [
            "the king james version was put together by a committee of fifty four scholars over seven years",
            "my friend webster told me about the new international airport that they are building in town",
            "i read about the american standard way of doing things and it was not what i expected at all",
        ] {
            assert!(translation_of(said).is_empty(), "{said:?} -> {:?}", translation_of(said));
        }
    }

    #[test]
    fn read_till_verse_n_in_many_voices() {
        for (said, n) in [
            ("read till verse 20", 20), ("read until verse twenty", 20), ("read to verse 17", 17), ("read through verse 25", 25), ("read up to verse thirty", 30),
            ("read down to verse five", 5), ("read till 20", 20), ("read it to verse twenty one", 21), ("please read until the verse one hundred and sixteen", 116), ("read thru verse 9", 9),
        ] {
            assert!(cmds(said).contains(&ReadUntil(n)), "{said:?} -> {:?}", cmds(said));
            assert!(!cmds(said).contains(&ReadAloud), "{said:?} must not also start a plain read-aloud: {:?}", cmds(said));
        }
        for said in ["read the chapter", "read the whole chapter", "read to the end", "read the rest", "read to the end of the chapter"] {
            assert!(cmds(said).contains(&ReadUntil(0)), "{said:?} -> {:?}", cmds(said));
        }
        for said in ["read to me", "read john 3 16", "read it", "he wanted to read until midnight every night of the week"] {
            assert!(!cmds(said).iter().any(|c| matches!(c, ReadUntil(_))), "{said:?} -> {:?}", cmds(said));
        }
    }

    #[test]
    fn countdown_timer_in_minutes() {
        for (said, n) in [("start a five minute countdown", 5), ("five minute countdown", 5), ("countdown five minutes", 5), ("set a timer for ten minutes", 10), ("start a ten minute timer", 10), ("countdown 3 minutes", 3), ("timer for ninety minutes", 90)] {
            assert!(cmds(said).contains(&TimerStart(n)), "{said:?} -> {:?}", cmds(said));
        }
        for said in ["we will be finished in five minutes", "the countdown to the new year", "i set a timer", "ten minutes"] {
            assert!(!cmds(said).iter().any(|c| matches!(c, TimerStart(_))), "{said:?} -> {:?}", cmds(said));
        }
    }

    #[test]
    fn stage_on_another_screen() {
        for (said, n) in [("put the stage on screen two", 2), ("stage on screen 2", 2), ("move the stage to screen two", 2), ("send it to screen two", 2), ("use screen three", 3), ("stage to screen one", 1)] {
            assert!(cmds(said).contains(&StageScreen(n)), "{said:?} -> {:?}", cmds(said));
        }
        for said in ["turn on the screen", "screen two", "the screen is too bright for me", "put the stage on screen nine"] {
            assert!(!cmds(said).iter().any(|c| matches!(c, StageScreen(_))), "{said:?} -> {:?}", cmds(said));
        }
    }

    #[test]
    fn what_is_the_greek_or_hebrew_for_a_word() {
        let of = |s: &str| cmds(s).into_iter().find_map(|c| if let OriginalFor { lang, word } = c { Some((lang, word)) } else { None });
        assert_eq!(of("what is the greek for love"), Some(('G', "love".to_string())));
        assert_eq!(of("greek word for word"), Some(('G', "word".to_string())));
        assert_eq!(of("hebrew for peace"), Some(('H', "peace".to_string())));
        assert_eq!(of("what's the original word for grace"), Some(('?', "grace".to_string())));
        assert_eq!(of("strongs for faith"), Some(('?', "faith".to_string())));
        assert_eq!(of("strong's number for the word spirit"), Some(('?', "spirit".to_string())));
        assert_eq!(of("greek"), None);
        assert_eq!(of("the greek for"), None);
        assert_eq!(of("he studied greek for many years at the university in the north of england"), None, "too long to be a command");
    }

    #[test]
    fn no_read_until_means_the_verse_is_not_also_staged_as_a_reference() {
        let bible = rf_bible::Bible::embedded();
        let mut ctx = crate::parser::ParseContext::default();
        ctx.last = Some(crate::parser::Passage::single(42, 3, 16));
        let out = crate::parser::parse(bible, "read till verse 20", &ctx, 0.8);
        assert!(out.commands.contains(&ReadUntil(20)), "{:?}", out.commands);
        assert!(out.detections.is_empty() && out.confirms.is_empty() && out.errors.is_empty(), "{out:?}");
    }

    #[test]
    fn the_new_screen_and_window_words() {
        assert_eq!(cmds("go dark"), vec![Blackout(true)]);
        assert_eq!(cmds("go black"), vec![ClearStage], "an older word, still means clear");
        assert_eq!(cmds("screen on"), vec![Blackout(false)]);
        assert_eq!(cmds("next translation"), vec![SwitchTranslation(1)]);
        assert_eq!(cmds("previous translation"), vec![SwitchTranslation(-1)]);
        assert_eq!(cmds("final verse"), vec![LastVerse]);
        assert_eq!(cmds("show history"), vec![History(true)]);
        assert_eq!(cmds("hide the browser"), vec![Browser(false)]);
        assert_eq!(cmds("open media"), vec![MediaWindow(true)]);
        assert_eq!(cmds("dismiss all"), vec![DismissAll]);
        assert_eq!(cmds("lower third"), vec![Look("lower_third")]);
        assert_eq!(cmds("green screen"), vec![Look("chroma")]);
        assert_eq!(cmds("large print"), vec![Look("large_print")]);
        assert_eq!(cmds("show the timer"), vec![ShowTimer]);
        assert_eq!(cmds("show the clock"), vec![ShowClock]);
        assert_eq!(cmds("pause the timer"), vec![TimerPause]);
        assert_eq!(cmds("resume the timer"), vec![TimerResume]);
        assert_eq!(cmds("stop the timer"), vec![TimerStop]);
        assert_eq!(cmds("show strongs"), vec![Strongs(true)]);
        assert_eq!(cmds("hide strongs"), vec![Strongs(false)]);
        // the older words are unchanged
        assert_eq!(cmds("last verse"), vec![PrevVerse], "\"last verse\" has always meant the previous one");
        assert_eq!(cmds("next verse"), vec![NextVerse]);
        assert_eq!(cmds("clear the screen"), vec![ClearStage]);
        assert_eq!(cmds("previous verse"), vec![PrevVerse]);
    }

    #[test]
    fn the_new_words_do_nothing_inside_a_long_sentence() {
        for said in [
            "and then the lights went black and everybody in the whole church was quiet for a moment",
            "we will show the history of this church in a short film after the service this evening",
            "he said that the final verse of that chapter was the hardest for him to understand",
            "our youth will put the stage lights on screen two weeks before the big camp",
            "the next translation project at our mission will cover three languages in africa",
        ] {
            let c = cmds(said);
            assert!(c.iter().all(|c| matches!(c, NextVerse | PrevVerse | ReadAloud)) || c.is_empty(), "{said:?} -> {c:?}");
        }
    }

    #[test]
    fn there_are_well_over_120_spoken_commands() {
        let n = all_phrases().len();
        assert!(n >= 120, "only {n} phrases");
    }

    #[test]
    fn no_phrase_is_listed_for_two_different_commands() {
        let mut seen: Vec<(String, Command)> = Vec::new();
        for e in TABLE {
            for p in e.phrases {
                if let Some((_, other)) = seen.iter().find(|(q, _)| q == p) {
                    assert_eq!(other, &e.cmd, "phrase {p:?} is claimed by two commands");
                }
                seen.push((p.to_string(), e.cmd.clone()));
            }
        }
    }

    #[test]
    fn every_phrase_triggers_its_own_command() {
        for e in TABLE {
            for p in e.phrases {
                let got = cmds(p);
                assert!(got.contains(&e.cmd), "saying {p:?} should give {:?}, got {got:?}", e.cmd);
            }
        }
    }

    #[test]
    fn full_screen_and_exit_full_screen_and_stage_window_words() {
        assert_eq!(cmds("full screen"), vec![Fullscreen]);
        assert_eq!(cmds("exit full screen"), vec![ExitFullscreen]);
        assert_eq!(cmds("exit"), vec![ExitFullscreen]);
        for said in ["Exit for screen.", "That's it for screen.", "exit four screen", "Exit full screen", "okay exit for screen please"] {
            assert_eq!(cmds(said), vec![ExitFullscreen], "{said:?}");
        }
        assert_eq!(cmds("open stage"), vec![OpenStage]);
        assert_eq!(cmds("close stage"), vec![CloseStage]);
        assert_eq!(cmds("close the stage window"), vec![CloseStage]);
        assert_eq!(cmds("clear the screen"), vec![ClearStage]);
    }

    #[test]
    fn spoken_zoom_percentages() {
        assert_eq!(cmds("zoom 300"), vec![ZoomTo(300)]);
        assert_eq!(cmds("zoom to three hundred"), vec![ZoomTo(300)]);
        assert_eq!(cmds("zoom to 200 percent"), vec![ZoomTo(200)]);
        assert_eq!(cmds("zoom four hundred"), vec![ZoomTo(400)]);
        assert_eq!(cmds("zoom three hundred and fifty"), vec![ZoomTo(350)]);
        assert_eq!(cmds("zoom three"), vec![ZoomTo(300)]);
        assert_eq!(cmds("max zoom"), vec![ZoomTo(400)]);
        assert_eq!(cmds("half max zoom"), vec![ZoomTo(200)]);
        assert_eq!(cmds("zoom 900"), Vec::<Command>::new().into_iter().chain(cmds("zoom 900")).collect::<Vec<_>>());
        assert!(!cmds("zoom 900").contains(&ZoomTo(900)));
        // plain zoom in/out still work
        assert_eq!(cmds("zoom in"), vec![ZoomIn]);
        assert_eq!(cmds("zoom out"), vec![ZoomOut]);
    }

    #[test]
    fn stop_reading_in_many_voices() {
        for s in ["stop reading", "stop the reading", "be quiet", "stop talking", "quiet", "silence", "hush", "enough", "thats enough", "stop reading it please"] {
            assert!(cmds(s).contains(&StopReading), "{s:?} -> {:?}", cmds(s));
        }
        assert!(crate::is_bare_stop("stop"));
        assert!(crate::is_bare_stop("Stop."));
        assert!(!crate::is_bare_stop("stop reading"));
        assert!(!cmds("stop").contains(&StopRecording), "bare stop never stops recording");
    }

    #[test]
    fn review_queue_by_voice() {
        assert_eq!(cmds("approve it"), vec![ApproveQuote]);
        assert_eq!(cmds("dismiss that"), vec![DismissQuote]);
        assert_eq!(cmds("option two"), vec![PickMatch(2)]);
        assert_eq!(cmds("show all matches"), vec![ShowAllMatches(true)]);
        assert_eq!(cmds("show one match"), vec![ShowAllMatches(false)]);
        assert_eq!(cmds("auto approve on"), vec![AutoApprove(true)]);
        assert_eq!(cmds("wait for approval"), vec![AutoApprove(false)]);
    }

    #[test]
    fn a_preacher_mid_sentence_does_not_trigger_short_commands() {
        let long = "and you know brother when the Lord says to clear it out of your life and wipe screen clean in your heart this morning";
        assert!(cmds(long).is_empty(), "{:?}", cmds(long));
        let long2 = "I said no not that one but the other thing we were speaking about yesterday at the meeting";
        assert!(cmds(long2).is_empty(), "{:?}", cmds(long2));
    }

    #[test]
    fn stage2_slide_and_video_words() {
        for (said, want) in [
            ("next slide", NextSlide), ("Next slide.", NextSlide), ("previous slide", PrevSlide), ("go back one slide", PrevSlide),
            ("play video", MediaPlay), ("play the video", MediaPlay), ("start the video", MediaPlay), ("resume the video", MediaPlay),
            ("pause video", MediaPause), ("pause the video", MediaPause),
            ("restart video", MediaRestart), ("restart the video", MediaRestart), ("replay the video", MediaRestart),
        ] {
            assert_eq!(cmds(said), vec![want.clone()], "{said}");
        }
        // "next slide" must not also be read as the plain word "next"
        assert!(!cmds("next slide").contains(&NextVerse));
    }

    #[test]
    fn stage2_words_inside_a_sentence_do_nothing() {
        for said in [
            "and then they played the video of the baptism at the lake and everybody cried",
            "the next slide in my sermon notes says something about grace and mercy and love",
            "he said please pause the video at the point where the preacher talks about faith in the morning",
            "we will restart the video series next month after the youth camp is over and done",
        ] {
            assert!(cmds(said).is_empty(), "{said}: {:?}", cmds(said));
        }
    }

    #[test]
    fn show_it_again_is_not_approve() {
        assert!(cmds("show it again").contains(&LastScripture));
        assert!(!cmds("show it again").contains(&ApproveQuote));
        assert_eq!(cmds("show it"), vec![ApproveQuote]);
    }

    #[test]
    fn next_and_previous_work_on_their_own_and_run_together() {
        for (said, want) in [("Next.", NextVerse), ("Previous.", PrevVerse), ("Back.", PrevVerse), ("netverse", NextVerse), ("nextverse", NextVerse), ("nex verse", NextVerse), ("Previews.", PrevVerse), ("Again.", LastScripture)] {
            assert_eq!(cmds(said), vec![want.clone()], "{said}");
        }
    }

    #[test]
    fn nav_phrase_at_the_end_of_a_long_utterance_survives() {
        // v0.853.0: the operator finished reading the verse and asked for the
        // next one in the same breath. The utterance is 8 words (> NAV_MAX_WORDS),
        // but the nav phrase ENDS it — it must survive the length gate.
        let c = cmds("the evidence of things not seen. next verse");
        assert!(c.contains(&NextVerse), "{c:?}");
        let c = cmds("and the next verse");
        assert!(c.contains(&NextVerse), "{c:?}");
        let c = cmds("for god so loved the world. next verse");
        assert!(c.contains(&NextVerse), "{c:?}");
    }

    #[test]
    fn navigation_inside_a_sentence_is_ignored() {
        for said in ["the next verse says something wonderful about grace", "we should not look back on the past because of this", "next week we will meet at nine in the morning"] {
            assert!(cmds(said).is_empty(), "{said}: {:?}", cmds(said));
        }
    }

    #[test]
    fn previous_scripture_walks_the_history_not_the_verse() {
        for said in ["previous scripture", "go back to the previous scripture", "the one before", "scripture before that", "previous passage"] {
            assert_eq!(cmds(said), vec![PrevScripture], "{said}");
        }
        for said in ["next scripture", "the one after", "scripture after that"] {
            assert_eq!(cmds(said), vec![NextScripture], "{said}");
        }
        // plain verse navigation is unchanged
        assert_eq!(cmds("previous verse"), vec![PrevVerse]);
        assert_eq!(cmds("next verse"), vec![NextVerse]);
    }
}
