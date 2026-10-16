//! Feature 17: your own shortcuts. "Cmd+Shift+K" is read, shown, checked against reserved system keys and against
//! shortcuts already in use, saved, and looked up. The app decides what each action name does.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Combo {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Command on a Mac (the Windows key elsewhere).
    pub cmd: bool,
    /// One capital letter or digit, or a name: Space, Enter, Esc, Tab, Left, Right, Up, Down, F1..F12, PageUp, PageDown, Home, End, Delete, Backspace.
    pub key: String,
}

fn norm_key(k: &str) -> Option<String> {
    let t = k.trim();
    if t.is_empty() {
        return None;
    }
    let l = t.to_lowercase();
    let named = match l.as_str() {
        "space" | "spacebar" => "Space",
        "enter" | "return" => "Enter",
        "esc" | "escape" => "Esc",
        "tab" => "Tab",
        "left" | "arrowleft" => "Left",
        "right" | "arrowright" => "Right",
        "up" | "arrowup" => "Up",
        "down" | "arrowdown" => "Down",
        "pageup" | "pgup" => "PageUp",
        "pagedown" | "pgdn" => "PageDown",
        "home" => "Home",
        "end" => "End",
        "delete" | "del" => "Delete",
        "backspace" => "Backspace",
        "plus" => "+",
        "minus" => "-",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.to_string());
    }
    if let Some(n) = l.strip_prefix('f') {
        if let Ok(n) = n.parse::<u8>() {
            if (1..=12).contains(&n) {
                return Some(format!("F{n}"));
            }
        }
    }
    let mut cs = t.chars();
    let c = cs.next()?;
    if cs.next().is_none() && (c.is_alphanumeric() || "[]/\\;',.`=-+".contains(c)) {
        return Some(c.to_uppercase().collect());
    }
    None
}

impl Combo {
    pub fn parse(s: &str) -> Result<Combo, String> {
        let mut c = Combo { ctrl: false, shift: false, alt: false, cmd: false, key: String::new() };
        let t = s.trim();
        let parts: Vec<&str> = if t == "+" {
            vec!["+"]
        } else if let Some(front) = t.strip_suffix("++") {
            // "Cmd++" means Cmd and the plus key
            let mut v: Vec<&str> = front.split('+').collect();
            v.push("+");
            v
        } else {
            t.split('+').collect()
        };
        for (i, p) in parts.iter().enumerate() {
            let last = i + 1 == parts.len();
            match p.trim().to_lowercase().as_str() {
                "ctrl" | "control" if !last => c.ctrl = true,
                "shift" if !last => c.shift = true,
                "alt" | "option" | "opt" if !last => c.alt = true,
                "cmd" | "command" | "meta" | "super" | "win" if !last => c.cmd = true,
                _ if last => c.key = norm_key(p).ok_or_else(|| format!("\"{}\" is not a key I know", p.trim()))?,
                other => return Err(format!("\"{other}\" is not a modifier (use Cmd, Ctrl, Alt, Shift)")),
            }
        }
        if c.key.is_empty() {
            return Err("there is no key in that shortcut".into());
        }
        Ok(c)
    }

    pub fn has_modifier(&self) -> bool {
        self.ctrl || self.alt || self.cmd
    }
}

impl std::fmt::Display for Combo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.cmd {
            write!(f, "Cmd+")?;
        }
        if self.ctrl {
            write!(f, "Ctrl+")?;
        }
        if self.alt {
            write!(f, "Alt+")?;
        }
        if self.shift {
            write!(f, "Shift+")?;
        }
        write!(f, "{}", self.key)
    }
}

