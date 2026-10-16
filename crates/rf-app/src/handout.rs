//! The after-service handout (v0.835.0): one printable page (and a plain-text twin) of what happened today:
//! the scriptures that were read (with their words), the songs, the notes, the notices, a "next steps" list and a
//! giving QR code. Built from what the app already knows, so nobody types it twice.

use crate::overlay::{qr_svg, tidy_url};

#[derive(Clone, Debug, Default)]
pub struct HandoutInput {
    pub church: String,
    pub title: String,
    /// "Sunday 11th October 2026"
    pub date: String,
    /// (reference, verse text, Bible version) in the order shown; repeats are merged
    pub scriptures: Vec<(String, String, String)>,
    /// (title, CCLI number)
    pub songs: Vec<(String, String)>,
    pub notes: String,
    pub notices: Vec<(String, String)>,
    pub next_steps: Vec<String>,
    pub giving_label: String,
    pub giving_url: String,
    pub footer: String,
}

pub struct Handout {
    pub html: String,
    pub markdown: String,
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Merge scriptures shown more than once, keeping the first position.
pub fn unique_scriptures(list: &[(String, String, String)]) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for s in list {
        if !out.iter().any(|o| o.0 == s.0 && o.2 == s.2) {
            out.push(s.clone());
        }
    }
    out
}

