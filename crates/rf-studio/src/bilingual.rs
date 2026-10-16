//! Feature 20: lyrics with a translation line under each line. In the words, a line that starts with `~ ` (or `// `)
//! is the translation of the line above it. Show both, only the song's own language, or only the translation.

use crate::goto::label_key;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    /// Each line, with its translation under it (smaller).
    Both,
    /// All the first-language lines, then a gap, then all the translation lines.
    Stacked,
    Primary,
    Translation,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    /// 1.0 for the main language, smaller for the translation.
    pub scale: f32,
    pub is_translation: bool,
}

pub const TRANSLATION_SCALE: f32 = 0.7;

fn translation_of(line: &str) -> Option<&str> {
    let t = line.trim_start();
    t.strip_prefix("~ ").or_else(|| t.strip_prefix("// ")).or_else(|| t.strip_prefix("~")).map(|s| s.trim())
}

/// (main line, its translation if it has one)
pub fn split(text: &str) -> Vec<(String, Option<String>)> {
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    for l in text.lines() {
        if l.trim().is_empty() {
            continue;
        }
        match translation_of(l) {
            Some(t) if !t.is_empty() => match out.last_mut() {
                Some((_, slot @ None)) => *slot = Some(t.to_string()),
                Some((_, Some(prev))) => {
                    prev.push(' ');
                    prev.push_str(t);
                }
                None => out.push((String::new(), Some(t.to_string()))),
            },
            Some(_) => {}
            None => out.push((l.trim().to_string(), None)),
        }
    }
    out
}

pub fn has_translation(text: &str) -> bool {
    text.lines().any(|l| translation_of(l).map_or(false, |t| !t.is_empty()))
}

