//! Feature 1: bring songs in from the open song formats, and take them out again.
//!
//! Reads OpenLyrics (.xml), OpenSong (.xml / no extension), ChordPro (.cho .chordpro .crd .txt with `{title: ...}`),
//! chords-over-words text, and plain lyrics. Writes OpenLyrics, ChordPro and plain text, so a church is never locked in.
//!
//! Not read: other programs' private binary or database files. Export those songs as text or OpenLyrics from the
//! program that made them, then drop the result here.

use crate::chords::{self, ChordChart};
use crate::goto::{canon_label, label_key};
use crate::xml::{self, Child, Node};
use rf_service::songs::{Section, Song};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    OpenLyrics,
    OpenSong,
    ChordPro,
    ChordsOverWords,
    PlainText,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::OpenLyrics => "OpenLyrics",
            Format::OpenSong => "OpenSong",
            Format::ChordPro => "ChordPro",
            Format::ChordsOverWords => "chords over words",
            Format::PlainText => "plain lyrics",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Imported {
    pub song: Song,
    pub chart: Option<ChordChart>,
    pub format: Format,
    /// Things worth telling the operator ("2 translations were skipped").
    pub notes: Vec<String>,
}

const MAX_BYTES: usize = 2_000_000;

pub fn detect(file_name: &str, text: &str) -> Format {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    if t.starts_with('<') {
        if let Ok(n) = xml::parse(t) {
            if n.name == "song" {
                return if n.child("lyrics").map_or(false, |l| l.child("verse").is_some()) { Format::OpenLyrics } else { Format::OpenSong };
            }
        }
    }
    let lower = file_name.to_lowercase();
    let chordpro_ext = [".cho", ".chordpro", ".chopro", ".crd", ".cpro"].iter().any(|e| lower.ends_with(e));
    let directives = ["{title", "{t:", "{start_of", "{sov", "{soc", "{key:", "{subtitle", "{comment", "{c:"];
    if chordpro_ext || directives.iter().any(|d| t.contains(d)) {
        return Format::ChordPro;
    }
    if t.lines().any(|l| chords::is_chord_line(l) && l.trim().len() > 1) && t.lines().any(|l| !chords::is_chord_line(l) && !l.trim().is_empty()) {
        return Format::ChordsOverWords;
    }
    Format::PlainText
}

fn title_from_file(name: &str) -> String {
    let base = name.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(name);
    let stem = match base.rfind('.') {
        Some(p) if p > 0 && base.len() - p <= 9 => &base[..p],
        _ => base,
    };
    stem.replace('_', " ").trim().to_string()
}

pub fn import(file_name: &str, text: &str) -> Result<Imported, String> {
    if text.len() > MAX_BYTES {
        return Err(format!("{file_name}: the file is larger than 2 MB, which is far too big for one song"));
    }
    let text = text.trim_start_matches('\u{feff}');
    if text.trim().is_empty() {
        return Err(format!("{file_name}: the file is empty"));
    }
    match detect(file_name, text) {
        Format::OpenLyrics => open_lyrics(file_name, text),
        Format::OpenSong => open_song(file_name, text),
        Format::ChordPro => from_chart(file_name, text, Format::ChordPro),
        Format::ChordsOverWords => from_chart(file_name, text, Format::ChordsOverWords),
        Format::PlainText => {
            if text.trim_start().starts_with('<') {
                return Err(format!("{file_name}: this looks like XML but not a song file we know (we read OpenLyrics and OpenSong)"));
            }
            let mut song = Song::from_text(&title_from_file(file_name), text);
            if song.sections.is_empty() {
                return Err(format!("{file_name}: no words found"));
            }
            let first_title = song.title.is_empty();
            if first_title {
                song.title = "Untitled song".into();
            }
            Ok(Imported { song, chart: None, format: Format::PlainText, notes: Vec::new() })
        }
    }
}

/// Many files at once: the songs that worked, and one message per file that did not.
pub fn import_many(files: &[(String, String)]) -> (Vec<Imported>, Vec<String>) {
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    for (name, text) in files {
        match import(name, text) {
            Ok(i) => ok.push(i),
            Err(e) => bad.push(e),
        }
    }
    (ok, bad)
}

