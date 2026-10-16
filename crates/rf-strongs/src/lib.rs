//! rf-strongs — Strong's numbers for the operator (v0.816.0). No UI, no network.
//!
//! * `Lexicon`: the Greek dictionary is built in (CC0, from James Strong's 1890 Exhaustive
//!   Concordance, prepared by Ulrik Sandborg-Petersen). A Hebrew file can be added with `merge`.
//! * `search_english("word", 'G', 5)`: "what is the Greek for word": logos, rhema.
//!   It reads each entry's own "KJV rendering" list, so it needs no tagged Bible; it gives
//!   candidates, not the exact word in one particular verse (that is `TaggedBible`'s job).
//! * `TaggedBible`: reads an OSIS Bible whose words carry Strong's numbers (the format of the
//!   CrossWire KJV module) so each word of the verse on screen can be looked up exactly.

mod tagged;

use serde::Deserialize;
use std::collections::HashMap;

pub use tagged::{TaggedBible, TaggedWord};

const GREEK_JSON: &str = include_str!("../data/greek.json");
const SCHEMA: u32 = 1;

/// One dictionary entry (Greek or Hebrew).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// "G3056" / "H7965"
    pub number: String,
    /// the word in its own script: λόγος
    pub lemma: String,
    /// letters we can read: lógos
    pub translit: String,
    /// how to say it: log'-os
    pub pron: String,
    pub derivation: String,
    /// Strong's meaning
    pub definition: String,
    /// how the King James Version renders it: "account, cause, ... word, work"
    pub kjv: String,
}

#[derive(Deserialize)]
struct RawEntry {
    #[serde(default)]
    l: String,
    #[serde(default, alias = "x")]
    t: String,
    #[serde(default)]
    p: String,
    #[serde(default)]
    v: String,
    #[serde(default)]
    d: String,
    #[serde(default)]
    k: String,
}

#[derive(Deserialize)]
struct Doc {
    schema: u32,
    language: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    license: String,
    entries: HashMap<String, RawEntry>,
}

#[derive(Default, Debug)]
pub struct Lexicon {
    entries: Vec<Entry>,
    index: HashMap<String, usize>,
    licenses: Vec<String>,
}

mod fold_table;

/// Lowercase ASCII letters from accented ones: agápē -> agape, ʼĕlôhîym -> elohiym, eirḗnē -> eirene.
/// Letters that are not plain Latin letters with accents (Greek, Hebrew, the glottal marks ʼ ʻ) are dropped.
pub fn ascii_fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() || c == ' ' {
            out.push(c);
        } else if let Some(i) = fold_table::FROM.chars().position(|f| f == c) {
            if let Some(t) = fold_table::TO.chars().nth(i) {
                out.push(t);
            }
        }
    }
    out
}

/// "G03056" -> "G3056", "strong:H07225" -> "H7225", "  h 430 " -> "H430", "3056" -> "3056". None if not a number.
pub fn normalize_number(s: &str) -> Option<String> {
    let t: String = s.trim().trim_start_matches("strong:").chars().filter(|c| !c.is_whitespace()).collect();
    let (lang, digits) = match t.chars().next()? {
        c @ ('G' | 'g' | 'H' | 'h') => (Some(c.to_ascii_uppercase()), &t[1..]),
        _ => (None, t.as_str()),
    };
    if digits.is_empty() || digits.len() > 5 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n = digits.trim_start_matches('0');
    if n.is_empty() {
        return None;
    }
    Some(match lang {
        Some(l) => format!("{l}{n}"),
        None => n.to_string(),
    })
}

fn words_of(s: &str) -> Vec<String> {
    s.chars().map(|c| if c.is_alphabetic() { c.to_ascii_lowercase() } else { ' ' }).collect::<String>().split_whitespace().map(String::from).collect()
}

/// word, word-without-ending, ...: loved -> loved, love; believing -> believe; spirits -> spirit.
fn variants(w: &str) -> Vec<String> {
    let mut v = vec![w.to_string()];
    for suf in ["ing", "ed", "es", "s", "d", "ly"] {
        if let Some(b) = w.strip_suffix(suf) {
            if b.len() >= 3 {
                v.push(b.to_string());
                if suf == "ing" || suf == "ed" {
                    v.push(format!("{b}e"));
                }
            }
        }
    }
    v.dedup();
    v
}

fn number_order(n: &str) -> (u8, u32) {
    (if n.starts_with('G') { 0 } else { 1 }, n[1..].parse().unwrap_or(0))
}

impl Lexicon {
    /// The built-in Greek dictionary (5,624 entries).
    pub fn greek() -> Lexicon {
        Lexicon::from_json(GREEK_JSON).expect("the built-in Greek dictionary is valid")
    }

