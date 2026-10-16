//! The service order (v0.825.0): the list of things in today's service, in order, and the buttons
//! that run it (Next / Back / Clear / Black / Undo).
//!
//! Two parts:
//!  * `Order` has no screen in it. It wraps `rf_service::Runner` (which already refuses to lose its
//!    place when you reorder things) and saves the list safely to disk. It is fully tested.
//!  * `impl RhemaFlowApp` below draws the panel and puts the chosen item on the stage.
//!
//! NOT VERIFIED on a real Mac window: how the panel looks, and dragging from the MEDIA bar.

use crate::app::RhemaFlowApp;
use crate::present::OrderAdd;
use crate::state::{StageContent, StageKind};
use crate::theme::*;
use egui::RichText;
use rf_service::runner::{Mode, Output};
use rf_service::{Autosave, Item, ItemKind, Runner, Service};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RightTab {
    Review,
    Order,
    Strongs,
    History,
    Index,
}

impl RightTab {
    pub fn key(self) -> &'static str {
        match self {
            RightTab::Review => "review",
            RightTab::Order => "order",
            RightTab::Strongs => "strongs",
            RightTab::History => "history",
            RightTab::Index => "index",
        }
    }
    pub fn from_key(k: &str) -> RightTab {
        [RightTab::Order, RightTab::Review, RightTab::Strongs, RightTab::History, RightTab::Index].into_iter().find(|t| t.key() == k).unwrap_or(RightTab::Order)
    }
}

pub struct Order {
    pub runner: Runner,
    path: PathBuf,
    autosave: Autosave,
    t0: Instant,
    pub note: Option<(String, Instant)>,
    /// Set when the saved list was damaged and an older copy was used instead.
    pub recovered: bool,
    /// Earlier and later versions of the list, for Undo edit / Redo edit (v0.841.0).
    hist: EditHistory,
    /// The item whose details (name, minutes, note) are open in the panel.
    pub expanded: Option<u64>,
    /// The name being typed in the open details (kept apart so a half-typed or empty name is not snapped back).
    pub name_buf: String,
    /// Which item has been on the screen, and since when (for the countdown).
    live_since: Option<(u64, Instant)>,
}

/// Up to `CAP` earlier versions of the list. Only the list is kept (names, order, notes), never what is on the screen.
#[derive(Default)]
struct EditHistory {
    undo: Vec<Vec<Item>>,
    redo: Vec<Vec<Item>>,
    last_tag: Option<(u64, &'static str, Instant)>,
}
const CAP: usize = 50;

impl Order {
    /// Open the saved service order (or start an empty one). Never shows anything on screen by itself.
    pub fn open(path: PathBuf) -> Order {
        let (service, recovered, note) = match rf_service::load(&path) {
            Ok(o) => (o.service, o.recovered_from.is_some(), None),
            Err(rf_service::StoreError::Io(_)) => (Service::new("Service order"), false, None),
            Err(e) => (Service::new("Service order"), false, Some((format!("The saved service order could not be read ({e}). Starting an empty one; the old file was left alone."), Instant::now()))),
        };
        Order { runner: Runner::new(service), path, autosave: Autosave::new(800, 5000), t0: Instant::now(), note, recovered, hist: EditHistory::default(), expanded: None, name_buf: String::new(), live_since: None }
    }

    fn now_ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }

    fn dirty(&mut self) {
        let n = self.now_ms();
        self.autosave.mark_dirty(n);
    }

    pub fn say(&mut self, m: impl Into<String>) {
        self.note = Some((m.into(), Instant::now()));
    }

    /// Call every frame: saves a moment after the last change.
    pub fn tick(&mut self) {
        let live_id = self.live().map(|(id, _)| id);
        match (live_id, self.live_since) {
            (Some(id), Some((was, _))) if id == was => {}
            (Some(id), _) => self.live_since = Some((id, Instant::now())),
            (None, _) => self.live_since = None,
        }
        if self.autosave.should_save(self.now_ms()) {
            self.save_now();
        }
        if let Some((_, t)) = &self.note {
            if t.elapsed() > Duration::from_secs(10) {
                self.note = None;
            }
        }
    }

    pub fn save_now(&mut self) {
        match rf_service::save(&self.path, self.runner.service()) {
            Ok(()) => self.autosave.saved(),
            Err(e) => {
                self.autosave.saved(); // do not retry every frame; the next change tries again
                self.say(format!("Could not save the service order ({e})."));
            }
        }
    }

    pub fn items(&self) -> &[Item] {
        &self.runner.service().items
    }

    pub fn add(&mut self, title: &str, kind: ItemKind) -> Option<u64> {
        let snap = self.items().to_vec();
        match self.runner.add_item(title, kind) {
            Ok(id) => {
                self.keep(snap);
                self.dirty();
                self.say(format!("Added \"{title}\" to the service order."));
                Some(id)
            }
            Err(e) => {
                self.say(format!("Not added: {e}. Switch to Edit to change the list."));
                None
            }
        }
    }

    pub fn remove(&mut self, id: u64) {
        let snap = self.items().to_vec();
        match self.runner.remove_item(id) {
            Ok(()) => {
                self.keep(snap);
                self.dirty();
                if self.expanded == Some(id) {
                    self.expanded = None;
                }
                self.say("Removed. Undo edit brings it back.");
            }
            Err(e) => self.say(format!("Not removed: {e}.")),
        }
    }

