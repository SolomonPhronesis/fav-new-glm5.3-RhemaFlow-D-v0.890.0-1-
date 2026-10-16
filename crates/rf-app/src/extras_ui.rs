//! The "Tools" window for the v0.835.0 features, and the "restore where you were" window.

use crate::access::{Contrast, StageFont};
use crate::app::RhemaFlowApp;
use crate::extras::{unix_now, XTab};
use crate::overlay::L3Style;
use crate::runsheet::{clock_text, RunStep, StepAction};
use crate::screens::{Role, ScreenProfile};

fn hint(ui: &mut egui::Ui, t: &str) {
    ui.label(egui::RichText::new(t).weak().small());
}

fn rgb(ui: &mut egui::Ui, c: &mut [u8; 3]) -> bool {
    ui.color_edit_button_srgb(c).changed()
}

impl RhemaFlowApp {
    pub fn ui_extras(&mut self, ctx: &egui::Context) {
        if self.x.recovery_offer.is_some() {
            self.ui_recovery(ctx);
        }
        if !self.x.open {
            return;
        }
        let mut open = true;
        let locked = self.x.is_locked();
        egui::Window::new("Tools").open(&mut open).default_width(560.0).default_height(520.0).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for t in XTab::ALL {
                    if locked && !XTab::LOCKED_OK.contains(&t) {
                        continue;
                    }
                    if ui.selectable_label(self.x.tab == t, t.label()).clicked() {
                        self.x.tab = t;
                    }
                }
            });
            ui.separator();
            if locked && !XTab::LOCKED_OK.contains(&self.x.tab) {
                self.x.tab = XTab::Lock;
            }
            egui::ScrollArea::vertical().show(ui, |ui| match self.x.tab {
                XTab::Songs => self.xt_songs(ui),
                XTab::LowerThird => self.xt_lower_third(ui),
                XTab::Cards => self.xt_cards(ui),
                XTab::Announce => self.xt_announce(ui),
                XTab::RunSheet => self.xt_runsheet(ui, ctx),
                XTab::ReadAlong => self.xt_readalong(ui),
                XTab::Clips => self.xt_clips(ui),
                XTab::Handout => self.xt_handout(ui),
                XTab::Looks => self.xt_looks(ui, ctx),
                XTab::Translate => self.xt_translate(ui),
                XTab::Controllers => self.xt_controllers(ui),
                XTab::Access => self.xt_access(ui),
                XTab::Backup => self.xt_backup(ui),
                XTab::Lock => self.xt_lock(ui),
                XTab::Studio => self.xt_studio(ui, ctx),
            });
        });
        if !open {
            self.x.open = false;
            self.x.save();
        }
    }

    fn xt_songs(&mut self, ui: &mut egui::Ui) {
        ui.heading("Song words by pasting");
        ui.label("Paste the words of a song, give it a title, and it becomes a song you can show. Lines like [Verse 1] or [Chorus] (or Verse 1: ...) make the parts; a blank line also splits parts.");
        self.present.paste_form(ui);
        hint(ui, "The same box is in the Songs tab of the media dock (\"Paste lyrics\").");
    }

    fn xt_lower_third(&mut self, ui: &mut egui::Ui) {
        ui.heading("Lower thirds");
        ui.label("A name bar over the bottom of the screen: speaker, scripture reference, or song title.");
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut self.x.p.lt_name);
        });
        ui.horizontal(|ui| {
            ui.label("Role");
            ui.text_edit_singleline(&mut self.x.p.lt_role);
        });
        ui.horizontal(|ui| {
            ui.label("Style");
            for s in L3Style::ALL {
                if ui.selectable_label(self.x.lt.style == s, s.label()).clicked() {
                    self.x.lt.style = s;
                    self.x.p.lt_style = s.key().into();
                    changed = true;
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.checkbox(&mut self.x.lt.left, "Left side").changed() {
                self.x.p.lt_left = self.x.lt.left;
                changed = true;
            }
            ui.label("Colour");
            if rgb(ui, &mut self.x.p.lt_accent) {
                self.x.lt.accent = self.x.p.lt_accent;
                changed = true;
            }
            ui.label("Stays for");
            if ui.add(egui::DragValue::new(&mut self.x.p.lt_hold_secs).clamp_range(2..=60).suffix(" s")).changed() {
                self.x.lt.hold_ms = self.x.p.lt_hold_secs as u64 * 1000;
                changed = true;
            }
        });
        if ui.checkbox(&mut self.x.p.auto_ref_lt, "Show the reference as a lower third when a verse goes up").changed() {
            changed = true;
        }
        ui.horizontal(|ui| {
            if ui.button("Show speaker").clicked() {
                let (a, b) = crate::overlay::speaker_lines(&self.x.p.lt_name, &self.x.p.lt_role);
                let h = self.x.p.lt_hold_secs as u64 * 1000;
                self.x_show_lower_third(a, b, h);
            }
            if ui.button("Show this scripture").clicked() && !self.stage.label.is_empty() {
                let (a, b) = crate::overlay::scripture_lines(&self.stage.label, &self.stage.translation);
                let h = self.x.p.lt_hold_secs as u64 * 1000;
                self.x_show_lower_third(a, b, h);
            }
            if ui.button("Hide").clicked() {
                let now = self.x.now_ms();
                self.x.lt.hide(now);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Song");
            let infos = self.present.song_infos();
            egui::ComboBox::from_id_source("lt-song").selected_text("pick a song").show_ui(ui, |ui| {
                for (_, t, a, c) in &infos {
                    if ui.selectable_label(false, t).clicked() {
                        let (l1, l2) = crate::overlay::song_lines(t, a, c);
                        let h = self.x.p.lt_hold_secs as u64 * 1000;
                        self.x_show_lower_third(l1, l2, h);
                    }
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Any text");
            ui.text_edit_singleline(&mut self.x.lt.line1);
            ui.text_edit_singleline(&mut self.x.lt.line2);
            if ui.button("Show").clicked() {
                let (a, b, h) = (self.x.lt.line1.clone(), self.x.lt.line2.clone(), self.x.p.lt_hold_secs as u64 * 1000);
                self.x_show_lower_third(a, b, h);
            }
        });
        if changed {
            self.x.save();
        }
    }

    fn xt_cards(&mut self, ui: &mut egui::Ui) {
        ui.heading("Giving card with QR code");
        let mut ch = false;
        ui.horizontal(|ui| {
            ui.label("Headline");
            ch |= ui.text_edit_singleline(&mut self.x.p.giving_headline).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Message");
            ch |= ui.text_edit_singleline(&mut self.x.p.giving_message).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Web address");
            ch |= ui.text_edit_singleline(&mut self.x.p.giving_url).changed();
        });
        let on = self.x.card_src == crate::extras::CardSrc::Giving;
        if ui.button(if on { "Hide giving card" } else { "Show giving card" }).clicked() {
            self.x_toggle_giving();
        }
        hint(ui, "The QR code is drawn by RhemaFlow itself (nothing is sent anywhere). Test it with a phone before Sunday.");
        ui.separator();
        ui.heading("Any other card");
        ui.horizontal(|ui| {
            ui.label("Title");
            ui.text_edit_singleline(&mut self.x.custom_title);
        });
        ui.add(egui::TextEdit::multiline(&mut self.x.custom_lines).desired_rows(4).hint_text("One line per row"));
        ui.horizontal(|ui| {
            if ui.button("Show card").clicked() {
                self.x_show_custom_card();
            }
            if ui.button("Hide card").clicked() {
                self.x_hide_card();
            }
        });
        if ch {
            self.x.save();
        }
    }

    fn xt_announce(&mut self, ui: &mut egui::Ui) {
        ui.heading("Announcement loop");
        ui.label("Notices that rotate on the screen before the service. One notice per paragraph; the first line is the title.");
        let r = ui.add(egui::TextEdit::multiline(&mut self.x.p.announce_text).desired_rows(8).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            ui.label("Each notice stays");
            let c = ui.add(egui::DragValue::new(&mut self.x.p.announce_secs).clamp_range(3..=120).suffix(" s")).changed();
            if r.changed() || c {
                self.x.announce.items = crate::announce::parse(&self.x.p.announce_text, self.x.p.announce_secs);
                self.x.save();
            }
        });
        let n = self.x.announce.items.len();
        ui.label(format!("{n} notices"));
        ui.horizontal(|ui| {
            let label = if self.x.announce.running { "Stop loop" } else { "Start loop" };
            if ui.button(label).clicked() {
                self.x_action(ui.ctx(), "announce_toggle");
            }
            if ui.button("Next notice").clicked() {
                self.x_action(ui.ctx(), "announce_next");
            }
        });
        hint(ui, "If you put anything else on the screen, the loop stops by itself so it never covers your content.");
    }

    fn xt_runsheet(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Run-sheet");
        let now = self.x.now_ms();
        ui.horizontal(|ui| {
            if ui.checkbox(&mut self.x.runsheet.auto, "Move on by itself when a step's time is up").changed() {
                self.x.p.run_auto = self.x.runsheet.auto;
                self.x.save();
            }
        });
        ui.horizontal(|ui| {
            if self.x.runsheet.is_running() {
                if ui.button("Next step").clicked() {
                    self.x_action(ctx, "runsheet_next");
                }
                if ui.button("Previous").clicked() {
                    self.x_action(ctx, "runsheet_prev");
                }
                if ui.button(if self.x.runsheet.is_paused() { "Resume clock" } else { "Pause clock" }).clicked() {
                    self.x_action(ctx, "runsheet_pause");
                }
                if ui.button("Stop").clicked() {
                    self.x.runsheet.stop();
                }
            } else if ui.button("Start run-sheet").clicked() {
                if let Some(i) = self.x.runsheet.start(now) {
                    self.x_apply_step(ctx, i);
                }
            }
        });
        if let Some(rem) = self.x.runsheet.remaining_ms(now) {
            ui.label(format!("Time left in this step: {}", clock_text(rem / 1000)));
        }
        if self.live {
            if let Some(b) = self.x.runsheet.behind_secs(now, 0) {
                let _ = b;
            }
        }
        ui.separator();
        let mut remove: Option<usize> = None;
        let mut mv: Option<(usize, i32)> = None;
        let mut go: Option<usize> = None;
        let cur = self.x.runsheet.current;
        let total = self.x.runsheet.steps.len();
        for i in 0..total {
            ui.horizontal(|ui| {
                let mark = if cur == Some(i) { "\u{25B6}" } else { " " };
                ui.label(mark);
                ui.label(format!("{}", clock_text(self.x.runsheet.planned_start(i) as i64)));
                let s = &mut self.x.runsheet.steps[i];
                ui.add(egui::TextEdit::singleline(&mut s.title).desired_width(160.0));
                let mut mins = (s.secs / 60) as u32;
                if ui.add(egui::DragValue::new(&mut mins).clamp_range(0..=180).suffix(" min")).changed() {
                    s.secs = mins * 60;
                }
                ui.label(egui::RichText::new(step_label(&s.action)).weak());
                if ui.small_button("Go").clicked() {
                    go = Some(i);
                }
                if ui.small_button("\u{2191}").clicked() {
                    mv = Some((i, -1));
                }
                if ui.small_button("\u{2193}").clicked() {
                    mv = Some((i, 1));
                }
                if ui.small_button("x").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some((i, d)) = mv {
            self.x.runsheet.move_step(i, d);
        }
        if let Some(i) = remove {
            self.x.runsheet.remove_step(i);
        }
        if let Some(i) = go {
            if let Some(k) = self.x.runsheet.go(i, now) {
                self.x_apply_step(ctx, k);
            }
        }
        ui.label(format!("Total: {} min", self.x.runsheet.total_secs() / 60));
        ui.separator();
        ui.label("Add a step");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.x.step_title).on_hover_text("Title, or a scripture like John 3:16");
            ui.add(egui::DragValue::new(&mut self.x.step_mins).clamp_range(1..=180).suffix(" min"));
            egui::ComboBox::from_id_source("stepkind").selected_text(["Note", "Scripture", "Slide", "Clear screen", "Announcements", "Giving card"][self.x.step_kind]).show_ui(ui, |ui| {
                for (k, n) in ["Note", "Scripture", "Slide", "Clear screen", "Announcements", "Giving card"].iter().enumerate() {
                    ui.selectable_value(&mut self.x.step_kind, k, *n);
                }
            });
            if ui.button("Add").clicked() && !self.x.step_title.trim().is_empty() {
                let t = self.x.step_title.trim().to_string();
                let action = match self.x.step_kind {
                    1 => StepAction::Scripture(t.clone()),
                    2 => StepAction::Slide(self.x.step_text.clone()),
                    3 => StepAction::Clear,
                    4 => StepAction::Announcements,
                    5 => StepAction::Giving,
                    _ => StepAction::Note,
                };
                self.x.runsheet.steps.push(RunStep::new(&t, self.x.step_mins * 60, action));
                self.x.step_title.clear();
            }
        });
        if self.x.step_kind == 2 {
            ui.add(egui::TextEdit::multiline(&mut self.x.step_text).desired_rows(2).hint_text("Words for the slide"));
        }
        if ui.button("Fill from the service order").clicked() {
            self.x_runsheet_from_order();
        }
        ui.separator();
        ui.heading("Plan from sermon notes");
        ui.label("Paste your notes. Scriptures found in them become steps (and go on the next-scripture list); headings become timed points.");
        ui.add(egui::TextEdit::multiline(&mut self.x.p.sermon_notes).desired_rows(6).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            ui.label("Scripture");
            ui.add(egui::DragValue::new(&mut self.x.p.scripture_secs).clamp_range(10..=1800).suffix(" s"));
            ui.label("Point");
            ui.add(egui::DragValue::new(&mut self.x.p.step_secs).clamp_range(30..=3600).suffix(" s"));
            if ui.button("Make the run-sheet").clicked() {
                let notes = self.x.p.sermon_notes.clone();
                self.x_autoplan(&notes);
                self.x.save();
            }
        });
        for m in &self.x.autoplan_msgs {
            ui.label(m);
        }
    }

    fn xt_readalong(&mut self, ui: &mut egui::Ui) {
        ui.heading("Read-along");
        ui.label("Shows the whole chapter on the screen and highlights the verse being read. It follows what the preacher says (if the microphone is on) or you step it by hand.");
        let on = self.x.ra.is_some();
        ui.horizontal(|ui| {
            if ui.button(if on { "Stop read-along" } else { "Start from the verse on screen" }).clicked() {
                self.x_action(ui.ctx(), "readalong_toggle");
            }
            if on {
                if ui.button("\u{2193} Next verse").clicked() {
                    self.x_ra_step(1);
                }
                if ui.button("\u{2191} Back").clicked() {
                    self.x_ra_step(-1);
                }
            }
        });
        hint(ui, "Put the verse up first (it reads that chapter). Voice-following needs the live transcript running.");
    }

    fn xt_clips(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sermon clips");
        ui.label("Mark good moments while you go. Each clip keeps its words as a caption file; if the recording is there, the sound is cut too.");
        ui.horizontal(|ui| {
            if ui.button("Start here").clicked() {
                self.x_action(ui.ctx(), "clip_start");
            }
            if ui.button("End here").clicked() {
                self.x_action(ui.ctx(), "clip_end");
            }
            if ui.button("Keep last 30 s").clicked() {
                self.x_action(ui.ctx(), "clip_quick");
            }
        });
        let mut remove = None;
        for c in &mut self.x.clips.clips {
            ui.horizontal(|ui| {
                ui.label(format!("{}\u{2013}{}", crate::export::hms(c.start_ms), crate::export::hms(c.end_ms)));
                ui.text_edit_singleline(&mut c.title);
                if ui.small_button("x").clicked() {
                    remove = Some(c.id);
                }
            });
        }
        if let Some(id) = remove {
            self.x.clips.remove(id);
        }
        if !self.x.clips.clips.is_empty() && ui.button("Save clips to the service folder").clicked() {
            self.x_export_clips();
        }
        if !self.x.clips_msg.is_empty() {
            ui.label(&self.x.clips_msg);
        }
    }

    fn xt_handout(&mut self, ui: &mut egui::Ui) {
        ui.heading("After-service handout");
        ui.label("A one-page sheet for the congregation: the scriptures shown, the songs, your notes, next steps and the giving link. Saved as a web page (print it or send it) and as plain text.");
        ui.label("Notes / key points");
        ui.add(egui::TextEdit::multiline(&mut self.x.p.handout_notes).desired_rows(4).desired_width(f32::INFINITY));
        ui.label("Next steps (one per line)");
        ui.add(egui::TextEdit::multiline(&mut self.x.p.next_steps).desired_rows(3).desired_width(f32::INFINITY));
        if ui.button("Make the handout").clicked() {
            self.x_build_handout();
            self.x.save();
        }
        if !self.x.handout_msg.is_empty() {
            ui.label(&self.x.handout_msg);
        }
    }

    fn xt_looks(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Sermon-series looks");
        ui.label("Save the colours, style and size as a named look, and switch with one click.");
        let names: Vec<String> = self.x.themes.themes.iter().map(|t| t.name.clone()).collect();
        let active = self.x.themes.active.clone();
        let mut apply = None;
        let mut remove = None;
        for n in &names {
            ui.horizontal(|ui| {
                let is = active.as_deref() == Some(n.as_str());
                if ui.selectable_label(is, n).clicked() {
                    apply = Some(n.clone());
                }
                if ui.small_button("delete").clicked() {
                    remove = Some(n.clone());
                }
            });
        }
        if let Some(n) = apply {
            self.x_apply_theme(ctx, &n);
        }
        if let Some(n) = remove {
            self.x.themes.remove(&n);
            self.x.save();
        }
        ui.horizontal(|ui| {
            ui.label("Series title");
            ui.text_edit_singleline(&mut self.x.p.series_title);
        });
        ui.horizontal(|ui| {
            ui.label("Save this look as");
            ui.text_edit_singleline(&mut self.x.theme_name);
            if ui.button("Save").clicked() && !self.x.theme_name.trim().is_empty() {
                let t = self.x_current_look_as_theme(&self.x.theme_name.clone());
                if let Some(p) = t.problem() {
                    self.x.lock_msg = p;
                }
                self.x.themes.save(t);
                self.x.save();
            }
        });
        if !self.x.lock_msg.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(220, 160, 60), &self.x.lock_msg.clone());
        }
        ui.separator();
        ui.heading("Different looks per screen");
        ui.label("Add lobby, overflow room or livestream screens. Each has its own content, colours and size.");
        let main_names: Vec<String> = self.displays.iter().map(|d| d.name.clone()).collect();
        let mut remove = None;
        let n = self.x.p.screens.len();
        for i in 0..n {
            ui.group(|ui| {
                let p = &mut self.x.p.screens[i];
                ui.horizontal(|ui| {
                    ui.checkbox(&mut p.enabled, "On");
                    ui.text_edit_singleline(&mut p.name);
                    if ui.small_button("remove").clicked() {
                        remove = Some(i);
                    }
                });
                ui.label(egui::RichText::new(p.role.blurb()).weak().small());
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_source(("scr-d", i)).selected_text(p.display.clone().unwrap_or_else(|| "next free screen".into())).show_ui(ui, |ui| {
                        if ui.selectable_label(p.display.is_none(), "next free screen").clicked() {
                            p.display = None;
                        }
                        for d in &main_names {
                            if ui.selectable_label(p.display.as_deref() == Some(d.as_str()), d).clicked() {
                                p.display = Some(d.clone());
                            }
                        }
                    });
                    ui.checkbox(&mut p.fullscreen, "Full screen");
                    ui.checkbox(&mut p.captions, "Captions");
                    ui.checkbox(&mut p.lower_thirds, "Lower thirds");
                });
                ui.horizontal(|ui| {
                    ui.label("Size");
                    ui.add(egui::Slider::new(&mut p.zoom, 0.5..=2.0));
                });
            });
        }
        if let Some(i) = remove {
            self.x.p.screens.remove(i);
        }
        ui.horizontal(|ui| {
            for r in Role::ALL {
                if ui.button(format!("+ {}", r.label())).clicked() {
                    self.x.p.screens.push(ScreenProfile::for_role(r));
                }
            }
        });
        if ui.button("Save screens").clicked() {
            self.x.save();
        }
        hint(ui, "Extra screens could not be tested with real second monitors here; try them before a service.");
    }

    fn xt_translate(&mut self, ui: &mut egui::Ui) {
        ui.heading("Live translation of captions");
        ui.label("Each caption line is sent to a translation service you choose, and the translated words show on the screen. Nothing is sent unless this is ticked.");
        let mut ch = ui.checkbox(&mut self.x.p.tr_enabled, "Translate captions").changed();
        ui.horizontal(|ui| {
            ui.label("Service");
            for p in crate::translate::Provider::ALL {
                if ui.selectable_label(self.x.p.tr_provider == p.key(), p.label()).clicked() {
                    self.x.p.tr_provider = p.key().into();
                    self.x.p.tr_endpoint = p.default_endpoint().into();
                    ch = true;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Address");
            ch |= ui.text_edit_singleline(&mut self.x.p.tr_endpoint).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Key");
            ch |= ui.add(egui::TextEdit::singleline(&mut self.x.p.tr_key).password(true)).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Into (language code, e.g. es, fr, pt, pl)");
            ch |= ui.text_edit_singleline(&mut self.x.p.tr_target).changed();
        });
        if let Ok(e) = self.x.tr_error.lock() {
            if let Some(e) = e.as_ref() {
                ui.colored_label(egui::Color32::from_rgb(220, 120, 90), format!("Last problem: {e}"));
            }
        }
        hint(ui, "Captions must be on for this to work. Not tested against a live service here; use the Test button with your key.");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.x.tr_test);
            if ui.button("Test").clicked() && !self.x.tr_test.trim().is_empty() {
                let cfg = self.x.translate_cfg();
                let (shared, err) = (self.x.tr_shared.clone(), self.x.tr_error.clone());
                let text = self.x.tr_test.clone();
                std::thread::spawn(move || match crate::translate::translate_blocking(&cfg, &text) {
                    Ok(t) => {
                        if let Ok(mut g) = shared.lock() {
                            *g = (text, t);
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
        });
        let g = self.x.tr_shared.lock().map(|g| g.clone()).unwrap_or_default();
        if !g.1.is_empty() {
            ui.label(format!("Result: {}", g.1));
        }
        if ch {
            self.x.save();
        }
    }

    fn xt_controllers(&mut self, ui: &mut egui::Ui) {
        ui.heading("Foot pedal, MIDI and Stream Deck");
        ui.label("Foot pedal: most send Page Down / Page Up. That already moves forward / back (run-sheet step, slide, or verse). Nothing to set up.");
        ui.separator();
        ui.label("MIDI pad or switch");
        ui.horizontal(|ui| {
            if ui.button("Refresh devices").clicked() {
                self.x_midi_refresh_ports();
            }
            let ports = self.x.midi_ports.clone();
            egui::ComboBox::from_id_source("midi-port").selected_text(if self.x.p.midi_port.is_empty() { "first device".to_string() } else { self.x.p.midi_port.clone() }).show_ui(ui, |ui| {
                for p in &ports {
                    if ui.selectable_label(&self.x.p.midi_port == p, p).clicked() {
                        self.x.p.midi_port = p.clone();
                    }
                }
            });
            if self.x.midi_conn.is_some() {
                if ui.button("Disconnect").clicked() {
                    self.x.midi_conn = None;
                    self.x.p.midi_on = false;
                    self.x.midi_status = "Disconnected.".into();
                    self.x.save();
                }
            } else if ui.button("Connect").clicked() {
                self.x_midi_connect();
                self.x.p.midi_on = self.x.midi_conn.is_some();
                self.x.save();
            }
        });
        if !self.x.midi_status.is_empty() {
            ui.label(&self.x.midi_status);
        }
        let entries = self.x.p.midi.entries.clone();
        let mut remove = None;
        for (t, a) in &entries {
            ui.horizontal(|ui| {
                ui.label(t.describe());
                ui.label("\u{2192}");
                ui.label(crate::controllers::label_of(a));
                if ui.small_button("x").clicked() {
                    remove = Some(*t);
                }
            });
        }
        if let Some(t) = remove {
            self.x.p.midi.remove(t);
            self.x.save();
        }
        ui.horizontal(|ui| {
            ui.label("Teach a button:");
            let shown = self.x.learning.as_ref().map(|a| crate::controllers::label_of(a)).unwrap_or_else(|| "choose what it should do".into());
            egui::ComboBox::from_id_source("learn-act").selected_text(shown).show_ui(ui, |ui| {
                for (n, l) in crate::controllers::ACTIONS {
                    if ui.selectable_label(self.x.learning.as_deref() == Some(*n), *l).clicked() {
                        self.x.learning = Some((*n).to_string());
                        self.x.midi_status = "Now press the button on your device...".into();
                    }
                }
            });
        });
        hint(ui, "MIDI was written to the standard but not tried with real hardware here. Tell me which device you have if it misbehaves.");
        ui.separator();
        ui.label("Stream Deck");
        ui.label("Add a \"Website\" button (set it to run in the background) for each action, using these addresses. Turn the phone remote / web server on first (Settings).");
        let pin = self.web_pin.clone();
        let port = self.web_port;
        egui::CollapsingHeader::new("Addresses for each button").show(ui, |ui| {
            for (n, l) in crate::controllers::ACTIONS {
                let url = crate::controllers::stream_deck_url("localhost", port, if pin.is_empty() { "PIN" } else { &pin }, n);
                ui.horizontal(|ui| {
                    ui.label(*l);
                    if ui.small_button("copy").clicked() {
                        ui.ctx().copy_text(url.clone());
                    }
                });
            }
        });
        hint(ui, "From another computer replace localhost with this computer's address on your network.");
    }

    fn xt_access(&mut self, ui: &mut egui::Ui) {
        ui.heading("Accessibility");
        let mut ch = false;
        ch |= ui.checkbox(&mut self.x.p.access.large_print, "Large print on the screen").changed();
        ui.horizontal(|ui| {
            ui.label("Colours");
            for c in [Contrast::Off, Contrast::WhiteOnBlack, Contrast::YellowOnBlack, Contrast::BlackOnWhite] {
                if ui.selectable_label(self.x.p.access.contrast == c, c.label()).clicked() {
                    self.x.p.access.contrast = c;
                    ch = true;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Font");
            for f in [StageFont::Serif, StageFont::Dyslexic, StageFont::Hyperlegible] {
                if ui.selectable_label(self.x.p.access.font == f, f.label()).clicked() {
                    self.x.p.access.font = f;
                    ch = true;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Operator window size");
            ch |= ui.add(egui::Slider::new(&mut self.x.p.access.ui_scale, 0.8..=1.8)).changed();
        });
        if ui.button("Everything on (for a low-vision congregation)").clicked() {
            self.x.p.access = crate::access::Access::everything();
            ch = true;
        }
        if ui.button("Back to normal").clicked() {
            self.x.p.access = crate::access::Access::default();
            ch = true;
        }
        hint(ui, "Fonts: OpenDyslexic and Atkinson Hyperlegible, both free under the SIL Open Font License.");
        if ch {
            self.x.save();
        }
    }

    fn xt_backup(&mut self, ui: &mut egui::Ui) {
        ui.heading("Backup");
        ui.label("Copies your songs, settings, looks, run-sheet settings and media list into one zip file in a folder you choose. Pick a folder that syncs to the cloud (iCloud Drive, Dropbox, OneDrive, Google Drive) and it is off-site too.");
        let mut ch = false;
        ui.horizontal(|ui| {
            ui.label("Folder");
            ch |= ui.text_edit_singleline(&mut self.x.p.backup_dir).changed();
            if ui.button("Choose...").clicked() && self.x.pick_rx.is_none() {
                let (tx, rx) = std::sync::mpsc::channel();
                self.x.pick_rx = Some(rx);
                std::thread::spawn(move || {
                    let _ = tx.send(crate::export::choose_folder_dialog());
                });
            }
        });
        if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) {
            for (n, p) in crate::backup::cloud_folders(&home) {
                if ui.small_button(format!("Use {n}")).clicked() {
                    self.x.p.backup_dir = p.join("RhemaFlow backups").to_string_lossy().to_string();
                    ch = true;
                }
            }
        }
        for (n, p) in crate::backup::usb_volumes(std::path::Path::new("/Volumes")) {
            if ui.small_button(format!("Use drive: {n}")).on_hover_text("Backs up to this USB stick or drive, pictures and videos included").clicked() {
                self.x.p.backup_dir = p.join("RhemaFlow backups").to_string_lossy().to_string();
                self.x.p.backup_media = true;
                ch = true;
            }
        }
        ch |= ui.checkbox(&mut self.x.p.backup_media, "Include the picture / video files themselves (can be huge)").changed();
        ch |= ui.checkbox(&mut self.x.p.auto_backup, "Back up automatically once a day (when you start the app, never during a service)").changed();
        ui.horizontal(|ui| {
            ui.label("Keep the newest");
            ch |= ui.add(egui::DragValue::new(&mut self.x.p.backup_keep).clamp_range(1..=100)).changed();
        });
        let busy = self.x.backup_busy.load(std::sync::atomic::Ordering::Relaxed);
        ui.horizontal(|ui| {
            if ui.add_enabled(!busy, egui::Button::new("Back up now")).clicked() {
                self.x_backup_now();
            }
            if ui.add_enabled(!busy, egui::Button::new("Restore the latest")).clicked() {
                self.x_restore_backup();
            }
        });
        if busy {
            ui.label("Backing up...");
        }
        if !self.x.backup_msg.is_empty() {
            ui.label(&self.x.backup_msg);
        }
        if self.x.p.last_backup > 0 {
            let days = (unix_now() - self.x.p.last_backup) / 86400;
            ui.label(format!("Last backup: {} day(s) ago", days));
        }
        hint(ui, "Your licence ticket is never put in a backup. \"Cloud\" here means a folder that your own sync app copies up; RhemaFlow does not log in to any cloud itself.");
        if ch {
            self.x.save();
        }
    }

    fn xt_lock(&mut self, ui: &mut egui::Ui) {
        ui.heading("Volunteer lock");
        ui.label("When locked, a helper can move the screen on (verses, slides, run-sheet, lower thirds) but cannot end the service, delete songs or media, change settings, or close the app. A PIN unlocks it.");
        let now = self.x.now_ms();
        if self.x.p.lock.locked {
            ui.label("Locked.");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.x.pin_try).password(true).hint_text("PIN"));
                if ui.button("Unlock").clicked() {
                    let pin = self.x.pin_try.clone();
                    match self.x.p.lock.unlock(&pin, now) {
                        Ok(()) => {
                            self.x.lock_msg = "Unlocked.".into();
                            self.x.save();
                        }
                        Err(e) => self.x.lock_msg = e,
                    }
                    self.x.pin_try.clear();
                }
            });
        } else {
            if !self.x.p.lock.has_pin() {
                ui.label("Choose a PIN (4 to 8 digits):");
            } else {
                ui.label("A PIN is set. Change it:");
            }
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.x.pin_new).password(true).hint_text("new PIN"));
                if ui.button("Set PIN").clicked() {
                    let p = self.x.pin_new.clone();
                    self.x.lock_msg = match self.x.p.lock.set_pin(&p) {
                        Ok(()) => {
                            self.x.save();
                            "PIN saved. Write it down somewhere safe.".into()
                        }
                        Err(e) => e,
                    };
                    self.x.pin_new.clear();
                }
            });
            if ui.add_enabled(self.x.p.lock.has_pin(), egui::Button::new("Lock now")).clicked() {
                self.x.lock_msg = match self.x.p.lock.lock() {
                    Ok(()) => {
                        self.x.save();
                        "Locked.".into()
                    }
                    Err(e) => e,
                };
            }
        }
        if !self.x.lock_msg.is_empty() {
            ui.label(&self.x.lock_msg);
        }
        hint(ui, "If you forget the PIN, delete the lock line from extras.json in the .rhemaflow folder while the app is closed.");
    }

    pub fn ui_recovery(&mut self, ctx: &egui::Context) {
        let Some(s) = self.x.recovery_offer.clone() else { return };
        egui::Window::new("RhemaFlow did not close properly").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(s.describe(unix_now()));
            ui.label("Put it back the way it was?");
            ui.horizontal(|ui| {
                if ui.button("Restore").clicked() {
                    self.x_restore(ctx);
                }
                if ui.button("Start fresh").clicked() {
                    self.x_dismiss_recovery();
                }
            });
        });
    }
}

fn step_label(a: &StepAction) -> String {
    match a {
        StepAction::Note => "note".into(),
        StepAction::Scripture(s) => format!("scripture: {s}"),
        StepAction::Slide(_) => "slide".into(),
        StepAction::Order(_) => "service order item".into(),
        StepAction::Media(_) => "media".into(),
        StepAction::Announcements => "announcements".into(),
        StepAction::Giving => "giving card".into(),
        StepAction::Clear => "clear screen".into(),
    }
}
