//! Learning from operator corrections (v0.819.0). Pure logic, std only.
//!
//! When the operator fixes a transcript line ("pastor adaoyin" -> "Pastor Adeoye"),
//! we remember the changed words. After the SAME fix is made twice we apply it
//! automatically to later lines, and we feed the corrected names to Whisper's
//! prompt so it hears them better next time. One slip never changes anything.

use std::collections::HashMap;

/// A fix must be confirmed this many times before it is applied by itself.
pub const APPLY_AFTER: u32 = 2;
const MAX_WORDS: usize = 6;

#[derive(Default, Debug, Clone)]
pub struct Learner {
    /// (heard words lowercase, fixed words as typed) -> times confirmed
    fixes: HashMap<(String, String), u32>,
}

fn words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'').to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

impl Learner {
    pub fn new() -> Self { Self::default() }

    /// Record one correction. Returns true if something was learned.
    pub fn record(&mut self, heard: &str, fixed: &str) -> bool {
        let (h, f) = (words(heard), words(fixed));
        let lc = |v: &[String]| v.iter().map(|w| w.to_lowercase()).collect::<Vec<_>>();
        let (hl, fl) = (lc(&h), lc(&f));
        // trim the words both lines share at the start and the end
        let mut a = 0;
        while a < hl.len() && a < fl.len() && hl[a] == fl[a] { a += 1; }
        let (mut he, mut fe) = (hl.len(), fl.len());
        while he > a && fe > a && hl[he - 1] == fl[fe - 1] { he -= 1; fe -= 1; }
        let (hs, fs) = (&h[a..he], &f[a..fe]);
        if hs.is_empty() || fs.is_empty() || hs.len() > MAX_WORDS || fs.len() > MAX_WORDS { return false; }
        // verse numbers and references are not names: leave them to the parser
        if fs.iter().chain(hs.iter()).any(|w| w.chars().any(|c| c.is_ascii_digit())) { return false; }
        let heard_key = hs.join(" ").to_lowercase();
        let fixed_val = fs.join(" ");
        if heard_key == fixed_val.to_lowercase() { return false; } // only capitals changed
        if heard_key.len() < 3 { return false; } // too short: would hit ordinary words
        *self.fixes.entry((heard_key, fixed_val)).or_insert(0) += 1;
        true
    }