/// Shortcuts the computer keeps for itself (and ones that would make a service unsafe to leave).
pub fn reserved(c: &Combo) -> Option<&'static str> {
    let k = c.key.as_str();
    let plain_cmd = c.cmd && !c.ctrl && !c.alt;
    if plain_cmd && !c.shift && ["Q", "H", "M", "Tab", "Space"].contains(&k) {
        return Some("the computer keeps this one for itself");
    }
    if c.alt && c.key == "F4" {
        return Some("Alt+F4 closes the window");
    }
    if c.ctrl && c.alt && k == "Delete" {
        return Some("the computer keeps this one for itself");
    }
    None
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindError {
    Reserved(String),
    /// Already used by this action.
    InUse(String),
    NoKey(String),
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindError::Reserved(why) => write!(f, "That shortcut cannot be used: {why}."),
            BindError::InUse(a) => write!(f, "That shortcut already runs \"{a}\"."),
            BindError::NoKey(e) => write!(f, "{e}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub combo: Combo,
    pub action: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotkeys {
    pub bindings: Vec<Binding>,
}

impl Hotkeys {
    /// The shortcuts a new install starts with (action names are the app's own).
    pub fn defaults() -> Hotkeys {
        let mut h = Hotkeys::default();
        for (k, a) in [("Space", "next"), ("Right", "next"), ("Left", "prev"), ("B", "blackout"), ("C", "clear"), ("F1", "help"), ("Cmd+K", "palette"), ("Cmd+Z", "undo"), ("Cmd+Shift+Z", "redo")] {
            let _ = h.bind(k, a);
        }
        h
    }

    pub fn lookup(&self, c: &Combo) -> Option<&str> {
        self.bindings.iter().find(|b| b.combo == *c).map(|b| b.action.as_str())
    }
    pub fn for_action(&self, action: &str) -> Vec<&Combo> {
        self.bindings.iter().filter(|b| b.action == action).map(|b| &b.combo).collect()
    }

    /// Add a shortcut. Refuses reserved keys and keys already in use (use [`Hotkeys::rebind`] to take one over).
    pub fn bind(&mut self, combo: &str, action: &str) -> Result<(), BindError> {
        let c = Combo::parse(combo).map_err(BindError::NoKey)?;
        if let Some(why) = reserved(&c) {
            return Err(BindError::Reserved(why.to_string()));
        }
        if let Some(existing) = self.lookup(&c) {
            if existing != action {
                return Err(BindError::InUse(existing.to_string()));
            }
            return Ok(());
        }
        self.bindings.push(Binding { combo: c, action: action.to_string() });
        Ok(())
    }

    /// Give the shortcut to this action, taking it away from any other. Returns the action that lost it.
    pub fn rebind(&mut self, combo: &str, action: &str) -> Result<Option<String>, BindError> {
        let c = Combo::parse(combo).map_err(BindError::NoKey)?;
        if let Some(why) = reserved(&c) {
            return Err(BindError::Reserved(why.to_string()));
        }
        let lost = self.lookup(&c).filter(|a| *a != action).map(|a| a.to_string());
        self.bindings.retain(|b| b.combo != c);
        self.bindings.push(Binding { combo: c, action: action.to_string() });
        Ok(lost)
    }

    pub fn unbind(&mut self, combo: &str) -> bool {
        let Ok(c) = Combo::parse(combo) else { return false };
        let n = self.bindings.len();
        self.bindings.retain(|b| b.combo != c);
        self.bindings.len() != n
    }

    /// Shortcuts that share a key (only possible in a hand-edited or damaged file).
    pub fn clashes(&self) -> Vec<(Combo, Vec<String>)> {
        let mut out: Vec<(Combo, Vec<String>)> = Vec::new();
        for b in &self.bindings {
            match out.iter_mut().find(|(c, _)| *c == b.combo) {
                Some((_, v)) => v.push(b.action.clone()),
                None => out.push((b.combo.clone(), vec![b.action.clone()])),
            }
        }
        out.retain(|(_, v)| v.len() > 1);
        out
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Load a saved file. Reserved keys and repeats are dropped (first one wins) and the list of what was dropped is returned.
    pub fn from_json(s: &str) -> Result<(Hotkeys, Vec<String>), String> {
        let raw: Hotkeys = serde_json::from_str(s).map_err(|e| format!("not a shortcuts file: {e}"))?;
        let mut h = Hotkeys::default();
        let mut dropped = Vec::new();
        for b in raw.bindings {
            let text = b.combo.to_string();
            if reserved(&b.combo).is_some() {
                dropped.push(format!("{text} (reserved)"));
            } else if h.lookup(&b.combo).is_some() {
                dropped.push(format!("{text} (used twice)"));
            } else {
                h.bindings.push(b);
            }
        }
        Ok((h, dropped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_are_read_in_many_spellings_and_shown_the_same_way() {
        for s in ["Cmd+Shift+K", "command + shift + k", "SHIFT+CMD+k", "meta+Shift+K"] {
            let c = Combo::parse(s).unwrap();
            assert_eq!(c.to_string(), "Cmd+Shift+K", "{s}");
        }
        assert_eq!(Combo::parse("spacebar").unwrap().to_string(), "Space");
        assert_eq!(Combo::parse("ctrl+Escape").unwrap().to_string(), "Ctrl+Esc");
        assert_eq!(Combo::parse("f5").unwrap().key, "F5");
        assert_eq!(Combo::parse("Option+Left").unwrap().to_string(), "Alt+Left");
        assert_eq!(Combo::parse("Cmd++").unwrap().key, "+");
        assert_eq!(Combo::parse("Cmd+plus").unwrap().key, "+");
    }

    #[test]
    fn bad_shortcuts_are_explained() {
        for bad in ["", "Cmd+", "Cmd+Shift", "Banana+K", "F13", "Cmd+ab", "K+K"] {
            assert!(Combo::parse(bad).is_err(), "{bad:?}");
        }
        assert!(Combo::parse("Cmd+").unwrap_err().contains("not a key"));
    }

    #[test]
    fn reserved_keys_are_refused() {
        let mut h = Hotkeys::default();
        for k in ["Cmd+Q", "Cmd+H", "Alt+F4", "Ctrl+Alt+Delete", "Cmd+Tab"] {
            assert!(matches!(h.bind(k, "x"), Err(BindError::Reserved(_))), "{k}");
        }
        assert!(h.bind("Cmd+Shift+Q", "x").is_ok());
    }

    #[test]
    fn a_key_in_use_is_refused_unless_you_take_it_over() {
        let mut h = Hotkeys::defaults();
        assert_eq!(h.bind("B", "something"), Err(BindError::InUse("blackout".into())));
        assert!(h.bind("B", "blackout").is_ok()); // same again: fine
        let lost = h.rebind("B", "something").unwrap();
        assert_eq!(lost.as_deref(), Some("blackout"));
        assert_eq!(h.lookup(&Combo::parse("b").unwrap()), Some("something"));
        assert!(h.clashes().is_empty());
        assert!(BindError::InUse("x".into()).to_string().contains("already runs"));
    }

    #[test]
    fn lookup_finds_the_action_and_one_action_may_have_several_keys() {
        let h = Hotkeys::defaults();
        assert_eq!(h.lookup(&Combo::parse("space").unwrap()), Some("next"));
        assert_eq!(h.for_action("next").len(), 2);
        assert_eq!(h.lookup(&Combo::parse("Q").unwrap()), None);
    }

    #[test]
    fn unbinding() {
        let mut h = Hotkeys::defaults();
        assert!(h.unbind("space"));
        assert!(!h.unbind("space"));
        assert!(!h.unbind("???"));
    }

    #[test]
    fn saved_files_round_trip_and_damaged_ones_are_cleaned() {
        let h = Hotkeys::defaults();
        let (back, dropped) = Hotkeys::from_json(&h.to_json()).unwrap();
        assert_eq!(back, h);
        assert!(dropped.is_empty());
        let mut bad = h.clone();
        bad.bindings.push(Binding { combo: Combo::parse("Cmd+Shift+Q").unwrap(), action: "ok".into() });
        bad.bindings.push(Binding { combo: Combo { ctrl: false, shift: false, alt: false, cmd: true, key: "Q".into() }, action: "quit".into() });
        bad.bindings.push(Binding { combo: Combo::parse("B").unwrap(), action: "again".into() });
        let (fixed, dropped) = Hotkeys::from_json(&bad.to_json()).unwrap();
        assert_eq!(dropped.len(), 2, "{dropped:?}");
        assert!(fixed.lookup(&Combo::parse("B").unwrap()) == Some("blackout"));
        assert!(Hotkeys::from_json("nope").is_err());
        assert!(!bad.clashes().is_empty());
    }
}
