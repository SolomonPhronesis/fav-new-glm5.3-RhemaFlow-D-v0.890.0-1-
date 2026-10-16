//! Scoring a whole folder of real clips (v0.851.0). Pure logic, std only.
//!
//! `rhemaflow-d evalset FOLDER` transcribes every `clip.wav` with each engine and hands the results here. We
//! align what was said (`clip.txt`) against what the engine heard word by word, then add everything up:
//! overall word error rate, how often the right scripture was found, the clips that went worst, and the
//! mix-ups that keep coming back ("habakkuk" heard as "have a cook"). The repeated mix-ups are the best list
//! of words to teach the vocabulary prompt and the personal lexicon.

use crate::wer::{normalize_words, Wer};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Match(String),
    Sub(String, String),
    Del(String),
    Ins(String),
}

/// Word-by-word alignment of `hypothesis` against `reference` (same normalisation as the WER).
pub fn align(reference: &str, hypothesis: &str) -> Vec<Op> {
    let r = normalize_words(reference);
    let h = normalize_words(hypothesis);
    let (n, m) = (r.len(), h.len());
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=m {
        dp[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub = dp[i - 1][j - 1] + usize::from(r[i - 1] != h[j - 1]);
            dp[i][j] = sub.min(dp[i - 1][j] + 1).min(dp[i][j - 1] + 1);
        }
    }
    let (mut i, mut j) = (n, m);
    let mut ops = Vec::new();
    // On a tie prefer: a match, then a dropped word, then an extra word, and a substitution last, so a mix-up
    // stays one tidy run ("habakkuk" -> "have a cook") instead of smearing over the words that were right.
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && r[i - 1] == h[j - 1] && dp[i][j] == dp[i - 1][j - 1] {
            ops.push(Op::Match(r[i - 1].clone()));
            i -= 1;
            j -= 1;
        } else if i > 0 && dp[i][j] == dp[i - 1][j] + 1 {
            ops.push(Op::Del(r[i - 1].clone()));
            i -= 1;
        } else if j > 0 && dp[i][j] == dp[i][j - 1] + 1 {
            ops.push(Op::Ins(h[j - 1].clone()));
            j -= 1;
        } else {
            ops.push(Op::Sub(r[i - 1].clone(), h[j - 1].clone()));
            i -= 1;
            j -= 1;
        }
    }
    ops.reverse();
    ops
}

/// Runs of mistakes as (what was said, what was heard): "habakkuk" -> "have a cook". An empty second part means
/// the words were dropped; an empty first part (extra words only) is not listed.
pub fn confusions(ops: &[Op]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let (mut said, mut heard): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    let flush = |said: &mut Vec<String>, heard: &mut Vec<String>, out: &mut Vec<(String, String)>| {
        if !said.is_empty() {
            out.push((said.join(" "), heard.join(" ")));
        }
        said.clear();
        heard.clear();
    };
    for op in ops {
        match op {
            Op::Match(_) => flush(&mut said, &mut heard, &mut out),
            Op::Sub(r, h) => {
                said.push(r.clone());
                heard.push(h.clone());
            }
            Op::Del(r) => said.push(r.clone()),
            Op::Ins(h) => heard.push(h.clone()),
        }
    }
    flush(&mut said, &mut heard, &mut out);
    out
}

/// One clip's result for one engine.
#[derive(Clone, Debug)]
pub struct Clip {
    pub name: String,
    pub wer: Wer,
    pub confusions: Vec<(String, String)>,
    /// the scripture the clip should produce ("John 3:16"), from `clip.expect`
    pub expected: Option<String>,
    /// labels the parser produced; a trailing "?" means it only asked "did you mean?"
    pub detected: Vec<String>,
}