    /// Apply confirmed fixes to a new line (whole words only, longest first).
    pub fn apply(&self, text: &str) -> String {
        let mut rules: Vec<(&String, &String)> = self
            .fixes.iter().filter(|(_, n)| **n >= APPLY_AFTER).map(|((h, f), _)| (h, f)).collect();
        rules.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.cmp(b.0)));
        let toks: Vec<&str> = text.split_whitespace().collect();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0;
        'outer: while i < toks.len() {
            for (h, f) in &rules {
                let n = h.split(' ').count();
                if i + n > toks.len() { continue; }
                let slice = &toks[i..i + n];
                let core: Vec<String> = slice.iter()
                    .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'').to_lowercase()).collect();
                if core.join(" ") == **h {
                    // keep punctuation that trailed the last word, e.g. "adaoyin,"
                    let last = slice[n - 1];
                    let tail: String = last.chars().rev().take_while(|c| !c.is_alphanumeric()).collect::<Vec<_>>().into_iter().rev().collect();
                    out.push(format!("{}{}", f, tail));
                    i += n;
                    continue 'outer;
                }
            }
            out.push(toks[i].to_string());
            i += 1;
        }
        out.join(" ")
    }

    /// Corrected names for Whisper's prompt: confirmed fixes that look like names
    /// (start with a capital), most-confirmed first, within `max_chars`.
    pub fn prompt_words(&self, max_chars: usize) -> String {
        let mut v: Vec<(&String, u32)> = self.fixes.iter()
            .filter(|((_, f), n)| **n >= APPLY_AFTER && f.chars().next().map_or(false, |c| c.is_uppercase()))
            .map(|((_, f), n)| (f, *n)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let mut out = String::new();
        for (f, _) in v {
            if out.contains(f.as_str()) { continue; }
            if out.len() + f.len() + 2 > max_chars { break; }
            if !out.is_empty() { out.push_str(", "); }
            out.push_str(f);
        }
        out
    }

    /// Forget one fix (the operator's "undo this learning").
    pub fn forget(&mut self, heard: &str) -> usize {
        let key = words(heard).join(" ").to_lowercase();
        let before = self.fixes.len();
        self.fixes.retain(|(h, _), _| *h != key);
        before - self.fixes.len()
    }

    /// Plain text file: `count<TAB>heard<TAB>fixed` per line.
    pub fn to_text(&self) -> String {
        let mut v: Vec<_> = self.fixes.iter().collect();
        v.sort();
        v.iter().map(|((h, f), n)| format!("{}\t{}\t{}\n", n, h, f)).collect()
    }
    pub fn from_text(s: &str) -> Self {
        let mut l = Self::new();
        for line in s.lines() {
            let p: Vec<&str> = line.split('\t').collect();
            if p.len() == 3 {
                if let Ok(n) = p[0].parse::<u32>() { l.fixes.insert((p[1].to_string(), p[2].to_string()), n); }
            }
        }
        l
    }
    /// (heard, fixed, times) for showing the list, most-confirmed first.
    pub fn fixes(&self) -> Vec<(String, String, u32)> {
        let mut v: Vec<_> = self.fixes.iter().map(|((h, f), n)| (h.clone(), f.clone(), *n)).collect();
        v.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
        v
    }
    pub fn len(&self) -> usize { self.fixes.len() }
    pub fn is_empty(&self) -> bool { self.fixes.is_empty() }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn taught() -> Learner {
        let mut l = Learner::new();
        l.record("welcome pastor adaoyin to the pulpit", "Welcome Pastor Adeoye to the pulpit");
        l.record("thank you pastor adaoyin", "Thank you Pastor Adeoye");
        l
    }
    #[test] fn one_fix_does_nothing_yet() {
        let mut l = Learner::new();
        assert!(l.record("pastor adaoyin said", "Pastor Adeoye said"));
        assert_eq!(l.apply("pastor adaoyin said"), "pastor adaoyin said");
    }
    #[test] fn two_fixes_apply_later() {
        // only the changed word is learned ("pastor" was already right)
        assert_eq!(taught().apply("here is pastor adaoyin, today"), "here is pastor Adeoye, today");
        assert_eq!(taught().apply("adaoyin"), "Adeoye");
    }
    #[test] fn learns_only_the_changed_words() {
        let mut l = Learner::new();
        l.record("we meet at redeemers hall", "we meet at Redeemer's Hall");
        assert!(l.to_text().contains("redeemers\tRedeemer's"));
    }
    #[test] fn ignores_capitals_only_digits_and_rewrites() {
        let mut l = Learner::new();
        assert!(!l.record("john three sixteen", "John three sixteen"));
        assert!(!l.record("john three sixteen", "John 3:16"));
        assert!(!l.record("a b c d e f g h i", "totally different long sentence goes here ok"));
        assert!(!l.record("same", "same"));
        assert!(l.is_empty());
    }
    #[test] fn short_words_never_become_rules() {
        let mut l = Learner::new();
        assert!(!l.record("so we go", "To we go")); // 'so' is 2 letters
        assert!(l.is_empty());
    }
    #[test] fn whole_words_only() {
        let t = taught();
        assert_eq!(t.apply("pastor adaoyins book"), "pastor adaoyins book");
        assert_eq!(t.apply("adaoyin and adaoyinx"), "Adeoye and adaoyinx");
    }
    #[test] fn prompt_gets_confirmed_names_only() {
        let mut t = taught();
        t.record("gracias hall", "Gracie's Hall");
        assert_eq!(t.prompt_words(100), "Adeoye");
        assert_eq!(t.prompt_words(5), "");
    }
    #[test] fn saves_and_loads_and_forgets() {
        let t = taught();
        let mut back = Learner::from_text(&t.to_text());
        assert_eq!(back.apply("pastor adaoyin"), "pastor Adeoye");
        assert_eq!(back.forget("Adaoyin"), 1);
        assert_eq!(back.apply("pastor adaoyin"), "pastor adaoyin");
    }
    #[test] fn bad_file_lines_are_skipped() {
        let l = Learner::from_text("garbage\nx\ty\tz\n2\tabc def\tAbc Def\n");
        assert_eq!(l.len(), 1);
    }
}
