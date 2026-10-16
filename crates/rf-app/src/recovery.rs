//! Crash recovery (v0.835.0): the app writes a small snapshot of where the service is every few seconds, and a
//! "clean exit" marker when it is closed properly. If the next start finds a snapshot WITHOUT that marker, the
//! app (or the Mac) died mid-service, and it offers to put everything back: what was on the screen, the plan and
//! where you were in it, the run-sheet position, blackout, captions. Pure file + logic, no UI.

use crate::runsheet::RunSheet;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub saved_unix: i64,
    pub clean_exit: bool,
    pub was_live: bool,
    pub service_ms: u64,
    pub service_title: String,
    /// "blank" | "scripture" | "points" | "slide" | "media" | "countdown"
    pub stage_kind: String,
    pub stage_label: String,
    pub stage_text: String,
    pub stage_translation: String,
    /// book index, chapter, first verse, last verse of the scripture on screen
    pub stage_passage: Option<(u8, u16, u16, u16)>,
    /// the media file on screen, if any
    pub media_path: Option<String>,
    pub blackout: bool,
    pub captions_on: bool,
    pub bible_version: String,
    pub plan_text: String,
    pub plan_pos: usize,
    pub runsheet: Option<RunSheet>,
    /// the service-order item and slide that were live
    pub order_live: Option<(u64, usize)>,
    pub lower_third: Option<(String, String)>,
}

pub fn file_in(dir: &Path) -> PathBuf {
    dir.join("recovery.json")
}

pub const MAX_AGE_SECS: i64 = 12 * 3600;

impl Snapshot {
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        crate::fsutil::write_atomic(&file_in(dir), serde_json::to_string(self).unwrap_or_default().as_bytes())
    }

    pub fn load(dir: &Path) -> Option<Snapshot> {
        serde_json::from_str(&std::fs::read_to_string(file_in(dir)).ok()?).ok()
    }

    /// Say the app was closed on purpose (so the next start does not offer anything).
    pub fn mark_clean(dir: &Path) {
        if let Some(mut s) = Snapshot::load(dir) {
            s.clean_exit = true;
            let _ = s.save(dir);
        }
    }

    /// Is this snapshot worth offering back? Only after a non-clean exit, of something that mattered, in the last 12 hours.
    pub fn worth_offering(&self, now_unix: i64) -> bool {
        if self.clean_exit {
            return false;
        }
        let age = now_unix - self.saved_unix;
        if !(0..=MAX_AGE_SECS).contains(&age) {
            return false;
        }
        self.was_live || self.stage_kind != "blank" || self.runsheet.as_ref().map_or(false, |r| r.is_running()) || self.plan_pos > 0
    }

    /// Plain words for the offer.
    pub fn describe(&self, now_unix: i64) -> String {
        let mins = ((now_unix - self.saved_unix).max(0) / 60) as u64;
        let ago = if mins < 1 { "less than a minute ago".to_string() } else if mins == 1 { "1 minute ago".to_string() } else { format!("{mins} minutes ago") };
        let mut s = format!("RhemaFlow did not close properly {ago}.");
        if self.was_live {
            let m = self.service_ms / 60_000;
            s.push_str(&format!(" The service{} had been running for {m} min.", if self.service_title.is_empty() { String::new() } else { format!(" \"{}\"", self.service_title) }));
        }
        match self.stage_kind.as_str() {
            "scripture" => s.push_str(&format!(" The screen was showing {}.", self.stage_label)),
            "slide" | "points" => s.push_str(&format!(" The screen was showing \"{}\".", self.stage_label.lines().next().unwrap_or("a slide"))),
            "media" => s.push_str(&format!(" The screen was showing {}.", self.stage_label)),
            _ => {}
        }
        if let Some(r) = &self.runsheet {
            if let Some(i) = r.current {
                if let Some(st) = r.steps.get(i) {
                    s.push_str(&format!(" The run-sheet was on step {} \"{}\".", i + 1, st.title));
                }
            }
        }
        s
    }

    /// Has anything worth saving changed since `other` (ignoring the time stamp and service clock, which always move)?
    pub fn differs_from(&self, other: &Snapshot) -> bool {
        let mut a = self.clone();
        let mut b = other.clone();
        a.saved_unix = 0;
        b.saved_unix = 0;
        a.service_ms = a.service_ms / 30_000;
        b.service_ms = b.service_ms / 30_000;
        a != b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runsheet::{RunStep, StepAction};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rf-recovery-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn live() -> Snapshot {
        Snapshot { saved_unix: 1_000_000, was_live: true, service_ms: 43 * 60_000, service_title: "Sunday".into(), stage_kind: "scripture".into(), stage_label: "John 3:16".into(), stage_passage: Some((42, 3, 16, 16)), plan_pos: 2, ..Default::default() }
    }

    #[test]
    fn a_snapshot_round_trips_through_the_file() {
        let d = tmp("rt");
        let mut s = live();
        s.runsheet = Some(RunSheet { steps: vec![RunStep::new("Welcome", 60, StepAction::Note)], current: Some(0), auto: true, ..Default::default() });
        s.save(&d).unwrap();
        let back = Snapshot::load(&d).unwrap();
        assert_eq!(back.stage_passage, Some((42, 3, 16, 16)));
        assert_eq!(back.runsheet.unwrap().steps[0].title, "Welcome");
        assert!(Snapshot::load(&tmp("none")).is_none());
        std::fs::write(file_in(&d), "{broken").unwrap();
        assert!(Snapshot::load(&d).is_none(), "a damaged file is no recovery, never a crash");
    }

    #[test]
    fn only_an_unclean_recent_meaningful_session_is_offered() {
        let mut s = live();
        assert!(s.worth_offering(1_000_600));
        assert!(!s.worth_offering(1_000_000 + MAX_AGE_SECS + 1), "too old");
        assert!(!s.worth_offering(999_000), "a clock that went backwards is not trusted");
        s.clean_exit = true;
        assert!(!s.worth_offering(1_000_600), "closed properly");
        let idle = Snapshot { saved_unix: 1_000_000, stage_kind: "blank".into(), ..Default::default() };
        assert!(!idle.worth_offering(1_000_100), "nothing was going on");
    }

    #[test]
    fn closing_properly_cancels_the_offer() {
        let d = tmp("clean");
        live().save(&d).unwrap();
        assert!(!Snapshot::load(&d).unwrap().clean_exit);
        Snapshot::mark_clean(&d);
        assert!(Snapshot::load(&d).unwrap().clean_exit);
        Snapshot::mark_clean(&tmp("nofile")); // harmless
    }

    #[test]
    fn the_offer_says_what_was_happening() {
        let mut s = live();
        s.runsheet = Some(RunSheet { steps: vec![RunStep::new("Welcome", 60, StepAction::Note), RunStep::new("Reading", 60, StepAction::Note)], current: Some(1), ..Default::default() });
        let t = s.describe(1_000_000 + 5 * 60);
        assert!(t.contains("5 minutes ago") && t.contains("43 min") && t.contains("John 3:16") && t.contains("step 2 \"Reading\""), "{t}");
        assert!(live().describe(1_000_000).contains("less than a minute"));
    }

    #[test]
    fn writes_are_skipped_when_nothing_changed() {
        let a = live();
        let mut b = a.clone();
        b.saved_unix += 5;
        b.service_ms += 5_000;
        assert!(!a.differs_from(&b), "only the clock moved");
        b.stage_label = "John 3:17".into();
        assert!(a.differs_from(&b));
    }
}
