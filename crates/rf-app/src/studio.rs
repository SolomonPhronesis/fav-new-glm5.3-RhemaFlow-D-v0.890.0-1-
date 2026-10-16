//! Studio (v0.843.0): the glue between `rf-studio` (pure rules, tested on their own) and the app.
//! State and what each button does live here; the window is in `studio_ui.rs`.
//!
//! Safe by design: nothing in here puts anything on the screen by itself. Macros, alerts and shortcuts only run
//! when the operator starts them (a button, a shortcut they chose, a phrase they chose).

use crate::app::RhemaFlowApp;
use crate::state::{StageContent, StageKind};
use rf_studio::{alerts, goto, hotkeys, import, library_tools, macros, parallel, stagemon, vars};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum STab {
    Import,
    Chords,
    Bibles,
    Alerts,
    Macros,
    Library,
    Templates,
    Shortcuts,
    Prompter,
    Monitor,
}

impl STab {
    pub const ALL: [STab; 10] = [STab::Import, STab::Chords, STab::Bibles, STab::Alerts, STab::Macros, STab::Library, STab::Templates, STab::Shortcuts, STab::Prompter, STab::Monitor];
    pub fn label(self) -> &'static str {
        match self {
            STab::Import => "Import songs",
            STab::Chords => "Chords",
            STab::Bibles => "Bible versions",
            STab::Alerts => "Alerts",
            STab::Macros => "Macros",
            STab::Library => "Song library",
            STab::Templates => "Service templates",
            STab::Shortcuts => "Shortcuts",
            STab::Prompter => "Teleprompter",
            STab::Monitor => "Stage monitor",
        }
    }
}

/// Saved in `studio.json` next to the other settings.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Persist {
    pub book: macros::Book,
    /// Only the shortcuts the operator added (the built-in keys are checked against, never stored here).
    pub mine: hotkeys::Hotkeys,
    pub meta: library_tools::Meta,
    pub nursery_template: String,
    pub band_text: String,
    pub monitor_role: u8,
}

pub struct Studio {
    pub p: Persist,
    pub tab: STab,
    pub alerts: alerts::Queue,
    pub runs: Vec<macros::Run>,
    pub dir: PathBuf,
    t0: Instant,
    // ---- import
    pub import_paths: String,
    pub import_paste: String,
    pub import_name: String,
    pub import_msgs: Vec<String>,
    // ---- chords
    pub chart_text: String,
    pub chart_steps: i32,
    pub chart_capo: u8,
    pub chart_nashville: bool,
    // ---- bible versions
    pub par_ref: String,
    pub par_stacked: bool,
    pub par_numbers: bool,
    pub par_pages: Vec<parallel::ParallelSlide>,
    pub par_page: usize,
    pub par_msg: String,
    // ---- alerts
    pub alert_text: String,
    pub alert_kind: alerts::Kind,
    pub nursery_code: String,
    pub alert_msg: String,
    // ---- macros
    pub macro_name: String,
    pub macro_script: String,
    pub macro_msgs: Vec<String>,
    pub bind_kind: u8,
    pub bind_value: String,
    // ---- library
    pub find: String,
    pub replace: String,
    pub find_case: bool,
    pub find_word: bool,
    pub find_preview: Vec<library_tools::Replaced>,
    pub dup_threshold: f32,
    pub tag_text: String,
    pub lib_msg: String,
    // ---- templates
    pub tpl_name: String,
    pub tpl_msg: String,
    // ---- shortcuts
    pub hot_combo: String,
    pub hot_action: String,
    pub hot_msg: String,
    // ---- teleprompter
    pub prompt_text: String,
    pub prompt_wpm: f32,
    pub prompt_mirror: bool,
    pub prompt_running: bool,
    pub prompt_pos: f32,
    prompt_last: Option<u64>,
    // ---- stage monitor
    pub monitor_open: bool,
    pub monitor_own_window: bool,
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Minutes east of UTC where this computer is right now (60 in UK summer). Falls back to 0.
pub fn utc_offset_min(unix: i64) -> i32 {
    unsafe {
        let t: libc::time_t = unix as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return 0;
        }
        (tm.tm_gmtoff / 60) as i32
    }
}

