//! Stage 2 operator side (v0.815.0): pictures, video, slides, songs, backgrounds, timers.
//!
//! Everything here is plain state + methods so it can be tested without a window. The window
//! (`Present::ui`) is a thin layer on top. Rules are in docs/STAGE2-SPEC.md; the pure parts
//! (slide parsing, song library, timers, fit maths, video clock) live in `rf-service`, and the
//! pixel work (pictures, H.264 video) lives in `rf-media`.

use crate::docview::{self, DocAction, DocState};
use crate::sound::Sound;
use crate::stage::{ArtImage, StageArt};
use crate::state::{StageContent, StageKind};
use crate::theme;
use rf_media::{load_image, Player, Rgba, MAX_IMAGE_SIDE};
use rf_service::media::{classify, Classified, FitMode, MediaKind, DEFAULT_DIM_PCT, MAX_DIM_PCT};
use rf_service::library::{planned_work, prepare, Added, LibError, MediaLibrary, Mode as ShelfMode, Policy as ShelfPolicy, Prepared, SortBy};
use rf_service::slides::{slides_from_text, text_from_bytes, text_from_pptx, ParsedSlide};
use rf_service::songs::{load_library, save_library, Library, Song};
use rf_service::timer::{format_time_of_day, parse_time_of_day, Phase, Timer, TimerKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The text on the clipboard, if there is any and it can be read (best effort; the "Paste" buttons).
/// Created fresh each time because a held clipboard handle can block other apps on some systems.
fn clipboard_text() -> Option<String> {
    match arboard::Clipboard::new() {
        Ok(mut c) => c.get_text().ok().filter(|t| !t.trim().is_empty()),
        Err(_) => None,
    }
}

/// Adding a file (reading it, fingerprinting it, copying it) happens on a helper thread so the
/// window never freezes; these carry the work there and the result back.
struct JobReq {
    id: u64,
    src: PathBuf,
    policy: ShelfPolicy,
    dir: PathBuf,
    done: Arc<AtomicU64>,
    total: u64,
    ctx: egui::Context,
}
struct JobDone {
    id: u64,
    res: Result<Prepared, LibError>,
}
struct JobInfo {
    id: u64,
    folder: String,
    word: &'static str,
    name: String,
    total: u64,
    done: Arc<AtomicU64>,
}

/// Selected files: teal (the blue border is "on screen now", gold is "mouse is over it").
const SEL: egui::Color32 = egui::Color32::from_rgb(64, 200, 190);
const SEL_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(10, 40, 38, 60);

fn human_size(b: u64) -> String {
    if b >= 1_000_000_000 {
        format!("{:.1} GB", b as f64 / 1e9)
    } else if b >= 1_000_000 {
        format!("{:.1} MB", b as f64 / 1e6)
    } else {
        format!("{} KB", (b / 1000).max(1))
    }
}

/// What the MEDIA bar's widgets asked for this frame (applied after drawing).
#[derive(Default)]
struct DockOut {
    /// "last shown 3 days ago" for the item whose menu is open (v0.850.0)
    menu_note: String,
    show: Option<u64>,
    remove: Option<u64>,
    relink: Option<u64>,
    rename: Option<(u64, String, bool)>,
    moves: Vec<(u64, String)>,
    new_folder: bool,
    clicked: Option<u64>,
    to_order: Option<u64>,
    /// Filled once per frame for the right-click "When it ends" list: (id, name) of everything on the shelf.
    choices: Vec<(u64, String, String)>,
    /// What each timed item does at its end (for the tick marks in the menu), by id.
    after_now: HashMap<u64, AfterEnd>,
    set_after: Option<(u64, AfterEnd)>,
    duplicate: Option<u64>,
    reveal: Option<u64>,
    /// (id, Cmd held, Shift held): a click on a file this frame (v0.844.0)
    picked: Option<(u64, bool, bool)>,
    /// how many files are selected (the right-click menu offers "Copy / Remove N selected")
    sel_count: usize,
    copy_sel: bool,
    remove_sel: bool,
    /// a drag of a file began on this id (selects it first if it was not selected)
    drag_began: Option<u64>,
    /// where each drawn file is, for the drag box
    rects: Vec<(u64, [f32; 4])>,
}

fn item_menu(ui: &mut egui::Ui, id: u64, name: &str, is_missing: bool, timed: bool, folders: &[String], out: &mut DockOut) {
    if !out.menu_note.is_empty() {
        ui.label(egui::RichText::new(out.menu_note.clone()).small().color(theme::MUTED));
        ui.separator();
    }
    if out.sel_count > 1 {
        if ui.button(format!("Copy {} selected files   (Cmd+C)", out.sel_count)).clicked() {
            out.copy_sel = true;
            ui.close_menu();
        }
        if ui.button(format!("Remove {} selected from RhemaFlow   (Delete)", out.sel_count)).clicked() {
            out.remove_sel = true;
            ui.close_menu();
        }
        ui.separator();
    } else if ui.button("Copy file   (Cmd+C)").clicked() {
        out.copy_sel = true;
        ui.close_menu();
    }
    if !is_missing && ui.button("Show on screen").clicked() {
        out.show = Some(id);
        ui.close_menu();
    }
    if ui.button("Rename\u{2026}").clicked() {
        out.rename = Some((id, name.to_string(), false));
        ui.close_menu();
    }
    if ui.button("Add to service order").clicked() {
        out.to_order = Some(id);
        ui.close_menu();
    }
    if ui.button("Duplicate").on_hover_text("List this file a second time (it shares the same stored copy)").clicked() {
        out.duplicate = Some(id);
        ui.close_menu();
    }
    if cfg!(target_os = "macos") && !is_missing && ui.button("Show in Finder").clicked() {
        out.reveal = Some(id);
        ui.close_menu();
    }
    if timed {
        let now = out.after_now.get(&id).cloned().unwrap_or(AfterEnd::Nothing);
        let title = format!("When it ends: {}", after_label(&now, &shown_name(&out.choices, &now)));
        ui.menu_button(title, |ui| {
            if ui.selectable_label(now == AfterEnd::Nothing, "Nothing (stay as it is)").clicked() {
                out.set_after = Some((id, AfterEnd::Nothing));
                ui.close_menu();
            }
            if ui.selectable_label(now == AfterEnd::Clear, "Clear the screen").clicked() {
                out.set_after = Some((id, AfterEnd::Clear));
                ui.close_menu();
            }
            ui.separator();
            ui.label(egui::RichText::new("Show next (picture, video or sound):").small());
            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                let choices = out.choices.clone();
                for (_cid, cname, cpath) in choices.iter().filter(|(c, _, _)| *c != id) {
                    let sel = matches!(&now, AfterEnd::Show(p) if p == cpath);
                    if ui.selectable_label(sel, cname).clicked() {
                        out.set_after = Some((id, AfterEnd::Show(cpath.clone())));
                        ui.close_menu();
                    }
                }
            });
            ui.label(egui::RichText::new("Does not happen while Loop is on.").small().color(theme::MUTED));
        });
    }
    ui.menu_button("Move to folder", |ui| {
        if ui.button("(no folder)").clicked() {
            out.moves.push((id, String::new()));
            ui.close_menu();
        }
        for f in folders {
            if ui.button(f).clicked() {
                out.moves.push((id, f.clone()));
                ui.close_menu();
            }
        }
    });
    if is_missing && ui.button("Re-link (path in Options)").clicked() {
        out.relink = Some(id);
        ui.close_menu();
    }
    ui.separator();
    if ui.button("Remove from RhemaFlow").clicked() {
        out.remove = Some(id);
        ui.close_menu();
    }
}

/// `Show: Easter.mp4` / `Clear the screen` / `Nothing`.
fn after_label(a: &AfterEnd, shown: &str) -> String {
    match a {
        AfterEnd::Nothing => "Nothing".into(),
        AfterEnd::Clear => "Clear the screen".into(),
        AfterEnd::Show(_) => format!("Show {shown}"),
    }
}

/// The name the operator knows the "show next" item by (its shelf name, not the stored file's number-name).
fn shown_name(choices: &[(u64, String, String)], a: &AfterEnd) -> String {
    match a {
        AfterEnd::Show(p) => choices.iter().find(|(_, _, cp)| cp == p).map(|(_, n, _)| n.clone()).unwrap_or_else(|| file_name(p)),
        _ => String::new(),
    }
}

pub const CISCO_CREDIT: &str = "OpenH264 Video Codec provided by Cisco Systems, Inc.";

/// True while a video is playing: a small helper thread (main.rs) then asks for a redraw every frame, even when
/// macOS would otherwise let the window sleep, so the picture on the stage never freezes behind the sound.
pub static KEEP_AWAKE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Media,
    Slides,
    Songs,
    Background,
    Timers,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickFor {
    Media,
    Song,
    Background,
}

/// What happens when a video or sound reaches its end (and Loop is off).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AfterEnd {
    /// Stay as it is (a video keeps its last picture).
    Nothing,
    /// Clear the screen.
    Clear,
    /// Show this file next (a picture, a video or a sound), by its path.
    Show(String),
}

impl AfterEnd {
    /// How it is saved: nothing is not saved at all; "clear"; or "show:<path>".
    pub fn encode(&self) -> Option<String> {
        match self {
            AfterEnd::Nothing => None,
            AfterEnd::Clear => Some("clear".into()),
            AfterEnd::Show(p) => Some(format!("show:{p}")),
        }
    }
    pub fn decode(v: &str) -> AfterEnd {
        if v == "clear" {
            AfterEnd::Clear
        } else if let Some(p) = v.strip_prefix("show:") {
            if p.is_empty() { AfterEnd::Nothing } else { AfterEnd::Show(p.to_string()) }
        } else {
            AfterEnd::Nothing
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct PresentSettings {
    /// Per file path: what to do when it ends (see `AfterEnd`). Missing = nothing.
    pub after_end: std::collections::BTreeMap<String, String>,
    pub media: Vec<String>,
    /// "fit" | "fill" | "stretch"
    pub fit: String,
    pub loop_video: bool,
    pub mute: bool,
    pub background: Option<String>,
    pub dim_pct: u8,
    pub lines_per_slide: usize,
    pub twelve_hour: bool,
    pub slides_text: String,
    /// How new pictures/videos join the shelf: "auto" (copy, but link very big videos) | "copy" | "link"
    pub shelf_policy: String,
    /// the church's CCLI licence number (printed under songs that have a CCLI song number)
    pub ccli_licence: String,
    /// MEDIA bar: thumbnail width in points (80..=200), sort ("added"|"newest"|"name"), view ("grid"|"list")
    pub thumb_w: f32,
    pub sort: String,
    pub view: String,
    /// ms the picture waits after a jump so the sound (which starts late) lines up with it
    pub sound_sync_ms: u64,
    /// Where a video, sound or document was when it left the stage (ms, or the page number): coming back carries on from there (v0.840.0)
    pub resume: std::collections::BTreeMap<String, u64>,
    /// documents open filling the screen's width (true) or showing the whole page (false)
    pub doc_fit_width: bool,
    /// zoom (1.0 = the whole page) each document was last shown at, by path (v0.841.0)
    pub doc_zoom: std::collections::BTreeMap<String, f32>,
    /// how many lines the document reading line covers at once: 1, 2 or 3 (v0.842.0)
    pub doc_guide_lines: usize,
    /// which reading-line highlight colour: an index into stage::DOC_GUIDE_COLORS (v0.842.0)
    pub doc_guide_color: u8,
}

impl Default for PresentSettings {
    fn default() -> Self {
        PresentSettings {
            after_end: Default::default(),
            media: Vec::new(),
            fit: "fit".into(),
            loop_video: false,
            mute: false,
            background: None,
            dim_pct: DEFAULT_DIM_PCT,
            lines_per_slide: 4,
            twelve_hour: false,
            slides_text: String::new(),
            shelf_policy: "copy".into(),
            ccli_licence: String::new(),
            thumb_w: 124.0,
            sort: "added".into(),
            view: "grid".into(),
            sound_sync_ms: 500,
            resume: Default::default(),
            doc_fit_width: false,
            doc_zoom: Default::default(),
            doc_guide_lines: 1,
            doc_guide_color: 0,
        }
    }
}

fn fit_of(key: &str) -> FitMode {
    match key {
        "fill" => FitMode::Fill,
        "stretch" => FitMode::Stretch,
        _ => FitMode::Fit,
    }
}
fn fit_key(m: FitMode) -> &'static str {
    match m {
        FitMode::Fit => "fit",
        FitMode::Fill => "fill",
        FitMode::Stretch => "stretch",
    }
}

fn file_name(p: &str) -> String {
    Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.to_string())
}

/// A path typed or dragged in: strip quotes and the backslashes Terminal puts before spaces.
/// A zoom worth keeping: anything that is not the plain whole-page view (1.0), kept inside the zoom limits.
/// `None` means "forget it" (the document opens at the whole page again).
pub fn zoom_to_remember(z: Option<f32>) -> Option<f32> {
    let z = z?;
    if !z.is_finite() || (z - 1.0).abs() < 0.01 {
        return None;
    }
    Some(z.clamp(0.5, 5.0))
}

pub fn clean_path(s: &str) -> String {
    let t = s.trim().trim_matches(|c| c == '"' || c == '\'');
    t.replace("\\ ", " ")
}

/// Everything the play bar shows, for a video or a sound.
#[derive(Clone, Debug, PartialEq)]
pub struct Transport {
    pub pos_ms: u64,
    pub dur_ms: u64,
    pub playing: bool,
    pub ended: bool,
    pub looping: bool,
    pub is_video: bool,
    /// Jumping needs ffmpeg for sound (video can always jump).
    pub can_jump: bool,
    /// A jump in a sound is being prepared.
    pub seeking: bool,
}

/// `3:07` or `1:02:09`.
pub fn fmt_clock(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// `-2:42 left` (or nothing if the length is unknown).
pub fn left_text(pos_ms: u64, dur_ms: u64) -> String {
    if dur_ms == 0 { String::new() } else { format!("-{} left", fmt_clock(dur_ms.saturating_sub(pos_ms))) }
}

/// `ends 10:42:05` : the clock time it will finish, if it keeps playing (`wall_ms` = ms since midnight).
pub fn ends_text(pos_ms: u64, dur_ms: u64, wall_ms: u64) -> String {
    if dur_ms == 0 {
        return String::new();
    }
    let at = (wall_ms + dur_ms.saturating_sub(pos_ms)) % 86_400_000 / 1000;
    format!("ends {:02}:{:02}:{:02}", at / 3600, (at / 60) % 60, at % 60)
}

/// The line under the buttons: state, `1:23 / 4:05`, time left, finish time.
pub fn transport_line(t: &Transport, wall_ms: u64) -> String {
    let state = if t.seeking { "jumping\u{2026}" } else if t.ended { "ended" } else if t.playing { "playing" } else { "paused" };
    let mut out = format!("{state}  {}", fmt_clock(t.pos_ms));
    if t.dur_ms > 0 {
        out.push_str(&format!(" / {}", fmt_clock(t.dur_ms)));
        if t.looping {
            out.push_str("  \u{b7} looping");
        } else if t.playing {
            out.push_str(&format!("  \u{b7} {}  \u{b7} {}", left_text(t.pos_ms, t.dur_ms), ends_text(t.pos_ms, t.dur_ms, wall_ms)));
        } else if !t.ended {
            out.push_str(&format!("  \u{b7} {}", left_text(t.pos_ms, t.dur_ms)));
        }
    }
    out
}

/// A video or sound left earlier than this (or this close to its end) starts from the top next time.
pub const RESUME_MIN_MS: u64 = 3_000;

/// How far the Back / Forward buttons jump.
pub const SKIP_MS: i64 = 10_000;

/// What is on the stage right now as media.
pub enum Loaded {
    Image { tex: egui::TextureHandle, size: [f32; 2] },
    Video { player: Player, tex: Option<egui::TextureHandle>, serial: u64, size: [f32; 2], sound: Sound, path: String, last_pos: u64, error_told: bool },
    /// A sound file (mp3, wav, m4a ...): no picture, played by the Mac's own player.
    Audio { sound: Sound, path: String, paused: bool },
    /// A PDF or HTML page on the stage, with its zoom and reading line (v0.840.0).
    Document(DocState),
}

pub struct Background {
    pub name: String,
    pub tex: Option<egui::TextureHandle>,
    pub size: [f32; 2],
    pub player: Option<(Player, u64)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimerChoice {
    CountDown,
    UntilClock,
    StopWatch,
    Clock,
}

pub struct TimerUi {
    pub choice: TimerChoice,
    pub title: String,
    pub minutes: u32,
    pub until_text: String,
}

#[derive(Default)]
pub struct SongEdit {
    pub title: String,
    pub author: String,
    pub ccli_song: String,
    pub copyright: String,
    pub lyrics: String,
    pub arrangement: String,
}

/// Things the media windows ask the service order to add (the app drains this every frame).
#[derive(Clone, Debug, PartialEq)]
pub enum OrderAdd {
    Media(u64),
    Song(u64),
}

pub struct Present {
    /// scroll the song editor into view on the next frame (the Edit button)
    edit_focus: bool,
    /// the song waiting for a second click on Delete
    confirm_delete: Option<u64>,
    scroll_slide_to: Option<usize>,
    /// the editor's text as it was when loaded or saved (to show "unsaved changes")
    song_edit_base: String,
    dir: PathBuf,
    pub order_outbox: Vec<OrderAdd>,
    pub open: bool,
    pub tab: Tab,
    pub settings: PresentSettings,
    // slides
    pub slides: Vec<ParsedSlide>,
    pub slide_index: Option<usize>,
    pub slides_source: String,
    // songs
    pub library: Library,
    pub song_query: String,
    pub song_sel: Option<u64>,
    pub song_edit: SongEdit,
    pub ccli_logged: std::collections::HashSet<u64>,
    /// songs put on screen this run, in order (for the handout)
    pub shown_songs: Vec<u64>,
    /// Songs shown for the first time this run, not yet noted by the Studio usage counter.
    pub newly_shown: Vec<u64>,
    // media
    pub loaded: Option<Loaded>,
    pub loaded_idx: Option<usize>,
    /// which SHELF ITEM is on the stage (two items may share one file, so the path alone cannot say)
    pub loaded_id: Option<u64>,
    /// page-making jobs for documents, by file path (also used for the shelf thumbnails)
    doc_jobs: HashMap<String, rf_media::document::DocJob>,
    doc_root: PathBuf,
    /// While the play bar's slider is being dragged: where it points (the jump happens on release).
    pub scrub_ms: Option<u64>,
    /// picture under the scrub bar (v0.835.0)
    pub scrub: crate::scrub::Scrub,
    /// "Paste lyrics" form (v0.835.0)
    pub paste_open: bool,
    pub paste_title: String,
    pub paste_text: String,
    pub paste_notes: Vec<String>,
    /// volunteer lock: deleting is refused (v0.835.0)
    pub locked: bool,
    /// The "when it ends" step has already happened for what is on the stage.
    after_fired: bool,
    /// The media shelf: pictures and videos added once, shown later by clicking their thumbnail.
    pub shelf: MediaLibrary,
    pub shelf_view: Option<String>,
    pub shelf_filter: String,
    /// words of every PDF on the shelf, read once in the background: path -> text of each page (v0.850.0)
    pub pdf_texts: std::sync::Arc<std::sync::Mutex<HashMap<String, Vec<String>>>>,
    pub pdf_started: std::collections::HashSet<String>,
    /// (filter, PDFs read when computed, hits as (item id, name, page, snippet))
    pub pdf_hit_cache: (String, usize, Vec<(u64, String, usize, String)>),
    /// the "Sunday check" list, shown under its button
    pub sunday_lines: Vec<String>,
    /// when the last backup finished (unix seconds, 0 = never); set by the app each frame
    pub last_backup_unix: i64,
    pub new_shelf: String,
    /// the item being renamed or moved: (id, text)
    pub shelf_edit: Option<(u64, String, bool)>,
    /// is the MEDIA bar at the bottom of the main window open (true) or folded to one line?
    pub dock_open: bool,
    /// on a short window the media bar starts folded (once), so the stage preview keeps its room
    dock_auto_done: bool,
    /// the keyboard highlight in the MEDIA bar (arrow keys move it, Enter shows it)
    pub dock_cursor: Option<u64>,
    dock_rows_seen: (usize, usize),
    /// the files selected in the MEDIA bar (v0.844.0): click, Cmd+click, Shift+click, drag a box, Cmd+A
    pub dock_sel: std::collections::HashSet<u64>,
    /// where a Shift+click range starts
    dock_anchor: Option<u64>,
    /// where each file was drawn last frame (screen points), for the drag box
    dock_rects: Vec<(u64, [f32; 4])>,
    /// a drag box in progress: where it started, and what was selected before (kept when Cmd is held)
    dock_box: Option<([f32; 2], std::collections::HashSet<u64>)>,
    /// Delete was pressed once: a second press within a few seconds removes the selected files
    dock_del_armed: Option<Instant>,
    /// add files on a helper thread (the app turns this on; tests keep it off and add instantly)
    pub bg_adds: bool,
    jobs: Vec<JobInfo>,
    job_seq: u64,
    job_tx: Option<Sender<JobReq>>,
    done_tx: Sender<JobDone>,
    done_rx: Receiver<JobDone>,
    batch_added: usize,
    batch_known: usize,
    /// folder being named: (None = a new folder | Some(old name) = rename, text)
    pub folder_edit: Option<(Option<String>, String)>,
    /// where each folder row was drawn last frame (so files dropped from Finder land in the right folder)
    folder_rects: Vec<(String, egui::Rect)>,
    /// a folder chosen for the files being added right now ("" = none)
    pub target_folder: Option<String>,
    durations: HashMap<u64, u64>,
    thumb_busy: bool,
    /// video previews are made on helper threads (a big video must never freeze the window)
    thumb_tx: std::sync::mpsc::Sender<(u64, Option<(rf_media::Rgba, u64)>)>,
    thumb_rx: std::sync::mpsc::Receiver<(u64, Option<(rf_media::Rgba, u64)>)>,
    thumb_pending: std::collections::HashSet<u64>,
    thumbs: HashMap<u64, Option<egui::TextureHandle>>,
    pub bg: Option<Background>,
    // timers
    pub timer: Timer,
    pub timer_ui: TimerUi,
    clock_base: (u64, Instant),
    t0: Instant,
    // messages for the operator
    pub msg: Option<(String, Instant)>,
    /// set when something should also show in the main window's note line
    pub note_for_app: Option<String>,
    pub path_input: String,
    pick: Option<(PickFor, Receiver<Vec<PathBuf>>)>,
}

/// "Verse 3:" (next number), or the plain name for the other parts (v0.845.0).
fn next_part_header(lyrics: &str, kind: &str) -> String {
    if kind == "Verse" {
        let n = lyrics.lines().filter(|l| { let l = l.trim().to_lowercase(); l.starts_with("verse") && l.ends_with(':') }).count();
        format!("Verse {}:", n + 1)
    } else {
        format!("{kind}:")
    }
}

fn append_part(lyrics: &str, header: &str) -> String {
    if lyrics.trim().is_empty() { format!("{header}\n") } else { format!("{}\n\n{header}\n", lyrics.trim_end()) }
}

/// Trailing spaces off every line, at most one blank line between parts, no blank lines at the ends.
fn tidy_lyrics(t: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in t.replace("\r\n", "\n").replace('\r', "\n").lines() {
        let l = line.trim_end().to_string();
        if l.is_empty() && out.last().map_or(true, |p| p.is_empty()) { continue; }
        out.push(l);
    }
    while out.last().map_or(false, |l| l.is_empty()) { out.pop(); }
    out.join("\n")
}

#[cfg(test)]
mod song_edit_helper_tests {
    use super::*;
    #[test]
    fn part_buttons_and_tidy() {
        assert_eq!(next_part_header("Verse 1:\nx\n\nChorus:\ny", "Verse"), "Verse 2:");
        assert_eq!(next_part_header("", "Bridge"), "Bridge:");
        assert_eq!(append_part("", "Verse 1:"), "Verse 1:\n");
        assert_eq!(append_part("Verse 1:\nhi  \n", "Chorus:"), "Verse 1:\nhi\n\nChorus:\n");
        assert_eq!(tidy_lyrics("\n\nVerse 1:  \nhi\n\n\n\nChorus:\nyo\n\n"), "Verse 1:\nhi\n\nChorus:\nyo");
    }
}

/// The part names in the words box, in order: "Verse 1", "Chorus"... (lines ending in a colon).
fn part_headers(lyrics: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in lyrics.lines() {
        let l = l.trim();
        if let Some(h) = l.strip_suffix(':') {
            let h = h.trim();
            if !h.is_empty() && h.chars().count() <= 30 && !out.iter().any(|o| o == h) {
                out.push(h.to_string());
            }
        }
    }
    out
}

/// "Order sung" with `part` added at the end. An empty order first becomes every part in the order written.
fn repeat_in_order(lyrics: &str, arrangement: &str, part: &str) -> String {
    let base = if arrangement.trim().is_empty() { part_headers(lyrics).join(", ") } else { arrangement.trim().trim_end_matches(',').to_string() };
    if base.is_empty() { part.to_string() } else { format!("{base}, {part}") }
}

fn is_note(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some('A'..='G')) && matches!(c.as_str(), "" | "#" | "b")
}

/// Am7, F#m, C/E, Gsus4, Dmaj7, Bb ... but not words such as Amen or Dad.
fn is_chord(tok: &str) -> bool {
    let t = tok.trim_matches(|c| c == '(' || c == ')');
    let (main, bass) = match t.split_once('/') {
        Some((a, b)) => (a, Some(b)),
        None => (t, None),
    };
    if let Some(b) = bass {
        if !is_note(b) {
            return false;
        }
    }
    let mut chars = main.chars();
    if !matches!(chars.next(), Some('A'..='G')) {
        return false;
    }
    let mut rest = chars.as_str();
    if let Some(r) = rest.strip_prefix('#').or_else(|| rest.strip_prefix('b')) {
        rest = r;
    }
    'outer: while !rest.is_empty() {
        for p in ["maj", "min", "dim", "aug", "sus", "add", "m", "M", "+", "-", "#", "b", "\u{b0}"] {
            if let Some(r) = rest.strip_prefix(p) {
                rest = r;
                continue 'outer;
            }
        }
        if let Some(r) = rest.strip_prefix(|c: char| c.is_ascii_digit()) {
            rest = r;
            continue;
        }
        return false;
    }
    true
}

fn is_chord_line(line: &str) -> bool {
    let toks: Vec<&str> = line.split_whitespace().collect();
    !toks.is_empty() && toks.iter().all(|t| is_chord(t))
}

/// Words copied from a web page or a chord sheet -> plain words: web tags and entities gone, chord lines dropped,
/// "[Verse 1]" -> "Verse 1:", one blank line between parts.
fn clean_pasted_lyrics(t: &str) -> String {
    let mut no_tags = String::new();
    let mut in_tag = false;
    for c in t.replace("<br>", "\n").replace("<br/>", "\n").replace("&nbsp;", " ").replace("&amp;", "&").chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => no_tags.push(c),
            _ => {}
        }
    }
    let mut out: Vec<String> = Vec::new();
    for line in no_tags.replace("\r\n", "\n").lines() {
        let l = line.trim();
        if is_chord_line(l) {
            continue;
        }
        let l = match l.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
            Some(h) if !h.trim().is_empty() => format!("{}:", h.trim()),
            _ => l.to_string(),
        };
        if l.is_empty() && out.last().map_or(true, |p| p.is_empty()) {
            continue;
        }
        out.push(l);
    }
    while out.last().map_or(false, |l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

#[cfg(test)]
mod song_clean_tests {
    use super::*;
    #[test]
    fn chords_web_junk_and_repeats() {
        for c in ["Am7", "F#m", "C/E", "Gsus4", "Dmaj7", "Bb", "(G)"] {
            assert!(is_chord(c), "{c}");
        }
        for w in ["Amen", "Dad", "Be", "God", "Gone", "A1x"] {
            assert!(!is_chord(w), "{w}");
        }
        assert!(is_chord_line("G   D/F#  Em7  C") && !is_chord_line("Amen to that"));
        assert_eq!(clean_pasted_lyrics("[Verse 1]\nG    D\nJesus you are the best<br>\n\n\n[Chorus]\nAm  F\nHoly &amp; true\n"), "Verse 1:\nJesus you are the best\n\nChorus:\nHoly & true");
        assert_eq!(part_headers("Verse 1:\nx\n\nChorus:\ny\n\nChorus:\nz"), vec!["Verse 1", "Chorus"]);
        assert_eq!(repeat_in_order("Verse 1:\nx\n\nChorus:\ny", "", "Chorus"), "Verse 1, Chorus, Chorus");
        assert_eq!(repeat_in_order("Verse 1:\nx", "Verse 1, Chorus,", "Verse 1"), "Verse 1, Chorus, Verse 1");
    }
}

/// A song pasted with every part written out (no part names) -> named parts + the order sung. A block that appears
/// more than once becomes "Chorus" (kept once); the others are "Verse 1", "Verse 2"... Blocks that already have a
/// name line ("Bridge:") keep it. Returns (words, order); words is empty when there is nothing to work on.
fn detect_repeats(lyrics: &str) -> (String, String) {
    struct G {
        header: Option<String>,
        body: String,
        norm: String,
        count: usize,
    }
    let norm = |s: &str| s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    let text = lyrics.replace("\r\n", "\n");
    let mut groups: Vec<G> = Vec::new();
    let mut seq: Vec<usize> = Vec::new();
    for block in text.split("\n\n") {
        let lines: Vec<&str> = block.lines().map(|l| l.trim_end()).filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            continue;
        }
        let (header, body_lines) = match lines[0].trim().strip_suffix(':') {
            Some(h) if !h.trim().is_empty() && h.chars().count() <= 30 && lines.len() > 1 => (Some(h.trim().to_string()), &lines[1..]),
            _ => (None, &lines[..]),
        };
        let body = body_lines.join("\n");
        let n = norm(&body);
        let found = groups.iter().position(|g| g.norm == n && (header.is_none() || g.header.is_none() || g.header == header));
        match found {
            Some(i) => {
                groups[i].count += 1;
                seq.push(i);
            }
            None => {
                groups.push(G { header, body, norm: n, count: 1 });
                seq.push(groups.len() - 1);
            }
        }
    }
    if groups.is_empty() {
        return (String::new(), String::new());
    }
    let (mut verse, mut chorus) = (0, 0);
    let names: Vec<String> = groups
        .iter()
        .map(|g| match &g.header {
            Some(h) => h.clone(),
            None if g.count > 1 => {
                chorus += 1;
                if chorus == 1 { "Chorus".to_string() } else { format!("Chorus {chorus}") }
            }
            None => {
                verse += 1;
                format!("Verse {verse}")
            }
        })
        .collect();
    let words = groups.iter().zip(&names).map(|(g, n)| format!("{n}:\n{}", g.body)).collect::<Vec<_>>().join("\n\n");
    let order = seq.iter().map(|i| names[*i].clone()).collect::<Vec<_>>().join(", ");
    (words, order)
}

