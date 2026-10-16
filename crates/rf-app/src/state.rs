//! Shared application state types.

use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub enum StageKind {
    Blank,
    Scripture,
    Points,
    /// v0.815.0: announcement or one part of a song (words, centred)
    Slide,
    /// v0.815.0: a picture or a playing video (the pixels come from `StageArt`)
    Media,
    /// v0.815.0: countdown / stopwatch / clock (`label` = title, `text` = the numbers)
    Countdown,
}

/// What the congregation currently sees.
#[derive(Clone, Debug)]
pub struct StageContent {
    pub kind: StageKind,
    /// "John 3:16" / "John 3:16-18" / points title
    pub label: String,
    /// verse text (scripture) or joined items (points)
    pub text: String,
    pub translation: String,
    pub chapter_only: bool,
    pub updated: Option<Instant>,
}

impl Default for StageContent {
    fn default() -> Self {
        StageContent {
            kind: StageKind::Blank,
            label: String::new(),
            text: String::new(),
            translation: "KJV".into(),
            chapter_only: false,
            updated: None,
        }
    }
}

/// What the operator said in this line, so the transcript can colour-code it
/// (v0.831.0 blue/green panel: green = a scripture was detected, blue = a voice
/// command, plain = ordinary speech).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineKind {
    #[default]
    Plain,
    /// this utterance was about a scripture (staged, queued for review, or an out-of-range report)
    Verse,
    /// this utterance was a voice command (next, read, blackout...)
    Command,
}

#[derive(Clone, Debug)]
pub struct TranscriptLine {
    pub service_ms: u64,
    pub text: String,
    /// true if this line triggered an action (staged / error / confirm)
    pub flagged: bool,
    /// what kind of utterance this was (drives the line colour in the transcript panel)
    pub kind: LineKind,
}

