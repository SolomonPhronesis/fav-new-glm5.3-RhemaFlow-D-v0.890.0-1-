//! v0.835.0 features, wired into the app: lower thirds, giving card, announcement loop, run-sheet with auto-advance,
//! read-along, clips, series looks, caption translation, MIDI / pedal control, accessibility, volunteer lock,
//! crash recovery, backup, extra screens. The rules live in their own modules (tested on their own); this file holds
//! the state, the per-frame work, and what each action does. The window is in `extras_ui.rs`.

use crate::access::{Access, StageFont};
use crate::announce::Loop;
use crate::clips::ClipBook;
use crate::controllers::{Debounce, MidiMap};
use crate::lock::Lock;
use crate::overlay::{Card, LowerThird};
use crate::recovery::Snapshot;
use crate::runsheet::{RunSheet, StepAction};
use crate::screens::{Content, ScreenProfile};
use crate::state::*;
use crate::themes::ThemeBook;
use crate::translate;
use crate::app::RhemaFlowApp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XTab {
    Songs,
    LowerThird,
    Cards,
    Announce,
    RunSheet,
    ReadAlong,
    Clips,
    Handout,
    Looks,
    Translate,
    Controllers,
    Access,
    Backup,
    Lock,
    Studio,
}

impl XTab {
    pub const ALL: [XTab; 15] = [
        XTab::Songs, XTab::LowerThird, XTab::Cards, XTab::Announce, XTab::RunSheet, XTab::ReadAlong, XTab::Clips, XTab::Handout, XTab::Looks, XTab::Translate, XTab::Controllers, XTab::Access, XTab::Backup, XTab::Lock, XTab::Studio,
    ];
    /// What a volunteer may use while the app is locked.
    pub const LOCKED_OK: [XTab; 7] = [XTab::LowerThird, XTab::Cards, XTab::Announce, XTab::RunSheet, XTab::ReadAlong, XTab::Clips, XTab::Lock];
    pub fn label(self) -> &'static str {
        match self {
            XTab::Songs => "Song words",
            XTab::LowerThird => "Lower thirds",
            XTab::Cards => "Giving & cards",
            XTab::Announce => "Announcements",
            XTab::RunSheet => "Run-sheet",
            XTab::ReadAlong => "Read-along",
            XTab::Clips => "Clips",
            XTab::Handout => "Handout",
            XTab::Looks => "Looks & screens",
            XTab::Translate => "Translate",
            XTab::Controllers => "Pedal / MIDI / Deck",
            XTab::Access => "Accessibility",
            XTab::Backup => "Backup",
            XTab::Lock => "Lock",
            XTab::Studio => "Studio",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardSrc {
    None,
    Giving,
    ReadAlong,
    Custom,
}

/// Everything remembered between runs (`~/.rhemaflow/extras.json`).
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Persist {
    pub lt_name: String,
    pub lt_role: String,
    pub lt_style: String,
    pub lt_left: bool,
    pub lt_hold_secs: u32,
    pub lt_accent: [u8; 3],
    pub auto_ref_lt: bool,
    pub giving_headline: String,
    pub giving_message: String,
    pub giving_url: String,
    pub announce_text: String,
    pub announce_secs: u32,
    pub series_title: String,
    pub tr_provider: String,
    pub tr_endpoint: String,
    pub tr_key: String,
    pub tr_target: String,
    pub tr_enabled: bool,
    pub midi: MidiMap,
    pub midi_port: String,
    pub midi_on: bool,
    pub access: Access,
    pub lock: Lock,
    pub screens: Vec<ScreenProfile>,
    pub backup_dir: String,
    pub backup_media: bool,
    pub backup_keep: usize,
    pub auto_backup: bool,
    pub last_backup: i64,
    pub next_steps: String,
    pub handout_notes: String,
    pub sermon_notes: String,
    pub scripture_secs: u32,
    pub step_secs: u32,
    pub run_auto: bool,
}

impl Default for Persist {
    fn default() -> Self {
        Persist {
            lt_name: String::new(),
            lt_role: String::new(),
            lt_style: "bar".into(),
            lt_left: true,
            lt_hold_secs: 8,
            lt_accent: [224, 178, 90],
            auto_ref_lt: false,
            giving_headline: "Give".into(),
            giving_message: "Thank you for your generosity".into(),
            giving_url: String::new(),
            announce_text: String::new(),
            announce_secs: 10,
            series_title: String::new(),
            tr_provider: "libre".into(),
            tr_endpoint: translate::Provider::Libre.default_endpoint().into(),
            tr_key: String::new(),
            tr_target: "es".into(),
            tr_enabled: false,
            midi: MidiMap::starter(),
            midi_port: String::new(),
            midi_on: false,
            access: Access::default(),
            lock: Lock::default(),
            screens: Vec::new(),
            backup_dir: String::new(),
            backup_media: false,
            backup_keep: 14,
            auto_backup: false,
            last_backup: 0,
            next_steps: String::new(),
            handout_notes: String::new(),
            sermon_notes: String::new(),
            scripture_secs: 120,
            step_secs: 300,
            run_auto: true,
        }
    }
}

pub struct ReadAlong {
    pub book: usize,
    pub chapter: usize,
    pub verses: Vec<String>,
    pub current: usize,
    pub seen_transcript: usize,
}

/// What the stage windows paint on top of the content.
#[derive(Clone)]
pub struct Overlays {
    pub alerts: Vec<(String, [u8; 3])>,
    pub lt: LowerThird,
    pub lt_alpha: f32,
    pub card: Option<Card>,
}

pub struct Extras {
    pub studio: crate::studio::Studio,
    pub open: bool,
    pub tab: XTab,
    pub p: Persist,
    pub themes: ThemeBook,
    pub t0: Instant,
    pub lt: LowerThird,
    pub card: Option<Card>,
    pub card_src: CardSrc,
    pub custom_title: String,
    pub custom_lines: String,
    pub announce: Loop,
    pub runsheet: RunSheet,
    pub clips: ClipBook,
    pub ra: Option<ReadAlong>,
    pub tr_shared: Arc<Mutex<(String, String)>>,
    pub tr_error: Arc<Mutex<Option<String>>>,
    tr_sent: String,
    pub midi_conn: Option<midir::MidiInputConnection<()>>,
    pub midi_tx: crossbeam_channel::Sender<Vec<u8>>,
    pub midi_rx: crossbeam_channel::Receiver<Vec<u8>>,
    pub midi_ports: Vec<String>,
    pub midi_status: String,
    pub learning: Option<String>,
    debounce: Debounce,
    pub queue: Vec<String>,
    pub recovery_offer: Option<Snapshot>,
    last_snap: Option<Snapshot>,
    last_snap_at: Instant,
    pub data_dir: PathBuf,
    font_applied: StageFont,
    zoom_applied: f32,
    contrast_applied: bool,
    seq_seen: u64,
    pub pick_rx: Option<std::sync::mpsc::Receiver<Option<String>>>,
    // temporary form state
    pub autoplan_msgs: Vec<String>,
    pub theme_name: String,
    pub pin_new: String,
    pub pin_try: String,
    pub lock_msg: String,
    pub handout_msg: String,
    pub backup_msg: String,
    pub clips_msg: String,
    pub step_title: String,
    pub step_mins: u32,
    pub step_kind: usize,
    pub step_text: String,
    pub tr_test: String,
    pub backup_busy: Arc<std::sync::atomic::AtomicBool>,
    pub backup_result: Arc<Mutex<Option<String>>>,
    pub started_tick: bool,
    pub lock_said_at: Option<Instant>,
}

/// Action names this file handles (run_named hands them over; a locked app still allows them).
pub fn is_x_action(n: &str) -> bool {
    matches!(
        n,
        "runsheet_next" | "runsheet_prev" | "runsheet_pause" | "lowerthird_show" | "lowerthird_hide" | "giving_toggle" | "announce_toggle" | "announce_next" | "readalong_toggle" | "captions_toggle" | "clip_start" | "clip_end" | "clip_quick"
    )
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Extras {
    pub fn load() -> Extras {
        let dir = crate::fsutil::data_dir().unwrap_or_else(std::env::temp_dir);
        let p: Persist = std::fs::read_to_string(dir.join("extras.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let themes = ThemeBook::load(&dir.join("themes.json"));
        let (midi_tx, midi_rx) = crossbeam_channel::unbounded();
        let offer = Snapshot::load(&dir).filter(|s| s.worth_offering(unix_now()));
        let mut announce = Loop::default();
        announce.items = crate::announce::parse(&p.announce_text, p.announce_secs);
        let mut lt = LowerThird::default();
        lt.style = crate::overlay::L3Style::from_key(&p.lt_style);
        lt.left = p.lt_left;
        lt.accent = p.lt_accent;
        lt.hold_ms = p.lt_hold_secs as u64 * 1000;
        let runsheet = RunSheet { auto: p.run_auto, ..Default::default() };
        Extras {
            studio: crate::studio::Studio::load(&dir),
            open: false,
            tab: XTab::LowerThird,
            p,
            themes,
            t0: Instant::now(),
            lt,
            card: None,
            card_src: CardSrc::None,
            custom_title: String::new(),
            custom_lines: String::new(),
            announce,
            runsheet,
            clips: ClipBook::default(),
            ra: None,
            tr_shared: Arc::new(Mutex::new((String::new(), String::new()))),
            tr_error: Arc::new(Mutex::new(None)),
            tr_sent: String::new(),
            midi_conn: None,
            midi_tx,
            midi_rx,
            midi_ports: Vec::new(),
            midi_status: String::new(),
            learning: None,
            debounce: Debounce::default(),
            queue: Vec::new(),
            recovery_offer: offer,
            last_snap: None,
            last_snap_at: Instant::now(),
            data_dir: dir,
            font_applied: StageFont::Serif,
            zoom_applied: 1.0,
            contrast_applied: false,
            seq_seen: 0,
            pick_rx: None,
            autoplan_msgs: Vec::new(),
            theme_name: String::new(),
            pin_new: String::new(),
            pin_try: String::new(),
            lock_msg: String::new(),
            handout_msg: String::new(),
            backup_msg: String::new(),
            clips_msg: String::new(),
            step_title: String::new(),
            step_mins: 5,
            step_kind: 0,
            step_text: String::new(),
            tr_test: String::new(),
            backup_busy: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            backup_result: Arc::new(Mutex::new(None)),
            started_tick: false,
            lock_said_at: None,
        }
    }

    pub fn now_ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }

    pub fn save(&self) {
        let _ = crate::fsutil::write_atomic(&self.data_dir.join("extras.json"), serde_json::to_string_pretty(&self.p).unwrap_or_default().as_bytes());
        let _ = self.themes.store(&self.data_dir.join("themes.json"));
    }

    pub fn translate_cfg(&self) -> translate::Config {
        translate::Config { provider: translate::Provider::from_key(&self.p.tr_provider), endpoint: self.p.tr_endpoint.clone(), api_key: self.p.tr_key.clone(), target: self.p.tr_target.clone(), enabled: self.p.tr_enabled }
    }

    /// The caption the screens show: the translation of the current line when there is one.
    pub fn caption_for_stage(&self, original: &str) -> String {
        if !self.p.tr_enabled || original.is_empty() {
            return original.to_string();
        }
        let g = self.tr_shared.lock().map(|g| g.clone()).unwrap_or_default();
        translate::caption_text(original, if g.0 == original { Some(g.1.as_str()) } else { None })
    }

    pub fn overlays(&self, blackout: bool) -> Overlays {
        let now = self.now_ms();
        Overlays { alerts: self.studio.alert_lines(blackout), lt: self.lt.clone(), lt_alpha: if blackout { 0.0 } else { self.lt.alpha(now) }, card: if blackout { None } else { self.card.clone() } }
    }

    pub fn is_locked(&self) -> bool {
        self.p.lock.locked
    }
}

/// Paint lower third and card on top of a stage. `fg`/`bg` are the screen's own colours.
pub fn paint_overlays(ui: &egui::Ui, rect: egui::Rect, ov: &Overlays, fg: [u8; 3], bg: [u8; 3], show_lt: bool) {
    if let Some(c) = &ov.card {
        crate::overlay::draw_card(ui, rect, c, fg, bg, 1.0);
    }
    if show_lt {
        crate::overlay::draw_lower_third(ui, rect, &ov.lt, ov.lt_alpha);
    }
    crate::studio_ui::draw_alerts(ui, rect, &ov.alerts);
}

fn content_of(k: &StageKind) -> Content {
    match k {
        StageKind::Scripture | StageKind::Points => Content::Scripture,
        StageKind::Slide => Content::Slide,
        StageKind::Media => Content::Media,
        StageKind::Countdown => Content::Countdown,
        StageKind::Blank => Content::Slide,
    }
}

impl RhemaFlowApp {
    fn toast(&mut self, m: impl Into<String>) {
        self.short_utterance_note = Some((m.into(), Instant::now()));
    }

    // ------------------------------------------------------------------ per frame

    pub fn x_tick(&mut self, ctx: &egui::Context) {
        self.studio_tick(ctx);
        self.studio_monitor(ctx);
        let now = self.x.now_ms();
        // ---- accessibility: font, window size
        let acc = self.x.p.access;
        if acc.font != self.x.font_applied {
            crate::app::load_stage_fonts_with(ctx, acc.font);
            self.x.font_applied = acc.font;
        }
        if (acc.ui_zoom() - self.x.zoom_applied).abs() > 0.001 {
            ctx.set_zoom_factor(acc.ui_zoom());
            self.x.zoom_applied = acc.ui_zoom();
        }
        let hc = acc.contrast != crate::access::Contrast::Off;
        if hc != self.x.contrast_applied {
            self.x.contrast_applied = hc;
        }
        // ---- the lock
        self.present.locked = self.x.is_locked();
        if self.x.is_locked() {
            self.x_enforce_lock(ctx);
        }
        // ---- a folder chosen in the file dialog
        if let Some(rx) = &self.x.pick_rx {
            if let Ok(res) = rx.try_recv() {
                self.x.pick_rx = None;
                if let Some(p) = res {
                    self.x.p.backup_dir = p;
                    self.x.save();
                }
            }
        }
        // ---- pedal keys
        if !ctx.wants_keyboard_input() {
            let (down, up) = ctx.input(|i| (i.key_pressed(egui::Key::PageDown), i.key_pressed(egui::Key::PageUp)));
            if down || up {
                let name = crate::controllers::pedal_action(down, self.x.runsheet.is_running(), self.stage.kind == StageKind::Slide);
                self.x.queue.push(name.to_string());
            }
        }
        // ---- MIDI (reconnect the device used last time)
        if self.x.p.midi_on && self.x.midi_conn.is_none() && self.x.midi_status.is_empty() {
            self.x_midi_connect();
        }
        self.x_midi_poll(now);
        // ---- queued actions (from the phone, Stream Deck, MIDI, pedals, run-sheet buttons)
        let q = std::mem::take(&mut self.x.queue);
        for a in q {
            self.x_action(ctx, &a);
        }
        // ---- announcement loop yields to the operator, then ticks
        if self.x.announce.yield_if_taken(self.stage_seq) {
            self.toast("Announcements paused: something else was put on the screen.");
        }
        if let Some(i) = self.x.announce.tick(now) {
            self.x_show_notice(i);
        }
        // ---- run-sheet
        if let Some(i) = self.x.runsheet.tick(now) {
            self.x_apply_step(ctx, i);
        }
        // ---- automatic lower third for a newly staged verse
        if self.stage_seq != self.x.seq_seen {
            self.x.seq_seen = self.stage_seq;
            if self.x.p.auto_ref_lt && self.stage.kind == StageKind::Scripture {
                let (a, b) = crate::overlay::scripture_lines(&self.stage.label, &self.stage.translation);
                self.x_show_lower_third(a, b, 6_000);
            }
            self.x_follow_read_along();
        }
        self.x_read_along_listen();
        // ---- caption translation
        self.x_translate_tick();
        // ---- daily backup, snapshot for crash recovery
        self.x_backup_tick();
        self.x_snapshot_tick();
        // keep the screen alive while something moves
        if self.x.lt.is_showing(now) || self.x.announce.running || (self.x.runsheet.is_running() && !self.x.runsheet.is_paused()) {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if self.x.midi_conn.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// While locked, the windows a helper must not use are closed again.
    fn x_enforce_lock(&mut self, ctx: &egui::Context) {
        let mut shut = false;
        for flag in [&mut self.save_visible, &mut self.versions_visible, &mut self.web_visible, &mut self.preflight_open] {
            if *flag {
                *flag = false;
                shut = true;
            }
        }
        if self.account.open {
            self.account.open = false;
            shut = true;
        }
        if self.present.open {
            self.present.open = false;
            shut = true;
        }
        if shut {
            let msg = crate::lock::refusal(crate::lock::Guarded::Settings);
            self.toast(msg);
        }
        // a locked app does not close
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.toast(crate::lock::refusal(crate::lock::Guarded::Quit));
        }
        self.order.set_show_mode(true);
    }

    // ------------------------------------------------------------------ actions

    /// Everything a button, key, pedal, MIDI pad, Stream Deck or phone can ask for that this file knows about.
    pub fn x_action(&mut self, ctx: &egui::Context, name: &str) {
        let now = self.x.now_ms();
        match name {
            "runsheet_next" => {
                match self.x.runsheet.next(now) {
                    Some(i) => self.x_apply_step(ctx, i),
                    None => self.toast("That is the last step."),
                }
            }
            "runsheet_prev" => {
                if let Some(i) = self.x.runsheet.prev(now) {
                    self.x_apply_step(ctx, i);
                }
            }
            "runsheet_pause" => {
                if self.x.runsheet.is_paused() {
                    self.x.runsheet.resume(now);
                } else {
                    self.x.runsheet.pause(now);
                }
            }
            "lowerthird_show" => {
                let hold = self.x.lt.hold_ms;
                let (a, b) = (self.x.lt.line1.clone(), self.x.lt.line2.clone());
                if a.trim().is_empty() && !self.x.p.lt_name.trim().is_empty() {
                    let (n, r) = crate::overlay::speaker_lines(&self.x.p.lt_name, &self.x.p.lt_role);
                    self.x_show_lower_third(n, r, hold);
                } else {
                    self.x_show_lower_third(a, b, hold);
                }
            }
            "lowerthird_hide" => self.x.lt.hide(now),
            "giving_toggle" => self.x_toggle_giving(),
            "announce_toggle" => {
                if self.x.announce.running {
                    self.x.announce.stop();
                } else {
                    self.x.announce.items = crate::announce::parse(&self.x.p.announce_text, self.x.p.announce_secs);
                    match self.x.announce.start(now) {
                        Some(i) => self.x_show_notice(i),
                        None => self.toast("There are no announcements yet. Type some in Tools > Announcements."),
                    }
                }
            }
            "announce_next" => {
                if let Some(i) = self.x.announce.step(1, now) {
                    self.x_show_notice(i);
                }
            }
            "readalong_toggle" => {
                if self.x.ra.is_some() {
                    self.x_stop_read_along();
                } else {
                    self.x_start_read_along();
                }
            }
            "captions_toggle" => self.captions_on = !self.captions_on,
            "clip_start" => {
                let t = self.service_ms();
                self.x.clips.mark_in(t);
                self.x.clips_msg = "Clip start marked.".into();
                self.toast("Clip start marked");
            }
            "clip_end" => {
                let t = self.service_ms();
                match self.x.clips.mark_out(t, &self.export_lines) {
                    Ok(_) => {
                        self.x.clips_msg = "Clip saved to the list.".into();
                        self.toast("Clip saved");
                    }
                    Err(e) => self.toast(e),
                }
            }
            "clip_quick" => {
                let t = self.service_ms();
                match self.x.clips.quick(t, crate::clips::QUICK_BACK_MS, &self.export_lines) {
                    Ok(_) => self.toast("Kept the last 30 seconds as a clip"),
                    Err(e) => self.toast(e),
                }
            }
            other => {
                // not ours: the ordinary actions (next verse, blackout ...)
                if !is_x_action(other) {
                    self.run_named(other);
                }
            }
        }
    }

    pub fn x_show_lower_third(&mut self, line1: String, line2: String, hold_ms: u64) {
        let now = self.x.now_ms();
        self.x.lt.line1 = line1;
        self.x.lt.line2 = line2;
        self.x.lt.hold_ms = hold_ms;
        self.x.lt.show(now);
    }

    fn x_show_notice(&mut self, i: usize) {
        let Some(item) = self.x.announce.items.get(i).cloned() else { return };
        self.show_content(StageContent { kind: StageKind::Slide, label: item.title.clone(), text: item.screen_text(), translation: String::new(), chapter_only: false, updated: Some(Instant::now()) });
        self.blackout = false;
        self.x.announce.last_seq = self.stage_seq;
        self.x.seq_seen = self.stage_seq;
    }

    pub fn x_toggle_giving(&mut self) {
        if self.x.card_src == CardSrc::Giving {
            self.x.card = None;
            self.x.card_src = CardSrc::None;
            return;
        }
        if self.x.p.giving_url.trim().is_empty() {
            self.toast("Type the giving web address first (Tools > Giving & cards).");
            return;
        }
        let c = Card::giving(&self.x.p.giving_headline, &self.x.p.giving_message, &self.x.p.giving_url);
        if c.qr.is_none() {
            self.toast("That web address is too long for a QR code.");
            return;
        }
        self.x_stop_read_along();
        self.x.card = Some(c);
        self.x.card_src = CardSrc::Giving;
    }

    pub fn x_show_custom_card(&mut self) {
        let lines: Vec<&str> = self.x.custom_lines.lines().collect();
        let c = Card::info(&self.x.custom_title, &lines);
        self.x_stop_read_along();
        self.x.card = Some(c);
        self.x.card_src = CardSrc::Custom;
    }

    pub fn x_hide_card(&mut self) {
        self.x.card = None;
        self.x.card_src = CardSrc::None;
        self.x.ra = None;
    }

    // ------------------------------------------------------------------ run-sheet

    pub fn x_apply_step(&mut self, ctx: &egui::Context, i: usize) {
        let Some(step) = self.x.runsheet.steps.get(i).cloned() else { return };
        self.x_hide_card();
        match step.action {
            StepAction::Note => {}
            StepAction::Clear => {
                self.clear_stage();
                self.stage_seq += 1;
            }
            StepAction::Scripture(label) => {
                let found = crate::autoplan::refs_in(self.bible, &label);
                match found.first() {
                    Some(f) => self.do_stage(f.passage, "run-sheet", 1.0, false),
                    None => self.toast(format!("Could not find \"{label}\" in the Bible.")),
                }
            }
            StepAction::Slide(text) => {
                self.show_content(StageContent { kind: StageKind::Slide, label: step.title.clone(), text, translation: String::new(), chapter_only: false, updated: Some(Instant::now()) });
                self.blackout = false;
            }
            StepAction::Order(id) => {
                if self.order.runner.go(id, 0).is_ok() {
                    self.order_apply(ctx);
                } else {
                    self.toast("That service-order item no longer exists.");
                }
            }
            StepAction::Media(id) => self.pending_media = Some(id),
            StepAction::Announcements => {
                let now = self.x.now_ms();
                self.x.announce.items = crate::announce::parse(&self.x.p.announce_text, self.x.p.announce_secs);
                if let Some(k) = self.x.announce.start(now) {
                    self.x_show_notice(k);
                }
            }
            StepAction::Giving => {
                self.x.card_src = CardSrc::None;
                self.x_toggle_giving();
            }
        }
        // a step that put something on the screen must not be undone by the loop noticing "someone else" changed it
        self.x.announce.last_seq = if self.x.announce.running { self.stage_seq } else { self.x.announce.last_seq };
    }

    /// Replace the run-sheet from pasted sermon notes; the scriptures also go on the "next scripture" list.
    pub fn x_autoplan(&mut self, notes: &str) {
        let plan = crate::autoplan::build(self.bible, notes, self.x.p.scripture_secs, self.x.p.step_secs);
        self.x.autoplan_msgs = plan.warnings.clone();
        if plan.steps.is_empty() {
            return;
        }
        let n = plan.steps.len();
        let refs = plan.refs.join("\n");
        self.x.runsheet.steps = plan.steps;
        self.x.runsheet.stop();
        if !refs.is_empty() {
            self.plan_text = refs;
            self.load_plan();
        }
        let total = self.x.runsheet.total_secs() / 60;
        self.x.autoplan_msgs.insert(0, format!("Made {n} steps ({total} min) and put {} scriptures on the next-scripture list.", plan.refs.len()));
    }

    /// The service order as run-sheet steps (5 minutes each; timers keep their own length).
    pub fn x_runsheet_from_order(&mut self) {
        use rf_service::model::ItemKind;
        let items: Vec<_> = self.order.items().to_vec();
        let mut steps = Vec::new();
        for it in items {
            let secs = match &it.kind {
                ItemKind::Timer { seconds, .. } => *seconds,
                _ => 300,
            };
            steps.push(crate::runsheet::RunStep::new(&it.title, secs, StepAction::Order(it.id)));
        }
        if steps.is_empty() {
            self.toast("The service order is empty.");
            return;
        }
        self.x.runsheet.steps = steps;
        self.x.runsheet.stop();
    }

    // ------------------------------------------------------------------ read-along

    pub fn x_start_read_along(&mut self) {
        let Some(p) = self.parse_ctx.last.filter(|_| self.stage.kind == StageKind::Scripture) else {
            self.toast("Put a scripture on the screen first: the read-along shows that chapter.");
            return;
        };
        let bible = self.text_bible();
        let (b, c) = (p.book as usize, p.chapter as usize);
        let n = bible.verse_count(b, c);
        if n == 0 {
            return;
        }
        let verses: Vec<String> = (1..=n).map(|v| bible.verse(b, c, v).to_string()).collect();
        self.x.ra = Some(ReadAlong { book: b, chapter: c, verses, current: (p.from as usize).saturating_sub(1), seen_transcript: self.transcript.len() });
        self.x.card_src = CardSrc::ReadAlong;
        self.x_refresh_ra_card();
    }

    pub fn x_stop_read_along(&mut self) {
        if self.x.card_src == CardSrc::ReadAlong {
            self.x.card = None;
            self.x.card_src = CardSrc::None;
        }
        self.x.ra = None;
    }

    pub fn x_refresh_ra_card(&mut self) {
        let Some(ra) = &self.x.ra else { return };
        let title = format!("{} {}", self.bible.book_name(ra.book), ra.chapter);
        let mut c = Card::info(&title, &[]);
        c.kind = crate::overlay::CardKind::ReadAlong;
        c.lines = crate::readalong::lines(&ra.verses, 1);
        c.highlight = Some(ra.current);
        self.x.card = Some(c);
    }

    /// Move the highlight by hand.
    pub fn x_ra_step(&mut self, d: i32) {
        if let Some(ra) = &mut self.x.ra {
            let n = ra.verses.len() as i32;
            ra.current = (ra.current as i32 + d).clamp(0, n - 1) as usize;
        }
        self.x_refresh_ra_card();
    }

    /// The operator staged another verse of the same chapter (or another chapter): follow it.
    fn x_follow_read_along(&mut self) {
        if self.x.ra.is_none() {
            return;
        }
        if self.stage.kind != StageKind::Scripture {
            return;
        }
        let Some(p) = self.parse_ctx.last else { return };
        let same = self.x.ra.as_ref().map_or(false, |r| r.book == p.book as usize && r.chapter == p.chapter as usize);
        if same {
            if let Some(ra) = &mut self.x.ra {
                ra.current = (p.from as usize).saturating_sub(1).min(ra.verses.len().saturating_sub(1));
            }
            self.x_refresh_ra_card();
        } else {
            self.x_start_read_along();
        }
    }

    /// Listen to what is being said and move forward when the next verse is being read.
    fn x_read_along_listen(&mut self) {
        let Some(ra) = &mut self.x.ra else { return };
        if self.transcript.len() <= ra.seen_transcript {
            return;
        }
        let heard: String = self.transcript[ra.seen_transcript.saturating_sub(1).min(self.transcript.len() - 1)..].iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join(" ");
        ra.seen_transcript = self.transcript.len();
        let next = crate::readalong::advance(&ra.verses, ra.current, &heard);
        if next != ra.current {
            ra.current = next;
            self.x_refresh_ra_card();
        }
    }

    // ------------------------------------------------------------------ translation

    fn x_translate_tick(&mut self) {
        if !self.x.p.tr_enabled || !self.captions_on {
            return;
        }
        let cur = self.caption_now();
        if cur.is_empty() || cur == self.x.tr_sent {
            return;
        }
        self.x.tr_sent = cur.clone();
        let cfg = self.x.translate_cfg();
        let (shared, err) = (self.x.tr_shared.clone(), self.x.tr_error.clone());
        std::thread::spawn(move || match translate::translate_blocking(&cfg, &cur) {
            Ok(t) => {
                if let Ok(mut g) = shared.lock() {
                    *g = (cur, t);
                }
                if let Ok(mut e) = err.lock() {
                    *e = None;
                }
            }
            Err(e) => {
                if let Ok(mut g) = err.lock() {
                    *g = Some(e);
                }
            }
        });
    }

    // ------------------------------------------------------------------ MIDI

    pub fn x_midi_refresh_ports(&mut self) {
        self.x.midi_ports = match midir::MidiInput::new("RhemaFlow") {
            Ok(inp) => inp.ports().iter().filter_map(|p| inp.port_name(p).ok()).collect(),
            Err(_) => Vec::new(),
        };
    }

    pub fn x_midi_connect(&mut self) {
        self.x.midi_conn = None;
        let mut inp = match midir::MidiInput::new("RhemaFlow") {
            Ok(i) => i,
            Err(e) => {
                self.x.midi_status = format!("MIDI is not available ({e}).");
                return;
            }
        };
        inp.ignore(midir::Ignore::All);
        let ports = inp.ports();
        if ports.is_empty() {
            self.x.midi_status = "No MIDI device found. Plug it in and press Refresh.".into();
            return;
        }
        let want = self.x.p.midi_port.clone();
        let port = ports.iter().find(|p| inp.port_name(p).map_or(false, |n| n == want)).unwrap_or(&ports[0]).clone();
        let name = inp.port_name(&port).unwrap_or_default();
        let tx = self.x.midi_tx.clone();
        match inp.connect(&port, "rhemaflow-in", move |_t, msg, _| {
            let _ = tx.send(msg.to_vec());
        }, ()) {
            Ok(c) => {
                self.x.midi_conn = Some(c);
                self.x.midi_status = format!("Listening to {name}.");
                self.x.p.midi_port = name;
            }
            Err(e) => self.x.midi_status = format!("Could not open that MIDI device ({e})."),
        }
    }

    fn x_midi_poll(&mut self, now: u64) {
        while let Ok(msg) = self.x.midi_rx.try_recv() {
            let Some(t) = crate::controllers::parse_midi(&msg) else { continue };
            if !self.x.debounce.accept(t, now) {
                continue;
            }
            if let Some(action) = self.x.learning.take() {
                let _ = self.x.p.midi.set(t, &action);
                self.x.midi_status = format!("{} is now {}", t.describe(), crate::controllers::label_of(&action));
                self.x.save();
                continue;
            }
            self.x.midi_status = format!("Pressed {}", t.describe());
            if let Some(a) = self.x.p.midi.action_for(t).map(|s| s.to_string()) {
                self.x.queue.push(a);
            }
        }
    }

    // ------------------------------------------------------------------ series looks

    pub fn x_apply_theme(&mut self, ctx: &egui::Context, name: &str) {
        let Some(t) = self.x.themes.get(name).cloned() else { return };
        self.stage_bg = t.bg;
        self.stage_fg = t.fg;
        self.stage_style = StageStyle::from_key(&t.style);
        self.stage_zoom = t.zoom.clamp(crate::stage::ZOOM_MIN, crate::stage::ZOOM_MAX);
        if let Some(c) = &t.church_line {
            self.church_name = c.clone();
        }
        self.x.lt.accent = t.accent;
        self.x.p.lt_accent = t.accent;
        self.x.p.series_title = t.series_title.clone();
        match &t.background {
            Some(p) if std::path::Path::new(p).exists() => self.present.set_background(ctx, p),
            Some(_) => self.toast("The series background picture is missing (moved or deleted?)."),
            None => {}
        }
        self.x.themes.active = Some(t.name.clone());
        self.x.save();
        crate::app::save_settings(self);
    }

    pub fn x_current_look_as_theme(&self, name: &str) -> crate::themes::SeriesTheme {
        crate::themes::SeriesTheme {
            name: name.trim().to_string(),
            series_title: self.x.p.series_title.clone(),
            bg: self.stage_bg,
            fg: self.stage_fg,
            accent: self.x.p.lt_accent,
            style: self.stage_style.key().to_string(),
            zoom: self.stage_zoom,
            background: None,
            church_line: None,
        }
    }

    // ------------------------------------------------------------------ extra screens

    pub fn x_screens(&mut self, ctx: &egui::Context) {
        if self.x.p.screens.iter().all(|s| !s.enabled) {
            return;
        }
        let names: Vec<String> = self.displays.iter().map(|d| d.name.clone()).collect();
        let main = crate::displays::choose(&self.displays, self.output_display.as_deref());
        let stage = self.stage.clone();
        let art = self.present.art(&stage.kind);
        let (acc_bg, acc_fg) = self.x.p.access.stage_colors(self.stage_bg, self.stage_fg);
        let zoom_base = self.stage_zoom * self.x.p.access.zoom_factor();
        let caption = self.x.caption_for_stage(&self.caption_now());
        let ov = self.x.overlays(false);
        let blackout = self.blackout;
        let church = self.church_name.clone();
        let fade = self.stage_fade_ms;
        let base_style = self.stage_style;
        let mut closed: Vec<usize> = Vec::new();
        for (idx, prof) in self.x.p.screens.clone().iter().enumerate() {
            if !prof.enabled {
                continue;
            }
            let target = crate::screens::choose_display(&names, prof.display.as_deref(), main).map(|i| self.displays[i].clone());
            let (pos, size) = match &target {
                Some(d) => crate::displays::place(d, prof.fullscreen),
                None => ((80.0, 80.0), (640.0, 360.0)),
            };
            let vp = egui::ViewportId(egui::Id::new(("rf-screen", idx)));
            let builder = egui::ViewportBuilder::default()
                .with_title(format!("{} \u{2014} {}", crate::theme::TITLE, prof.name))
                .with_inner_size([size.0, size.1])
                .with_position([pos.0, pos.1])
                .with_decorations(!prof.fullscreen);
            let (bg, fg) = prof.colors.unwrap_or((acc_bg, acc_fg));
            let style = prof.style.as_deref().map(StageStyle::from_key).unwrap_or(base_style);
            let shows = prof.shows(content_of(&stage.kind));
            let shown_stage = if shows { stage.clone() } else { StageContent::default() };
            let look = crate::stage::StageLook { style, blackout: blackout && prof.follow_blackout, bg, fg, fade_ms: fade };
            let show_art = if shows { art.clone() } else { crate::stage::StageArt::default() };
            let zoom = zoom_base * prof.zoom;
            let cap = if prof.captions { caption.clone() } else { String::new() };
            let (lt_on, church_on) = (prof.lower_thirds, matches!(prof.role, crate::screens::Role::Lobby | crate::screens::Role::Overflow));
            let ov = ov.clone();
            let church = church.clone();
            let mut close = false;
            ctx.show_viewport_immediate(vp, builder, |vctx, _| {
                if vctx.input(|i| i.viewport().close_requested()) {
                    close = true;
                }
                egui::CentralPanel::default().frame(egui::Frame::none().fill(egui::Color32::from_rgb(bg[0], bg[1], bg[2]))).show(vctx, |ui| {
                    let r = ui.max_rect();
                    crate::stage::draw_stage(ui, &shown_stage, true, zoom, look, &show_art);
                    if !look.blackout {
                        paint_overlays(ui, r, &ov, fg, bg, lt_on);
                    }
                    if !cap.is_empty() && !look.blackout {
                        let w = (r.width() * 0.82).min(1500.0);
                        let size = (r.height() * 0.045).clamp(18.0, 56.0);
                        let galley = ui.painter().layout(cap.clone(), egui::FontId::proportional(size), egui::Color32::WHITE, w - 40.0);
                        let h = galley.size().y + 24.0;
                        let b = egui::Rect::from_center_size(egui::pos2(r.center().x, r.bottom() - h * 0.5 - r.height() * 0.06), egui::vec2(w, h));
                        ui.painter().rect_filled(b, 10.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 175));
                        ui.painter().galley(egui::pos2(b.left() + 20.0, b.top() + 12.0), galley, egui::Color32::WHITE);
                    }
                    if church_on && !church.is_empty() {
                        ui.painter().text(egui::pos2(r.right() - 24.0, r.bottom() - 24.0), egui::Align2::RIGHT_BOTTOM, &church, egui::FontId::proportional(15.0), egui::Color32::from_rgba_unmultiplied(150, 156, 168, 170));
                    }
                });
                vctx.request_repaint_after(Duration::from_millis(100));
            });
            if close {
                closed.push(idx);
            }
        }
        for i in closed {
            if let Some(s) = self.x.p.screens.get_mut(i) {
                s.enabled = false;
            }
            self.x.save();
        }
    }

    // ------------------------------------------------------------------ crash recovery

    fn x_make_snapshot(&self) -> Snapshot {
        let passage = if self.stage.kind == StageKind::Scripture { self.parse_ctx.last.map(|p| (p.book, p.chapter, p.from, p.to)) } else { None };
        let media = if self.stage.kind == StageKind::Media { self.present.loaded_path() } else { None };
        Snapshot {
            saved_unix: unix_now(),
            clean_exit: false,
            was_live: self.live,
            service_ms: if self.live { self.service_ms() } else { 0 },
            service_title: self.service_title.clone(),
            stage_kind: match self.stage.kind {
                StageKind::Blank => "blank",
                StageKind::Scripture => "scripture",
                StageKind::Points => "points",
                StageKind::Slide => "slide",
                StageKind::Media => "media",
                StageKind::Countdown => "countdown",
            }
            .into(),
            stage_label: self.stage.label.clone(),
            stage_text: self.stage.text.clone(),
            stage_translation: self.stage.translation.clone(),
            stage_passage: passage,
            media_path: media,
            blackout: self.blackout,
            captions_on: self.captions_on,
            bible_version: self.translation.clone(),
            plan_text: self.plan_text.clone(),
            plan_pos: self.plan_pos,
            runsheet: if self.x.runsheet.steps.is_empty() { None } else { Some(self.x.runsheet.clone()) },
            order_live: self.order.live(),
            lower_third: if self.x.lt.line1.is_empty() { None } else { Some((self.x.lt.line1.clone(), self.x.lt.line2.clone())) },
        }
    }

    fn x_snapshot_tick(&mut self) {
        if self.x.recovery_offer.is_some() || self.x.last_snap_at.elapsed() < Duration::from_secs(4) {
            return;
        }
        self.x.last_snap_at = Instant::now();
        let snap = self.x_make_snapshot();
        let changed = self.x.last_snap.as_ref().map_or(true, |p| snap.differs_from(p));
        if changed {
            let _ = snap.save(&self.x.data_dir);
            self.x.last_snap = Some(snap);
        }
    }

    pub fn x_mark_clean_exit(&mut self) {
        if let Some(mut s) = self.x.last_snap.clone() {
            s.clean_exit = true;
            let _ = s.save(&self.x.data_dir);
        } else {
            Snapshot::mark_clean(&self.x.data_dir);
        }
    }

    pub fn x_restore(&mut self, ctx: &egui::Context) {
        let Some(s) = self.x.recovery_offer.take() else { return };
        self.translation = if s.bible_version.is_empty() { self.translation.clone() } else { s.bible_version.clone() };
        self.service_title = s.service_title.clone();
        self.captions_on = s.captions_on;
        if !s.plan_text.is_empty() {
            self.plan_text = s.plan_text.clone();
            self.load_plan();
            self.plan_pos = s.plan_pos.min(self.plan.len());
            for (i, p) in self.plan.iter_mut().enumerate() {
                p.used = i < s.plan_pos;
            }
        }
        if s.was_live && !self.live {
            self.start_service();
        }
        match s.stage_kind.as_str() {
            "scripture" => {
                if let Some((b, c, f, t)) = s.stage_passage {
                    self.do_stage(rf_core::Passage { book: b, chapter: c, from: f, to: t }, "recovery", 1.0, false);
                }
            }
            "slide" | "points" | "countdown" => {
                let kind = if s.stage_kind == "points" { StageKind::Points } else { StageKind::Slide };
                self.show_content(StageContent { kind, label: s.stage_label.clone(), text: s.stage_text.clone(), translation: s.stage_translation.clone(), chapter_only: false, updated: Some(Instant::now()) });
            }
            "media" => {
                if let Some(p) = &s.media_path {
                    if let Some(c) = self.present.stage_path(ctx, p) {
                        self.show_content(c);
                    }
                }
            }
            _ => {}
        }
        self.blackout = s.blackout;
        if let Some(mut r) = s.runsheet.clone() {
            let cur = r.current;
            r.stop();
            r.auto = self.x.runsheet.auto;
            self.x.runsheet = r;
            if let Some(i) = cur {
                let now = self.x.now_ms();
                self.x.runsheet.go(i, now);
            }
        }
        if let Some((a, b)) = s.lower_third {
            self.x.lt.line1 = a;
            self.x.lt.line2 = b;
        }
        if let Some((item, slide)) = s.order_live {
            if self.order.runner.go(item, slide).is_ok() {
                self.order_apply(ctx);
                if s.stage_kind == "blank" {
                    // nothing else was on the screen
                }
            }
        }
        let _ = Snapshot::mark_clean(&self.x.data_dir);
        self.toast("Restored where you were. A new recording was started if the service was live; the first part is in the earlier folder.");
    }

    pub fn x_dismiss_recovery(&mut self) {
        self.x.recovery_offer = None;
        Snapshot::mark_clean(&self.x.data_dir);
    }

    // ------------------------------------------------------------------ backup

    pub fn x_backup_now(&mut self) {
        let dest = self.x.p.backup_dir.trim().to_string();
        if dest.is_empty() {
            self.x.backup_msg = "Choose a backup folder first.".into();
            return;
        }
        if self.x.backup_busy.swap(true, std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let (data, keep, media) = (self.x.data_dir.clone(), self.x.p.backup_keep.max(1), self.x.p.backup_media);
        let (busy, result) = (self.x.backup_busy.clone(), self.x.backup_result.clone());
        let (stamp, _) = crate::naming::now_local();
        let stamp = format!("{:04}-{:02}-{:02} {:02}{:02}", stamp.y, stamp.mo, stamp.d, stamp.h, stamp.mi);
        self.x.p.last_backup = unix_now();
        self.x.save();
        std::thread::spawn(move || {
            let msg = match crate::backup::make(&data, std::path::Path::new(&dest), &stamp, media) {
                Ok(r) => {
                    let pruned = crate::backup::prune(std::path::Path::new(&dest), keep);
                    format!("Backed up {} files ({}){}{}", r.files, crate::export::human_size(r.bytes), if r.skipped_media > 0 { format!("; {} media files left out (tick \"include media\" to add them)", r.skipped_media) } else { String::new() }, if pruned > 0 { format!("; removed {pruned} old backups") } else { String::new() })
                }
                Err(e) => e,
            };
            if let Ok(mut g) = result.lock() {
                *g = Some(msg);
            }
            busy.store(false, std::sync::atomic::Ordering::Relaxed);
        });
    }

    fn x_backup_tick(&mut self) {
        if let Ok(mut g) = self.x.backup_result.lock() {
            if let Some(m) = g.take() {
                self.x.backup_msg = m;
            }
        }
        if self.x.p.auto_backup && !self.x.p.backup_dir.trim().is_empty() && !self.live && crate::backup::due(self.x.p.last_backup, unix_now()) && !self.x.started_tick {
            // once per run, shortly after start, so it never lands in the middle of a service
            self.x.started_tick = true;
            self.x_backup_now();
        }
    }

    pub fn x_restore_backup(&mut self) {
        let dest = std::path::PathBuf::from(self.x.p.backup_dir.trim());
        let Some(zip) = crate::backup::latest(&dest) else {
            self.x.backup_msg = "No backup found in that folder.".into();
            return;
        };
        let (stamp, _) = crate::naming::now_local();
        let stamp = format!("{:04}-{:02}-{:02} {:02}{:02}", stamp.y, stamp.mo, stamp.d, stamp.h, stamp.mi);
        self.x.backup_msg = match crate::backup::restore(&zip, &self.x.data_dir.clone(), &dest, &stamp) {
            Ok(n) => format!("Restored {n} files from {}. Restart RhemaFlow to load them. (What was here before is saved as a \"before-restore\" backup.)", zip.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
            Err(e) => e,
        };
    }

    // ------------------------------------------------------------------ handout / clips export

    pub fn x_service_folder(&self) -> Option<PathBuf> {
        self.service_dir.clone().or_else(|| self.saved.as_ref().map(|s| s.dir.clone()))
    }

    pub fn x_build_handout(&mut self) {
        let Some(dir) = self.x_service_folder() else {
            self.x.handout_msg = "Start a service first, so there is a folder to save the handout in.".into();
            return;
        };
        let (start, _) = self.service_start.clone().unwrap_or_else(crate::naming::now_local);
        let scriptures: Vec<(String, String, String)> = self.export_cites.iter().map(|c| (c.label.clone(), c.text.clone(), self.translation.clone())).collect();
        let notices: Vec<(String, String)> = crate::announce::parse(&self.x.p.announce_text, 10).into_iter().map(|a| (a.title, a.body)).collect();
        let songs = self.present.songs_shown();
        let input = crate::handout::HandoutInput {
            church: self.church_name.clone(),
            title: self.service_title.clone(),
            date: start.long_date(),
            scriptures,
            songs,
            notes: self.x.p.handout_notes.clone(),
            notices,
            next_steps: self.x.p.next_steps.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect(),
            giving_label: self.x.p.giving_headline.clone(),
            giving_url: self.x.p.giving_url.clone(),
            footer: String::new(),
        };
        let h = crate::handout::build(&input);
        let base = if self.live_base.is_empty() { crate::naming::live_base(&start, &self.service_title) } else { self.live_base.clone() };
        let (html_p, md_p) = (dir.join(crate::naming::file_name(&base, "handout.html")), dir.join(crate::naming::file_name(&base, "handout.md")));
        let ok = std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&html_p, h.html).is_ok() && std::fs::write(&md_p, h.markdown).is_ok();
        self.x.handout_msg = if ok { format!("Saved: {}", html_p.display()) } else { "Could not save the handout.".into() };
        if ok && cfg!(target_os = "macos") {
            let _ = std::process::Command::new("open").arg(&html_p).spawn();
        }
    }

    pub fn x_export_clips(&mut self) {
        let Some(dir) = self.x_service_folder() else {
            self.x.clips_msg = "There is no service folder yet.".into();
            return;
        };
        let rec = self.rec_paths.iter().find(|p| p.exists()).cloned().or_else(|| {
            std::fs::read_dir(&dir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().map_or(false, |e| e == "wav"))
        });
        let ff = rf_media::convert::find_ffmpeg();
        let r = crate::clips::export(&dir.join("clips"), &self.x.clips.clips, &self.export_lines, rec.as_deref(), 0, ff.as_deref());
        self.x.clips_msg = format!("Saved {} files in the \"clips\" folder{}{}", r.written.len(), if r.audio_made > 0 { format!(", {} audio clips", r.audio_made) } else { String::new() }, if r.problems.is_empty() { String::new() } else { format!(". Note: {}", r.problems.join(" ")) });
    }
}