#[cfg(test)]
mod song_repeat_tests {
    use super::*;
    #[test]
    fn repeated_blocks_become_a_chorus_and_the_order_is_filled() {
        let t = "Line a\nline b\n\nHoly holy\nGod is here\n\nLine c\nline d\n\nHoly holy\nGod is here!\n";
        let (w, o) = detect_repeats(t);
        assert_eq!(o, "Verse 1, Chorus, Verse 2, Chorus");
        assert_eq!(w, "Verse 1:\nLine a\nline b\n\nChorus:\nHoly holy\nGod is here\n\nVerse 2:\nLine c\nline d");
        let (w2, o2) = detect_repeats("Bridge:\nOh oh\n\nVerse 1:\nx\ny");
        assert_eq!(o2, "Bridge, Verse 1");
        assert!(w2.starts_with("Bridge:\nOh oh"));
        assert_eq!(detect_repeats("  \n\n"), (String::new(), String::new()));
    }
}

/// Lines longer than `max` letters are broken at a space (a part's name line such as "Chorus:" is left alone), so a
/// long line does not run off the screen (v0.850.0).
fn wrap_long_lines(t: &str, max: usize) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in t.replace("\r\n", "\n").lines() {
        let l = line.trim_end();
        if l.chars().count() <= max || l.trim_end().ends_with(':') {
            out.push(l.to_string());
            continue;
        }
        let mut cur = String::new();
        for w in l.split_whitespace() {
            if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > max {
                out.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(w);
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod song_wrap_tests {
    use super::*;
    #[test]
    fn long_lines_break_at_spaces_and_short_ones_and_headers_stay() {
        let t = "Chorus:\nHoly holy holy is the Lord God Almighty who was and is and is to come\nShort line";
        let w = wrap_long_lines(t, 30);
        for l in w.lines() {
            assert!(l.chars().count() <= 30, "{l}");
        }
        assert!(w.starts_with("Chorus:\n") && w.ends_with("\nShort line"));
        assert_eq!(w.replace('\n', " "), t.replace('\n', " "));
        assert_eq!(wrap_long_lines("a verylongwordwithoutspacesatall here", 10), "a\nverylongwordwithoutspacesatall\nhere");
    }
}

impl Present {
    /// `dir` is the app's data folder (`~/.rhemaflow`): `present.json` and `songs.json` live there.
    pub fn new(dir: PathBuf) -> Present {
        let settings: PresentSettings = std::fs::read(dir.join("present.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let loaded_lib = load_library(&dir.join("songs.json"));
        crate::sound::kill_strays(); // a player left behind by a force-quit
        let mut shelf = MediaLibrary::open(&dir.join("media-library"));
        shelf.purge_trash(30); // v0.846.0: removed files are kept 30 days for "Undo remove"
        shelf.tidy_stored_names(); // v0.845.0: hash-named copies get their real names
        let stray_files = shelf.orphans(); // v0.845.0 health check (reported below, never deleted automatically)
        let (done_tx, done_rx) = channel::<JobDone>();
        let thumb_channel = channel();
        let mut p = Present {
            dir,
            order_outbox: Vec::new(),
            edit_focus: false,
            confirm_delete: None,
            scroll_slide_to: None,
            song_edit_base: String::new(),
            open: false,
            tab: Tab::Slides,
            slides: Vec::new(),
            slide_index: None,
            slides_source: String::new(),
            library: loaded_lib.library,
            song_query: String::new(),
            song_sel: None,
            song_edit: SongEdit::default(),
            ccli_logged: Default::default(),
            shown_songs: Vec::new(),
            newly_shown: Vec::new(),
            loaded: None,
            loaded_idx: None,
            loaded_id: None,
            doc_jobs: HashMap::new(),
            doc_root: PathBuf::new(),
            scrub_ms: None,
            scrub: Default::default(),
            paste_open: false,
            paste_title: String::new(),
            paste_text: String::new(),
            paste_notes: Vec::new(),
            locked: false,
            after_fired: false,
            shelf,
            shelf_view: None,
            shelf_filter: String::new(),
            pdf_texts: Default::default(),
            pdf_started: Default::default(),
            pdf_hit_cache: (String::new(), 0, Vec::new()),
            sunday_lines: Vec::new(),
            last_backup_unix: 0,
            new_shelf: String::new(),
            shelf_edit: None,
            dock_open: true,
            dock_auto_done: false,
            dock_cursor: None,
            dock_rows_seen: (0, 0),
            dock_sel: std::collections::HashSet::new(),
            dock_anchor: None,
            dock_rects: Vec::new(),
            dock_box: None,
            dock_del_armed: None,
            bg_adds: false,
            jobs: Vec::new(),
            job_seq: 0,
            job_tx: None,
            done_tx,
            done_rx,
            batch_added: 0,
            batch_known: 0,
            folder_edit: None,
            folder_rects: Vec::new(),
            target_folder: None,
            durations: HashMap::new(),
            thumb_busy: false,
            thumb_tx: thumb_channel.0,
            thumb_rx: thumb_channel.1,
            thumb_pending: std::collections::HashSet::new(),
            thumbs: HashMap::new(),
            bg: None,
            timer: Timer::new(TimerKind::CountDown { total_ms: 5 * 60_000 }, ""),
            timer_ui: TimerUi { choice: TimerChoice::CountDown, title: "Service starts in".into(), minutes: 5, until_text: "10:30".into() },
            clock_base: (0, Instant::now()),
            t0: Instant::now(),
            msg: None,
            note_for_app: None,
            path_input: String::new(),
            pick: None,
            settings,
        };
        p.doc_root = p.dir.join("doc-cache");
        docview::prune_cache(&p.doc_root, 24);
        if !p.settings.slides_text.trim().is_empty() {
            let t = p.settings.slides_text.clone();
            p.set_slides(&t, "Typed slides");
        }
        p.adopt_old_media_list();
        if let Some(w) = loaded_lib.warning {
            p.say(w);
        }
        p.sync_clock();
        p.apply_shelf_moves();
        if !stray_files.is_empty() {
            let mb = stray_files.iter().map(|(_, n)| *n).sum::<u64>() as f64 / 1_048_576.0;
            p.say(format!("Library check: {} unused file{} ({mb:.1} MB) in the media-library/files folder. Nothing was deleted.", stray_files.len(), if stray_files.len() == 1 { "" } else { "s" }));
        }
        p
    }

    // ---------------------------------------------------------------- time

    fn now_ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }
    /// Milliseconds since local midnight (synced from the Mac's clock when a timer starts).
    fn wall_ms(&self) -> u64 {
        (self.clock_base.0 + self.clock_base.1.elapsed().as_millis() as u64) % 86_400_000
    }
    fn sync_clock(&mut self) {
        let (st, _) = crate::naming::now_local();
        self.clock_base = ((st.h as u64 * 3600 + st.mi as u64 * 60 + st.s as u64) * 1000, Instant::now());
    }

    // ------------------------------------------------------------ messages

    pub fn say(&mut self, m: impl Into<String>) {
        let m = m.into();
        self.note_for_app = Some(m.clone());
        self.msg = Some((m, Instant::now()));
    }

    // --------------------------------------------------------- persistence

    pub fn save_settings(&mut self) {
        if let Ok(j) = serde_json::to_vec_pretty(&self.settings) {
            let _ = std::fs::create_dir_all(&self.dir);
            let _ = rf_service::store::atomic_write(&self.dir.join("present.json"), &j);
        }
    }
    pub fn save_library(&mut self) {
        if let Err(e) = save_library(&self.dir.join("songs.json"), &self.library) {
            self.say(format!("The song library could not be saved: {e}"));
        }
    }

    // -------------------------------------------------------------- slides

    pub fn set_slides(&mut self, text: &str, source: &str) {
        self.slides = slides_from_text(text);
        self.slide_index = None;
        self.slides_source = source.to_string();
    }

    fn slide_content(&self, i: usize) -> Option<StageContent> {
        let s = self.slides.get(i)?;
        Some(StageContent { kind: StageKind::Slide, label: s.label.clone(), text: s.text.clone(), translation: String::new(), chapter_only: false, updated: Some(Instant::now()) })
    }

    pub fn stage_slide(&mut self, i: usize) -> Option<StageContent> {
        let c = self.slide_content(i)?;
        self.slide_index = Some(i);
        Some(c)
    }

    /// S1.4: step through the slides. None (with a message) at either end.
    pub fn step_slide(&mut self, dir: i32) -> Option<StageContent> {
        if self.slides.is_empty() {
            self.say("There are no slides yet. Type some in Media & slides > Slides, or pick a song.");
            return None;
        }
        let next = match (self.slide_index, dir.signum()) {
            (None, 1) => 0,
            (None, _) => {
                self.say("First slide");
                return None;
            }
            (Some(i), 1) => {
                if i + 1 >= self.slides.len() {
                    self.say("Last slide");
                    return None;
                }
                i + 1
            }
            (Some(i), _) => {
                if i == 0 {
                    self.say("First slide");
                    return None;
                }
                i - 1
            }
        };
        self.stage_slide(next)
    }

    // --------------------------------------------------------------- songs

    pub fn use_song(&mut self, id: u64) {
        let Some(song) = self.library.get(id) else { return };
        let (slides, unknown) = song.slides(self.settings.lines_per_slide);
        let title = song.title.clone();
        self.slides = slides;
        self.slide_index = None;
        self.slides_source = format!("Song: {title}");
        if unknown.is_empty() {
            let n = self.slides.len();
            self.say(format!("\"{title}\" is ready: {n} slide{}. Press Next slide to start.", if n == 1 { "" } else { "s" }));
        } else {
            self.say(format!("\"{title}\": these names in the song order match no section and were skipped: {}", unknown.join(", ")));
        }
    }

    fn song_edit_key(e: &SongEdit) -> String {
        format!("{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}", e.title, e.author, e.ccli_song, e.copyright, e.lyrics, e.arrangement)
    }

    pub fn select_song(&mut self, id: u64) {
        if let Some(s) = self.library.get(id) {
            self.song_edit = SongEdit { title: s.title.clone(), author: s.author.clone(), ccli_song: s.ccli_song.clone(), copyright: s.copyright.clone(), lyrics: s.to_text(), arrangement: s.arrangement_text() };
            self.song_edit_base = Self::song_edit_key(&self.song_edit);
            self.song_sel = Some(id);
            self.confirm_delete = None;
        }
    }

    /// Cmd/Ctrl+S: save the song being edited (only when the Songs tab is showing). True if it was handled.
    pub fn save_song_shortcut(&mut self) -> bool {
        if self.open && self.tab == Tab::Songs && self.song_sel.is_some() {
            self.save_song_edit();
            return true;
        }
        false
    }

    /// Songs editor shortcuts (v0.849.0), only while a song is open for editing:
    /// Shift+Cmd+T Tidy, Shift+Cmd+L strip chords / web junk, Shift+Cmd+R find repeats, Shift+Cmd+P add the next Verse.
    /// Returns the id of the command that ran (for the menu flash).
    pub fn song_edit_shortcut(&mut self, ctx: &egui::Context) -> Option<&'static str> {
        if !(self.open && self.tab == Tab::Songs && self.song_sel.is_some()) {
            return None;
        }
        let m = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        let hit = |k: egui::Key| ctx.input_mut(|i| i.consume_key(m, k));
        if hit(egui::Key::T) {
            self.song_edit.lyrics = tidy_lyrics(&self.song_edit.lyrics);
            Some("song_tidy")
        } else if hit(egui::Key::L) {
            self.song_edit.lyrics = clean_pasted_lyrics(&self.song_edit.lyrics);
            Some("song_strip")
        } else if hit(egui::Key::R) {
            let (words, order) = detect_repeats(&self.song_edit.lyrics);
            if !words.is_empty() {
                self.song_edit.lyrics = words;
                self.song_edit.arrangement = order;
            }
            Some("song_repeats")
        } else if hit(egui::Key::W) {
            self.song_edit.lyrics = wrap_long_lines(&self.song_edit.lyrics, 42);
            Some("song_wrap")
        } else if hit(egui::Key::P) {
            let head = next_part_header(&self.song_edit.lyrics, "Verse");
            self.song_edit.lyrics = append_part(&self.song_edit.lyrics, &head);
            Some("song_add_verse")
        } else {
            None
        }
    }

    /// Copy a song under a new title ("... (copy)") and open it for editing.
    pub fn duplicate_song(&mut self, id: u64) {
        let Some(s) = self.library.get(id) else { return };
        let mut c = Song::from_text(&format!("{} (copy)", s.title), &s.to_text());
        c.author = s.author.clone();
        c.ccli_song = s.ccli_song.clone();
        c.copyright = s.copyright.clone();
        let arr = s.arrangement_text();
        c.set_arrangement_text(&arr);
        let nid = self.library.add(c);
        self.save_library();
        self.select_song(nid);
        self.edit_focus = true;
        self.say("Copied. Rename it and change the words, then Save song.");
    }

    pub fn new_song(&mut self) {
        let id = self.library.add(Song::from_text("New song", "Verse 1:\n"));
        self.save_library();
        self.select_song(id);
    }

    /// (title, CCLI number) of the songs shown this run, in order.
    pub fn songs_shown(&self) -> Vec<(String, String)> {
        self.shown_songs.iter().filter_map(|id| self.library.get(*id)).map(|s| (s.title.clone(), s.ccli_song.clone())).collect()
    }

    /// Path of the picture or video on the stage, if any.
    pub fn loaded_path(&self) -> Option<String> {
        self.loaded_idx.and_then(|i| self.settings.media.get(i).cloned())
    }

    /// Songs for pick-lists: (id, title, author, CCLI number).
    pub fn song_infos(&self) -> Vec<(u64, String, String, String)> {
        self.library.songs().iter().map(|s| (s.id, s.title.clone(), s.author.clone(), s.ccli_song.clone())).collect()
    }

    /// Add a song from pasted words (title guessed if blank). Returns its id.
    pub fn import_pasted_song(&mut self, text: &str, title: &str) -> Option<u64> {
        if text.trim().is_empty() {
            self.say("Paste the song words first.");
            return None;
        }
        let imp = crate::lyrics::import_pasted(text, title);
        if imp.song.sections.is_empty() {
            self.say("No words were found in that text.");
            return None;
        }
        let id = self.library.add(imp.song);
        self.save_library();
        self.select_song(id);
        self.paste_notes = imp.notes;
        Some(id)
    }

    /// The "Paste lyrics" form: used in the Songs tab and in the Tools window.
    pub fn paste_form(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Paste the words from anywhere (a lyric sheet, a chart, an email). Labels such as [Verse 1] or Chorus:, the CCLI number, the copyright line and chord lines are understood. A chorus written out again each time becomes one Chorus that the song order repeats.").small().weak());
        ui.horizontal(|ui| {
            ui.label("Title");
            ui.add(egui::TextEdit::singleline(&mut self.paste_title).desired_width(260.0).hint_text("optional: the first line is used if blank"));
        });
        ui.add(egui::TextEdit::multiline(&mut self.paste_text).desired_rows(9).desired_width(f32::INFINITY).hint_text("Paste lyrics here..."));
        ui.horizontal(|ui| {
            if ui.button("Paste from clipboard").on_hover_text("Put what you copied (Cmd+C) into the box above, then press Add song").clicked() {
                match clipboard_text() {
                    Some(t) => self.paste_text = t,
                    None => self.say("The clipboard is empty (or could not be read). Copy the words first, then try again."),
                }
            }
            if ui.button("Add song").clicked() {
                let (t, n) = (self.paste_text.clone(), self.paste_title.clone());
                if self.import_pasted_song(&t, &n).is_some() {
                    self.paste_text.clear();
                    self.paste_title.clear();
                    self.say("Song added. Check the title and the order, then press Use.");
                }
            }
            if ui.button("Clear").clicked() {
                self.paste_text.clear();
                self.paste_title.clear();
                self.paste_notes.clear();
            }
        });
        for n in &self.paste_notes {
            ui.label(egui::RichText::new(format!("\u{2022} {n}")).small().color(theme::MUTED));
        }
    }

    pub fn save_song_edit(&mut self) {
        let Some(id) = self.song_sel else { return };
        let e = &self.song_edit;
        let tidy: String = e.lyrics.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
        let mut s = Song::from_text(e.title.trim(), &tidy);
        s.author = e.author.trim().to_string();
        s.ccli_song = e.ccli_song.trim().to_string();
        s.copyright = e.copyright.trim().to_string();
        s.set_arrangement_text(&e.arrangement);
        if s.title.is_empty() {
            self.say("Please give the song a title.");
            return;
        }
        let mut was_slides = false;
        if let Some(old) = self.library.get_mut(id) {
            was_slides = self.slides_source == format!("Song: {}", old.title);
            s.id = id;
            *old = s;
        }
        self.save_library();
        self.select_song(id); // reload the tidied text so "unsaved" clears
        if was_slides {
            // the edited song is the one in the slides: rebuild them, keeping the place (the screen changes when you press a slide)
            let keep = self.slide_index;
            self.use_song(id);
            self.slide_index = keep.filter(|i| *i < self.slides.len());
            self.say("Song saved and its slides updated. Click the slide again to refresh the screen.");
        } else {
            self.say("Song saved.");
        }
    }

    pub fn delete_song(&mut self, id: u64) {
        if self.locked {
            return self.say(crate::lock::refusal(crate::lock::Guarded::EditSongs));
        }
        if self.library.remove(id) {
            if self.song_sel == Some(id) {
                self.song_sel = None;
                self.song_edit = SongEdit::default();
            }
            self.save_library();
        }
    }

    // --------------------------------------------------------------- files

    /// Route a file the operator gave us (picker, drag-and-drop, typed path) by what it is.
    pub fn route_path(&mut self, ctx: &egui::Context, path: &Path, purpose: PickFor) {
        let p = path.to_string_lossy().to_string();
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !path.exists() {
            self.say(format!("Cannot find \"{p}\"."));
            return;
        }
        if ext == "txt" || ext == "pptx" {
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => return self.say(format!("Cannot read \"{}\" ({e}).", file_name(&p))),
            };
            let text = if ext == "pptx" {
                match text_from_pptx(&bytes) {
                    Ok(t) => t,
                    Err(e) => return self.say(e),
                }
            } else {
                text_from_bytes(&bytes)
            };
            let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Imported".into());
            if purpose == PickFor::Song {
                let id = self.library.add(Song::from_text(&name, &text));
                self.save_library();
                self.select_song(id);
                self.tab = Tab::Songs;
                self.say(format!("Imported \"{name}\" as a song. Check the section names, then Save."));
            } else {
                self.settings.slides_text = text.clone();
                self.set_slides(&text, &format!("File: {}", file_name(&p)));
                self.save_settings();
                self.tab = Tab::Slides;
                let n = self.slides.len();
                self.say(format!("{n} slide{} read from {}.", if n == 1 { "" } else { "s" }, file_name(&p)));
            }
            return;
        }
        match classify(&p) {
            Classified::Ok(kind) => {
                if purpose == PickFor::Background {
                    self.set_background(ctx, &p);
                } else {
                    let word = match kind { MediaKind::Video => "video", MediaKind::Audio => "sound file", MediaKind::Image => "picture", MediaKind::Document => "document" };
                    let folder = self.target_folder.clone().or_else(|| self.shelf_view.clone()).unwrap_or_default();
                    if self.bg_adds || (kind == MediaKind::Video && wants_repair(&p)) {
                        self.queue_job(ctx, path, word, folder);
                    } else {
                        match self.shelf.add(path, self.shelf_policy(), &folder) {
                            Ok(added) => self.after_add(added, &file_name(&p), word, true),
                            Err(e) => self.say(e.to_string()),
                        }
                    }
                }
            }
            Classified::Unsupported(advice) => {
                if purpose != PickFor::Background && wants_repair(&p) {
                    let folder = self.target_folder.clone().or_else(|| self.shelf_view.clone()).unwrap_or_default();
                    let word = if repair_target(&p) == Some(rf_media::convert::Target::Audio) { "sound file" } else { "video" };
                    self.say(format!("\"{}\" needs converting first. RhemaFlow is doing that in the background (a few minutes for a long video).", file_name(&p)));
                    self.queue_job(ctx, path, word, folder);
                } else if repair_target(&p).is_some() {
                    self.say(format!("{advice} {}", rf_media::convert::INSTALL_HINT));
                } else {
                    self.say(advice);
                }
            }
            Classified::Unknown => self.say(format!("\"{}\" is not a picture, video, sound, PDF, HTML, text or PowerPoint file.", file_name(&p))),
        }
    }

    /// The box where a path can be typed or pasted (works everywhere, also when there is no file chooser).
    pub fn add_typed_path(&mut self, ctx: &egui::Context, purpose: PickFor) {
        let p = clean_path(&self.path_input);
        if p.is_empty() {
            return;
        }
        self.route_path(ctx, Path::new(&p), purpose);
        self.path_input.clear();
    }

    /// Bookkeeping after a file joined (or was already on) the shelf.
    fn after_add(&mut self, added: Added, name: &str, word: &str, say_it: bool) {
        let (id, fresh) = match added {
            Added::New(i) => (i, true),
            Added::AlreadyThere(i) => (i, false),
        };
        let (stored, mode) = self.shelf.get(id).map(|e| (e.path.clone(), e.mode)).unwrap_or((String::new(), ShelfMode::Link));
        if !stored.is_empty() && !self.settings.media.contains(&stored) {
            self.settings.media.push(stored);
            self.save_settings();
        }
        let _ = self.shelf.save();
        if fresh {
            self.batch_added += 1;
        } else {
            self.batch_known += 1;
        }
        if say_it {
            let how = if mode == ShelfMode::Copy { "copied onto the shelf" } else { "linked (the original stays where it is)" };
            if fresh {
                let extra = self.shelf.get(id).and_then(|e| rf_service::library::size_warning(e.size)).map(|w| format!(" {w}")).unwrap_or_default();
                self.say(format!("Added {word} \"{name}\": {how}.{extra}"));
            } else {
                self.say(format!("\"{name}\" is already on the shelf."));
            }
            self.batch_added = 0;
            self.batch_known = 0;
        }
    }

    /// Hand a file to the helper thread. The window keeps running; a progress bar shows in the bar.
    fn queue_job(&mut self, ctx: &egui::Context, path: &Path, word: &'static str, folder: String) {
        let policy = self.shelf_policy();
        let total = planned_work(path, policy).max(1);
        self.job_seq += 1;
        let id = self.job_seq;
        let done = Arc::new(AtomicU64::new(0));
        self.jobs.push(JobInfo { id, folder, word, name: file_name(&path.to_string_lossy()), total, done: done.clone() });
        if self.job_tx.is_none() {
            let (tx, rx) = channel::<JobReq>();
            let done_tx = self.done_tx.clone();
            std::thread::spawn(move || {
                for r in rx {
                    let res = add_with_repair(&r.src, r.policy, &r.dir, &r.done, r.total);
                    let _ = done_tx.send(JobDone { id: r.id, res });
                    r.ctx.request_repaint();
                }
            });
            self.job_tx = Some(tx);
        }
        if let Some(tx) = &self.job_tx {
            let _ = tx.send(JobReq { id, src: path.to_path_buf(), policy, dir: self.shelf.dir().to_path_buf(), done, total, ctx: ctx.clone() });
        }
    }

    /// Collect finished background adds (cheap; called every frame).
    pub fn poll_jobs(&mut self) {
        while let Ok(d) = self.done_rx.try_recv() {
            let Some(i) = self.jobs.iter().position(|j| j.id == d.id) else { continue };
            let job = self.jobs.remove(i);
            match d.res {
                Ok(prep) => {
                    let added = self.shelf.commit(prep, &job.folder);
                    self.after_add(added, &job.name, job.word, false);
                }
                Err(e) => self.say(e.to_string()),
            }
            if self.jobs.is_empty() && (self.batch_added + self.batch_known) > 0 {
                let (n, k) = (self.batch_added, self.batch_known);
                self.say(match (n, k) {
                    (1, 0) => format!("Added {} \"{}\".", job.word, job.name),
                    (n, 0) => format!("Added {n} files."),
                    (0, k) => format!("{k} file(s) were already on the shelf."),
                    (n, k) => format!("Added {n} file(s); {k} were already there."),
                });
                self.batch_added = 0;
                self.batch_known = 0;
            }
        }
    }

    pub fn jobs_pending(&self) -> usize {
        self.jobs.len()
    }

    /// (bytes done, bytes planned) across everything still being added.
    pub fn jobs_progress(&self) -> (u64, u64) {
        self.jobs.iter().fold((0, 0), |(d, t), j| (d + j.done.load(Ordering::Relaxed).min(j.total), t + j.total))
    }

    /// Which folder row is under this point? (Some("") = "All media".) For files dropped from Finder.
    pub fn folder_at(&self, pos: egui::Pos2) -> Option<String> {
        self.folder_rects.iter().find(|(_, r)| r.contains(pos)).map(|(n, _)| n.clone())
    }

    /// A whole folder dropped from Finder becomes a folder here, with its pictures and videos inside
    /// (sub-folders too, named "Parent / Child"). At most 500 files, so a wrong drop cannot run away.
    pub fn add_directory(&mut self, ctx: &egui::Context, dir: &Path) {
        let base = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Folder".into());
        let before = self.shelf.entries.len();
        let mut budget = 500usize;
        let mut songs_in = 0usize;
        let mut stack = vec![(dir.to_path_buf(), base.clone(), 0u8)];
        while let Some((d, name, depth)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            let mut items: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
            items.sort();
            let Ok(folder) = self.shelf.create_folder(&name) else { continue };
            self.target_folder = Some(folder);
            for p in items {
                if p.is_dir() {
                    if depth < 2 {
                        let child = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        stack.push((p, format!("{name} / {child}"), depth + 1));
                    }
                } else if budget > 0 && matches!(p.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref(), Some("txt") | Some("pptx")) {
                    // v0.850.0: a whole service folder: song words (.txt / .pptx) become songs
                    budget -= 1;
                    songs_in += 1;
                    self.route_path(ctx, &p, PickFor::Song);
                } else if budget > 0 && matches!(rf_service::media::classify(&p.to_string_lossy()), rf_service::media::Classified::Ok(_)) {
                    budget -= 1;
                    self.route_path(ctx, &p, PickFor::Media);
                }
            }
        }
        self.target_folder = None;
        let added = self.shelf.entries.len() - before;
        let _ = self.shelf.save();
        self.shelf_view = Some(base.clone());
        let songs_note = if songs_in > 0 { format!(" and {songs_in} song file(s) imported as songs") } else { String::new() };
        self.say(format!("Added {added} file(s){songs_note} from \"{base}\"."));
    }

    fn shelf_policy(&self) -> ShelfPolicy {
        match self.settings.shelf_policy.as_str() {
            "copy" => ShelfPolicy::AlwaysCopy,
            "link" => ShelfPolicy::AlwaysLink,
            _ => ShelfPolicy::AlwaysCopy, // "copy", and old settings that said "auto"
        }
    }

    /// Files added before the shelf existed stay usable: they join it as links (nothing is copied behind your back).
    fn adopt_old_media_list(&mut self) {
        let old: Vec<String> = self.settings.media.clone();
        let mut changed = false;
        for p in old {
            if !self.shelf.entries.iter().any(|e| e.path == p) && Path::new(&p).exists() && self.shelf.add(Path::new(&p), ShelfPolicy::AlwaysLink, "").is_ok() {
                changed = true;
            }
        }
        if changed {
            let _ = self.shelf.save();
        }
    }

    /// Stored files that were renamed (by the tidy-up at start, or by renaming an item) change path: fix the lists that
    /// remember the old path (the stage's file list and the "when it ends" steps). The service order finds them through
    /// `resolve_moved` in `stage_path`.
    pub fn apply_shelf_moves(&mut self) {
        let moves = self.shelf.take_moves();
        if moves.is_empty() {
            return;
        }
        for (old, new) in &moves {
            for m in self.settings.media.iter_mut() {
                if m == old {
                    *m = new.clone();
                }
            }
            if let Some(v) = self.settings.after_end.remove(old) {
                self.settings.after_end.insert(new.clone(), v);
            }
            let from = format!("show:{old}");
            for v in self.settings.after_end.values_mut() {
                if *v == from {
                    *v = format!("show:{new}");
                }
            }
        }
        self.save_settings();
    }

    /// "Last shown 3 days ago" / "Not shown yet" for the right-click menu (v0.850.0).
    pub fn last_shown_text(&self, id: u64) -> String {
        match self.shelf.shown_days_ago(id) {
            None => "Not shown yet".to_string(),
            Some(0) => "Last shown today".to_string(),
            Some(1) => "Last shown yesterday".to_string(),
            Some(d) if d < 60 => format!("Last shown {d} days ago"),
            Some(d) => format!("Last shown {} months ago", d / 30),
        }
    }

    /// Start reading the words of every PDF on the shelf (once each, in the background), and work out which pages
    /// match the MEDIA search box. Returns (hits, still reading). v0.850.0.
    pub fn pdf_search(&mut self, ctx: &egui::Context) -> (Vec<(u64, String, usize, String)>, bool) {
        let q = self.shelf_filter.trim().to_string();
        let pdfs: Vec<(u64, String, String)> = self.shelf.entries.iter().filter(|e| e.path.to_lowercase().ends_with(".pdf") && std::path::Path::new(&e.path).exists()).map(|e| (e.id, e.name.clone(), e.path.clone())).collect();
        for (_, _, path) in &pdfs {
            if self.pdf_started.insert(path.clone()) {
                let (slot, path, c2) = (self.pdf_texts.clone(), path.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let r = rf_media::docsearch::pdf_page_texts(std::path::Path::new(&path)).unwrap_or_default();
                    if let Ok(mut g) = slot.lock() {
                        g.insert(path, r);
                    }
                    c2.request_repaint();
                });
            }
        }
        let have = self.pdf_texts.lock().map(|g| g.len()).unwrap_or(0);
        let reading = pdfs.iter().any(|(_, _, p)| self.pdf_texts.lock().map_or(true, |g| !g.contains_key(p)));
        if reading {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
        if self.pdf_hit_cache.0 != q || self.pdf_hit_cache.1 != have {
            let mut hits = Vec::new();
            if let Ok(g) = self.pdf_texts.lock() {
                for (id, name, path) in &pdfs {
                    if let Some(pages) = g.get(path) {
                        for h in rf_media::docsearch::search_pages(pages, &q, 3) {
                            hits.push((*id, name.clone(), h.page, h.snippet));
                        }
                    }
                }
            }
            self.pdf_hit_cache = (q, have, hits);
        }
        (self.pdf_hit_cache.2.clone(), reading)
    }

    /// The "Sunday check": things worth fixing before the service, one line each (v0.850.0).
    pub fn sunday_check(&self, now_unix: i64) -> Vec<String> {
        let mut v = Vec::new();
        let missing = self.shelf.missing_ids().len();
        if missing > 0 {
            v.push(format!("{missing} file{} missing (moved or unplugged): right-click > Re-link, or Options > Re-link all", if missing == 1 { " is" } else { "s are" }));
        }
        let big = self.shelf.entries.iter().filter(|e| rf_service::library::size_warning(e.size).is_some()).count();
        if big > 0 {
            v.push(format!("{big} very big file{}: play {} through once on the big screen", if big == 1 { "" } else { "s" }, if big == 1 { "it" } else { "them" }));
        }
        let bad: Vec<String> = self.library.search("").iter().filter(|s| !s.slides(self.settings.lines_per_slide).1.is_empty()).map(|s| s.title.clone()).collect();
        if !bad.is_empty() {
            v.push(format!("{} song{} name a part in \"Order sung\" that does not exist: {}", bad.len(), if bad.len() == 1 { "" } else { "s" }, bad.iter().take(3).cloned().collect::<Vec<_>>().join(", ")));
        }
        let dup = self.shelf.duplicate_groups().len();
        if dup > 0 {
            v.push(format!("{dup} file{} listed more than once (Options > Find duplicates)", if dup == 1 { "" } else { "s" }));
        }
        let strays = self.shelf.orphans().len();
        if strays > 0 {
            v.push(format!("{strays} unused file{} in the media-library/files folder", if strays == 1 { "" } else { "s" }));
        }
        let old = self.shelf.unused_ids_at(90, now_unix.max(0) as u64).len();
        if old > 0 {
            v.push(format!("{old} file{} not shown for over 3 months (could be archived)", if old == 1 { "" } else { "s" }));
        }
        if self.last_backup_unix <= 0 {
            v.push("no backup yet: Tools > Backup".to_string());
        } else if now_unix - self.last_backup_unix > 7 * 86_400 {
            v.push(format!("last backup was {} days ago: Tools > Backup", (now_unix - self.last_backup_unix) / 86_400));
        }
        if v.is_empty() {
            v.push("All clear: nothing to fix.".to_string());
        }
        v
    }

    /// Options > Undo last remove.
    pub fn undo_remove_shelf(&mut self) {
        match self.shelf.undo_remove() {
            Some(id) => {
                let _ = self.shelf.save();
                self.apply_shelf_moves();
                if let Some((name, path)) = self.shelf.get(id).map(|e| (e.name.clone(), e.path.clone())) {
                    if !self.settings.media.contains(&path) {
                        self.settings.media.push(path);
                        self.save_settings();
                    }
                    self.say(format!("Brought back \"{name}\"."));
                }
            }
            None => self.say("Nothing to undo: no file has been removed since RhemaFlow started."),
        }
    }

    /// Options > Find duplicates: the same picture or video listed more than once.
    pub fn report_duplicates(&mut self) {
        let groups = self.shelf.duplicate_groups();
        if groups.is_empty() {
            return self.say("No duplicates: every file is listed once.");
        }
        let names: Vec<String> = groups.iter().take(4).map(|g| g.iter().filter_map(|id| self.shelf.get(*id).map(|e| e.name.clone())).collect::<Vec<_>>().join(" = ")).collect();
        let more = if groups.len() > 4 { format!(" ... and {} more", groups.len() - 4) } else { String::new() };
        self.say(format!("{} file{} listed more than once (same content): {}{}. Remove the extras with right-click > Remove.", groups.len(), if groups.len() == 1 { "" } else { "s" }, names.join("; "), more));
    }

    /// Options > Re-link all missing: the folder is the one typed in the path box.
    pub fn relink_all_from_path_box(&mut self) {
        let np = clean_path(&self.path_input);
        if np.is_empty() || !Path::new(&np).is_dir() {
            return self.say("Type or paste the FOLDER where the files are now (in the path box), then press Re-link all again.");
        }
        let (ok, left) = self.shelf.relink_all(Path::new(&np));
        let _ = self.shelf.save();
        self.thumbs.clear();
        self.say(format!("Re-linked {ok} file{}; {left} still missing.", if ok == 1 { "" } else { "s" }));
    }

    /// Show a shelf item: this is what clicking its thumbnail does.
    pub fn stage_entry(&mut self, ctx: &egui::Context, id: u64) -> Option<StageContent> {
        let Some(path) = self.shelf.path_to_show(id) else {
            let n = self.shelf.get(id).map(|e| e.name.clone()).unwrap_or_default();
            self.say(format!("\"{n}\" is missing (moved or deleted?). Use Re-link, or remove it."));
            return None;
        };
        self.shelf.touch(id); // v0.846.0: feeds "Recently used"
        let p = path.to_string_lossy().to_string();
        let idx = match self.settings.media.iter().position(|m| *m == p) {
            Some(i) => i,
            None => {
                self.settings.media.push(p);
                self.save_settings();
                self.settings.media.len() - 1
            }
        };
        let shown = self.stage_media(ctx, idx);
        if shown.is_some() {
            self.loaded_id = Some(id);
        }
        shown
    }

    /// Show a file by its path (a service-order item remembers the path, not the shelf number).
    pub fn stage_path(&mut self, ctx: &egui::Context, path: &str) -> Option<StageContent> {
        let moved = self.shelf.resolve_any(path); // v0.846.0/0.848.0: a stored file renamed or moved since the list was saved
        let path = moved.as_str();
        if let Some(id) = self.shelf.entries.iter().find(|e| e.path == path).map(|e| e.id) {
            return self.stage_entry(ctx, id);
        }
        let idx = match self.settings.media.iter().position(|m| m == path) {
            Some(i) => i,
            None => {
                self.settings.media.push(path.to_string());
                self.save_settings();
                self.settings.media.len() - 1
            }
        };
        self.stage_media(ctx, idx)
    }

    /// (title, part names in the order sung) for a song, so the service order knows how many slides it has.
    pub fn song_parts(&self, id: u64) -> Option<(String, Vec<String>)> {
        let song = self.library.get(id)?;
        let (slides, _) = song.slides(self.settings.lines_per_slide);
        Some((song.title.clone(), slides.iter().map(|s| s.label.clone()).collect()))
    }

    /// Show slide `i` of a song (builds the song's slides first).
    pub fn stage_song_slide(&mut self, id: u64, i: usize) -> Option<StageContent> {
        self.use_song(id);
        let mut content = self.stage_slide(i)?;
        if !self.shown_songs.contains(&id) {
            self.shown_songs.push(id);
            self.newly_shown.push(id);
        }
        if let Some(song) = self.library.get(id) {
            // the CCLI number and copyright line under the words, and one line in the usage report per song per run
            let foot = crate::ccli::footer("", &song.author, &song.ccli_song, &song.copyright, &self.settings.ccli_licence);
            if !foot.is_empty() {
                content.label = if content.label.trim().is_empty() { foot } else { format!("{} \u{b7} {}", content.label, foot) };
            }
            if !song.ccli_song.trim().is_empty() && self.ccli_logged.insert(id) {
                let (now, _) = crate::naming::now_local();
                let date = format!("{:04}-{:02}-{:02}", now.y, now.mo, now.d);
                let line = crate::ccli::report_line(&date, &now.time_hms(), &song.title, &song.author, &song.ccli_song, &self.settings.ccli_licence);
                crate::ccli::append_report(&self.dir, &line);
            }
        }
        Some(content)
    }

    /// Start a countdown of `seconds` and put it on the stage (a service-order timer item).
    pub fn stage_countdown(&mut self, title: &str, seconds: u32) -> Option<StageContent> {
        self.timer_ui.choice = TimerChoice::CountDown;
        self.timer_ui.title = title.to_string();
        self.timer_ui.minutes = (seconds + 59) / 60;
        self.timer_start();
        self.stage_timer()
    }

    pub fn remove_from_shelf(&mut self, id: u64) {
        if self.locked {
            return self.say(crate::lock::refusal(crate::lock::Guarded::EditMedia));
        }
        if let Some(path) = self.shelf.get(id).map(|e| e.path.clone()) {
            if let Some(i) = self.settings.media.iter().position(|m| *m == path) {
                self.remove_media(i);
            }
        }
        self.thumbs.remove(&id);
        let _ = self.shelf.remove(id);
        let _ = self.shelf.save();
    }

    /// Small preview for the grid, made once per item. Pictures are made at once; a video's preview is made on a
    /// helper thread (any file size) and appears a moment later.
    fn thumb(&mut self, ctx: &egui::Context, id: u64) -> Option<egui::TextureHandle> {
        while let Ok((tid, res)) = self.thumb_rx.try_recv() {
            self.thumb_pending.remove(&tid);
            let t = res.map(|(img, ms)| {
                self.durations.insert(tid, ms);
                Self::texture(ctx, &format!("shelf-thumb-{tid}"), &img)
            });
            self.thumbs.insert(tid, t);
        }
        if !self.thumbs.contains_key(&id) {
            let path = self.shelf.path_to_show(id);
            match (path, self.shelf.kind(id)) {
                (Some(p), Some(MediaKind::Image)) => {
                    let img = load_image(&p, 256).ok();
                    let t = img.map(|img| Self::texture(ctx, &format!("shelf-thumb-{id}"), &img));
                    self.thumbs.insert(id, t);
                }
                (Some(p), Some(MediaKind::Video)) => {
                    if !self.thumb_pending.contains(&id) && self.thumb_pending.len() < 2 {
                        self.thumb_pending.insert(id);
                        let tx = self.thumb_tx.clone();
                        let c2 = ctx.clone();
                        std::thread::spawn(move || {
                            let r = rf_media::thumb::video_thumbnail(&p, 256);
                            let _ = tx.send((id, r));
                            c2.request_repaint();
                        });
                    }
                    ctx.request_repaint_after(Duration::from_millis(200));
                    return None;
                }
                (Some(p), Some(MediaKind::Document)) => {
                    let ps = p.to_string_lossy().to_string();
                    let job = self.doc_job(&ps);
                    let dir = docview::cache_dir(&self.doc_root, &ps);
                    match docview::first_page_thumb(&dir, 256) {
                        Some(img) => {
                            let t = Self::texture(ctx, &format!("shelf-thumb-{id}"), &img);
                            self.thumbs.insert(id, Some(t));
                        }
                        None if job.is_done() => {
                            self.thumbs.insert(id, None);
                        }
                        None => {
                            ctx.request_repaint_after(Duration::from_millis(300));
                            return None;
                        }
                    }
                }
                _ => {
                    self.thumbs.insert(id, None);
                }
            }
        }
        self.thumbs.get(&id).cloned().flatten()
    }

    pub fn remove_media(&mut self, idx: usize) {
        if idx < self.settings.media.len() {
            self.settings.media.remove(idx);
            if self.loaded_idx == Some(idx) {
                self.loaded = None;
                self.loaded_idx = None;
                self.loaded_id = None;
            }
            self.save_settings();
        }
    }

    /// Ask the Mac for files (runs in the background so the window never freezes).
    pub fn spawn_pick(&mut self, purpose: PickFor) {
        #[cfg(target_os = "macos")]
        {
            let (tx, rx) = channel();
            self.pick = Some((purpose, rx));
            std::thread::spawn(move || {
                let script = [
                    "set theFiles to choose file with prompt \"Choose files for RhemaFlow\" with multiple selections allowed",
                    "set out to \"\"",
                    "repeat with f in theFiles",
                    "set out to out & POSIX path of f & linefeed",
                    "end repeat",
                    "return out",
                ];
                let mut cmd = std::process::Command::new("osascript");
                for l in script {
                    cmd.arg("-e").arg(l);
                }
                let files = match cmd.output() {
                    Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).lines().map(|l| PathBuf::from(l.trim())).filter(|p| !p.as_os_str().is_empty()).collect(),
                    _ => Vec::new(),
                };
                let _ = tx.send(files);
            });
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (purpose, channel::<Vec<PathBuf>>);
            self.say("The file chooser is only built for the Mac. Drag files onto the window, or type or paste the path in the box.");
        }
    }

    fn poll_pick(&mut self, ctx: &egui::Context) {
        let done = match &self.pick {
            Some((_, rx)) => rx.try_recv().ok(),
            None => None,
        };
        if let Some(files) = done {
            let purpose = self.pick.take().map(|p| p.0).unwrap_or(PickFor::Media);
            for f in files {
                self.route_path(ctx, &f, purpose);
            }
        }
    }

    // --------------------------------------------------------------- media

    fn texture(ctx: &egui::Context, name: &str, img: &Rgba) -> egui::TextureHandle {
        let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.pixels);
        ctx.load_texture(name, ci, egui::TextureOptions::LINEAR)
    }

    /// Put picture/video number `idx` of the media list on the stage. Errors become messages and
    /// leave the stage untouched (S0.5).
    pub fn stage_media(&mut self, ctx: &egui::Context, idx: usize) -> Option<StageContent> {
        let Some(path) = self.settings.media.get(idx).cloned() else { return None };
        if !Path::new(&path).exists() {
            self.say(format!("\"{}\" is missing (moved or deleted?).", file_name(&path)));
            return None;
        }
        let loaded = match classify(&path) {
            Classified::Ok(MediaKind::Image) => match load_image(Path::new(&path), MAX_IMAGE_SIDE) {
                Ok(img) => Loaded::Image { size: [img.width as f32, img.height as f32], tex: Self::texture(ctx, &path, &img) },
                Err(e) => {
                    self.say(e);
                    return None;
                }
            },
            Classified::Ok(MediaKind::Video) => match Player::open(Path::new(&path), self.settings.loop_video) {
                Ok(player) => {
                    let info = player.info().clone();
                    player.play();
                    let mut sound = Sound::none();
                    if !self.settings.mute {
                        sound.start(&path, 1.0);
                    }
                    Loaded::Video { player, tex: None, serial: 0, size: [info.width as f32, info.height as f32], sound, path: path.clone(), last_pos: 0, error_told: false }
                }
                Err(e) => {
                    self.say(e);
                    return None;
                }
            },
            Classified::Ok(MediaKind::Audio) => {
                let mut sound = Sound::none();
                if !sound.start(&path, 1.0) {
                    self.say(if cfg!(target_os = "macos") { format!("The sound of \"{}\" could not be started. Is the Mac's sound player (afplay) available?", file_name(&path)) } else { "Sound files play on a Mac (this computer has no sound player RhemaFlow can use).".to_string() });
                    return None;
                }
                Loaded::Audio { sound, path: path.clone(), paused: false }
            }
            Classified::Ok(MediaKind::Document) => {
                let job = self.doc_job(&path);
                let name = self.shelf.entries.iter().find(|e| e.path == path).map(|e| e.name.clone()).unwrap_or_else(|| file_name(&path));
                let page = self.settings.resume.get(&path).map(|p| *p as usize).unwrap_or(0);
                let mut d = DocState::open(&path, &name, job, page, self.settings.doc_fit_width);
                d.zoom = zoom_to_remember(self.settings.doc_zoom.get(&path).copied()).unwrap_or(1.0);
                d.guide_lines = self.settings.doc_guide_lines.max(1);
                let ci = self.settings.doc_guide_color as usize % crate::stage::DOC_GUIDE_COLORS.len();
                d.guide_color = crate::stage::DOC_GUIDE_COLORS[ci].1;
                Loaded::Document(d)
            }
            Classified::Unsupported(m) => {
                self.say(m);
                return None;
            }
            Classified::Unknown => {
                self.say("That is not a picture, video, sound or document file.");
                return None;
            }
        };
        self.bookmark_current(); // what was on the stage remembers where it was
        self.loaded = Some(loaded);
        self.loaded_id = None;
        self.after_fired = false;
        self.loaded_idx = Some(idx);
        // coming back to a video or sound carries on from where it was left (v0.840.0)
        if matches!(self.loaded, Some(Loaded::Video { .. }) | Some(Loaded::Audio { .. })) {
            if let Some(ms) = self.settings.resume.get(&path).copied() {
                if ms >= RESUME_MIN_MS && self.media_seek(ms) {
                    self.say(format!("Carrying on from {}. Restart plays it from the beginning.", fmt_clock(ms)));
                }
            }
        }
        if let Some(Loaded::Document(d)) = &self.loaded {
            if d.index > 0 {
                let m = format!("Back on page {} of \"{}\".", d.index + 1, d.name);
                self.say(m);
            }
        }
        let label = self.shelf.entries.iter().find(|e| e.path == path).map(|e| e.name.clone()).unwrap_or_else(|| file_name(&path));
        Some(StageContent { kind: StageKind::Media, label, text: String::new(), translation: String::new(), chapter_only: false, updated: Some(Instant::now()) })
    }

    /// Remember where the video, sound or document on the stage is, so coming back carries on from there.
    /// A video that has ended, or is within a few seconds of either end, is forgotten (it starts from the top).
    fn bookmark_current(&mut self) {
        let mut entry: Option<(String, Option<u64>)> = None;
        let mut doc_zoom: Option<(String, Option<f32>)> = None;
        match &mut self.loaded {
            Some(Loaded::Video { player, path, .. }) => {
                let st = player.state();
                let keep = !st.ended && st.position_ms >= RESUME_MIN_MS && st.position_ms + RESUME_MIN_MS < st.duration_ms;
                entry = Some((path.clone(), if keep { Some(st.position_ms) } else { None }));
            }
            Some(Loaded::Audio { sound, path, .. }) => {
                let (pos, dur) = (sound.position_ms(), sound.duration_ms());
                let keep = dur > 0 && pos >= RESUME_MIN_MS && pos + RESUME_MIN_MS < dur;
                entry = Some((path.clone(), if keep { Some(pos) } else { None }));
            }
            Some(Loaded::Document(d)) => {
                entry = Some((d.path.clone(), if d.index > 0 { Some(d.index as u64) } else { None }));
                doc_zoom = Some((d.path.clone(), zoom_to_remember(Some(d.zoom))));
            }
            _ => {}
        }
        if let Some((path, z)) = doc_zoom {
            let changed = match z {
                Some(z) => self.settings.doc_zoom.insert(path, z) != Some(z),
                None => self.settings.doc_zoom.remove(&path).is_some(),
            };
            if changed {
                self.save_settings();
            }
        }
        if let Some((path, v)) = entry {
            let changed = match v {
                Some(ms) => self.settings.resume.insert(path, ms) != Some(ms),
                None => self.settings.resume.remove(&path).is_some(),
            };
            if changed {
                self.save_settings();
            }
        }
    }

    /// The page-making job for a document: started the first time it is needed, then shared.
    fn doc_job(&mut self, path: &str) -> rf_media::document::DocJob {
        if let Some(j) = self.doc_jobs.get(path) {
            return j.clone();
        }
        let dir = docview::cache_dir(&self.doc_root, path);
        let j = rf_media::document::start_job(Path::new(path), &dir);
        self.doc_jobs.insert(path.to_string(), j.clone());
        j
    }

    /// True while a PDF or HTML page is on the stage.
    pub fn has_document(&self) -> bool {
        matches!(self.loaded, Some(Loaded::Document(_)))
    }

    /// Page, reading line or zoom for the document on the stage. False if there is none.
    pub fn doc_act(&mut self, a: DocAction) -> bool {
        match &mut self.loaded {
            Some(Loaded::Document(d)) => {
                d.act(a);
                true
            }
            _ => false,
        }
    }

    /// Take the document off the stage (it remembers its page).
    pub fn doc_close(&mut self) -> Option<StageContent> {
        if !self.has_document() {
            return None;
        }
        self.bookmark_current();
        self.loaded = None;
        self.loaded_idx = None;
        self.loaded_id = None;
        Some(StageContent { updated: Some(Instant::now()), ..Default::default() })
    }

    /// The bar for a PDF or HTML page: page back / forward, go to page, zoom, fit, the reading line.
    fn doc_ui(&mut self, ui: &mut egui::Ui, request: &mut Option<StageContent>) {
        let mut close = false;
        let mut fit_changed: Option<bool> = None;
        let mut guide_lines_set: Option<usize> = None;
        let mut guide_color_set: Option<u8> = None;
        let Some(Loaded::Document(d)) = &mut self.loaded else { return };
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("DOCUMENT").small().strong().color(theme::MUTED));
            ui.label(egui::RichText::new(d.name.clone()).small().color(theme::TEXT));
            if d.page_count() == 0 {
                ui.label(egui::RichText::new("preparing the pages\u{2026}").small().color(theme::GOLD));
                if ui.small_button("Close").clicked() {
                    close = true;
                }
                return;
            }
            if ui.small_button("\u{25c0} Page").on_hover_text("Previous page (\u{2190} or PageUp)").clicked() {
                d.act(DocAction::PrevPage);
            }
            let more = if d.preparing() { "+" } else { "" };
            ui.label(egui::RichText::new(format!("{} / {}{}", d.index + 1, d.page_count(), more)).monospace());
            if ui.small_button("Page \u{25b6}").on_hover_text("Next page (\u{2192} or PageDown)").clicked() {
                d.act(DocAction::NextPage);
            }
            let mut pg = d.index + 1;
            let r = ui.add(egui::DragValue::new(&mut pg).clamp_range(1..=d.page_count().max(1)).speed(0.2).prefix("go to "));
            if r.changed() {
                d.goto(pg.saturating_sub(1));
            }
            ui.separator();
            if ui.small_button("A\u{2212}").on_hover_text("Zoom out (the - key on the Mac menu: View > Smaller text)").clicked() {
                d.act(DocAction::ZoomOut);
            }
            ui.label(egui::RichText::new(format!("{:.0}%", d.zoom * 100.0)).small().monospace());
            if ui.small_button("A+").on_hover_text("Zoom in").clicked() {
                d.act(DocAction::ZoomIn);
            }
            if ui.selectable_label(!d.fit_width, "Fit page").on_hover_text("The whole page on the screen").clicked() {
                d.fit_width = false;
                d.zoom = 1.0;
                fit_changed = Some(false);
            }
            if ui.selectable_label(d.fit_width, "Fit width").on_hover_text("The page fills the screen's width; the reading line scrolls it").clicked() {
                d.fit_width = true;
                d.zoom = 1.0;
                fit_changed = Some(true);
            }
            ui.separator();
            let mut g = d.guide_on;
            if ui.checkbox(&mut g, "Reading line").on_hover_text("Highlights one line and dims the rest, so people can follow (key K)").changed() {
                d.guide_on = g;
            }
            if d.guide_on {
                if ui.small_button("\u{25b2} Line").on_hover_text("Line up (\u{2191})").clicked() {
                    d.act(DocAction::LineUp);
                }
                if ui.small_button("\u{25bc} Line").on_hover_text("Line down (\u{2193})").clicked() {
                    d.act(DocAction::LineDown);
                }
                ui.separator();
                ui.label(egui::RichText::new("Lines at a time").small().color(theme::MUTED));
                for n in [1usize, 2, 3] {
                    if ui.selectable_label(d.guide_lines == n, format!("{n}")).on_hover_text(format!("Highlight {n} line{} at a time", if n == 1 { "" } else { "s" })).clicked() {
                        d.guide_lines = n;
                        guide_lines_set = Some(n);
                    }
                }
                ui.separator();
                ui.label(egui::RichText::new("Colour").small().color(theme::MUTED));
                for (i, (name, rgb)) in crate::stage::DOC_GUIDE_COLORS.iter().enumerate() {
                    let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
                    let picked = d.guide_color == *rgb;
                    let btn = egui::Button::new(egui::RichText::new(" ").monospace()).fill(egui::Color32::from_rgb(r, g, b)).min_size(egui::vec2(20.0, 16.0));
                    if ui.add(btn).on_hover_text(if picked { format!("{name} (picked)") } else { name.to_string() }).clicked() {
                        d.guide_color = *rgb;
                        guide_color_set = Some(i as u8);
                    }
                }
            } else if d.zoom > 1.01 || d.fit_width {
                if ui.small_button("\u{25b2} Up").clicked() {
                    d.scroll_by(-0.12);
                }
                if ui.small_button("\u{25bc} Down").clicked() {
                    d.scroll_by(0.12);
                }
            }
            if ui.small_button("Close").on_hover_text("Take the document off the screen (it remembers the page)").clicked() {
                close = true;
            }
        });
        if let Some(n) = Some(d.note()).filter(|n| !n.is_empty()) {
            ui.label(egui::RichText::new(n).small().color(theme::GOLD));
        }
        // search inside the document and a strip of page pictures (v0.843.0)
        if d.page_count() > 0 {
            let ctx = ui.ctx().clone();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Find").small().color(theme::MUTED));
                ui.add(egui::TextEdit::singleline(&mut d.search).desired_width(160.0).hint_text("words in this PDF"));
                ui.checkbox(&mut d.show_thumbs, "Page pictures");
                if !d.search_note.is_empty() {
                    ui.label(egui::RichText::new(d.search_note.clone()).small().color(theme::MUTED));
                }
            });
            d.search_tick(&ctx);
            let hits = d.hits.clone();
            if !hits.is_empty() {
                egui::ScrollArea::vertical().id_source("doc-hits").max_height(90.0).show(ui, |ui| {
                    for h in hits.iter().take(20) {
                        if ui.small_button(format!("p.{}  {}", h.page + 1, h.snippet)).clicked() {
                            d.goto(h.page);
                        }
                    }
                });
            }
            if d.show_thumbs {
                let count = d.page_count();
                egui::ScrollArea::horizontal().id_source("doc-thumbs").max_height(96.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for i in 0..count {
                            let cur = i == d.index;
                            match d.thumb(&ctx, i) {
                                Some(t) => {
                                    let size = t.size_vec2();
                                    let k = 80.0 / size.y.max(1.0);
                                    let img = egui::ImageButton::new(egui::load::SizedTexture::new(t.id(), size * k)).selected(cur);
                                    if ui.add(img).on_hover_text(format!("Page {}", i + 1)).clicked() {
                                        d.goto(i);
                                    }
                                }
                                None => {
                                    if ui.add_sized([60.0, 80.0], egui::Button::new(format!("{}", i + 1)).selected(cur)).clicked() {
                                        d.goto(i);
                                    }
                                }
                            }
                        }
                    });
                });
            }
        }
        if let Some(f) = fit_changed {
            self.settings.doc_fit_width = f;
            self.save_settings();
        }
        if let Some(n) = guide_lines_set {
            self.settings.doc_guide_lines = n;
            self.save_settings();
        }
        if let Some(c) = guide_color_set {
            self.settings.doc_guide_color = c % crate::stage::DOC_GUIDE_COLORS.len() as u8;
            self.save_settings();
        }
        if close {
            *request = self.doc_close();
        }
    }

    /// The play bar for the video or sound on the stage: Restart, Back 10s, Play/Pause, Forward 10s,
    /// a bar to drag, time played / length, time left, the clock time it ends, and Loop.
    fn transport_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, request: &mut Option<StageContent>) {
        let Some(t) = self.transport() else {
            self.scrub_ms = None;
            return;
        };
        let wall = self.wall_ms();
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(if t.is_video { "VIDEO" } else { "SOUND" }).small().strong().color(theme::MUTED));
            if ui.small_button("Restart").clicked() {
                self.media_restart();
            }
            if ui.small_button("Stop").on_hover_text("Stop and clear the screen").clicked() {
                *request = self.media_stop();
            }
            let jump = ui.add_enabled_ui(t.can_jump, |ui| {
                (ui.small_button("Back 10s").clicked(), ui.small_button("Forward 10s").clicked())
            });
            let (back, fwd) = jump.inner;
            if back {
                self.media_skip(-SKIP_MS);
            }
            if fwd {
                self.media_skip(SKIP_MS);
            }
            if t.playing {
                if ui.small_button("Pause").clicked() {
                    self.media_pause();
                }
            } else if ui.small_button("Play").clicked() {
                self.media_play();
            }
            let mut lp = self.settings.loop_video;
            if ui.checkbox(&mut lp, "Loop").changed() {
                self.set_loop(lp);
            }
            if let Some(Loaded::Video { path, .. }) | Some(Loaded::Audio { path, .. }) = &self.loaded {
                let a = self.after_of(path);
                let choices: Vec<(u64, String, String)> = self.shelf.entries.iter().map(|e| (e.id, e.name.clone(), e.path.clone())).collect();
                ui.label(egui::RichText::new(format!("When it ends: {}  (right-click it on the shelf to change)", after_label(&a, &shown_name(&choices, &a)))).small().color(theme::MUTED));
            }
            if !t.can_jump {
                ui.label(egui::RichText::new("(jumping in a sound needs ffmpeg: brew install ffmpeg)").small().color(theme::MUTED));
            }
        });
        ui.horizontal(|ui| {
            let shown = self.scrub_ms.unwrap_or(t.pos_ms);
            if t.dur_ms > 0 {
                let mut secs = shown as f32 / 1000.0;
                let r = ui.add_enabled(t.can_jump, egui::Slider::new(&mut secs, 0.0..=(t.dur_ms as f32 / 1000.0)).show_value(false).trailing_fill(true).smallest_positive(1.0));
                if r.changed() {
                    self.scrub_ms = Some((secs * 1000.0) as u64);
                }
                // a small picture of the video at the moment under the pointer (v0.835.0)
                let video_path = match &self.loaded {
                    Some(Loaded::Video { path, .. }) => Some(path.clone()),
                    _ => None,
                };
                let r = if let Some(vp) = video_path {
                    self.scrub.ensure(&vp, t.dur_ms);
                    self.scrub.poll(ctx);
                    let hover_ms = if r.dragged() {
                        self.scrub_ms
                    } else if r.hovered() {
                        ctx.input(|i| i.pointer.hover_pos()).map(|p| crate::scrub::time_at((p.x - r.rect.left()) / r.rect.width().max(1.0), t.dur_ms))
                    } else {
                        None
                    };
                    match hover_ms {
                        Some(ms) => {
                            self.scrub.want(ms);
                            let pic = self.scrub.picture(ms);
                            let ready = self.scrub.is_ready(ms);
                            if !ready {
                                ctx.request_repaint_after(Duration::from_millis(40));
                            }
                            r.on_hover_ui_at_pointer(|ui| {
                                match &pic {
                                    Some(tex) => {
                                        let sz = tex.size_vec2();
                                        let k = (200.0 / sz.x.max(1.0)).min(1.0).max(0.4);
                                        ui.image((tex.id(), sz * k));
                                    }
                                    None => {
                                        ui.label("...");
                                    }
                                }
                                let s = ms / 1000;
                                ui.label(egui::RichText::new(format!("{}:{:02}", s / 60, s % 60)).monospace());
                            })
                        }
                        None => r,
                    }
                } else {
                    r
                };
                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                    if let Some(ms) = self.scrub_ms.take() {
                        self.media_seek(ms);
                    }
                }
            }
            let mut line = t.clone();
            line.pos_ms = shown;
            ui.label(egui::RichText::new(transport_line(&line, wall)).small());
        });
        ctx.request_repaint_after(Duration::from_millis(if t.playing { 200 } else { 500 }));
    }

    #[allow(dead_code)]
    pub fn has_audio(&self) -> bool {
        matches!(self.loaded, Some(Loaded::Audio { .. }))
    }

    pub fn has_video(&self) -> bool {
        matches!(self.loaded, Some(Loaded::Video { .. }))
    }

    /// True while a video's sound is coming out of the speakers: the microphone is then ignored (S3.9).
    pub fn video_sound_playing(&mut self) -> bool {
        match &mut self.loaded {
            Some(Loaded::Video { sound, .. }) | Some(Loaded::Audio { sound, .. }) => sound.is_playing(),
            _ => false,
        }
    }

    /// S3.5. Returns false when there is no video on the stage.
    pub fn media_play(&mut self) -> bool {
        let mute = self.settings.mute;
        match &mut self.loaded {
            Some(Loaded::Video { player, sound, path, .. }) => {
                let ended = player.state().ended;
                player.play();
                if ended {
                    if !mute {
                        sound.start(path, 1.0);
                    }
                } else {
                    sound.resume();
                }
                true
            }
            Some(Loaded::Audio { sound, path, paused }) => {
                if sound.is_playing() {
                    return true;
                }
                if *paused {
                    sound.resume();
                } else {
                    sound.start(path, 1.0); // it had finished: play again from the start
                }
                *paused = false;
                true
            }
            _ => false,
        }
    }
    pub fn media_pause(&mut self) -> bool {
        match &mut self.loaded {
            Some(Loaded::Video { player, sound, .. }) => {
                player.pause();
                sound.pause();
                true
            }
            Some(Loaded::Audio { sound, paused, .. }) => {
                sound.pause();
                *paused = true;
                true
            }
            _ => false,
        }
    }
    /// What this file does when it ends.
    pub fn after_of(&self, path: &str) -> AfterEnd {
        self.settings.after_end.get(path).map(|v| AfterEnd::decode(v)).unwrap_or(AfterEnd::Nothing)
    }

    pub fn set_after(&mut self, path: &str, a: AfterEnd) {
        match a.encode() {
            Some(v) => {
                self.settings.after_end.insert(path.to_string(), v);
            }
            None => {
                self.settings.after_end.remove(path);
            }
        }
        self.save_settings();
    }

    /// What the play bar shows. `None` when no video or sound is on the stage.
    pub fn transport(&mut self) -> Option<Transport> {
        let looping = self.settings.loop_video;
        match &mut self.loaded {
            Some(Loaded::Video { player, .. }) => {
                let st = player.state();
                Some(Transport { pos_ms: st.position_ms, dur_ms: st.duration_ms, playing: st.playing, ended: st.ended, looping: st.looping, is_video: true, can_jump: true, seeking: false })
            }
            Some(Loaded::Audio { sound, paused, .. }) => {
                let seeking = sound.is_seeking();
                let playing = sound.is_playing();
                let ended = !seeking && !*paused && !playing && sound.finished_ok();
                Some(Transport { pos_ms: sound.position_ms(), dur_ms: sound.duration_ms(), playing, ended, looping, is_video: false, can_jump: sound.is_native() || rf_media::convert::find_ffmpeg().is_some(), seeking })
            }
            _ => None,
        }
    }

    /// Jump to `ms` in the video or sound on the stage. False if there is none, or a sound could not jump (a message says why).
    pub fn media_seek(&mut self, ms: u64) -> bool {
        let mute = self.settings.mute;
        let sync_ms = self.settings.sound_sync_ms;
        let mut told: Option<String> = None;
        let ok = match &mut self.loaded {
            Some(Loaded::Video { player, sound, .. }) => {
                let was_playing = player.state().playing;
                sound.latency_ms = sync_ms;
                player.seek(ms);
                if !mute && !sound.path_is_empty() {
                    match sound.seek(ms) {
                        Ok(()) => {
                            if was_playing {
                                if sound.is_native() {
                                    // v0.839.0: the Mac's player jumps instantly; nothing to wait for
                                    sound.resume();
                                } else {
                                    // afplay fallback (v0.836.0): hold the picture still until the sound for the new
                                    // time is ready, then let both go together
                                    player.pause();
                                    sound.resume();
                                    sound.hold_video();
                                }
                            }
                        }
                        Err(e) => told = Some(e),
                    }
                }
                true
            }
            Some(Loaded::Audio { sound, paused, .. }) => match sound.seek(ms) {
                Ok(()) => {
                    if !*paused {
                        sound.resume();
                    }
                    true
                }
                Err(e) => {
                    told = Some(e);
                    false
                }
            },
            _ => false,
        };
        if let Some(m) = told {
            self.say(m);
        }
        ok
    }

    /// Back (negative) or forward (positive) from where it is now.
    pub fn media_skip(&mut self, delta_ms: i64) -> bool {
        let Some(t) = self.transport() else { return false };
        let target = match &self.loaded {
            Some(Loaded::Audio { sound, .. }) => crate::sound::jump_target(t.pos_ms, delta_ms, sound.duration_ms()),
            _ => (t.pos_ms as i64 + delta_ms).clamp(0, t.dur_ms.max(1) as i64) as u64,
        };
        self.media_seek(target)
    }

    /// Stop the video or sound and take it off the screen. Returns the blank stage to show (None if nothing was playing).
    pub fn media_stop(&mut self) -> Option<StageContent> {
        self.loaded.as_ref()?;
        // Stop means "I am done with it": the next time it starts from the top
        let stopped = match &self.loaded {
            Some(Loaded::Video { path, .. }) | Some(Loaded::Audio { path, .. }) => Some(path.clone()),
            _ => None,
        };
        if let Some(p) = stopped {
            if self.settings.resume.remove(&p).is_some() {
                self.save_settings();
            }
        }
        self.loaded = None; // dropping it stops the picture thread and the sound
        self.loaded_idx = None;
        self.loaded_id = None;
        self.scrub_ms = None;
        self.scrub.stop();
        Some(StageContent { updated: Some(Instant::now()), ..Default::default() })
    }

    pub fn media_restart(&mut self) -> bool {
        let mute = self.settings.mute;
        match &mut self.loaded {
            Some(Loaded::Video { player, sound, path, .. }) => {
                player.restart();
                if !mute {
                    sound.start(path, 1.0);
                }
                true
            }
            Some(Loaded::Audio { sound, path, paused }) => {
                sound.start(path, 1.0);
                *paused = false;
                true
            }
            _ => false,
        }
    }
    pub fn set_loop(&mut self, on: bool) {
        self.settings.loop_video = on;
        if let Some(Loaded::Video { player, .. }) = &self.loaded {
            player.set_looping(on);
        }
        self.save_settings();
    }
    pub fn set_mute(&mut self, on: bool) {
        self.settings.mute = on;
        if on {
            if let Some(Loaded::Video { sound, .. }) = &mut self.loaded {
                sound.stop();
            }
        }
        self.save_settings();
    }

    // ---------------------------------------------------------- background

    pub fn set_background(&mut self, ctx: &egui::Context, path: &str) {
        let name = file_name(path);
        match classify(path) {
            Classified::Ok(MediaKind::Image) => match load_image(Path::new(path), MAX_IMAGE_SIDE) {
                Ok(img) => {
                    self.bg = Some(Background { name, size: [img.width as f32, img.height as f32], tex: Some(Self::texture(ctx, path, &img)), player: None });
                }
                Err(e) => return self.say(e),
            },
            Classified::Ok(MediaKind::Video) => match Player::open(Path::new(path), true) {
                Ok(player) => {
                    let info = player.info().clone();
                    player.play(); // a background loop is always looping and silent (S4.1)
                    self.bg = Some(Background { name, size: [info.width as f32, info.height as f32], tex: None, player: Some((player, 0)) });
                }
                Err(e) => return self.say(e),
            },
            Classified::Ok(MediaKind::Audio) => return self.say("A sound file cannot be a background. Choose a picture or a video."),
            Classified::Ok(MediaKind::Document) => return self.say("A PDF or HTML file cannot be a background. Choose a picture or a video."),
            Classified::Unsupported(m) => return self.say(m),
            Classified::Unknown => return self.say("That is not a picture or a video."),
        }
        self.settings.background = Some(path.to_string());
        self.save_settings();
        self.say("Background set. It shows behind slides and timers (never behind scripture).");
    }

    pub fn clear_background(&mut self) {
        self.bg = None;
        self.settings.background = None;
        self.save_settings();
    }

    /// Load the remembered background at start-up (needs a graphics context).
    pub fn restore_background(&mut self, ctx: &egui::Context) {
        if self.bg.is_none() {
            if let Some(p) = self.settings.background.clone() {
                if Path::new(&p).exists() {
                    self.set_background(ctx, &p);
                }
            }
        }
    }

    // -------------------------------------------------------------- timers

    fn build_timer(&mut self) -> Result<Timer, String> {
        let title = self.timer_ui.title.trim().to_string();
        let kind = match self.timer_ui.choice {
            TimerChoice::CountDown => TimerKind::CountDown { total_ms: self.timer_ui.minutes.max(0) as u64 * 60_000 },
            TimerChoice::UntilClock => {
                let secs = parse_time_of_day(&self.timer_ui.until_text).ok_or_else(|| "Type the time like 10:30, 9:05pm or 0930.".to_string())?;
                TimerKind::UntilClock { target_secs: secs }
            }
            TimerChoice::StopWatch => TimerKind::StopWatch,
            TimerChoice::Clock => TimerKind::Clock { twelve_hour: self.settings.twelve_hour },
        };
        Ok(Timer::new(kind, &title))
    }

    pub fn timer_start(&mut self) {
        match self.build_timer() {
            Ok(mut t) => {
                self.sync_clock();
                t.start(self.now_ms(), self.wall_ms());
                self.timer = t;
            }
            Err(e) => self.say(e),
        }
    }
    pub fn timer_pause(&mut self) {
        let n = self.now_ms();
        self.timer.pause(n);
    }
    pub fn timer_resume(&mut self) {
        let n = self.now_ms();
        self.timer.resume(n);
    }
    pub fn timer_reset(&mut self) {
        if let Ok(t) = self.build_timer() {
            self.timer = t;
        }
    }
    pub fn timer_display(&self) -> String {
        self.timer.display(self.now_ms(), self.wall_ms())
    }
    pub fn timer_phase(&self) -> Phase {
        self.timer.phase(self.now_ms(), self.wall_ms())
    }

    /// S5: put the timer on the stage. A timer that was never started is built from the inputs.
    pub fn stage_timer(&mut self) -> Option<StageContent> {
        if self.timer.kind() != self.build_timer().ok().map(|t| t.kind()).unwrap_or(self.timer.kind()) && self.timer_phase() == Phase::Idle {
            self.timer_reset();
        }
        Some(StageContent {
            kind: StageKind::Countdown,
            label: self.timer.title().to_string(),
            text: self.timer_display(),
            translation: String::new(),
            chapter_only: false,
            updated: Some(Instant::now()),
        })
    }

    /// "Show the clock": the time of day on the stage (replaces the panel's timer).
    pub fn stage_clock(&mut self) -> StageContent {
        self.timer = Timer::new(TimerKind::Clock { twelve_hour: self.settings.twelve_hour }, "");
        self.sync_clock();
        let (n, w) = (self.now_ms(), self.wall_ms());
        self.timer.start(n, w);
        StageContent { kind: StageKind::Countdown, label: String::new(), text: self.timer.display(n, w), translation: String::new(), chapter_only: false, updated: Some(Instant::now()) }
    }

    // ---------------------------------------------------------------- tick

    pub fn tick(&mut self, ctx: &egui::Context, stage: &mut StageContent) -> bool {
        KEEP_AWAKE.store(false, std::sync::atomic::Ordering::Relaxed); // the video arm below sets it again while playing
        let (n, w) = (self.now_ms(), self.wall_ms());
        self.tick_at(ctx, stage, n, w)
    }

    /// Called every frame by the app. Frees media that left the stage (which stops a video and its
    /// sound), uploads the newest video frame, keeps a countdown on the stage up to date.
    /// Returns true when the stage text changed (so the OBS page gets a new version).
    pub fn tick_at(&mut self, ctx: &egui::Context, stage: &mut StageContent, now_ms: u64, wall_ms: u64) -> bool {
        self.poll_jobs();
        self.poll_pick(ctx);
        if let Some((_, t)) = &self.msg {
            if t.elapsed() > Duration::from_secs(12) {
                self.msg = None;
            }
        }
        // S0.2 / S0.3: a video that is no longer on the stage stops
        if stage.kind != StageKind::Media && self.loaded.is_some() {
            self.bookmark_current(); // going away from a video or document: remember the place
            self.loaded = None;
            self.loaded_idx = None;
            self.loaded_id = None;
        }
        if let Some(Loaded::Document(d)) = &mut self.loaded {
            d.tick(ctx);
        }
        let mute = self.settings.mute;
        let mut told: Option<String> = None;
        if let Some(Loaded::Video { player, tex, serial, size, sound, path, last_pos, error_told }) = &mut self.loaded {
            if let Some((s, f)) = player.latest() {
                if s != *serial {
                    *serial = s;
                    *size = [f.image.width as f32, f.image.height as f32];
                    let ci = egui::ColorImage::from_rgba_unmultiplied([f.image.width as usize, f.image.height as usize], &f.image.pixels);
                    match tex {
                        Some(t) => t.set(ci, egui::TextureOptions::LINEAR),
                        None => *tex = Some(ctx.load_texture("video", ci, egui::TextureOptions::LINEAR)),
                    }
                }
            }
            let st = player.state();
            if let (Some(e), false) = (&st.error, *error_told) {
                *error_told = true;
                told = Some(e.clone());
            }
            if st.looping && st.position_ms + 500 < *last_pos && !mute {
                sound.start(path, 1.0); // the picture wrapped round; the sound starts again with it
            }
            *last_pos = st.position_ms;
            KEEP_AWAKE.store(st.playing, std::sync::atomic::Ordering::Relaxed);
            ctx.request_repaint_after(Duration::from_millis(if st.playing { 16 } else { 100 }));
        }
        if let Some(b) = &mut self.bg {
            if let Some((player, serial)) = &mut b.player {
                if let Some((s, f)) = player.latest() {
                    if s != *serial {
                        *serial = s;
                        b.size = [f.image.width as f32, f.image.height as f32];
                        let ci = egui::ColorImage::from_rgba_unmultiplied([f.image.width as usize, f.image.height as usize], &f.image.pixels);
                        match &mut b.tex {
                            Some(t) => t.set(ci, egui::TextureOptions::LINEAR),
                            None => b.tex = Some(ctx.load_texture("background-video", ci, egui::TextureOptions::LINEAR)),
                        }
                    }
                }
                if matches!(stage.kind, StageKind::Slide | StageKind::Countdown) {
                    ctx.request_repaint_after(Duration::from_millis(33));
                }
            }
        }
        if told.is_none() {
            if let Some(Loaded::Document(d)) = &mut self.loaded {
                if let Some(e) = d.error.take() {
                    d.error_told = true;
                    told = Some(e);
                }
            }
        }
        // sound failures (bad format, unreadable file) are told once, in plain words
        let looping = self.settings.loop_video;
        match &mut self.loaded {
            Some(Loaded::Video { player, sound, path, .. }) if told.is_none() => {
                if sound.is_seeking() || sound.is_holding() {
                    ctx.request_repaint_after(Duration::from_millis(30)); // keep checking until the jumped-to sound is ready and started
                }
                told = sound.poll_seek();
                if told.is_none() {
                    told = sound.poll_failure(path);
                }
                if sound.take_video_release() {
                    player.play(); // picture and the new sound start together
                }
                // v0.839.0: the sound is the clock. If the picture has wandered from the sound's true position
                // (a stall, a busy Mac), pull it back; no setting to adjust.
                if sound.is_native() && !sound.is_holding() && sound.drift_check_due() && sound.is_playing() {
                    let st = player.state();
                    if st.playing && !st.ended {
                        let drift = sound.position_ms() as i64 - st.position_ms as i64;
                        if drift.abs() > 90 && drift.abs() < 2000 {
                            player.sync_to(sound.position_ms());
                        }
                    }
                }
            }
            Some(Loaded::Audio { sound, path, paused }) => {
                told = sound.poll_seek();
                if told.is_none() {
                    told = sound.poll_failure(path);
                }
                if told.is_none() && looping && !*paused && sound.finished_ok() {
                    sound.start(path, 1.0); // "Loop" also repeats a song
                }
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            _ => {}
        }
        if let Some(m) = told {
            self.say(m);
        }
        let mut changed = false;
        // "When it ends": a video or sound that played to its end (Loop off) hands over to what was chosen
        if !self.after_fired && !looping {
            let ended_path = match &mut self.loaded {
                Some(Loaded::Video { player, path, .. }) if player.state().ended => Some(path.clone()),
                Some(Loaded::Audio { sound, path, paused }) => if !*paused && sound.finished_ok() { Some(path.clone()) } else { None },
                _ => None,
            };
            if let Some(p) = ended_path {
                self.after_fired = true;
                match self.after_of(&p) {
                    AfterEnd::Nothing => {}
                    AfterEnd::Clear => {
                        *stage = StageContent::default();
                        stage.updated = Some(Instant::now());
                        changed = true;
                    }
                    AfterEnd::Show(next) => {
                        if let Some(c) = self.stage_path(ctx, &next) {
                            *stage = c;
                            changed = true;
                        }
                    }
                }
            }
        }
        if stage.kind == StageKind::Countdown {
            let t = self.timer.display(now_ms, wall_ms);
            if t != stage.text {
                stage.text = t;
                changed = true;
            }
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if self.open && self.tab == Tab::Timers {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        changed
    }

    /// What the stage should paint right now.
    pub fn art(&self, kind: &StageKind) -> StageArt {
        let mut art = StageArt { fit: fit_of(&self.settings.fit), dim_pct: self.settings.dim_pct.min(MAX_DIM_PCT), ..Default::default() };
        if *kind == StageKind::Media {
            art.media = match &self.loaded {
                Some(Loaded::Image { tex, size }) => Some(ArtImage { texture: tex.id(), size: *size }),
                Some(Loaded::Video { tex: Some(t), size, .. }) => Some(ArtImage { texture: t.id(), size: *size }),
                _ => None,
            };
            art.sound_only = matches!(self.loaded, Some(Loaded::Audio { .. }));
            if let Some(Loaded::Document(d)) = &self.loaded {
                art.doc = d.art();
            }
        }
        if matches!(kind, StageKind::Slide | StageKind::Countdown) {
            if let Some(Background { tex: Some(t), size, .. }) = &self.bg {
                art.background = Some(ArtImage { texture: t.id(), size: *size });
            }
        }
        art
    }

    // ----------------------------------------------------------------- UI

    /// The window. Returns something to put on the stage if the operator pressed a "Show" button.
    pub fn ui(&mut self, ctx: &egui::Context) -> Option<StageContent> {
        if !self.open {
            return None;
        }
        let mut open = true;
        let mut request: Option<StageContent> = None;
        egui::Window::new("Songs, slides, backgrounds and timers")
            .id(egui::Id::new("present_window"))
            .open(&mut open)
            .default_size([860.0, 640.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (t, name) in [(Tab::Slides, "Slides"), (Tab::Songs, "Songs"), (Tab::Background, "Background"), (Tab::Timers, "Timers")] {
                        if ui.selectable_label(self.tab == t, name).clicked() {
                            self.tab = t;
                        }
                    }
                });
                if let Some((m, _)) = &self.msg {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(m).color(theme::GOLD));
                }
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                    Tab::Media => self.tab_media(ui, ctx, &mut request),
                    Tab::Slides => self.tab_slides(ui, &mut request),
                    Tab::Songs => self.tab_songs(ui, &mut request),
                    Tab::Background => self.tab_background(ui, ctx),
                    Tab::Timers => self.tab_timers(ui, &mut request),
                });
            });
        self.open = open;
        request
    }

    fn add_row(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, purpose: PickFor, what: &str) {
        ui.horizontal(|ui| {
            if ui.button(what).clicked() {
                self.spawn_pick(purpose);
            }
            ui.label("or paste a path:");
            let r = ui.add(egui::TextEdit::singleline(&mut self.path_input).desired_width(220.0));
            if ui.button("Add").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                self.add_typed_path(ctx, purpose);
            }
        });
    }

    /// The old Pictures & video tab: media now lives in the bar docked at the bottom of the main window.
    fn tab_media(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context, _request: &mut Option<StageContent>) {
        ui.add_space(8.0);
        ui.label("Pictures and videos now live in the MEDIA bar at the bottom of the main window: drag files onto it and click a thumbnail to show it.");
        if ui.button("Show the MEDIA bar").clicked() {
            self.dock_open = true;
        }
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Video sound plays through the Mac's own sound player and the microphone is ignored while it plays.").weak());
        ui.label(egui::RichText::new(CISCO_CREDIT).weak().small());
    }

    /// The MEDIA bar, docked at the bottom of the main window (not a pop-up). Drop pictures, videos or
    /// whole folders on it; each shows as a thumbnail; click one to show it on screen 2.
    pub fn dock_ui(&mut self, ctx: &egui::Context) -> Option<StageContent> {
        self.poll_jobs();
        let mut request: Option<StageContent> = None;
        let hovering_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if !self.dock_auto_done {
            self.dock_auto_done = true;
            if ctx.screen_rect().height() < 760.0 && !hovering_files {
                self.dock_open = false;
            }
        }
        let frame = egui::Frame::none().fill(theme::PANEL).inner_margin(egui::Margin::symmetric(10.0, 6.0));
        let panel = egui::TopBottomPanel::bottom("media_dock").frame(frame);
        let panel = if self.dock_open {
            panel.resizable(true).default_height((ctx.screen_rect().height() * 0.17).clamp(110.0, 190.0)).min_height(90.0).max_height(560.0)
        } else {
            panel.resizable(false).exact_height(36.0)
        };
        panel.show(ctx, |ui| {
            self.dock_contents(ui, ctx, &mut request, hovering_files);
        });
        request
    }

    /// One thumbnail card (grid view).
    #[allow(clippy::too_many_arguments)]
    fn dock_card(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, id: u64, n: usize, w: f32, h: f32, on_path: &Option<String>, folders: &[String], out: &mut DockOut) {
        let tex = self.thumb(ctx, id);
        let dur = self.durations.get(&id).copied();
        let Some(e) = self.shelf.get(id) else { return };
        let (name, path, is_missing, is_copy) = (e.name.clone(), e.path.clone(), !Path::new(&e.path).exists(), e.mode == ShelfMode::Copy);
        let is_video = self.shelf.kind(id) == Some(MediaKind::Video);
        let is_audio = self.shelf.kind(id) == Some(MediaKind::Audio);
        let on = on_path.is_some() && match self.loaded_id { Some(l) => l == id, None => on_path.as_deref() == Some(path.as_str()) };
        let cursor = self.dock_cursor == Some(id);
        let selected = self.dock_sel.contains(&id);
        ui.vertical(|ui| {
            ui.set_width(w);
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click_and_drag());
            let painter = ui.painter();
            painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 22, 28));
            match &tex {
                Some(t) => {
                    let ts = t.size_vec2();
                    let k = (rect.width() / ts.x).min(rect.height() / ts.y);
                    let r = egui::Rect::from_center_size(rect.center(), ts * k);
                    painter.image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                }
                None => {
                    let t = if is_missing { "missing" } else if is_video { "\u{25b6} video" } else if is_audio { "\u{266a} sound" } else { "picture" };
                    painter.text(rect.center(), egui::Align2::CENTER_CENTER, t, egui::FontId::proportional(13.0), egui::Color32::from_gray(150));
                }
            }
            if let Some(ms) = dur {
                let s = ms / 1000;
                let g = painter.layout_no_wrap(format!("{}:{:02}", s / 60, s % 60), egui::FontId::monospace(11.0), egui::Color32::WHITE);
                let br = egui::Rect::from_min_size(egui::pos2(rect.left() + 3.0, rect.bottom() - g.size().y - 5.0), g.size() + egui::vec2(6.0, 2.0));
                painter.rect_filled(br, 2.0, egui::Color32::from_black_alpha(170));
                painter.galley(br.min + egui::vec2(3.0, 1.0), g, egui::Color32::WHITE);
            }
            if on {
                // on the screen right now: a badge as well as a border (colour alone is not enough)
                painter.rect_stroke(rect, 4.0, egui::Stroke::new(3.0, egui::Color32::from_rgb(60, 130, 246)));
                let g = painter.layout_no_wrap("ON SCREEN".into(), egui::FontId::proportional(10.0), egui::Color32::WHITE);
                let br = egui::Rect::from_min_size(rect.right_top() - egui::vec2(g.size().x + 10.0, 0.0), g.size() + egui::vec2(8.0, 3.0));
                painter.rect_filled(br, 2.0, egui::Color32::from_rgb(60, 130, 246));
                painter.galley(br.min + egui::vec2(4.0, 1.5), g, egui::Color32::WHITE);
            } else if is_missing {
                painter.rect_stroke(rect, 4.0, egui::Stroke::new(2.0, theme::RED));
            } else if resp.hovered() {
                painter.rect_stroke(rect, 4.0, egui::Stroke::new(2.0, theme::GOLD));
            }
            if selected {
                // SELECTED: a tint, a teal border and a tick badge (the tick, so colour is not the only clue)
                painter.rect_filled(rect, 4.0, SEL_FILL);
                painter.rect_stroke(rect.expand(2.5), 6.0, egui::Stroke::new(2.5, SEL));
                let br = egui::Rect::from_min_size(rect.left_top() + egui::vec2(4.0, 4.0), egui::vec2(18.0, 18.0));
                painter.circle_filled(br.center(), 9.0, SEL);
                painter.text(br.center(), egui::Align2::CENTER_CENTER, "\u{2713}", egui::FontId::proportional(13.0), egui::Color32::BLACK);
            }
            if cursor && !selected {
                painter.rect_stroke(rect.expand(2.0), 5.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
            }
            out.rects.push((id, [rect.left(), rect.top(), rect.right(), rect.bottom() + 18.0]));
            let mut short = name.clone();
            let max_chars = ((w / 7.2) as usize).max(8);
            if short.chars().count() > max_chars {
                short = short.chars().take(max_chars - 1).collect::<String>() + "\u{2026}";
            }
            let label = egui::RichText::new(format!("{} {}", n + 1, short)).small();
            ui.label(if selected { label.color(SEL).strong() } else { label });
            let resp = resp.on_hover_text(format!("{name}\n{}\nClick = show. Cmd+click = add to selection. Shift+click = range. Drag a box around files to select them.", if is_copy { "RhemaFlow keeps its own copy" } else { "linked to the original file" }));
            if resp.clicked() {
                let (cmd, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                out.picked = Some((id, cmd, shift));
                out.clicked = Some(id);
                if !is_missing && !cmd && !shift {
                    out.show = Some(id);
                }
            }
            if resp.secondary_clicked() && !selected {
                out.picked = Some((id, false, false));
            }
            if resp.drag_started() {
                out.drag_began = Some(id);
            }
            resp.dnd_set_drag_payload(id);
            let timed = matches!(self.shelf.kind(id), Some(MediaKind::Video) | Some(MediaKind::Audio));
        out.menu_note = self.last_shown_text(id);
        resp.context_menu(|ui| item_menu(ui, id, &name, is_missing, timed, folders, out));
        });
    }

    /// One row (list view): easier to scan when there are many files.
    fn dock_row(&mut self, ui: &mut egui::Ui, id: u64, n: usize, on_path: &Option<String>, folders: &[String], out: &mut DockOut) {
        let Some(e) = self.shelf.get(id) else { return };
        let (name, path, is_missing, size, folder) = (e.name.clone(), e.path.clone(), !Path::new(&e.path).exists(), e.size, e.folder.clone());
        let kind = self.shelf.kind(id);
        let on = on_path.is_some() && match self.loaded_id { Some(l) => l == id, None => on_path.as_deref() == Some(path.as_str()) };
        let cursor = self.dock_cursor == Some(id);
        let selected = self.dock_sel.contains(&id);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), egui::Sense::click_and_drag());
        let painter = ui.painter();
        if on {
            painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(30, 60, 120));
        } else if resp.hovered() {
            painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(34, 38, 46));
        }
        if selected {
            painter.rect_filled(rect, 3.0, SEL_FILL);
            painter.rect_stroke(rect, 3.0, egui::Stroke::new(1.5, SEL));
            painter.text(rect.left_center() + egui::vec2(-2.0, 0.0), egui::Align2::LEFT_CENTER, "\u{2713}", egui::FontId::proportional(14.0), SEL);
        } else if cursor {
            painter.rect_stroke(rect, 3.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
        }
        out.rects.push((id, [rect.left(), rect.top(), rect.right(), rect.bottom()]));
        let col = if is_missing { theme::RED } else { theme::TEXT };
        painter.text(rect.left_center() + egui::vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, format!("{:>3}   {}  {}", n + 1, match kind { Some(MediaKind::Video) => "\u{25b6}", Some(MediaKind::Audio) => "\u{266a}", _ => "\u{25aa}" }, name), egui::FontId::proportional(13.0), col);
        let right = format!("{}{}{}", if on { "ON SCREEN   " } else { "" }, if is_missing { "missing   ".to_string() } else { String::new() }, human_size(size));
        painter.text(rect.right_center() - egui::vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, right, egui::FontId::proportional(12.0), theme::MUTED);
        if !folder.is_empty() {
            painter.text(rect.right_center() - egui::vec2(150.0, 0.0), egui::Align2::RIGHT_CENTER, format!("\u{1f4c1} {folder}"), egui::FontId::proportional(12.0), theme::MUTED);
        }
        if resp.clicked() {
            let (cmd, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
            out.picked = Some((id, cmd, shift));
            out.clicked = Some(id);
            if !is_missing && !cmd && !shift {
                out.show = Some(id);
            }
        }
        if resp.secondary_clicked() && !selected {
            out.picked = Some((id, false, false));
        }
        if resp.drag_started() {
            out.drag_began = Some(id);
        }
        resp.dnd_set_drag_payload(id);
        let timed = matches!(self.shelf.kind(id), Some(MediaKind::Video) | Some(MediaKind::Audio));
        out.menu_note = self.last_shown_text(id);
        resp.context_menu(|ui| item_menu(ui, id, &name, is_missing, timed, folders, out));
    }

    // ------------------------------------------------------------------ selection (v0.844.0)

    /// The files in the folder being looked at, in the order they are drawn.
    pub fn dock_visible_ids(&self) -> Vec<u64> {
        let by = match self.settings.sort.as_str() {
            "newest" => SortBy::Newest,
            "name" => SortBy::Name,
            "recent" => SortBy::Recent,
            _ => SortBy::Added,
        };
        self.shelf.list_sorted(self.shelf_view.as_deref(), &self.shelf_filter, by)
    }

    /// Cmd+A: select every file in the folder being looked at.
    pub fn dock_select_all(&mut self) -> String {
        self.dock_open = true;
        let ids = self.dock_visible_ids();
        if ids.is_empty() {
            return "There are no files here to select.".into();
        }
        self.dock_anchor = ids.first().copied();
        self.dock_sel = ids.into_iter().collect();
        format!("Selected all {} files. Cmd+C copies them, Delete removes them from RhemaFlow, Esc selects nothing.", self.dock_sel.len())
    }

    pub fn dock_clear_selection(&mut self) {
        self.dock_sel.clear();
        self.dock_del_armed = None;
    }

    /// The selected files in the order they are drawn: (name, path on disk).
    pub fn dock_selected_files(&self) -> Vec<(String, String)> {
        self.dock_visible_ids()
            .into_iter()
            .filter(|id| self.dock_sel.contains(id))
            .filter_map(|id| self.shelf.get(id).map(|e| (e.name.clone(), e.path.clone())))
            .collect()
    }

    /// Cmd+C on the selected files. On a Mac the FILES go on the clipboard (paste them in Finder, Mail or here);
    /// anywhere else, or if that fails, their paths go on it as text. Returns what to tell the operator.
    pub fn dock_copy_selected(&mut self, ctx: &egui::Context) -> String {
        let files = self.dock_selected_files();
        if files.is_empty() {
            return "Nothing is selected. Click a file (Cmd+click or Shift+click for several, or drag a box round them), then press Cmd+C.".into();
        }
        let paths: Vec<String> = files.iter().map(|(_, p)| p.clone()).collect();
        let n = paths.len();
        if crate::clip::write_clipboard_files(&paths) {
            return format!("Copied {n} file{}. Paste them in Finder, or press Cmd+V here to add them again.", if n == 1 { "" } else { "s" });
        }
        ctx.copy_text(paths.join("\n"));
        format!("Copied the path{} of {n} file{} as text.", if n == 1 { "" } else { "s" }, if n == 1 { "" } else { "s" })
    }

    /// Delete on the selected files: the first press asks, a second press within 5 seconds removes them.
    /// (RhemaFlow's own copy goes; a linked original is never touched.)
    pub fn dock_remove_selected(&mut self) {
        let ids: Vec<u64> = self.dock_visible_ids().into_iter().filter(|id| self.dock_sel.contains(id)).collect();
        if ids.is_empty() {
            return;
        }
        if self.locked {
            return self.say(crate::lock::refusal(crate::lock::Guarded::EditMedia));
        }
        let armed = self.dock_del_armed.map_or(false, |t| t.elapsed() < Duration::from_secs(5));
        if !armed {
            self.dock_del_armed = Some(Instant::now());
            self.say(format!("Press Delete again to remove {} file{} from RhemaFlow (your originals are not touched). Esc cancels.", ids.len(), if ids.len() == 1 { "" } else { "s" }));
            return;
        }
        self.dock_del_armed = None;
        let n = ids.len();
        for id in ids {
            self.remove_from_shelf(id);
        }
        self.dock_sel.clear();
        self.dock_cursor = None;
        self.say(format!("Removed {n} file{} from RhemaFlow.", if n == 1 { "" } else { "s" }));
    }

    /// The drag box: press on empty space in the bar and drag; everything the box touches is selected.
    /// Hold Cmd to add to what is already selected. A plain click on empty space selects nothing.
    fn dock_drag_box(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, body: egui::Rect, dragging_file: bool) {
        let (pressed, down, pos, mods) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.latest_pos(), i.modifiers));
        if pressed && !dragging_file {
            if let Some(p) = pos {
                let on_item = self.dock_rects.iter().any(|(_, r)| p.x >= r[0] && p.x <= r[2] && p.y >= r[1] && p.y <= r[3]);
                let in_body = body.contains(p) && p.x < body.right() - 14.0; // not on the scroll bar
                let top_layer = ctx.layer_id_at(p).map_or(true, |l| l == ui.layer_id()); // not under a menu
                if in_body && !on_item && top_layer {
                    let base = if mods.command { self.dock_sel.clone() } else { std::collections::HashSet::new() };
                    self.dock_box = Some(([p.x, p.y], base));
                }
            }
        }
        let Some((start, base)) = self.dock_box.clone() else { return };
        let cur = pos.map(|p| [p.x, p.y]).unwrap_or(start);
        let real = crate::selection::is_real_drag(start, cur);
        if down {
            if real {
                let hits = crate::selection::box_hits(&self.dock_rects, start, cur);
                self.dock_sel = base.iter().copied().chain(hits.iter().copied()).collect();
                if let Some(first) = hits.first() {
                    self.dock_anchor = Some(*first);
                }
                let r = egui::Rect::from_two_pos(egui::pos2(start[0], start[1]), egui::pos2(cur[0], cur[1]));
                ui.painter().rect(r, 2.0, SEL.gamma_multiply(0.18), egui::Stroke::new(1.5, SEL));
            }
            ctx.request_repaint();
        } else {
            if !real && !mods.command {
                self.dock_sel.clear(); // a click on empty space
            }
            self.dock_box = None;
        }
    }

    fn dock_contents(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, request: &mut Option<StageContent>, hovering_files: bool) {
        self.thumb_busy = false;
        let folders = self.shelf.folders();
        let on_path = self.loaded_idx.and_then(|i| self.settings.media.get(i).cloned());
        let mut out = DockOut::default();
        out.choices = self.shelf.entries.iter().map(|e| (e.id, e.name.clone(), e.path.clone())).collect();
        for e in &self.shelf.entries {
            if matches!(self.shelf.kind(e.id), Some(MediaKind::Video) | Some(MediaKind::Audio)) {
                let a = self.after_of(&e.path);
                if a != AfterEnd::Nothing {
                    out.after_now.insert(e.id, a);
                }
            }
        }
        {
            // files that were removed leave the selection
            let shelf = &self.shelf;
            crate::selection::keep_existing(&mut self.dock_sel, |id| shelf.get(id).is_some());
        }
        out.sel_count = self.dock_sel.len();
        let mut view_ids: Vec<u64> = Vec::new();
        let mut folder_rects: Vec<(String, egui::Rect)> = Vec::new();
        let mut folder_action: Option<(String, bool)> = None;
        let dragging = egui::DragAndDrop::has_payload_of_type::<u64>(ctx);
        let missing = self.shelf.missing_ids().len();
        let panel_rect = ui.max_rect();

        // ---------- row 1: collapse, title, folders
        ui.horizontal_wrapped(|ui| {
            if ui.small_button(if self.dock_open { "\u{25be}" } else { "\u{25b8}" }).on_hover_text("Hide / show the media bar").clicked() {
                self.dock_open = !self.dock_open;
            }
            ui.label(egui::RichText::new("MEDIA").small().strong().color(theme::MUTED));
            if !self.dock_open {
                ui.label(egui::RichText::new(format!("{} item(s)", self.shelf.entries.len())).small().color(theme::MUTED));
                return;
            }
            let mut rows: Vec<Option<String>> = vec![None];
            rows.extend(folders.iter().cloned().map(Some));
            for key in rows {
                let count = self.shelf.count_in(key.as_deref());
                if let (Some(name), Some((Some(old), _))) = (&key, &self.folder_edit) {
                    if old == name {
                        let mut text = self.folder_edit.as_ref().map(|f| f.1.clone()).unwrap_or_default();
                        let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(110.0));
                        r.request_focus();
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            match self.shelf.rename_folder(old, &text) {
                                Ok(()) => {
                                    if self.shelf_view.as_deref() == Some(old.as_str()) {
                                        self.shelf_view = Some(text.trim().to_string());
                                    }
                                    let _ = self.shelf.save();
                                }
                                Err(e) => self.say(e),
                            }
                            self.folder_edit = None;
                        } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            self.folder_edit = None;
                        } else {
                            self.folder_edit = Some((Some(old.clone()), text));
                        }
                        continue;
                    }
                }
                let label = match &key {
                    None => format!("All  {count}"),
                    Some(f) => format!("\u{1f4c1} {f}  {count}"),
                };
                let resp = ui.selectable_label(self.shelf_view == key, label);
                folder_rects.push((key.clone().unwrap_or_default(), resp.rect));
                if resp.clicked() {
                    self.shelf_view = key.clone();
                }
                if dragging && resp.dnd_hover_payload::<u64>().is_some() {
                    ui.painter().rect_stroke(resp.rect, 3.0, egui::Stroke::new(2.0, theme::GOLD));
                }
                if let Some(id) = resp.dnd_release_payload::<u64>() {
                    let to = key.clone().unwrap_or_default();
                    if self.dock_sel.contains(&*id) {
                        // dragging one of several selected files moves them all
                        for sid in self.dock_sel.iter().copied() {
                            out.moves.push((sid, to.clone()));
                        }
                    } else {
                        out.moves.push((*id, to));
                    }
                }
                resp.context_menu(|ui| {
                    if ui.button("New folder").clicked() {
                        out.new_folder = true;
                        ui.close_menu();
                    }
                    if let Some(f) = &key {
                        if ui.button("Rename folder").clicked() {
                            folder_action = Some((f.clone(), false));
                            ui.close_menu();
                        }
                        if ui.button("Delete folder (files are kept)").clicked() {
                            folder_action = Some((f.clone(), true));
                            ui.close_menu();
                        }
                    }
                });
            }
            if let Some((None, text0)) = self.folder_edit.clone() {
                let mut text = text0;
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(110.0));
                r.request_focus();
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    match self.shelf.create_folder(&text) {
                        Ok(name) => {
                            self.shelf_view = Some(name);
                            let _ = self.shelf.save();
                        }
                        Err(e) => self.say(e),
                    }
                    self.folder_edit = None;
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.folder_edit = None;
                } else {
                    self.folder_edit = Some((None, text));
                }
            }
            if ui.small_button("+ folder").on_hover_text("New folder (or right-click a folder)").clicked() {
                out.new_folder = true;
            }
            // v0.840.0: the add / search / size / view / options controls share the folder row (one bar, not two)
            ui.separator();
            {
                if ui.button("Add files\u{2026}").clicked() {
                    self.spawn_pick(PickFor::Media);
                }
                ui.add(egui::TextEdit::singleline(&mut self.shelf_filter).hint_text("\u{1f50d} Search (name or type: pdf, mp4)").desired_width(130.0));
                if !self.shelf_filter.trim().is_empty() {
                    let n_songs = self.library.search(&self.shelf_filter).len();
                    if n_songs > 0 && ui.small_button(format!("\u{266a} {n_songs} song{} match", if n_songs == 1 { "" } else { "s" })).on_hover_text("Open the Songs list with this search").clicked() {
                        self.song_query = self.shelf_filter.clone();
                        self.open_at(Tab::Songs);
                    }
                    if self.shelf_filter.trim().chars().count() >= 3 {
                        let ctx2 = ui.ctx().clone();
                        let (hits, reading) = self.pdf_search(&ctx2);
                        if !hits.is_empty() {
                            ui.menu_button(format!("\u{1f4c4} {} PDF page{} match", hits.len(), if hits.len() == 1 { "" } else { "s" }), |ui| {
                                for (id, name, page, snippet) in hits.iter().take(15) {
                                    if ui.button(format!("{name}  p.{}  {snippet}", page + 1)).clicked() {
                                        if let Some(path) = self.shelf.get(*id).map(|e| e.path.clone()) {
                                            self.settings.resume.insert(path, *page as u64);
                                        }
                                        out.show = Some(*id);
                                        ui.close_menu();
                                    }
                                }
                            });
                        } else if reading {
                            ui.label(egui::RichText::new("reading PDFs\u{2026}").small().color(theme::MUTED));
                        }
                    }
                }
                let mut w = self.settings.thumb_w.clamp(80.0, 200.0);
                let r = ui.add_enabled(self.settings.view == "grid", egui::Slider::new(&mut w, 80.0..=200.0).show_value(false)).on_hover_text("Thumbnail size");
                if r.changed() {
                    self.settings.thumb_w = w;
                }
                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                    self.save_settings();
                }
                for (key, glyph, tip) in [("grid", "\u{25a6}", "Thumbnails"), ("list", "\u{2630}", "List")] {
                    if ui.selectable_label(self.settings.view == key, glyph).on_hover_text(tip).clicked() {
                        self.settings.view = key.into();
                        self.save_settings();
                    }
                }
                ui.menu_button("\u{2699} Options", |ui| {
                    ui.label("Sort");
                    for (k, n) in [("added", "Order added"), ("newest", "Newest first"), ("name", "Name A to Z"), ("recent", "Recently used")] {
                        if ui.selectable_label(self.settings.sort == k, n).clicked() {
                            self.settings.sort = k.into();
                            self.save_settings();
                        }
                    }
                    ui.separator();
                    ui.label("Picture size on screen 2");
                    for (m, n) in [(FitMode::Fit, "Fit (whole picture)"), (FitMode::Fill, "Fill (crop edges)"), (FitMode::Stretch, "Stretch")] {
                        if ui.selectable_label(fit_of(&self.settings.fit) == m, n).clicked() {
                            self.settings.fit = fit_key(m).into();
                            self.save_settings();
                        }
                    }
                    ui.separator();
                    let mut lp = self.settings.loop_video;
                    if ui.checkbox(&mut lp, "Loop video or sound").changed() {
                        self.set_loop(lp);
                    }
                    let mut mute = self.settings.mute;
                    if ui.checkbox(&mut mute, "Mute video sound").changed() {
                        self.set_mute(mute);
                    }
                    ui.separator();
                    ui.label("New files are:");
                    for (key, name) in [("copy", "always copied (recommended: safe if you move or delete the original)"), ("link", "only linked (saves disk space)")] {
                        if ui.selectable_label(self.settings.shelf_policy == key, name).clicked() {
                            self.settings.shelf_policy = key.into();
                            self.save_settings();
                        }
                    }
                    ui.separator();
                    ui.label("Add by path (also used by Re-link):");
                    let r = ui.add(egui::TextEdit::singleline(&mut self.path_input).desired_width(260.0));
                    if ui.button("Add this path").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                        self.add_typed_path(ctx, PickFor::Media);
                    }
                    ui.separator();
                    if ui.add_enabled(self.shelf.can_undo_remove() > 0, egui::Button::new("Undo last remove")).clicked() {
                        self.undo_remove_shelf();
                    }
                    if ui.button("Find duplicates").clicked() {
                        self.report_duplicates();
                    }
                    if ui.button("Re-link all missing (folder in the path box)").clicked() {
                        self.relink_all_from_path_box();
                    }
                    if ui.button("Sunday check").on_hover_text("Missing files, big videos, songs with a wrong order, duplicates, old backup").clicked() {
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
                        self.sunday_lines = self.sunday_check(now);
                    }
                    for l in &self.sunday_lines {
                        ui.label(egui::RichText::new(format!("\u{2022} {l}")).small());
                    }
                });
                if missing > 0 {
                    ui.colored_label(egui::Color32::from_rgb(220, 150, 40), format!("{missing} missing (right-click: Re-link or Remove)"));
                }
            }
        });

        if self.dock_open {
            // the message gets its own full-width line so long advice wraps instead of printing over the buttons
            if let Some((m, _)) = self.msg.clone() {
                ui.label(egui::RichText::new(m).small().color(theme::GOLD)); // labels wrap by default
            }
            self.transport_ui(ui, ctx, request);
            self.doc_ui(ui, request);
            // progress of files being added in the background
            if self.jobs_pending() > 0 {
                let (d, t) = self.jobs_progress();
                let frac = if t == 0 { 0.0 } else { d as f32 / t as f32 };
                ui.add(egui::ProgressBar::new(frac).desired_width(ui.available_width()).text(format!("Adding {} file(s)\u{2026} {:.0}%  (you can keep working)", self.jobs_pending(), frac * 100.0)));
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            if let Some((id, mut text, _)) = self.shelf_edit.clone() {
                ui.horizontal(|ui| {
                    ui.label("New name:");
                    let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(200.0));
                    r.request_focus();
                    let go = ui.button("OK").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if go {
                        let _ = self.shelf.rename(id, &text);
                        let _ = self.shelf.save();
                        self.apply_shelf_moves();
                        self.shelf_edit = None;
                    } else if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.shelf_edit = None;
                    } else {
                        self.shelf_edit = Some((id, text, false));
                    }
                });
            }
            ui.add_space(2.0);

            // ---------- body: only the rows you can see are drawn (and only their thumbnails are made)
            let by = match self.settings.sort.as_str() {
                "newest" => SortBy::Newest,
                "name" => SortBy::Name,
                "recent" => SortBy::Recent,
            "recent" => SortBy::Recent,
                _ => SortBy::Added,
            };
            let ids = self.shelf.list_sorted(self.shelf_view.as_deref(), &self.shelf_filter, by);
            view_ids = ids.clone();
            let grid = self.settings.view != "list";
            let w = self.settings.thumb_w.clamp(80.0, 200.0);
            let h = (w * 9.0 / 16.0).round();
            let gap = 8.0;
            let avail_w = ui.available_width().max(w);
            let cols = if grid { (((avail_w + gap) / (w + gap)).floor() as usize).max(1) } else { 1 };
            let nrows = (ids.len() + cols - 1) / cols;
            let row_h = if grid { h + 22.0 } else { 22.0 };
            // keyboard: arrows move the highlight, Enter or Space shows it (only while the mouse is over the bar and no box is being typed in)
            let over = ctx.input(|i| i.pointer.hover_pos()).map_or(false, |p| panel_rect.contains(p));
            let mut scroll_to: Option<f32> = None;
            if over && !ctx.wants_keyboard_input() && !ids.is_empty() {
                let cur = self.dock_cursor.and_then(|c| ids.iter().position(|x| *x == c));
                let last = ids.len() - 1;
                let mut ni = cur;
                let mut extend = false;
                ctx.input(|i| {
                    extend = i.modifiers.shift;
                    if i.modifiers.command {
                        return; // Cmd+arrows are not used here
                    }
                    if i.key_pressed(egui::Key::ArrowRight) {
                        ni = Some(cur.map_or(0, |c| (c + 1).min(last)));
                    }
                    if i.key_pressed(egui::Key::ArrowLeft) {
                        ni = Some(cur.map_or(0, |c| c.saturating_sub(1)));
                    }
                    if i.key_pressed(egui::Key::ArrowDown) {
                        ni = Some(cur.map_or(0, |c| (c + cols).min(last)));
                    }
                    if i.key_pressed(egui::Key::ArrowUp) {
                        ni = Some(cur.map_or(0, |c| c.saturating_sub(cols)));
                    }
                });
                if ni != cur {
                    self.dock_cursor = ni.map(|k| ids[k]);
                    if let Some(k) = ni {
                        // the highlight follows the arrow keys; Shift extends it
                        let nid = ids[k];
                        if extend {
                            let a = self.dock_anchor.or(cur.map(|c| ids[c])).unwrap_or(nid);
                            self.dock_anchor = Some(a);
                            self.dock_sel = crate::selection::range_between(&ids, Some(a), nid).into_iter().collect();
                        } else {
                            self.dock_sel.clear();
                            self.dock_sel.insert(nid);
                            self.dock_anchor = Some(nid);
                        }
                        let row = k / cols;
                        if row < self.dock_rows_seen.0 || row >= self.dock_rows_seen.1.saturating_sub(1).max(self.dock_rows_seen.0 + 1) {
                            scroll_to = Some(row as f32 * (row_h + ui.spacing().item_spacing.y));
                        }
                    }
                }
                // Esc: select nothing. Delete / Backspace: remove the selected files (asks once more).
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.dock_sel.clear();
                    self.dock_del_armed = None;
                }
                if !self.dock_sel.is_empty() && ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
                    out.remove_sel = true;
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space)) {
                    if let Some(c) = self.dock_cursor.filter(|c| ids.contains(c)) {
                        if !self.shelf.is_missing(c) {
                            out.show = Some(c);
                        }
                    }
                }
            }
            if ids.is_empty() {
                ui.add_space(14.0);
                ui.vertical_centered(|ui| {
                    if self.shelf.entries.is_empty() {
                        ui.label(egui::RichText::new("Drag pictures, videos or whole folders here").size(16.0).color(theme::MUTED));
                        ui.label(egui::RichText::new("They show up as thumbnails. Click one to put it on screen 2.").small().weak());
                    } else {
                        ui.label(egui::RichText::new("Nothing matches. Clear the search or pick another folder.").color(theme::MUTED));
                    }
                });
            }
            let body_rect = ui.available_rect_before_wrap();
            self.dock_drag_box(ui, ctx, body_rect, dragging);
            // right-click on the empty background: new folder
            let bg = ui.interact(body_rect, egui::Id::new("media-dock-bg"), egui::Sense::click());
            bg.context_menu(|ui| {
                if ui.button("New folder").clicked() {
                    out.new_folder = true;
                    ui.close_menu();
                }
            });
            // the mouse drag is for the selection box, so it must not also scroll the list (the wheel and bar still do)
            let mut sa = egui::ScrollArea::vertical().id_source("media-dock-scroll").auto_shrink([false, false]).drag_to_scroll(false);
            if let Some(off) = scroll_to {
                sa = sa.vertical_scroll_offset(off);
            }
            let shown = sa.show_rows(ui, row_h, nrows, |ui, range| {
                ui.spacing_mut().item_spacing.x = gap;
                for r in range.clone() {
                    ui.allocate_ui_with_layout(egui::vec2(avail_w, row_h), egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                        for c in 0..cols {
                            let i = r * cols + c;
                            if i >= ids.len() {
                                break;
                            }
                            if grid {
                                self.dock_card(ui, ctx, ids[i], i, w, h, &on_path, &folders, &mut out);
                            } else {
                                self.dock_row(ui, ids[i], i, &on_path, &folders, &mut out);
                            }
                        }
                    });
                }
                range
            });
            self.dock_rows_seen = (shown.inner.start, shown.inner.end);
            self.dock_rects = std::mem::take(&mut out.rects);
            // a small badge follows the pointer while several files are dragged
            if dragging && self.dock_sel.len() > 1 {
                egui::show_tooltip_at_pointer(ctx, ui.layer_id(), egui::Id::new("rf-drag-count"), |ui| {
                    ui.label(format!("{} files: drop on a folder to move them all", self.dock_sel.len()));
                });
            }
        } else {
            self.dock_rects.clear();
        }
        if hovering_files {
            ui.painter().rect_stroke(panel_rect.expand(4.0), 6.0, egui::Stroke::new(2.5, theme::GOLD));
        }
        self.folder_rects = folder_rects;
        if out.new_folder {
            self.dock_open = true;
            self.folder_edit = Some((None, "New folder".into()));
        }
        if let Some((f, delete)) = folder_action {
            if delete {
                self.shelf.delete_folder(&f);
                if self.shelf_view.as_deref() == Some(f.as_str()) {
                    self.shelf_view = None;
                }
                let _ = self.shelf.save();
            } else {
                self.folder_edit = Some((Some(f.clone()), f));
            }
        }
        if let Some(c) = out.clicked {
            self.dock_cursor = Some(c);
        }
        if let Some(id) = out.drag_began {
            if !self.dock_sel.contains(&id) {
                self.dock_sel.clear();
                self.dock_sel.insert(id);
                self.dock_anchor = Some(id);
            }
        }
        if let Some((id, cmd, shift)) = out.picked {
            let kind = crate::selection::click_kind(cmd, shift);
            self.dock_anchor = crate::selection::apply_click(&mut self.dock_sel, &view_ids, self.dock_anchor, id, kind);
            self.dock_cursor = Some(id);
        }
        if out.copy_sel {
            let note = self.dock_copy_selected(ctx);
            self.say(note);
        }
        if out.remove_sel {
            self.dock_remove_selected();
        }
        for (id, folder) in std::mem::take(&mut out.moves) {
            let _ = self.shelf.set_folder(id, &folder);
            let _ = self.shelf.save();
        }
        if out.rename.is_some() {
            self.shelf_edit = out.rename.take();
        }
        if let Some(id) = out.show {
            if let Some(c) = self.stage_entry(ctx, id) {
                *request = Some(c);
            }
        }
        if let Some(id) = out.relink {
            let np = clean_path(&self.path_input);
            if np.is_empty() {
                self.say("Open Options, paste the file's new path, then choose Re-link again.");
            } else {
                match self.shelf.relink(id, Path::new(&np), false) {
                    Ok(()) => {
                        let _ = self.shelf.save();
                        self.path_input.clear();
                        self.thumbs.remove(&id);
                        self.say("Re-linked.");
                    }
                    Err(e) => self.say(e.to_string()),
                }
            }
        }
        if let Some(id) = out.to_order {
            self.order_outbox.push(OrderAdd::Media(id));
        }
        if let Some(id) = out.duplicate.take() {
            match self.shelf.duplicate(id) {
                Ok(nid) => {
                    let _ = self.shelf.save();
                    let n = self.shelf.get(nid).map(|e| e.name.clone()).unwrap_or_default();
                    self.say(format!("Listed again as \"{n}\". Both use the same stored file."));
                }
                Err(e) => self.say(e.to_string()),
            }
        }
        if let Some(id) = out.reveal.take() {
            #[cfg(target_os = "macos")]
            if let Some(e) = self.shelf.get(id) {
                let _ = std::process::Command::new("open").arg("-R").arg(&e.path).spawn();
            }
            #[cfg(not(target_os = "macos"))]
            let _ = id;
        }
        if let Some((id, a)) = out.set_after.take() {
            if let Some(path) = self.shelf.get(id).map(|e| e.path.clone()) {
                self.set_after(&path, a.clone());
                self.say(format!("\"{}\": when it ends, {}.", file_name(&path), after_label(&a, &shown_name(&out.choices, &a)).to_lowercase()));
            }
        }
        if let Some(id) = out.remove {
            self.remove_from_shelf(id);
        }
    }

    fn tab_slides(&mut self, ui: &mut egui::Ui, request: &mut Option<StageContent>) {
        ui.label("Type announcements or words. A blank line starts a new slide. A first line ending in a colon (like \"Chorus:\") names the slide for you; the congregation does not see the name.");
        let before = self.settings.slides_text.clone();
        ui.add(egui::TextEdit::multiline(&mut self.settings.slides_text).desired_rows(7).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            if ui.button("Make slides").clicked() || before != self.settings.slides_text && false {
                let t = self.settings.slides_text.clone();
                self.set_slides(&t, "Typed slides");
                self.save_settings();
                let n = self.slides.len();
                self.say(format!("{n} slide{}.", if n == 1 { "" } else { "s" }));
            }
            ui.label("or paste a .txt / .pptx path in the Pictures tab and press Add.");
        });
        ui.separator();
        if self.slides.is_empty() {
            ui.label(egui::RichText::new("No slides yet.").weak());
            return;
        }
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&self.slides_source).color(theme::GOLD));
            if ui.button("< Previous").clicked() {
                if let Some(c) = self.step_slide(-1) {
                    *request = Some(c);
                }
            }
            if ui.button("Next >").clicked() {
                if let Some(c) = self.step_slide(1) {
                    *request = Some(c);
                }
            }
        });
        let mut show = None;
        for (i, s) in self.slides.iter().enumerate() {
            let first = s.text.lines().next().unwrap_or("");
            let name = if s.label.is_empty() { format!("{}.  {first}", i + 1) } else { format!("{}.  [{}]  {first}", i + 1, s.label) };
            let r = ui.selectable_label(self.slide_index == Some(i), name);
            if r.clicked() {
                show = Some(i);
            }
            if self.slide_index == Some(i) && self.scroll_slide_to != Some(i) {
                r.scroll_to_me(Some(egui::Align::Center)); // keep the slide that is on the screen in view
            }
        }
        self.scroll_slide_to = self.slide_index;
        if let Some(i) = show {
            if let Some(c) = self.stage_slide(i) {
                *request = Some(c);
            }
        }
    }

    /// Open this window on a tab (used by the toolbar's Songs button).
    pub fn open_at(&mut self, tab: Tab) {
        self.tab = tab;
        self.open = true;
    }

    /// How many songs are saved.
    pub fn song_count(&self) -> usize {
        self.library.songs().len()
    }

    fn tab_songs(&mut self, ui: &mut egui::Ui, request: &mut Option<StageContent>) {
        egui::CollapsingHeader::new("How songs work (read me)").default_open(self.library.songs().is_empty()).show(ui, |ui| {
            ui.label("1. ADD: press \"Paste lyrics\" (paste the words, give a title) or \"Import .txt / .pptx\" or \"New song\" and type.\n2. SEE: every saved song is listed below. Click a title to read or edit its words; use the Search box to find one.\n3. DISPLAY: press \"Show on stage\" next to a song. Its first slide goes up on the congregation screen.\n4. MOVE ON: press Next / Previous (or say \"next slide\"). The slides are also on the Slides tab.\n5. PLAN: \"+ Service order\" puts the song in your service order.");
        });
        ui.label(egui::RichText::new("RhemaFlow includes no song words. Type or import your own. Many songs are protected by copyright: make sure your church holds the right licence (in the UK this is usually CCLI).").weak());
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.add(egui::TextEdit::singleline(&mut self.song_query).desired_width(220.0).hint_text("title, author or words"));
            if ui.button("New song").clicked() {
                self.new_song();
            }
            if ui.selectable_label(self.paste_open, "Paste lyrics").on_hover_text("Add a song by pasting its words").clicked() {
                self.paste_open = !self.paste_open;
            }
        });
        if self.paste_open {
            ui.group(|ui| self.paste_form(ui));
        }
        let ctx = ui.ctx().clone();
        self.add_row(ui, &ctx, PickFor::Song, "Import .txt / .pptx as a song...");
        if let Some(d) = crate::fsutil::data_dir() {
            if ui.small_button("Open the folder where songs are saved").clicked() {
                crate::export::open_folder(&d);
            }
        }
        let mut hits: Vec<(u64, String, usize)> = self.library.search(&self.song_query).iter().map(|s| (s.id, s.title.clone(), s.sections.len())).collect();
        hits.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        ui.separator();
        let total = self.library.songs().len();
        if total > 0 {
            ui.label(egui::RichText::new(if self.song_query.trim().is_empty() { format!("{total} song{} (A to Z)", if total == 1 { "" } else { "s" }) } else { format!("{} of {total} songs match", hits.len()) }).small().color(theme::MUTED));
        }
        let recent: Vec<(u64, String)> = self.shown_songs.iter().rev().take(5).filter_map(|id| self.library.get(*id).map(|s| (*id, s.title.clone()))).collect();
        let mut recent_show = None;
        if !recent.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Recently shown:").small().color(theme::MUTED));
                for (id, title) in &recent {
                    if ui.small_button(title).on_hover_text("Show this song again from its first slide").clicked() {
                        recent_show = Some(*id);
                    }
                }
            });
        }
        if let Some(id) = recent_show {
            if let Some(c) = self.stage_song_slide(id, 0) {
                *request = Some(c);
            }
            self.tab = Tab::Slides;
        }
        if hits.is_empty() {
            ui.label(egui::RichText::new(if self.library.songs().is_empty() { "No songs yet. Press \"Paste lyrics\" above to add your first one." } else { "No song matches." }).weak());
        }
        let (mut use_id, mut sel_id, mut show_id, mut dup_id) = (None, None, None, None);
        for (id, title, n) in &hits {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.song_sel == Some(*id), format!("{title}  ({n} parts)")).clicked() {
                    sel_id = Some(*id);
                }
                if ui.button("\u{270f} Edit words").on_hover_text("Change this song's title, words and order").clicked() {
                    sel_id = Some(*id);
                    self.edit_focus = true;
                }
                if ui.button("Show on stage").on_hover_text("Make this song's slides and put the first one on the screen (then Next / Previous, or say \"next slide\")").clicked() {
                    show_id = Some(*id);
                }
                if ui.button("Copy song").on_hover_text("Make a copy to adapt (a shorter version, another key)").clicked() {
                    dup_id = Some(*id);
                }
                if ui.button("Make slides").on_hover_text("Make this song's slides without showing anything yet (see the Slides tab)").clicked() {
                    use_id = Some(*id);
                }
                if ui.button("+ Service order").on_hover_text("Add this song to the service order").clicked() {
                    self.order_outbox.push(OrderAdd::Song(*id));
                }
            });
        }
        if let Some(id) = sel_id {
            self.select_song(id);
        }
        if let Some(id) = use_id {
            self.use_song(id);
            self.tab = Tab::Slides;
        }
        if let Some(id) = show_id {
            if let Some(c) = self.stage_song_slide(id, 0) {
                *request = Some(c); // also adds the CCLI line and the usage-report entry
            }
            self.tab = Tab::Slides;
        }
        if let Some(id) = dup_id {
            self.duplicate_song(id);
        }
        if let Some(id) = self.song_sel {
            ui.separator();
            let head = ui.heading(format!("\u{270f} Editing: {}", self.song_edit.title));
            if self.edit_focus {
                head.scroll_to_me(Some(egui::Align::TOP));
                self.edit_focus = false;
            }
            ui.horizontal(|ui| {
                ui.label("Title");
                ui.add(egui::TextEdit::singleline(&mut self.song_edit.title).desired_width(200.0));
                ui.label("Author");
                ui.add(egui::TextEdit::singleline(&mut self.song_edit.author).desired_width(140.0));
            });
            ui.horizontal(|ui| {
                ui.label("CCLI song #");
                ui.add(egui::TextEdit::singleline(&mut self.song_edit.ccli_song).desired_width(90.0));
                ui.label("\u{a9}");
                ui.add(egui::TextEdit::singleline(&mut self.song_edit.copyright).desired_width(200.0).hint_text("2015 Integrity Music"));
                ui.label("Church CCLI licence #");
                if ui.add(egui::TextEdit::singleline(&mut self.settings.ccli_licence).desired_width(90.0)).changed() {
                    self.save_settings();
                }
            });
            ui.label("Words (blank line between parts; name a part with a first line like \"Chorus:\")");
            ui.add(egui::TextEdit::multiline(&mut self.song_edit.lyrics).desired_rows(8).desired_width(f32::INFINITY).hint_text("Type or paste the words here (Cmd+V works in the box too)"));
            // v0.845.0: quick part buttons, Tidy, and a live look at the slides
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Add part").small().color(theme::MUTED));
                for kind in ["Verse", "Chorus", "Pre-chorus", "Bridge", "Tag"] {
                    if ui.small_button(format!("+ {kind}")).on_hover_text("Adds the part's name line at the end, ready to type under (Verse: Shift+Cmd+P)").clicked() {
                        let head = next_part_header(&self.song_edit.lyrics, kind);
                        self.song_edit.lyrics = append_part(&self.song_edit.lyrics, &head);
                    }
                }
                if ui.small_button("Strip chords").on_hover_text("For words pasted from a web page or chord sheet: drops chord lines and web tags, turns [Verse 1] into Verse 1: (Shift+Cmd+L)").clicked() {
                    self.song_edit.lyrics = clean_pasted_lyrics(&self.song_edit.lyrics);
                }
                let heads = part_headers(&self.song_edit.lyrics);
                if !heads.is_empty() {
                    ui.menu_button("Repeat a part", |ui| {
                        for h in heads {
                            if ui.button(&h).clicked() {
                                self.song_edit.arrangement = repeat_in_order(&self.song_edit.lyrics, &self.song_edit.arrangement, &h);
                                ui.close_menu();
                            }
                        }
                    });
                }
                if ui.small_button("Find repeats").on_hover_text("For a song pasted with every verse and chorus written out: names the parts, keeps one copy of a repeated chorus and fills in \"Order sung\" (Shift+Cmd+R)").clicked() {
                    let (words, order) = detect_repeats(&self.song_edit.lyrics);
                    if !words.is_empty() {
                        self.song_edit.lyrics = words;
                        self.song_edit.arrangement = order;
                    }
                }
                if ui.small_button("Wrap long lines").on_hover_text("Splits any line over 42 letters at a space, so it fits the screen (Shift+Cmd+W)").clicked() {
                    self.song_edit.lyrics = wrap_long_lines(&self.song_edit.lyrics, 42);
                }
                if ui.small_button("Tidy").on_hover_text("Removes trailing spaces and extra blank lines (Shift+Cmd+T)").clicked() {
                    self.song_edit.lyrics = tidy_lyrics(&self.song_edit.lyrics);
                }
            });
            // v0.850.0: the order as chips: click one to take it out, or add any part with "+ part"
            {
                let chip_heads = part_headers(&self.song_edit.lyrics);
                let mut order: Vec<String> = self.song_edit.arrangement.split(',').map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|s| !s.is_empty()).collect();
                if !chip_heads.is_empty() || !order.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new("Order").small().color(theme::MUTED));
                        let mut remove: Option<usize> = None;
                        for (i, o) in order.iter().enumerate() {
                            if ui.small_button(format!("{o} \u{d7}")).on_hover_text("Click to take this out of the order").clicked() {
                                remove = Some(i);
                            }
                        }
                        if let Some(i) = remove {
                            order.remove(i);
                            self.song_edit.arrangement = order.join(", ");
                        }
                        ui.menu_button("+ part", |ui| {
                            for h in &chip_heads {
                                if ui.button(h).clicked() {
                                    self.song_edit.arrangement = repeat_in_order(&self.song_edit.lyrics, &self.song_edit.arrangement, h);
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                }
            }
            egui::CollapsingHeader::new(egui::RichText::new("Slides preview").small().color(theme::MUTED)).id_source("song-slides-preview").show(ui, |ui| {
                let mut pv = Song::from_text(self.song_edit.title.trim(), &self.song_edit.lyrics);
                pv.set_arrangement_text(&self.song_edit.arrangement);
                for (i, sl) in pv.slides(self.settings.lines_per_slide).0.iter().enumerate() {
                    ui.label(egui::RichText::new(format!("{}  {}", i + 1, sl.label)).small().color(theme::GOLD));
                    // drawn like the stage: light words centred on a dark panel
                    egui::Frame::none().fill(egui::Color32::from_rgb(16, 28, 60)).rounding(4.0).inner_margin(egui::Margin::symmetric(10.0, 6.0)).show(ui, |ui| {
                        let w = ui.available_width().min(380.0);
                        ui.set_width(w);
                        ui.set_min_height(w * 9.0 / 16.0 - 12.0); // a 16:9 screen
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(sl.text.clone()).size(15.0).color(egui::Color32::WHITE));
                        });
                    });
                }
            });
            {
                let mut preview = Song::from_text(self.song_edit.title.trim(), &self.song_edit.lyrics);
                preview.set_arrangement_text(&self.song_edit.arrangement);
                let (made, unknown_parts) = preview.slides(self.settings.lines_per_slide);
                let n_slides = made.len();
                let long = self.song_edit.lyrics.lines().filter(|l| l.chars().count() > 60).count();
                let dirty = Self::song_edit_key(&self.song_edit) != self.song_edit_base;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{} part{}, {n_slides} slide{}", preview.sections.len(), if preview.sections.len() == 1 { "" } else { "s" }, if n_slides == 1 { "" } else { "s" })).small().color(theme::MUTED));
                    if !unknown_parts.is_empty() {
                        ui.label(egui::RichText::new(format!("\u{26a0} \"Order sung\" names a part that does not exist: {}", unknown_parts.join(", "))).small().color(theme::GOLD));
                    }
                    if long > 0 {
                        ui.label(egui::RichText::new(format!("{long} very long line{} (may be small on screen)", if long == 1 { "" } else { "s" })).small().color(theme::GOLD));
                    }
                    if dirty {
                        ui.label(egui::RichText::new("\u{25cf} unsaved changes (Cmd+S saves)").small().color(theme::GOLD));
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.label("Order sung");
                ui.add(egui::TextEdit::singleline(&mut self.song_edit.arrangement).desired_width(300.0).hint_text("Verse 1, Chorus, Verse 2, Chorus"));
            });
            ui.horizontal(|ui| {
                ui.label("Lines per slide (0 = never split)");
                let mut n = self.settings.lines_per_slide as u32;
                if ui.add(egui::DragValue::new(&mut n).clamp_range(0..=12)).changed() {
                    self.settings.lines_per_slide = n as usize;
                    self.save_settings();
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Save song").clicked() {
                    self.save_song_edit();
                }
                if ui.button("Copy words").on_hover_text("Copy the words to the clipboard").clicked() {
                    ui.ctx().copy_text(self.song_edit.lyrics.clone());
                }
                if ui.button("Paste into words").on_hover_text("Put the clipboard text into the Words box: fills an empty box, otherwise adds it at the end as a new part").clicked() {
                    match clipboard_text() {
                        Some(t) => {
                            if self.song_edit.lyrics.trim().is_empty() {
                                self.song_edit.lyrics = t;
                            } else {
                                self.song_edit.lyrics.push_str("\n\n");
                                self.song_edit.lyrics.push_str(&t);
                            }
                            self.say("Pasted into the words. Check the parts and the order, then press Save song.");
                        }
                        None => self.say("The clipboard is empty (or could not be read). Copy the words first, then try again."),
                    }
                }
                let armed = self.confirm_delete == Some(id);
                if ui.button(if armed { "Really delete? Click again" } else { "Delete song" }).clicked() {
                    if armed {
                        self.confirm_delete = None;
                        self.delete_song(id);
                    } else {
                        self.confirm_delete = Some(id);
                    }
                }
            });
        }
    }

    fn tab_background(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label("A picture or a looping video behind slides, songs and timers (never behind scripture). Videos here always loop and are silent.");
        ui.add_space(4.0);
        match &self.bg {
            Some(b) => {
                ui.label(egui::RichText::new(format!("Current background: {}", b.name)).color(theme::GOLD));
            }
            None => {
                ui.label(egui::RichText::new("No background (plain stage colour).").weak());
            }
        }
        self.add_row(ui, ctx, PickFor::Background, "Choose background...");
        if self.bg.is_some() && ui.button("Remove background").clicked() {
            self.clear_background();
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Darken");
            let mut d = self.settings.dim_pct;
            if ui.add(egui::Slider::new(&mut d, 0..=MAX_DIM_PCT).suffix(" %")).changed() {
                self.settings.dim_pct = d;
                self.save_settings();
            }
        });
        ui.label(egui::RichText::new("Darkening keeps the words readable on a bright picture. The most it goes is 85 %.").weak());
    }

    fn tab_timers(&mut self, ui: &mut egui::Ui, request: &mut Option<StageContent>) {
        ui.horizontal(|ui| {
            for (c, n) in [(TimerChoice::CountDown, "Countdown"), (TimerChoice::UntilClock, "Count down to a time"), (TimerChoice::StopWatch, "Stopwatch"), (TimerChoice::Clock, "Clock")] {
                ui.radio_value(&mut self.timer_ui.choice, c, n);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Title shown above the numbers");
            ui.add(egui::TextEdit::singleline(&mut self.timer_ui.title).desired_width(220.0));
        });
        match self.timer_ui.choice {
            TimerChoice::CountDown => {
                ui.horizontal(|ui| {
                    ui.label("Minutes");
                    ui.add(egui::DragValue::new(&mut self.timer_ui.minutes).clamp_range(0..=600));
                });
            }
            TimerChoice::UntilClock => {
                ui.horizontal(|ui| {
                    ui.label("Until");
                    ui.add(egui::TextEdit::singleline(&mut self.timer_ui.until_text).desired_width(90.0));
                    match parse_time_of_day(&self.timer_ui.until_text) {
                        Some(s) => ui.label(format!("= {}", format_time_of_day(s, false))),
                        None => ui.label(egui::RichText::new("type a time like 10:30 or 9:05pm").color(theme::GOLD)),
                    };
                });
            }
            TimerChoice::Clock => {
                let mut h = self.settings.twelve_hour;
                if ui.checkbox(&mut h, "12-hour clock (AM / PM)").changed() {
                    self.settings.twelve_hour = h;
                    self.save_settings();
                }
            }
            TimerChoice::StopWatch => {}
        }
        ui.add_space(6.0);
        let phase = self.timer_phase();
        let phase_text = match phase {
            Phase::Idle => "ready",
            Phase::Running => "running",
            Phase::Warning => "less than a minute left",
            Phase::Paused => "paused",
            Phase::Done => "finished",
        };
        ui.label(egui::RichText::new(self.timer_display()).size(40.0).monospace());
        ui.label(egui::RichText::new(phase_text).weak());
        ui.horizontal(|ui| {
            if ui.button("Start").clicked() {
                self.timer_start();
            }
            if ui.button("Pause").clicked() {
                self.timer_pause();
            }
            if ui.button("Resume").clicked() {
                self.timer_resume();
            }
            if ui.button("Reset").clicked() {
                self.timer_reset();
            }
            if ui.button("Show on stage").clicked() {
                *request = self.stage_timer();
            }
        });
        ui.label(egui::RichText::new("The timer keeps running when you show a scripture; press Show on stage again to bring it back. The congregation never sees a colour change or a flash.").weak());
    }
}


