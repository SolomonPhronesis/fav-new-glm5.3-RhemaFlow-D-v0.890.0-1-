//! Sermon notes -> a timed plan (v0.835.0).
//!
//! Paste the preacher's outline. Lines that carry a scripture become scripture steps (and go on the "next scripture"
//! list); a time in a line ("(8 min)", "5m", "10'") sets that step's length; headings become slide steps. Lines that
//! are neither (sub-points) are kept as notes on the step above, so nothing is lost. Everything is a suggestion the
//! operator can edit in the run-sheet.

use crate::runsheet::{RunStep, StepAction};

pub struct AutoPlan {
    pub steps: Vec<RunStep>,
    /// every scripture found, in order (duplicates kept: a verse may be read twice)
    pub refs: Vec<String>,
    pub warnings: Vec<String>,
}

/// Take a duration out of a line: "(8 min)", "8 mins", "8m", "8'", "[8 minutes]", "0:30" is NOT taken (could be a verse).
/// Returns (seconds, line without it).
pub fn take_minutes(line: &str) -> (Option<u32>, String) {
    let toks: Vec<&str> = line.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    let mut found: Option<u32> = None;
    let mut i = 0;
    while i < toks.len() {
        let raw = toks[i];
        let t = raw.trim_matches(|c| matches!(c, '(' | ')' | '[' | ']' | '~' | ',' | ';'));
        let low = t.to_lowercase();
        let unit = |s: &str| matches!(s.trim_matches(|c: char| !c.is_alphabetic() && c != '\''), "min" | "mins" | "minute" | "minutes" | "m" | "'");
        // "8m", "8min", "8'"
        let split = low.find(|c: char| !c.is_ascii_digit());
        if found.is_none() {
            if let Some(p) = split {
                if p > 0 && p <= 3 && unit(&low[p..]) && !low[p..].is_empty() {
                    if let Ok(n) = low[..p].parse::<u32>() {
                        found = Some(n * 60);
                        i += 1;
                        continue;
                    }
                }
            } else if !low.is_empty() && low.len() <= 3 && i + 1 < toks.len() {
                // "8 min"
                let nxt = toks[i + 1].trim_matches(|c| matches!(c, '(' | ')' | '[' | ']' | '~' | ',' | ';')).to_lowercase();
                if matches!(nxt.as_str(), "min" | "mins" | "minute" | "minutes") {
                    if let Ok(n) = low.parse::<u32>() {
                        found = Some(n * 60);
                        i += 2;
                        continue;
                    }
                }
            }
        }
        out.push(raw.to_string());
        i += 1;
    }
    let text = out.join(" ").trim().trim_matches(|c| matches!(c, '-' | ':' | '(' | ')' | '[' | ']' | '\u{2013}' | '\u{2014}')).trim().to_string();
    (found, text)
}

fn clean_bullet(line: &str) -> (String, bool) {
    let t = line.trim();
    let heading = t.starts_with('#') || (t.ends_with(':') && t.len() < 60);
    let t = t.trim_start_matches('#').trim();
    let t = t.trim_start_matches(|c: char| matches!(c, '-' | '*' | '\u{2022}' | '\u{25cf}')).trim();
    // "1. Intro" / "2) Point"
    let t = match t.find(|c: char| c == '.' || c == ')') {
        Some(p) if p > 0 && p <= 2 && t[..p].chars().all(|c| c.is_ascii_digit()) && t[p + 1..].starts_with(' ') => t[p + 1..].trim(),
        _ => t,
    };
    (t.to_string(), heading)
}

/// Every scripture in a line. "Romans 8:28 and Psalm 23:1" is two: the line is also tried in pieces split at
/// ";", "&", "/" and the word "and", and the larger answer wins.
pub fn refs_in(bible: &rf_bible::Bible, text: &str) -> Vec<crate::state::PlanItem> {
    let whole = crate::state::plan_from_text(bible, text);
    let spaced = text.replace(';', "\n").replace('&', "\n").replace(" / ", "\n").replace(" and ", "\n").replace(" And ", "\n");
    let parts: Vec<crate::state::PlanItem> = spaced.lines().flat_map(|l| crate::state::plan_from_text(bible, l)).collect();
    if parts.len() > whole.len() { parts } else { whole }
}