/// A title that is not already used: "Amazing Grace" -> "Amazing Grace (2)".
pub fn unique_title(existing: &[String], title: &str) -> String {
    let taken = |t: &str| existing.iter().any(|e| e.trim().eq_ignore_ascii_case(t.trim()));
    if !taken(title) {
        return title.to_string();
    }
    (2..).map(|n| format!("{title} ({n})")).find(|t| !taken(t)).unwrap_or_else(|| title.to_string())
}

/// Sections in the order they appear, merged by name, plus the order they are sung.
#[derive(Default)]
struct SectionBuilder {
    sections: Vec<Section>,
    order: Vec<String>,
}

impl SectionBuilder {
    /// Add a block. A repeated name with the same words only extends the order; with different words, it gets a new number.
    fn add(&mut self, label: &str, text: &str, merge_parts: bool) {
        let label = canon_label(label);
        let label = if label.is_empty() { format!("Verse {}", self.sections.len() + 1) } else { label };
        let key = label_key(&label);
        if let Some(s) = self.sections.iter_mut().find(|s| label_key(&s.label) == key) {
            if merge_parts {
                if !s.text.is_empty() && !text.is_empty() {
                    s.text.push('\n');
                }
                s.text.push_str(text);
                return;
            }
            if s.text.trim() == text.trim() {
                self.order.push(s.label.clone());
                return;
            }
            let family = key.trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ').to_string();
            let fam_disp = canon_label(&family);
            let n = (2..).find(|n| !self.sections.iter().any(|s| label_key(&s.label) == format!("{family} {n}"))).unwrap_or(2);
            let nl = format!("{fam_disp} {n}");
            self.sections.push(Section { label: nl.clone(), text: text.to_string() });
            self.order.push(nl);
            return;
        }
        self.sections.push(Section { label: label.clone(), text: text.to_string() });
        self.order.push(label);
    }

    fn finish(self, title: String, author: String, ccli: String, copyright: String, explicit_order: Option<Vec<String>>, notes: &mut Vec<String>) -> Song {
        let labels: Vec<String> = self.sections.iter().map(|s| label_key(&s.label)).collect();
        let arrangement = match explicit_order {
            Some(o) => {
                let mut kept = Vec::new();
                for tok in o {
                    let c = canon_label(&tok);
                    if labels.contains(&label_key(&c)) {
                        kept.push(c);
                    } else {
                        notes.push(format!("The song order names \"{tok}\" but there is no such part; it was left out."));
                    }
                }
                kept
            }
            None => {
                // only keep a spelled-out order when something repeats
                let mut seen = std::collections::HashSet::new();
                if self.order.iter().any(|l| !seen.insert(label_key(l))) {
                    self.order.clone()
                } else {
                    Vec::new()
                }
            }
        };
        Song { id: 0, title, author, ccli_song: ccli, copyright, sections: self.sections, arrangement }
    }
}

// ---------------------------------------------------------------- OpenLyrics

fn collapse_ws(s: &str) -> String {
    // inside <lines>, a newline plus indentation is just layout, not a line break
    let mut out = String::new();
    let mut last_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out
}

fn lines_text(n: &Node, out: &mut String) {
    for c in &n.children {
        match c {
            Child::Text(t) => out.push_str(&collapse_ws(t)),
            Child::Node(x) => match x.name.as_str() {
                "br" => {
                    let trimmed = out.trim_end_matches(' ').len();
                    out.truncate(trimmed);
                    out.push('\n');
                }
                "chord" => {
                    if let Some(name) = x.attr("name").or_else(|| x.attr("root")) {
                        out.push('[');
                        out.push_str(name);
                        out.push(']');
                    }
                }
                "comment" => {}
                _ => lines_text(x, out),
            },
        }
    }
}

