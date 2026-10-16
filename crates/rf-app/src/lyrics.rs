//! Song words pasted as plain text (v0.835.0).
//!
//! Paste what you copied from a lyric sheet, a chart or a document and RhemaFlow works out the rest:
//! the title, `[Verse 1]` / `Chorus:` labels (also `V1`, `C`, `Bridge`, `Pre-Chorus`, `Tag`...), blocks with no
//! label, a chorus that is written out again each time (it becomes ONE section the arrangement repeats),
//! the CCLI number, the copyright line, the author, and chord lines (dropped, so words show cleanly).
//! No song words are shipped: the church pastes its own and holds its own licence.

use rf_service::songs::{Section, Song};

/// Words of a section label that are recognised when they stand on their own line.
const LABEL_WORDS: &[&str] = &["verse", "chorus", "refrain", "bridge", "pre-chorus", "prechorus", "pre chorus", "tag", "intro", "outro", "ending", "interlude", "instrumental", "turnaround", "coda", "vamp", "hook", "v", "c", "b", "pc", "t", "i", "o", "e"];

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn words_only(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Is this whole line a section label? Returns the tidy label ("Verse 1", "Chorus").
pub fn label_of(line: &str) -> Option<String> {
    let t = line.trim().trim_matches(|c: char| matches!(c, '[' | ']' | '(' | ')' | '{' | '}' | ':' | '-' | '*' | '#' | '='  | '_')).trim();
    if t.is_empty() || t.len() > 30 {
        return None;
    }
    let low = t.to_lowercase();
    // "verse 2", "chorus 1", "bridge", "v1", "c2", "verse two"
    let (head, tail) = match low.find(|c: char| c.is_ascii_digit() || c == ' ') {
        Some(i) => (low[..i].trim().to_string(), low[i..].trim().to_string()),
        None => (low.clone(), String::new()),
    };
    // "pre-chorus 2" keeps its hyphen in the head
    let (head, tail) = if head.is_empty() { (low.clone(), String::new()) } else { (head, tail) };
    if !LABEL_WORDS.contains(&head.as_str()) {
        return None;
    }
    // one-letter heads only count when they are bracketed or followed by a number ("V1", "[C]", "C2")
    let bracketed = line.trim().starts_with(['[', '(', '{']);
    if head.len() <= 2 && !bracketed && tail.is_empty() {
        return None;
    }
    if !tail.is_empty() && !tail.chars().all(|c| c.is_ascii_digit() || c == ' ' || c == 'x') {
        // "verse two" etc: allow a few number words
        let ok = ["one", "two", "three", "four", "five", "six", "1", "2", "3", "4", "5", "6"].contains(&tail.as_str());
        if !ok {
            return None;
        }
    }
    let full = match head.as_str() {
        "v" => "Verse",
        "c" => "Chorus",
        "b" => "Bridge",
        "pc" => "Pre-Chorus",
        "prechorus" | "pre chorus" => "Pre-Chorus",
        "t" => "Tag",
        "i" => "Intro",
        "o" => "Outro",
        "e" => "Ending",
        other => {
            let mut c = other.chars();
            return Some(match c.next() {
                Some(f) => {
                    let name = format!("{}{}", f.to_uppercase(), c.as_str());
                    let name = if name == "Pre-chorus" { "Pre-Chorus".to_string() } else { name };
                    if tail.is_empty() { name } else { format!("{name} {}", tail_digits(&tail)) }
                }
                None => return None,
            });
        }
    };
    Some(if tail.is_empty() { full.to_string() } else { format!("{full} {}", tail_digits(&tail)) })
}

fn tail_digits(t: &str) -> String {
    match t {
        "one" => "1".into(),
        "two" => "2".into(),
        "three" => "3".into(),
        "four" => "4".into(),
        "five" => "5".into(),
        "six" => "6".into(),
        other => other.trim_end_matches('x').trim().to_string(),
    }
}

/// A line of chord names only ("G   D/F#   Em7   C") is not a lyric.
pub fn is_chord_line(line: &str) -> bool {
    let toks: Vec<&str> = line.split_whitespace().collect();
    if toks.is_empty() {
        return false;
    }
    fn chord(t: &str) -> bool {
        let t = t.trim_matches(|c| c == '(' || c == ')' || c == '|');
        if t.is_empty() {
            return true; // a bar line
        }
        let mut chars = t.chars().peekable();
        match chars.next() {
            Some('A'..='G') => {}
            _ => return false,
        }
        let rest: String = chars.collect();
        let rest = rest.trim_start_matches(['#', 'b']);
        let (main, bass) = rest.split_once('/').unwrap_or((rest, ""));
        let ok_main = main.chars().all(|c| c.is_ascii_digit()) || ["m", "maj", "min", "dim", "aug", "sus", "add", "M"].iter().any(|q| main.starts_with(q)) && main.chars().all(|c| c.is_ascii_alphanumeric() || c == '#' || c == 'b');
        let ok_bass = bass.is_empty() || bass.chars().next().map_or(false, |c| ('A'..='G').contains(&c));
        ok_main && ok_bass
    }
    let all = toks.iter().all(|t| chord(t));
    // a single "A" or "Am" on its own could be a word: demand 2+ chords or wide spacing
    all && (toks.len() >= 2 || line.starts_with("  ") || line.contains("   "))
}

fn find_ccli(line: &str) -> Option<String> {
    let low = line.to_lowercase();
    let i = low.find("ccli")?;
    let after = &line[i + 4..];
    let digits: String = after.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    // licence numbers are longer than song numbers: "CCLI License # 123456" has "license" before the digits
    let between = after.to_lowercase();
    let is_licence = between.split(|c: char| c.is_ascii_digit()).next().map_or(false, |s| s.contains("licen"));
    if digits.len() >= 4 && !is_licence {
        Some(digits)
    } else {
        None
    }
}

/// What the pasted text turned into, with a note of what was guessed.
pub struct Imported {
    pub song: Song,
    pub notes: Vec<String>,
}

/// Turn pasted text into a song. `title_hint` (what the person typed in the title box) wins over a guessed title.
pub fn import_pasted(text: &str, title_hint: &str) -> Imported {
    let mut notes = Vec::new();
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n");
    let mut title = title_hint.trim().to_string();
    let (mut author, mut copyright, mut ccli) = (String::new(), String::new(), String::new());
    let mut lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end().to_string();
        let t = line.trim();
        let low = t.to_lowercase();
        if let Some(n) = find_ccli(t) {
            ccli = n;
            continue;
        }
        if low.starts_with("ccli") {
            continue; // the church's licence number line
        }
        if t.starts_with('\u{a9}') || low.starts_with("copyright") || low.starts_with("(c)") {
            if copyright.is_empty() {
                copyright = t.to_string();
            }
            continue;
        }
        for p in ["words and music by", "words & music by", "written by", "words by", "music by", "author:", "by:"] {
            if low.starts_with(p) {
                if author.is_empty() {
                    author = t[p.len()..].trim().trim_start_matches(':').trim().to_string();
                }
                break;
            }
        }
        if !author.is_empty() && ["words and music by", "words & music by", "written by", "words by", "music by", "author:", "by:"].iter().any(|p| low.starts_with(p)) {
            continue;
        }
        if let Some(rest) = low.strip_prefix("title:") {
            let _ = rest;
            if title.is_empty() {
                title = t[6..].trim().to_string();
            }
            continue;
        }
        if is_chord_line(&line) {
            continue;
        }
        lines.push(line);
    }
    while lines.first().map_or(false, |l| l.trim().is_empty()) {
        lines.remove(0);
    }
    // a first line followed by a blank line, short, not a label: the title
    if title.is_empty() {
        if let (Some(first), Some(second)) = (lines.first(), lines.get(1)) {
            if second.trim().is_empty() && first.trim().len() <= 60 && label_of(first).is_none() {
                title = first.trim().trim_matches('"').to_string();
                lines.remove(0);
                notes.push(format!("The first line, \"{title}\", was used as the title."));
            }
        }
    }
    // split into blocks; a label line starts a new block
    struct Block {
        label: Option<String>,
        lines: Vec<String>,
    }
    let mut blocks: Vec<Block> = Vec::new();
    let mut cur = Block { label: None, lines: Vec::new() };
    let flush = |cur: &mut Block, blocks: &mut Vec<Block>| {
        if !cur.lines.is_empty() || cur.label.is_some() {
            let b = std::mem::replace(cur, Block { label: None, lines: Vec::new() });
            blocks.push(b);
        }
    };
    for l in &lines {
        if l.trim().is_empty() {
            if cur.label.is_some() && cur.lines.is_empty() {
                continue; // blank line right after a label
            }
            flush(&mut cur, &mut blocks);
        } else if let Some(lab) = label_of(l) {
            flush(&mut cur, &mut blocks);
            cur.label = Some(lab);
        } else {
            cur.lines.push(l.trim().to_string());
        }
    }
    flush(&mut cur, &mut blocks);
    blocks.retain(|b| !b.lines.is_empty());

    // unlabelled blocks that repeat word for word: the first repeated one is the chorus
    let key = |b: &Block| words_only(&b.lines.join(" "));
    let mut repeat_counts: Vec<(String, usize)> = Vec::new();
    for b in blocks.iter().filter(|b| b.label.is_none()) {
        let k = key(b);
        match repeat_counts.iter_mut().find(|(kk, _)| *kk == k) {
            Some((_, n)) => *n += 1,
            None => repeat_counts.push((k, 1)),
        }
    }
    let has_chorus_label = blocks.iter().any(|b| b.label.as_deref().map_or(false, |l| norm(l).starts_with("chorus")));
    let chorus_key: Option<String> = if has_chorus_label { None } else { blocks.iter().filter(|b| b.label.is_none()).map(|b| key(b)).find(|k| repeat_counts.iter().any(|(kk, n)| kk == k && *n >= 2)) };

    let mut sections: Vec<Section> = Vec::new();
    let mut arrangement: Vec<String> = Vec::new();
    let mut verse_n = 0;
    let mut had_repeat = false;
    for b in &blocks {
        let body = b.lines.join("\n");
        let k = key(b);
        // same words as a section already made: repeat it, do not make a second copy
        if let Some(existing) = sections.iter().find(|s| words_only(&s.text) == k) {
            arrangement.push(existing.label.clone());
            had_repeat = true;
            continue;
        }
        let label = match &b.label {
            Some(l) => {
                // keep a label unique
                if sections.iter().any(|s| norm(&s.label) == norm(l)) {
                    let mut n = 2;
                    while sections.iter().any(|s| norm(&s.label) == norm(&format!("{l} ({n})"))) {
                        n += 1;
                    }
                    format!("{l} ({n})")
                } else {
                    l.clone()
                }
            }
            None if chorus_key.as_deref() == Some(k.as_str()) => "Chorus".to_string(),
            None => {
                verse_n += 1;
                format!("Verse {verse_n}")
            }
        };
        arrangement.push(label.clone());
        sections.push(Section { label, text: body });
    }
    if chorus_key.is_some() {
        notes.push("A block that appeared more than once was made into the Chorus.".into());
    }
    if had_repeat {
        notes.push("Repeated blocks are kept once and the song order repeats them.".into());
    }
    let arrangement = if had_repeat { arrangement } else { Vec::new() };
    if title.is_empty() {
        title = "New song".into();
        notes.push("No title found: it is called \"New song\". Type a title before saving.".into());
    }
    let song = Song { id: 0, title, author, ccli_song: ccli, copyright, sections, arrangement };
    Imported { song, notes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_in_every_common_spelling() {
        assert_eq!(label_of("[Verse 1]").as_deref(), Some("Verse 1"));
        assert_eq!(label_of("Chorus:").as_deref(), Some("Chorus"));
        assert_eq!(label_of("(chorus)").as_deref(), Some("Chorus"));
        assert_eq!(label_of("V2").as_deref(), Some("Verse 2"));
        assert_eq!(label_of("[C]").as_deref(), Some("Chorus"));
        assert_eq!(label_of("Pre-Chorus").as_deref(), Some("Pre-Chorus"));
        assert_eq!(label_of("BRIDGE").as_deref(), Some("Bridge"));
        assert_eq!(label_of("Verse two").as_deref(), Some("Verse 2"));
        assert_eq!(label_of("Chorus 2x").as_deref(), Some("Chorus 2"));
        // ordinary words and lines are not labels
        assert_eq!(label_of("Amazing grace how sweet the sound"), None);
        assert_eq!(label_of("C"), None, "one bare letter could be a word");
        assert_eq!(label_of("Verse of the day is long and not a label at all"), None);
    }

    #[test]
    fn chord_lines_are_dropped_but_words_stay() {
        assert!(is_chord_line("G      D/F#    Em7   C"));
        assert!(is_chord_line("| Am  G  F |"));
        assert!(!is_chord_line("Amazing grace"));
        assert!(!is_chord_line("A"));
        assert!(!is_chord_line("Be still and know"));
    }

    #[test]
    fn a_labelled_song_with_metadata_is_understood() {
        let t = "Great Is Thy Faithfulness\n\nWords by Thomas Chisholm\n\u{a9} 1923 Hope Publishing\nCCLI Song # 18723\n\n[Verse 1]\nGreat is Thy faithfulness\nMorning by morning new mercies I see\n\n[Chorus]\nGreat is Thy faithfulness\nLord unto me\n\n[Verse 2]\nSummer and winter\nSpringtime and harvest\n\n[Chorus]\nGreat is Thy faithfulness\nLord unto me\n";
        let r = import_pasted(t, "");
        let s = r.song;
        assert_eq!(s.title, "Great Is Thy Faithfulness");
        assert_eq!(s.author, "Thomas Chisholm");
        assert_eq!(s.ccli_song, "18723");
        assert!(s.copyright.contains("Hope Publishing"));
        let labels: Vec<&str> = s.sections.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(labels, ["Verse 1", "Chorus", "Verse 2"], "the written-out second chorus is the same section");
        assert_eq!(s.arrangement, ["Verse 1", "Chorus", "Verse 2", "Chorus"]);
    }

    #[test]
    fn unlabelled_blocks_get_verse_numbers_and_a_repeated_block_becomes_the_chorus() {
        let t = "First verse line one\nFirst verse line two\n\nWe sing it again\nand again\n\nSecond verse line one\nSecond verse line two\n\nWe sing it again\nAnd again!\n";
        let s = import_pasted(t, "My Song").song;
        assert_eq!(s.title, "My Song");
        let labels: Vec<&str> = s.sections.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(labels, ["Verse 1", "Chorus", "Verse 2"]);
        assert_eq!(s.arrangement, ["Verse 1", "Chorus", "Verse 2", "Chorus"]);
    }

    #[test]
    fn plain_text_with_one_block_still_works_and_windows_line_endings_are_fine() {
        let s = import_pasted("Amazing grace\r\nhow sweet the sound\r\n", "").song;
        assert_eq!(s.sections.len(), 1);
        assert_eq!(s.sections[0].label, "Verse 1");
        assert_eq!(s.title, "New song");
        assert!(s.sections[0].text.contains("how sweet the sound"));
        assert!(import_pasted("", "x").song.sections.is_empty());
    }

    #[test]
    fn the_churchs_own_licence_line_is_not_taken_as_the_song_number() {
        let s = import_pasted("Song\n\nLine one\nCCLI License # 123456\n", "").song;
        assert_eq!(s.ccli_song, "");
        assert!(!s.sections[0].text.to_lowercase().contains("ccli"));
    }

    #[test]
    fn the_result_round_trips_through_the_normal_song_slides() {
        let s = import_pasted("[Verse 1]\na\nb\n\n[Chorus]\nc\nd\n", "T").song;
        let (slides, unknown) = s.slides(0);
        assert!(unknown.is_empty());
        assert_eq!(slides.len(), 2);
    }
}