#[derive(Clone, Debug)]
pub struct ScriptureIndexEntry {
    pub service_ms: u64,
    pub label: String,
    pub source: &'static str, // voice | confirm | manual | nav | quotation
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ServiceEvent {
    pub service_ms: u64,
    pub kind: String,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct QueuedDetection {
    pub id: u64,
    pub det: rf_core::Detection,
    /// verse preview text
    pub preview: String,
}

#[derive(Clone, Debug)]
pub struct QueuedError {
    #[allow(dead_code)]
    pub id: u64,
    pub message: String,
    pub candidates: Vec<rf_core::Passage>,
}

#[derive(Clone, Debug)]
pub enum AsrMode {
    Whisper(String),  // whisper.cpp model path
    Parakeet(String), // NVIDIA Parakeet TDT model path
    /// A local streaming-ASR service speaking the Confucius4-R2T2 WebSocket protocol
    Stream(String),
    Mock,
    Offline, // no model, no mock — simulator + meter only
}

impl AsrMode {
    /// True when a real speech model is loaded (as opposed to mock/offline).
    pub fn has_model(&self) -> bool {
        matches!(self, AsrMode::Whisper(_) | AsrMode::Parakeet(_) | AsrMode::Stream(_))
    }

    pub fn label(&self) -> &str {
        match self {
            AsrMode::Whisper(_) => "whisper (offline)",
            AsrMode::Parakeet(_) => "parakeet (offline)",
            AsrMode::Stream(_) => "streaming (R2T2 service)",
            AsrMode::Mock => "mock (demo)",
            AsrMode::Offline => "no ASR — simulator only",
        }
    }
}

/// What was just saved, for the "Service saved" window.
#[derive(Clone, Debug)]
pub struct SavedInfo {
    pub dir: std::path::PathBuf,
    pub files: Vec<(String, u64)>,
    /// this service's file-name prefix (other services the same day share the folder)
    pub prefix: String,
    pub refreshed: Instant,
    /// e.g. "saving to Downloads instead (chosen folder not writable)"
    pub note: Option<String>,
}

/// May this quotation card be staged without asking? (the "auto-approve" setting)
///
/// v0.890.0 — the operator's adjustable version of the v0.856 grid bars.
/// The DEFAULTS are still the grid's winner (5 words / 90%), chosen by the
/// whole-Bible sweep (docs/GRID-PROV19-21-v0856.md): every 4-6-word window
/// of the entire KJV (1.7 million of them, the words a preacher can say IN
/// PASSING) went through the matcher, and the shipped v0.855 rule auto-staged
/// 653 of every 1,000 — that is how "there are many devices" put Proverbs
/// 19:21 on the big screen.
///
/// The three knobs (all in the settings window, saved with the rest):
///
/// 1. `min_words` — the utterance must carry at least this many words.
///    5 (grid winner) but the operator may trade down to 3 for a livelier
///    Sunday; a fragment shorter than that may still CARD (the operator
///    decides), it just never takes the stage by itself.
/// 2. `min_pct` — the best candidate must score at least this (default 90).
/// 3. `near_tie_card` — what happens when TWO candidates both clear the bar
///    ("91 and 92"): v0.856 asked the operator (a card, via the 12-point
///    margin rule); v0.890 defaults to SHOWING THE WINNER — the 92 goes to
///    the stage, no click — and the operator can switch back to "ask me".
///
/// Everything below the bars still cards — a person, not a wrong verse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoGates {
    /// shortest utterance (words) that may auto-stage a quotation
    pub min_words: u8,
    /// accuracy bar for the best candidate
    pub min_pct: u8,
    /// true = a near-tie above the bar raises a CARD (v0.856 margin rule);
    /// false = the winner auto-stages even when the runner-up is also over
    /// the bar (the 91-vs-92 rule the operator asked for)
    pub near_tie_card: bool,
}

impl Default for AutoGates {
    fn default() -> Self {
        AutoGates { min_words: rf_core::parser::AUTO_MIN_WORDS, min_pct: rf_core::parser::AUTO_STAGE_PCT, near_tie_card: false }
    }
}

/// How many points the winner must lead by when `near_tie_card` is on
/// (v0.856's grid bar 2, kept as the card-mode margin).
pub const NEAR_TIE_MARGIN: u8 = 12;

pub fn should_auto_approve(det: &rf_core::Detection, enabled: bool) -> bool {
    should_auto_approve_g(det, enabled, &AutoGates::default())
}

/// The gates-aware form used by the live app (the settings window's values).
pub fn should_auto_approve_g(det: &rf_core::Detection, enabled: bool, g: &AutoGates) -> bool {
    if !enabled || det.kind != rf_core::DetectionKind::Quotation || det.passage.chapter == 0 {
        return false;
    }
    // grid bar 1: short fragments never auto-stage. words == 0 means
    // "unknown" (a detection path that does not carry the count) and keeps
    // the old behaviour so the accuracy bars below still bind it.
    if det.words > 0 && det.words < g.min_words {
        return false;
    }
    if det.alternatives.is_empty() {
        // single candidate: the exact-text unique tier (pct == 0) behaves as
        // before (unique in the whole KJV, the words ARE the verse's opening);
        // everything else must clear the operator's bar.
        return det.pct == 0 || det.pct >= g.min_pct;
    }
    // several candidates: is the runner-up also over the bar? ("91 and 92")
    let second = det.alt_pct.first().copied().unwrap_or(0);
    let both_over = second >= g.min_pct;
    if both_over && !g.near_tie_card {
        // v0.890.0 near-tie rule (default): both cleared the bar, the higher
        // one goes to the stage — the 92 is still auto-shown.
        return det.pct >= g.min_pct;
    }
    // card mode (or the runner-up under the bar): a clear winner — bar + 12 points ahead
    det.pct >= g.min_pct && det.pct.saturating_sub(second) >= NEAR_TIE_MARGIN
}

/// v0.853.0: suppress a quotation card for a verse that is ALREADY on stage
/// and was staged moments ago. The preacher stumbled and repeated the quote
/// — the operator said "let this mind be in you" three times and got three
/// review cards for the same passage. Within the window a repeat adds
/// nothing; after it, a genuine second showing is still offered.
pub const QUOTE_REPEAT_WINDOW_SECS: u64 = 120;

pub fn quotation_repeat_suppressed(
    passage: &rf_core::Passage,
    on_stage: Option<rf_core::Passage>,
    last_stage_at: Option<std::time::Instant>,
    now: std::time::Instant,
) -> bool {
    let Some(on_stage) = on_stage else { return false };
    if on_stage != *passage {
        return false;
    }
    match last_stage_at {
        Some(t) => now.duration_since(t).as_secs() < QUOTE_REPEAT_WINDOW_SECS,
        None => false,
    }
}

/// How the congregation stage looks (v0.812.0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StageStyle {
    /// white serif text centred on black
    #[default]
    Classic,
    /// text in a band in the lower third: for live-stream overlays
    LowerThird,
    /// solid green background for chroma-keying in OBS / ATEM
    ChromaGreen,
    /// bigger, heavier text, no reference line: for the back of a big room
    LargePrint,
}

