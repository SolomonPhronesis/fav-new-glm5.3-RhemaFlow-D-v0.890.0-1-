//! Feature 4: make words fit. Line breaks that look even, pages that never leave a lonely last line,
//! and the biggest font size that still fits a box.

/// Width of a character on screen, in "letters": wide characters (Chinese, Japanese, Korean) count double.
fn cw(c: char) -> usize {
    let u = c as u32;
    if (0x1100..=0x115F).contains(&u) || (0x2E80..=0xA4CF).contains(&u) || (0xAC00..=0xD7A3).contains(&u) || (0xF900..=0xFAFF).contains(&u) || (0xFE30..=0xFE6F).contains(&u) || (0xFF00..=0xFF60).contains(&u) {
        2
    } else {
        1
    }
}

pub fn display_width(s: &str) -> usize {
    s.chars().map(cw).sum()
}

/// Greedy wrap: fill each line as far as it goes. Existing line breaks are kept. A word longer than the line gets its own line.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for src in text.lines() {
        if src.trim().is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = String::new();
        let mut lw = 0;
        for w in src.split_whitespace() {
            let ww = display_width(w);
            if lw == 0 {
                line.push_str(w);
                lw = ww;
            } else if lw + 1 + ww <= width {
                line.push(' ');
                line.push_str(w);
                lw += 1 + ww;
            } else {
                out.push(std::mem::take(&mut line));
                line.push_str(w);
                lw = ww;
            }
        }
        out.push(line);
    }
    out
}

fn balance_one(src: &str, width: usize) -> Vec<String> {
    let words: Vec<&str> = src.split_whitespace().collect();
    let greedy = wrap(src, width);
    let k = greedy.len();
    if k <= 1 || words.len() < 2 {
        return greedy;
    }
    let n = words.len();
    let wl: Vec<usize> = words.iter().map(|w| display_width(w)).collect();
    let total: usize = wl.iter().sum::<usize>() + n - 1;
    let target = (total as f64 / k as f64).ceil() as i64;
    let seg = |i: usize, j: usize| -> usize { wl[i..j].iter().sum::<usize>() + (j - i - 1) };
    // dp[l][j]: least cost to put the first j words on exactly l lines
    let inf = i64::MAX / 4;
    let mut dp = vec![vec![inf; n + 1]; k + 1];
    let mut from = vec![vec![0usize; n + 1]; k + 1];
    dp[0][0] = 0;
    for l in 1..=k {
        for j in l..=n {
            for i in (l - 1)..j {
                if dp[l - 1][i] >= inf {
                    continue;
                }
                let len = seg(i, j);
                if len > width && j - i > 1 {
                    continue;
                }
                let d = len as i64 - target;
                let c = dp[l - 1][i] + d * d;
                if c < dp[l][j] {
                    dp[l][j] = c;
                    from[l][j] = i;
                }
            }
        }
    }
    if dp[k][n] >= inf {
        return greedy;
    }
    let mut lines = Vec::new();
    let mut j = n;
    for l in (1..=k).rev() {
        let i = from[l][j];
        lines.push(words[i..j].join(" "));
        j = i;
    }
    lines.reverse();
    lines
}

/// Like [`wrap`], but the lines come out nearly the same length ("even" instead of "full, then a stub").
pub fn balance(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for src in text.lines() {
        if src.trim().is_empty() {
            out.push(String::new());
        } else {
            out.extend(balance_one(src, width.max(1)));
        }
    }
    out
}

/// Split words onto pages of at most `max_lines` lines. Breaks fall between the original lines whenever they can,
/// pages come out about the same size, and a short last page is avoided.
pub fn paginate(text: &str, width: usize, max_lines: usize, balanced: bool) -> Vec<String> {
    let max_lines = max_lines.max(1);
    let groups: Vec<Vec<String>> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| if balanced { balance(l, width) } else { wrap(l, width) })
        .collect();
    let total: usize = groups.iter().map(|g| g.len()).sum();
    if total == 0 {
        return Vec::new();
    }
    let pages_wanted = (total + max_lines - 1) / max_lines;
    let target = (total + pages_wanted - 1) / pages_wanted;
    let mut pages: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for g in groups {
        if g.len() > max_lines {
            // one very long line: cut it into even chunks
            if !cur.is_empty() {
                pages.push(std::mem::take(&mut cur));
            }
            let parts = (g.len() + max_lines - 1) / max_lines;
            let each = (g.len() + parts - 1) / parts;
            for c in g.chunks(each) {
                pages.push(c.to_vec());
            }
            continue;
        }
        if !cur.is_empty() && (cur.len() + g.len() > max_lines || (cur.len() >= target && cur.len() + g.len() > target)) {
            pages.push(std::mem::take(&mut cur));
        }
        cur.extend(g);
    }
    if !cur.is_empty() {
        pages.push(cur);
    }
    // never end on a one-line page when the page before has room
    if pages.len() >= 2 {
        let last_len = pages[pages.len() - 1].len();
        let prev_len = pages[pages.len() - 2].len();
        if last_len == 1 && prev_len < max_lines {
            let l = pages.pop().unwrap();
            pages.last_mut().unwrap().extend(l);
        }
    }
    pages.into_iter().map(|p| p.join("\n")).collect()
}

