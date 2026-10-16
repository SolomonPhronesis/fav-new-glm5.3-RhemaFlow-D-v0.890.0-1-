//! Personal lexicon — learns how *this* speaker gets mis-heard.
//!
//! Every church, accent and microphone has its own recurring mistakes
//! ("Habakkuk" → "have a cook", "Thessalonians" → "thus alone ions"). A
//! general model can't know them; a few minutes of reading a known script
//! can. `learn` aligns what was meant against what the engine produced and
//! remembers the substitutions; `apply` repairs the same mistakes later.
//!
//! Safety rules (a wrong repair is worse than none):
//! * a pattern must be seen `min_count` times before it is ever applied;
//! * single common words are never learned (one bad pair must not turn every
//!   "the" into something else);
//! * if one heard phrase has competing repairs, the winner must clearly lead;
//! * applying is deterministic and works on whole words only.

use crate::wer::normalize_words;
use std::collections::HashMap;

const MAX_PHRASE: usize = 4;
const NEVER_LEARN_AS_TRIGGER: &[&str] = &[
    "the", "a", "an", "and", "of", "to", "in", "is", "it", "he", "she", "we", "i", "you", "that", "for", "on", "with", "as", "at", "by", "be", "was", "his", "her",
];

#[derive(Default, Clone, Debug)]
pub struct Lexicon {
    /// heard phrase → (meant phrase → times seen)
    map: HashMap<String, HashMap<String, u32>>,
    min_count: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Match,
    Sub,
    Del, // reference word the engine dropped
    Ins, // extra word the engine produced
}

/// Word-level alignment of reference vs hypothesis (minimum edit distance).
fn align(reference: &[String], hypothesis: &[String]) -> Vec<(Op, Option<usize>, Option<usize>)> {
    let (n, m) = (reference.len(), hypothesis.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..=n {
        d[i][0] = i;
    }
    for j in 0..=m {
        d[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = if reference[i - 1] == hypothesis[j - 1] { 0 } else { 1 };
            d[i][j] = (d[i - 1][j - 1] + cost).min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
        }
    }
    let (mut i, mut j) = (n, m);
    let mut ops = Vec::new();
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && reference[i - 1] == hypothesis[j - 1] && d[i][j] == d[i - 1][j - 1] {
            ops.push((Op::Match, Some(i - 1), Some(j - 1)));
            i -= 1;
            j -= 1;
        } else if i > 0 && j > 0 && d[i][j] == d[i - 1][j - 1] + 1 {
            ops.push((Op::Sub, Some(i - 1), Some(j - 1)));
            i -= 1;
            j -= 1;
        } else if i > 0 && d[i][j] == d[i - 1][j] + 1 {
            ops.push((Op::Del, Some(i - 1), None));
            i -= 1;
        } else {
            ops.push((Op::Ins, None, Some(j - 1)));
            j -= 1;
        }
    }
    ops.reverse();
    ops
}

impl Lexicon {
    pub fn new() -> Self {
        Lexicon { map: HashMap::new(), min_count: 2 }
    }