    /// Move an item up (`-1`) or down (`+1`).
    pub fn nudge(&mut self, id: u64, delta: i32) {
        let Some(i) = self.runner.service().index_of(id) else { return };
        let j = (i as i32 + delta).clamp(0, self.items().len() as i32 - 1) as usize;
        if i != j {
            let snap = self.items().to_vec();
            match self.runner.move_item(id, j) {
                Ok(()) => {
                    self.keep(snap);
                    self.dirty();
                }
                Err(e) => self.say(format!("Not moved: {e}.")),
            }
        }
    }

    // ---- v0.841.0: undo / redo of list edits, rename, duplicate, minutes, notes

    /// Remember the list as it was before an edit that worked.
    fn keep(&mut self, before: Vec<Item>) {
        self.hist.undo.push(before);
        if self.hist.undo.len() > CAP {
            self.hist.undo.remove(0);
        }
        self.hist.redo.clear();
    }

    pub fn can_undo_edit(&self) -> bool {
        !self.hist.undo.is_empty()
    }

    pub fn can_redo_edit(&self) -> bool {
        !self.hist.redo.is_empty()
    }

    /// Go back one edit of the list. Never changes what is on the screen unless the live item itself was just
    /// added or removed (then the screen is cleared, never left pointing at nothing).
    pub fn undo_edit(&mut self) -> bool {
        let Some(prev) = self.hist.undo.pop() else {
            self.say("Nothing to undo in the list.");
            return false;
        };
        let now = self.items().to_vec();
        match self.runner.restore_items(prev.clone()) {
            Ok(()) => {
                self.hist.redo.push(now);
                self.dirty();
                self.say("Undid the last change to the list.");
                true
            }
            Err(e) => {
                self.hist.undo.push(prev);
                self.say(format!("Not undone: {e}. Switch to Edit to change the list."));
                false
            }
        }
    }

    pub fn redo_edit(&mut self) -> bool {
        let Some(next) = self.hist.redo.pop() else {
            self.say("Nothing to redo in the list.");
            return false;
        };
        let now = self.items().to_vec();
        match self.runner.restore_items(next.clone()) {
            Ok(()) => {
                self.hist.undo.push(now);
                self.dirty();
                self.say("Redid the change.");
                true
            }
            Err(e) => {
                self.hist.redo.push(next);
                self.say(format!("Not redone: {e}. Switch to Edit to change the list."));
                false
            }
        }
    }

    /// Like `keep`, but a burst of changes to the same thing (dragging a number, typing a name) is ONE undo step.
    fn keep_tagged(&mut self, id: u64, tag: &'static str, before: Vec<Item>) {
        let burst = matches!(self.hist.last_tag, Some((i, t, at)) if i == id && t == tag && at.elapsed() < Duration::from_millis(1500));
        self.hist.last_tag = Some((id, tag, Instant::now()));
        if !burst {
            self.keep(before);
        } else {
            self.hist.redo.clear();
        }
    }

    pub fn rename(&mut self, id: u64, title: &str) {
        let before = self.items().to_vec();
        if self.items().iter().any(|i| i.id == id && i.title == title.trim()) || title.trim().is_empty() {
            return;
        }
        match self.runner.rename_item(id, title) {
            Ok(()) => {
                self.keep_tagged(id, "rename", before);
                self.dirty();
            }
            Err(e) => self.say(format!("Not renamed: {e}. Switch to Edit to change the list.")),
        }
    }

    /// A private note for the operator. Allowed while the list is locked (it changes nothing on screen).
    pub fn set_notes(&mut self, id: u64, notes: &str) {
        if self.items().iter().any(|i| i.id == id && i.notes == notes) {
            return;
        }
        if self.runner.set_item_notes(id, notes).is_ok() {
            self.dirty();
        }
    }

    pub fn set_minutes(&mut self, id: u64, minutes: Option<u32>) {
        if self.items().iter().any(|i| i.id == id && i.minutes == minutes) {
            return;
        }
        let before = self.items().to_vec();
        if self.runner.set_item_minutes(id, minutes).is_ok() {
            self.keep_tagged(id, "minutes", before);
            self.dirty();
        }
    }

    /// A copy right under the original (notes and minutes come along).
    pub fn duplicate(&mut self, id: u64) -> Option<u64> {
        let it = self.items().iter().find(|i| i.id == id)?.clone();
        let at = self.runner.service().index_of(id)? + 1;
        let before = self.items().to_vec();
        let new = match self.runner.add_item(&format!("{} (copy)", it.title), it.kind.clone()) {
            Ok(n) => n,
            Err(e) => {
                self.say(format!("Not copied: {e}. Switch to Edit to change the list."));
                return None;
            }
        };
        let _ = self.runner.move_item(new, at);
        let _ = self.runner.set_item_notes(new, &it.notes);
        let _ = self.runner.set_item_minutes(new, it.minutes);
        self.keep(before);
        self.dirty();
        Some(new)
    }

    /// Seconds the item on the screen has been on it (0 when nothing is).
    pub fn live_elapsed_secs(&self) -> u64 {
        self.live_since.map_or(0, |(_, t)| t.elapsed().as_secs())
    }

    /// The whole list as plain text, for printing or pasting into a message.
    pub fn run_sheet_text(&self) -> String {
        run_sheet_text(self.runner.service().title.as_str(), self.items())
    }