impl StageStyle {
    pub const ALL: [StageStyle; 4] = [StageStyle::Classic, StageStyle::LowerThird, StageStyle::ChromaGreen, StageStyle::LargePrint];
    pub fn label(self) -> &'static str {
        match self {
            StageStyle::Classic => "Classic",
            StageStyle::LowerThird => "Lower third",
            StageStyle::ChromaGreen => "Green screen",
            StageStyle::LargePrint => "Large print",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            StageStyle::Classic => "classic",
            StageStyle::LowerThird => "lower",
            StageStyle::ChromaGreen => "chroma",
            StageStyle::LargePrint => "large",
        }
    }
    pub fn from_key(k: &str) -> StageStyle {
        Self::ALL.into_iter().find(|s| s.key() == k).unwrap_or_default()
    }
}

/// The three window layouts (v0.831.0, from the approved mock-up).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LayoutMode {
    /// A · Operator console: transcript left, preview + index centre, tabbed dock right (default)
    #[default]
    Console,
    /// B · Dock-first: transcript hidden, wider dock — media-heavy services
    DockFirst,
    /// C · Focus (simple mode): big preview, transcript on the right, fewest distractions
    Focus,
}

impl LayoutMode {
    pub const ALL: [LayoutMode; 3] = [LayoutMode::Console, LayoutMode::DockFirst, LayoutMode::Focus];
    pub fn label(self) -> &'static str {
        match self {
            LayoutMode::Console => "A · Operator console",
            LayoutMode::DockFirst => "B · Dock-first",
            LayoutMode::Focus => "C · Focus",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            LayoutMode::Console => "A · Operator console — transcript left, preview centre, tabbed dock right (best all-round)",
            LayoutMode::DockFirst => "B · Dock-first — transcript hidden, wider dock (media-heavy services)",
            LayoutMode::Focus => "C · Focus — big preview, transcript right, fewest distractions (simple mode)",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            LayoutMode::Console => "console",
            LayoutMode::DockFirst => "dock",
            LayoutMode::Focus => "focus",
        }
    }
    pub fn from_key(k: &str) -> LayoutMode {
        Self::ALL.into_iter().find(|s| s.key() == k).unwrap_or_default()
    }
}

/// One entry of the sermon plan (the queue of scriptures the preacher will read).
#[derive(Clone, Debug, PartialEq)]
pub struct PlanItem {
    pub passage: rf_core::Passage,
    pub label: String,
    /// already shown this service (tick)
    pub used: bool,
}

/// Pull every scripture reference out of pasted sermon notes, in order, one per
/// reference (duplicates kept: a verse may be read twice). Lines are parsed one by one.
pub fn plan_from_text(bible: &rf_bible::Bible, text: &str) -> Vec<PlanItem> {
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        // "John 3:16" style punctuation -> words the speech parser knows
        let spoken = l.replace(':', " verse ").replace(['.', ',', ';', '(', ')'], " ").replace('-', " to ");
        let o = rf_core::parse(bible, &spoken, &rf_core::ParseContext::default(), 0.0);
        let mut found: Vec<rf_core::Passage> = o.detections.iter().map(|d| d.passage).collect();
        for d in &o.confirms {
            if d.passage.chapter > 0 && d.kind != rf_core::DetectionKind::Quotation {
                found.push(d.passage);
            }
        }
        for p in found {
            if bible.exists(p.book as usize, p.chapter as usize, p.from as usize) {
                out.push(PlanItem { label: p.label(), passage: p, used: false });
            }
        }
    }
    out
}

