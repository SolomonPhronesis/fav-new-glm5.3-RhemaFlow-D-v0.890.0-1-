//! Word error rate (WER) — the standard way to compare speech engines.
//!
//! `WER = (substitutions + deletions + insertions) / words in the reference`.
//! Lower is better; 0.0 is a perfect transcript; it can exceed 1.0 when the
//! hypothesis has many extra words. Used by `rhemaflow-d benchmark` to compare
//! Whisper, Parakeet and any external transcript on the same audio.
//!
//! Normalisation is deliberately simple and applied to both sides: lowercase,
//! apostrophes removed, every other non-alphanumeric character treated as a
//! word break. Numerals and number words are NOT unified ("16" ≠ "sixteen"),
//! so a WER measured here is a conservative upper bound for scripture audio.

/// Result of comparing a hypothesis against a reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wer {
    pub ref_words: usize,
    pub hyp_words: usize,
    pub substitutions: usize,
    pub deletions: usize,
    pub insertions: usize,
}

impl Wer {
    /// Total word errors (edit distance over words).
    pub fn errors(&self) -> usize {
        self.substitutions + self.deletions + self.insertions
    }

    /// Errors per reference word. An empty reference counts as one word so the
    /// rate stays finite (0.0 for an empty hypothesis, `insertions` otherwise).
    pub fn rate(&self) -> f64 {
        self.errors() as f64 / self.ref_words.max(1) as f64
    }
}

/// Lowercase, strip apostrophes, split on anything non-alphanumeric.
pub fn normalize_words(text: &str) -> Vec<String> {
    let cleaned: String = text
        .chars()
        .filter(|c| *c != '\'' && *c != '\u{2019}')
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { ' ' })
        .collect();
    cleaned.split_whitespace().map(|w| w.to_string()).collect()
}

/// Compare `hypothesis` to `reference`.
pub fn word_error_rate(reference: &str, hypothesis: &str) -> Wer {
    let r = normalize_words(reference);
    let h = normalize_words(hypothesis);
    let (n, m) = (r.len(), h.len());

    // dp[i][j] = edit distance between r[..i] and h[..j]
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

    // walk back to split the distance into S / D / I
    let (mut i, mut j) = (n, m);
    let (mut s, mut d, mut ins) = (0, 0, 0);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && dp[i][j] == dp[i - 1][j - 1] + usize::from(r[i - 1] != h[j - 1]) {
            if r[i - 1] != h[j - 1] {
                s += 1;
            }
            i -= 1;
            j -= 1;
        } else if i > 0 && dp[i][j] == dp[i - 1][j] + 1 {
            d += 1;
            i -= 1;
        } else {
            ins += 1;
            j -= 1;
        }
    }
    Wer { ref_words: n, hyp_words: m, substitutions: s, deletions: d, insertions: ins }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_is_zero() {
        let w = word_error_rate("turn to John chapter three", "turn to John chapter three");
        assert_eq!(w.errors(), 0);
        assert_eq!(w.rate(), 0.0);
    }

    #[test]
    fn case_punctuation_and_apostrophes_are_ignored() {
        let w = word_error_rate("Let's turn to John 3:16.", "lets TURN to john 3 16");
        assert_eq!(w.errors(), 0, "{w:?}");
    }

    #[test]
    fn one_substitution_in_four_words() {
        let w = word_error_rate("turn to john three", "turn to jon three");
        assert_eq!((w.substitutions, w.deletions, w.insertions), (1, 0, 0));
        assert!((w.rate() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn deletion_and_insertion_are_told_apart() {
        let del = word_error_rate("first corinthians chapter five", "first corinthians five");
        assert_eq!((del.substitutions, del.deletions, del.insertions), (0, 1, 0));
        let ins = word_error_rate("first corinthians five", "first corinthians chapter five");
        assert_eq!((ins.substitutions, ins.deletions, ins.insertions), (0, 0, 1));
    }

    #[test]
    fn breakdown_sums_to_edit_distance() {
        let w = word_error_rate("for god so loved the world", "for the lord loved world today");
        assert_eq!(w.errors(), w.substitutions + w.deletions + w.insertions);
        // hand-checked: god→the, so→lord (2 subs), "the" deleted, "today" inserted = 4
        assert_eq!(w.errors(), 4, "{w:?}");
    }

    #[test]
    fn empty_cases_are_finite() {
        assert_eq!(word_error_rate("", "").rate(), 0.0);
        let w = word_error_rate("", "extra words here");
        assert_eq!(w.insertions, 3);
        assert_eq!(w.rate(), 3.0);
        let w = word_error_rate("some words", "");
        assert_eq!(w.deletions, 2);
        assert_eq!(w.rate(), 1.0);
    }

    #[test]
    fn completely_different_is_all_substitutions() {
        let w = word_error_rate("alpha beta gamma", "one two three");
        assert_eq!(w.substitutions, 3);
        assert_eq!(w.rate(), 1.0);
    }

    #[test]
    fn rate_can_exceed_one() {
        let w = word_error_rate("amen", "amen amen amen amen");
        assert_eq!(w.insertions, 3);
        assert!(w.rate() > 1.0);
    }

    #[test]
    fn numerals_and_number_words_are_different_words() {
        // documented limitation: conservative upper bound
        assert_eq!(word_error_rate("chapter 3", "chapter three").substitutions, 1);
    }
}