/// A small preview picture for a video: the first frame that is not (nearly) black, looking at most
/// 40 frames in. Files over 150 MB get no preview (reading them would freeze the window).
/// What kind of converted copy could fix this file, going by its name (None = nothing to try).
fn repair_target(path: &str) -> Option<rf_media::convert::Target> {
    use rf_media::convert::Target;
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())?;
    match ext.as_str() {
        "mov" | "mkv" | "avi" | "webm" | "wmv" | "flv" | "mpg" | "mpeg" | "hevc" | "h265" | "ts" | "mp4" | "m4v" => Some(Target::Video),
        "ogg" | "oga" | "opus" | "wma" | "mid" | "midi" | "amr" => Some(Target::Audio),
        _ => None,
    }
}

/// True when adding this file should go through the background helper (so a repair can run).
fn wants_repair(path: &str) -> bool {
    repair_target(path).is_some() && rf_media::convert::find_ffmpeg().is_some()
}

/// Add a file to the shelf, checking now (not on Sunday) that it will really play. A video the player
/// cannot open, or a sound/video type we do not play, is converted with ffmpeg when it is installed;
/// otherwise the person is told exactly what to do. Runs on the helper thread.
fn add_with_repair(src: &Path, policy: ShelfPolicy, dir: &Path, done: &AtomicU64, total: u64) -> Result<Prepared, LibError> {
    let s = src.to_string_lossy().to_string();
    let problem: Option<(String, rf_media::convert::Target)> = match classify(&s) {
        Classified::Ok(MediaKind::Video) => rf_media::VideoReader::open(src).err().map(|e| (e, rf_media::convert::Target::Video)),
        Classified::Unsupported(m) => repair_target(&s).map(|t| (m, t)),
        _ => None,
    };
    let Some((why, target)) = problem else {
        return prepare(src, policy, dir, Some(done));
    };
    if rf_media::convert::find_ffmpeg().is_none() {
        return Err(LibError::NotMedia(format!("{why} {}", rf_media::convert::INSTALL_HINT)));
    }
    let out = rf_media::convert::convert(src, &dir.join("converted"), target, &mut |p| done.store((p as f64 * total as f64) as u64, Ordering::Relaxed)).map_err(LibError::NotMedia)?;
    // the converted copy lives in our own folder already, so it is linked, not copied a second time
    let mut prep = prepare(&out, ShelfPolicy::AlwaysLink, dir, None)?;
    prep.name = src.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or(prep.name);
    Ok(prep)
}

