//! Slides: announcements and song words (Stage 2, spec section 1).
//!
//! Plain text in, slides out. A blank line starts a new slide; a first line that ends in a colon
//! labels the slide. Also reads words out of `.txt` and `.pptx` files so a church can bring
//! what it already has. Pure functions, no UI, no clock.

use std::io::Read;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedSlide {
    /// "Chorus", "Verse 2"... or empty. Shown to the operator, never to the congregation.
    pub label: String,
    pub text: String,
}

/// Longest label (in characters, before the colon) that still counts as a label.
const MAX_LABEL_CHARS: usize = 30;

fn normalise_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Spec S1.1 to S1.3.
pub fn slides_from_text(text: &str) -> Vec<ParsedSlide> {
    let norm = normalise_newlines(text);
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for line in norm.split('\n') {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                blocks.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(line);
        }
    }
    if !cur.is_empty() {
        blocks.push(cur);
    }
    blocks
        .into_iter()
        .map(|mut lines| {
            let mut label = String::new();
            if lines.len() > 1 {
                if let Some(l) = label_of(lines[0]) {
                    label = l;
                    lines.remove(0);
                }
            }
            ParsedSlide { label, text: lines.join("\n").trim().to_string() }
        })
        .collect()
}

/// S1.2: `Chorus:` style first line. 1 to 30 characters, no other colon.
fn label_of(first_line: &str) -> Option<String> {
    let t = first_line.trim();
    let before = t.strip_suffix(':')?;
    let n = before.chars().count();
    if n == 0 || n > MAX_LABEL_CHARS || before.contains(':') {
        return None;
    }
    let label = before.trim();
    if label.is_empty() {
        None
    } else {
        Some(label.to_string())
    }
}

/// S1.7: a text file's bytes as text. Never fails.
pub fn text_from_bytes(bytes: &[u8]) -> String {
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    normalise_newlines(&String::from_utf8_lossy(b))
}

/// S1.7: the words of a PowerPoint file, one slide per slide (in slide-number order), one line
/// per paragraph, slides separated by a blank line (so `slides_from_text` turns it into slides).
pub fn text_from_pptx(bytes: &[u8]) -> Result<String, String> {
    const NOT_PPTX: &str = "This does not look like a PowerPoint (.pptx) file. Open it in PowerPoint or Keynote and use Save As / Export > .pptx, or copy the words into the Slides box.";
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| NOT_PPTX.to_string())?;
    let mut found: Vec<(u32, String)> = Vec::new();
    for i in 0..zip.len() {
        let Ok(f) = zip.by_index(i) else { continue };
        let name = f.name().to_string();
        if let Some(n) = name.strip_prefix("ppt/slides/slide").and_then(|r| r.strip_suffix(".xml")).and_then(|d| d.parse::<u32>().ok()) {
            found.push((n, name));
        }
    }
    if found.is_empty() {
        return Err(NOT_PPTX.to_string());
    }
    found.sort();
    let mut slides: Vec<String> = Vec::new();
    for (_, name) in found {
        let mut xml = String::new();
        let f = zip.by_name(&name).map_err(|_| NOT_PPTX.to_string())?;
        // limit what we are willing to read from one slide (a hostile file must not eat memory)
        if f.take(8 * 1024 * 1024).read_to_string(&mut xml).is_err() {
            continue;
        }
        let words = words_of_slide_xml(&xml);
        if !words.is_empty() {
            slides.push(words);
        }
    }
    if slides.is_empty() {
        return Err("This PowerPoint file has no words on its slides.".to_string());
    }
    Ok(slides.join("\n\n"))
}

/// The paragraphs (`<a:p>`) of a slide, each made of its text runs (`<a:t>`). Empty paragraphs
/// are dropped: a blank line would otherwise split the slide in two.
fn words_of_slide_xml(xml: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut para = String::new();
    let mut i = 0;
    while let Some(rel) = xml[i..].find('<') {
        let start = i + rel;
        let Some(len) = xml[start..].find('>') else { break };
        let end = start + len;
        let tag = &xml[start + 1..end];
        i = end + 1;
        let closing = tag.starts_with('/');
        let self_closing = tag.ends_with('/');
        let name = tag.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
        match (closing, name) {
            (false, "a:p") if !self_closing => para.clear(),
            (true, "a:p") => {
                push_para(&mut lines, &para);
                para.clear();
            }
            (false, "a:br") => para.push('\n'),
            (false, "a:t") if !self_closing => {
                if let Some(close) = xml[i..].find("</a:t>") {
                    para.push_str(&decode_entities(&xml[i..i + close]));
                    i += close + "</a:t>".len();
                }
            }
            _ => {}
        }
    }
    lines.join("\n")
}

fn push_para(lines: &mut Vec<String>, para: &str) {
    for l in para.split('\n') {
        let l = l.trim_end();
        if !l.trim().is_empty() {
            lines.push(l.trim_start().to_string());
        }
    }
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        let after = &rest[p + 1..];
        match after.find(';').filter(|&e| e <= 10) {
            Some(e) => {
                let ent = &after[..e];
                let rep = match ent {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    _ => ent
                        .strip_prefix("#x")
                        .or_else(|| ent.strip_prefix("#X"))
                        .and_then(|h| u32::from_str_radix(h, 16).ok())
                        .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse::<u32>().ok()))
                        .and_then(char::from_u32),
                };
                match rep {
                    Some(c) => out.push(c),
                    None => {
                        out.push('&');
                        out.push_str(ent);
                        out.push(';');
                    }
                }
                rest = &after[e + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}