/// Take item `i` off the list; keeps `pos` (the "next" marker) pointing at the same item.
pub fn plan_remove_at(plan: &mut Vec<PlanItem>, pos: &mut usize, i: usize) {
    if i < plan.len() {
        plan.remove(i);
        if *pos > i {
            *pos -= 1;
        }
    }
}

/// Move item `i` one place up or down; the "next" marker follows the item it was on.
pub fn plan_move_at(plan: &mut [PlanItem], pos: &mut usize, i: usize, down: bool) {
    let j = if down { i + 1 } else { i.wrapping_sub(1) };
    if i < plan.len() && j < plan.len() {
        plan.swap(i, j);
        if *pos == i {
            *pos = j;
        } else if *pos == j {
            *pos = i;
        }
    }
}

#[cfg(test)]
mod plan_tests {
    use super::*;

    fn bible() -> &'static rf_bible::Bible {
        rf_bible::Bible::embedded()
    }

    fn item(l: &str) -> PlanItem {
        PlanItem { passage: rf_core::Passage { book: 42, chapter: 3, from: 16, to: 16 }, label: l.into(), used: false }
    }

    #[test]
    fn a_typed_list_becomes_planned_scriptures_in_order_and_not_yet_used() {
        let v = plan_from_text(bible(), "John 3:16, then Romans 8:28-30\nPsalm 23");
        let labels: Vec<_> = v.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(labels.len(), 3, "{labels:?}");
        assert!(labels[0].starts_with("John 3:16") && labels[2].starts_with("Psalms 23"), "{labels:?}");
        assert!(v.iter().all(|p| !p.used));
    }

    #[test]
    fn removing_and_moving_keep_the_next_marker_on_the_same_item() {
        let mut plan = vec![item("a"), item("b"), item("c")];
        let mut pos = 1; // next = b
        plan_move_at(&mut plan, &mut pos, 1, false); // b goes up
        assert_eq!((plan[0].label.as_str(), pos), ("b", 0));
        plan_move_at(&mut plan, &mut pos, 2, true); // last cannot go down: nothing happens
        assert_eq!(plan[2].label, "c");
        plan_remove_at(&mut plan, &mut pos, 0); // remove the next item itself
        assert_eq!((plan.len(), pos), (2, 0));
        plan_remove_at(&mut plan, &mut pos, 9); // out of range: ignored
        assert_eq!(plan.len(), 2);
    }
}