/// The largest font size (points, in steps of 0.5) at which `text` fits a box `box_w` by `box_h` (any one unit).
/// `glyph_em` is the average letter width as a fraction of the font size (0.5 suits most serif fonts).
pub fn best_font_pt(text: &str, box_w: f32, box_h: f32, max_pt: f32, min_pt: f32, glyph_em: f32, line_spacing: f32) -> f32 {
    let fits = |pt: f32| -> bool {
        let per_line = ((box_w / (pt * glyph_em)).floor() as usize).max(1);
        let lines = wrap(text, per_line).len().max(1) as f32;
        lines * pt * line_spacing <= box_h
    };
    let step = 0.5f32;
    let mut lo = (min_pt / step).ceil() as i32;
    let mut hi = (max_pt / step).floor() as i32;
    if hi < lo {
        return min_pt;
    }
    if !fits(lo as f32 * step) {
        return min_pt;
    }
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if fits(mid as f32 * step) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo as f32 * step
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_keeps_breaks_and_never_loses_words() {
        let t = "Amazing grace how sweet the sound\nThat saved a wretch like me";
        let w = wrap(t, 20);
        assert!(w.iter().all(|l| display_width(l) <= 20), "{w:?}");
        assert_eq!(w.join(" ").split_whitespace().count(), 12);
        assert_eq!(wrap("a\n\nb", 10), vec!["a", "", "b"]);
        assert_eq!(wrap("supercalifragilistic", 5), vec!["supercalifragilistic"]);
    }

    #[test]
    fn balanced_lines_are_more_even_than_greedy() {
        let t = "Amazing grace how sweet the sound that saved a wretch like me";
        let g = wrap(t, 40);
        let b = balance(t, 40);
        assert_eq!(g.len(), b.len());
        let spread = |v: &Vec<String>| v.iter().map(|l| l.len()).max().unwrap() - v.iter().map(|l| l.len()).min().unwrap();
        assert!(spread(&b) < spread(&g), "{g:?} vs {b:?}");
        assert!(b.iter().all(|l| l.len() <= 40));
        assert_eq!(b.join(" "), t);
    }

    #[test]
    fn balance_of_short_or_odd_text_is_safe() {
        assert_eq!(balance("hi", 10), vec!["hi"]);
        assert_eq!(balance("", 10), Vec::<String>::new());
        assert_eq!(balance("one two", 3), vec!["one", "two"]);
        assert_eq!(balance("a b c d e f g h", 1).len(), 8);
    }

    #[test]
    fn pages_are_even_and_within_the_limit() {
        let t = (1..=8).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let p = paginate(&t, 40, 4, false);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].lines().count(), 4);
        let t5 = (1..=5).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let p5 = paginate(&t5, 40, 4, false);
        assert_eq!(p5.iter().map(|s| s.lines().count()).collect::<Vec<_>>(), vec![3, 2]);
        assert_eq!(paginate("", 40, 4, true), Vec::<String>::new());
        assert_eq!(paginate("short", 40, 4, true), vec!["short"]);
    }

    #[test]
    fn one_huge_line_is_cut_into_even_pages() {
        let t = "word ".repeat(60);
        let p = paginate(&t, 20, 3, true);
        assert!(p.len() >= 3);
        assert!(p.iter().all(|s| s.lines().count() <= 3));
        let words: usize = p.iter().map(|s| s.split_whitespace().count()).sum();
        assert_eq!(words, 60);
    }

    #[test]
    fn no_lonely_last_line_when_the_page_before_has_room() {
        let t = "aaa\nbbb\nccc\nddd\neee";
        let p = paginate(t, 40, 4, false);
        assert!(p.iter().all(|s| s.lines().count() >= 2), "{p:?}");
    }

    #[test]
    fn wide_characters_count_double() {
        assert_eq!(display_width("abc"), 3);
        assert_eq!(display_width("\u{4F60}\u{597D}"), 4);
        assert_eq!(wrap("\u{4F60}\u{597D} \u{4E16}\u{754C}", 5).len(), 2);
    }

    #[test]
    fn longer_text_gets_a_smaller_font() {
        let short = best_font_pt("Jesus wept", 1000.0, 500.0, 120.0, 20.0, 0.5, 1.2);
        let long = best_font_pt(&"For God so loved the world that he gave his only begotten Son ".repeat(3), 1000.0, 500.0, 120.0, 20.0, 0.5, 1.2);
        assert!(short > long, "{short} {long}");
        assert!(long >= 20.0);
        assert_eq!(short, 120.0);
    }

    #[test]
    fn the_chosen_size_really_fits_and_one_step_up_does_not() {
        let t = "For God so loved the world that he gave his only begotten Son that whosoever believeth in him should not perish";
        let pt = best_font_pt(t, 900.0, 400.0, 100.0, 12.0, 0.5, 1.2);
        let lines = |p: f32| wrap(t, ((900.0 / (p * 0.5)).floor() as usize).max(1)).len() as f32 * p * 1.2;
        assert!(lines(pt) <= 400.0);
        assert!(lines(pt + 0.5) > 400.0 || pt >= 100.0);
    }

    #[test]
    fn text_that_cannot_fit_returns_the_minimum() {
        assert_eq!(best_font_pt(&"word ".repeat(500), 100.0, 50.0, 80.0, 18.0, 0.5, 1.2), 18.0);
        assert_eq!(best_font_pt("x", 100.0, 100.0, 10.0, 30.0, 0.5, 1.2), 30.0);
    }
}