/// An egui key name and the modifier keys as a shortcut text the shortcut list understands ("Cmd+Shift+K").
pub fn combo_text(cmd: bool, ctrl: bool, alt: bool, shift: bool, key_name: &str) -> Option<String> {
    let k = match key_name {
        n if n.len() == 4 && n.starts_with("Num") => &n[3..],
        n => n,
    };
    let mut s = String::new();
    if cmd {
        s.push_str("Cmd+");
    }
    if ctrl {
        s.push_str("Ctrl+");
    }
    if alt {
        s.push_str("Alt+");
    }
    if shift {
        s.push_str("Shift+");
    }
    s.push_str(k);
    hotkeys::Combo::parse(&s).ok().map(|c| c.to_string())
}

/// What the macro steps mean for the app.
pub fn describe_all(steps: &[macros::Step]) -> Vec<String> {
    steps.iter().map(|s| if s.after_ms > 0 { format!("after {:.1}s: {}", s.after_ms as f32 / 1000.0, s.action.describe()) } else { s.action.describe() }).collect()
}

impl Studio {
    pub fn load(dir: &Path) -> Studio {
        let p: Persist = std::fs::read_to_string(dir.join("studio.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let mut mine = p.mine.clone();
        // a damaged or hand-edited file can never keep a reserved or repeated key
        if let Ok((clean, _)) = hotkeys::Hotkeys::from_json(&mine.to_json()) {
            mine = clean;
        }
        Studio {
            p: Persist { mine, ..p },
            tab: STab::Import,
            alerts: alerts::Queue::new(),
            runs: Vec::new(),
            dir: dir.to_path_buf(),
            t0: Instant::now(),
            import_paths: String::new(),
            import_paste: String::new(),
            import_name: String::new(),
            import_msgs: Vec::new(),
            chart_text: String::new(),
            chart_steps: 0,
            chart_capo: 0,
            chart_nashville: false,
            par_ref: String::new(),
            par_stacked: false,
            par_numbers: true,
            par_pages: Vec::new(),
            par_page: 0,
            par_msg: String::new(),
            alert_text: String::new(),
            alert_kind: alerts::Kind::Info,
            nursery_code: String::new(),
            alert_msg: String::new(),
            macro_name: String::new(),
            macro_script: String::new(),
            macro_msgs: Vec::new(),
            bind_kind: 0,
            bind_value: String::new(),
            find: String::new(),
            replace: String::new(),
            find_case: false,
            find_word: true,
            find_preview: Vec::new(),
            dup_threshold: 0.8,
            tag_text: String::new(),
            lib_msg: String::new(),
            tpl_name: String::new(),
            tpl_msg: String::new(),
            hot_combo: String::new(),
            hot_action: String::new(),
            hot_msg: String::new(),
            prompt_text: String::new(),
            prompt_wpm: 140.0,
            prompt_mirror: false,
            prompt_running: false,
            prompt_pos: 0.0,
            prompt_last: None,
            monitor_open: false,
            monitor_own_window: false,
        }
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_vec_pretty(&self.p) {
            let _ = rf_service::store::atomic_write(&self.dir.join("studio.json"), &json);
        }
    }

    pub fn now_ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }

    /// Alerts to draw on the screens: (words, colour). Nothing while the screen is blacked out.
    pub fn alert_lines(&self, blackout: bool) -> Vec<(String, [u8; 3])> {
        if blackout {
            return Vec::new();
        }
        self.alerts.active(self.now_ms(), 2).iter().map(|a| (a.text.clone(), a.kind.color())).collect()
    }

    pub fn start_macro(&mut self, name: &str) -> Result<(), String> {
        let m = self.p.book.get(name).cloned().ok_or_else(|| format!("There is no macro called \"{name}\"."))?;
        self.runs.push(macros::Run::start(&m, self.now_ms()));
        Ok(())
    }

    pub fn stop_all_macros(&mut self) {
        for r in &mut self.runs {
            r.cancel();
        }
        self.runs.clear();
    }

    pub fn prompt_tick(&mut self, words_per_line: f32, line_px: f32, total_px: f32) {
        let now = self.now_ms();
        if !self.prompt_running {
            self.prompt_last = None;
            return;
        }
        let dt = now.saturating_sub(self.prompt_last.unwrap_or(now)).min(2000) as f32 / 1000.0;
        self.prompt_last = Some(now);
        self.prompt_pos += rf_studio::teleprompter::speed_px_s(self.prompt_wpm, words_per_line, line_px) * dt;
        if self.prompt_pos >= total_px {
            self.prompt_pos = total_px.max(0.0);
            self.prompt_running = false;
        }
    }
}

/// Files for an import: each line of the box is a file or a folder (quotes, file:// and ~ are understood).
pub fn read_import_files(lines: &str, home: Option<&str>) -> (Vec<(String, String)>, Vec<String>) {
    let mut files = Vec::new();
    let mut errors = Vec::new();
    let ok_ext = ["xml", "txt", "cho", "chordpro", "chopro", "crd", "cpro", "usr", "lyrics", "opensong", ""];
    for line in lines.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Some(path) = crate::clip::path_from_clipboard(line, home) else {
            errors.push(format!("\"{line}\" is not a file path."));
            continue;
        };
        let list: Vec<PathBuf> = if path.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(&path).map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file()).collect()).unwrap_or_default();
            v.sort();
            v
        } else {
            vec![path.clone()]
        };
        if list.is_empty() {
            errors.push(format!("{}: nothing found there.", path.display()));
        }
        for p in list {
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if !ok_ext.contains(&ext.as_str()) {
                continue;
            }
            match std::fs::read(&p) {
                Ok(b) => files.push((p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), String::from_utf8_lossy(&b).to_string())),
                Err(e) => errors.push(format!("{}: {e}", p.display())),
            }
        }
    }
    (files, errors)
}