pub fn mmss(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// Keep only the most recent `max` entries of an append-ordered (oldest
/// first) list, dropping the oldest ones from the front. Pulled out as a
/// small, pure, directly-testable function — `RhemaFlowApp` needs a real
/// `eframe::CreationContext` to construct, so logic that matters gets
/// factored out where it can actually be unit tested rather than only
/// exercised by hand.
pub fn cap_oldest<T>(list: &mut Vec<T>, max: usize) {
    if list.len() > max {
        list.drain(0..list.len() - max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rf_core::{Detection, DetectionKind, Passage};

    fn quote(pct: u8, alt: Vec<u8>) -> Detection {
        Detection {
            kind: DetectionKind::Quotation,
            passage: Passage::single(42, 3, 16),
            confidence: 0.62,
            alternatives: alt.iter().map(|_| Passage::single(44, 1, 1)).collect(),
            alt_pct: alt,
            pct,
            words: 0,
            note: None,
        }
    }

    /// a quotation with the utterance word count the real parser now records
    fn quote_w(pct: u8, alt: Vec<u8>, words: u8) -> Detection {
        let mut d = quote(pct, alt);
        d.words = words;
        d
    }

    #[test]
    fn auto_approve_is_off_by_default_and_needs_the_setting() {
        assert!(!should_auto_approve(&quote(0, vec![]), false));
        assert!(should_auto_approve(&quote(0, vec![]), true));
    }

    /// v0.856.0 — the Proverbs 19:21 regression, at the gate level: the
    /// utterance "there are many devices" is FOUR words and scored 89 with a
    /// 12+ lead, so the v0.855 rule auto-staged it. Both grid bars must stop it.
    #[test]
    fn prov1921_four_word_fragment_never_auto_stages() {
        // words gate first: 4 < 5, no auto-stage at ANY accuracy
        assert!(!should_auto_approve(&quote_w(89, vec![61], 4), true));
        assert!(!should_auto_approve(&quote_w(100, vec![], 4), true));
        // and the accuracy bar catches its 5-6-word siblings that score below 90
        assert!(!should_auto_approve(&quote_w(62, vec![55], 6), true));
        assert!(!should_auto_approve(&quote_w(67, vec![50], 7), true));
    }

    /// v0.856.0 grid bar 2: with several candidates the winner needs 90 AND
    /// a 12-point lead (was 75). The mock 82-vs-65 case now waits (the REAL
    /// kings-and-priests parse scores 93 and still stages — see the test
    /// below); every sub-bar case still CARDS, never vanishes.
    #[test]
    fn auto_approve_needs_the_grid_bars() {
        assert!(!should_auto_approve(&quote_w(82, vec![65], 6), true), "kings-and-priests case: 82 vs 65 now waits");
        assert!(!should_auto_approve(&quote_w(89, vec![61], 4), true), "under the word gate");
        assert!(should_auto_approve(&quote_w(95, vec![70], 8), true), "clear winner above the bar");
        assert!(should_auto_approve(&quote_w(91, vec![], 6), true), "lone candidate above the bar");
        assert!(!should_auto_approve(&quote_w(89, vec![], 6), true), "lone candidate below the bar waits");
        assert!(!should_auto_approve(&quote_w(70, vec![40], 6), true), "winner not confident enough");
        assert!(!should_auto_approve(&quote_w(55, vec![], 6), true), "lone weak match waits");
        // words == 0 (unknown count) keeps the accuracy bar binding
        assert!(!should_auto_approve(&quote(82, vec![65]), true));
        assert!(should_auto_approve(&quote(95, vec![70]), true));
    }

    /// v0.890.0 — the NEAR-TIE rule the operator asked for: two candidates both
    /// over the bar ("91 and 92"). Default = the winner (92) is still auto-shown;
    /// `near_tie_card` = the v0.856 behaviour (both wait as one card, 12-point margin).
    #[test]
    fn near_tie_both_over_the_bar() {
        // default gates (near_tie_card = false): 92 vs 91 stages
        assert!(should_auto_approve(&quote_w(92, vec![91], 6), true), "both over 90: the 92 is still auto shown");
        assert!(!should_auto_approve(&quote_w(92, vec![91], 6), false), "auto-approve off means off");
        // the 12-point margin rule is gone in the default near-tie mode...
        assert!(should_auto_approve(&quote_w(91, vec![90], 6), true), "91 vs 90: winner over the bar stages");
        // ...but under-the-bar pairs still card, whatever the mode
        assert!(!should_auto_approve(&quote_w(89, vec![88], 6), true), "both under the bar: wait");
        // the bar is >= : two candidates AT the bar are a near-tie over it
        assert!(should_auto_approve(&quote_w(90, vec![90], 6), true), "both at 90: the winner shows");
        // near_tie_card = true restores v0.856 exactly (margin 12 required)
        let card = AutoGates { min_words: 5, min_pct: 90, near_tie_card: true };
        assert!(!should_auto_approve_g(&quote_w(92, vec![91], 6), true, &card), "ask-me mode: 92 vs 91 waits");
        assert!(should_auto_approve_g(&quote_w(95, vec![70], 6), true, &card), "ask-me mode: clear winner still stages");
    }

    /// v0.890.0 — the auto bars are SETTINGS. The word floor and the % bar can
    /// be lowered (a livelier Sunday) or raised (a stricter one) without
    /// recompiling; the word floor never blocks words == 0 (unknown count).
    #[test]
    fn auto_bars_are_adjustable() {
        // 3 words / 80: a 4-word 85% fragment now stages (v0.856 carded it)
        let loose = AutoGates { min_words: 3, min_pct: 80, near_tie_card: false };
        assert!(should_auto_approve_g(&quote_w(85, vec![], 4), true, &loose));
        assert!(should_auto_approve_g(&quote_w(82, vec![65], 6), true, &loose), "82 over the 80 bar with a 17 lead");
        // still bounded: under the bar waits, short waits
        assert!(!should_auto_approve_g(&quote_w(79, vec![], 4), true, &loose));
        assert!(!should_auto_approve_g(&quote_w(95, vec![], 2), true, &loose));
        // stricter than the grid (6 words / 95) also works
        let strict = AutoGates { min_words: 6, min_pct: 95, near_tie_card: false };
        assert!(!should_auto_approve_g(&quote_w(93, vec![], 5), true, &strict), "the grid winner (5w/90) would stage this");
        assert!(should_auto_approve_g(&quote_w(96, vec![], 6), true, &strict));
    }

    /// The case from the 4 Oct screenshot: Revelation 1:6 and 5:10 tied at 73%,
    /// the card waited — that part is unchanged. The kings-and-priests quote
    /// measures 93% on the real matcher (above the v0.856.0 bar of 90 with a
    /// 12+ lead), so it STILL auto-stages — the grid bars did not cost it.
    /// Run the REAL parser, not a mock.
    #[test]
    fn kings_and_priests_still_identified_and_grace_family_waits() {
        let b = rf_bible::Bible::embedded();
        for said in [
            "and have made us kings and priests",
            "and have made us kings and priests unto god",
            "and have made us kings and priests unto god and his father",
        ] {
            let o = rf_core::parse(b, said, &rf_core::ParseContext::default(), 0.75);
            let q = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).expect("a quotation card");
            assert_eq!(q.passage.label(), "Revelation 1:6", "{said}");
            assert!(should_auto_approve(q, true), "{said}: pct={} alts={:?} must still auto-stage (93-class)", q.pct, q.alt_pct);
            assert!(!should_auto_approve(q, false), "off means off");
        }
        // the grace-of-our-Lord family (Rev 22:21 / 2 Cor 13:14 / Gal 6:18 /
        // Phil 4:23 / 1 Thess 5:28 — five near-identical KJV benedictions):
        // the parse scores the top candidate 99 with five 95s behind it. This
        // is the operator's "91 and 92" case: v0.890's DEFAULT (near-tie
        // auto-show) puts the 99 on the stage; switching the setting to
        // "ask me (card)" restores the v0.856 wait.
        let o = rf_core::parse(b, "the grace of our lord jesus christ be with you", &rf_core::ParseContext::default(), 0.75);
        let q = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).unwrap();
        assert!(should_auto_approve(q, true), "near-tie over the bar: the winner shows ({:?} {:?})", q.pct, q.alt_pct);
        let ask_me = AutoGates { near_tie_card: true, ..AutoGates::default() };
        assert!(!should_auto_approve_g(q, true, &ask_me), "ask-me mode: the same family waits ({:?} {:?})", q.pct, q.alt_pct);
    }

    /// v0.856.0 — the Proverbs 19:21 bug, end to end through the REAL parser
    /// + the REAL auto-approve rule: the preacher says "there are many
    /// devices" (about phones!) — the verse must still be FOUND (the card
    /// tells the operator) but must never take the stage by itself. The full
    /// verse read must still auto-stage.
    #[test]
    fn prov1921_fragment_cards_but_never_stages_full_verse_still_stages() {
        let b = rf_bible::Bible::embedded();
        for said in [
            "there are many devices",
            "there are many devices here today",
            "there are many devices in this room",
        ] {
            let o = rf_core::parse(b, said, &rf_core::ParseContext::default(), 0.75);
            let q = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation);
            if let Some(q) = q {
                assert_eq!(q.passage.label(), "Proverbs 19:21", "{said}");
                assert!(!should_auto_approve(q, true), "{said}: pct={} words={} must NOT auto-stage", q.pct, q.words);
            }
            // whichever way the parser scored it, nothing may AUTO-STAGE Prov 19:21
        }
        // the real read still works end to end
        let full = "there are many devices in a man's heart nevertheless the counsel of the lord that shall stand";
        let o = rf_core::parse(b, full, &rf_core::ParseContext::default(), 0.75);
        let q = o.confirms.iter().find(|d| d.kind == DetectionKind::Quotation).expect("the full read cards");
        assert_eq!(q.passage.label(), "Proverbs 19:21");
        assert!(should_auto_approve(q, true), "full verse read (pct={} words={}) must auto-stage", q.pct, q.words);
    }

    #[test]
    fn stage_style_keys_round_trip() {
        for st in StageStyle::ALL {
            assert_eq!(StageStyle::from_key(st.key()), st);
        }
        assert_eq!(StageStyle::from_key("nonsense"), StageStyle::Classic);
    }

    #[test]
    fn layout_keys_round_trip() {
        for m in LayoutMode::ALL {
            assert_eq!(LayoutMode::from_key(m.key()), m);
        }
        // unknown / older settings fall back to layout A, never a blank screen
        assert_eq!(LayoutMode::from_key("nonsense"), LayoutMode::Console);
        assert_eq!(LayoutMode::from_key(""), LayoutMode::Console);
    }

    #[test]
    fn transcript_line_kind_defaults_to_plain() {
        let l = TranscriptLine { service_ms: 1, text: "hello".into(), flagged: false, kind: LineKind::default() };
        assert_eq!(l.kind, LineKind::Plain);
    }

    #[test]
    fn plan_is_read_out_of_pasted_sermon_notes() {
        let b = rf_bible::Bible::embedded();
        let notes = "Intro\nOpen with John 3:16-18\n\nthen Romans 8:28 and\nPsalm 23:1\njust some words with no reference";
        let plan = plan_from_text(b, notes);
        let labels: Vec<&str> = plan.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(labels, vec!["John 3:16-18", "Romans 8:28", "Psalms 23:1"], "{labels:?}");
    }

    #[test]
    fn auto_approve_only_applies_to_quotations_with_a_chapter() {
        let mut d = quote(0, vec![]);
        d.kind = DetectionKind::BookOnly;
        assert!(!should_auto_approve(&d, true));
        let mut d = quote(0, vec![]);
        d.passage.chapter = 0;
        assert!(!should_auto_approve(&d, true));
    }

    #[test]
    fn cap_oldest_drops_from_the_front_once_over_the_limit() {
        let mut v: Vec<i32> = (0..5).collect();
        cap_oldest(&mut v, 10);
        assert_eq!(v, vec![0, 1, 2, 3, 4], "under the cap: untouched");

        let mut v: Vec<i32> = (0..250).collect();
        cap_oldest(&mut v, 200);
        assert_eq!(v.len(), 200);
        assert_eq!(v[0], 50, "oldest 50 dropped");
        assert_eq!(*v.last().unwrap(), 249, "newest kept");
    }

    #[test]
    fn cap_oldest_at_exactly_the_limit_is_a_no_op() {
        let mut v: Vec<i32> = (0..200).collect();
        cap_oldest(&mut v, 200);
        assert_eq!(v.len(), 200);
        assert_eq!(v[0], 0);
    }
}

#[cfg(test)]
mod v0853_tests {
    use super::*;
    use rf_core::Passage;

    #[test]
    fn quotation_repeat_suppression_window() {
        let p = Passage::single(18, 2, 5);
        let other = Passage::single(42, 3, 16);
        let now = std::time::Instant::now();
        // nothing on stage -> never suppressed
        assert!(!quotation_repeat_suppressed(&p, None, Some(now), now));
        // different verse on stage -> not suppressed
        assert!(!quotation_repeat_suppressed(&p, Some(other), Some(now), now));
        // same verse just staged -> suppressed (the "said it three times" fix)
        assert!(quotation_repeat_suppressed(&p, Some(p), Some(now), now));
        // same verse staged outside the window -> offered again
        let old = now - std::time::Duration::from_secs(QUOTE_REPEAT_WINDOW_SECS + 5);
        assert!(!quotation_repeat_suppressed(&p, Some(p), Some(old), now));
        // no timestamp known -> not suppressed
        assert!(!quotation_repeat_suppressed(&p, Some(p), None, now));
    }
}