    pub fn set_show_mode(&mut self, on: bool) {
        self.runner.set_mode(if on { Mode::Show } else { Mode::Edit });
    }

    pub fn show_mode(&self) -> bool {
        self.runner.mode() == Mode::Show
    }

    /// The item (and slide) that is live, if any.
    pub fn live(&self) -> Option<(u64, usize)> {
        match self.runner.output() {
            Output::Content { item_id, slide } => Some((item_id, slide)),
            _ => None,
        }
    }

    /// Remember where we were, so the position survives a restart (it is never shown by itself).
    pub fn remember_position(&mut self) {
        self.dirty();
    }
}

/// How the list adds up and what comes next (v0.841.0). No screen in it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RunSummary {
    /// all the planned minutes
    pub total_min: u32,
    /// planned minutes from the item on the screen to the end (or all of them when nothing is live)
    pub left_min: u32,
    /// items with no planned time (they count as zero)
    pub untimed: usize,
}

pub fn summarize(items: &[Item], live_id: Option<u64>) -> RunSummary {
    let from = live_id.and_then(|id| items.iter().position(|i| i.id == id)).unwrap_or(0);
    RunSummary {
        total_min: items.iter().filter_map(|i| i.minutes).sum(),
        left_min: items[from.min(items.len())..].iter().filter_map(|i| i.minutes).sum(),
        untimed: items.iter().filter(|i| i.minutes.is_none()).count(),
    }
}

/// `1 h 05 min` / `42 min`.
pub fn minutes_text(m: u32) -> String {
    if m >= 60 {
        format!("{} h {:02} min", m / 60, m % 60)
    } else {
        format!("{m} min")
    }
}

/// What comes next after the slide that is on the screen: the next slide of the same item, or the next item.
pub fn next_up(items: &[Item], live: Option<(u64, usize)>) -> Option<String> {
    let (id, slide) = live?;
    let idx = items.iter().position(|i| i.id == id)?;
    let cur = &items[idx];
    if slide + 1 < cur.slide_count() {
        return Some(format!("{} \u{2014} part {} of {}", cur.title, slide + 2, cur.slide_count()));
    }
    items[idx + 1..].iter().find(|i| i.slide_count() > 0).map(|i| i.title.clone())
}

/// The clock for the item on the screen. With planned minutes it counts down, then counts how far over; without them it counts up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemClock {
    pub text: String,
    pub over: bool,
    /// under a minute left (the panel turns it amber)
    pub soon: bool,
}

pub fn item_clock(planned_min: Option<u32>, elapsed_secs: u64) -> ItemClock {
    let mmss = |s: u64| format!("{}:{:02}", s / 60, s % 60);
    match planned_min {
        Some(m) if m > 0 => {
            let plan = m as u64 * 60;
            if elapsed_secs <= plan {
                let left = plan - elapsed_secs;
                ItemClock { text: format!("{} left", mmss(left)), over: false, soon: left <= 60 }
            } else {
                ItemClock { text: format!("{} over", mmss(elapsed_secs - plan)), over: true, soon: false }
            }
        }
        _ => ItemClock { text: format!("{} on screen", mmss(elapsed_secs)), over: false, soon: false },
    }
}

/// The list as numbered plain text with planned minutes and notes.
pub fn run_sheet_text(title: &str, items: &[Item]) -> String {
    let mut out = format!("{}\n", if title.trim().is_empty() { "Service order" } else { title });
    for (n, it) in items.iter().enumerate() {
        let mins = it.minutes.map(|m| format!("  [{m} min]")).unwrap_or_default();
        out.push_str(&format!("{:>2}. {}  ({}){}\n", n + 1, it.title, kind_word(&it.kind), mins));
        if !it.notes.trim().is_empty() {
            out.push_str(&format!("      note: {}\n", it.notes.trim().replace('\n', " / ")));
        }
    }
    let s = summarize(items, None);
    if s.total_min > 0 {
        out.push_str(&format!("\nPlanned time: {}\n", minutes_text(s.total_min)));
    }
    out
}

/// Labels for the five dock tabs, short enough to stay on one line however narrow the dock is (v0.841.0).
/// `avail` is the width in points. Three sizes of wording: full, medium, tight ("G/H" = Greek and Hebrew).
pub fn dock_tab_labels(avail: f32, orders: usize, reviews: usize, history: usize) -> [String; 5] {
    let full = [format!("Order {orders}"), format!("Review {reviews}"), "Strong's".to_string(), format!("History {history}"), "Index".to_string()];
    let medium = [format!("Order {orders}"), if reviews > 0 { format!("Review {reviews}") } else { "Review".into() }, "Strong's".into(), "History".into(), "Index".into()];
    let tight = [format!("Ord {orders}"), if reviews > 0 { format!("Rev {reviews}") } else { "Rev".into() }, "G/H".into(), "Hist".into(), "Idx".into()];
    let width = |l: &[String; 5]| l.iter().map(|s| s.chars().count() as f32 * 6.6 + 12.0).sum::<f32>();
    if width(&full) <= avail {
        full
    } else if width(&medium) <= avail {
        medium
    } else {
        tight
    }
}

/// A short icon-and-word for each kind of item.
pub fn kind_word(k: &ItemKind) -> &'static str {
    match k {
        ItemKind::Scripture { .. } => "scripture",
        ItemKind::Slides { .. } => "slides",
        ItemKind::Song { .. } => "song",
        ItemKind::Media { path } => match rf_service::media::classify(path) {
            rf_service::media::Classified::Ok(rf_service::media::MediaKind::Video) => "video",
            rf_service::media::Classified::Ok(rf_service::media::MediaKind::Audio) => "sound",
            _ => "picture",
        },
        ItemKind::Timer { .. } => "timer",
    }
}

