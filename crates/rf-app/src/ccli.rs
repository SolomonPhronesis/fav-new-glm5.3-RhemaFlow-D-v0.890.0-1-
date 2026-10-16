//! v0.833.0: CCLI song details for the congregation screen, and the usage report.
//!
//! A church with a CCLI licence must (1) show the song's CCLI number and copyright line where people can see it and
//! (2) report which songs it used. The footer text and one report line per song per day are made here.

/// "Way Maker · Sinach · CCLI Song #7115744 · © 2015 Integrity Music · CCLI Licence #123456"
pub fn footer(title: &str, author: &str, ccli_song: &str, copyright: &str, licence: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for p in [title.trim(), author.trim()] {
        if !p.is_empty() {
            parts.push(p.to_string());
        }
    }
    let n = ccli_song.trim().trim_start_matches('#');
    if !n.is_empty() {
        parts.push(format!("CCLI Song #{n}"));
    }
    let c = copyright.trim().trim_start_matches('\u{a9}').trim();
    if !c.is_empty() {
        parts.push(format!("\u{a9} {c}"));
    }
    let l = licence.trim().trim_start_matches('#');
    if !l.is_empty() && !n.is_empty() {
        parts.push(format!("CCLI Licence #{l}"));
    }
    parts.join(" \u{b7} ")
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub const REPORT_HEADER: &str = "date,time,title,author,ccli_song_number,church_licence";

pub fn report_line(date: &str, time: &str, title: &str, author: &str, ccli_song: &str, licence: &str) -> String {
    [date, time, title, author, ccli_song.trim().trim_start_matches('#'), licence.trim().trim_start_matches('#')].iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(",")
}

/// Adds one line to `<dir>/ccli-report.csv` (the header is written the first time).
pub fn append_report(dir: &std::path::Path, line: &str) {
    use std::io::Write;
    let path = dir.join("ccli-report.csv");
    let new = !path.exists();
    let _ = std::fs::create_dir_all(dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        if new {
            let _ = writeln!(f, "{REPORT_HEADER}");
        }
        let _ = writeln!(f, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_footer_has_only_what_is_known() {
        assert_eq!(footer("Way Maker", "Sinach", "7115744", "2015 Integrity Music", "123456"), "Way Maker \u{b7} Sinach \u{b7} CCLI Song #7115744 \u{b7} \u{a9} 2015 Integrity Music \u{b7} CCLI Licence #123456");
        assert_eq!(footer("Amazing Grace", "", "", "", "123456"), "Amazing Grace", "no CCLI number = no licence line (it is public domain or not reported)");
        assert_eq!(footer("X", "Y", "#55", "\u{a9} 2001 Z", ""), "X \u{b7} Y \u{b7} CCLI Song #55 \u{b7} \u{a9} 2001 Z");
    }

    #[test]
    fn report_lines_survive_commas_and_quotes() {
        assert_eq!(report_line("2026-10-11", "10:31", "Holy, Holy", "A \"B\"", "#42", "9"), "2026-10-11,10:31,\"Holy, Holy\",\"A \"\"B\"\"\",42,9");
    }

    #[test]
    fn the_report_file_gets_a_header_once() {
        let d = std::env::temp_dir().join(format!("rf-ccli-{}", std::process::id()));
        append_report(&d, "a,b");
        append_report(&d, "c,d");
        let t = std::fs::read_to_string(d.join("ccli-report.csv")).unwrap();
        assert_eq!(t.lines().count(), 3);
        assert!(t.starts_with(REPORT_HEADER));
        let _ = std::fs::remove_dir_all(&d);
    }
}
