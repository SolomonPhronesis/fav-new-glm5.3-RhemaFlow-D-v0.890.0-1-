//! Feature 18: jump targets and group colours.
//!
//! "chorus", "v2", "verse 2", "3", "last", "+2", "-1", "next": what an operator types or says is turned
//! into a slide position. Section names ("v1", "Verse1", "VERSE 1") all mean the same thing.

/// Standard display name for a section label: `v1` -> `Verse 1`, `c` -> `Chorus`, `pre chorus` -> `Pre-Chorus`.
pub fn canon_label(raw: &str) -> String {
    let t = raw.trim().trim_matches(|c| c == '[' || c == ']' || c == ':' || c == '.').trim();
    if t.is_empty() {
        return String::new();
    }
    let lower = t.to_lowercase();
    let split = lower.find(|c: char| c.is_ascii_digit()).unwrap_or(lower.len());
    let (word, num) = lower.split_at(split);
    let word = word.trim().trim_end_matches('-').trim();
    let num = num.trim();
    let base = match word {
        "v" | "ve" | "vs" | "ver" | "verse" => "Verse",
        "c" | "ch" | "chor" | "chorus" | "refrain" | "r" => "Chorus",
        "p" | "pc" | "pre" | "prechorus" | "pre chorus" | "pre-chorus" | "prechor" => "Pre-Chorus",
        "b" | "br" | "bridge" => "Bridge",
        "t" | "tag" => "Tag",
        "e" | "end" | "ending" => "Ending",
        "outro" => "Outro",
        "o" | "other" => "Other",
        "i" | "intro" => "Intro",
        "inst" | "instrumental" | "interlude" | "int" => "Interlude",
        _ => "",
    };
    if base.is_empty() {
        // not a known name: keep the words, tidy the capitals
        return t.split_whitespace().map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
                None => String::new(),
            }
        }).collect::<Vec<_>>().join(" ");
    }
    if num.is_empty() {
        base.to_string()
    } else {
        let digits: String = num.chars().take_while(|c| c.is_ascii_digit()).collect();
        format!("{base} {digits}")
    }
}

/// Lower-case, single-spaced version used to compare labels.
pub fn label_key(s: &str) -> String {
    canon_label(s).to_lowercase()
}

/// The part of a label before any number: `Verse 2` -> `verse`.
fn family(label: &str) -> String {
    label_key(label).trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ').to_string()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Next,
    Prev,
    First,
    Last,
    /// 1-based slide number as people say it ("slide 3").
    Number(usize),
    /// `+2` / `-1`
    Relative(i32),
    /// A section name, with its number when one was given.
    Label(String),
}

/// Understand what was typed. `None` when it means nothing.
pub fn parse(text: &str) -> Option<Target> {
    let t = text.trim().to_lowercase();
    let t = t.trim_start_matches("go to ").trim_start_matches("goto ").trim_start_matches("jump to ").trim();
    let t = t.strip_prefix("slide ").unwrap_or(t).trim();
    if t.is_empty() {
        return None;
    }
    match t {
        "next" | "forward" | "n" => return Some(Target::Next),
        "previous" | "prev" | "back" | "p" => return Some(Target::Prev),
        "first" | "start" | "top" | "beginning" => return Some(Target::First),
        "last" | "end" | "final" => return Some(Target::Last),
        _ => {}
    }
    if let Some(rest) = t.strip_prefix('+') {
        return rest.trim().parse::<i32>().ok().filter(|n| *n != 0).map(Target::Relative);
    }
    if let Some(rest) = t.strip_prefix('-') {
        return rest.trim().parse::<i32>().ok().filter(|n| *n != 0).map(|n| Target::Relative(-n));
    }
    if let Ok(n) = t.parse::<usize>() {
        return if n >= 1 { Some(Target::Number(n)) } else { None };
    }
    let c = canon_label(t);
    let known = ["Verse", "Chorus", "Pre-Chorus", "Bridge", "Tag", "Ending", "Outro", "Intro", "Interlude"];
    if known.iter().any(|k| c.starts_with(k)) {
        return Some(Target::Label(c));
    }
    None
}