    pub fn with_min_count(mut self, n: u32) -> Self {
        self.min_count = n.max(1);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Learn from one (what was said, what the engine heard) pair.
    /// Returns how many substitution patterns were recorded.
    pub fn learn(&mut self, reference: &str, hypothesis: &str) -> usize {
        let r = normalize_words(reference);
        let h = normalize_words(hypothesis);
        let ops = align(&r, &h);
        let mut learned = 0;
        let mut k = 0;
        while k < ops.len() {
            if ops[k].0 == Op::Match {
                k += 1;
                continue;
            }
            let start = k;
            while k < ops.len() && ops[k].0 != Op::Match {
                k += 1;
            }
            let run = &ops[start..k];
            let heard: Vec<&str> = run.iter().filter_map(|(_, _, j)| j.map(|j| h[j].as_str())).collect();
            let meant: Vec<&str> = run.iter().filter_map(|(_, i, _)| i.map(|i| r[i].as_str())).collect();
            if heard.is_empty() || heard.len() > MAX_PHRASE || meant.len() > MAX_PHRASE {
                continue;
            }
            if heard.len() == 1 && NEVER_LEARN_AS_TRIGGER.contains(&heard[0]) {
                continue;
            }
            let (heard, meant) = (heard.join(" "), meant.join(" "));
            if heard == meant {
                continue;
            }
            *self.map.entry(heard).or_default().entry(meant).or_insert(0) += 1;
            learned += 1;
        }
        learned
    }

    /// The repair for `heard`, if it has earned one.
    fn repair(&self, heard: &str) -> Option<&str> {
        let options = self.map.get(heard)?;
        let mut ranked: Vec<(&String, &u32)> = options.iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let (best, &n) = ranked[0];
        let needed = if best.is_empty() { self.min_count.max(3) } else { self.min_count };
        if n < needed {
            return None;
        }
        if let Some((_, &second)) = ranked.get(1) {
            if n < second * 2 {
                return None; // not a clear winner
            }
        }
        Some(best.as_str())
    }

    /// Repair known mishearings. Output is normalised (lower-case words).
    pub fn apply(&self, text: &str) -> String {
        if self.map.is_empty() {
            return text.to_string();
        }
        let words = normalize_words(text);
        let mut out: Vec<String> = Vec::with_capacity(words.len());
        let mut i = 0;
        while i < words.len() {
            let mut hit = false;
            for len in (1..=MAX_PHRASE.min(words.len() - i)).rev() {
                let key = words[i..i + len].join(" ");
                if let Some(meant) = self.repair(&key) {
                    if !meant.is_empty() {
                        out.push(meant.to_string());
                    }
                    i += len;
                    hit = true;
                    break;
                }
            }
            if !hit {
                out.push(words[i].clone());
                i += 1;
            }
        }
        out.join(" ")
    }

    // ---- persistence: one plain-text line per pattern, easy to read and edit
    //      `count<TAB>heard<TAB>meant`

    pub fn to_tsv(&self) -> String {
        let mut lines: Vec<(String, String, u32)> = self
            .map
            .iter()
            .flat_map(|(h, m)| m.iter().map(move |(meant, n)| (h.clone(), meant.clone(), *n)))
            .collect();
        lines.sort();
        lines.iter().map(|(h, m, n)| format!("{n}\t{h}\t{m}\n")).collect()
    }

    pub fn from_tsv(s: &str) -> Self {
        let mut lex = Lexicon::new();
        for line in s.lines() {
            let mut parts = line.splitn(3, '\t');
            if let (Some(n), Some(h), Some(m)) = (parts.next(), parts.next(), parts.next()) {
                if let Ok(n) = n.trim().parse::<u32>() {
                    if !h.trim().is_empty() {
                        *lex.map.entry(h.trim().to_string()).or_default().entry(m.trim().to_string()).or_insert(0) += n;
                    }
                }
            }
        }
        lex
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mishearing_is_not_repaired_until_seen_twice() {
        let mut lex = Lexicon::new();
        lex.learn("turn to habakkuk chapter two", "turn to have a cook chapter two");
        assert_eq!(lex.apply("have a cook chapter two"), "have a cook chapter two", "one sighting is not enough");
        lex.learn("habakkuk chapter three", "have a cook chapter three");
        assert_eq!(lex.apply("turn to have a cook chapter one"), "turn to habakkuk chapter one");
    }

    #[test]
    fn multi_word_and_single_word_patterns_both_work() {
        let mut lex = Lexicon::new();
        for _ in 0..2 {
            lex.learn("first thessalonians five", "first thus alone ions five");
            lex.learn("pastor okafor says", "pastor oh car for says");
        }
        assert_eq!(lex.apply("first thus alone ions five sixteen"), "first thessalonians five sixteen");
        assert_eq!(lex.apply("Pastor oh car for says amen"), "pastor okafor says amen");
    }

    #[test]
    fn common_words_are_never_learned_as_triggers() {
        let mut lex = Lexicon::new();
        for _ in 0..5 {
            lex.learn("verse sixteen", "the sixteen");
            lex.learn("so the lord", "so a lord");
        }
        assert_eq!(lex.apply("the lord is good"), "the lord is good");
        assert_eq!(lex.apply("a lord"), "a lord");
    }

    #[test]
    fn competing_repairs_need_a_clear_winner() {
        let mut lex = Lexicon::new();
        for _ in 0..2 {
            lex.learn("alpha", "wren");
            lex.learn("beta", "wren");
        }
        assert_eq!(lex.apply("wren"), "wren", "tie → leave it alone");
        lex.learn("alpha", "wren");
        lex.learn("alpha", "wren");
        assert_eq!(lex.apply("wren"), "alpha", "4 vs 2 → clear winner");
    }

    #[test]
    fn correct_speech_is_untouched_and_learning_ignores_perfect_matches() {
        let mut lex = Lexicon::new();
        assert_eq!(lex.learn("john chapter three verse sixteen", "john chapter three verse sixteen"), 0);
        assert!(lex.is_empty());
        assert_eq!(lex.apply("john chapter three"), "john chapter three");
    }

    #[test]
    fn dropped_words_need_three_sightings() {
        let mut lex = Lexicon::new();
        for _ in 0..2 {
            lex.learn("holy ghost fire", "holy goat fire"); // sub
        }
        assert_eq!(lex.apply("holy goat fire"), "holy ghost fire");
        let mut lex = Lexicon::new();
        for _ in 0..2 {
            lex.learn("praise god now", "praise uh god now"); // engine inserted a filler
        }
        // "uh" → (nothing) is only a deletion repair; 2 sightings is not enough
        assert_eq!(lex.apply("praise uh god now"), "praise uh god now");
        lex.learn("praise god now", "praise uh god now");
        assert_eq!(lex.apply("praise uh god now"), "praise god now");
    }

    #[test]
    fn tsv_roundtrip_preserves_behaviour() {
        let mut lex = Lexicon::new();
        for _ in 0..3 {
            lex.learn("habakkuk", "have a cook");
        }
        let back = Lexicon::from_tsv(&lex.to_tsv());
        assert_eq!(back.apply("have a cook"), "habakkuk");
        assert!(Lexicon::from_tsv("garbage line\n\n5\t\t\n").is_empty());
    }
}