fn shrink_rgba(img: &rf_media::Rgba, max_side: u32) -> rf_media::Rgba {
    rf_media::thumb::shrink(img, max_side)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_documents_zoom_is_remembered_only_when_it_is_not_the_plain_page() {
        assert_eq!(zoom_to_remember(None), None);
        assert_eq!(zoom_to_remember(Some(1.0)), None);
        assert_eq!(zoom_to_remember(Some(1.004)), None);
        assert_eq!(zoom_to_remember(Some(1.25)), Some(1.25));
        assert_eq!(zoom_to_remember(Some(99.0)), Some(5.0));
        assert_eq!(zoom_to_remember(Some(0.01)), Some(0.5));
        assert_eq!(zoom_to_remember(Some(f32::NAN)), None);
        // old settings files have no doc_zoom: they still load
        let s: PresentSettings = serde_json::from_str(r#"{"fit":"fill"}"#).unwrap();
        assert!(s.doc_zoom.is_empty());
    }

    const CLIP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../rf-media/tests/fixtures/clip_bframes_audio.mp4");
    const PPTX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../rf-service/tests/fixtures/sample.pptx");

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rf-present-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn png(dir: &Path, name: &str, w: u32, h: u32) -> String {
        let img = image_png(w, h);
        let p = dir.join(name);
        std::fs::write(&p, img).unwrap();
        p.to_string_lossy().to_string()
    }

    /// A small PNG made by hand (zlib "stored" blocks) so this test needs no extra crates.
    fn image_png(w: u32, h: u32) -> Vec<u8> {
        fn crc(data: &[u8]) -> u32 {
            let mut c = !0u32;
            for &b in data {
                c ^= b as u32;
                for _ in 0..8 {
                    c = if c & 1 == 1 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
                }
            }
            !c
        }
        fn adler(data: &[u8]) -> u32 {
            let (mut a, mut b) = (1u32, 0u32);
            for &x in data {
                a = (a + x as u32) % 65521;
                b = (b + a) % 65521;
            }
            (b << 16) | a
        }
        fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let mut body = kind.to_vec();
            body.extend_from_slice(data);
            out.extend_from_slice(&body);
            out.extend_from_slice(&crc(&body).to_be_bytes());
        }
        let mut raw = Vec::new();
        for _ in 0..h {
            raw.push(0u8);
            for x in 0..w {
                raw.extend_from_slice(if x < w / 2 { &[200, 30, 30] } else { &[30, 30, 200] });
            }
        }
        let mut z = vec![0x78, 0x01];
        let mut chunks = raw.chunks(65535).peekable();
        while let Some(c) = chunks.next() {
            z.push(if chunks.peek().is_none() { 1 } else { 0 });
            z.extend_from_slice(&(c.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
            z.extend_from_slice(c);
        }
        z.extend_from_slice(&adler(&raw).to_be_bytes());
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }

    fn ctx() -> egui::Context {
        egui::Context::default()
    }

    fn stage(kind: StageKind) -> StageContent {
        StageContent { kind, ..Default::default() }
    }

    // ---- slides (S1.4, S1.7)

    #[test]
    fn s1_4_next_and_previous_step_through_slides_and_say_so_at_the_ends() {
        let mut p = Present::new(tmpdir("slides"));
        assert!(p.step_slide(1).is_none(), "no slides: nothing happens");
        assert!(p.msg.as_ref().unwrap().0.contains("no slides"));
        p.set_slides("One\n\nTwo\n\nThree", "Typed slides");
        assert!(p.step_slide(-1).is_none());
        assert_eq!(p.msg.as_ref().unwrap().0, "First slide");
        assert_eq!(p.step_slide(1).unwrap().text, "One", "first Next goes to slide 1");
        assert_eq!(p.step_slide(1).unwrap().text, "Two");
        assert_eq!(p.step_slide(1).unwrap().text, "Three");
        assert!(p.step_slide(1).is_none());
        assert_eq!(p.msg.as_ref().unwrap().0, "Last slide");
        assert_eq!(p.slide_index, Some(2), "still on the last slide");
        assert_eq!(p.step_slide(-1).unwrap().text, "Two");
        let c = p.stage_slide(0).unwrap();
        assert_eq!((c.kind, c.text.as_str()), (StageKind::Slide, "One"));
        assert!(p.step_slide(-1).is_none());
        assert_eq!(p.msg.as_ref().unwrap().0, "First slide");
    }

    #[test]
    fn s1_7_a_pptx_dropped_on_the_window_becomes_slides() {
        let mut p = Present::new(tmpdir("pptx"));
        p.route_path(&ctx(), Path::new(PPTX), PickFor::Media);
        assert_eq!(p.slides.len(), 12);
        assert_eq!(p.tab, Tab::Slides);
        assert!(p.msg.as_ref().unwrap().0.contains("12 slides"));
        assert!(p.slides_source.contains("sample.pptx"));
    }

    #[test]
    fn s1_7_a_text_file_becomes_slides_and_a_missing_file_says_so() {
        let d = tmpdir("txt");
        let f = d.join("notices.txt");
        std::fs::write(&f, "\u{feff}Welcome\r\n\r\nTea after church").unwrap();
        let mut p = Present::new(d.clone());
        p.route_path(&ctx(), &f, PickFor::Media);
        assert_eq!(p.slides.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), vec!["Welcome", "Tea after church"]);
        p.route_path(&ctx(), &d.join("nope.txt"), PickFor::Media);
        assert!(p.msg.as_ref().unwrap().0.contains("Cannot find"));
        p.route_path(&ctx(), Path::new(PPTX), PickFor::Media);
        let bad = d.join("bad.pptx");
        std::fs::write(&bad, b"not a zip").unwrap();
        p.route_path(&ctx(), &bad, PickFor::Media);
        assert!(p.msg.as_ref().unwrap().0.contains("PowerPoint"));
        assert_eq!(p.slides.len(), 12, "a bad file leaves the slides as they were");
    }

    // ---- songs (S2)

    #[test]
    fn s2_a_song_becomes_slides_in_the_order_sung_and_unknown_names_are_reported() {
        let mut p = Present::new(tmpdir("song"));
        p.new_song();
        let id = p.song_sel.unwrap();
        p.song_edit.title = "Grace".into();
        p.song_edit.lyrics = "Verse 1:\nline a\nline b\n\nChorus:\nsing\nit".into();
        p.song_edit.arrangement = "Verse 1, Chorus, Chorus, Bridge".into();
        p.save_song_edit();
        p.use_song(id);
        assert_eq!(p.slides.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), vec!["Verse 1", "Chorus", "Chorus"]);
        assert!(p.msg.as_ref().unwrap().0.contains("Bridge"), "{:?}", p.msg);
        assert_eq!(p.slides_source, "Song: Grace");
        // lines per slide splits long parts
        p.song_edit.lyrics = "Verse 1:\n1\n2\n3\n4\n5".into();
        p.song_edit.arrangement = String::new();
        p.save_song_edit();
        p.settings.lines_per_slide = 2;
        p.use_song(id);
        assert_eq!(p.slides.len(), 3);
    }

    #[test]
    fn s2_6_songs_survive_a_restart_and_deleting_works() {
        let d = tmpdir("songsave");
        let mut p = Present::new(d.clone());
        p.new_song();
        let id = p.song_sel.unwrap();
        p.song_edit.title = "Kept".into();
        p.song_edit.lyrics = "Words here".into();
        p.save_song_edit();
        let p2 = Present::new(d.clone());
        assert_eq!(p2.library.get(id).unwrap().title, "Kept");
        let mut p3 = Present::new(d);
        p3.delete_song(id);
        assert!(Present::new(p3.dir.clone()).library.songs().is_empty());
    }

    #[test]
    fn s2_6_a_song_with_no_title_is_not_saved_and_a_damaged_library_is_reported() {
        let d = tmpdir("songdamaged");
        std::fs::write(d.join("songs.json"), b"{{{").unwrap();
        let p = Present::new(d.clone());
        assert!(p.msg.as_ref().unwrap().0.contains("damaged"));
        assert!(d.join("songs.json.damaged").exists());
        let mut p = Present::new(tmpdir("songtitle"));
        p.new_song();
        p.song_edit.title = "   ".into();
        p.save_song_edit();
        assert!(p.msg.as_ref().unwrap().0.contains("title"));
    }

    #[test]
    fn s2_importing_a_text_file_as_a_song_names_it_after_the_file() {
        let d = tmpdir("songimport");
        let f = d.join("Great Is Thy Faithfulness.txt");
        std::fs::write(&f, "Verse 1:\nGreat is thy faithfulness\n\nChorus:\nMorning by morning").unwrap();
        let mut p = Present::new(d);
        p.route_path(&ctx(), &f, PickFor::Song);
        let id = p.song_sel.unwrap();
        assert_eq!(p.library.get(id).unwrap().title, "Great Is Thy Faithfulness");
        assert_eq!(p.library.get(id).unwrap().sections.len(), 2);
        assert_eq!(p.tab, Tab::Songs);
    }

    // ---- pictures (S3.1, S3.2, S3.11, S0.5)

    #[test]
    fn s3_a_picture_goes_on_the_stage_and_leaves_when_something_else_comes() {
        let d = tmpdir("pic");
        let f = png(&d, "cross.png", 64, 32);
        let mut p = Present::new(d);
        let c = ctx();
        p.route_path(&c, Path::new(&f), PickFor::Media);
        assert_eq!(p.settings.media.len(), 1, "the shelf keeps its own copy, so the list holds that copy");
        assert_eq!(std::fs::read(&p.settings.media[0]).unwrap(), std::fs::read(&f).unwrap());
        let content = p.stage_media(&c, 0).expect("shows");
        assert_eq!((content.kind.clone(), content.label.as_str()), (StageKind::Media, "cross"), "the stage shows the real name, not the shelf's internal file name");
        let art = p.art(&StageKind::Media);
        let m = art.media.expect("a texture is ready");
        assert_eq!(m.size, [64.0, 32.0]);
        assert!(p.art(&StageKind::Scripture).media.is_none(), "only the Media kind gets the picture");
        // S0.3: a scripture replaces it, and the picture is freed
        let mut st = stage(StageKind::Scripture);
        p.tick_at(&c, &mut st, 0, 0);
        assert!(p.loaded.is_none());
        // adding the same file twice lists it twice; persists
        p.route_path(&c, Path::new(&f), PickFor::Media);
        let again = Present::new(p.dir.clone());
        assert_eq!(again.shelf.entries.len(), 2, "the shelf is remembered");
        assert!(again.shelf.path_to_show(1).is_some());
    }

    #[test]
    fn s0_5_bad_files_leave_the_stage_alone_and_say_why() {
        let d = tmpdir("bad");
        let c = ctx();
        let mut p = Present::new(d.clone());
        let broken = d.join("broken.png");
        std::fs::write(&broken, b"not a png").unwrap();
        p.settings.media = vec![broken.to_string_lossy().to_string()];
        assert!(p.stage_media(&c, 0).is_none());
        assert!(p.msg.as_ref().unwrap().0.contains("could not be opened"), "{:?}", p.msg);
        assert!(p.loaded.is_none());
        // S3.11: a file that went missing
        p.settings.media = vec![d.join("gone.png").to_string_lossy().to_string()];
        assert!(p.stage_media(&c, 0).is_none());
        assert!(p.msg.as_ref().unwrap().0.contains("missing"));
        // S3.1: formats we cannot play are refused with advice
        let mov = d.join("film.mov");
        std::fs::write(&mov, b"x").unwrap();
        p.route_path(&c, &mov, PickFor::Media);
        let m = p.msg.as_ref().unwrap().0.clone();
        assert!(m.contains("HandBrake") || m.contains("needs converting"), "{m}");
        assert!(p.settings.media.len() == 1, "the .mov was not added");
        let odd = d.join("notes.docx");
        std::fs::write(&odd, b"x").unwrap();
        p.route_path(&c, &odd, PickFor::Media);
        assert!(p.msg.as_ref().unwrap().0.contains("not a picture"));
        // out of range index
        assert!(p.stage_media(&c, 9).is_none());
    }

    #[test]
    fn typed_and_dragged_paths_are_cleaned() {
        assert_eq!(clean_path("  \"/Users/me/My Pics/a.png\" "), "/Users/me/My Pics/a.png");
        assert_eq!(clean_path("/Users/me/My\\ Pics/a.png"), "/Users/me/My Pics/a.png");
        assert_eq!(clean_path("'/x/y.mp4'"), "/x/y.mp4");
    }

    #[test]
    fn s3_11_removing_the_picture_that_is_on_the_stage_frees_it() {
        let d = tmpdir("remove");
        let f = png(&d, "a.png", 8, 8);
        let mut p = Present::new(d);
        let c = ctx();
        p.route_path(&c, Path::new(&f), PickFor::Media);
        p.stage_media(&c, 0).unwrap();
        p.remove_media(0);
        assert!(p.loaded.is_none() && p.settings.media.is_empty());
    }

    // ---- video (S3.4 - S3.6, S0.2, S0.3)

    #[test]
    fn s3_video_plays_pauses_restarts_and_stops_when_it_leaves_the_stage() {
        let c = ctx();
        let mut p = Present::new(tmpdir("video"));
        p.settings.media = vec![CLIP.to_string()];
        p.settings.mute = true;
        assert!(!p.media_play(), "no video on the stage yet");
        let content = p.stage_media(&c, 0).expect("the real clip opens");
        assert_eq!(content.kind, StageKind::Media);
        assert!(p.has_video());
        let mut st = content;
        let deadline = Instant::now() + Duration::from_secs(5);
        while p.art(&StageKind::Media).media.is_none() {
            assert!(Instant::now() < deadline, "no frame arrived");
            p.tick_at(&c, &mut st, 0, 0);
            std::thread::sleep(Duration::from_millis(20));
        }
        let art = p.art(&StageKind::Media);
        assert_eq!(art.media.unwrap().size, [128.0, 72.0]);
        // it is playing: the picture changes
        let first = art.media.unwrap().texture;
        std::thread::sleep(Duration::from_millis(400));
        p.tick_at(&c, &mut st, 0, 0);
        assert_eq!(p.art(&StageKind::Media).media.unwrap().texture, first, "the same texture is updated in place");
        assert!(p.media_pause());
        if let Some(Loaded::Video { player, .. }) = &p.loaded {
            assert!(!player.state().playing);
        }
        assert!(p.media_restart());
        if let Some(Loaded::Video { player, .. }) = &p.loaded {
            assert!(player.state().playing);
        }
        assert!(!p.video_sound_playing(), "muted: no sound, so the microphone stays on");
        // clear / replace: the video is freed
        let mut cleared = stage(StageKind::Blank);
        p.tick_at(&c, &mut cleared, 0, 0);
        assert!(!p.has_video());
        assert!(!p.media_play());
    }

    #[test]
    fn play_bar_text_shows_time_played_time_left_and_when_it_ends() {
        assert_eq!(fmt_clock(0), "0:00");
        assert_eq!(fmt_clock(83_999), "1:23");
        assert_eq!(fmt_clock(3_725_000), "1:02:05");
        assert_eq!(left_text(60_000, 245_000), "-3:05 left");
        assert_eq!(left_text(0, 0), "");
        // 10:00:00 now, 3:05 left -> ends 10:03:05; wraps past midnight
        assert_eq!(ends_text(60_000, 245_000, 36_000_000), "ends 10:03:05");
        assert_eq!(ends_text(0, 120_000, 86_340_000), "ends 00:01:00");
        let t = Transport { pos_ms: 83_000, dur_ms: 245_000, playing: true, ended: false, looping: false, is_video: false, can_jump: true, seeking: false };
        let line = transport_line(&t, 36_000_000);
        assert!(line.contains("playing") && line.contains("1:23 / 4:05") && line.contains("-2:42 left") && line.contains("ends 10:02:42"), "{line}");
        let paused = Transport { playing: false, ..t.clone() };
        let line = transport_line(&paused, 36_000_000);
        assert!(line.contains("paused") && line.contains("-2:42 left") && !line.contains("ends"), "{line}");
        let looping = Transport { looping: true, ..t.clone() };
        assert!(transport_line(&looping, 0).contains("looping"));
        let unknown = Transport { dur_ms: 0, ..t.clone() };
        assert_eq!(transport_line(&unknown, 0), "playing  1:23");
        let busy = Transport { seeking: true, ..t };
        assert!(transport_line(&busy, 0).starts_with("jumping"));
    }

    #[test]
    fn the_play_bar_jumps_a_video_back_and_forward_and_loops() {
        let c = ctx();
        let mut p = Present::new(tmpdir("transport"));
        p.settings.media = vec![CLIP.to_string()];
        p.settings.mute = true;
        assert!(p.transport().is_none() && !p.media_skip(10_000), "nothing on the stage: no bar, no jump");
        let mut st = p.stage_media(&c, 0).expect("clip opens");
        let t = p.transport().expect("a video has a play bar");
        assert!(t.is_video && t.can_jump && t.dur_ms > 1900 && t.dur_ms < 2100, "{t:?}");
        assert!(p.media_pause());
        assert!(p.media_seek(1500));
        assert!((1500..=1600).contains(&p.transport().unwrap().pos_ms));
        assert!(p.media_skip(-1000));
        assert!((500..=600).contains(&p.transport().unwrap().pos_ms));
        assert!(p.media_skip(-10_000));
        assert_eq!(p.transport().unwrap().pos_ms, 0, "Back never goes below 0:00");
        assert!(p.media_skip(10_000_000));
        assert!(p.transport().unwrap().ended, "Forward past the end lands on the end");
        // Loop is on the bar too
        p.set_loop(true);
        assert!(p.transport().unwrap().looping);
        p.tick_at(&c, &mut st, 0, 0);
    }

    #[test]
    fn when_it_ends_rules_are_saved_and_read_back() {
        assert_eq!(AfterEnd::Nothing.encode(), None);
        assert_eq!(AfterEnd::decode("clear"), AfterEnd::Clear);
        let a = AfterEnd::Show("/m/My Pics/easter.png".into());
        assert_eq!(AfterEnd::decode(&a.encode().unwrap()), a, "spaces and colons in a path survive");
        assert_eq!(AfterEnd::decode("rubbish"), AfterEnd::Nothing);
        assert_eq!(AfterEnd::decode("show:"), AfterEnd::Nothing);
        let d = tmpdir("afterend");
        let mut p = Present::new(d.clone());
        p.set_after("/a/b.mp4", a.clone());
        assert_eq!(Present::new(d.clone()).after_of("/a/b.mp4"), a, "remembered after a restart");
        p.set_after("/a/b.mp4", AfterEnd::Nothing);
        assert_eq!(Present::new(d).after_of("/a/b.mp4"), AfterEnd::Nothing, "Nothing removes the rule");
    }

    #[test]
    fn a_video_that_ends_clears_the_screen_or_shows_the_next_picture() {
        let c = ctx();
        let d = tmpdir("afterplay");
        let pic = png(&d, "next.png", 32, 18);
        let pic_s = pic.clone();
        let mut p = Present::new(d);
        p.settings.media = vec![CLIP.to_string(), pic_s.clone()];
        p.settings.mute = true;

        // nothing chosen: the video just stays on its last picture
        let mut st = p.stage_media(&c, 0).unwrap();
        assert!(p.media_skip(10_000_000));
        p.tick_at(&c, &mut st, 0, 0);
        assert_eq!(st.kind, StageKind::Media);
        assert!(p.has_video(), "still the video");

        // clear the screen
        p.set_after(CLIP, AfterEnd::Clear);
        let mut st = p.stage_media(&c, 0).unwrap();
        assert!(p.media_skip(10_000_000));
        let changed = p.tick_at(&c, &mut st, 0, 0);
        assert!(changed && st.kind == StageKind::Blank, "{:?}", st.kind);
        p.tick_at(&c, &mut st, 0, 0);
        assert!(!p.has_video(), "the finished video is freed");

        // show a picture next
        p.set_after(CLIP, AfterEnd::Show(pic_s));
        let mut st = p.stage_media(&c, 0).unwrap();
        assert!(p.media_skip(10_000_000));
        let changed = p.tick_at(&c, &mut st, 0, 0);
        assert!(changed && st.kind == StageKind::Media);
        assert!(!p.has_video() && p.loaded_idx == Some(1), "the picture is on the stage now");

        // Loop on: the video never ends, so nothing is handed over
        p.set_loop(true);
        let mut st = p.stage_media(&c, 0).unwrap();
        assert!(p.media_skip(10_000_000));
        p.tick_at(&c, &mut st, 0, 0);
        assert!(p.has_video(), "looping: it stays");
    }

    #[test]
    fn stop_ends_the_video_and_clears_the_screen() {
        let c = ctx();
        let mut p = Present::new(tmpdir("stopbtn"));
        p.settings.media = vec![CLIP.to_string()];
        p.settings.mute = true;
        assert!(p.media_stop().is_none(), "nothing playing: nothing to stop");
        let mut st = p.stage_media(&c, 0).unwrap();
        let blank = p.media_stop().expect("a blank screen to show");
        assert_eq!(blank.kind, StageKind::Blank);
        assert!(!p.has_video() && p.transport().is_none());
        st = blank;
        p.tick_at(&c, &mut st, 0, 0);
        assert!(!p.has_video());
    }

    #[test]
    fn s3_6_loop_and_mute_settings_are_remembered() {
        let d = tmpdir("loopmute");
        let mut p = Present::new(d.clone());
        p.set_loop(true);
        p.set_mute(true);
        let p2 = Present::new(d);
        assert!(p2.settings.loop_video && p2.settings.mute);
    }

    #[test]
    fn s3_10_a_damaged_video_leaves_the_stage_alone() {
        let d = tmpdir("badvideo");
        let f = d.join("bad.mp4");
        std::fs::write(&f, b"definitely not an mp4").unwrap();
        let mut p = Present::new(d);
        p.settings.media = vec![f.to_string_lossy().to_string()];
        assert!(p.stage_media(&ctx(), 0).is_none());
        assert!(p.msg.as_ref().unwrap().0.contains("MP4"));
        assert!(p.loaded.is_none());
    }

    // ---- background (S4)

    #[test]
    fn s4_a_background_shows_behind_slides_and_timers_only_and_darken_is_capped() {
        let d = tmpdir("bg");
        let f = png(&d, "sky.png", 32, 18);
        let mut p = Present::new(d.clone());
        let c = ctx();
        p.set_background(&c, &f);
        assert!(p.art(&StageKind::Slide).background.is_some());
        assert!(p.art(&StageKind::Countdown).background.is_some());
        assert!(p.art(&StageKind::Scripture).background.is_none(), "S4.2");
        assert!(p.art(&StageKind::Media).background.is_none());
        p.settings.dim_pct = 200;
        assert_eq!(p.art(&StageKind::Slide).dim_pct, 85, "S4.3 never more than 85");
        // S4.5 a stage that is showing a slide is not disturbed by changing background
        let mut st = stage(StageKind::Slide);
        st.text = "Words".into();
        assert!(!p.tick_at(&c, &mut st, 0, 0));
        assert_eq!(st.text, "Words");
        // remembered, and restored on the next start
        let mut p2 = Present::new(d);
        assert!(p2.bg.is_none());
        p2.restore_background(&c);
        assert!(p2.bg.is_some());
        p2.clear_background();
        assert!(p2.art(&StageKind::Slide).background.is_none());
        assert!(Present::new(p2.dir.clone()).settings.background.is_none());
    }

    #[test]
    fn s4_1_a_video_background_loops_silently_and_a_bad_background_changes_nothing() {
        let c = ctx();
        let mut p = Present::new(tmpdir("bgvideo"));
        p.set_background(&c, CLIP);
        let b = p.bg.as_ref().expect("video background");
        assert!(b.player.is_some());
        assert_eq!(p.loaded_idx, None);
        let mut st = stage(StageKind::Slide);
        let deadline = Instant::now() + Duration::from_secs(5);
        while p.art(&StageKind::Slide).background.is_none() {
            assert!(Instant::now() < deadline, "no background frame");
            p.tick_at(&c, &mut st, 0, 0);
            std::thread::sleep(Duration::from_millis(20));
        }
        let before = p.settings.background.clone();
        p.set_background(&c, "/no/such/file.png");
        assert_eq!(p.settings.background, before, "a bad file does not replace a good background");
        assert!(p.bg.is_some());
    }

    // ---- timers (S5)

    #[test]
    fn s5_a_countdown_on_the_stage_keeps_counting_and_tells_the_web_page() {
        let c = ctx();
        let mut p = Present::new(tmpdir("timer"));
        p.timer_ui.minutes = 5;
        p.timer_ui.title = "Service starts in".into();
        p.timer_start();
        let mut st = p.stage_timer().unwrap();
        assert_eq!((st.kind.clone(), st.label.as_str()), (StageKind::Countdown, "Service starts in"));
        let n = p.now_ms();
        assert!(!p.tick_at(&c, &mut st, n, 0), "nothing changed in the same second");
        assert!(p.tick_at(&c, &mut st, n + 2_000, 0), "two seconds later the text changed: the web page must be told");
        assert_eq!(st.text, "04:58");
        assert!(!p.tick_at(&c, &mut st, n + 2_100, 0));
        // the timer is NOT stopped by showing something else (it keeps running in the panel)
        let mut scripture = stage(StageKind::Scripture);
        assert!(!p.tick_at(&c, &mut scripture, n + 60_000, 0));
        assert_eq!(p.timer.display(n + 60_000, 0), "04:00");
    }

    #[test]
    fn s5_2_until_a_clock_time_is_checked_and_bad_times_say_how_to_type_them() {
        let mut p = Present::new(tmpdir("until"));
        p.timer_ui.choice = TimerChoice::UntilClock;
        p.timer_ui.until_text = "half past ten".into();
        p.timer_start();
        assert!(p.msg.as_ref().unwrap().0.contains("10:30"));
        p.timer_ui.until_text = "10:30".into();
        p.timer_start();
        assert!(matches!(p.timer.kind(), TimerKind::UntilClock { target_secs: 37_800 }));
    }

    #[test]
    fn s5_stopwatch_pause_resume_reset_and_clock() {
        let mut p = Present::new(tmpdir("stopwatch"));
        p.timer_ui.choice = TimerChoice::StopWatch;
        p.timer_start();
        assert_eq!(p.timer_phase(), Phase::Running);
        p.timer_pause();
        assert_eq!(p.timer_phase(), Phase::Paused);
        p.timer_resume();
        assert_eq!(p.timer_phase(), Phase::Running);
        p.timer_reset();
        assert_eq!(p.timer_phase(), Phase::Idle);
        assert_eq!(p.timer_display(), "00:00");
        p.timer_ui.choice = TimerChoice::Clock;
        p.settings.twelve_hour = true;
        p.timer_start();
        let d = p.timer_display();
        assert!(d.ends_with("AM") || d.ends_with("PM"), "{d}");
    }

    #[test]
    fn the_clock_can_be_put_on_the_stage_with_no_title() {
        let mut p = Present::new(tmpdir("clock"));
        let c = p.stage_clock();
        assert_eq!((c.kind.clone(), c.label.as_str()), (StageKind::Countdown, ""));
        assert_eq!(c.text.len(), 5, "HH:MM, got {:?}", c.text);
        assert_eq!(c.text.as_bytes()[2], b':');
        p.settings.twelve_hour = true;
        let c = p.stage_clock();
        assert!(c.text.ends_with("AM") || c.text.ends_with("PM"), "{}", c.text);
    }

    #[test]
    fn s5_showing_a_timer_that_was_never_started_shows_its_full_time() {
        let mut p = Present::new(tmpdir("idle"));
        p.timer_ui.choice = TimerChoice::CountDown;
        p.timer_ui.minutes = 7;
        p.timer_reset();
        assert_eq!(p.stage_timer().unwrap().text, "07:00");
        // changing the minutes before it has started is picked up when shown
        p.timer_ui.minutes = 3;
        assert_eq!(p.stage_timer().unwrap().text, "03:00");
    }

    #[test]
    fn the_shelf_shows_by_clicking_and_survives_the_original_going_away() {
        let d = tmpdir("shelf");
        let f = png(&d, "cross.png", 16, 16);
        let mut p = Present::new(d.clone());
        let c = ctx();
        p.route_path(&c, Path::new(&f), PickFor::Media);
        std::fs::remove_file(&f).unwrap(); // the USB stick is gone
        let content = p.stage_entry(&c, 1).expect("the shelf's own copy still shows");
        assert_eq!(content.kind, StageKind::Media);
        assert!(p.shelf.missing_ids().is_empty());
        p.remove_from_shelf(1);
        assert!(p.shelf.entries.is_empty() && p.settings.media.is_empty());
        assert!(p.stage_entry(&c, 1).is_none(), "an item that is gone says so and leaves the stage alone");
    }


    #[test]
    fn a_shelf_item_can_be_renamed_and_moved_and_the_stage_uses_the_new_name() {
        let d = tmpdir("shelf2");
        let f = png(&d, "img1.png", 8, 8);
        let mut p = Present::new(d);
        let c = ctx();
        p.route_path(&c, Path::new(&f), PickFor::Media);
        p.shelf.rename(1, "Welcome slide").unwrap();
        p.shelf.set_folder(1, "Church").unwrap();
        assert_eq!(p.shelf.list(Some("Church"), "welcome"), vec![1]);
        assert_eq!(p.stage_entry(&c, 1).unwrap().label, "Welcome slide");
    }

    #[test]
    fn a_video_preview_shrinks_and_a_picture_preview_is_made_once() {
        let img = rf_media::Rgba { width: 1920, height: 1080, pixels: vec![200; 1920 * 1080 * 4] };
        let s = shrink_rgba(&img, 256);
        assert_eq!((s.width, s.height), (256, 144));
        assert_eq!(s.pixels.len(), 256 * 144 * 4);
        let tiny = shrink_rgba(&rf_media::Rgba { width: 4, height: 2, pixels: vec![1; 32] }, 256);
        assert_eq!((tiny.width, tiny.height), (4, 2));
    }


    #[test]
    fn a_whole_folder_dropped_from_finder_becomes_a_folder_with_its_pictures_inside() {
        let d = tmpdir("dirdrop");
        let src = d.join("Easter 2026");
        std::fs::create_dir_all(src.join("Songs")).unwrap();
        png(&src, "welcome.png", 8, 8);
        png(&src.join("Songs"), "holy.png", 9, 9);
        std::fs::write(src.join("notes.txt"), "not media").unwrap();
        std::fs::write(src.join(".DS_Store"), "junk").unwrap();
        let mut p = Present::new(d);
        let c = ctx();
        p.add_directory(&c, &src);
        assert_eq!(p.shelf.folders(), vec!["Easter 2026", "Easter 2026 / Songs"]);
        assert_eq!(p.shelf.count_in(Some("Easter 2026")), 1);
        assert_eq!(p.shelf.count_in(Some("Easter 2026 / Songs")), 1);
        assert_eq!(p.shelf.entries.len(), 2, "the text file and the junk file were left out");
        assert_eq!(p.shelf_view.as_deref(), Some("Easter 2026"));
        assert!(p.target_folder.is_none(), "the temporary target is cleared again");
    }

    #[test]
    fn files_dropped_on_a_folder_go_into_that_folder() {
        let d = tmpdir("dropfolder");
        let f = png(&d, "a.png", 8, 8);
        let mut p = Present::new(d);
        let c = ctx();
        p.shelf.create_folder("Church").unwrap();
        p.target_folder = Some("Church".into());
        p.route_path(&c, Path::new(&f), PickFor::Media);
        p.target_folder = None;
        assert_eq!(p.shelf.list(Some("Church"), ""), vec![1]);
        assert_eq!(p.folder_at(egui::pos2(5.0, 5.0)), None, "nothing drawn yet, nothing to hit");
    }


    #[test]
    fn the_media_bar_is_docked_not_a_popup_and_folds_to_one_line() {
        let c = ctx();
        crate::app::load_stage_fonts(&c);
        let d = tmpdir("dock");
        let mut p = Present::new(d);
        assert!(!p.open, "the pop-up window is closed");
        let frame = |p: &mut Present, c: &egui::Context| -> String {
            let mut raw = egui::RawInput::default();
            raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1100.0, 800.0)));
            let mut texts = Vec::new();
            for _ in 0..3 {
                let out = c.run(raw.clone(), |ctx| {
                    let _ = p.dock_ui(ctx);
                });
                for cs in out.shapes {
                    collect_text(&cs.shape, &mut texts);
                }
            }
            texts.join("\n")
        };
        let open = frame(&mut p, &c);
        assert!(open.contains("MEDIA") && open.contains("Drag pictures, videos or whole folders here"), "{open}");
        p.dock_open = false;
        let folded = frame(&mut p, &c);
        assert!(folded.contains("MEDIA") && folded.contains("item(s)") && !folded.contains("Add files"), "{folded}");
    }


    fn draw_dock(p: &mut Present, c: &egui::Context, frames: usize) -> String {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1100.0, 800.0)));
        let mut texts = Vec::new();
        for _ in 0..frames {
            let out = c.run(raw.clone(), |ctx| {
                let _ = p.dock_ui(ctx);
            });
            for cs in out.shapes {
                collect_text(&cs.shape, &mut texts);
            }
        }
        texts.join("\n")
    }

    #[test]
    fn adding_in_the_background_keeps_the_window_free_and_finishes_with_a_message() {
        let d = tmpdir("bg");
        let f = png(&d, "later.png", 8, 8);
        let mut p = Present::new(d);
        p.bg_adds = true;
        let c = ctx();
        p.route_path(&c, Path::new(&f), PickFor::Media);
        assert_eq!(p.jobs_pending(), 1, "queued, not yet on the shelf");
        let t0 = std::time::Instant::now();
        while p.jobs_pending() > 0 && t0.elapsed().as_secs() < 10 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            p.poll_jobs();
        }
        assert_eq!(p.jobs_pending(), 0, "finished");
        assert_eq!(p.shelf.entries.len(), 1);
        assert_eq!(p.settings.media.len(), 1, "joined the list the stage uses");
        assert!(p.msg.as_ref().map_or(false, |m| m.0.contains("Added picture")), "{:?}", p.msg);
        assert_eq!(p.jobs_progress(), (0, 0));
        // the same file again is allowed now: a second listing (v0.840.0)
        p.route_path(&c, Path::new(&f), PickFor::Media);
        while p.jobs_pending() > 0 && t0.elapsed().as_secs() < 20 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            p.poll_jobs();
        }
        assert_eq!(p.shelf.entries.len(), 2);
    }

    #[test]
    fn a_big_shelf_only_makes_thumbnails_for_the_rows_you_can_see() {
        let d = tmpdir("many");
        let mut p = Present::new(d.clone());
        let c = ctx();
        crate::app::load_stage_fonts(&c);
        let src = d.join("src");
        std::fs::create_dir_all(&src).unwrap();
        for i in 0..300u32 {
            let f = png(&src, &format!("pic{i:03}.png", i = i), 8 + (i % 20), 8 + (i / 20));
            p.route_path(&c, Path::new(&f), PickFor::Media);
        }
        assert_eq!(p.shelf.entries.len(), 300);
        let text = draw_dock(&mut p, &c, 4);
        assert!(text.contains("1 pic000") && !text.contains("300 pic299"), "only the first rows are drawn");
        assert!(p.thumbs.len() < 90, "{} thumbnails were made for 300 files", p.thumbs.len());
    }

    #[test]
    fn list_view_search_and_sort_work() {
        let d = tmpdir("listview");
        let mut p = Present::new(d.clone());
        let c = ctx();
        crate::app::load_stage_fonts(&c);
        for (n, w) in [("zebra.png", 8), ("apple.png", 9), ("mango.png", 10)] {
            let f = png(&d, n, w, 8);
            p.route_path(&c, Path::new(&f), PickFor::Media);
        }
        p.settings.view = "list".into();
        p.settings.sort = "name".into();
        let text = draw_dock(&mut p, &c, 3);
        let (a, m, z) = (text.find("\u{25aa}  apple").unwrap(), text.find("\u{25aa}  mango").unwrap(), text.find("\u{25aa}  zebra").unwrap());
        assert!(a < m && m < z, "A to Z: {text}");
        assert!(text.contains("KB"), "sizes show in the list");
        p.shelf_filter = "man".into();
        let text = draw_dock(&mut p, &c, 3);
        assert!(text.contains("\u{25aa}  mango") && !text.contains("\u{25aa}  apple") && !text.contains("\u{25aa}  zebra"));
        p.shelf_filter = "nothing like it".into();
        assert!(draw_dock(&mut p, &c, 3).contains("Nothing matches"));
    }

    #[test]
    fn v0_844_selecting_files_in_the_media_bar_and_removing_them_safely() {
        let d = tmpdir("select");
        let mut p = Present::new(d.clone());
        let c = ctx();
        crate::app::load_stage_fonts(&c);
        for (n, w) in [("a.png", 8), ("b.png", 9), ("c.png", 10)] {
            let f = png(&d, n, w, 8);
            p.route_path(&c, Path::new(&f), PickFor::Media);
        }
        assert!(p.dock_selected_files().is_empty(), "nothing is selected at first");
        // Cmd+A: everything in the folder being looked at
        let msg = p.dock_select_all();
        assert!(msg.contains("Selected all 3"), "{msg}");
        assert_eq!(p.dock_selected_files().len(), 3);
        // the selection is drawn (a tick badge) without trouble in both views
        p.settings.view = "list".into();
        let list = draw_dock(&mut p, &c, 3);
        assert!(list.contains('\u{2713}'), "a tick shows on selected rows: {list}");
        p.settings.view = "grid".into();
        let grid = draw_dock(&mut p, &c, 3);
        assert!(grid.contains("MEDIA"), "the thumbnail view draws with a selection: {grid}");
        // Esc / select nothing
        p.dock_clear_selection();
        assert!(p.dock_selected_files().is_empty());
        // the search narrows what Cmd+A means
        p.shelf_filter = "b".into();
        p.dock_select_all();
        assert_eq!(p.dock_selected_files().len(), 1);
        p.shelf_filter.clear();
        p.dock_select_all();
        // Delete asks first, and only the second press removes
        p.dock_remove_selected();
        assert_eq!(p.shelf.entries.len(), 3, "the first press only asks");
        assert!(p.msg.as_ref().map_or(false, |m| m.0.contains("Press Delete again")));
        p.dock_remove_selected();
        assert_eq!(p.shelf.entries.len(), 0, "the second press removes them");
        assert!(p.dock_selected_files().is_empty());
        // a locked (volunteer) screen refuses
        let f = png(&d, "d.png", 8, 8);
        p.route_path(&c, Path::new(&f), PickFor::Media);
        p.dock_select_all();
        p.locked = true;
        p.dock_remove_selected();
        p.dock_remove_selected();
        assert_eq!(p.shelf.entries.len(), 1, "locked: nothing is removed");
        // copying with nothing selected explains itself
        p.dock_clear_selection();
        assert!(p.dock_copy_selected(&c).contains("Nothing is selected"));
        // on this (non-Mac) test machine the copy falls back to the paths as text
        p.dock_select_all();
        assert!(p.dock_copy_selected(&c).contains("Copied"));
    }

    // ---- the window

    #[test]
    fn the_window_draws_every_tab_without_trouble_and_shows_the_licence_notes() {
        let c = ctx();
        crate::app::load_stage_fonts(&c);
        let d = tmpdir("ui");
        let f = png(&d, "x.png", 8, 8);
        let gone = png(&d, "gone.png", 9, 9);
        let mut p = Present::new(d);
        p.route_path(&c, Path::new(&f), PickFor::Media);
        p.settings.shelf_policy = "link".into();
        p.route_path(&c, Path::new(&gone), PickFor::Media);
        std::fs::remove_file(&gone).unwrap(); // a linked original that moved: the shelf must say so
        p.new_song();
        p.set_slides("A\n\nChorus:\nB\nC", "Typed slides");
        p.open = true;
        let mut texts: Vec<String> = Vec::new();
        for tab in [Tab::Media, Tab::Slides, Tab::Songs, Tab::Background, Tab::Timers] {
            p.tab = tab;
            let mut raw = egui::RawInput::default();
            raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1100.0, 800.0)));
            for _ in 0..3 {
                let out = c.run(raw.clone(), |ctx| {
                    p.ui(ctx);
                    let _ = p.dock_ui(ctx);
                });
                for cs in out.shapes {
                    collect_text(&cs.shape, &mut texts);
                }
            }
        }
        let all = texts.join("\n");
        for want in ["MEDIA", "Add files", "gone", "missing", "Re-link", "All  2", "Options", "OpenH264 Video Codec provided by Cisco Systems, Inc.", "Make slides", "CCLI", "Darken", "Show on stage", "Start"] {
            assert!(all.contains(want), "the window never showed {want:?}");
        }
        assert!(p.open, "drawing the window does not close it");
    }

    fn collect_text(s: &egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| collect_text(x, out)),
            _ => {}
        }
    }

    #[test]
    fn the_window_is_silent_when_closed() {
        let c = ctx();
        let mut p = Present::new(tmpdir("closed"));
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)));
        let mut req = None;
        let out = c.run(raw, |ctx| {
            req = p.ui(ctx);
        });
        assert!(req.is_none());
        assert!(out.shapes.is_empty());
    }

    fn wait_for_jobs(p: &mut Present) {
        for _ in 0..3000 {
            p.poll_jobs();
            if p.jobs_pending() == 0 {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the background add never finished");
    }

    fn ffmpeg_make(args: &[&str], out: &Path) -> bool {
        match rf_media::convert::find_ffmpeg() {
            Some(ff) => std::process::Command::new(ff).args(["-y", "-hide_banner", "-loglevel", "error"]).args(args).arg(out).status().map(|s| s.success()).unwrap_or(false),
            None => false,
        }
    }

    #[test]
    fn an_iphone_hevc_video_is_converted_when_added_and_then_plays() {
        if rf_media::convert::find_ffmpeg().is_none() {
            eprintln!("SKIPPED: ffmpeg is not installed here");
            return;
        }
        let d = tmpdir("hevc-add");
        let c = ctx();
        let mut p = Present::new(d.clone());
        p.bg_adds = true;
        let src = d.join("Baptism.mp4");
        assert!(ffmpeg_make(&["-f", "lavfi", "-i", "testsrc=size=320x180:rate=25:duration=1", "-f", "lavfi", "-i", "sine=duration=1", "-c:v", "libx265", "-pix_fmt", "yuv420p", "-tag:v", "hvc1", "-c:a", "aac", "-shortest"], &src));
        assert!(Player::open(&src, false).is_err(), "as it is, the player refuses it");
        p.route_path(&c, &src, PickFor::Media);
        wait_for_jobs(&mut p);
        assert_eq!(p.shelf.entries.len(), 1, "added after converting: {:?}", p.msg);
        let id = p.shelf.entries[0].id;
        assert_eq!(p.shelf.get(id).unwrap().name, "Baptism");
        assert!(p.shelf.get(id).unwrap().path.contains("converted"), "the shelf points at the playable copy");
        let content = p.stage_entry(&c, id);
        assert!(content.is_some(), "it plays now: {:?}", p.msg);
        assert!(p.has_video());
    }

    #[test]
    fn a_mov_and_an_opus_sound_are_converted_and_a_broken_one_is_explained() {
        if rf_media::convert::find_ffmpeg().is_none() {
            eprintln!("SKIPPED: ffmpeg is not installed here");
            return;
        }
        let d = tmpdir("mov-add");
        let c = ctx();
        let mut p = Present::new(d.clone());
        p.bg_adds = true;
        let mov = d.join("Welcome.mov");
        assert!(ffmpeg_make(&["-f", "lavfi", "-i", "testsrc=size=320x180:rate=25:duration=1", "-c:v", "libx264", "-pix_fmt", "yuv420p"], &mov));
        p.route_path(&c, &mov, PickFor::Media);
        wait_for_jobs(&mut p);
        assert_eq!(p.shelf.entries.len(), 1, "{:?}", p.msg);
        let broken = d.join("Broken.mov");
        std::fs::write(&broken, b"nothing").unwrap();
        p.route_path(&c, &broken, PickFor::Media);
        wait_for_jobs(&mut p);
        assert_eq!(p.shelf.entries.len(), 1, "a broken file is not added");
        assert!(p.msg.as_ref().map_or(false, |m| m.0.contains("did not work")), "{:?}", p.msg);
    }
}
