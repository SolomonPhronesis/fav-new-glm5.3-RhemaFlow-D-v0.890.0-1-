//! Search inside a PDF: the text of each page comes from the free `pdftotext` tool (if it is installed);
//! the search itself is plain code that is tested without any PDF.

use crate::document::find_tool;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextHit {
    /// 0-based page number
    pub page: usize,
    /// A few words around the match, on one line.
    pub snippet: String,
}

/// `pdftotext` separates pages with a form-feed character.
pub fn split_pages(raw: &str) -> Vec<String> {
    let mut v: Vec<String> = raw.split('\u{c}').map(|s| s.to_string()).collect();
    if v.len() > 1 && v.last().map_or(false, |l| l.trim().is_empty()) {
        v.pop();
    }
    v
}

/// The words of every page. An error says what to install when the tool is missing.
pub fn pdf_page_texts(src: &Path) -> Result<Vec<String>, String> {
    let bin = find_tool("pdftotext").ok_or("Searching needs the free \"poppler\" tools (in Terminal: brew install poppler).")?;
    let out = std::process::Command::new(bin).arg("-layout").arg(src).arg("-").output().map_err(|e| format!("could not run pdftotext: {e}"))?;
    if !out.status.success() {
        return Err("pdftotext could not read this PDF (it may be a scan with no text).".into());
    }
    Ok(split_pages(&String::from_utf8_lossy(&out.stdout)))
}

fn squash(s: &str) -> Vec<char> {
    let mut out = Vec::new();
    let mut sp = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !sp && !out.is_empty() {
                out.push(' ');
            }
            sp = true;
        } else {
            out.push(c);
            sp = false;
        }
    }
    out
}

/// The first match on each page (case does not matter), up to `max` pages, in page order.
pub fn search_pages(pages: &[String], query: &str, max: usize) -> Vec<TextHit> {
    let q: Vec<char> = squash(query.trim()).iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (i, p) in pages.iter().enumerate() {
        let chars = squash(p);
        let low: Vec<char> = chars.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
        if low.len() < q.len() {
            continue;
        }
        if let Some(pos) = (0..=low.len() - q.len()).find(|&s| low[s..s + q.len()] == q[..]) {
            let a = pos.saturating_sub(35);
            let b = (pos + q.len() + 35).min(chars.len());
            let mut s: String = chars[a..b].iter().collect();
            if a > 0 {
                s.insert(0, '\u{2026}');
            }
            if b < chars.len() {
                s.push('\u{2026}');
            }
            hits.push(TextHit { page: i, snippet: s });
            if hits.len() >= max {
                break;
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages() -> Vec<String> {
        vec!["Welcome to the service\n\nToday we read   John 3".into(), "Grace and PEACE to you all,\nfrom the elders".into(), "".into(), "no match here".into()]
    }

    #[test]
    fn pages_are_split_at_form_feeds_and_the_empty_tail_dropped() {
        assert_eq!(split_pages("a\u{c}b\u{c}"), vec!["a", "b"]);
        assert_eq!(split_pages("only"), vec!["only"]);
        assert_eq!(split_pages("").len(), 1);
    }

    #[test]
    fn search_finds_one_hit_per_page_ignoring_case_and_line_breaks() {
        let h = search_pages(&pages(), "peace", 10);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].page, 1);
        assert!(h[0].snippet.contains("PEACE"));
        let h = search_pages(&pages(), "john 3", 10);
        assert_eq!(h[0].page, 0, "{h:?}");
        assert!(search_pages(&pages(), "to", 10).len() == 2);
        // a phrase that spans a line break still matches
        assert_eq!(search_pages(&pages(), "you all, from", 10).len(), 1);
    }

    #[test]
    fn snippets_are_short_one_line_and_marked_when_cut() {
        let long = format!("{} needle {}", "word ".repeat(40), "tail ".repeat(40));
        let h = search_pages(&[long], "needle", 5);
        assert!(h[0].snippet.starts_with('\u{2026}') && h[0].snippet.ends_with('\u{2026}'));
        assert!(h[0].snippet.chars().count() < 100 && !h[0].snippet.contains('\n'));
    }

    #[test]
    fn empty_queries_and_limits_are_safe() {
        assert!(search_pages(&pages(), "", 10).is_empty());
        assert!(search_pages(&pages(), "   ", 10).is_empty());
        assert!(search_pages(&[], "x", 10).is_empty());
        assert_eq!(search_pages(&pages(), "e", 1).len(), 1);
        assert!(search_pages(&pages(), "a very long query that no page can hold at all", 5).is_empty());
        assert_eq!(search_pages(&["Se\u{f1}or es bueno".to_string()], "SE\u{d1}OR", 3).len(), 1);
    }
}