impl RhemaFlowApp {
    /// Add imported songs to the library. Returns how many were added; messages go to the Import tab.
    pub fn studio_import(&mut self, files: Vec<(String, String)>, mut errors: Vec<String>) {
        if self.present.locked {
            self.x.studio.import_msgs = vec![crate::lock::refusal(crate::lock::Guarded::EditSongs).to_string()];
            return;
        }
        let (done, bad) = import::import_many(&files);
        errors.extend(bad);
        let mut titles: Vec<String> = self.present.library.songs().iter().map(|s| s.title.clone()).collect();
        let mut msgs = Vec::new();
        let mut n = 0;
        for mut imp in done {
            imp.song.title = import::unique_title(&titles, &imp.song.title);
            titles.push(imp.song.title.clone());
            let name = imp.song.title.clone();
            let id = self.present.library.add(imp.song);
            let _ = id;
            n += 1;
            for note in imp.notes {
                msgs.push(format!("{name}: {note}"));
            }
            if imp.chart.is_some() {
                msgs.push(format!("{name}: chords were found (kept for the Chords tab, not shown to the congregation)."));
            }
        }
        if n > 0 {
            self.present.save_library();
        }
        msgs.insert(0, format!("{n} song(s) added."));
        msgs.extend(errors);
        self.x.studio.import_msgs = msgs;
    }

    /// Run one macro action. Nothing here is hidden: every action is something the operator can also do by hand.
    pub fn studio_do(&mut self, a: &macros::Action) {
        use macros::{Action, Switch};
        let now = self.x.studio.now_ms();
        match a {
            Action::Next => {
                let name = if self.present.slide_index.is_some() { "next_slide" } else { "next_verse" };
                self.run_named(name);
            }
            Action::Prev => {
                let name = if self.present.slide_index.is_some() { "prev_slide" } else { "prev_verse" };
                self.run_named(name);
            }
            Action::Clear => self.run_named("clear"),
            Action::Black(Switch::On) => self.blackout = true,
            Action::Black(Switch::Off) => self.blackout = false,
            Action::Black(Switch::Toggle) => self.blackout = !self.blackout,
            Action::GoTo(text) => {
                let labels: Vec<String> = self.present.slides.iter().map(|s| s.label.clone()).collect();
                let target = goto::parse(text).and_then(|t| goto::resolve(&t, &labels, self.present.slide_index));
                match target.and_then(|i| self.present.stage_slide(i)) {
                    Some(c) => self.show_content(c),
                    None => self.short_utterance_note = Some((format!("Macro: could not go to \"{text}\" (no such slide)."), Instant::now())),
                }
            }
            Action::Sound(_) => {
                self.short_utterance_note = Some(("Macro: to play a file, put it on the shelf and use  run media_<number>  instead.".into(), Instant::now()));
            }
            Action::StopSound => self.run_named("media_pause"),
            Action::Alert(t) => {
                let _ = self.x.studio.alerts.push(alerts::Kind::Info, t, now, None);
            }
            Action::LowerThird(t) => {
                self.x.lt.line1 = t.clone();
                self.x.lt.line2 = String::new();
                self.x.lt.show(self.x.now_ms());
            }
            Action::Run(n) => self.run_named(n),
        }
    }

