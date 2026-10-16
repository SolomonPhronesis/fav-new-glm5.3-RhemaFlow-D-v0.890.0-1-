//! Studio window tabs (v0.843.0). The rules are in `rf-studio`; what each button does is in `studio.rs`.

use crate::app::RhemaFlowApp;
use crate::studio::{describe_all, read_import_files, unix_now, STab};
use egui::{Color32, RichText};
use rf_studio::{alerts, chords, hotkeys, import, library_tools, macros, stagemon, teleprompter};

const MUTED: Color32 = Color32::from_gray(150);

fn hint(ui: &mut egui::Ui, t: &str) {
    ui.label(RichText::new(t).small().color(MUTED));
}

/// Chords as Nashville numbers, inline: `[4]grace`.
fn nashville_text(chart: &chords::ChordChart) -> String {
    let key = chart.key.as_deref().and_then(|k| chords::Chord::parse(k.trim_end_matches('m'))).unwrap_or(chords::Chord { root: 0, suffix: String::new(), bass: None });
    let mut out = String::new();
    for s in &chart.sections {
        out.push_str(&format!("{}:\n", s.label));
        for l in &s.lines {
            let chars: Vec<char> = l.text.chars().collect();
            let mut ci = 0;
            for pos in 0..=chars.len() {
                while ci < l.chords.len() && l.chords[ci].0 == pos {
                    out.push_str(&format!("[{}]", chords::nashville(&l.chords[ci].1, &key)));
                    ci += 1;
                }
                if pos < chars.len() {
                    out.push(chars[pos]);
                }
            }
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

impl RhemaFlowApp {
    pub fn xt_studio(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            for t in STab::ALL {
                if ui.selectable_label(self.x.studio.tab == t, t.label()).clicked() {
                    self.x.studio.tab = t;
                }
            }
        });
        ui.separator();
        match self.x.studio.tab {
            STab::Import => self.st_import(ui),
            STab::Chords => self.st_chords(ui, ctx),
            STab::Bibles => self.st_bibles(ui),
            STab::Alerts => self.st_alerts(ui),
            STab::Macros => self.st_macros(ui),
            STab::Library => self.st_library(ui),
            STab::Templates => self.st_templates(ui),
            STab::Shortcuts => self.st_shortcuts(ui),
            STab::Prompter => self.st_prompter(ui, ctx),
            STab::Monitor => self.st_monitor(ui),
        }
    }

    fn st_import(&mut self, ui: &mut egui::Ui) {
        ui.heading("Import songs");
        ui.label("Reads OpenLyrics, OpenSong, ChordPro, chords-over-words and plain lyrics files. Songs from other programs: export them there as text or OpenLyrics first, then bring them in here.");
        ui.label("Files or folders (one path per line):");
        ui.add(egui::TextEdit::multiline(&mut self.x.studio.import_paths).desired_rows(3).desired_width(f32::INFINITY));
        if ui.button("Import these files").clicked() {
            let home = std::env::var("HOME").ok();
            let (files, errs) = read_import_files(&self.x.studio.import_paths, home.as_deref());
            self.studio_import(files, errs);
        }
        ui.separator();
        ui.label("Or paste a song (any of those formats):");
        ui.horizontal(|ui| {
            ui.label("Title if it has none");
            ui.text_edit_singleline(&mut self.x.studio.import_name);
        });
        ui.add(egui::TextEdit::multiline(&mut self.x.studio.import_paste).desired_rows(6).desired_width(f32::INFINITY));
        if ui.button("Import the pasted song").clicked() {
            let name = if self.x.studio.import_name.trim().is_empty() { "pasted.txt".to_string() } else { format!("{}.txt", self.x.studio.import_name.trim()) };
            let text = self.x.studio.import_paste.clone();
            self.studio_import(vec![(name, text)], Vec::new());
        }
        ui.separator();
        if ui.button("Export every song as OpenLyrics files").on_hover_text("Writes one .xml file per song into a song-export folder next to your settings.").clicked() {
            let dir = self.x.studio.dir.join("song-export");
            let mut n = 0;
            for s in self.present.library.songs() {
                let safe: String = s.title.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
                if rf_service::store::atomic_write(&dir.join(format!("{}-{}.xml", s.id, safe.trim())), import::to_openlyrics(s).as_bytes()).is_ok() {
                    n += 1;
                }
            }
            self.x.studio.import_msgs = vec![format!("{n} song(s) written to {}", dir.display())];
        }
        for m in &self.x.studio.import_msgs {
            ui.label(m);
        }
    }

    fn st_chords(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Chords");
        hint(ui, "Paste ChordPro ([G]Amazing) or chords written over the words. Chords are for the band's screen, never the congregation's.");
        ui.add(egui::TextEdit::multiline(&mut self.x.studio.chart_text).desired_rows(8).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
        let chart = chords::parse_chart(&self.x.studio.chart_text);
        ui.horizontal(|ui| {
            ui.label("Transpose");
            ui.add(egui::DragValue::new(&mut self.x.studio.chart_steps).clamp_range(-11..=11).suffix(" semitones"));
            ui.label("Capo");
            ui.add(egui::DragValue::new(&mut self.x.studio.chart_capo).clamp_range(0..=11));
            ui.checkbox(&mut self.x.studio.chart_nashville, "Nashville numbers");
        });
        ui.horizontal(|ui| {
            ui.label(format!("Key found: {}", chart.key.as_deref().unwrap_or("none")));
            if !chords::all_chords(&chart).is_empty() && ui.small_button("Easiest capo").clicked() {
                self.x.studio.chart_capo = chords::easiest_capo(&chart);
            }
        });
        let mut moved = chords::transpose_chart(&chart, self.x.studio.chart_steps, None);
        if self.x.studio.chart_capo > 0 {
            moved = chords::with_capo(&moved, self.x.studio.chart_capo);
        }
        let acc = moved.key.as_deref().map(chords::accidentals_for_key).unwrap_or(chords::Accidentals::Sharps);
        let shown = if self.x.studio.chart_nashville { nashville_text(&moved) } else { chords::render_over(&moved, acc) };
        if self.x.studio.chart_capo > 0 {
            ui.label(format!("Play these shapes with the capo on fret {} (sounding key {}).", self.x.studio.chart_capo, chart.key.as_deref().map(|k| chords::steps_between("C", k).map(|_| k).unwrap_or(k)).unwrap_or("?")));
        }
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new(&shown).monospace());
        });
        ui.horizontal(|ui| {
            if ui.button("Copy").clicked() {
                ctx.copy_text(shown.clone());
            }
            if ui.button("Show on the band's screen").clicked() {
                self.x.studio.p.band_text = shown.clone();
                self.x.studio.save();
            }
            if ui.button("Add as a song (words only)").clicked() {
                let text = chords::render_chordpro(&chart, chords::Accidentals::Sharps);
                let name = format!("{}.cho", if chart.title.is_empty() { "chart" } else { &chart.title });
                self.studio_import(vec![(name, text)], Vec::new());
                self.x.studio.tab = STab::Import;
            }
        });
    }

    fn st_bibles(&mut self, ui: &mut egui::Ui) {
        ui.heading("Bible versions side by side");
        hint(ui, "Shows the main Bible and up to three more (Versions window) for one verse or range. Long passages are paged.");
        ui.horizontal(|ui| {
            ui.label("Reference");
            let r = ui.text_edit_singleline(&mut self.x.studio.par_ref);
            let go = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Find").clicked() || go {
                self.studio_parallel();
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.x.studio.par_stacked, "Stacked (one above the other)");
            ui.checkbox(&mut self.x.studio.par_numbers, "Verse numbers");
        });
        if !self.x.studio.par_msg.is_empty() {
            ui.label(RichText::new(&self.x.studio.par_msg).color(Color32::from_rgb(240, 170, 0)));
        }
        let pages = self.x.studio.par_pages.len();
        if pages > 0 {
            let cur = self.x.studio.par_page.min(pages - 1);
            ui.horizontal(|ui| {
                for i in 0..pages {
                    if ui.selectable_label(i == cur, format!("{}", self.x.studio.par_pages[i].reference)).clicked() {
                        self.x.studio.par_page = i;
                    }
                }
            });
            let page = self.x.studio.par_pages[cur].clone();
            for b in &page.blocks {
                ui.label(RichText::new(&b.label).strong());
                ui.label(&b.text);
            }
            if ui.button("Put this page on the screen").clicked() {
                self.studio_show_parallel(cur);
            }
        }
    }

    fn st_alerts(&mut self, ui: &mut egui::Ui) {
        ui.heading("Alerts and nursery calls");
        hint(ui, "A short message over the picture. Hidden while the screen is blacked out. Nursery calls go away after a minute.");
        let now = self.x.studio.now_ms();
        ui.horizontal(|ui| {
            ui.label("Nursery code");
            ui.add(egui::TextEdit::singleline(&mut self.x.studio.nursery_code).desired_width(80.0));
            if ui.button("Call parents").clicked() {
                let tpl = Some(self.x.studio.p.nursery_template.clone()).filter(|t| !t.trim().is_empty());
                match alerts::nursery_message(&self.x.studio.nursery_code, tpl.as_deref()) {
                    Ok(t) => {
                        let _ = self.x.studio.alerts.push(alerts::Kind::Nursery, &t, now, None);
                        self.x.studio.alert_msg.clear();
                        self.x.studio.nursery_code.clear();
                    }
                    Err(e) => self.x.studio.alert_msg = e,
                }
            }
        });
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_source("alert-kind").selected_text(self.x.studio.alert_kind.label()).show_ui(ui, |ui| {
                for k in [alerts::Kind::Info, alerts::Kind::Nursery, alerts::Kind::Urgent] {
                    ui.selectable_value(&mut self.x.studio.alert_kind, k, k.label());
                }
            });
            ui.add(egui::TextEdit::singleline(&mut self.x.studio.alert_text).desired_width(260.0));
            if ui.button("Send").clicked() {
                let (k, t) = (self.x.studio.alert_kind, self.x.studio.alert_text.clone());
                match self.x.studio.alerts.push(k, &t, now, None) {
                    Ok(_) => {
                        self.x.studio.alert_text.clear();
                        self.x.studio.alert_msg.clear();
                    }
                    Err(e) => self.x.studio.alert_msg = e,
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Nursery wording");
            if ui.add(egui::TextEdit::singleline(&mut self.x.studio.p.nursery_template).hint_text(alerts::NURSERY_TEMPLATE).desired_width(360.0)).lost_focus() {
                self.x.studio.save();
            }
        });
        if !self.x.studio.alert_msg.is_empty() {
            ui.label(RichText::new(&self.x.studio.alert_msg).color(Color32::from_rgb(240, 170, 0)));
        }
        ui.separator();
        let active: Vec<(u64, String, alerts::Kind)> = self.x.studio.alerts.active(now, 10).iter().map(|a| (a.id, a.text.clone(), a.kind)).collect();
        if active.is_empty() {
            ui.label("Nothing on the screen.");
        }
        for (id, text, kind) in active {
            ui.horizontal(|ui| {
                ui.label(RichText::new(kind.label()).color(Color32::from_rgb(kind.color()[0], kind.color()[1], kind.color()[2])));
                ui.label(&text);
                if let Some(s) = self.x.studio.alerts.seconds_left(id, now) {
                    ui.label(RichText::new(format!("{s}s")).small().color(MUTED));
                }
                if ui.small_button("Take down").clicked() {
                    self.x.studio.alerts.dismiss(id);
                }
            });
        }
        if ui.button("Take everything down").clicked() {
            self.x.studio.alerts.clear();
        }
        let log: Vec<String> = self.x.studio.alerts.log.iter().rev().take(5).map(|s| format!("{}: {}", s.kind.label(), s.text)).collect();
        if !log.is_empty() {
            hint(ui, "Recently sent:");
            for l in log {
                hint(ui, &l);
            }
        }
    }

    fn st_macros(&mut self, ui: &mut egui::Ui) {
        ui.heading("Macros");
        hint(ui, "One button, key or phrase runs a short script. Commands: next, prev, clear, black on/off, goto chorus, wait 2s, alert text, lowerthird text, run <action>.");
        let names: Vec<String> = self.x.studio.p.book.macros.iter().map(|m| m.name.clone()).collect();
        ui.horizontal_wrapped(|ui| {
            for n in &names {
                if ui.selectable_label(self.x.studio.macro_name.eq_ignore_ascii_case(n), n).clicked() {
                    if let Some(m) = self.x.studio.p.book.get(n) {
                        self.x.studio.macro_script = macros::to_script(&m.steps);
                    }
                    self.x.studio.macro_name = n.clone();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut self.x.studio.macro_name);
        });
        ui.add(egui::TextEdit::multiline(&mut self.x.studio.macro_script).desired_rows(7).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
        ui.horizontal(|ui| {
            if ui.button("Check").clicked() {
                self.x.studio.macro_msgs = match macros::parse_script(&self.x.studio.macro_script) {
                    Ok(s) => describe_all(&s),
                    Err(e) => e,
                };
            }
            if ui.button("Save").clicked() {
                let name = self.x.studio.macro_name.trim().to_string();
                match (name.is_empty(), macros::parse_script(&self.x.studio.macro_script)) {
                    (true, _) => self.x.studio.macro_msgs = vec!["Give the macro a name.".into()],
                    (_, Err(e)) => self.x.studio.macro_msgs = e,
                    (_, Ok(steps)) => {
                        self.x.studio.p.book.upsert(macros::Macro { name: name.clone(), steps });
                        self.x.studio.save();
                        self.x.studio.macro_msgs = vec![format!("Saved \"{name}\".")];
                    }
                }
            }
            if ui.button("Run").clicked() {
                let n = self.x.studio.macro_name.clone();
                if let Err(e) = self.x.studio.start_macro(&n) {
                    self.x.studio.macro_msgs = vec![e];
                }
            }
            if ui.button("Stop all running").clicked() {
                self.x.studio.stop_all_macros();
            }
            if ui.button("Delete").clicked() {
                let n = self.x.studio.macro_name.clone();
                self.x.studio.p.book.remove(&n);
                self.x.studio.save();
            }
        });
        for m in &self.x.studio.macro_msgs {
            ui.label(m);
        }
        ui.separator();
        ui.label("Start this macro by:");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_source("bind-kind").selected_text(["A shortcut", "A spoken phrase"][self.x.studio.bind_kind as usize % 2]).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.x.studio.bind_kind, 0, "A shortcut");
                ui.selectable_value(&mut self.x.studio.bind_kind, 1, "A spoken phrase");
            });
            ui.add(egui::TextEdit::singleline(&mut self.x.studio.bind_value).hint_text(if self.x.studio.bind_kind == 0 { "Cmd+Shift+1" } else { "let us pray" }).desired_width(200.0));
            if ui.button("Bind").clicked() {
                let name = self.x.studio.macro_name.trim().to_string();
                let v = self.x.studio.bind_value.clone();
                let res = if self.x.studio.p.book.get(&name).is_none() {
                    Err("Save the macro first.".to_string())
                } else if self.x.studio.bind_kind == 0 {
                    self.bind_shortcut(&v, &format!("macro:{name}"))
                } else {
                    self.x.studio.p.book.bind(macros::Trigger::Voice(v), &name)
                };
                self.x.studio.macro_msgs = vec![match res {
                    Ok(()) => {
                        self.x.studio.save();
                        "Bound.".into()
                    }
                    Err(e) => e,
                }];
            }
        });
        let trig: Vec<String> = self.x.studio.p.book.triggers.iter().map(|(t, m)| format!("{} -> {m}", match t { macros::Trigger::Voice(p) => format!("saying \"{p}\""), macros::Trigger::Hotkey(k) => k.clone(), other => format!("{other:?}") })).collect();
        for t in trig {
            hint(ui, &t);
        }
    }

    /// Add a shortcut of the operator's own. The app's built-in keys cannot be taken.
    fn bind_shortcut(&mut self, combo: &str, action: &str) -> Result<(), String> {
        let c = hotkeys::Combo::parse(combo)?;
        if hotkeys::Hotkeys::defaults().lookup(&c).is_some() || ["X", "R", "S", "F", "J", "K", "L", "T", "1", "2", "3", "4", "5", "Esc", "F11", "PageUp", "PageDown", "Up", "Down", "Enter"].contains(&c.key.as_str()) && !c.has_modifier() {
            return Err("The app already uses that key. Pick one with Cmd, Ctrl or Alt.".into());
        }
        self.x.studio.p.mine.bind(combo, action).map_err(|e| e.to_string())?;
        if let Some(n) = action.strip_prefix("macro:") {
            let _ = self.x.studio.p.book.bind(macros::Trigger::Hotkey(c.to_string()), n);
        }
        Ok(())
    }

    fn st_library(&mut self, ui: &mut egui::Ui) {
        ui.heading("Song library");
        let locked = self.present.locked;
        ui.label(format!("{} songs", self.present.library.songs().len()));
        ui.collapsing("Duplicates", |ui| {
            ui.horizontal(|ui| {
                ui.label("Words at least");
                ui.add(egui::Slider::new(&mut self.x.studio.dup_threshold, 0.5..=1.0));
            });
            let dups = library_tools::find_duplicates(&self.present.library, self.x.studio.dup_threshold);
            if dups.is_empty() {
                ui.label("No duplicates found.");
            }
            let mut merge: Option<(u64, u64)> = None;
            for d in dups.iter().take(30) {
                let t = |id: u64| self.present.library.get(id).map(|s| s.title.clone()).unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(format!("{}  =  {}  ({:.0}%{})", t(d.ids.0), t(d.ids.1), d.score * 100.0, if d.same_title { ", same title" } else { "" }));
                    if !locked && ui.small_button("Merge into the first").clicked() {
                        merge = Some(d.ids);
                    }
                });
            }
            if let Some((keep, drop)) = merge {
                let st = &mut self.x.studio;
                match library_tools::merge(&mut self.present.library, &mut st.p.meta, keep, drop) {
                    Ok(n) => {
                        self.present.save_library();
                        st.save();
                        st.lib_msg = format!("Merged ({n} part(s) added).");
                    }
                    Err(e) => st.lib_msg = e,
                }
            }
        });
        ui.collapsing("Find and replace in every song", |ui| {
            ui.horizontal(|ui| {
                ui.label("Find");
                ui.text_edit_singleline(&mut self.x.studio.find);
                ui.label("Replace with");
                ui.text_edit_singleline(&mut self.x.studio.replace);
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.x.studio.find_case, "Match capitals");
                ui.checkbox(&mut self.x.studio.find_word, "Whole words only");
                if ui.button("Preview").clicked() {
                    let st = &self.x.studio;
                    let r = library_tools::find_replace(&mut self.present.library, &st.find, &st.replace, st.find_case, st.find_word, false);
                    self.x.studio.find_preview = r;
                }
                if !locked && ui.button("Replace everywhere").clicked() {
                    let st = &self.x.studio;
                    let r = library_tools::find_replace(&mut self.present.library, &st.find, &st.replace, st.find_case, st.find_word, true);
                    if !r.is_empty() {
                        self.present.save_library();
                    }
                    self.x.studio.lib_msg = format!("Changed {} song(s), {} place(s).", r.len(), r.iter().map(|x| x.count).sum::<usize>());
                    self.x.studio.find_preview.clear();
                }
            });
            for r in self.x.studio.find_preview.iter().take(20) {
                hint(ui, &format!("{}: {} place(s)", r.title, r.count));
            }
        });
        ui.collapsing("Tags", |ui| match self.present.song_sel {
            None => {
                ui.label("Pick a song in the Songs list first.");
            }
            Some(id) => {
                let title = self.present.library.get(id).map(|s| s.title.clone()).unwrap_or_default();
                ui.label(RichText::new(title).strong());
                let have = self.x.studio.p.meta.songs.get(&id).map(|m| m.tags.clone()).unwrap_or_default();
                ui.horizontal_wrapped(|ui| {
                    for t in &have {
                        if ui.small_button(format!("{t}  x")).clicked() {
                            self.x.studio.p.meta.untag(id, t);
                            self.x.studio.save();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.x.studio.tag_text).hint_text("easter, slow, communion...").desired_width(200.0));
                    if ui.button("Add tag").clicked() {
                        let t = std::mem::take(&mut self.x.studio.tag_text);
                        self.x.studio.p.meta.tag(id, &t);
                        self.x.studio.save();
                    }
                });
            }
        });
        ui.collapsing("Most used (last 90 days)", |ui| {
            let used = self.x.studio.p.meta.most_used(unix_now() - 90 * 86_400, 10);
            if used.is_empty() {
                ui.label("Nothing yet: songs are counted from now on.");
            }
            for (id, n) in used {
                ui.label(format!("{}  x{n}", self.present.library.get(id).map(|s| s.title.clone()).unwrap_or_else(|| "(removed)".into())));
            }
        });
        if !self.x.studio.lib_msg.is_empty() {
            ui.label(&self.x.studio.lib_msg);
        }
    }

    fn st_templates(&mut self, ui: &mut egui::Ui) {
        ui.heading("Service templates");
        hint(ui, "Save the current service order as a template; start next Sunday's from it. Dates written as {date} in an item name are filled in.");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.x.studio.tpl_name).hint_text("Standard Sunday").desired_width(200.0));
            if ui.button("Save current order as template").clicked() {
                let n = self.x.studio.tpl_name.clone();
                self.x.studio.tpl_msg = match self.studio_save_template(&n) {
                    Ok(()) => format!("Saved \"{}\".", n.trim()),
                    Err(e) => e,
                };
            }
        });
        for n in self.studio_template_names() {
            ui.horizontal(|ui| {
                ui.label(&n);
                if ui.small_button("Add its items to the order").clicked() {
                    self.x.studio.tpl_msg = match self.studio_use_template(&n) {
                        Ok(k) => format!("Added {k} item(s)."),
                        Err(e) => e,
                    };
                }
            });
        }
        if !self.x.studio.tpl_msg.is_empty() {
            ui.label(&self.x.studio.tpl_msg);
        }
    }

    fn st_shortcuts(&mut self, ui: &mut egui::Ui) {
        ui.heading("Your shortcuts");
        hint(ui, "Start a macro (macro:Name) or any named action (run:next_slide, run:blackout). Built-in keys cannot be changed here.");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.x.studio.hot_combo).hint_text("Cmd+Shift+1").desired_width(140.0));
            ui.add(egui::TextEdit::singleline(&mut self.x.studio.hot_action).hint_text("macro:Walk-in").desired_width(180.0));
            if ui.button("Add").clicked() {
                let (c, a) = (self.x.studio.hot_combo.clone(), self.x.studio.hot_action.clone());
                self.x.studio.hot_msg = match self.bind_shortcut(&c, &a) {
                    Ok(()) => {
                        self.x.studio.save();
                        "Added.".into()
                    }
                    Err(e) => e,
                };
            }
        });
        let list: Vec<(String, String)> = self.x.studio.p.mine.bindings.iter().map(|b| (b.combo.to_string(), b.action.clone())).collect();
        for (c, a) in list {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&c).monospace());
                ui.label(&a);
                if ui.small_button("Remove").clicked() {
                    self.x.studio.p.mine.unbind(&c);
                    self.x.studio.save();
                }
            });
        }
        if !self.x.studio.hot_msg.is_empty() {
            ui.label(&self.x.studio.hot_msg);
        }
    }

    fn st_prompter(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        ui.heading("Teleprompter");
        hint(ui, "Sermon notes that scroll at your pace. This is a preview on the operator screen; copy the speed to the preacher's pace by ear.");
        if ui.button("Use the notes of the item that is live").clicked() {
            if let Some((id, _)) = self.order.live() {
                if let Some(it) = self.order.items().iter().find(|i| i.id == id) {
                    self.x.studio.prompt_text = it.notes.clone();
                }
            }
        }
        ui.add(egui::TextEdit::multiline(&mut self.x.studio.prompt_text).desired_rows(4).desired_width(f32::INFINITY));
        let script = teleprompter::Script::from_text(&self.x.studio.prompt_text);
        let (lines, _starts) = script.layout(36);
        let line_px = 34.0;
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut self.x.studio.prompt_wpm, teleprompter::MIN_WPM..=teleprompter::MAX_WPM).text("words a minute"));
            ui.checkbox(&mut self.x.studio.prompt_mirror, "Mirror");
            let label = if self.x.studio.prompt_running { "Pause" } else { "Start" };
            if ui.button(label).clicked() {
                self.x.studio.prompt_running = !self.x.studio.prompt_running;
            }
            if ui.button("Back to top").clicked() {
                self.x.studio.prompt_pos = 0.0;
            }
        });
        ui.label(format!("{} words, about {} minutes at this pace.", script.words(), script.duration_ms(self.x.studio.prompt_wpm) / 60_000));
        let total = lines.len() as f32 * line_px;
        self.x.studio.prompt_tick(7.0, line_px, total);
        let pos = self.x.studio.prompt_pos;
        let mirror = self.x.studio.prompt_mirror;
        egui::ScrollArea::vertical().max_height(260.0).vertical_scroll_offset(pos).auto_shrink([false, false]).show(ui, |ui| {
            for l in &lines {
                let mut t = RichText::new(if l.is_empty() { " " } else { l.as_str() }).size(26.0);
                if mirror {
                    t = t.italics();
                }
                ui.add_sized([ui.available_width(), line_px], egui::Label::new(t));
            }
        });
    }

    fn st_monitor(&mut self, ui: &mut egui::Ui) {
        ui.heading("Stage monitor");
        hint(ui, "What the preacher, the band or the tech desk would see: now, next, clock, sermon timer, notes, alerts. Open it in its own window to drag onto another screen.");
        ui.horizontal(|ui| {
            for r in stagemon::Role::ALL {
                if ui.selectable_label(self.x.studio.p.monitor_role == r as u8, r.label()).clicked() {
                    self.x.studio.p.monitor_role = r as u8;
                    self.x.studio.save();
                }
            }
        });
        ui.checkbox(&mut self.x.studio.monitor_open, "Show the stage monitor");
        ui.checkbox(&mut self.x.studio.monitor_own_window, "In its own window");
    }

    /// The stage monitor itself: drawn inside a window of ours, or in a separate operating-system window.
    pub fn studio_monitor(&mut self, ctx: &egui::Context) {
        if !self.x.studio.monitor_open {
            return;
        }
        let role = stagemon::Role::ALL.get(self.x.studio.p.monitor_role as usize).copied().unwrap_or(stagemon::Role::Preacher);
        let layout = stagemon::layout_for(role);
        let drawn = stagemon::resolve(&layout, &self.studio_stage_ctx());
        let bg = layout.background;
        let title = format!("Stage monitor: {}", role.label());
        if self.x.studio.monitor_own_window {
            let mut close = false;
            ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("rf-stage-monitor"), egui::ViewportBuilder::default().with_title(title).with_inner_size([960.0, 540.0]), |ctx, _class| {
                egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::from_rgb(bg[0], bg[1], bg[2]))).show(ctx, |ui| paint_monitor(ui, &drawn, bg));
                if ctx.input(|i| i.viewport().close_requested()) {
                    close = true;
                }
            });
            if close {
                self.x.studio.monitor_open = false;
            }
        } else {
            let mut open = true;
            egui::Window::new(title).open(&mut open).default_size([480.0, 290.0]).show(ctx, |ui| paint_monitor(ui, &drawn, bg));
            if !open {
                self.x.studio.monitor_open = false;
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

fn paint_monitor(ui: &mut egui::Ui, drawn: &[stagemon::Drawn], bg: [u8; 3]) {
    use rf_studio::layout::{Source, VAlign};
    let avail = ui.available_size();
    let (w, h) = if avail.x / avail.y > 16.0 / 9.0 { (avail.y * 16.0 / 9.0, avail.y) } else { (avail.x, avail.x * 9.0 / 16.0) };
    let (resp, painter) = ui.allocate_painter(egui::vec2(w.max(10.0), h.max(10.0)), egui::Sense::hover());
    let canvas = resp.rect;
    painter.rect_filled(canvas, 0.0, Color32::from_rgb(bg[0], bg[1], bg[2]));
    for d in drawn {
        let r = egui::Rect::from_min_size(canvas.min + egui::vec2(d.rect.x * canvas.width(), d.rect.y * canvas.height()), egui::vec2(d.rect.w * canvas.width(), d.rect.h * canvas.height()));
        let alpha = (d.opacity.clamp(0.0, 1.0) * 255.0) as u8;
        match (&d.source, &d.text) {
            (Source::Shape(c), _) => {
                painter.rect_filled(r, 4.0, Color32::from_rgba_unmultiplied(c[0], c[1], c[2], ((c[3] as f32) * d.opacity) as u8));
            }
            (_, Some(text)) if !text.trim().is_empty() => {
                let col = Color32::from_rgba_unmultiplied(d.style.rgb[0], d.style.rgb[1], d.style.rgb[2], alpha);
                let mut px = d.style.size_pt * canvas.height() / 1080.0;
                let family = if d.style.font == "mono" { egui::FontFamily::Monospace } else { egui::FontFamily::Proportional };
                let align = match d.style.align {
                    rf_service::Align::Left => egui::Align::LEFT,
                    rf_service::Align::Center => egui::Align::Center,
                    rf_service::Align::Right => egui::Align::RIGHT,
                };
                let galley = loop {
                    let mut job = egui::text::LayoutJob::simple(text.clone(), egui::FontId::new(px.max(5.0), family.clone()), col, r.width());
                    job.halign = align;
                    let g = painter.layout_job(job);
                    if g.size().y <= r.height() || px <= 6.0 || !d.style.auto_fit {
                        break g;
                    }
                    px *= 0.9;
                };
                let y = match d.style.valign {
                    VAlign::Top => r.top(),
                    VAlign::Middle => r.center().y - galley.size().y / 2.0,
                    VAlign::Bottom => r.bottom() - galley.size().y,
                };
                let x = match d.style.align {
                    rf_service::Align::Left => r.left(),
                    rf_service::Align::Center => r.center().x,
                    rf_service::Align::Right => r.right(),
                };
                painter.with_clip_rect(r.expand(2.0)).galley(egui::pos2(x, y), galley, col);
            }
            _ => {}
        }
    }
}

/// Alerts over the picture: one bar per message across the top, in the colour of its kind.
pub fn draw_alerts(ui: &egui::Ui, rect: egui::Rect, lines: &[(String, [u8; 3])]) {
    let h = rect.height() * 0.085;
    for (i, (text, c)) in lines.iter().enumerate() {
        let bar = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, i as f32 * h), egui::vec2(rect.width(), h));
        let p = ui.painter();
        p.rect_filled(bar, 0.0, Color32::from_rgba_unmultiplied(c[0] / 2, c[1] / 2, c[2] / 2, 235));
        let font = egui::FontId::proportional((h * 0.55).clamp(14.0, 64.0));
        p.text(bar.center(), egui::Align2::CENTER_CENTER, text, font, Color32::WHITE);
    }
}

/// A QR code drawn with plain squares (black on white, with the quiet border scanners need).
pub fn draw_qr(ui: &mut egui::Ui, data: &str, size: f32) {
    let Some(m) = crate::overlay::qr_modules(data) else { return };
    let total = m.len() + 8;
    let cell = size / total as f32;
    let (resp, p) = ui.allocate_painter(egui::vec2(size, size), egui::Sense::hover());
    p.rect_filled(resp.rect, 0.0, Color32::WHITE);
    for (y, row) in m.iter().enumerate() {
        for (x, dark) in row.iter().enumerate() {
            if *dark {
                let tl = resp.rect.min + egui::vec2((x + 4) as f32 * cell, (y + 4) as f32 * cell);
                p.rect_filled(egui::Rect::from_min_size(tl, egui::vec2(cell + 0.5, cell + 0.5)), 0.0, Color32::BLACK);
            }
        }
    }
}
