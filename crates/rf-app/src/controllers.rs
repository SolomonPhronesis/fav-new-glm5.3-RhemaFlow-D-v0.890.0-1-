//! Hands-free control (v0.835.0): foot pedals, MIDI pads and Stream Deck.
//!
//! * **Foot pedals** that send keys (most do: Page Down / Page Up) work through `pedal_action`.
//! * **MIDI** notes and buttons (a pad controller, a MIDI foot switch) are mapped to actions; "learn" listens for the
//!   next press. This file has the maths and the map; the `midir` connection is in `extras.rs`.
//! * **Stream Deck** needs nothing special: its "Website" (or "API ping") button opens a web address, and RhemaFlow's
//!   built-in web server turns that into an action. `stream_deck_url` makes the address for each action.

use serde::{Deserialize, Serialize};

/// Every action a controller can run: (name, what it does in plain words). The names are the same ones the phone
/// remote and the voice commands use.
pub const ACTIONS: &[(&str, &str)] = &[
    ("next_verse", "Next verse"),
    ("prev_verse", "Previous verse"),
    ("next_chapter", "Next chapter"),
    ("prev_chapter", "Previous chapter"),
    ("next_slide", "Next slide / song part"),
    ("prev_slide", "Previous slide / song part"),
    ("plan_next", "Next planned scripture"),
    ("history_back", "Back to the previous scripture"),
    ("history_forward", "Forward to the next scripture"),
    ("approve", "Approve the first waiting suggestion"),
    ("dismiss", "Dismiss the first waiting suggestion"),
    ("blackout", "Blackout on / off"),
    ("clear", "Clear the screen"),
    ("media_play", "Play the video / sound"),
    ("media_pause", "Pause the video / sound"),
    ("media_restart", "Restart the video / sound"),
    ("runsheet_next", "Run-sheet: next step"),
    ("runsheet_prev", "Run-sheet: previous step"),
    ("runsheet_pause", "Run-sheet: pause / resume the clock"),
    ("lowerthird_show", "Lower third: show"),
    ("lowerthird_hide", "Lower third: hide"),
    ("giving_toggle", "Giving card on / off"),
    ("announce_toggle", "Announcement loop on / off"),
    ("announce_next", "Announcements: next notice"),
    ("readalong_toggle", "Read-along on / off"),
    ("captions_toggle", "Captions on / off"),
    ("clip_quick", "Clip: keep the last 30 seconds"),
    ("clip_start", "Clip: start"),
    ("clip_end", "Clip: end"),
    ("toggle_stage", "Open / close the stage window"),
    ("toggle_fullscreen", "Full screen on / off"),
    ("read", "Read the verse aloud"),
    ("stop_reading", "Stop reading aloud"),
    ("zoom_in", "Bigger text"),
    ("zoom_out", "Smaller text"),
];

