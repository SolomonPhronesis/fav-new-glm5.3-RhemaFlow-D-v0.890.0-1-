//! Different looks per screen (v0.835.0): the main congregation screen plus extra screens, each with its own look.
//! A lobby TV that loops notices, an overflow room that also shows captions, a livestream feed on green with lower
//! thirds. Pure settings and rules; `app.rs` opens a window for each enabled screen.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Lobby,
    Overflow,
    Livestream,
    Mirror,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Lobby, Role::Overflow, Role::Livestream, Role::Mirror];
    pub fn label(self) -> &'static str {
        match self {
            Role::Lobby => "Lobby / foyer",
            Role::Overflow => "Overflow room",
            Role::Livestream => "Livestream feed",
            Role::Mirror => "Mirror of the main screen",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Role::Lobby => "Notices and the giving card; scripture and song words are left off.",
            Role::Overflow => "Everything the main screen shows, plus captions at the bottom.",
            Role::Livestream => "Words on a band over green (for OBS / a switcher), captions and lower thirds.",
            Role::Mirror => "Exactly what the main screen shows.",
        }
    }
}

/// What kinds of content a screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    Scripture,
    Slide,
    Media,
    Card,
    Countdown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenProfile {
    pub name: String,
    pub role: Role,
    pub enabled: bool,
    /// the display (by name) it opens on; None = the first one that is not the main screen
    pub display: Option<String>,
    pub fullscreen: bool,
    pub captions: bool,
    pub lower_thirds: bool,
    /// does blackout blank this screen too?
    pub follow_blackout: bool,
    /// text size multiplier on top of the main one
    pub zoom: f32,
    /// own colours (bg, text); None = the main screen's
    pub colors: Option<([u8; 3], [u8; 3])>,
    /// `StageStyle` key; None = the main screen's
    pub style: Option<String>,
}

impl ScreenProfile {
    pub fn for_role(role: Role) -> ScreenProfile {
        let base = ScreenProfile { name: role.label().to_string(), role, enabled: false, display: None, fullscreen: false, captions: false, lower_thirds: false, follow_blackout: true, zoom: 1.0, colors: None, style: None };
        match role {
            Role::Lobby => ScreenProfile { zoom: 0.9, ..base },
            Role::Overflow => ScreenProfile { captions: true, lower_thirds: true, ..base },
            Role::Livestream => ScreenProfile { captions: true, lower_thirds: true, follow_blackout: false, style: Some("lower".into()), colors: Some(([0, 177, 64], [255, 255, 255])), ..base },
            Role::Mirror => ScreenProfile { lower_thirds: true, ..base },
        }
    }

    /// Does this screen show this kind of content?
    pub fn shows(&self, c: Content) -> bool {
        match self.role {
            Role::Lobby => matches!(c, Content::Slide | Content::Card | Content::Countdown | Content::Media),
            Role::Overflow | Role::Mirror => true,
            Role::Livestream => matches!(c, Content::Scripture | Content::Slide | Content::Card),
        }
    }
}

/// Which of the connected displays an extra screen should use: its own choice if still connected, otherwise the first
/// one that the main screen is not on. `main` and the result are indexes into the display list.
pub fn choose_display(names: &[String], wanted: Option<&str>, main: Option<usize>) -> Option<usize> {
    if let Some(w) = wanted {
        if let Some(i) = names.iter().position(|n| n == w) {
            return Some(i);
        }
    }
    (0..names.len()).find(|i| Some(*i) != main)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_role_starts_with_a_sensible_look() {
        let l = ScreenProfile::for_role(Role::Lobby);
        assert!(!l.shows(Content::Scripture) && l.shows(Content::Slide) && l.shows(Content::Card));
        let o = ScreenProfile::for_role(Role::Overflow);
        assert!(o.captions && o.shows(Content::Scripture) && o.shows(Content::Media));
        let s = ScreenProfile::for_role(Role::Livestream);
        assert!(s.captions && s.lower_thirds && !s.follow_blackout && s.colors.is_some() && s.style.as_deref() == Some("lower"));
        assert!(!s.shows(Content::Media), "a picture or video is not sent to the stream feed");
        assert!(ScreenProfile::for_role(Role::Mirror).shows(Content::Countdown));
        assert!(Role::ALL.iter().all(|r| !ScreenProfile::for_role(*r).enabled), "nothing opens until asked");
    }

    #[test]
    fn an_extra_screen_never_lands_on_the_main_screen_by_accident() {
        let n: Vec<String> = ["Built-in", "Projector", "TV"].iter().map(|s| s.to_string()).collect();
        assert_eq!(choose_display(&n, Some("TV"), Some(1)), Some(2));
        assert_eq!(choose_display(&n, Some("Gone"), Some(1)), Some(0), "its screen was unplugged: first other screen");
        assert_eq!(choose_display(&n, None, Some(0)), Some(1));
        assert_eq!(choose_display(&n[..1].to_vec(), None, Some(0)), None, "only one screen: nowhere to put it");
        assert_eq!(choose_display(&[], None, None), None);
    }

    #[test]
    fn profiles_survive_saving() {
        let p = ScreenProfile::for_role(Role::Livestream);
        let j = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<ScreenProfile>(&j).unwrap(), p);
    }
}