/// Build the plan. `default_scripture_secs` is used when a scripture line carries no time; `default_step_secs` for a heading with none.
pub fn build(bible: &rf_bible::Bible, notes: &str, default_scripture_secs: u32, default_step_secs: u32) -> AutoPlan {
    let mut steps: Vec<RunStep> = Vec::new();
    let mut refs: Vec<String> = Vec::new();
    let mut warnings = Vec::new();
    for raw in notes.replace("\r\n", "\n").lines() {
        if raw.trim().is_empty() {
            continue;
        }
        let (line, is_heading) = clean_bullet(raw);
        if line.is_empty() {
            continue;
        }
        let (mins, text) = take_minutes(&line);
        let found = refs_in(bible, &text);
        if !found.is_empty() {
            for f in &found {
                refs.push(f.label.clone());
            }
            let first = found[0].label.clone();
            let title = if text.trim().is_empty() { first.clone() } else { text.clone() };
            let secs = mins.unwrap_or(default_scripture_secs);
            let mut st = RunStep::new(&title, secs, StepAction::Scripture(first));
            if found.len() > 1 {
                st.note = format!("Also: {}", found[1..].iter().map(|f| f.label.clone()).collect::<Vec<_>>().join(", "));
            }
            steps.push(st);
        } else if mins.is_some() || is_heading || steps.is_empty() {
            let secs = mins.unwrap_or(default_step_secs);
            steps.push(RunStep::new(&text, secs, StepAction::Slide(text.clone())));
        } else if let Some(last) = steps.last_mut() {
            if !last.note.is_empty() {
                last.note.push('\n');
            }
            last.note.push_str(&text);
        }
    }
    if steps.is_empty() {
        warnings.push("Nothing to plan: paste the sermon outline or notes.".to_string());
    }
    // a step whose time was invented is worth saying
    if steps.iter().any(|s| s.secs == default_step_secs) && notes.to_lowercase().find("min").is_none() {
        warnings.push(format!("No times were found, so each step was given {} min. Change them in the run-sheet.", default_step_secs / 60));
    }
    AutoPlan { steps, refs, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_are_read_in_the_usual_spellings_and_removed_from_the_text() {
        assert_eq!(take_minutes("Welcome (5 min)"), (Some(300), "Welcome".into()));
        assert_eq!(take_minutes("Prayer 3m"), (Some(180), "Prayer".into()));
        assert_eq!(take_minutes("Worship 12 mins"), (Some(720), "Worship".into()));
        assert_eq!(take_minutes("Intro 10'"), (Some(600), "Intro".into()));
        assert_eq!(take_minutes("[8 minutes] Main point"), (Some(480), "Main point".into()));
        // not times
        assert_eq!(take_minutes("John 3:16").0, None);
        assert_eq!(take_minutes("Psalm 23 verse 1").0, None);
        assert_eq!(take_minutes("Romans 8:28-30 (4 min)"), (Some(240), "Romans 8:28-30".into()));
    }

    #[test]
    fn an_outline_becomes_timed_steps_with_the_scriptures_pulled_out() {
        let b = rf_bible::Bible::embedded();
        let notes = "# Welcome (5 min)\n1. Opening prayer 3m\nIntroduction: God so loved (6 min)\n- John 3:16 (4 min)\n  remember the whole verse\n- Romans 8:28 and Psalm 23:1\nClosing (2 min)\n";
        let p = build(b, notes, 120, 300);
        let titles: Vec<&str> = p.steps.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles.len(), 6, "{titles:?}");
        assert_eq!(p.steps[0].secs, 300);
        assert_eq!(p.steps[1].secs, 180);
        let john = p.steps.iter().find(|s| s.action == StepAction::Scripture("John 3:16".into())).expect("john step");
        assert_eq!(john.secs, 240);
        assert!(john.note.contains("remember the whole verse"), "a sub-point stays as a note: {:?}", john.note);
        let rom = p.steps.iter().find(|s| matches!(&s.action, StepAction::Scripture(l) if l.starts_with("Romans 8:28"))).unwrap();
        assert_eq!(rom.secs, 120, "no time given: the scripture default");
        assert!(rom.note.contains("Psalms 23:1"), "note={:?} refs={:?}", rom.note, p.refs);
        assert_eq!(p.refs, ["John 3:16", "Romans 8:28", "Psalms 23:1"]);
        assert_eq!(p.steps.last().unwrap().secs, 120);
    }

    #[test]
    fn empty_notes_say_so_and_untimed_notes_get_a_warning() {
        let b = rf_bible::Bible::embedded();
        let e = build(b, "  \n\n", 120, 300);
        assert!(e.steps.is_empty() && !e.warnings.is_empty());
        let u = build(b, "Opening\nJohn 1:1", 120, 300);
        assert!(!u.steps.is_empty());
        assert!(u.warnings.iter().any(|w| w.contains("No times")));
    }

    #[test]
    fn a_plain_sentence_under_a_scripture_is_a_note_not_a_step() {
        let b = rf_bible::Bible::embedded();
        let p = build(b, "Hebrews 11:1 (3 min)\nfaith is the substance\nand the evidence", 120, 300);
        assert_eq!(p.steps.len(), 1);
        assert_eq!(p.steps[0].note, "faith is the substance\nand the evidence");
    }
}
