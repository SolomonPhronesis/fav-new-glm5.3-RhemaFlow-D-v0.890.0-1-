//! Feature 2: chord charts. Read them from ChordPro (`[G]Amazing`) or chords written over the words,
//! then transpose, put a capo on, show Nashville numbers, find the key, and write them out again.

use crate::goto::canon_label;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chord {
    /// 0 = C ... 11 = B
    pub root: u8,
    /// Everything after the note name and before a slash: `m7`, `sus4`, `maj7`...
    pub suffix: String,
    pub bass: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accidentals {
    Sharps,
    Flats,
}

const SHARP_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const FLAT_NAMES: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];

pub fn note_name(semitone: u8, acc: Accidentals) -> &'static str {
    let i = (semitone % 12) as usize;
    match acc {
        Accidentals::Sharps => SHARP_NAMES[i],
        Accidentals::Flats => FLAT_NAMES[i],
    }
}

/// A note name at the start of `s`: (semitone, bytes used).
fn read_note(s: &str) -> Option<(u8, usize)> {
    let mut cs = s.chars();
    let base = match cs.next()? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    match cs.next() {
        Some('#') | Some('\u{266F}') => Some(((base + 1) % 12, 1 + s[1..].chars().next().map_or(0, |c| c.len_utf8()))),
        Some('b') | Some('\u{266D}') => {
            // "Bb" is B-flat, but "Bb" can never be a word we confuse here because chord lines need every token to be a chord
            Some(((base + 11) % 12, 1 + s[1..].chars().next().map_or(0, |c| c.len_utf8())))
        }
        _ => Some((base, 1)),
    }
}

/// Is `suffix` something a chord can end in (m, 7, maj7, sus4, add9, dim, aug, (b5)...)?
fn suffix_ok(suffix: &str) -> bool {
    if suffix.ends_with('/') || suffix.starts_with('/') {
        return false;
    }
    let mut s = suffix;
    let words = ["maj", "min", "dim", "aug", "sus", "add", "omit", "no", "alt", "m", "M"];
    let marks = ["+", "-", "\u{00B0}", "\u{00F8}", "(", ")", "#", "b", ",", "/"];
    while !s.is_empty() {
        if let Some(w) = words.iter().find(|w| s.starts_with(**w)) {
            s = &s[w.len()..];
        } else if let Some(m) = marks.iter().find(|m| s.starts_with(**m)) {
            s = &s[m.len()..];
        } else if s.starts_with(|c: char| c.is_ascii_digit()) {
            s = s.trim_start_matches(|c: char| c.is_ascii_digit());
        } else {
            return false;
        }
    }
    true
}

impl Chord {
    pub fn parse(s: &str) -> Option<Chord> {
        let s = s.trim();
        if s.is_empty() || s.len() > 14 {
            return None;
        }
        let (root, used) = read_note(s)?;
        let rest = &s[used..];
        let (suffix, bass) = match rest.find('/') {
            Some(p) if rest[p + 1..].chars().next().map_or(false, |c| "ABCDEFG".contains(c)) => {
                let (b, bu) = read_note(&rest[p + 1..])?;
                if bu != rest[p + 1..].len() {
                    return None;
                }
                (&rest[..p], Some(b))
            }
            _ => (rest, None),
        };
        if !suffix_ok(suffix) {
            return None;
        }
        Some(Chord { root, suffix: suffix.to_string(), bass })
    }

    pub fn name(&self, acc: Accidentals) -> String {
        let mut s = format!("{}{}", note_name(self.root, acc), self.suffix);
        if let Some(b) = self.bass {
            s.push('/');
            s.push_str(note_name(b, acc));
        }
        s
    }

    pub fn transposed(&self, semitones: i32) -> Chord {
        let mv = |n: u8| ((n as i32 + semitones).rem_euclid(12)) as u8;
        Chord { root: mv(self.root), suffix: self.suffix.clone(), bass: self.bass.map(mv) }
    }

    pub fn is_minor(&self) -> bool {
        self.suffix.starts_with('m') && !self.suffix.starts_with("maj")
    }
}

