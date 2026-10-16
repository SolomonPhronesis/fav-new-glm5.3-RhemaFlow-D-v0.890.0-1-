//! Feature 3: two or three Bible versions on one slide (side by side or stacked), paged so nothing is tiny.
//!
//! Verse numbers are matched across versions, so a verse that one version leaves out does not shift the others.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    SideBySide,
    Stacked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    /// "KJV", "ASV"...
    pub label: String,
    /// (verse number, words), in any order
    pub verses: Vec<(u32, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub label: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParallelSlide {
    /// "John 3:16" or "John 3:16-18"
    pub reference: String,
    pub mode: Mode,
    pub blocks: Vec<Block>,
    pub first_verse: u32,
    pub last_verse: u32,
}

pub const MISSING: &str = "(not in this version)";

/// `chapter` is "John 3". Up to 4 versions are used (more would be unreadable).
pub fn build(chapter: &str, versions: &[Version], mode: Mode, show_numbers: bool, max_chars_per_block: usize) -> Vec<ParallelSlide> {
    let versions: Vec<&Version> = versions.iter().take(4).collect();
    if versions.is_empty() {
        return Vec::new();
    }
    let mut numbers: Vec<u32> = versions.iter().flat_map(|v| v.verses.iter().map(|(n, _)| *n)).collect();
    numbers.sort_unstable();
    numbers.dedup();
    if numbers.is_empty() {
        return Vec::new();
    }
    let numbered = show_numbers && numbers.len() > 1;
    let piece = |v: &Version, n: u32| -> Option<String> {
        v.verses.iter().find(|(k, _)| *k == n).map(|(_, t)| {
            let t = t.trim();
            if numbered {
                format!("{n} {t}")
            } else {
                t.to_string()
            }
        })
    };
    let page_text = |v: &Version, page: &[u32]| -> String {
        let parts: Vec<String> = page.iter().filter_map(|n| piece(v, *n)).collect();
        if parts.is_empty() {
            MISSING.to_string()
        } else {
            parts.join(" ")
        }
    };
    let mut pages: Vec<Vec<u32>> = Vec::new();
    let mut cur: Vec<u32> = Vec::new();
    for n in numbers {
        let mut trial = cur.clone();
        trial.push(n);
        let longest = versions.iter().map(|v| page_text(v, &trial).chars().count()).max().unwrap_or(0);
        if !cur.is_empty() && max_chars_per_block > 0 && longest > max_chars_per_block {
            pages.push(std::mem::take(&mut cur));
            cur.push(n);
        } else {
            cur = trial;
        }
    }
    if !cur.is_empty() {
        pages.push(cur);
    }
    pages
        .into_iter()
        .map(|p| {
            let (first, last) = (p[0], *p.last().unwrap());
            let reference = if first == last { format!("{chapter}:{first}") } else { format!("{chapter}:{first}-{last}") };
            let blocks = versions.iter().map(|v| Block { label: v.label.clone(), text: page_text(v, &p) }).collect();
            ParallelSlide { reference, mode, blocks, first_verse: first, last_verse: last }
        })
        .collect()
}

/// How much smaller than a single-version slide the words should be.
pub fn font_scale(versions: usize, mode: Mode) -> f32 {
    match (versions, mode) {
        (0..=1, _) => 1.0,
        (2, Mode::SideBySide) => 0.78,
        (3, Mode::SideBySide) => 0.62,
        (4, Mode::SideBySide) => 0.5,
        (2, Mode::Stacked) => 0.72,
        (3, Mode::Stacked) => 0.55,
        _ => 0.45,
    }
}

/// Where each block goes, as fractions of the safe area: (x, y, width, height). A small gap separates them.
pub fn rects(count: usize, mode: Mode) -> Vec<[f32; 4]> {
    if count == 0 {
        return Vec::new();
    }
    let gap = 0.03f32;
    let n = count as f32;
    (0..count)
        .map(|i| {
            let i = i as f32;
            match mode {
                Mode::SideBySide => {
                    let w = (1.0 - gap * (n - 1.0)) / n;
                    [i * (w + gap), 0.0, w, 1.0]
                }
                Mode::Stacked => {
                    let h = (1.0 - gap * (n - 1.0)) / n;
                    [0.0, i * (h + gap), 1.0, h]
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(label: &str, verses: &[(u32, &str)]) -> Version {
        Version { label: label.into(), verses: verses.iter().map(|(n, t)| (*n, t.to_string())).collect() }
    }

    #[test]
    fn two_versions_one_verse() {
        let s = build("John 3", &[v("KJV", &[(16, "For God so loved")]), v("BBE", &[(16, "For God had such love")])], Mode::SideBySide, true, 500);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].reference, "John 3:16");
        assert_eq!(s[0].blocks[0], Block { label: "KJV".into(), text: "For God so loved".into() });
        assert_eq!(s[0].blocks[1].label, "BBE");
    }

    #[test]
    fn a_range_is_numbered_and_a_missing_verse_does_not_shift_the_other_version() {
        let a = v("A", &[(1, "one"), (2, "two"), (3, "three")]);
        let b = v("B", &[(1, "uno"), (3, "tres")]);
        let s = build("X 1", &[a, b], Mode::Stacked, true, 500);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].reference, "X 1:1-3");
        assert_eq!(s[0].blocks[0].text, "1 one 2 two 3 three");
        assert_eq!(s[0].blocks[1].text, "1 uno 3 tres");
    }

    #[test]
    fn long_passages_are_paged_by_the_longest_version() {
        let words = |n: usize| "word ".repeat(n).trim().to_string();
        let a = v("A", &[(1, &words(10)), (2, &words(10)), (3, &words(10))]);
        let b = v("B", &[(1, "short"), (2, "short"), (3, "short")]);
        let s = build("X 1", &[a, b], Mode::SideBySide, false, 60);
        assert!(s.len() >= 2, "{}", s.len());
        for p in &s {
            assert!(p.blocks[0].text.chars().count() <= 60 || p.first_verse == p.last_verse);
        }
        assert_eq!(s.first().unwrap().first_verse, 1);
        assert_eq!(s.last().unwrap().last_verse, 3);
        // no verse is lost or repeated
        let mut seen: Vec<u32> = s.iter().flat_map(|p| p.first_verse..=p.last_verse).collect();
        seen.dedup();
        assert_eq!(seen, vec![1, 2, 3]);
    }

    #[test]
    fn a_verse_longer_than_the_limit_still_gets_its_own_page() {
        let a = v("A", &[(1, &"x".repeat(300)), (2, "short")]);
        let s = build("X 1", &[a], Mode::Stacked, false, 100);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn nothing_to_show_gives_nothing() {
        assert!(build("X 1", &[], Mode::Stacked, true, 100).is_empty());
        assert!(build("X 1", &[v("A", &[])], Mode::Stacked, true, 100).is_empty());
    }

    #[test]
    fn a_version_without_the_verse_says_so() {
        let s = build("X 1", &[v("A", &[(5, "five")]), v("B", &[])], Mode::SideBySide, true, 100);
        assert_eq!(s[0].blocks[1].text, MISSING);
    }

    #[test]
    fn at_most_four_versions() {
        let vs: Vec<Version> = (0..6).map(|i| v(&format!("V{i}"), &[(1, "a")])).collect();
        assert_eq!(build("X 1", &vs, Mode::Stacked, true, 100)[0].blocks.len(), 4);
    }

    #[test]
    fn layout_rectangles_tile_the_area_without_overlap() {
        for mode in [Mode::SideBySide, Mode::Stacked] {
            for n in 1..=4 {
                let r = rects(n, mode);
                assert_eq!(r.len(), n);
                for w in r.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    match mode {
                        Mode::SideBySide => assert!(a[0] + a[2] <= b[0] + 1e-6),
                        Mode::Stacked => assert!(a[1] + a[3] <= b[1] + 1e-6),
                    }
                }
                let last = r[n - 1];
                assert!(last[0] + last[2] <= 1.0001 && last[1] + last[3] <= 1.0001);
            }
        }
        assert!(font_scale(3, Mode::SideBySide) < font_scale(2, Mode::SideBySide));
        assert_eq!(font_scale(1, Mode::Stacked), 1.0);
    }
}