/// Where to go. `labels` are the slide labels in order (empty string = unlabelled); `current` is the slide on screen.
pub fn resolve(target: &Target, labels: &[String], current: Option<usize>) -> Option<usize> {
    let n = labels.len();
    if n == 0 {
        return None;
    }
    match target {
        Target::Next => Some(current.map_or(0, |c| (c + 1).min(n - 1))),
        Target::Prev => Some(current.map_or(0, |c| c.saturating_sub(1))),
        Target::First => Some(0),
        Target::Last => Some(n - 1),
        Target::Number(k) => (*k >= 1 && *k <= n).then(|| k - 1),
        Target::Relative(d) => {
            let c = current.unwrap_or(0) as i64 + *d as i64;
            Some(c.clamp(0, n as i64 - 1) as usize)
        }
        Target::Label(want) => {
            let key = label_key(want);
            let exact: Vec<usize> = (0..n).filter(|i| label_key(&labels[*i]) == key).collect();
            let hits = if exact.is_empty() && !key.chars().any(|c| c.is_ascii_digit()) {
                // "chorus" also matches "Chorus 2"
                (0..n).filter(|i| family(&labels[*i]) == key).collect()
            } else {
                exact
            };
            if hits.is_empty() {
                return None;
            }
            // several matches (a repeated chorus): the next one after where we are, else the first
            let after = current.map(|c| c + 1).unwrap_or(0);
            Some(hits.iter().copied().find(|i| *i >= after).unwrap_or(hits[0]))
        }
    }
}

/// A steady colour for each kind of section (operator screens only), so a long list is readable at a glance.
pub fn group_color(label: &str) -> [u8; 3] {
    match family(label).as_str() {
        "verse" => [66, 133, 244],
        "chorus" => [219, 68, 55],
        "pre-chorus" => [244, 160, 0],
        "bridge" => [15, 157, 88],
        "tag" | "ending" | "outro" | "other" => [171, 71, 188],
        "intro" | "interlude" => [0, 172, 193],
        _ => [150, 150, 150],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> Vec<String> {
        ["Verse 1", "Chorus", "Verse 2", "Chorus", "Bridge", "Chorus"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn names_are_made_standard() {
        for (raw, want) in [("v1", "Verse 1"), ("V2", "Verse 2"), ("verse1", "Verse 1"), ("[Verse 3]", "Verse 3"), ("c", "Chorus"), ("c2", "Chorus 2"), ("pre chorus", "Pre-Chorus"), ("PC", "Pre-Chorus"), ("b", "Bridge"), ("e", "Ending"), ("Amen", "Amen"), ("final   words", "Final Words"), ("", "")] {
            assert_eq!(canon_label(raw), want, "{raw}");
        }
    }

    #[test]
    fn typed_targets_are_understood() {
        assert_eq!(parse("go to chorus"), Some(Target::Label("Chorus".into())));
        assert_eq!(parse("V2"), Some(Target::Label("Verse 2".into())));
        assert_eq!(parse("slide 3"), Some(Target::Number(3)));
        assert_eq!(parse("+2"), Some(Target::Relative(2)));
        assert_eq!(parse("-1"), Some(Target::Relative(-1)));
        assert_eq!(parse("LAST"), Some(Target::Last));
        assert_eq!(parse("banana"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("0"), None);
    }

    #[test]
    fn a_repeated_chorus_goes_to_the_next_one_then_wraps() {
        let l = labels();
        let t = Target::Label("Chorus".into());
        assert_eq!(resolve(&t, &l, None), Some(1));
        assert_eq!(resolve(&t, &l, Some(1)), Some(3));
        assert_eq!(resolve(&t, &l, Some(3)), Some(5));
        assert_eq!(resolve(&t, &l, Some(5)), Some(1)); // wraps to the first
    }

    #[test]
    fn numbers_and_moves_never_leave_the_list() {
        let l = labels();
        assert_eq!(resolve(&Target::Number(1), &l, None), Some(0));
        assert_eq!(resolve(&Target::Number(7), &l, None), None);
        assert_eq!(resolve(&Target::Relative(10), &l, Some(2)), Some(5));
        assert_eq!(resolve(&Target::Relative(-10), &l, Some(2)), Some(0));
        assert_eq!(resolve(&Target::Next, &l, Some(5)), Some(5));
        assert_eq!(resolve(&Target::Last, &[], None), None);
    }

    #[test]
    fn a_family_name_finds_a_numbered_section() {
        let l: Vec<String> = ["Verse 1", "Chorus 2"].iter().map(|s| s.to_string()).collect();
        assert_eq!(resolve(&Target::Label("Chorus".into()), &l, None), Some(1));
        assert_eq!(resolve(&Target::Label("Chorus 3".into()), &l, None), None);
    }

    #[test]
    fn colours_are_steady_per_family() {
        assert_eq!(group_color("Verse 1"), group_color("verse 4"));
        assert_ne!(group_color("Verse 1"), group_color("Chorus"));
        assert_eq!(group_color("zzz"), [150, 150, 150]);
    }
}