fn label_key(s: &str) -> String {
    s.trim().trim_end_matches('?').split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

impl Clip {
    /// Some(true) = found it for sure, Some(false) = missed or only asked, None = nothing was expected.
    pub fn scripture_found(&self) -> Option<bool> {
        let want = label_key(self.expected.as_deref()?);
        Some(self.detected.iter().any(|d| !d.trim().ends_with('?') && label_key(d) == want))
    }

    /// Found only as a "did you mean?" suggestion.
    pub fn scripture_asked(&self) -> bool {
        match self.expected.as_deref() {
            Some(e) => {
                let want = label_key(e);
                self.scripture_found() == Some(false) && self.detected.iter().any(|d| label_key(d) == want)
            }
            None => false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub clips: usize,
    pub ref_words: usize,
    pub errors: usize,
    pub scripture_expected: usize,
    pub scripture_found: usize,
    pub scripture_asked: usize,
    /// (clip, word error rate), worst first
    pub worst: Vec<(String, f64)>,
    /// ((said, heard), times), most frequent first
    pub confusions: Vec<((String, String), usize)>,
}

impl Summary {
    pub fn wer(&self) -> f64 {
        self.errors as f64 / self.ref_words.max(1) as f64
    }
}

pub fn summarize(clips: &[Clip]) -> Summary {
    let mut s = Summary { clips: clips.len(), ..Default::default() };
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for c in clips {
        s.ref_words += c.wer.ref_words;
        s.errors += c.wer.errors();
        s.worst.push((c.name.clone(), c.wer.rate()));
        match c.scripture_found() {
            Some(true) => {
                s.scripture_expected += 1;
                s.scripture_found += 1;
            }
            Some(false) => {
                s.scripture_expected += 1;
                if c.scripture_asked() {
                    s.scripture_asked += 1;
                }
            }
            None => {}
        }
        for pair in &c.confusions {
            *counts.entry(pair.clone()).or_insert(0) += 1;
        }
    }
    s.worst.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    let mut v: Vec<((String, String), usize)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    s.confusions = v;
    s
}

const COMMON: &[&str] = &[
    "the", "a", "an", "and", "of", "to", "in", "is", "it", "he", "she", "we", "i", "you", "that", "for", "on", "with", "as", "at", "by", "be", "was", "his", "her", "are", "this", "they", "not", "but", "or", "so", "my", "me", "our", "your", "him", "them", "have", "has", "had", "do", "did", "will", "shall", "unto", "thy", "thou", "ye",
];

/// Words worth teaching the vocabulary prompt: said (not just a filler word) and mis-heard at least `min_times`.
pub fn vocabulary_suggestions(s: &Summary, min_times: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for ((said, _), n) in &s.confusions {
        if *n < min_times {
            continue;
        }
        let words: Vec<&str> = said.split_whitespace().collect();
        if words.is_empty() || words.iter().all(|w| COMMON.contains(w) || w.len() < 3) || words.len() > 4 {
            continue;
        }
        if !out.contains(said) {
            out.push(said.clone());
        }
    }
    out
}

/// A readable report for one engine.
pub fn report(engine: &str, s: &Summary) -> String {
    let mut t = format!("## {engine}\n\n");
    t.push_str(&format!("- clips: {}\n- words said: {}\n- word error rate: {:.1}%  ({} errors)\n", s.clips, s.ref_words, s.wer() * 100.0, s.errors));
    if s.scripture_expected > 0 {
        t.push_str(&format!(
            "- right scripture found: {} of {}  ({:.0}%)  [+{} only asked \"did you mean?\"]\n",
            s.scripture_found,
            s.scripture_expected,
            100.0 * s.scripture_found as f64 / s.scripture_expected as f64,
            s.scripture_asked
        ));
    }
    t.push_str("\n### Worst clips\n\n");
    for (name, rate) in s.worst.iter().take(5) {
        t.push_str(&format!("- {name}: {:.1}%\n", rate * 100.0));
    }
    t.push_str("\n### Mix-ups that came back (said -> heard)\n\n");
    for ((said, heard), n) in s.confusions.iter().filter(|(_, n)| *n >= 2).take(15) {
        t.push_str(&format!("- {n}x  \"{said}\" -> \"{}\"\n", if heard.is_empty() { "(dropped)" } else { heard }));
    }
    let vocab = vocabulary_suggestions(s, 2);
    if !vocab.is_empty() {
        t.push_str("\n### Words to teach (add to the vocabulary / calibrate)\n\n");
        for w in vocab {
            t.push_str(&format!("- {w}\n"));
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wer::word_error_rate;

    fn clip(name: &str, said: &str, heard: &str, expected: Option<&str>, detected: &[&str]) -> Clip {
        Clip { name: name.into(), wer: word_error_rate(said, heard), confusions: confusions(&align(said, heard)), expected: expected.map(String::from), detected: detected.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn alignment_finds_substitutions_drops_and_extras() {
        let ops = align("turn to habakkuk chapter two", "turn to have a cook chapter");
        assert_eq!(confusions(&ops), vec![("habakkuk".to_string(), "have a cook".to_string()), ("two".to_string(), String::new())]);
        assert_eq!(align("a b", "a b"), vec![Op::Match("a".into()), Op::Match("b".into())]);
        assert!(confusions(&align("a b", "a x b")).is_empty(), "extra words alone are not a mix-up");
    }

    #[test]
    fn alignment_agrees_with_the_word_error_rate() {
        let (s, h) = ("let us read john chapter three verse sixteen", "lets read john chapter free sixteen");
        let w = word_error_rate(s, h);
        let ops = align(s, h);
        let wrong = ops.iter().filter(|o| !matches!(o, Op::Match(_))).count();
        assert_eq!(wrong, ops.len() - ops.iter().filter(|o| matches!(o, Op::Match(_))).count());
        let subs = ops.iter().filter(|o| matches!(o, Op::Sub(..))).count();
        let dels = ops.iter().filter(|o| matches!(o, Op::Del(_))).count();
        let ins = ops.iter().filter(|o| matches!(o, Op::Ins(_))).count();
        // the split between substitutions / drops / extras can differ on ties; the total cannot
        assert_eq!(subs + dels + ins, w.errors());
    }

    #[test]
    fn summary_adds_up_and_ranks() {
        let clips = vec![
            clip("a", "turn to habakkuk chapter two", "turn to have a cook chapter two", Some("Habakkuk 2"), &["Habakkuk 2"]),
            clip("b", "read habakkuk chapter three", "read have a cook chapter three", Some("Habakkuk 3"), &["Habakkuk 3?"]),
            clip("c", "john chapter three verse sixteen", "john chapter three verse sixteen", Some("John 3:16"), &["john 3:16"]),
            clip("d", "next verse", "next verse", None, &[]),
        ];
        let s = summarize(&clips);
        assert_eq!((s.clips, s.scripture_expected, s.scripture_found, s.scripture_asked), (4, 3, 2, 1));
        assert!(s.worst[0].1 >= s.worst[1].1 && s.worst[3].1 == 0.0);
        assert_eq!(s.confusions[0], (("habakkuk".to_string(), "have a cook".to_string()), 2));
        assert_eq!(vocabulary_suggestions(&s, 2), vec!["habakkuk".to_string()]);
        let r = report("whisper", &s);
        assert!(r.contains("2 of 3") && r.contains("\"habakkuk\" -> \"have a cook\"") && r.contains("- habakkuk"));
    }

    #[test]
    fn filler_words_are_never_suggested() {
        let clips = vec![clip("a", "the king", "a king", None, &[]), clip("b", "the lord", "a lord", None, &[])];
        assert!(vocabulary_suggestions(&summarize(&clips), 2).is_empty());
    }
}