pub fn build(h: &HandoutInput) -> Handout {
    let scr = unique_scriptures(&h.scriptures);
    let url = tidy_url(&h.giving_url);
    let title = if h.title.trim().is_empty() { "Today's service".to_string() } else { h.title.trim().to_string() };

    // ---- markdown
    let mut md = format!("# {title}\n\n");
    if !h.church.trim().is_empty() || !h.date.trim().is_empty() {
        md.push_str(&format!("{}{}{}\n\n", h.church.trim(), if h.church.trim().is_empty() || h.date.trim().is_empty() { "" } else { " \u{b7} " }, h.date.trim()));
    }
    if !scr.is_empty() {
        md.push_str("## Scriptures today\n\n");
        for (r, t, v) in &scr {
            md.push_str(&format!("**{r}**{}\n> {}\n\n", if v.is_empty() { String::new() } else { format!(" ({v})") }, t.replace('\n', " ")));
        }
    }
    if !h.notes.trim().is_empty() {
        md.push_str(&format!("## Notes\n\n{}\n\n", h.notes.trim()));
    }
    if !h.songs.is_empty() {
        md.push_str("## Songs\n\n");
        for (t, c) in &h.songs {
            md.push_str(&format!("* {t}{}\n", if c.trim().is_empty() { String::new() } else { format!(" (CCLI {})", c.trim()) }));
        }
        md.push('\n');
    }
    if !h.notices.is_empty() {
        md.push_str("## This week\n\n");
        for (t, b) in &h.notices {
            md.push_str(&format!("* **{t}**{}\n", if b.trim().is_empty() { String::new() } else { format!(" \u{2014} {}", b.trim().replace('\n', ", ")) }));
        }
        md.push('\n');
    }
    if !h.next_steps.is_empty() {
        md.push_str("## Next steps\n\n");
        for s in &h.next_steps {
            md.push_str(&format!("* {s}\n"));
        }
        md.push('\n');
    }
    if !url.is_empty() {
        md.push_str(&format!("## {}\n\n{url}\n\n", if h.giving_label.trim().is_empty() { "Give" } else { h.giving_label.trim() }));
    }
    if !h.footer.trim().is_empty() {
        md.push_str(&format!("---\n{}\n", h.footer.trim()));
    }

    // ---- html (one self-contained file: prints on one or two A4 pages, no outside files)
    let mut body = String::new();
    body.push_str(&format!("<h1>{}</h1>", esc(&title)));
    if !h.church.trim().is_empty() || !h.date.trim().is_empty() {
        body.push_str(&format!("<p class=\"meta\">{}{}{}</p>", esc(h.church.trim()), if h.church.trim().is_empty() || h.date.trim().is_empty() { "" } else { " &middot; " }, esc(h.date.trim())));
    }
    if !scr.is_empty() {
        body.push_str("<h2>Scriptures today</h2>");
        for (r, t, v) in &scr {
            body.push_str(&format!("<blockquote><b>{}</b>{}<br>{}</blockquote>", esc(r), if v.is_empty() { String::new() } else { format!(" <span class=\"v\">({})</span>", esc(v)) }, esc(&t.replace('\n', " "))));
        }
    }
    if !h.notes.trim().is_empty() {
        body.push_str("<h2>Notes</h2>");
        for para in h.notes.trim().split("\n\n") {
            body.push_str(&format!("<p>{}</p>", esc(para).replace('\n', "<br>")));
        }
    }
    if !h.songs.is_empty() {
        body.push_str("<h2>Songs</h2><ul>");
        for (t, c) in &h.songs {
            body.push_str(&format!("<li>{}{}</li>", esc(t), if c.trim().is_empty() { String::new() } else { format!(" <span class=\"v\">(CCLI {})</span>", esc(c.trim())) }));
        }
        body.push_str("</ul>");
    }
    if !h.notices.is_empty() {
        body.push_str("<h2>This week</h2><ul>");
        for (t, b) in &h.notices {
            body.push_str(&format!("<li><b>{}</b>{}</li>", esc(t), if b.trim().is_empty() { String::new() } else { format!(" &mdash; {}", esc(&b.trim().replace('\n', ", "))) }));
        }
        body.push_str("</ul>");
    }
    if !h.next_steps.is_empty() {
        body.push_str("<h2>Next steps</h2><ul>");
        for s in &h.next_steps {
            body.push_str(&format!("<li>{}</li>", esc(s)));
        }
        body.push_str("</ul>");
    }
    if let (false, Some(svg)) = (url.is_empty(), if url.is_empty() { None } else { qr_svg(&url, 170) }) {
        body.push_str(&format!("<div class=\"give\">{svg}<div><h2>{}</h2><p>{}</p></div></div>", esc(if h.giving_label.trim().is_empty() { "Give" } else { h.giving_label.trim() }), esc(url.trim_start_matches("https://"))));
    }
    if !h.footer.trim().is_empty() {
        body.push_str(&format!("<p class=\"foot\">{}</p>", esc(h.footer.trim())));
    }
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>\
body{{font:16px/1.5 Georgia,serif;color:#1b1b1b;max-width:720px;margin:32px auto;padding:0 20px}}h1{{margin:0 0 4px;font-size:30px}}h2{{font-size:18px;margin:26px 0 8px;border-bottom:2px solid #c9a24b;padding-bottom:3px}}\
.meta{{color:#666;margin:0 0 10px}}blockquote{{margin:10px 0;padding:6px 14px;border-left:4px solid #c9a24b;background:#faf6ea}}.v{{color:#777;font-size:14px}}\
.give{{display:flex;gap:20px;align-items:center;margin-top:26px;page-break-inside:avoid}}.give h2{{border:0;margin:0}}.foot{{color:#777;font-size:13px;margin-top:30px}}@media print{{body{{margin:0}}}}</style></head><body>{}</body></html>",
        esc(&title),
        body
    );
    Handout { html, markdown: md }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> HandoutInput {
        HandoutInput {
            church: "Grace Chapel".into(),
            title: "Faith & Works".into(),
            date: "Sunday 11th October 2026".into(),
            scriptures: vec![
                ("James 2:17".into(), "Even so faith, if it hath not works, is dead.".into(), "KJV".into()),
                ("John 3:16".into(), "For God so loved the world".into(), "KJV".into()),
                ("James 2:17".into(), "Even so faith, if it hath not works, is dead.".into(), "KJV".into()),
            ],
            songs: vec![("Great Is Thy Faithfulness".into(), "18723".into()), ("Amazing Grace".into(), String::new())],
            notes: "Faith shows itself in what we do.\n\nPick one thing this week.".into(),
            notices: vec![("Youth night".into(), "Friday 7pm\nthe hall".into())],
            next_steps: vec!["Join a small group".into()],
            giving_label: "Give".into(),
            giving_url: "give.gracechapel.org".into(),
            footer: "Thank you for coming".into(),
        }
    }

    #[test]
    fn a_verse_read_twice_is_listed_once_in_the_order_first_read() {
        let h = build(&input());
        assert_eq!(h.markdown.matches("**James 2:17**").count(), 1);
        assert!(h.markdown.find("James 2:17").unwrap() < h.markdown.find("John 3:16").unwrap());
        assert!(h.markdown.contains("> Even so faith"));
        assert_eq!(unique_scriptures(&input().scriptures).len(), 2);
    }

    #[test]
    fn every_part_appears_in_both_forms() {
        let h = build(&input());
        for want in ["Grace Chapel", "Sunday 11th October 2026", "Great Is Thy Faithfulness", "CCLI 18723", "Youth night", "Join a small group", "Pick one thing", "Thank you for coming", "give.gracechapel.org"] {
            assert!(h.markdown.contains(want), "markdown lacks {want}");
            assert!(h.html.contains(want), "html lacks {want}");
        }
        assert!(h.markdown.contains("Amazing Grace\n"), "a song with no CCLI number has no empty brackets");
    }

    #[test]
    fn the_page_is_safe_html_with_a_working_qr_picture() {
        let mut i = input();
        i.title = "<script>alert(1)</script> & more".into();
        i.notes = "a < b".into();
        let h = build(&i);
        assert!(!h.html.contains("<script>"), "words are escaped");
        assert!(h.html.contains("&lt;script&gt;"));
        assert!(h.html.contains("<svg") && h.html.contains("shape-rendering"));
        assert!(h.html.starts_with("<!doctype html>"));
        assert!(!h.html.contains("http://") || !h.html.contains("src="), "no outside files are loaded");
    }

    #[test]
    fn an_empty_service_still_makes_a_valid_short_page() {
        let h = build(&HandoutInput::default());
        assert!(h.markdown.starts_with("# Today's service"));
        assert!(!h.html.contains("<h2>"), "no empty sections");
        assert!(!h.html.contains("<svg"), "no QR without an address");
    }
}