impl RhemaFlowApp {
    /// Things asked for by the media windows (right-click "Add to service order", the Songs tab).
    pub fn order_collect(&mut self) {
        let adds: Vec<OrderAdd> = std::mem::take(&mut self.present.order_outbox);
        for a in adds {
            self.order_add(a);
        }
    }

    pub fn order_add(&mut self, a: OrderAdd) {
        match a {
            OrderAdd::Media(id) => {
                let Some((name, path)) = self.present.shelf.get(id).map(|e| (e.name.clone(), e.path.clone())) else { return };
                self.order.add(&name, ItemKind::Media { path });
            }
            OrderAdd::Song(id) => {
                let Some((title, parts)) = self.present.song_parts(id) else { return };
                self.order.add(&title, ItemKind::Song { song_id: id.to_string(), arrangement: parts });
            }
        }
    }

    /// "+ What is on the stage now".
    fn order_add_from_stage(&mut self) {
        let st = self.stage.clone();
        match st.kind {
            StageKind::Scripture => {
                let tr = st.translation.clone();
                self.order.add(&st.label, ItemKind::Scripture { reference: st.label.clone(), translation: tr });
            }
            StageKind::Media => {
                let idx = self.present.loaded_idx;
                match idx.and_then(|i| self.present.settings.media.get(i).cloned()) {
                    Some(path) => {
                        self.order.add(&st.label, ItemKind::Media { path });
                    }
                    None => self.order.say("Nothing to add yet."),
                }
            }
            StageKind::Slide => {
                let title = if st.label.is_empty() { "Slide".to_string() } else { st.label.clone() };
                self.order.add(&title, ItemKind::Slides { slides: vec![rf_service::Slide { id: 0, text: st.text.clone(), notes: String::new(), theme: None }] });
            }
            _ => self.order.say("Put a scripture, picture, video, sound or slide on the stage first, then add it."),
        }
    }

    /// Put what the runner says is live onto the stage.
    pub fn order_apply(&mut self, ctx: &egui::Context) {
        match self.order.runner.output() {
            Output::Clear | Output::Logo => {
                self.blackout = false;
                self.clear_stage();
            }
            Output::Black => {
                self.blackout = true;
            }
            Output::Content { item_id, slide } => {
                let Some(item) = self.order.runner.service().item(item_id).cloned() else { return };
                self.blackout = false;
                let shown: Option<StageContent> = match &item.kind {
                    ItemKind::Scripture { reference, .. } => {
                        let ctx_p = rf_core::ParseContext::default();
                        let out = rf_core::parse(&self.bible, reference, &ctx_p, 0.0);
                        let p = out.detections.first().or(out.confirms.first()).map(|d| d.passage);
                        match p {
                            Some(p) if p.chapter > 0 => {
                                self.do_stage(p, "service order", 1.0, false);
                                self.touches += 1;
                                None // do_stage already put it on the stage
                            }
                            _ => {
                                self.order.say(format!("Could not find \"{reference}\" in the Bible."));
                                None
                            }
                        }
                    }
                    ItemKind::Media { path } => self.present.stage_path(ctx, path),
                    ItemKind::Slides { slides } => slides.get(slide).map(|s| StageContent {
                        kind: StageKind::Slide,
                        label: item.title.clone(),
                        text: s.text.clone(),
                        translation: String::new(),
                        chapter_only: false,
                        updated: Some(Instant::now()),
                    }),
                    ItemKind::Song { song_id, .. } => match song_id.parse::<u64>() {
                        Ok(id) => self.present.stage_song_slide(id, slide),
                        Err(_) => None,
                    },
                    ItemKind::Timer { seconds, label } => self.present.stage_countdown(label, *seconds),
                };
                if let Some(c) = shown {
                    self.show_content(c);
                } else if matches!(item.kind, ItemKind::Media { .. } | ItemKind::Song { .. }) {
                    // it could not be shown (missing file ...): Present already said why. Do not leave the list pointing at it.
                    self.order.runner.undo();
                }
            }
        }
        self.order.remember_position();
    }