/// Which spelling suits a key: flats for F, Bb, Eb, Ab, Db, Gb and their relative minors, sharps otherwise.
pub fn accidentals_for_key(key: &str) -> Accidentals {
    let k = key.trim();
    let minor = k.ends_with('m') && !k.ends_with("maj");
    let Some(c) = Chord::parse(k.trim_end_matches('m')) else { return Accidentals::Sharps };
    let flat_majors = [5u8, 10, 3, 8, 1, 6]; // F Bb Eb Ab Db Gb
    let tonic = if minor { (c.root + 3) % 12 } else { c.root }; // relative major
    if flat_majors.contains(&tonic) || (!minor && k.contains('b')) {
        Accidentals::Flats
    } else {
        Accidentals::Sharps
    }
}

/// Number of semitones to go up (0..=11) from one key to another.
pub fn steps_between(from_key: &str, to_key: &str) -> Option<i32> {
    let a = Chord::parse(from_key.trim_end_matches('m'))?;
    let b = Chord::parse(to_key.trim_end_matches('m'))?;
    Some(((b.root as i32 - a.root as i32) + 12) % 12)
}

/// Nashville number of a chord in a key: in G, C is `4`, Em is `6m`, D/F# is `5/7`.
pub fn nashville(chord: &Chord, key: &Chord) -> String {
    fn degree(semis: u8) -> &'static str {
        ["1", "b2", "2", "b3", "3", "4", "b5", "5", "b6", "6", "b7", "7"][(semis % 12) as usize]
    }
    let rel = |n: u8| (n + 12 - key.root) % 12;
    let mut s = format!("{}{}", degree(rel(chord.root)), chord.suffix);
    if let Some(b) = chord.bass {
        s.push('/');
        s.push_str(degree(rel(b)));
    }
    s
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ChartLine {
    pub text: String,
    /// (position in characters, chord)
    pub chords: Vec<(usize, Chord)>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ChartSection {
    pub label: String,
    pub lines: Vec<ChartLine>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ChordChart {
    pub title: String,
    pub artist: String,
    pub key: Option<String>,
    pub capo: u8,
    pub tempo: Option<u32>,
    pub copyright: String,
    pub ccli: String,
    pub sections: Vec<ChartSection>,
}

/// `[G]Amazing [C]grace` -> words and where the chords sit. Bracketed words that are not chords stay as text.
pub fn parse_inline_line(line: &str) -> ChartLine {
    let mut out = ChartLine::default();
    let mut rest = line;
    let mut count = 0usize;
    while let Some(p) = rest.find('[') {
        let (before, after) = rest.split_at(p);
        out.text.push_str(before);
        count += before.chars().count();
        match after.find(']') {
            Some(e) => {
                let inside = &after[1..e];
                match Chord::parse(inside) {
                    Some(c) => out.chords.push((count, c)),
                    None => {
                        out.text.push_str(&after[..=e]);
                        count += after[..=e].chars().count();
                    }
                }
                rest = &after[e + 1..];
            }
            None => {
                out.text.push_str(after);
                count += after.chars().count();
                rest = "";
            }
        }
    }
    out.text.push_str(rest);
    out
}

/// A line made only of chords (and spaces, bars, dashes): `G      C   D`
pub fn is_chord_line(line: &str) -> bool {
    let toks: Vec<&str> = line.split_whitespace().filter(|t| !t.chars().all(|c| "|-/\\.:".contains(c))).collect();
    !toks.is_empty() && toks.iter().all(|t| Chord::parse(t.trim_matches(|c| c == '(' || c == ')' || c == '|')).is_some())
}

/// Chords written above a line of words: each chord sits over the letter it falls on.
pub fn merge_chord_line(chord_line: &str, lyric: &str) -> ChartLine {
    let mut chords = Vec::new();
    let mut col = 0usize;
    let mut tok = String::new();
    let mut start = 0usize;
    let flush = |tok: &mut String, start: usize, chords: &mut Vec<(usize, Chord)>| {
        if !tok.is_empty() {
            if let Some(c) = Chord::parse(tok.trim_matches(|c| c == '(' || c == ')' || c == '|')) {
                chords.push((start, c));
            }
            tok.clear();
        }
    };
    for ch in chord_line.chars() {
        if ch.is_whitespace() {
            flush(&mut tok, start, &mut chords);
        } else {
            if tok.is_empty() {
                start = col;
            }
            tok.push(ch);
        }
        col += 1;
    }
    flush(&mut tok, start, &mut chords);
    ChartLine { text: lyric.to_string(), chords }
}

fn directive(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    let inner = t.strip_prefix('{')?.strip_suffix('}')?;
    let (name, value) = match inner.find(|c| c == ':' || c == ' ') {
        Some(p) => (&inner[..p], inner[p + 1..].trim()),
        None => (inner, ""),
    };
    Some((name.trim().to_lowercase(), value.to_string()))
}

fn is_section_label(t: &str) -> bool {
    let core = t.trim().trim_matches(|c| c == '[' || c == ']' || c == ':').trim();
    if core.is_empty() || core.len() > 24 {
        return false;
    }
    let c = canon_label(core);
    ["Verse", "Chorus", "Pre-Chorus", "Bridge", "Tag", "Ending", "Outro", "Intro", "Interlude"].iter().any(|k| c == *k || c.starts_with(&format!("{k} ")))
}

/// Read a chart from pasted text: ChordPro directives, `[Verse 1]` / `Chorus:` labels, inline `[G]` chords,
/// chords above words, and blank lines between sections.
pub fn parse_chart(text: &str) -> ChordChart {
    let mut chart = ChordChart::default();
    let lines: Vec<&str> = text.lines().collect();
    let mut cur: Option<ChartSection> = None;
    let mut i = 0;
    let close = |cur: &mut Option<ChartSection>, chart: &mut ChordChart| {
        if let Some(s) = cur.take() {
            if !s.lines.is_empty() {
                chart.sections.push(s);
            }
        }
    };
    while i < lines.len() {
        let raw = lines[i].trim_end();
        let t = raw.trim();
        i += 1;
        if t.starts_with('#') {
            continue;
        }
        if let Some((name, value)) = directive(t) {
            match name.as_str() {
                "title" | "t" => chart.title = value,
                "subtitle" | "st" | "artist" | "author" => {
                    if chart.artist.is_empty() {
                        chart.artist = value
                    }
                }
                "key" => chart.key = Some(value).filter(|v| !v.is_empty()),
                "capo" => chart.capo = value.parse().unwrap_or(0),
                "tempo" | "bpm" => chart.tempo = value.parse().ok(),
                "copyright" => chart.copyright = value,
                "ccli" | "ccli_song" | "ccli_number" => chart.ccli = value,
                "start_of_verse" | "sov" | "start_of_chorus" | "soc" | "start_of_bridge" | "sob" => {
                    close(&mut cur, &mut chart);
                    let default = match name.as_str() {
                        "start_of_chorus" | "soc" => "Chorus",
                        "start_of_bridge" | "sob" => "Bridge",
                        _ => "",
                    };
                    let label = if value.is_empty() { default.to_string() } else { canon_label(&value) };
                    cur = Some(ChartSection { label, lines: Vec::new() });
                }
                "end_of_verse" | "eov" | "end_of_chorus" | "eoc" | "end_of_bridge" | "eob" => close(&mut cur, &mut chart),
                "comment" | "c" | "comment_italic" | "ci" | "comment_box" | "cb" => {
                    if is_section_label(&value) {
                        close(&mut cur, &mut chart);
                        cur = Some(ChartSection { label: canon_label(&value), lines: Vec::new() });
                    }
                }
                _ => {}
            }
            continue;
        }
        if t.is_empty() {
            close(&mut cur, &mut chart);
            continue;
        }
        if is_section_label(t) && !t.contains(' ') || (is_section_label(t) && (t.starts_with('[') || t.ends_with(':') || t.chars().last().map_or(false, |c| c.is_ascii_digit()))) {
            close(&mut cur, &mut chart);
            cur = Some(ChartSection { label: canon_label(t), lines: Vec::new() });
            continue;
        }
        let section = cur.get_or_insert_with(ChartSection::default);
        let line = if is_chord_line(t) && !t.contains('[') {
            // chords over the next line of words, if there is one
            match lines.get(i).map(|l| l.trim_end()) {
                Some(next) if !next.trim().is_empty() && !is_chord_line(next) && directive(next).is_none() && !is_section_label(next) => {
                    i += 1;
                    merge_chord_line(raw, next)
                }
                _ => merge_chord_line(raw, ""),
            }
        } else {
            parse_inline_line(raw)
        };
        section.lines.push(line);
    }
    close(&mut cur, &mut chart);
    // unlabelled sections become Verse 1, Verse 2...
    let mut n = 0;
    for s in &mut chart.sections {
        if s.label.is_empty() {
            n += 1;
            s.label = format!("Verse {n}");
        }
    }
    if chart.key.is_none() {
        chart.key = detect_key(&chart);
    }
    chart
}

pub fn all_chords(chart: &ChordChart) -> Vec<&Chord> {
    chart.sections.iter().flat_map(|s| s.lines.iter()).flat_map(|l| l.chords.iter().map(|(_, c)| c)).collect()
}

/// A best guess at the key from the chords used and where the song starts and ends.
pub fn detect_key(chart: &ChordChart) -> Option<String> {
    let chords = all_chords(chart);
    if chords.is_empty() {
        return None;
    }
    // chords of a major key by semitones above the tonic, and whether each is minor
    let diatonic: [(u8, bool); 6] = [(0, false), (2, true), (4, true), (5, false), (7, false), (9, true)];
    let mut best: (i32, u8) = (-1, 0);
    for tonic in 0..12u8 {
        let mut score = 0;
        for c in &chords {
            let rel = (c.root + 12 - tonic) % 12;
            if diatonic.iter().any(|(r, minor)| *r == rel && *minor == c.is_minor()) {
                score += 2;
            } else if rel == 11 || rel == 10 {
                score += 0; // a passing chord: neither for nor against
            } else {
                score -= 1;
            }
        }
        if chords[0].root == tonic && !chords[0].is_minor() {
            score += 3;
        }
        if chords[chords.len() - 1].root == tonic && !chords[chords.len() - 1].is_minor() {
            score += 3;
        }
        if score > best.0 {
            best = (score, tonic);
        }
    }
    let tonic = best.1;
    // a song that starts and ends on the relative minor is in the minor key
    let minor_root = (tonic + 9) % 12;
    let first = chords[0];
    let last = chords[chords.len() - 1];
    let minor = first.root == minor_root && first.is_minor() && last.root == minor_root && last.is_minor();
    let acc = if matches!(accidentals_for_key(&format!("{}{}", note_name(tonic, Accidentals::Sharps), "")), Accidentals::Flats) { Accidentals::Flats } else { Accidentals::Sharps };
    Some(if minor { format!("{}m", note_name(minor_root, acc)) } else { note_name(tonic, acc).to_string() })
}

/// Move every chord. `prefer` of `None` chooses the spelling from the new key.
pub fn transpose_chart(chart: &ChordChart, semitones: i32, prefer: Option<Accidentals>) -> ChordChart {
    let mut out = chart.clone();
    if let Some(k) = &chart.key {
        let rootchord = Chord::parse(k.trim_end_matches('m'));
        if let Some(r) = rootchord {
            let minor = k.ends_with('m');
            let moved = r.transposed(semitones);
            let acc = prefer.unwrap_or_else(|| accidentals_for_key(&format!("{}{}", note_name(moved.root, Accidentals::Sharps), if minor { "m" } else { "" })));
            out.key = Some(format!("{}{}", note_name(moved.root, acc), if minor { "m" } else { "" }));
        }
    }
    for s in &mut out.sections {
        for l in &mut s.lines {
            for (_, c) in &mut l.chords {
                *c = c.transposed(semitones);
            }
        }
    }
    out
}

/// The shapes to play with a capo on fret `capo`: the sounding chords moved down by that many frets.
pub fn with_capo(chart: &ChordChart, capo: u8) -> ChordChart {
    let mut shaped = transpose_chart(chart, -(capo as i32), None);
    shaped.capo = capo;
    shaped
}

/// Which capo fret gives the easiest shapes: tries 0..=7 and prefers open-friendly keys (G, C, D, A, E and minors Em, Am, Dm).
pub fn easiest_capo(chart: &ChordChart) -> u8 {
    let friendly = [0u8, 7, 2, 9, 4, 5]; // C G D A E F
    let mut best = (i32::MIN, 0u8);
    for capo in 0..=7u8 {
        let moved = transpose_chart(chart, -(capo as i32), None);
        let chords = all_chords(&moved);
        let score: i32 = chords.iter().map(|c| if friendly.contains(&c.root) && !c.suffix.contains('#') { 2 } else { -1 }).sum::<i32>() - capo as i32;
        if score > best.0 {
            best = (score, capo);
        }
    }
    best.1
}

impl ChartLine {
    /// Two lines for a monospace display: the chords, then the words under them.
    pub fn over(&self, acc: Accidentals) -> (String, String) {
        let mut chord_line = String::new();
        let mut text: Vec<char> = self.text.chars().collect();
        let mut shift = 0usize; // spaces added to the words so two chords never touch
        let mut col = 0usize;
        for (pos, c) in &self.chords {
            let name = c.name(acc);
            let want = *pos + shift;
            if want < col {
                // chord names overlap: push the words along
                let push = col - want + 1;
                let at = (*pos).min(text.len());
                for _ in 0..push {
                    text.insert(at, ' ');
                }
                shift += push;
            }
            let want = *pos + shift;
            while col < want {
                chord_line.push(' ');
                col += 1;
            }
            chord_line.push_str(&name);
            col += name.chars().count();
            if col == want + name.chars().count() && col >= want {
                chord_line.push(' ');
                col += 1;
            }
        }
        (chord_line.trim_end().to_string(), text.into_iter().collect::<String>())
    }

    pub fn inline(&self, acc: Accidentals) -> String {
        let mut out = String::new();
        let chars: Vec<char> = self.text.chars().collect();
        let mut ci = 0;
        for pos in 0..=chars.len() {
            while ci < self.chords.len() && self.chords[ci].0 == pos {
                out.push('[');
                out.push_str(&self.chords[ci].1.name(acc));
                out.push(']');
                ci += 1;
            }
            if pos < chars.len() {
                out.push(chars[pos]);
            }
        }
        // chords past the end of the words
        while ci < self.chords.len() {
            out.push('[');
            out.push_str(&self.chords[ci].1.name(acc));
            out.push(']');
            ci += 1;
        }
        out
    }
}

/// The chart as readable chords-over-words text.
pub fn render_over(chart: &ChordChart, acc: Accidentals) -> String {
    let mut out = String::new();
    for s in &chart.sections {
        out.push_str(&format!("{}:\n", s.label));
        for l in &s.lines {
            let (c, t) = l.over(acc);
            if !c.is_empty() {
                out.push_str(&c);
                out.push('\n');
            }
            if !t.trim().is_empty() || c.is_empty() {
                out.push_str(&t);
                out.push('\n');
            }
        }
        out.push('\n');
    }
    out.trim_end().to_string() + "\n"
}

/// The chart as ChordPro text.
pub fn render_chordpro(chart: &ChordChart, acc: Accidentals) -> String {
    let mut out = String::new();
    if !chart.title.is_empty() {
        out.push_str(&format!("{{title: {}}}\n", chart.title));
    }
    if !chart.artist.is_empty() {
        out.push_str(&format!("{{artist: {}}}\n", chart.artist));
    }
    if let Some(k) = &chart.key {
        out.push_str(&format!("{{key: {k}}}\n"));
    }
    if chart.capo > 0 {
        out.push_str(&format!("{{capo: {}}}\n", chart.capo));
    }
    if let Some(t) = chart.tempo {
        out.push_str(&format!("{{tempo: {t}}}\n"));
    }
    if !chart.copyright.is_empty() {
        out.push_str(&format!("{{copyright: {}}}\n", chart.copyright));
    }
    if !chart.ccli.is_empty() {
        out.push_str(&format!("{{ccli: {}}}\n", chart.ccli));
    }
    for s in &chart.sections {
        out.push_str(&format!("\n{{comment: {}}}\n", s.label));
        for l in &s.lines {
            out.push_str(&l.inline(acc));
            out.push('\n');
        }
    }
    out
}

/// Just the words (no chords): what the congregation sees.
pub fn lyrics_only(chart: &ChordChart) -> Vec<(String, String)> {
    chart.sections.iter().map(|s| (s.label.clone(), s.lines.iter().map(|l| l.text.trim_end().to_string()).filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join("\n"))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRACE: &str = "{title: Amazing Grace}\n{artist: John Newton}\n{key: G}\n\n{comment: Verse 1}\n[G]Amazing [C]grace how [G]sweet the [D]sound\nThat [G]saved a [Em]wretch like [D]me\n\n{comment: Chorus}\n[C]I once was [G]lost\n";

    #[test]
    fn chords_are_read_and_names_that_are_words_are_not_chords() {
        for good in ["G", "Am", "F#m7", "Bb", "C/E", "D7sus4", "Cmaj7", "Gadd9", "A+", "Ebdim", "G(b5)", "E5"] {
            assert!(Chord::parse(good).is_some(), "{good}");
        }
        for bad in ["Amazing", "Hello", "be", "Dad", "Egg", "the", "", "H", "G/", "Cx"] {
            assert!(Chord::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn names_round_trip_with_the_right_spelling() {
        let c = Chord::parse("Bb/D").unwrap();
        assert_eq!(c.name(Accidentals::Flats), "Bb/D");
        assert_eq!(c.name(Accidentals::Sharps), "A#/D");
    }

    #[test]
    fn transposing_moves_root_and_bass_and_keeps_the_quality() {
        let c = Chord::parse("Am7/G").unwrap();
        assert_eq!(c.transposed(2).name(Accidentals::Sharps), "Bm7/A");
        assert_eq!(c.transposed(-12).name(Accidentals::Sharps), "Am7/G");
        assert_eq!(Chord::parse("B").unwrap().transposed(1).name(Accidentals::Sharps), "C");
    }

    #[test]
    fn spelling_follows_the_key() {
        assert_eq!(accidentals_for_key("F"), Accidentals::Flats);
        assert_eq!(accidentals_for_key("Bb"), Accidentals::Flats);
        assert_eq!(accidentals_for_key("Dm"), Accidentals::Flats); // relative of F
        assert_eq!(accidentals_for_key("G"), Accidentals::Sharps);
        assert_eq!(accidentals_for_key("F#"), Accidentals::Flats); // Gb is the flat key; F# is its twin
    }

    #[test]
    fn steps_between_keys_go_up() {
        assert_eq!(steps_between("G", "A"), Some(2));
        assert_eq!(steps_between("A", "G"), Some(10));
        assert_eq!(steps_between("C", "C"), Some(0));
        assert_eq!(steps_between("X", "C"), None);
    }

    #[test]
    fn nashville_numbers() {
        let g = Chord::parse("G").unwrap();
        assert_eq!(nashville(&Chord::parse("C").unwrap(), &g), "4");
        assert_eq!(nashville(&Chord::parse("Em").unwrap(), &g), "6m");
        assert_eq!(nashville(&Chord::parse("D/F#").unwrap(), &g), "5/7");
        assert_eq!(nashville(&Chord::parse("Bb").unwrap(), &g), "b3");
    }

    #[test]
    fn a_chordpro_song_is_read_with_its_headers_and_sections() {
        let c = parse_chart(GRACE);
        assert_eq!(c.title, "Amazing Grace");
        assert_eq!(c.artist, "John Newton");
        assert_eq!(c.key.as_deref(), Some("G"));
        assert_eq!(c.sections.len(), 2);
        assert_eq!(c.sections[0].label, "Verse 1");
        assert_eq!(c.sections[0].lines[0].text, "Amazing grace how sweet the sound");
        assert_eq!(c.sections[0].lines[0].chords.len(), 4);
        assert_eq!(c.sections[0].lines[0].chords[1].0, 8); // [C] before "grace"
        assert_eq!(c.sections[1].label, "Chorus");
    }

    #[test]
    fn chords_over_words_are_merged_onto_the_letter_under_them() {
        let c = parse_chart("Verse 1:\nG       C\nAmazing grace\nD\nsound\n");
        let l0 = &c.sections[0].lines[0];
        assert_eq!(l0.text, "Amazing grace");
        assert_eq!(l0.chords.iter().map(|(p, _)| *p).collect::<Vec<_>>(), vec![0, 8]);
        assert_eq!(c.sections[0].lines[1].chords[0].0, 0);
    }

    #[test]
    fn a_line_of_words_is_not_taken_for_chords() {
        assert!(!is_chord_line("Amazing grace"));
        assert!(!is_chord_line("A Bm hello"));
        assert!(is_chord_line("G  C/E  D7"));
        assert!(is_chord_line("| G | C | D |"));
        assert!(!is_chord_line(""));
    }

    #[test]
    fn unlabelled_blocks_become_numbered_verses() {
        let c = parse_chart("[G]one\n\n[G]two\n");
        assert_eq!(c.sections.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), vec!["Verse 1", "Verse 2"]);
    }

    #[test]
    fn transposing_a_chart_moves_the_key_and_every_chord() {
        let c = parse_chart(GRACE);
        let up = transpose_chart(&c, 2, None);
        assert_eq!(up.key.as_deref(), Some("A"));
        assert_eq!(up.sections[0].lines[0].chords[0].1.name(Accidentals::Sharps), "A");
        let flat = transpose_chart(&c, -2, None); // F: flats
        assert_eq!(flat.key.as_deref(), Some("F"));
        let back = transpose_chart(&up, -2, None);
        assert_eq!(back.sections, c.sections);
    }

    #[test]
    fn the_key_is_found_from_the_chords() {
        let c = parse_chart("[G]a [C]b [D]c [Em]d [G]e");
        assert_eq!(c.key.as_deref(), Some("G"));
        let m = parse_chart("[Am]a [F]b [C]c [G]d [Am]e");
        assert_eq!(m.key.as_deref(), Some("Am"));
        let f = parse_chart("[F]a [Bb]b [C]c [F]d");
        assert_eq!(f.key.as_deref(), Some("F"));
        assert_eq!(parse_chart("no chords here").key, None);
    }

    #[test]
    fn a_capo_shows_the_shapes_to_play() {
        let c = parse_chart("{key: A}\n[A]a [D]b [E]c [A]d");
        let capo2 = with_capo(&c, 2);
        assert_eq!(capo2.capo, 2);
        assert_eq!(capo2.key.as_deref(), Some("G"));
        assert_eq!(capo2.sections[0].lines[0].chords[0].1.name(Accidentals::Sharps), "G");
    }

    #[test]
    fn the_easiest_capo_turns_hard_keys_into_open_shapes() {
        let c = parse_chart("[Bb]a [Eb]b [F]c [Bb]d");
        let capo = easiest_capo(&c);
        assert!(capo > 0);
        let shapes = with_capo(&c, capo);
        assert!(["G", "C", "D", "A", "E", "F"].contains(&shapes.key.as_deref().unwrap()), "{:?}", shapes.key);
        assert_eq!(easiest_capo(&parse_chart("[G]a [C]b [D]c")), 0);
    }

    #[test]
    fn chords_over_words_line_up_in_monospace() {
        let l = parse_inline_line("[G]Amazing [C]grace");
        let (c, t) = l.over(Accidentals::Sharps);
        assert_eq!(t, "Amazing grace");
        assert_eq!(c.find('C'), Some(8));
        assert_eq!(c.find('G'), Some(0));
    }

    #[test]
    fn crowded_chords_push_the_words_apart() {
        let l = parse_inline_line("[Gsus4][G]hi");
        let (c, t) = l.over(Accidentals::Sharps);
        let g_pos = c.rfind('G').unwrap();
        assert!(g_pos >= 6, "{c:?}");
        assert!(t.starts_with(' '), "{t:?}");
    }

    #[test]
    fn inline_text_round_trips() {
        let c = parse_chart(GRACE);
        let l = &c.sections[0].lines[0];
        assert_eq!(l.inline(Accidentals::Sharps), "[G]Amazing [C]grace how [G]sweet the [D]sound");
        let again = parse_chart(&render_chordpro(&c, Accidentals::Sharps));
        assert_eq!(again.sections, c.sections);
        assert_eq!(again.title, c.title);
    }

    #[test]
    fn lyrics_only_drops_the_chords() {
        let c = parse_chart(GRACE);
        let l = lyrics_only(&c);
        assert_eq!(l[0].0, "Verse 1");
        assert_eq!(l[0].1, "Amazing grace how sweet the sound\nThat saved a wretch like me");
    }

    #[test]
    fn an_empty_or_odd_paste_never_panics() {
        for s in ["", "\n\n", "{", "{title", "[", "[G", "]]", "{c: Chorus}", "G C D"] {
            let _ = parse_chart(s);
        }
    }
}
