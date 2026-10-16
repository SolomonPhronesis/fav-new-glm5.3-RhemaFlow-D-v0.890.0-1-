//! Feature 8: macros. One button (or key, or spoken phrase, or the start of an item) runs a short script:
//! `next`, `wait 2s`, `black on`, `goto chorus`, `sound walk-in.mp3`, `alert Nursery 214`.
//!
//! The runner only says WHICH actions are due; the app performs them. Nothing here touches the screen or the clock.

use serde::{Deserialize, Serialize};

pub const MAX_STEPS: usize = 200;
pub const MAX_TOTAL_MS: u64 = 3_600_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Switch {
    On,
    Off,
    Toggle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Next,
    Prev,
    Clear,
    Black(Switch),
    GoTo(String),
    Sound(String),
    StopSound,
    Alert(String),
    LowerThird(String),
    /// Any other action the app already knows by name (what the voice and pedal tables use).
    Run(String),
}

impl Action {
    pub fn describe(&self) -> String {
        match self {
            Action::Next => "next slide".into(),
            Action::Prev => "previous slide".into(),
            Action::Clear => "clear the screen".into(),
            Action::Black(Switch::On) => "black out".into(),
            Action::Black(Switch::Off) => "bring the picture back".into(),
            Action::Black(Switch::Toggle) => "toggle black".into(),
            Action::GoTo(t) => format!("go to {t}"),
            Action::Sound(p) => format!("play {p}"),
            Action::StopSound => "stop the sound".into(),
            Action::Alert(t) => format!("alert: {t}"),
            Action::LowerThird(t) => format!("lower third: {t}"),
            Action::Run(n) => format!("run {n}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// Milliseconds after the step before it (the first step: after the macro starts).
    pub after_ms: u32,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub steps: Vec<Step>,
}

fn parse_wait(arg: &str) -> Option<u32> {
    let a = arg.trim().to_lowercase();
    let (num, mult) = if let Some(n) = a.strip_suffix("ms") {
        (n, 1.0)
    } else if let Some(n) = a.strip_suffix('m') {
        (n, 60_000.0)
    } else if let Some(n) = a.strip_suffix('s') {
        (n, 1000.0)
    } else {
        (a.as_str(), 1000.0)
    };
    let v: f64 = num.trim().parse().ok()?;
    if !v.is_finite() || v < 0.0 || v * mult > MAX_TOTAL_MS as f64 {
        return None;
    }
    Some((v * mult).round() as u32)
}

/// Read a script. Every problem is reported with its line number; nothing is run on a script with problems.
pub fn parse_script(text: &str) -> Result<Vec<Step>, Vec<String>> {
    let mut steps: Vec<Step> = Vec::new();
    let mut errors = Vec::new();
    let mut pending: u64 = 0;
    let mut total: u64 = 0;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let ln = n + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (cmd, rest) = match line.split_once(char::is_whitespace) {
            Some((c, r)) => (c.to_lowercase(), r.trim()),
            None => (line.to_lowercase(), ""),
        };
        let need_arg = |what: &str, errors: &mut Vec<String>| -> bool {
            if rest.is_empty() {
                errors.push(format!("line {ln}: \"{cmd}\" needs {what}"));
                false
            } else {
                true
            }
        };
        let action = match cmd.as_str() {
            "wait" | "pause" | "delay" => {
                match parse_wait(rest) {
                    Some(ms) => {
                        pending += ms as u64;
                        total += ms as u64;
                    }
                    None => errors.push(format!("line {ln}: \"wait\" needs a time such as 2s, 500ms or 1m")),
                }
                continue;
            }
            "next" => Some(Action::Next),
            "prev" | "previous" | "back" => Some(Action::Prev),
            "clear" => Some(Action::Clear),
            "black" | "blackout" => match rest.to_lowercase().as_str() {
                "" | "toggle" => Some(Action::Black(Switch::Toggle)),
                "on" => Some(Action::Black(Switch::On)),
                "off" => Some(Action::Black(Switch::Off)),
                _ => {
                    errors.push(format!("line {ln}: \"black\" takes on, off or nothing"));
                    None
                }
            },
            "goto" | "go" | "jump" => {
                let r = rest.strip_prefix("to ").unwrap_or(rest);
                if need_arg("where to go (chorus, 3, last...)", &mut errors) {
                    Some(Action::GoTo(r.to_string()))
                } else {
                    None
                }
            }
            "sound" | "play" => {
                if need_arg("a file name", &mut errors) {
                    Some(Action::Sound(rest.to_string()))
                } else {
                    None
                }
            }
            "stop" => {
                if rest.is_empty() || rest.eq_ignore_ascii_case("sound") {
                    Some(Action::StopSound)
                } else {
                    errors.push(format!("line {ln}: only \"stop sound\" is known"));
                    None
                }
            }
            "stopsound" => Some(Action::StopSound),
            "alert" | "message" => {
                if need_arg("the words to show", &mut errors) {
                    Some(Action::Alert(rest.to_string()))
                } else {
                    None
                }
            }
            "lowerthird" | "lower" => {
                let r = rest.strip_prefix("third ").unwrap_or(rest);
                if need_arg("the words to show", &mut errors) {
                    Some(Action::LowerThird(r.to_string()))
                } else {
                    None
                }
            }
            "run" => {
                if need_arg("an action name", &mut errors) {
                    Some(Action::Run(rest.to_string()))
                } else {
                    None
                }
            }
            other => {
                errors.push(format!("line {ln}: \"{other}\" is not a command (try next, wait, black, goto, sound, alert, run)"));
                None
            }
        };
        if let Some(a) = action {
            steps.push(Step { after_ms: pending.min(u32::MAX as u64) as u32, action: a });
            pending = 0;
        }
    }
    if steps.len() > MAX_STEPS {
        errors.push(format!("a macro can have at most {MAX_STEPS} steps"));
    }
    if total > MAX_TOTAL_MS {
        errors.push("a macro can wait at most one hour in total".to_string());
    }
    if errors.is_empty() {
        Ok(steps)
    } else {
        Err(errors)
    }
}

/// The steps as a script again (what the editor shows).
pub fn to_script(steps: &[Step]) -> String {
    let mut out = String::new();
    for s in steps {
        if s.after_ms > 0 {
            if s.after_ms % 1000 == 0 {
                out.push_str(&format!("wait {}s\n", s.after_ms / 1000));
            } else {
                out.push_str(&format!("wait {}ms\n", s.after_ms));
            }
        }
        let line = match &s.action {
            Action::Next => "next".to_string(),
            Action::Prev => "prev".to_string(),
            Action::Clear => "clear".to_string(),
            Action::Black(Switch::On) => "black on".to_string(),
            Action::Black(Switch::Off) => "black off".to_string(),
            Action::Black(Switch::Toggle) => "black".to_string(),
            Action::GoTo(t) => format!("goto {t}"),
            Action::Sound(p) => format!("sound {p}"),
            Action::StopSound => "stop sound".to_string(),
            Action::Alert(t) => format!("alert {t}"),
            Action::LowerThird(t) => format!("lowerthird {t}"),
            Action::Run(n) => format!("run {n}"),
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// A running macro. Ask `due(now)` on every tick; it hands back each action once.
#[derive(Clone, Debug)]
pub struct Run {
    steps: Vec<Step>,
    at: Vec<u64>,
    started_ms: u64,
    next: usize,
    cancelled: bool,
}

impl Run {
    pub fn start(m: &Macro, now_ms: u64) -> Run {
        let mut t = 0u64;
        let at = m.steps.iter().map(|s| { t += s.after_ms as u64; t }).collect();
        Run { steps: m.steps.clone(), at, started_ms: now_ms, next: 0, cancelled: false }
    }
    pub fn due(&mut self, now_ms: u64) -> Vec<Action> {
        let mut out = Vec::new();
        if self.cancelled {
            return out;
        }
        let elapsed = now_ms.saturating_sub(self.started_ms);
        while self.next < self.steps.len() && self.at[self.next] <= elapsed {
            out.push(self.steps[self.next].action.clone());
            self.next += 1;
        }
        out
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
    pub fn is_finished(&self) -> bool {
        self.cancelled || self.next >= self.steps.len()
    }
    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        if self.is_finished() {
            return 0;
        }
        self.at.last().copied().unwrap_or(0).saturating_sub(now_ms.saturating_sub(self.started_ms))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trigger {
    /// "Cmd+1" (a combo as written in the shortcuts list)
    Hotkey(String),
    /// when a slide with this label comes up
    SlideLabel(String),
    /// when an item with this title starts
    ItemStart(String),
    /// when this phrase is spoken
    Voice(String),
    ServiceStart,
    ServiceEnd,
}

fn norm_words(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Book {
    pub macros: Vec<Macro>,
    pub triggers: Vec<(Trigger, String)>,
}

impl Book {
    pub fn get(&self, name: &str) -> Option<&Macro> {
        self.macros.iter().find(|m| m.name.eq_ignore_ascii_case(name.trim()))
    }
    pub fn upsert(&mut self, m: Macro) {
        match self.macros.iter_mut().find(|x| x.name.eq_ignore_ascii_case(&m.name)) {
            Some(x) => *x = m,
            None => self.macros.push(m),
        }
    }
    /// Removing a macro also removes the triggers that pointed at it.
    pub fn remove(&mut self, name: &str) -> bool {
        let n = self.macros.len();
        self.macros.retain(|m| !m.name.eq_ignore_ascii_case(name));
        self.triggers.retain(|(_, m)| !m.eq_ignore_ascii_case(name));
        self.macros.len() != n
    }
    pub fn bind(&mut self, t: Trigger, macro_name: &str) -> Result<(), String> {
        if self.get(macro_name).is_none() {
            return Err(format!("there is no macro called \"{macro_name}\""));
        }
        if self.triggers.iter().any(|(x, m)| *x == t && m.eq_ignore_ascii_case(macro_name)) {
            return Ok(());
        }
        self.triggers.push((t, macro_name.to_string()));
        Ok(())
    }
    pub fn for_trigger(&self, t: &Trigger) -> Vec<&Macro> {
        self.triggers.iter().filter(|(x, _)| x == t).filter_map(|(_, m)| self.get(m)).collect()
    }
    /// A spoken sentence that contains a trigger phrase as whole words.
    pub fn match_voice(&self, said: &str) -> Option<&Macro> {
        let said = format!(" {} ", norm_words(said));
        self.triggers.iter().find_map(|(t, m)| match t {
            Trigger::Voice(p) => {
                let p = norm_words(p);
                if !p.is_empty() && said.contains(&format!(" {p} ")) {
                    self.get(m)
                } else {
                    None
                }
            }
            _ => None,
        })
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
    pub fn from_json(s: &str) -> Result<Book, String> {
        serde_json::from_str(s).map_err(|e| format!("not a macro file: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALKIN: &str = "# start of service\nblack on\nsound walk-in.mp3\nwait 90s\nstop sound\nblack off\nwait 500ms\nnext\ngoto chorus\n";

    #[test]
    fn a_script_becomes_steps_with_delays_between() {
        let s = parse_script(WALKIN).unwrap();
        assert_eq!(s.len(), 6);
        assert_eq!(s[0], Step { after_ms: 0, action: Action::Black(Switch::On) });
        assert_eq!(s[2], Step { after_ms: 90_000, action: Action::StopSound });
        assert_eq!(s[4], Step { after_ms: 500, action: Action::Next });
        assert_eq!(s[5].action, Action::GoTo("chorus".into()));
    }

    #[test]
    fn the_script_round_trips() {
        let s = parse_script(WALKIN).unwrap();
        assert_eq!(parse_script(&to_script(&s)).unwrap(), s);
    }

    #[test]
    fn every_mistake_is_reported_with_its_line_and_nothing_runs() {
        let e = parse_script("next\nfly away\nwait banana\nblack maybe\ngoto\nsound\nstop music\n").unwrap_err();
        assert_eq!(e.len(), 6, "{e:?}");
        assert!(e[0].contains("line 2") && e[0].contains("fly"));
        assert!(e.iter().any(|m| m.contains("line 3") && m.contains("wait")));
    }

    #[test]
    fn waits_understand_units_and_refuse_nonsense() {
        assert_eq!(parse_wait("2"), Some(2000));
        assert_eq!(parse_wait("1.5s"), Some(1500));
        assert_eq!(parse_wait("250ms"), Some(250));
        assert_eq!(parse_wait("2m"), Some(120_000));
        assert_eq!(parse_wait("-1s"), None);
        assert_eq!(parse_wait("NaNs"), None);
        assert_eq!(parse_wait("99999m"), None);
    }

    #[test]
    fn limits_stop_runaway_macros() {
        let many = "next\n".repeat(MAX_STEPS + 1);
        assert!(parse_script(&many).is_err());
        assert!(parse_script("wait 50m\nnext\nwait 50m\nnext").is_err());
        assert_eq!(parse_script("").unwrap(), vec![]);
        assert_eq!(parse_script("# only a comment\n\n").unwrap(), vec![]);
    }

    #[test]
    fn the_runner_hands_out_each_action_once_at_the_right_time() {
        let m = Macro { name: "m".into(), steps: parse_script("next\nwait 2s\nblack on\nwait 1s\nclear").unwrap() };
        let mut r = Run::start(&m, 1000);
        assert_eq!(r.due(1000), vec![Action::Next]);
        assert_eq!(r.due(1000), vec![]);
        assert_eq!(r.due(2999), vec![]);
        assert_eq!(r.remaining_ms(2999), 1001);
        assert_eq!(r.due(3000), vec![Action::Black(Switch::On)]);
        assert!(!r.is_finished());
        // a long freeze: everything late comes out, in order
        assert_eq!(r.due(60_000), vec![Action::Clear]);
        assert!(r.is_finished());
        assert_eq!(r.remaining_ms(60_000), 0);
    }

    #[test]
    fn cancelling_stops_everything_that_is_left() {
        let m = Macro { name: "m".into(), steps: parse_script("next\nwait 5s\nclear").unwrap() };
        let mut r = Run::start(&m, 0);
        r.due(0);
        r.cancel();
        assert!(r.due(99_999).is_empty());
        assert!(r.is_finished());
    }

    #[test]
    fn triggers_find_their_macros_and_follow_renames_and_deletes() {
        let mut b = Book::default();
        b.upsert(Macro { name: "Walk-in".into(), steps: parse_script("black on").unwrap() });
        b.upsert(Macro { name: "Closing".into(), steps: parse_script("clear").unwrap() });
        assert!(b.bind(Trigger::Hotkey("Cmd+1".into()), "walk-in").is_ok());
        assert!(b.bind(Trigger::Hotkey("Cmd+1".into()), "walk-in").is_ok()); // twice is fine
        assert_eq!(b.triggers.len(), 1);
        assert!(b.bind(Trigger::ServiceEnd, "nope").is_err());
        b.bind(Trigger::ServiceEnd, "Closing").unwrap();
        assert_eq!(b.for_trigger(&Trigger::Hotkey("Cmd+1".into())).len(), 1);
        assert_eq!(b.for_trigger(&Trigger::ServiceEnd)[0].name, "Closing");
        assert!(b.remove("closing"));
        assert!(b.for_trigger(&Trigger::ServiceEnd).is_empty());
        b.upsert(Macro { name: "WALK-IN".into(), steps: vec![] }); // replaces, no duplicate
        assert_eq!(b.macros.len(), 1);
    }

    #[test]
    fn spoken_phrases_match_whole_words_only() {
        let mut b = Book::default();
        b.upsert(Macro { name: "Pray".into(), steps: parse_script("black on").unwrap() });
        b.bind(Trigger::Voice("let us pray".into()), "Pray").unwrap();
        assert!(b.match_voice("Okay, let's all —  Let us PRAY.").is_some());
        assert!(b.match_voice("they were praying").is_none());
        assert!(b.match_voice("let us prayer").is_none());
        assert!(b.match_voice("").is_none());
    }

    #[test]
    fn the_book_saves_and_loads() {
        let mut b = Book::default();
        b.upsert(Macro { name: "A".into(), steps: parse_script("alert hi\nwait 1s\nrun clear_all").unwrap() });
        b.bind(Trigger::SlideLabel("Chorus".into()), "A").unwrap();
        assert_eq!(Book::from_json(&b.to_json()).unwrap(), b);
        assert!(Book::from_json("[").is_err());
    }
}