    pub fn ui_order(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let panel_rect = ui.max_rect();
        let drop_zone = ui.interact(panel_rect, egui::Id::new("order_drop"), egui::Sense::hover());
        let dragging = egui::DragAndDrop::has_payload_of_type::<u64>(ctx);
        let mut apply = false;
        let mut remove: Option<u64> = None;
        let mut nudge: Option<(u64, i32)> = None;
        let mut go: Option<(u64, usize)> = None;
        let mut details: Option<u64> = None;
        let mut rename_req: Option<(u64, String)> = None;
        let mut minutes_req: Option<(u64, Option<u32>)> = None;
        let mut notes_req: Option<(u64, String)> = None;
        let mut dup_req: Option<u64> = None;
        let expanded = self.order.expanded;
        let mut name_buf = self.order.name_buf.clone();

        ui.horizontal(|ui| {
            ui.label(RichText::new("SERVICE ORDER").small().color(MUTED).strong());
            let mut show = self.order.show_mode();
            if ui.checkbox(&mut show, "Locked for the service").on_hover_text("ON: the list cannot be changed by accident while you are live. Next, Back, Clear and Black still work.").changed() {
                self.order.set_show_mode(show);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("\u{25c0} Back").clicked() {
                if !self.order.runner.prev() {
                    self.order.say("That is the start of the service.");
                }
                apply = true;
            }
            if ui.button(RichText::new("Next \u{25b6}").strong()).clicked() {
                if !self.order.runner.next() {
                    self.order.say("That was the last slide of the service.");
                }
                apply = true;
            }
            if ui.button("Clear").clicked() {
                self.order.runner.clear();
                apply = true;
            }
            if ui.button("Black").clicked() {
                self.order.runner.black();
                apply = true;
            }
            if ui.button("Undo screen").on_hover_text("Go back to what was on the screen before the last change (10 steps)").clicked() {
                if self.order.runner.undo() {
                    apply = true;
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.add_enabled(self.order.can_undo_edit(), egui::Button::new("Undo edit")).on_hover_text("Take back the last change to the LIST (add, remove, move, rename). Cmd+Z when no text box is active.").clicked() {
                self.order.undo_edit();
            }
            if ui.add_enabled(self.order.can_redo_edit(), egui::Button::new("Redo edit")).on_hover_text("Put the change back (Shift+Cmd+Z)").clicked() {
                self.order.redo_edit();
            }
            if ui.button("Copy run sheet").on_hover_text("Copy the list, with minutes and notes, as plain text to paste into a message or print").clicked() {
                ctx.copy_text(self.order.run_sheet_text());
                self.order.say("Copied the run sheet.");
            }
        });
        ui.horizontal(|ui| {
            if ui.button("+ What is on the stage").on_hover_text("Add the scripture, picture, video, sound or slide that is showing now").clicked() {
                self.order_add_from_stage();
            }
            if ui.button("+ 5 min timer").clicked() {
                self.order.add("Countdown 5:00", ItemKind::Timer { seconds: 300, label: "Starting soon".into() });
            }
        });
        ui.add_space(4.0);

        let live = self.order.live();
        let items: Vec<Item> = self.order.items().to_vec();
        let locked = self.order.show_mode();
        // what is on the screen, how long it has been there, what comes next, how much time is left (v0.841.0)
        {
            let sum = summarize(&items, live.map(|(id, _)| id));
            let clock = live.and_then(|(id, _)| items.iter().find(|i| i.id == id)).map(|it| item_clock(it.minutes, self.order.live_elapsed_secs()));
            let next = next_up(&items, live);
            if clock.is_some() || sum.total_min > 0 || next.is_some() {
                egui::Frame::none().fill(egui::Color32::from_rgb(24, 28, 40)).rounding(5.0).inner_margin(6.0).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if let Some(c) = &clock {
                        let col = if c.over { egui::Color32::from_rgb(235, 90, 80) } else if c.soon { egui::Color32::from_rgb(240, 180, 60) } else { GOLD };
                        ui.label(RichText::new(&c.text).strong().color(col));
                        ctx.request_repaint_after(Duration::from_millis(500)); // the clock ticks while something is live
                    }
                    if let Some(n) = &next {
                        ui.label(RichText::new(format!("Next: {n}")).small().color(TEXT));
                    }
                    if sum.total_min > 0 {
                        let more = if sum.untimed > 0 { format!("  ({} without a time)", sum.untimed) } else { String::new() };
                        ui.label(RichText::new(format!("Left {} of {}{more}", minutes_text(sum.left_min), minutes_text(sum.total_min))).small().color(MUTED));
                    }
                });
                ui.add_space(3.0);
            }
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 52.0).show(ui, |ui| {
            if items.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new("Nothing in the service yet.").color(MUTED));
                ui.label(RichText::new("Drag a picture, video or sound from the MEDIA bar into this panel, or use the buttons above.").small().color(MUTED));
            }
            for (n, it) in items.iter().enumerate() {
                let is_live = live.map_or(false, |(id, _)| id == it.id);
                let fill = if is_live { egui::Color32::from_rgb(30, 60, 120) } else { PANEL_2 };
                egui::Frame::none().fill(fill).rounding(5.0).inner_margin(6.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let title = if it.title.chars().count() > 26 { it.title.chars().take(25).collect::<String>() + "\u{2026}" } else { it.title.clone() };
                        let r = ui.add(egui::Label::new(RichText::new(format!("{:>2}  {}", n + 1, title)).strong().color(TEXT)).sense(egui::Sense::click()));
                        if r.clicked() {
                            go = Some((it.id, 0));
                        }
                        ui.label(RichText::new(kind_word(&it.kind)).small().color(MUTED));
                        if let Some(m) = it.minutes {
                            ui.label(RichText::new(format!("{m} min")).small().color(MUTED));
                        }
                        if !it.notes.trim().is_empty() {
                            ui.label(RichText::new("note").small().color(GOLD));
                        }
                        if is_live {
                            ui.label(RichText::new("ON SCREEN").small().strong().color(egui::Color32::WHITE).background_color(egui::Color32::from_rgb(60, 130, 246)));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let open = expanded == Some(it.id);
                            if ui.small_button(if open { "Close" } else { "Details" }).on_hover_text("Rename, planned minutes, a private note for the operator, duplicate").clicked() {
                                details = Some(it.id);
                            }
                            ui.add_enabled_ui(!locked, |ui| {
                                if ui.small_button("\u{2716}").on_hover_text("Remove from the service order (the file stays on the shelf)").clicked() {
                                    remove = Some(it.id);
                                }
                                if ui.small_button("\u{25bc}").clicked() {
                                    nudge = Some((it.id, 1));
                                }
                                if ui.small_button("\u{25b2}").clicked() {
                                    nudge = Some((it.id, -1));
                                }
                            });
                        });
                    });
                    let count = it.slide_count();
                    if count > 1 {
                        ui.horizontal_wrapped(|ui| {
                            for s in 0..count {
                                let on = live == Some((it.id, s));
                                let label = match &it.kind {
                                    ItemKind::Song { arrangement, .. } => arrangement.get(s).cloned().unwrap_or_else(|| (s + 1).to_string()),
                                    _ => (s + 1).to_string(),
                                };
                                if ui.selectable_label(on, label).clicked() {
                                    go = Some((it.id, s));
                                }
                            }
                        });
                    }
                    if expanded == Some(it.id) {
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Name").small().color(MUTED));
                            let r = ui.add_enabled(!locked, egui::TextEdit::singleline(&mut name_buf).desired_width(ui.available_width()));
                            if r.changed() {
                                rename_req = Some((it.id, name_buf.clone()));
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Planned").small().color(MUTED));
                            let mut m = it.minutes.unwrap_or(0);
                            if ui.add(egui::DragValue::new(&mut m).clamp_range(0..=600).suffix(" min")).on_hover_text("0 = no time planned. Drives the countdown and the time left.").changed() {
                                minutes_req = Some((it.id, if m == 0 { None } else { Some(m) }));
                            }
                            if ui.add_enabled(!locked, egui::Button::new("Duplicate").small()).clicked() {
                                dup_req = Some(it.id);
                            }
                        });
                        ui.label(RichText::new("Private note (never shown on the stage)").small().color(MUTED));
                        let mut n = it.notes.clone();
                        if ui.add(egui::TextEdit::multiline(&mut n).desired_rows(2).desired_width(ui.available_width())).changed() {
                            notes_req = Some((it.id, n));
                        }
                    }
                });
                ui.add_space(2.0);
            }
        });

        // the drop target: always at the bottom while dragging
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::hover());
        let hot = dragging && drop_zone.contains_pointer();
        ui.painter().rect_stroke(rect, 6.0, egui::Stroke::new(if hot { 2.5 } else { 1.0 }, if hot { GOLD } else { BORDER }));
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, if dragging { "Let go here to add to the service order" } else { "Drop from the MEDIA bar here" }, egui::FontId::proportional(13.0), if hot { GOLD } else { MUTED });
        if let Some((m, _)) = &self.order.note {
            ui.label(RichText::new(m).small().color(GOLD));
        }

        // a drag that was let go over this panel adds that file
        if let Some(id) = drop_zone.dnd_release_payload::<u64>() {
            self.order_add(OrderAdd::Media(*id));
        }
        if let Some(id) = details {
            if self.order.expanded == Some(id) {
                self.order.expanded = None;
            } else {
                self.order.expanded = Some(id);
                self.order.name_buf = self.order.items().iter().find(|i| i.id == id).map(|i| i.title.clone()).unwrap_or_default();
            }
        } else {
            self.order.name_buf = name_buf;
        }
        if let Some((id, t)) = rename_req {
            self.order.rename(id, &t);
        }
        if let Some((id, m)) = minutes_req {
            self.order.set_minutes(id, m);
        }
        if let Some((id, n)) = notes_req {
            self.order.set_notes(id, &n);
        }
        if let Some(id) = dup_req {
            self.order.duplicate(id);
        }
        if let Some(id) = remove {
            self.order.remove(id);
        }
        if let Some((id, d)) = nudge {
            self.order.nudge(id, d);
        }
        if let Some((id, s)) = go {
            if self.order.runner.go(id, s).is_ok() {
                apply = true;
            }
        }
        if apply {
            self.order_apply(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rf-order-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("service-order.json")
    }

    fn media(p: &str) -> ItemKind {
        ItemKind::Media { path: p.into() }
    }

    #[test]
    fn items_are_added_in_order_removed_and_moved() {
        let mut o = Order::open(tmp("basic"));
        let a = o.add("Welcome video", media("/x/welcome.mp4")).unwrap();
        let b = o.add("Walk-in song", media("/x/walk.mp3")).unwrap();
        let c = o.add("John 3:16", ItemKind::Scripture { reference: "John 3:16".into(), translation: "KJV".into() }).unwrap();
        assert_eq!(o.items().iter().map(|i| i.id).collect::<Vec<_>>(), vec![a, b, c]);
        o.nudge(c, -1);
        assert_eq!(o.items().iter().map(|i| i.id).collect::<Vec<_>>(), vec![a, c, b]);
        o.nudge(a, -1); // already first: nothing happens
        o.nudge(b, 1); // already last: nothing happens
        assert_eq!(o.items().iter().map(|i| i.id).collect::<Vec<_>>(), vec![a, c, b]);
        o.remove(c);
        assert_eq!(o.items().len(), 2);
    }

    #[test]
    fn the_lock_stops_edits_but_not_the_show() {
        let mut o = Order::open(tmp("lock"));
        let a = o.add("A", media("/x/a.png")).unwrap();
        o.add("B", media("/x/b.png")).unwrap();
        o.set_show_mode(true);
        assert!(o.add("C", media("/x/c.png")).is_none(), "locked: not added");
        assert!(o.note.as_ref().unwrap().0.contains("Edit"), "tells the operator how to unlock");
        o.remove(a);
        assert_eq!(o.items().len(), 2, "locked: not removed");
        assert!(o.runner.next(), "next still works while locked");
        assert_eq!(o.live(), Some((a, 0)));
        o.set_show_mode(false);
        assert_eq!(o.live(), Some((a, 0)), "unlocking does not change the screen");
    }

    #[test]
    fn the_list_survives_a_restart_but_never_goes_on_screen_by_itself() {
        let path = tmp("persist");
        {
            let mut o = Order::open(path.clone());
            o.add("Hymn", ItemKind::Song { song_id: "7".into(), arrangement: vec!["Verse 1".into(), "Chorus".into()] }).unwrap();
            o.add("Sound", media("/x/walk.mp3")).unwrap();
            assert!(o.runner.next());
            o.save_now();
        }
        let o = Order::open(path);
        assert_eq!(o.items().len(), 2);
        assert_eq!(o.items()[0].slide_count(), 2, "a song has one slide per part");
        assert_eq!(o.runner.output(), Output::Clear, "a relaunch starts on a clear screen");
        assert!(o.runner.resume_point().is_some(), "but the operator can resume where they were");
    }

    #[test]
    fn a_damaged_file_is_not_trusted_and_is_not_overwritten_silently() {
        let path = tmp("damaged");
        std::fs::write(&path, b"{ this is not json").unwrap();
        let o = Order::open(path.clone());
        assert_eq!(o.items().len(), 0);
        assert!(o.note.as_ref().map_or(false, |n| n.0.contains("could not be read")), "the operator is told");
        assert_eq!(std::fs::read(&path).unwrap(), b"{ this is not json", "the old file is left alone until the next real save");
    }

    #[test]
    fn next_flows_through_every_slide_then_stops_at_the_end() {
        let mut o = Order::open(tmp("flow"));
        o.add("Hymn", ItemKind::Song { song_id: "1".into(), arrangement: vec!["V1".into(), "C".into()] }).unwrap();
        o.add("Sound", media("/x/a.mp3")).unwrap();
        let mut seen = vec![];
        while o.runner.next() {
            seen.push(o.live().unwrap().1);
        }
        assert_eq!(seen, vec![0, 1, 0], "hymn slide 1, hymn slide 2, then the sound");
        assert!(!o.runner.next(), "the end changes nothing");
    }

    #[test]
    fn kinds_have_plain_words() {
        assert_eq!(kind_word(&media("/x/a.mp3")), "sound");
        assert_eq!(kind_word(&media("/x/a.MP4")), "video");
        assert_eq!(kind_word(&media("/x/a.png")), "picture");
        assert_eq!(kind_word(&ItemKind::Timer { seconds: 60, label: "x".into() }), "timer");
    }

    fn ids(o: &Order) -> Vec<u64> {
        o.items().iter().map(|i| i.id).collect()
    }

    #[test]
    fn undo_and_redo_walk_back_and_forth_through_list_edits() {
        let mut o = Order::open(tmp("undo"));
        let a = o.add("A", media("/x/a.png")).unwrap();
        let b = o.add("B", media("/x/b.png")).unwrap();
        let c = o.add("C", media("/x/c.png")).unwrap();
        o.nudge(c, -1);
        assert_eq!(ids(&o), vec![a, c, b]);
        o.remove(a);
        assert_eq!(ids(&o), vec![c, b]);
        assert!(o.undo_edit()); // the removal
        assert_eq!(ids(&o), vec![a, c, b]);
        assert!(o.undo_edit()); // the move
        assert_eq!(ids(&o), vec![a, b, c]);
        assert!(o.redo_edit());
        assert_eq!(ids(&o), vec![a, c, b]);
        // a new edit throws the redo future away
        o.rename(a, "Alpha");
        assert!(!o.can_redo_edit());
        assert!(o.undo_edit());
        assert_eq!(o.items()[0].title, "A");
        while o.undo_edit() {}
        assert!(ids(&o).is_empty(), "all the way back to the empty list");
        assert!(!o.undo_edit(), "nothing left; says so, does not panic");
    }

    #[test]
    fn a_burst_of_typing_or_dragging_is_one_undo_step() {
        let mut o = Order::open(tmp("burst"));
        let a = o.add("A", media("/x/a.png")).unwrap();
        for t in ["Wel", "Welc", "Welcome"] {
            o.rename(a, t);
        }
        for m in 1..=8 {
            o.set_minutes(a, Some(m));
        }
        assert_eq!((o.items()[0].title.as_str(), o.items()[0].minutes), ("Welcome", Some(8)));
        assert!(o.undo_edit()); // minutes
        assert_eq!(o.items()[0].minutes, None);
        assert!(o.undo_edit()); // the name
        assert_eq!(o.items()[0].title, "A");
    }

    #[test]
    fn undo_is_refused_while_locked_and_keeps_its_history() {
        let mut o = Order::open(tmp("locked-undo"));
        let a = o.add("A", media("/x/a.png")).unwrap();
        o.set_show_mode(true);
        assert!(!o.undo_edit());
        assert_eq!(ids(&o), vec![a]);
        o.set_show_mode(false);
        assert!(o.undo_edit(), "the step was not lost");
        assert!(ids(&o).is_empty());
    }

    #[test]
    fn undoing_the_add_of_the_live_item_clears_the_screen() {
        let mut o = Order::open(tmp("undo-live"));
        let _a = o.add("A", media("/x/a.png")).unwrap();
        let b = o.add("B", media("/x/b.png")).unwrap();
        o.runner.go(b, 0).unwrap();
        assert_eq!(o.live(), Some((b, 0)));
        assert!(o.undo_edit());
        assert_eq!(o.live(), None, "never left pointing at an item that is gone");
    }

    #[test]
    fn notes_minutes_and_copies_survive_a_restart() {
        let path = tmp("notes");
        let mut o = Order::open(path.clone());
        let a = o.add("Sermon", media("/x/s.png")).unwrap();
        o.set_notes(a, "wait for the pastor to nod");
        o.set_minutes(a, Some(35));
        let c = o.duplicate(a).unwrap();
        assert_eq!(o.items()[1].id, c, "the copy sits right under the original");
        assert_eq!((o.items()[1].title.as_str(), o.items()[1].notes.as_str(), o.items()[1].minutes), ("Sermon (copy)", "wait for the pastor to nod", Some(35)));
        o.save_now();
        let again = Order::open(path);
        assert_eq!(again.items().len(), 2);
        assert_eq!(again.items()[0].notes, "wait for the pastor to nod");
        assert_eq!(again.items()[0].minutes, Some(35));
    }

    #[test]
    fn the_list_adds_up_and_says_what_is_next() {
        let mut o = Order::open(tmp("sum"));
        let a = o.add("Welcome", media("/x/a.png")).unwrap();
        let b = o.add("Song", ItemKind::Song { song_id: "s".into(), arrangement: vec!["V1".into(), "C".into()] }).unwrap();
        let c = o.add("Sermon", media("/x/s.png")).unwrap();
        o.set_minutes(a, Some(5));
        o.set_minutes(b, Some(10));
        let s = summarize(o.items(), None);
        assert_eq!((s.total_min, s.left_min, s.untimed), (15, 15, 1));
        let s = summarize(o.items(), Some(b));
        assert_eq!((s.total_min, s.left_min), (15, 10), "time left counts from the item on the screen");
        assert_eq!(summarize(&[], None), RunSummary::default());
        assert_eq!(next_up(o.items(), Some((a, 0))).as_deref(), Some("Song"));
        assert_eq!(next_up(o.items(), Some((b, 0))).as_deref(), Some("Song \u{2014} part 2 of 2"));
        assert_eq!(next_up(o.items(), Some((b, 1))).as_deref(), Some("Sermon"));
        assert_eq!(next_up(o.items(), Some((c, 0))), None, "nothing after the last item");
        assert_eq!(next_up(o.items(), None), None);
        assert_eq!(minutes_text(42), "42 min");
        assert_eq!(minutes_text(65), "1 h 05 min");
    }

    #[test]
    fn the_item_clock_counts_down_then_over_or_just_up() {
        assert_eq!(item_clock(Some(5), 134), ItemClock { text: "2:46 left".into(), over: false, soon: false });
        assert!(item_clock(Some(5), 250).soon);
        assert_eq!(item_clock(Some(5), 300).text, "0:00 left");
        assert_eq!(item_clock(Some(5), 330), ItemClock { text: "0:30 over".into(), over: true, soon: false });
        assert_eq!(item_clock(None, 75).text, "1:15 on screen");
        assert_eq!(item_clock(Some(0), 75).text, "1:15 on screen");
    }

    #[test]
    fn the_run_sheet_text_lists_everything_in_order() {
        let mut o = Order::open(tmp("sheet"));
        let a = o.add("Welcome", media("/x/a.mp4")).unwrap();
        o.set_notes(a, "line one\nline two");
        o.set_minutes(a, Some(3));
        o.add("Offering", media("/x/o.png")).unwrap();
        let t = o.run_sheet_text();
        assert!(t.contains(" 1. Welcome  (video)  [3 min]"), "{t}");
        assert!(t.contains("note: line one / line two"), "{t}");
        assert!(t.contains(" 2. Offering  (picture)"), "{t}");
        assert!(t.contains("Planned time: 3 min"), "{t}");
    }

    #[test]
    fn the_dock_tab_is_remembered_by_name_and_a_bad_name_falls_back_to_order() {
        for t in [RightTab::Order, RightTab::Review, RightTab::Strongs, RightTab::History, RightTab::Index] {
            assert_eq!(RightTab::from_key(t.key()), t);
        }
        assert_eq!(RightTab::from_key("nonsense"), RightTab::Order);
        assert_eq!(RightTab::from_key(""), RightTab::Order);
    }

    #[test]
    fn dock_tabs_never_need_a_second_line() {
        // ~6.6 points per letter + padding: the widest set must fit when the dock is wide, the tightest when it is narrow
        let wide = dock_tab_labels(400.0, 3, 2, 12);
        assert_eq!(wide[0], "Order 3");
        assert_eq!(wide[3], "History 12");
        for w in [210.0_f32, 230.0, 260.0, 300.0, 380.0] {
            let l = dock_tab_labels(w, 12, 12, 120);
            let need: f32 = l.iter().map(|s| s.chars().count() as f32 * 6.6 + 12.0).sum();
            assert!(need <= w, "width {w}: {l:?} needs {need}");
        }
        assert_eq!(dock_tab_labels(210.0, 1, 0, 0)[2], "G/H");
    }
}