    /// Read a lexicon file in RhemaFlow's small JSON form (see tools/make_strongs_greek.py).
    pub fn from_json(json: &str) -> Result<Lexicon, String> {
        let doc: Doc = serde_json::from_str(json).map_err(|e| format!("This is not a Strong's lexicon file ({e})."))?;
        if doc.schema != SCHEMA {
            return Err(format!("This Strong's file is a different version ({}) from the one this app understands ({SCHEMA}).", doc.schema));
        }
        let mut entries: Vec<Entry> = doc
            .entries
            .into_iter()
            .map(|(number, r)| Entry { number, lemma: r.l, translit: r.t, pron: r.p, derivation: r.v, definition: r.d, kjv: r.k })
            .collect();
        entries.sort_by_key(|e| number_order(&e.number));
        let mut lex = Lexicon { entries, index: HashMap::new(), licenses: vec![format!("{}: {} ({})", doc.language, doc.source, doc.license)] };
        lex.reindex();
        Ok(lex)
    }

    fn reindex(&mut self) {
        self.index = self.entries.iter().enumerate().map(|(i, e)| (e.number.clone(), i)).collect();
    }

    /// Add another lexicon (for example Hebrew). Same number: the existing entry stays.
    pub fn merge(&mut self, other: Lexicon) {
        for e in other.entries {
            if !self.index.contains_key(&e.number) {
                self.entries.push(e);
            }
        }
        self.licenses.extend(other.licenses);
        self.entries.sort_by_key(|e| number_order(&e.number));
        self.reindex();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn has_language(&self, lang: char) -> bool {
        self.entries.iter().any(|e| e.number.starts_with(lang))
    }
    /// Where the data came from and under what licence, for the About text.
    pub fn licenses(&self) -> &[String] {
        &self.licenses
    }

    /// By number in any spelling. A bare number ("3056") means Greek if there is one, else Hebrew.
    pub fn get(&self, number: &str) -> Option<&Entry> {
        let n = normalize_number(number)?;
        if n.starts_with(['G', 'H']) {
            return self.index.get(&n).map(|&i| &self.entries[i]);
        }
        self.index.get(&format!("G{n}")).or_else(|| self.index.get(&format!("H{n}"))).map(|&i| &self.entries[i])
    }

    /// Whatever was typed or said: a number, Greek or Hebrew letters, how it sounds, or an English word.
    pub fn search(&self, query: &str, max: usize) -> Vec<&Entry> {
        let q = query.trim();
        if q.is_empty() || max == 0 || q.chars().count() > 64 {
            return Vec::new();
        }
        if let Some(n) = normalize_number(q) {
            if n.starts_with(['G', 'H']) {
                return self.get(&n).into_iter().collect();
            }
            let mut v: Vec<&Entry> = ["G", "H"].iter().filter_map(|p| self.index.get(&format!("{p}{n}"))).map(|&i| &self.entries[i]).collect();
            v.truncate(max);
            return v;
        }
        let mut out: Vec<&Entry> = Vec::new();
        if q.chars().any(|c| !c.is_ascii() && c.is_alphabetic() && ascii_fold(&c.to_string()).is_empty()) {
            // Greek / Hebrew letters
            for e in &self.entries {
                if e.lemma == q || (e.lemma.starts_with(q) && q.chars().count() >= 2) {
                    out.push(e);
                }
            }
            out.sort_by_key(|e| (e.lemma != q, number_order(&e.number)));
            out.truncate(max);
            return out;
        }
        let f = ascii_fold(q);
        if !f.is_empty() {
            let mut exact: Vec<&Entry> = Vec::new();
            let mut prefix: Vec<&Entry> = Vec::new();
            for e in &self.entries {
                let t = ascii_fold(&e.translit);
                if t == f {
                    exact.push(e);
                } else if f.len() >= 4 && t.starts_with(&f) {
                    prefix.push(e);
                }
            }
            out.extend(exact);
            out.extend(prefix);
        }
        let first = words_of(q).into_iter().next().unwrap_or_default();
        for e in self.search_english(&first, '?', max) {
            if !out.iter().any(|x| x.number == e.number) {
                out.push(e);
            }
        }
        out.truncate(max);
        out
    }

    /// "What is the Greek (G), Hebrew (H) or either (?) for this English word": best first.
    pub fn search_english(&self, word: &str, lang: char, max: usize) -> Vec<&Entry> {
        let raw = word.trim().to_lowercase();
        if max == 0 || raw.is_empty() || raw.chars().count() > 40 {
            return Vec::new();
        }
        let w = match words_of(&raw).into_iter().next() {
            Some(w) => w,
            None => return Vec::new(),
        };
        let vars = variants(&w);
        // first picks (curated): the words a church asks for most
        let mut boost: HashMap<&str, i32> = HashMap::new();
        for (english, picks) in CURATED {
            if vars.iter().any(|v| v == english) {
                for (i, (num, _)) in picks.iter().enumerate() {
                    boost.entry(num).or_insert(100_000 - i as i32 * 1_000);
                }
            }
        }
        let mut scored: Vec<(i32, &Entry)> = Vec::new();
        for e in &self.entries {
            if lang != '?' && !e.number.starts_with(lang) {
                continue;
            }
            let mut best = 0;
            for (vi, v) in vars.iter().enumerate() {
                let stem_penalty = if vi == 0 { 5 } else { 0 };
                for (ti, term) in e.kjv.split([',', ';']).enumerate() {
                    let toks = words_of(term);
                    let clean: Vec<&String> = toks.iter().filter(|t| !matches!(t.as_str(), "x" | "be" | "to" | "ed" | "r" | "s" | "ly" | "ing")).collect();
                    let sc = if clean.len() == 1 && clean[0] == v {
                        60
                    } else if toks.iter().any(|t| t == v) {
                        35
                    } else {
                        0
                    };
                    if sc > 0 {
                        best = best.max(sc + stem_penalty + if ti == 0 { 10 } else { 0 });
                    }
                }
                if words_of(&e.definition).iter().any(|t| t == v) {
                    best = best.max(8 + stem_penalty);
                }
            }
            let b = boost.get(e.number.as_str()).copied().unwrap_or(0);
            if best > 0 || b > 0 {
                scored.push((best + b, e));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(number_order(&a.1.number).cmp(&number_order(&b.1.number))));
        scored.into_iter().take(max).map(|s| s.1).collect()
    }
}

/// The words a church asks about most, with the Strong's numbers to show FIRST, best first, and the
/// letters-only spelling each should have (a test checks them against the dictionary).
/// Written by hand for this app; the Hebrew picks only appear when a Hebrew file is installed.
pub const CURATED: &[(&str, &[(&str, &str)])] = &[
    ("love", &[("G26", "agape"), ("G25", "agapao"), ("G5368", "phileo"), ("H157", "ahab")]),
    ("word", &[("G3056", "logos"), ("G4487", "rhema"), ("H1697", "dabar")]),
    ("spirit", &[("G4151", "pneuma"), ("H7307", "ruwach")]),
    ("faith", &[("G4102", "pistis")]),
    ("believe", &[("G4100", "pisteuo")]),
    ("grace", &[("G5485", "charis"), ("H2580", "chen")]),
    ("peace", &[("G1515", "eirene"), ("H7965", "shalowm")]),
    ("hope", &[("G1680", "elpis")]),
    ("joy", &[("G5479", "chara")]),
    ("truth", &[("G225", "aletheia"), ("H571", "emeth")]),
    ("life", &[("G2222", "zoe"), ("G5590", "psyche")]),
    ("soul", &[("G5590", "psyche"), ("H5315", "nephesh")]),
    ("light", &[("G5457", "phos"), ("H216", "owr")]),
    ("sin", &[("G266", "hamartia")]),
    ("salvation", &[("G4991", "soteria")]),
    ("righteousness", &[("G1343", "dikaiosyne")]),
    ("mercy", &[("G1656", "eleos"), ("H2617", "checed")]),
    ("glory", &[("G1391", "doxa")]),
    ("forgive", &[("G863", "aphiemi")]),
    ("repent", &[("G3340", "metanoeo")]),
    ("lord", &[("G2962", "kyrios")]),
    ("god", &[("G2316", "theos"), ("H430", "elohiym")]),
    ("jesus", &[("G2424", "iesous")]),
    ("christ", &[("G5547", "christos")]),
    ("heart", &[("G2588", "kardia"), ("H3820", "leb")]),
    ("church", &[("G1577", "ekklesia")]),
    ("gospel", &[("G2098", "euangelion")]),
    ("holy", &[("G40", "hagios")]),
    ("blood", &[("G129", "haima")]),
    ("cross", &[("G4716", "stauros")]),
    ("servant", &[("G1401", "doulos")]),
    ("prayer", &[("G4335", "proseuche")]),
    ("wisdom", &[("G4678", "sophia")]),
    ("power", &[("G1411", "dynamis")]),
    ("kingdom", &[("G932", "basileia")]),
    ("name", &[("G3686", "onoma")]),
    ("death", &[("G2288", "thanatos")]),
    ("new", &[("G2537", "kainos")]),
    ("creation", &[("G2937", "ktisis")]),
    ("creature", &[("G2937", "ktisis")]),
    ("world", &[("G2889", "kosmos")]),
    ("flesh", &[("G4561", "sarx")]),
    ("fear", &[("G5401", "phobos")]),
    ("angel", &[("G32", "angelos")]),
    ("baptize", &[("G907", "baptizo")]),
    ("commandment", &[("G1785", "entole")]),
    ("law", &[("G3551", "nomos")]),
    ("covenant", &[("G1242", "diatheke")]),
    ("amen", &[("G281", "amen")]),
];
