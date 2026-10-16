//! Live, append-only transcript text - the "words appear and never change" behaviour
//! of the R2T2 demo, for engines that do NOT stream that way.
//!
//! Problem: while someone is still talking, Whisper re-decodes the audio so far every
//! second or so, and each new decode may change words it showed before (real example
//! recorded from the app: "ask not watch your country" became "ask not what your
//! country"; "fellow American." became "fellow Americans."). Showing each decode as it
//! comes makes text flicker and rewrite itself.
//!
//! Fix (LocalAgreement-2, from the whisper-streaming line of work): a word is shown as
//! *committed* (solid, permanent) only once two consecutive decodes agree on it. Words
//! the decoder is still unsure about are shown as *tentative* (dimmed) and may change,
//! but committed words are never touched again for the rest of the utterance.
//!
//! For an engine that is already append-only (R2T2) use `Strategy::AppendOnly`: every
//! word is committed the moment it arrives.
//!
//! What this cannot do: if the decoder is consistently wrong ("American" twice in a
//! row), the wrong word is committed. The authoritative final text still replaces the
//! live strip when the utterance ends (`finish` reports how many committed words the
//! final text disagreed with).

fn norm(w: &str) -> String {
    w.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    /// commit only what two consecutive decodes agree on (Whisper-style engines)
    Agreement,
    /// every word is final as soon as it arrives (true streaming engines)
    AppendOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LiveView {
    /// permanent text, never revised during the utterance
    pub committed: String,
    /// the decoder's current guess for the rest; may change on the next update
    pub tentative: String,
}

impl LiveView {
    pub fn full(&self) -> String {
        match (self.committed.is_empty(), self.tentative.is_empty()) {
            (true, _) => self.tentative.clone(),
            (_, true) => self.committed.clone(),
            _ => format!("{} {}", self.committed, self.tentative),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinishReport {
    pub committed_words: usize,
    /// committed words that the final text does not contain at the same place
    pub disagreed_words: usize,
}

pub struct LiveText {
    strategy: Strategy,
    committed: Vec<String>, // display form
    tail_norm: Vec<String>, // previous decode's uncommitted words, normalised
    /// times a new decode changed words we had already committed (kept out of sight)
    pub ignored_revisions: usize,
}

impl LiveText {
    pub fn new(strategy: Strategy) -> Self {
        LiveText { strategy, committed: Vec::new(), tail_norm: Vec::new(), ignored_revisions: 0 }
    }

    pub fn strategy(&self) -> Strategy {
        self.strategy
    }

    /// Switch strategy (e.g. the streaming engine dropped and Whisper took over). Resets
    /// the utterance in progress, because the two kinds of "interim text" mean different things.
    pub fn set_strategy(&mut self, strategy: Strategy) {
        if self.strategy != strategy {
            self.strategy = strategy;
            self.reset();
        }
    }

    pub fn committed_text(&self) -> String {
        self.committed.join(" ")
    }

    /// Feed the newest decode of the WHOLE current utterance.
    pub fn update(&mut self, hypothesis: &str) -> LiveView {
        let words: Vec<&str> = hypothesis.split_whitespace().collect();
        let normed: Vec<String> = words.iter().map(|w| norm(w)).collect();
        let start = self.boundary(&normed);
        let rest: Vec<&str> = words[start..].to_vec();
        let rest_norm: Vec<String> = normed[start..].to_vec();

        let commit_n = match self.strategy {
            Strategy::AppendOnly => rest.len(),
            Strategy::Agreement => self.tail_norm.iter().zip(rest_norm.iter()).take_while(|(a, b)| a == b && !a.is_empty()).count(),
        };
        for w in &rest[..commit_n] {
            self.committed.push((*w).to_string());
        }
        self.tail_norm = rest_norm[commit_n..].to_vec();
        LiveView { committed: self.committed.join(" "), tentative: rest[commit_n..].join(" ") }
    }

    /// Where in `normed` the uncommitted part begins. Normally right after the committed
    /// words; if the decoder changed earlier words, re-anchor on the committed tail
    /// instead of rewriting anything.
    fn boundary(&mut self, normed: &[String]) -> usize {
        let c = self.committed.len();
        if c == 0 {
            return 0;
        }
        let committed_norm: Vec<String> = self.committed.iter().map(|w| norm(w)).collect();
        if normed.len() >= c && normed[..c] == committed_norm[..] {
            return c;
        }
        self.ignored_revisions += 1;
        // longest suffix of the committed words that occurs inside the new decode
        for k in (1..=c.min(6)).rev() {
            let suffix = &committed_norm[c - k..];
            for j in (k..=normed.len()).rev() {
                if &normed[j - k..j] == suffix {
                    return j;
                }
            }
        }
        // nothing lines up: fall back to the same position as before
        c.min(normed.len())
    }

    /// The utterance ended with `final_text` (the authoritative transcript). Reset for the next one.
    pub fn finish(&mut self, final_text: &str) -> FinishReport {
        let fin: Vec<String> = final_text.split_whitespace().map(norm).collect();
        let disagreed = self
            .committed
            .iter()
            .enumerate()
            .filter(|(i, w)| fin.get(*i).map_or(true, |f| *f != norm(w)))
            .count();
        let report = FinishReport { committed_words: self.committed.len(), disagreed_words: disagreed };
        self.reset();
        report
    }

    pub fn reset(&mut self) {
        self.committed.clear();
        self.tail_norm.clear();
        self.ignored_revisions = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn recorded(cond: &str) -> Vec<(f32, String)> {
        let raw = include_str!("../tests/fixtures/jfk-interims.jsonl");
        let mut v: Vec<(usize, f32, String)> = raw
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|j| j["cond"] == cond)
            .map(|j| (j["step"].as_u64().unwrap() as usize, j["secs"].as_f64().unwrap() as f32, j["text"].as_str().unwrap().to_string()))
            .collect();
        v.sort_by_key(|(step, _, _)| *step);
        v.into_iter().map(|(_, s, t)| (s, t)).collect()
    }

    const FINAL: &str = "And so my fellow Americans ask not what your country can do for you ask what you can do for your country.";

    fn is_prefix(a: &str, b: &str) -> bool {
        let (a, b): (Vec<_>, Vec<_>) = (a.split_whitespace().collect(), b.split_whitespace().collect());
        a.len() <= b.len() && a == b[..a.len()]
    }

    /// Words of the raw decodes that a naive display would have rewritten (a word shown
    /// at position i in one decode and different in the next).
    fn naive_rewrites(hyps: &[(f32, String)]) -> usize {
        let mut n = 0;
        for w in hyps.windows(2) {
            let (a, b): (Vec<String>, Vec<String>) = (w[0].1.split_whitespace().map(norm).collect(), w[1].1.split_whitespace().map(norm).collect());
            n += a.iter().zip(b.iter()).filter(|(x, y)| x != y).count();
        }
        n
    }

    #[test]
    fn real_whisper_decodes_clean_clip_committed_text_only_ever_grows() {
        let hyps = recorded("clean");
        assert_eq!(hyps.len(), 7, "fixture should hold 7 real interim decodes");
        let mut lt = LiveText::new(Strategy::Agreement);
        let mut last = String::new();
        println!("\nCLEAN clip - raw decode vs committed (solid) | tentative (dim)");
        for (secs, h) in &hyps {
            let v = lt.update(h);
            println!("{secs:>4.1}s raw: {h:<108}\n      solid: {} | dim: {}", v.committed, v.tentative);
            assert!(is_prefix(&last, &v.committed), "committed text was rewritten: {last:?} -> {:?}", v.committed);
            last = v.committed.clone();
        }
        let rep = lt.finish(FINAL);
        println!("naive rewrites avoided: {}; final report {rep:?}", naive_rewrites(&hyps));
        assert!(naive_rewrites(&hyps) >= 2, "the real recording does contain rewrites (American->Americans, watch->what)");
        assert_eq!(rep.disagreed_words, 0, "nothing committed disagreed with the final text on the clean clip");
        assert!(rep.committed_words >= 15, "most of the sentence should be committed before the end, got {}", rep.committed_words);
    }

    #[test]
    fn real_whisper_decodes_noisy_clip_never_rewrites_and_reports_honestly() {
        let hyps = recorded("noise10");
        assert_eq!(hyps.len(), 7);
        let mut lt = LiveText::new(Strategy::Agreement);
        let mut last = String::new();
        println!("\nNOISE 10 dB clip");
        for (secs, h) in &hyps {
            let v = lt.update(h);
            println!("{secs:>4.1}s raw: {h:<100}\n      solid: {} | dim: {}", v.committed, v.tentative);
            assert!(is_prefix(&last, &v.committed));
            last = v.committed.clone();
        }
        // REAL finding: in noise Whisper said "And not" in three consecutive decodes
        // (5.0, 6.5, 8.0 s) before it corrected itself to "Ask not" at 9.5 s. Agreement
        // cannot know that, so "And" stays on screen: that is the price of never
        // rewriting. The authoritative final line says "Ask not", and `finish` reports
        // the 1 disagreement so it is visible, not hidden.
        let final_noisy = "And so my fellow American. Ask not what your country can do for you. Ask what you can do for your country.";
        let rep = lt.finish(final_noisy);
        println!("report vs noisy final: {rep:?}");
        assert_eq!(rep.disagreed_words, 1, "exactly the 'And'/'Ask' word");
        let mut lt2 = LiveText::new(Strategy::Agreement);
        for (_, h) in &hyps {
            lt2.update(h);
        }
        let rep2 = lt2.finish(FINAL); // the TRUE sentence
        println!("report vs the true sentence: {rep2:?} (a consistently wrong word cannot be caught)");
        assert!(rep2.disagreed_words >= 1);
    }

    #[test]
    fn a_word_is_committed_only_after_two_decodes_agree() {
        let mut lt = LiveText::new(Strategy::Agreement);
        let v = lt.update("turn to john");
        assert_eq!((v.committed.as_str(), v.tentative.as_str()), ("", "turn to john"));
        let v = lt.update("turn to john chapter three");
        assert_eq!((v.committed.as_str(), v.tentative.as_str()), ("turn to john", "chapter three"));
        let v = lt.update("turn to john chapter three verse six");
        assert_eq!(v.committed, "turn to john chapter three");
        let v = lt.update("turn to john chapter three verse sixteen");
        assert_eq!(v.committed, "turn to john chapter three verse", "\"verse\" was seen twice; six -> sixteen is not agreed");
        assert_eq!(v.tentative, "sixteen", "the unsure number stays dim, it is never committed as 'six'");
        let v = lt.update("turn to john chapter three verse sixteen");
        assert_eq!(v.committed, "turn to john chapter three verse sixteen");
    }

    #[test]
    fn if_the_decoder_changes_a_committed_word_the_display_keeps_the_old_word_and_carries_on() {
        let mut lt = LiveText::new(Strategy::Agreement);
        lt.update("the lord is my shepherd");
        lt.update("the lord is my shepherd i shall");
        assert_eq!(lt.committed_text(), "the lord is my shepherd");
        // a later decode now hears "sheperd" and a different start
        let v = lt.update("a lord is my sheperd i shall not want");
        assert!(v.committed.starts_with("the lord is my shepherd"), "never rewritten: {v:?}");
        assert!(lt.ignored_revisions >= 1);
        let v = lt.update("a lord is my sheperd i shall not want");
        assert!(v.full().ends_with("i shall not want"), "{v:?}");
        assert!(!v.full().contains("sheperd") && !v.full().contains("a lord"), "no duplicated or re-added words: {v:?}");
    }

    #[test]
    fn append_only_engines_commit_every_word_immediately() {
        let mut lt = LiveText::new(Strategy::AppendOnly);
        let v = lt.update("turn with me");
        assert_eq!((v.committed.as_str(), v.tentative.as_str()), ("turn with me", ""));
        let v = lt.update("turn with me to john");
        assert_eq!(v.committed, "turn with me to john");
        assert_eq!(lt.finish("turn with me to john").disagreed_words, 0);
    }

    #[test]
    fn switching_strategy_resets_only_when_it_changes() {
        let mut lt = LiveText::new(Strategy::AppendOnly);
        lt.update("turn with me");
        lt.set_strategy(Strategy::AppendOnly);
        assert_eq!(lt.committed_text(), "turn with me");
        lt.set_strategy(Strategy::Agreement);
        assert_eq!(lt.committed_text(), "");
    }

    #[test]
    fn finish_resets_for_the_next_utterance_and_empty_input_is_safe() {
        let mut lt = LiveText::new(Strategy::Agreement);
        lt.update("");
        lt.update("amen amen");
        lt.update("amen amen");
        lt.finish("amen amen");
        assert_eq!(lt.committed_text(), "");
        assert_eq!(lt.update("fresh start").committed, "");
    }
}
