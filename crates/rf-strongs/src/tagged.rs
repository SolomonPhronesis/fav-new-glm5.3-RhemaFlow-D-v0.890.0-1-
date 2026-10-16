//! A Bible whose words carry Strong's numbers, read from OSIS XML (the form the CrossWire KJV
//! module exports). Way 1 of the Strong's feature: click a word, get its exact original word.
//!
//! Understood:
//!   <verse osisID="John.1.1"> ... <w lemma="strong:G3056 lemma.TR:λόγος" morph="robinson:N-NSM">Word</w> ... </verse>
//!   <verse sID="Gen.1.1" osisID="Gen.1.1"/> ... <w lemma="strong:H07225">beginning</w> ... <verse eID="Gen.1.1"/>
//! Numbers are tidied ("H07225" -> "H7225"). Other markup inside a verse (notes, italics) is skipped.

use std::collections::HashMap;

/// One word of a verse. `strongs` is empty for words the module did not tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaggedWord {
    pub text: String,
    pub strongs: Vec<String>,
}

#[derive(Default, Debug)]
pub struct TaggedBible {
    verses: HashMap<(usize, usize, usize), Vec<TaggedWord>>,
}

/// OSIS book ids in Bible order (index = our book index).
const OSIS_BOOKS: [&str; 66] = [
    "Gen", "Exod", "Lev", "Num", "Deut", "Josh", "Judg", "Ruth", "1Sam", "2Sam", "1Kgs", "2Kgs", "1Chr", "2Chr", "Ezra", "Neh", "Esth", "Job", "Ps", "Prov", "Eccl", "Song", "Isa", "Jer", "Lam", "Ezek", "Dan",
    "Hos", "Joel", "Amos", "Obad", "Jonah", "Mic", "Nah", "Hab", "Zeph", "Hag", "Zech", "Mal", "Matt", "Mark", "Luke", "John", "Acts", "Rom", "1Cor", "2Cor", "Gal", "Eph", "Phil", "Col", "1Thess", "2Thess",
    "1Tim", "2Tim", "Titus", "Phlm", "Heb", "Jas", "1Pet", "2Pet", "1John", "2John", "3John", "Jude", "Rev",
];

fn parse_osis_id(id: &str) -> Option<(usize, usize, usize)> {
    let first = id.split(['-', ' ']).next()?;
    let mut p = first.split('.');
    let name = p.next()?;
    let book = OSIS_BOOKS.iter().position(|b| b.eq_ignore_ascii_case(name))?;
    let ch: usize = p.next()?.parse().ok()?;
    let v: usize = p.next()?.parse().ok()?;
    (ch >= 1 && v >= 1).then_some((book, ch, v))
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    for q in ['"', '\''] {
        let key = format!("{name}={q}");
        if let Some(i) = tag.find(&key) {
            // make sure it is a whole attribute name, not the end of another
            if i == 0 || tag.as_bytes()[i - 1].is_ascii_whitespace() {
                let rest = &tag[i + key.len()..];
                return rest.find(q).map(|e| &rest[..e]);
            }
        }
    }
    None
}

fn decode(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn tidy(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric()).to_string()
}

/// "strong:G3056 lemma.TR:λόγος" -> ["G3056"]
fn numbers_in_lemma(lemma: &str) -> Vec<String> {
    lemma.split_whitespace().filter_map(|p| p.strip_prefix("strong:")).filter_map(crate::normalize_number).filter(|n| n.starts_with(['G', 'H'])).collect()
}

/// Strip any tags from a piece of text, keep the words.
fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut depth = false;
    for c in s.chars() {
        match c {
            '<' => depth = true,
            '>' => depth = false,
            c if !depth => out.push(c),
            _ => {}
        }
    }
    out
}

impl TaggedBible {
    pub fn from_osis(xml: &str) -> Result<TaggedBible, String> {
        if !xml.contains("<osis") {
            return Err("This does not look like an OSIS Bible file (no <osis> found). Strong's-tagged Bibles in OSIS form can be exported from a SWORD module (see docs/STRONGS.md).".to_string());
        }
        let mut verses: HashMap<(usize, usize, usize), Vec<TaggedWord>> = HashMap::new();
        let mut cur: Option<(usize, usize, usize)> = None;
        let mut i = 0;
        let mut tagged_seen = false;
        let push_plain = |verses: &mut HashMap<_, Vec<TaggedWord>>, key: (usize, usize, usize), text: &str| {
            for w in decode(&strip_tags(text)).split_whitespace() {
                let t = tidy(w);
                if !t.is_empty() {
                    verses.entry(key).or_default().push(TaggedWord { text: t, strongs: Vec::new() });
                }
            }
        };
        while i < xml.len() {
            let Some(lt) = xml[i..].find('<') else {
                if let Some(k) = cur {
                    push_plain(&mut verses, k, &xml[i..]);
                }
                break;
            };
            if lt > 0 {
                if let Some(k) = cur {
                    push_plain(&mut verses, k, &xml[i..i + lt]);
                }
            }
            let start = i + lt;
            let Some(gt) = xml[start..].find('>') else { break };
            let tag = &xml[start + 1..start + gt];
            let after = start + gt + 1;
            let name = tag.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
            let closing = tag.starts_with('/');
            let selfclose = tag.ends_with('/');
            i = after;
            match (name, closing) {
                ("verse", false) => {
                    if attr(tag, "eID").is_some() {
                        cur = None;
                    } else if let Some(k) = attr(tag, "osisID").and_then(parse_osis_id) {
                        cur = Some(k);
                        if !selfclose || attr(tag, "sID").is_some() {
                            verses.entry(k).or_default();
                        }
                    } else {
                        cur = None; // a verse we cannot place (unknown book)
                    }
                }
                ("verse", true) => cur = None,
                ("w", false) if !selfclose => {
                    let end = xml[after..].find("</w>").map(|e| after + e).unwrap_or(xml.len());
                    let inner = tidy(&decode(&strip_tags(&xml[after..end])));
                    if let (Some(k), false) = (cur, inner.is_empty()) {
                        let nums = attr(tag, "lemma").map(numbers_in_lemma).unwrap_or_default();
                        tagged_seen |= !nums.is_empty();
                        verses.entry(k).or_default().push(TaggedWord { text: inner, strongs: nums });
                    }
                    i = (end + "</w>".len()).min(xml.len());
                }
                _ => {}
            }
        }
        verses.retain(|_, w| !w.is_empty());
        if verses.is_empty() {
            return Err("No verses were found in this file (is it a Bible in OSIS form?).".to_string());
        }
        if !tagged_seen {
            return Err("This Bible has no Strong's numbers in it (it has verses, but no tagged words). A tagged module is needed for word-by-word lookup.".to_string());
        }
        Ok(TaggedBible { verses })
    }

    /// Number of verses read.
    pub fn len(&self) -> usize {
        self.verses.len()
    }
    pub fn is_empty(&self) -> bool {
        self.verses.is_empty()
    }

    /// 0-based book index, 1-based chapter and verse (as everywhere in RhemaFlow).
    pub fn verse(&self, book: usize, chapter: usize, verse: usize) -> Option<&[TaggedWord]> {
        self.verses.get(&(book, chapter, verse)).map(Vec::as_slice)
    }

    /// Every Strong's number in a verse, in order, each once.
    pub fn numbers_in(&self, book: usize, chapter: usize, verse: usize) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for w in self.verse(book, chapter, verse).unwrap_or(&[]) {
            for n in &w.strongs {
                if !out.contains(n) {
                    out.push(n.clone());
                }
            }
        }
        out
    }
}