pub fn render(text: &str, show: Show) -> Vec<Line> {
    let pairs = split(text);
    let line = |t: &str, tr: bool| Line { text: t.to_string(), scale: if tr { TRANSLATION_SCALE } else { 1.0 }, is_translation: tr };
    let mut out = Vec::new();
    match show {
        Show::Primary => out.extend(pairs.iter().filter(|(m, _)| !m.is_empty()).map(|(m, _)| line(m, false))),
        Show::Translation => out.extend(pairs.iter().filter_map(|(m, t)| t.as_deref().map(|t| line(t, true)).or_else(|| (!m.is_empty()).then(|| line(m, false))))),
        Show::Both => {
            for (m, t) in &pairs {
                if !m.is_empty() {
                    out.push(line(m, false));
                }
                if let Some(t) = t {
                    out.push(line(t, true));
                }
            }
        }
        Show::Stacked => {
            out.extend(pairs.iter().filter(|(m, _)| !m.is_empty()).map(|(m, _)| line(m, false)));
            let trans: Vec<Line> = pairs.iter().filter_map(|(_, t)| t.as_deref().map(|t| line(t, true))).collect();
            if !trans.is_empty() {
                out.push(Line { text: String::new(), scale: 0.4, is_translation: true });
                out.extend(trans);
            }
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Combined {
    pub sections: Vec<(String, String)>,
    /// Sections whose translation had a different number of lines (so the pairing may be off).
    pub uneven: Vec<String>,
    /// Sections of the translation that match nothing in the song.
    pub unmatched: Vec<String>,
}

/// Put a translation (as separate sections with the same labels) under the lines of the song.
pub fn combine(main: &[(String, String)], translation: &[(String, String)]) -> Combined {
    let mut used = vec![false; translation.len()];
    let mut sections = Vec::new();
    let mut uneven = Vec::new();
    for (label, text) in main {
        let key = label_key(label);
        let found = translation.iter().enumerate().find(|(i, (l, _))| !used[*i] && label_key(l) == key);
        let mut out = String::new();
        match found {
            Some((i, (_, t))) => {
                used[i] = true;
                let a: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                let b: Vec<&str> = t.lines().filter(|l| !l.trim().is_empty()).collect();
                if a.len() != b.len() {
                    uneven.push(label.clone());
                }
                for k in 0..a.len().max(b.len()) {
                    if let Some(x) = a.get(k) {
                        out.push_str(x.trim());
                        out.push('\n');
                    }
                    if let Some(y) = b.get(k) {
                        out.push_str("~ ");
                        out.push_str(y.trim());
                        out.push('\n');
                    }
                }
            }
            None => {
                for l in text.lines().filter(|l| !l.trim().is_empty()) {
                    out.push_str(l.trim());
                    out.push('\n');
                }
            }
        }
        sections.push((label.clone(), out.trim_end().to_string()));
    }
    let unmatched = translation.iter().enumerate().filter(|(i, _)| !used[*i]).map(|(_, (l, _))| l.clone()).collect();
    Combined { sections, uneven, unmatched }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: &str = "Sublime gracia\n~ Amazing grace\nque dulce el son\n~ how sweet the sound\n\nsin traducir";

    #[test]
    fn lines_are_paired_with_their_translation() {
        let p = split(T);
        assert_eq!(p[0], ("Sublime gracia".into(), Some("Amazing grace".into())));
        assert_eq!(p[2], ("sin traducir".into(), None));
        assert!(has_translation(T));
        assert!(!has_translation("one\ntwo"));
        assert!(!has_translation("one\n~ \ntwo"));
    }

    #[test]
    fn both_shows_each_pair_with_the_translation_smaller() {
        let l = render(T, Show::Both);
        assert_eq!(l.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), vec!["Sublime gracia", "Amazing grace", "que dulce el son", "how sweet the sound", "sin traducir"]);
        assert_eq!(l[0].scale, 1.0);
        assert_eq!(l[1].scale, TRANSLATION_SCALE);
        assert!(l[1].is_translation && !l[0].is_translation);
    }

    #[test]
    fn primary_only_and_translation_only() {
        let p = render(T, Show::Primary);
        assert_eq!(p.len(), 3);
        assert!(p.iter().all(|l| !l.is_translation));
        let t = render(T, Show::Translation);
        // untranslated lines still appear, so the slide is never missing a line
        assert_eq!(t.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), vec!["Amazing grace", "how sweet the sound", "sin traducir"]);
    }

    #[test]
    fn stacked_puts_all_of_one_language_then_the_other() {
        let l = render(T, Show::Stacked);
        let texts: Vec<&str> = l.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(texts, vec!["Sublime gracia", "que dulce el son", "sin traducir", "", "Amazing grace", "how sweet the sound"]);
        assert_eq!(render("just one\nline", Show::Stacked).len(), 2); // no translation: no gap line
    }

    #[test]
    fn a_translation_with_no_line_above_and_odd_markers_are_safe() {
        assert_eq!(split("~ orphan")[0], (String::new(), Some("orphan".into())));
        assert_eq!(render("~ orphan", Show::Primary).len(), 0);
        assert_eq!(split("a\n// b\n~ c")[0].1.as_deref(), Some("b c"));
        assert!(render("", Show::Both).is_empty());
    }

    #[test]
    fn combine_weaves_a_translation_in_by_label() {
        let main = vec![("Verse 1".to_string(), "Sublime gracia\nque dulce".to_string()), ("Chorus".to_string(), "Coro uno".to_string())];
        let tr = vec![("verse 1".to_string(), "Amazing grace\nhow sweet".to_string()), ("Bridge".to_string(), "x".to_string())];
        let c = combine(&main, &tr);
        assert_eq!(c.sections[0].1, "Sublime gracia\n~ Amazing grace\nque dulce\n~ how sweet");
        assert_eq!(c.sections[1].1, "Coro uno");
        assert_eq!(c.unmatched, vec!["Bridge"]);
        assert!(c.uneven.is_empty());
        // what combine writes is what split reads
        assert_eq!(split(&c.sections[0].1).len(), 2);
    }

    #[test]
    fn uneven_translations_are_flagged_but_nothing_is_lost() {
        let main = vec![("Verse 1".to_string(), "a\nb\nc".to_string())];
        let tr = vec![("Verse 1".to_string(), "A\nB".to_string())];
        let c = combine(&main, &tr);
        assert_eq!(c.uneven, vec!["Verse 1"]);
        assert_eq!(c.sections[0].1.lines().count(), 5);
    }
}