pub fn is_known(action: &str) -> bool {
    ACTIONS.iter().any(|(n, _)| *n == action) || action.strip_prefix("media_").map_or(false, |n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

pub fn label_of(action: &str) -> String {
    ACTIONS.iter().find(|(n, _)| *n == action).map(|(_, l)| l.to_string()).unwrap_or_else(|| action.to_string())
}

/// The address to give a Stream Deck button ("Website" action, "GET request in background").
pub fn stream_deck_url(host: &str, port: u16, pin: &str, action: &str) -> String {
    format!("http://{host}:{port}/cmd?c={action}&pin={pin}")
}

// ---------------------------------------------------------------- pedals

/// What a pedal key press should do, given what is going on. Page Down / Space-like keys go forward, Page Up back.
pub fn pedal_action(forward: bool, runsheet_running: bool, slide_on_stage: bool) -> &'static str {
    match (forward, runsheet_running, slide_on_stage) {
        (true, true, _) => "runsheet_next",
        (false, true, _) => "runsheet_prev",
        (true, false, true) => "next_slide",
        (false, false, true) => "prev_slide",
        (true, false, false) => "next_verse",
        (false, false, false) => "prev_verse",
    }
}

// ---------------------------------------------------------------- MIDI

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Trigger {
    Note { ch: u8, n: u8 },
    Cc { ch: u8, n: u8 },
}

impl Trigger {
    pub fn describe(&self) -> String {
        match self {
            Trigger::Note { ch, n } => format!("note {} ({}) ch {}", n, note_name(*n), ch + 1),
            Trigger::Cc { ch, n } => format!("button/knob {} ch {}", n, ch + 1),
        }
    }
}

pub fn note_name(n: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", NAMES[(n % 12) as usize], (n as i32 / 12) - 1)
}

/// A raw MIDI message to a press we care about: note-on with velocity, or a controller going to 64 or more.
/// Releases (note-off, velocity 0, controller below 64) and everything else are ignored.
pub fn parse_midi(msg: &[u8]) -> Option<Trigger> {
    let (&status, rest) = msg.split_first()?;
    let ch = status & 0x0F;
    match (status & 0xF0, rest) {
        (0x90, [n, vel, ..]) if *vel > 0 => Some(Trigger::Note { ch, n: *n & 0x7F }),
        (0xB0, [n, val, ..]) if *val >= 64 => Some(Trigger::Cc { ch, n: *n & 0x7F }),
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MidiMap {
    pub entries: Vec<(Trigger, String)>,
}

impl MidiMap {
    /// A starting layout on C4 upwards: easy to learn on any pad controller.
    pub fn starter() -> MidiMap {
        let a = ["next_verse", "prev_verse", "blackout", "clear", "next_slide", "prev_slide", "runsheet_next", "runsheet_prev"];
        MidiMap { entries: a.iter().enumerate().map(|(i, name)| (Trigger::Note { ch: 0, n: 60 + i as u8 }, name.to_string())).collect() }
    }

    /// Assign `action` to `t` (replacing whatever `t` did before; one action may have several triggers).
    pub fn set(&mut self, t: Trigger, action: &str) -> Result<(), String> {
        if !is_known(action) {
            return Err(format!("\"{action}\" is not an action RhemaFlow knows."));
        }
        self.entries.retain(|(x, _)| *x != t);
        self.entries.push((t, action.to_string()));
        Ok(())
    }

    pub fn remove(&mut self, t: Trigger) {
        self.entries.retain(|(x, _)| *x != t);
    }

    pub fn action_for(&self, t: Trigger) -> Option<&str> {
        self.entries.iter().find(|(x, _)| *x == t).map(|(_, a)| a.as_str())
    }
}

/// Stops a held pedal or a bouncing switch from firing the same action over and over.
#[derive(Debug, Default)]
pub struct Debounce {
    last: Option<(Trigger, u64)>,
}

pub const DEBOUNCE_MS: u64 = 150;

impl Debounce {
    pub fn accept(&mut self, t: Trigger, now_ms: u64) -> bool {
        if let Some((lt, at)) = self.last {
            if lt == t && now_ms.saturating_sub(at) < DEBOUNCE_MS {
                return false;
            }
        }
        self.last = Some((t, now_ms));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presses_are_read_and_releases_ignored() {
        assert_eq!(parse_midi(&[0x90, 60, 100]), Some(Trigger::Note { ch: 0, n: 60 }));
        assert_eq!(parse_midi(&[0x93, 61, 1]), Some(Trigger::Note { ch: 3, n: 61 }));
        assert_eq!(parse_midi(&[0x90, 60, 0]), None, "note-on with zero velocity is a release");
        assert_eq!(parse_midi(&[0x80, 60, 64]), None, "note-off");
        assert_eq!(parse_midi(&[0xB0, 64, 127]), Some(Trigger::Cc { ch: 0, n: 64 }), "a sustain pedal pressed");
        assert_eq!(parse_midi(&[0xB0, 64, 0]), None, "pedal released");
        assert_eq!(parse_midi(&[0xF8]), None, "clock ticks");
        assert_eq!(parse_midi(&[]), None);
        assert_eq!(parse_midi(&[0x90]), None, "cut-off message");
    }

    #[test]
    fn the_map_assigns_replaces_and_refuses_unknown_actions() {
        let mut m = MidiMap::default();
        let t = Trigger::Note { ch: 0, n: 60 };
        m.set(t, "next_verse").unwrap();
        m.set(t, "blackout").unwrap();
        assert_eq!(m.entries.len(), 1);
        assert_eq!(m.action_for(t), Some("blackout"));
        assert!(m.set(t, "launch_missiles").is_err());
        m.set(Trigger::Cc { ch: 0, n: 64 }, "blackout").unwrap();
        assert_eq!(m.entries.len(), 2, "two buttons may do the same thing");
        m.remove(t);
        assert_eq!(m.action_for(t), None);
    }

    #[test]
    fn the_starter_layout_only_uses_real_actions_and_names_notes_properly() {
        let m = MidiMap::starter();
        assert_eq!(m.entries.len(), 8);
        assert!(m.entries.iter().all(|(_, a)| is_known(a)));
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(61), "C#4");
        assert!(Trigger::Note { ch: 0, n: 60 }.describe().contains("C4"));
        let j = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<MidiMap>(&j).unwrap(), m);
    }

    #[test]
    fn a_held_pedal_fires_once() {
        let mut d = Debounce::default();
        let t = Trigger::Note { ch: 0, n: 60 };
        assert!(d.accept(t, 1_000));
        assert!(!d.accept(t, 1_100));
        assert!(d.accept(Trigger::Note { ch: 0, n: 61 }, 1_100), "a different button is fine");
        assert!(d.accept(t, 1_300));
    }

    #[test]
    fn a_pedal_goes_where_the_service_is() {
        assert_eq!(pedal_action(true, true, true), "runsheet_next");
        assert_eq!(pedal_action(false, true, false), "runsheet_prev");
        assert_eq!(pedal_action(true, false, true), "next_slide");
        assert_eq!(pedal_action(false, false, false), "prev_verse");
        assert!(is_known(pedal_action(true, false, false)));
    }

    #[test]
    fn stream_deck_addresses_are_complete_and_media_items_are_known() {
        assert_eq!(stream_deck_url("192.168.1.5", 8787, "1234", "next_verse"), "http://192.168.1.5:8787/cmd?c=next_verse&pin=1234");
        assert!(is_known("media_12") && !is_known("media_") && !is_known("media_x"));
        assert_eq!(label_of("blackout"), "Blackout on / off");
        let mut names: Vec<&str> = ACTIONS.iter().map(|a| a.0).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), ACTIONS.len(), "no action listed twice");
    }
}