fn open_lyrics(file_name: &str, text: &str) -> Result<Imported, String> {
    let root = xml::parse(text).map_err(|e| format!("{file_name}: not readable as XML ({e})"))?;
    let props = root.child("properties");
    let mut notes = Vec::new();
    let title = props.and_then(|p| p.child("titles")).and_then(|t| t.child("title")).map(|t| t.text().trim().to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| title_from_file(file_name));
    let author = props
        .and_then(|p| p.child("authors"))
        .map(|a| a.children_named("author").map(|x| x.text().trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    let copyright = props.and_then(|p| p.child("copyright")).map(|c| c.text().trim().to_string()).unwrap_or_default();
    let ccli = props.and_then(|p| p.child("ccliNo")).map(|c| c.text().trim().to_string()).unwrap_or_default();
    let order = props.and_then(|p| p.child("verseOrder")).map(|o| o.text().split_whitespace().map(|s| s.to_string()).collect::<Vec<_>>()).filter(|o| !o.is_empty());
    let lyrics = root.child("lyrics").ok_or_else(|| format!("{file_name}: the song has no <lyrics>"))?;
    let mut b = SectionBuilder::default();
    let mut chordpro = String::new();
    let mut skipped_lang = 0;
    // the first time a part name appears decides its language; the same name in another language is a translation
    let mut lang_of: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
    for v in lyrics.children_named("verse") {
        let lang = v.attr("lang").map(|s| s.to_string());
        let name_key = v.attr("name").unwrap_or("v1").to_string();
        let first = lang_of.entry(name_key).or_insert_with(|| lang.clone());
        if *first != lang {
            skipped_lang += 1;
            continue;
        }
        let mut body = String::new();
        for (i, l) in v.children_named("lines").enumerate() {
            if i > 0 {
                body.push('\n');
            }
            lines_text(l, &mut body);
        }
        let body = body.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
        if body.is_empty() {
            continue;
        }
        let name = v.attr("name").unwrap_or("v1");
        let label = canon_label(name);
        chordpro.push_str(&format!("\n{{comment: {label}}}\n{body}\n"));
        let plain: Vec<String> = body.lines().map(|l| chords::parse_inline_line(l).text).collect();
        b.add(&label, &plain.join("\n"), true);
    }
    if b.sections.is_empty() {
        return Err(format!("{file_name}: the song has no verses with words"));
    }
    if skipped_lang > 0 {
        notes.push(format!("{skipped_lang} verse(s) in another language were skipped (only the first language is kept)."));
    }
    let has_chords = chordpro.contains('[');
    let chart = if has_chords {
        let mut c = chords::parse_chart(&chordpro);
        c.title = title.clone();
        c.ccli = ccli.clone();
        c.copyright = copyright.clone();
        Some(c)
    } else {
        None
    };
    let song = b.finish(title, author, ccli, copyright, order, &mut notes);
    Ok(Imported { song, chart, format: Format::OpenLyrics, notes })
}

// ---------------------------------------------------------------- OpenSong

/// Remove every `ch` from the words and slide the chord positions to match.
fn drop_char(line: &mut chords::ChartLine, ch: char) {
    let mut removed_before = 0usize;
    let mut new_text = String::new();
    let mut map: Vec<usize> = Vec::new(); // old index -> new index
    for c in line.text.chars() {
        map.push(new_text.chars().count());
        if c == ch {
            removed_before += 1;
        } else {
            new_text.push(c);
        }
    }
    if removed_before == 0 {
        return;
    }
    let end = new_text.chars().count();
    for (p, _) in &mut line.chords {
        *p = map.get(*p).copied().unwrap_or(end).min(end);
    }
    line.text = new_text;
}

fn open_song(file_name: &str, text: &str) -> Result<Imported, String> {
    let root = xml::parse(text).map_err(|e| format!("{file_name}: not readable as XML ({e})"))?;
    let get = |n: &str| root.child(n).map(|c| c.text().trim().to_string()).unwrap_or_default();
    let title = Some(get("title")).filter(|t| !t.is_empty()).unwrap_or_else(|| title_from_file(file_name));
    let mut notes = Vec::new();
    let lyrics = root.child("lyrics").map(|l| l.text()).unwrap_or_default();
    if lyrics.trim().is_empty() {
        return Err(format!("{file_name}: the song has no lyrics"));
    }
    let mut b = SectionBuilder::default();
    let mut chart = chords::ChordChart { title: title.clone(), key: Some(get("key")).filter(|k| !k.is_empty()), ..Default::default() };
    chart.ccli = get("ccli");
    chart.copyright = get("copyright");
    let mut cur_label = String::new();
    let mut cur_lines: Vec<String> = Vec::new();
    let mut cur_chart: Vec<chords::ChartLine> = Vec::new();
    let mut pending_chords: Option<String> = None;
    let mut any_chords = false;
    let flush = |label: &mut String, lines: &mut Vec<String>, clines: &mut Vec<chords::ChartLine>, b: &mut SectionBuilder, chart: &mut chords::ChordChart| {
        if !lines.is_empty() {
            let l = if label.is_empty() { format!("Verse {}", b.sections.len() + 1) } else { label.clone() };
            b.add(&l, &lines.join("\n"), false);
            chart.sections.push(chords::ChartSection { label: canon_label(&l), lines: std::mem::take(clines) });
        }
        lines.clear();
        clines.clear();
    };
    let push_line = |lyric: &str, chord_line: Option<String>, lines: &mut Vec<String>, clines: &mut Vec<chords::ChartLine>, any: &mut bool| {
        let mut cl = match chord_line {
            Some(c) => {
                *any = true;
                chords::merge_chord_line(&c, lyric)
            }
            None => chords::ChartLine { text: lyric.to_string(), chords: Vec::new() },
        };
        drop_char(&mut cl, '_');
        let words = cl.text.trim_end().to_string();
        if !words.trim().is_empty() {
            lines.push(words);
        }
        clines.push(cl);
    };
    for raw in lyrics.replace("\r\n", "\n").lines() {
        let t = raw.trim();
        if t.starts_with('[') && t.ends_with(']') && t.len() > 2 {
            if let Some(c) = pending_chords.take() {
                push_line("", Some(c), &mut cur_lines, &mut cur_chart, &mut any_chords);
            }
            flush(&mut cur_label, &mut cur_lines, &mut cur_chart, &mut b, &mut chart);
            cur_label = canon_label(&t[1..t.len() - 1]);
            continue;
        }
        if t.starts_with(';') || t == "---" || t.is_empty() {
            continue;
        }
        if let Some(c) = raw.strip_prefix('.') {
            if let Some(prev) = pending_chords.replace(c.to_string()) {
                push_line("", Some(prev), &mut cur_lines, &mut cur_chart, &mut any_chords);
            }
            continue;
        }
        let lyric = raw.strip_prefix(' ').unwrap_or(raw);
        let lyric = if lyric.chars().next().map_or(false, |c| c.is_ascii_digit()) && lyric.chars().nth(1) == Some(' ') { &lyric[2..] } else { lyric };
        push_line(lyric, pending_chords.take(), &mut cur_lines, &mut cur_chart, &mut any_chords);
    }
    if let Some(c) = pending_chords.take() {
        push_line("", Some(c), &mut cur_lines, &mut cur_chart, &mut any_chords);
    }
    flush(&mut cur_label, &mut cur_lines, &mut cur_chart, &mut b, &mut chart);
    if b.sections.is_empty() {
        return Err(format!("{file_name}: the song has no words"));
    }
    let order = Some(get("presentation").split_whitespace().map(|s| s.to_string()).collect::<Vec<_>>()).filter(|o| !o.is_empty());
    if chart.key.is_none() {
        chart.key = chords::detect_key(&chart);
    }
    let song = b.finish(title, get("author"), get("ccli"), get("copyright"), order, &mut notes);
    Ok(Imported { song, chart: if any_chords { Some(chart) } else { None }, format: Format::OpenSong, notes })
}

// ---------------------------------------------------------------- ChordPro and chords over words

fn from_chart(file_name: &str, text: &str, format: Format) -> Result<Imported, String> {
    let mut chart = chords::parse_chart(text);
    if chart.sections.is_empty() {
        return Err(format!("{file_name}: no words found"));
    }
    let title = Some(chart.title.clone()).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| title_from_file(file_name));
    chart.title = title.clone();
    let mut b = SectionBuilder::default();
    for s in &chart.sections {
        let words = s.lines.iter().map(|l| l.text.trim_end().to_string()).filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join("\n");
        if !words.is_empty() {
            b.add(&s.label, &words, false);
        }
    }
    if b.sections.is_empty() {
        return Err(format!("{file_name}: the chart has chords but no words"));
    }
    let mut notes = Vec::new();
    let has_chords = !chords::all_chords(&chart).is_empty();
    let song = b.finish(title, chart.artist.clone(), chart.ccli.clone(), chart.copyright.clone(), None, &mut notes);
    Ok(Imported { song, chart: if has_chords { Some(chart) } else { None }, format, notes })
}

// ---------------------------------------------------------------- export

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn verse_code(label: &str) -> String {
    let c = canon_label(label);
    let (word, num) = match c.rsplit_once(' ') {
        Some((w, n)) if n.chars().all(|ch| ch.is_ascii_digit()) => (w.to_string(), n.to_string()),
        _ => (c.clone(), String::new()),
    };
    let letter = match word.as_str() {
        "Verse" => "v",
        "Chorus" => "c",
        "Pre-Chorus" => "p",
        "Bridge" => "b",
        "Intro" => "i",
        "Ending" | "Outro" | "Tag" => "e",
        _ => "o",
    };
    let num = if num.is_empty() && letter == "o" { "1".to_string() } else { num };
    format!("{letter}{num}")
}

/// The song as an OpenLyrics file (readable by many programs). Part names are made unique (`v1`, `c1`, `o1`...).
pub fn to_openlyrics(song: &Song) -> String {
    let mut codes: Vec<String> = Vec::new();
    for s in &song.sections {
        let mut code = verse_code(&s.label);
        let mut n: u32 = code[1..].parse().unwrap_or(1);
        while codes.contains(&code) {
            n += 1;
            code = format!("{}{}", &code[..1], n);
        }
        codes.push(code);
    }
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<song xmlns=\"http://openlyrics.info/namespace/2009/song\" version=\"0.8\">\n  <properties>\n");
    out.push_str(&format!("    <titles><title>{}</title></titles>\n", esc(&song.title)));
    if !song.author.is_empty() {
        out.push_str(&format!("    <authors><author>{}</author></authors>\n", esc(&song.author)));
    }
    if !song.copyright.is_empty() {
        out.push_str(&format!("    <copyright>{}</copyright>\n", esc(&song.copyright)));
    }
    if !song.ccli_song.is_empty() {
        out.push_str(&format!("    <ccliNo>{}</ccliNo>\n", esc(&song.ccli_song)));
    }
    if !song.arrangement.is_empty() {
        let order: Vec<&str> = song.arrangement.iter().filter_map(|a| song.sections.iter().position(|s| label_key(&s.label) == label_key(a)).map(|i| codes[i].as_str())).collect();
        if !order.is_empty() {
            out.push_str(&format!("    <verseOrder>{}</verseOrder>\n", order.join(" ")));
        }
    }
    out.push_str("  </properties>\n  <lyrics>\n");
    for (s, code) in song.sections.iter().zip(&codes) {
        let lines = s.text.lines().map(esc).collect::<Vec<_>>().join("<br/>");
        out.push_str(&format!("    <verse name=\"{code}\"><lines>{lines}</lines></verse>\n"));
    }
    out.push_str("  </lyrics>\n</song>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const OL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<song xmlns="http://openlyrics.info/namespace/2009/song" version="0.8">
  <properties>
    <titles><title>Amazing Grace</title></titles>
    <authors><author>John Newton</author><author>Edwin Excell</author></authors>
    <copyright>Public domain</copyright><ccliNo>22025</ccliNo>
    <verseOrder>v1 c v2 c</verseOrder>
  </properties>
  <lyrics>
    <verse name="v1"><lines>Amazing grace how <chord name="G"/>sweet the sound<br/>That saved a wretch like me</lines></verse>
    <verse name="c"><lines>Chorus line one<br/>Chorus line two</lines></verse>
    <verse name="v2"><lines>
        'Twas grace that taught<br/>
        my heart to fear
    </lines></verse>
    <verse name="v1" lang="es"><lines>Sublime gracia</lines></verse>
  </lyrics>
</song>"#;

    const OS: &str = "<song><title>How Great</title><author>Chris</author><copyright>2000</copyright><ccli>999</ccli><key>G</key><presentation>V1 C V1 C</presentation><lyrics>[V1]\n.G        C\n Then sings my soul\n ;a comment\n.D\n My Sa_vior God\n[C]\n .Em\n How great Thou art\n</lyrics></song>";

    #[test]
    fn openlyrics_is_read_in_full() {
        let i = import("grace.xml", OL).unwrap();
        assert_eq!(i.format, Format::OpenLyrics);
        let s = &i.song;
        assert_eq!(s.title, "Amazing Grace");
        assert_eq!(s.author, "John Newton, Edwin Excell");
        assert_eq!(s.ccli_song, "22025");
        assert_eq!(s.copyright, "Public domain");
        assert_eq!(s.sections.iter().map(|x| x.label.as_str()).collect::<Vec<_>>(), vec!["Verse 1", "Chorus", "Verse 2"]);
        assert_eq!(s.sections[0].text, "Amazing grace how sweet the sound\nThat saved a wretch like me");
        assert_eq!(s.sections[2].text, "'Twas grace that taught\nmy heart to fear");
        assert_eq!(s.arrangement, vec!["Verse 1", "Chorus", "Verse 2", "Chorus"]);
        assert!(i.notes.iter().any(|n| n.contains("another language")));
        let chart = i.chart.unwrap();
        assert_eq!(chart.sections[0].lines[0].chords[0].1.root, 7);
        assert_eq!(chart.sections[0].lines[0].chords[0].0, 18); // after "Amazing grace how "
    }

    #[test]
    fn opensong_chords_underscores_comments_and_order() {
        let i = import("howgreat", OS).unwrap();
        assert_eq!(i.format, Format::OpenSong);
        let s = &i.song;
        assert_eq!(s.title, "How Great");
        assert_eq!(s.ccli_song, "999");
        assert_eq!(s.sections[0].label, "Verse 1");
        assert_eq!(s.sections[0].text, "Then sings my soul\nMy Savior God");
        assert_eq!(s.sections[1].label, "Chorus");
        assert_eq!(s.arrangement, vec!["Verse 1", "Chorus", "Verse 1", "Chorus"]);
        let c = i.chart.unwrap();
        assert_eq!(c.key.as_deref(), Some("G"));
        assert_eq!(c.sections[0].lines[0].chords.len(), 2);
        assert_eq!(c.sections[0].lines[0].chords[1].0, 9);
        assert_eq!(c.sections[0].lines[1].text, "My Savior God");
    }

    #[test]
    fn chordpro_repeated_parts_become_one_part_and_an_order() {
        let t = "{title: Test}\n{comment: Verse 1}\n[G]one two\n{comment: Chorus}\n[C]sing it\n{comment: Verse 2}\n[G]three four\n{comment: Chorus}\n[C]sing it\n";
        let i = import("test.cho", t).unwrap();
        assert_eq!(i.format, Format::ChordPro);
        assert_eq!(i.song.sections.len(), 3);
        assert_eq!(i.song.arrangement, vec!["Verse 1", "Chorus", "Verse 2", "Chorus"]);
        assert_eq!(i.song.title, "Test");
        assert!(i.chart.is_some());
    }

    #[test]
    fn two_different_choruses_do_not_overwrite_each_other() {
        let t = "{title: T}\n{comment: Chorus}\nfirst\n{comment: Chorus}\nsecond\n";
        let i = import("t.cho", t).unwrap();
        assert_eq!(i.song.sections.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), vec!["Chorus", "Chorus 2"]);
    }

    #[test]
    fn chords_over_words_are_detected_and_the_chords_removed_from_the_words() {
        let t = "Verse 1:\nG       C\nAmazing grace\nD\nhow sweet\n";
        assert_eq!(detect("x.txt", t), Format::ChordsOverWords);
        let i = import("x.txt", t).unwrap();
        assert_eq!(i.song.sections[0].text, "Amazing grace\nhow sweet");
        assert_eq!(i.song.title, "x");
    }

    #[test]
    fn plain_lyrics_use_the_file_name_for_the_title() {
        let i = import("/songs/Great_Is_Thy_Faithfulness.txt", "Verse 1:\nGreat is thy faithfulness\nO God my Father\n\nChorus:\nGreat is thy faithfulness\nMorning by morning\n").unwrap();
        assert_eq!(i.format, Format::PlainText);
        assert_eq!(i.song.title, "Great Is Thy Faithfulness");
        assert_eq!(i.song.sections.len(), 2);
    }

    #[test]
    fn bad_files_give_a_clear_message_not_a_crash() {
        assert!(import("a.txt", "").unwrap_err().contains("empty"));
        assert!(import("a.xml", "<recipe><x/></recipe>").unwrap_err().contains("not a song"));
        assert!(import("a.xml", "<song><lyrics>  </lyrics></song>").unwrap_err().contains("no lyrics"));
        assert!(import("a.xml", "<song xmlns=\"http://openlyrics.info/namespace/2009/song\"><lyrics><verse name=\"v1\"><lines/></verse></lyrics></song>").is_err());
        assert!(import("big.txt", &"la ".repeat(1_000_000)).unwrap_err().contains("2 MB"));
    }

    #[test]
    fn import_many_separates_good_from_bad() {
        let files = vec![("a.txt".to_string(), "One:\nline\nline2".to_string()), ("b.xml".to_string(), "<x/>".to_string()), ("c.cho".to_string(), "{title: C}\n[G]hi there".to_string())];
        let (ok, bad) = import_many(&files);
        assert_eq!(ok.len(), 2);
        assert_eq!(bad.len(), 1);
    }

    #[test]
    fn titles_are_made_unique() {
        let ex = vec!["Amazing Grace".to_string(), "amazing grace (2)".to_string()];
        assert_eq!(unique_title(&ex, "Amazing Grace"), "Amazing Grace (3)");
        assert_eq!(unique_title(&ex, "Other"), "Other");
    }

    #[test]
    fn openlyrics_export_reads_back_the_same_song() {
        let i = import("grace.xml", OL).unwrap();
        let xml = to_openlyrics(&i.song);
        let back = import("out.xml", &xml).unwrap();
        assert_eq!(back.song.title, i.song.title);
        assert_eq!(back.song.author, "John Newton, Edwin Excell");
        assert_eq!(back.song.sections, i.song.sections);
        assert_eq!(back.song.arrangement, i.song.arrangement);
        assert_eq!(back.song.ccli_song, "22025");
    }

    #[test]
    fn export_escapes_special_characters() {
        let s = Song { id: 0, title: "Rock & <Roll>".into(), author: String::new(), ccli_song: String::new(), copyright: String::new(), sections: vec![Section { label: "Verse 1".into(), text: "a < b & c".into() }], arrangement: vec![] };
        let back = import("x.xml", &to_openlyrics(&s)).unwrap();
        assert_eq!(back.song.title, "Rock & <Roll>");
        assert_eq!(back.song.sections[0].text, "a < b & c");
    }

    #[test]
    fn export_with_two_bridges_keeps_unique_codes() {
        let sec = |l: &str, t: &str| Section { label: l.into(), text: t.into() };
        let s = Song { id: 0, title: "T".into(), author: String::new(), ccli_song: String::new(), copyright: String::new(), sections: vec![sec("Amen", "x"), sec("Coda", "y")], arrangement: vec![] };
        let xml = to_openlyrics(&s);
        assert!(xml.contains("name=\"o1\"") && xml.contains("name=\"o2\""), "{xml}");
    }
}