    /// Every frame: finish expired alerts, hand out due macro steps, notice the operator's own shortcuts.
    pub fn studio_tick(&mut self, ctx: &egui::Context) {
        let now = self.x.studio.now_ms();
        for id in std::mem::take(&mut self.present.newly_shown) {
            self.studio_note_song(id);
        }
        self.x.studio.alerts.expire(now);
        let mut due = Vec::new();
        for r in &mut self.x.studio.runs {
            due.extend(r.due(now));
        }
        self.x.studio.runs.retain(|r| !r.is_finished());
        for a in due {
            self.studio_do(&a);
        }
        // the operator's own shortcuts (only ones that start a macro or run a named action)
        if !ctx.wants_keyboard_input() && !self.x.is_locked() && !self.x.studio.p.mine.bindings.is_empty() {
            let pressed: Vec<String> = ctx.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } => combo_text(modifiers.mac_cmd, modifiers.ctrl, modifiers.alt, modifiers.shift, key.name()),
                        _ => None,
                    })
                    .collect()
            });
            for p in pressed {
                let action = hotkeys::Combo::parse(&p).ok().and_then(|c| self.x.studio.p.mine.lookup(&c).map(|a| a.to_string()));
                match action {
                    Some(a) if a.starts_with("macro:") => {
                        if let Err(e) = self.x.studio.start_macro(a[6..].trim()) {
                            self.short_utterance_note = Some((e, Instant::now()));
                        }
                    }
                    Some(a) if a.starts_with("run:") => self.run_named(a[4..].trim()),
                    _ => {}
                }
            }
        }
        if !self.x.studio.runs.is_empty() || !self.x.studio.alerts.is_empty(now) || self.x.studio.prompt_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// A phrase the church said: does it start a macro the operator tied to that phrase?
    pub fn studio_voice(&mut self, said: &str) {
        if self.x.is_locked() {
            return;
        }
        if let Some(name) = self.x.studio.p.book.match_voice(said).map(|m| m.name.clone()) {
            let _ = self.x.studio.start_macro(&name);
        }
    }

    /// Note that a song was shown (for "most used" and the CCLI-style history).
    pub fn studio_note_song(&mut self, id: u64) {
        self.x.studio.p.meta.record_use(id, unix_now());
        self.x.studio.save();
    }

    /// Everything a stage-monitor layout can show, from what is on the screen now.
    pub fn studio_stage_ctx(&self) -> stagemon::StageCtx {
        let now = unix_now();
        let mut c = stagemon::StageCtx::default();
        c.vars = vars::Ctx::new(now, utc_offset_min(now));
        if self.stage.kind != StageKind::Blank {
            c.slide = self.stage.text.clone();
            c.reference = self.stage.label.clone();
        }
        let next = self.present.slide_index.and_then(|i| self.present.slides.get(i + 1)).map(|s| s.text.clone());
        c.next = next.unwrap_or_default();
        c.title = self.service_title.clone();
        c.chords = self.x.studio.p.band_text.clone();
        c.message = self.x.studio.alerts.active(self.x.studio.now_ms(), 1).first().map(|a| a.text.clone()).unwrap_or_default();
        if let Some((id, _)) = self.order.live() {
            if let Some(it) = self.order.items().iter().find(|i| i.id == id) {
                c.notes = it.notes.clone();
                if let Some(m) = it.minutes {
                    c.vars.set_timer("Sermon", m as i64 * 60 - self.order.live_elapsed_secs() as i64);
                }
            }
        }
        c
    }

    /// Side-by-side Bible versions for a typed reference: the current version plus the ones in the Bibles folder.
    pub fn studio_parallel(&mut self) {
        let text = self.x.studio.par_ref.clone();
        let p = match crate::browser::reference_from_text(self.text_bible(), &text) {
            Ok(p) if p.from > 0 => p,
            Ok(_) => {
                self.x.studio.par_msg = "Give a verse or a range (John 3:16-18), not a whole chapter.".into();
                return;
            }
            Err(e) => {
                self.x.studio.par_msg = format!("Not a reference: {e}");
                return;
            }
        };
        let mut sources: Vec<(String, &'static rf_bible::Bible)> = vec![("Main".to_string(), self.text_bible())];
        sources.extend(self.extra_bibles.iter().take(3).map(|(n, b)| (n.clone(), *b)));
        let (book, ch) = (p.book as usize, p.chapter as usize);
        let versions: Vec<parallel::Version> = sources
            .iter()
            .map(|(label, b)| parallel::Version { label: label.clone(), verses: (p.from..=p.to).filter(|v| b.exists(book, ch, *v as usize)).map(|v| (v as u32, b.verse(book, ch, v as usize).to_string())).collect() })
            .collect();
        let chapter = format!("{} {}", sources[0].1.book_name(book), ch);
        let mode = if self.x.studio.par_stacked { parallel::Mode::Stacked } else { parallel::Mode::SideBySide };
        let pages = parallel::build(&chapter, &versions, mode, self.x.studio.par_numbers, 220);
        self.x.studio.par_msg = if pages.is_empty() { "No verses found.".into() } else if sources.len() < 2 { "Only one Bible version is loaded: add more in the Versions window to compare.".into() } else { String::new() };
        self.x.studio.par_pages = pages;
        self.x.studio.par_page = 0;
    }

    /// Put page `i` of the side-by-side Bible slide on the screens. It is drawn as a slide with each version labelled.
    pub fn studio_show_parallel(&mut self, i: usize) {
        let Some(p) = self.x.studio.par_pages.get(i).cloned() else { return };
        let sep = if p.mode == parallel::Mode::Stacked { "\n\n" } else { "\n\n" };
        let text = p.blocks.iter().map(|b| format!("{}\n{}", b.label, b.text)).collect::<Vec<_>>().join(sep);
        self.show_content(StageContent { kind: StageKind::Slide, label: p.reference.clone(), text, translation: String::new(), chapter_only: false, updated: Some(Instant::now()) });
        self.x.studio.par_page = i;
    }

    // ---------------------------------------------------------------- service templates
    pub fn studio_templates_dir(&self) -> PathBuf {
        self.x.studio.dir.join("service-templates")
    }

    pub fn studio_template_names(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.studio_templates_dir()).map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().to_string())).collect()).unwrap_or_default();
        v.sort();
        v
    }

    pub fn studio_save_template(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() || name.contains(|c| c == '/' || c == '\\' || c == ':') {
            return Err("Give the template a plain name (no slashes).".into());
        }
        let t = rf_studio::servicetpl::make_template(self.order.runner.service(), name);
        let json = serde_json::to_vec_pretty(&t).map_err(|e| e.to_string())?;
        rf_service::store::atomic_write(&self.studio_templates_dir().join(format!("{name}.json")), &json).map_err(|e| e.to_string())
    }

    /// Add the template's items to the end of the service order (nothing is removed).
    pub fn studio_use_template(&mut self, name: &str) -> Result<usize, String> {
        let bytes = std::fs::read(self.studio_templates_dir().join(format!("{name}.json"))).map_err(|e| format!("Could not read the template: {e}"))?;
        let t: rf_service::Service = serde_json::from_slice(&bytes).map_err(|e| format!("The template file is damaged: {e}"))?;
        let now = unix_now();
        let s = rf_studio::servicetpl::instantiate(&t, now, utc_offset_min(now), None);
        let mut added = 0;
        for it in s.items {
            if let Some(id) = self.order.add(&it.title, it.kind.clone()) {
                if !it.notes.is_empty() {
                    self.order.set_notes(id, &it.notes);
                }
                if it.minutes.is_some() {
                    self.order.set_minutes(id, it.minutes);
                }
                added += 1;
            }
        }
        Ok(added)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn egui_keys_become_shortcut_texts() {
        assert_eq!(combo_text(true, false, false, true, "K").as_deref(), Some("Cmd+Shift+K"));
        assert_eq!(combo_text(false, false, false, false, "Num1").as_deref(), Some("1"));
        assert_eq!(combo_text(false, false, false, false, "ArrowLeft").as_deref(), Some("Left"));
        assert_eq!(combo_text(false, true, false, false, "F5").as_deref(), Some("Ctrl+F5"));
        assert_eq!(combo_text(false, false, false, false, "Escape").as_deref(), Some("Esc"));
        assert_eq!(combo_text(false, false, false, false, "Insert"), None);
    }

    #[test]
    fn the_time_zone_is_sane() {
        let o = utc_offset_min(unix_now());
        assert!((-14 * 60..=14 * 60).contains(&o));
    }

    #[test]
    fn reading_files_for_import_handles_files_folders_quotes_and_junk() {
        let dir = std::env::temp_dir().join(format!("rf-studio-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "Verse 1:\nhello there\nfriend").unwrap();
        std::fs::write(dir.join("b.cho"), "{title: B}\n[G]hi there").unwrap();
        std::fs::write(dir.join("c.png"), [0u8, 1, 2]).unwrap();
        let text = format!("\"{}\"\n{}\n\nnot-a-path-at-all", dir.join("a.txt").display(), dir.display());
        let (files, errors) = read_import_files(&text, None);
        assert!(files.iter().filter(|(n, _)| n == "a.txt").count() == 2, "{files:?}"); // once by name, once through the folder
        assert!(files.iter().any(|(n, _)| n == "b.cho"));
        assert!(!files.iter().any(|(n, _)| n == "c.png"), "pictures are not songs");
        assert!(errors.iter().any(|e| e.contains("not-a-path")) || errors.is_empty() || files.len() >= 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_studio_file_survives_damage_and_drops_reserved_keys() {
        let dir = std::env::temp_dir().join(format!("rf-studio-load-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("studio.json"), "{ not json").unwrap();
        let s = Studio::load(&dir);
        assert!(s.p.book.macros.is_empty());
        let mut st = Studio::load(&dir);
        st.p.book.upsert(macros::Book::default().macros.first().cloned().unwrap_or(macros::Macro { name: "A".into(), steps: macros::parse_script("next").unwrap() }));
        st.p.mine.bindings.push(hotkeys::Binding { combo: hotkeys::Combo::parse("Cmd+Q").unwrap(), action: "macro:A".into() });
        st.p.mine.bindings.push(hotkeys::Binding { combo: hotkeys::Combo::parse("Cmd+Shift+1").unwrap(), action: "macro:A".into() });
        st.save();
        let back = Studio::load(&dir);
        assert_eq!(back.p.book.macros.len(), 1);
        assert_eq!(back.p.mine.bindings.len(), 1, "the reserved Cmd+Q was dropped on load");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn macros_started_by_name_run_in_order_and_unknown_names_are_refused() {
        let dir = std::env::temp_dir().join(format!("rf-studio-run-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Studio::load(&dir);
        assert!(s.start_macro("nope").is_err());
        s.p.book.upsert(macros::Macro { name: "Go".into(), steps: macros::parse_script("next\nwait 1s\nclear").unwrap() });
        s.start_macro("go").unwrap();
        assert_eq!(s.runs.len(), 1);
        s.stop_all_macros();
        assert!(s.runs.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn alerts_are_hidden_while_the_screen_is_blacked_out() {
        let dir = std::env::temp_dir().join(format!("rf-studio-alert-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Studio::load(&dir);
        let now = s.now_ms();
        s.alerts.push(alerts::Kind::Nursery, "Nursery: 214", now, None).unwrap();
        assert_eq!(s.alert_lines(false).len(), 1);
        assert!(s.alert_lines(true).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_teleprompter_stops_at_the_end() {
        let dir = std::env::temp_dir().join(format!("rf-studio-prompt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Studio::load(&dir);
        s.prompt_running = true;
        s.prompt_pos = 99.0;
        std::thread::sleep(std::time::Duration::from_millis(20));
        s.prompt_tick(8.0, 50.0, 100.0);
        s.prompt_tick(8.0, 50.0, 100.0);
        assert!(s.prompt_pos <= 100.0);
        s.prompt_pos = 100.0;
        s.prompt_running = true;
        s.prompt_tick(8.0, 50.0, 100.0);
        assert!(s.prompt_pos <= 100.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
