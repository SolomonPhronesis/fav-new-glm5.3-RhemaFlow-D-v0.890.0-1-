//! Command Centre panels (senior UI/UX: one screen, zero scrolling required
//! for the operator's core loop: what is on stage — what did it hear — what
//! needs me).

use crate::app::{save_settings, RhemaFlowApp};
use crate::state::*;
use crate::theme::*;
use std::time::Instant;
use egui::{Align, Color32, ComboBox, RichText, ScrollArea, TextEdit, Ui};

impl RhemaFlowApp {
    // ------------------------------------------------------------------
    // Menu bar across the top of the window: File, Edit, View, Scripture, Songs, Media, Window, Help
    // ------------------------------------------------------------------
    #[allow(dead_code)] // not drawn on the Mac, where the real menu bar is used
    pub fn ui_menubar(&mut self, ctx: &egui::Context) {
        use crate::extras::XTab;
        let mut act: Option<&'static str> = None;
        let mut layout: Option<LayoutMode> = None;
        let mut tools: Option<XTab> = None;
        let mut open_data = false;
        egui::TopBottomPanel::top("menubar").frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(8.0, 2.0))).show(ctx, |ui| {
            fn item(ui: &mut Ui, label: &str, keys: &str, act: &mut Option<&'static str>, name: &'static str) {
                if ui.add(egui::Button::new(label).shortcut_text(keys)).clicked() {
                    *act = Some(name);
                    ui.close_menu();
                }
            }
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    item(ui, "Open / close stage window", "F", &mut act, "toggle_stage");
                    item(ui, "Full screen", "F11", &mut act, "toggle_fullscreen");
                    ui.separator();
                    item(ui, "Songs\u{2026}", "", &mut act, "songs");
                    item(ui, "Slides & timers\u{2026}", "", &mut act, "slides_window");
                    ui.separator();
                    if ui.button("Backup\u{2026}").clicked() {
                        tools = Some(XTab::Backup);
                        ui.close_menu();
                    }
                    if ui.button("Open data folder").clicked() {
                        open_data = true;
                        ui.close_menu();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    item(ui, "Undo edit (list)", "Cmd+Z", &mut act, "undo_edit");
                    item(ui, "Redo edit (list)", "Shift+Cmd+Z", &mut act, "redo_edit");
                    ui.separator();
                    item(ui, "Paste and search scripture", "", &mut act, "paste_search");
                    item(ui, "Search scripture", "/", &mut act, "search");
                    item(ui, "Command palette", "Cmd+K", &mut act, "palette");
                    item(ui, "Save song", "Cmd+S", &mut act, "save_song");
                    ui.separator();
                    item(ui, "Clear the screen", "C", &mut act, "clear");
                    item(ui, "Blackout", "B", &mut act, "blackout");
                });
                ui.menu_button("View", |ui| {
                    for (m, name) in [(LayoutMode::Console, "Layout A \u{b7} Operator console"), (LayoutMode::DockFirst, "Layout B \u{b7} Dock first"), (LayoutMode::Focus, "Layout C \u{b7} Focus")] {
                        if ui.selectable_label(self.layout == m, name).clicked() {
                            layout = Some(m);
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    item(ui, if self.hide_transcript { "Show transcript" } else { "Hide transcript" }, "Cmd+T", &mut act, "toggle_transcript");
                    item(ui, "Bigger text", "", &mut act, "zoom_in");
                    item(ui, "Smaller text", "", &mut act, "zoom_out");
                    ui.separator();
                    item(ui, "Strong's (Greek & Hebrew)", "", &mut act, "strongs");
                    item(ui, "Voice commands list", "", &mut act, "voice_list");
                    item(ui, "Bible versions", "", &mut act, "versions");
                });
                ui.menu_button("Scripture", |ui| {
                    item(ui, "Approve first card", "Space", &mut act, "approve");
                    item(ui, "Dismiss first card", "X", &mut act, "dismiss");
                    ui.separator();
                    item(ui, "Next verse", "\u{2192}", &mut act, "next_verse");
                    item(ui, "Previous verse", "\u{2190}", &mut act, "prev_verse");
                    item(ui, "Next chapter", "\u{2193}", &mut act, "next_chapter");
                    item(ui, "Previous chapter", "\u{2191}", &mut act, "prev_chapter");
                    ui.separator();
                    item(ui, "Go to the reference on the clipboard", "Shift+Cmd+V", &mut act, "paste_go");
                    item(ui, "Copy verse", "", &mut act, "copy_verse");
                    item(ui, "Copy verse with reference", "", &mut act, "copy_verse_ref");
                    item(ui, "Copy reference", "", &mut act, "copy_ref");
                    ui.separator();
                    item(ui, "Previous scripture shown", "[", &mut act, "history_back");
                    item(ui, "Next scripture shown", "]", &mut act, "history_forward");
                    ui.separator();
                    item(ui, "Read aloud", "R", &mut act, "read");
                    item(ui, "Stop reading", "S", &mut act, "stop_reading");
                });
                ui.menu_button("Songs", |ui| {
                    item(ui, "Open Songs", "", &mut act, "songs");
                    item(ui, "Paste lyrics\u{2026}", "", &mut act, "paste_lyrics");
                    ui.separator();
                    item(ui, "Next slide / song part", "PageDown", &mut act, "next_slide");
                    item(ui, "Previous slide / song part", "PageUp", &mut act, "prev_slide");
                });
                ui.menu_button("Media", |ui| {
                    item(ui, "Play / pause", "K", &mut act, "media_toggle");
                    item(ui, "Back 10 seconds", "J", &mut act, "media_back");
                    item(ui, "Forward 10 seconds", "L", &mut act, "media_fwd");
                    item(ui, "Restart", "", &mut act, "media_restart");
                    ui.separator();
                    item(ui, "Add the file whose path is on the clipboard", "", &mut act, "media_paste_path");
                    ui.separator();
                    item(ui, "Select all files", "Cmd+A", &mut act, "media_select_all");
                    item(ui, "Select nothing", "Esc", &mut act, "media_select_none");
                    item(ui, "Copy selected files", "Cmd+C", &mut act, "media_copy");
                    item(ui, "Paste files or a song", "Cmd+V", &mut act, "media_paste");
                    item(ui, "Remove selected files", "Delete", &mut act, "media_remove_selected");
                });
                ui.menu_button("Window", |ui| {
                    if ui.button("Tools (lower thirds, run-sheet, lock\u{2026})").clicked() {
                        tools = Some(XTab::LowerThird);
                        ui.close_menu();
                    }
                    item(ui, "Command palette", "Cmd+K", &mut act, "palette");
                    item(ui, "Songs", "", &mut act, "songs");
                    item(ui, "Slides & timers", "", &mut act, "slides_window");
                });
                // Help is last, like every Mac app
                ui.menu_button("Help", |ui| {
                    item(ui, "Guide: first service, step by step", "F1", &mut act, "help");
                    item(ui, "Search help", "", &mut act, "help_search");
                    item(ui, "Keyboard shortcuts", "", &mut act, "help_keys");
                    item(ui, "Troubleshooting", "", &mut act, "help_trouble");
                    ui.separator();
                    if ui.button("Open data folder").clicked() {
                        open_data = true;
                        ui.close_menu();
                    }
                    item(ui, "About / copy diagnostics", "", &mut act, "about");
                });
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(if self.live { format!("\u{25cf} LIVE   {}", TITLE) } else { TITLE.to_string() }).small().color(if self.live { GOLD } else { MUTED }));
                });
            });
        });
        if open_data {
            act = Some("open_data");
        }
        if let Some(m) = layout {
            act = Some(match m {
                LayoutMode::Console => "layout_a",
                LayoutMode::DockFirst => "layout_b",
                LayoutMode::Focus => "layout_c",
            });
        }
        if let Some(t) = tools {
            act = Some(if t == XTab::Backup { "tools_backup" } else { "tools_lower" });
        }
        if let Some(id) = act {
            self.menu_action(ctx, id);
        }
    }

    /// One place that runs a menu choice, whether it came from the Mac menu bar, the in-window menu bar
    /// (other systems) or the command palette.
    pub fn menu_action(&mut self, ctx: &egui::Context, id: &str) {
        use crate::extras::XTab;
        match id {
            // list edits: when a text box has the cursor, Cmd+Z is the text box's own undo (v0.841.0)
            "undo_edit" | "redo_edit" => {
                if ctx.wants_keyboard_input() {
                    let mut modifiers = egui::Modifiers::COMMAND;
                    modifiers.shift = id == "redo_edit";
                    // v0.849.0: held for the next frame, like Copy / Paste, because windows drawn earlier this frame (Songs) would miss it
                    self.pending_edit_events.push(egui::Event::Key { key: egui::Key::Z, physical_key: Some(egui::Key::Z), pressed: true, repeat: false, modifiers });
                    ctx.request_repaint();
                } else if id == "undo_edit" {
                    self.right_tab = crate::order::RightTab::Order;
                    self.order.undo_edit();
                } else {
                    self.right_tab = crate::order::RightTab::Order;
                    self.order.redo_edit();
                }
            }
            "toggle_transcript" => {
                self.hide_transcript = !self.hide_transcript;
                save_settings(self);
            }
            "paste_go" => self.clip_go_to_reference(),
            "paste_search" => {
                let t = crate::macmenu::read_clipboard();
                let line: String = t.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(200).collect();
                if line.is_empty() {
                    self.short_utterance_note = Some(("The clipboard is empty.".into(), std::time::Instant::now()));
                } else {
                    self.browser.visible = true;
                    self.browser.query = line;
                    self.browser.focus_search = true;
                }
            }
            "save_run_sheet" => {
                let text = self.order.run_sheet_text();
                let note = match crate::fsutil::data_dir().map(|d| d.join("run-sheets")) {
                    Some(dir) if std::fs::create_dir_all(&dir).is_ok() => {
                        let file = dir.join(crate::clip::run_sheet_file_name(&self.order.runner.service().title, crate::fsutil::today_stamp()));
                        match std::fs::write(&file, text) {
                            Ok(()) => {
                                crate::export::open_folder(&dir);
                                format!("Saved the run sheet as {}.", file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
                            }
                            Err(e) => format!("Could not save the run sheet ({e})."),
                        }
                    }
                    _ => "Could not find a place to save the run sheet.".to_string(),
                };
                self.short_utterance_note = Some((note, std::time::Instant::now()));
            }
            "copy_verse" => self.clip_copy_verse(ctx, crate::clip::CopyKind::Text),
            "copy_verse_ref" => self.clip_copy_verse(ctx, crate::clip::CopyKind::WithReference),
            "copy_ref" => self.clip_copy_verse(ctx, crate::clip::CopyKind::Reference),
            "media_paste_path" => self.clip_add_path(ctx),
            "media_select_all" => {
                let m = self.present.dock_select_all();
                self.short_utterance_note = Some((m, std::time::Instant::now()));
            }
            "media_select_none" => self.present.dock_clear_selection(),
            "media_copy" => self.copy_anything(ctx),
            "media_paste" => self.paste_anything(ctx, None),
            "media_remove_selected" => self.present.dock_remove_selected(),
            "palette" => {
                self.palette_open = true;
                self.palette_q.clear();
            }
            "search" => {
                self.browser.visible = true;
                self.browser.focus_search = true;
            }
            "save_song" => {
                if !self.present.save_song_shortcut() {
                    self.present.open_at(crate::present::Tab::Songs);
                }
            }
            "slides_window" => self.present.open_at(crate::present::Tab::Slides),
            "paste_lyrics" => {
                self.present.open_at(crate::present::Tab::Songs);
                self.present.paste_open = true;
            }
            "strongs" => self.strongs.open = !self.strongs.open,
            "voice_list" => self.commands_visible = !self.commands_visible,
            "versions" => self.versions_visible = true,
            "layout_a" | "layout_b" | "layout_c" => {
                self.layout = match id {
                    "layout_a" => LayoutMode::Console,
                    "layout_b" => LayoutMode::DockFirst,
                    _ => LayoutMode::Focus,
                };
                save_settings(self);
            }
            "tools_backup" => {
                self.x.tab = XTab::Backup;
                self.x.open = true;
            }
            "tools_lower" => {
                self.x.tab = XTab::LowerThird;
                self.x.open = true;
            }
            "open_data" => {
                if let Some(d) = crate::fsutil::data_dir() {
                    crate::export::open_folder(&d);
                }
            }
            "help_search" => {
                self.help_visible = true;
                self.help_filter.clear();
            }
            "help_keys" => {
                self.help_visible = true;
                self.help_filter = "keyboard".into();
            }
            "help_trouble" => {
                self.help_visible = true;
                self.help_filter = "troubleshooting".into();
            }
            other => self.run_named(other),
        }
    }

    // ------------------------------------------------------------------
    // Header: identity, state, mic, timer, recording
    // ------------------------------------------------------------------
    pub fn ui_header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(14.0, 10.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(TITLE)
                            .size(19.0)
                            .color(GOLD)
                            .strong(),
                    );
                    ui.separator();

                    // LIVE badge
                    let (live_txt, live_col) = if self.live {
                        ("LIVE", GOLD)
                    } else {
                        ("IDLE", MUTED)
                    };
                    badge(ui, live_txt, live_col);
                    self.account_badge(ui);

                    // REC badge (pulses while recording)
                    if self.recording {
                        let pulse = (self.service_ms() / 600) % 2 == 0;
                        badge(ui, "REC", if pulse { RED } else { Color32::from_rgb(0x7a, 0x2a, 0x26) });
                        if let Some(rec) = &self.recorder {
                            ui.label(RichText::new(format!("{:.0}s", rec.seconds_written())).small().color(MUTED));
                        }
                    }

                    // service timer
                    if self.live {
                        ui.label(RichText::new(mmss(self.service_ms())).size(17.0).monospace());
                    }

                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        // ---- A/B/C layout switch (v0.831.0, from the approved mock-up) ----
                        for (i, m) in LayoutMode::ALL.into_iter().enumerate() {
                            let on = self.layout == m;
                            if ui
                                .selectable_label(on, RichText::new(['A', 'B', 'C'][i]).size(13.0).strong())
                                .on_hover_text(m.hint())
                                .clicked()
                            {
                                self.layout = m;
                                save_settings(self);
                            }
                        }
                        ui.label(RichText::new("layout").small().color(MUTED));
                        ui.separator();

                        // ASR mode chip
                        badge(ui, self.asr_mode.label(), if self.asr_mode.has_model() { GREEN } else { BLUE });

                        // mic meter (7 bars)
                        draw_meter(ui, self.last_level, self.speaking);
                        ui.label(RichText::new("mic").small().color(MUTED));

                        // model status
                        let model_ok = self.asr_mode.has_model();
                        badge(ui, if model_ok { "model ok" } else { "no model" }, if model_ok { GREEN } else { RED });
                    });
                });
            });
    }

    // ------------------------------------------------------------------
    // Live transcript — the blue/green panel (v0.831.0): green lines are
    // detected scriptures, blue lines are voice commands, plain lines are
    // ordinary speech. The panel sits LEFT in layout A, RIGHT in layout C.
    // ------------------------------------------------------------------
    pub fn ui_transcript(&mut self, ctx: &egui::Context, on_right: bool) {
        // WIDTH FIX (v0.811.0): one long line (a whole verse quoted in one breath) used
        // to widen this panel to fit it, squeezing the stage. The width is now bounded
        // and every line wraps inside it.
        let panel = if on_right {
            egui::SidePanel::right("transcript_r")
        } else {
            egui::SidePanel::left("transcript")
        };
        panel
            .resizable(true)
            .default_width((ctx.screen_rect().width() * 0.17).clamp(210.0, 290.0))
            .width_range(200.0..=460.0)
            .frame(egui::Frame::none().fill(TR_TOP).inner_margin(10.0))
            .show(ctx, |ui| {
                // the blue → green wash over the whole panel (including the frame margin)
                let rect = ui.max_rect().expand2(egui::vec2(10.0, 10.0));
                if rect.is_positive() {
                    let mut mesh = egui::Mesh::default();
                    let v0 = mesh.vertices.len() as u32;
                    mesh.colored_vertex(rect.left_top(), TR_TOP);
                    mesh.colored_vertex(rect.right_top(), TR_TOP);
                    mesh.colored_vertex(rect.right_bottom(), TR_BOT);
                    mesh.colored_vertex(rect.left_bottom(), TR_BOT);
                    mesh.add_triangle(v0, v0 + 1, v0 + 2);
                    mesh.add_triangle(v0, v0 + 2, v0 + 3);
                    ui.painter().add(egui::Shape::mesh(mesh));
                    ui.painter().rect_stroke(rect, egui::Rounding::same(0.0), egui::Stroke::new(1.0, TR_BORDER));
                }
                ui.horizontal(|ui| {
                    // the pulsing red dot: the panel is alive while the service runs
                    if self.live {
                        let now = ctx.input(|i| i.time);
                        let pulse = 0.45 + 0.55 * (0.5 + 0.5 * (now * 2.2).sin()) as f32;
                        let dot = Color32::from_rgba_unmultiplied(0xE5, 0x53, 0x4B, (pulse * 255.0) as u8);
                        ui.label(RichText::new("\u{25cf}").small().color(dot));
                    }
                    ui.label(RichText::new("TRANSCRIPT").small().color(TR_TEAL).strong());
                    // BUG (fixed): this used to require `self.speaking` too,
                    // which mirrors the VAD's mid-utterance flag — so the
                    // "listening" cue vanished on every natural pause (any
                    // silence over ~800ms), even though the mic was still
                    // fully open and capturing the whole time. That reads
                    // exactly like "it stops listening after a couple of
                    // seconds." The mic being open only depends on the
                    // service being live; VAD activity is a second, separate
                    // fact and gets its own badge below.
                    if self.live {
                        ui.label(RichText::new("listening").small().color(TR_GREEN));
                        if self.speaking {
                            ui.label(RichText::new("speaking").small().color(TR_GREEN));
                        }
                    }
                    if let Some((note, since)) = &self.short_utterance_note {
                        if since.elapsed().as_secs_f32() < 3.0 {
                            ui.label(RichText::new(note).small().color(MUTED));
                        }
                    }
                });
                ui.add_space(4.0);
                ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .max_width(f32::INFINITY)
                    .show(ui, |ui| {
                        if self.transcript.is_empty() {
                            ui.label(
                                RichText::new(if self.live {
                                    "Listening… speak naturally.\nTry: \"Let's turn to John chapter 3 verse 16.\""
                                } else {
                                    "Start a service to see the live transcript here."
                                })
                                .color(MUTED),
                            );
                        }
                        let mut fix: Option<(usize, String)> = None;
                        let mut start_edit: Option<(usize, String)> = None;
                        let mut cancel = false;
                        for (i, line) in self.transcript.iter().enumerate() {
                            // colour-code (v0.831.0): green edge = scripture, blue edge = command
                            let (accent, tint, base_col) = match line.kind {
                                LineKind::Verse => (TR_GREEN, Color32::from_rgba_unmultiplied(0x3E, 0xCF, 0x8E, 0x14), TEXT),
                                LineKind::Command => (TR_BLUE, Color32::from_rgba_unmultiplied(0x4F, 0x9D, 0xFF, 0x12), TR_BLUE_TEXT),
                                LineKind::Plain => (TR_EDGE, Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x08), MUTED),
                            };
                            let col = if line.flagged { TEXT } else { base_col };
                            let fr = egui::Frame::none()
                                .fill(tint)
                                .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                                .outer_margin(egui::Margin { left: 0.0, right: 0.0, top: 2.0, bottom: 2.0 })
                                .show(ui, |ui| {
                                    ui.set_max_width(ui.available_width());
                                    ui.horizontal_top(|ui| {
                                        ui.label(RichText::new(mmss(line.service_ms)).small().monospace().color(MUTED));
                                        let w = ui.available_width();
                                        ui.vertical(|ui| {
                                            ui.set_max_width(w);
                                            match self.edit_line.as_mut() {
                                                Some((ei, buf)) if *ei == i => {
                                                    let r = ui.add(egui::TextEdit::multiline(buf).desired_rows(2).desired_width(w));
                                                    r.request_focus();
                                                    if ui.input(|inp| inp.key_pressed(egui::Key::Escape)) {
                                                        cancel = true;
                                                    } else if ui.input(|inp| inp.key_pressed(egui::Key::Enter) && !inp.modifiers.shift) {
                                                        fix = Some((i, buf.trim_end_matches('\n').to_string()));
                                                    }
                                                    ui.label(RichText::new("Enter = save and learn \u{00b7} Esc = cancel").small().color(MUTED));
                                                }
                                                _ => {
                                                    let r = ui.add(egui::Label::new(RichText::new(line.text.clone()).color(col)).wrap().sense(egui::Sense::click()));
                                                    if r.on_hover_text("Double-click to correct this line. RhemaFlow learns from it.").double_clicked() {
                                                        start_edit = Some((i, line.text.clone()));
                                                    }
                                                }
                                            }
                                        });
                                    });
                                });
                            // the 3 px accent bar on the left edge of the line
                            let lr = fr.response.rect;
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(lr.left_top(), egui::vec2(3.0, lr.height())),
                                1.0,
                                accent,
                            );
                        }
                        if let Some(e) = start_edit {
                            self.edit_line = Some(e);
                        }
                        if cancel {
                            self.edit_line = None;
                        }
                        if let Some((i, t)) = fix {
                            self.edit_line = None;
                            self.correct_line(i, &t);
                        }
                        // provisional text for speech still in progress
                        if let Some((text, at)) = &self.live_partial {
                            if at.elapsed().as_secs_f32() < 4.0 {
                                let _ = text;
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("  …").small().monospace().color(MUTED));
                                    // solid = permanent for this utterance; dim = still being decided
                                    if !self.live_view.committed.is_empty() {
                                        ui.label(RichText::new(self.live_view.committed.clone()).color(TEXT));
                                    }
                                    if !self.live_view.tentative.is_empty() {
                                        ui.label(RichText::new(self.live_view.tentative.clone()).italics().color(MUTED));
                                    }
                                });
                            }
                        }
                        if self.scroll_to_end {
                            ui.scroll_to_cursor(Some(Align::BOTTOM));
                            self.scroll_to_end = false;
                        }
                    });
            });
    }

    // ------------------------------------------------------------------
    // Right: review queue (confirms · errors · suggestions)
    // ------------------------------------------------------------------
    pub fn ui_queue(&mut self, ctx: &egui::Context, wide: bool) {
        // layout B (Dock-first) gives the dock more room because the transcript is hidden
        let (default_w, range) = if wide {
            ((ctx.screen_rect().width() * 0.24).clamp(300.0, 380.0), 260.0..=460.0)
        } else {
            ((ctx.screen_rect().width() * 0.19).clamp(240.0, 300.0), 230.0..=440.0)
        };
        egui::SidePanel::right("queue")
            .resizable(true)
            .default_width(default_w)
            .width_range(range)
            .frame(egui::Frame::none().fill(PANEL).inner_margin(10.0))
            .show(ctx, |ui| {
                // dragging a file from the MEDIA bar brings the Service order forward so it can be dropped there
                if egui::DragAndDrop::has_payload_of_type::<u64>(ctx) && ctx.input(|i| i.pointer.hover_pos()).map_or(false, |p| p.y < ctx.screen_rect().bottom() - 230.0) {
                    self.right_tab = crate::order::RightTab::Order;
                }
                // Strong's / history used to be floating windows; asking for them now brings their tab forward
                if self.strongs.open {
                    self.strongs.open = false;
                    self.right_tab = crate::order::RightTab::Strongs;
                }
                use crate::order::RightTab as RT;
                // one line, always: the wording shortens as the dock narrows (v0.841.0)
                let labels = crate::order::dock_tab_labels(ui.available_width(), self.order.items().len(), self.confirms.len(), self.timeline.entries().len());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let tabs = [(RT::Order, labels[0].clone()), (RT::Review, labels[1].clone()), (RT::Strongs, labels[2].clone()), (RT::History, labels[3].clone()), (RT::Index, labels[4].clone())];
                    for (t, label) in tabs {
                        if ui.selectable_label(self.right_tab == t, RichText::new(label).size(12.5)).clicked() && self.right_tab != t {
                            self.right_tab = t;
                            save_settings(self);
                        }
                    }
                });
                ui.separator();
                match self.right_tab {
                    RT::Order => {
                        self.ui_order(ui, ctx);
                        return;
                    }
                    RT::Strongs => {
                        let verse = match (&self.stage.kind, self.parse_ctx.last) {
                            (StageKind::Scripture, Some(p)) => Some((p.book as usize, p.chapter as usize, p.from as usize, p.to as usize, self.stage.text.clone())),
                            _ => None,
                        };
                        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            if let Some(c) = self.strongs.body(ui, verse) {
                                self.show_content(c);
                            }
                        });
                        return;
                    }
                    RT::History => {
                        self.ui_history_body(ui);
                        return;
                    }
                    RT::Index => {
                        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            // v0.831.0: the Index tab owns the timestamped index; the middle
                            // panel keeps only the working scripture lists (no more duplicate)
                            self.center_index_list(ui);
                        });
                        return;
                    }
                    RT::Review => {}
                }
                ui.label(RichText::new("NEEDS YOUR REVIEW").small().color(MUTED).strong());
                // quotation settings: said without book / chapter / verse
                ui.horizontal_wrapped(|ui| {
                    let mut changed = false;
                    changed |= ui
                        .checkbox(&mut self.auto_approve_quotes, "auto-approve quotes")
                        .on_hover_text("ON: a clear, unambiguous quotation goes straight to the stage.\nOFF: it waits here for you to approve.\nSeveral near-equal verses always wait for you. Voice: \"auto approve on / off\"")
                        .changed();
                    changed |= ui
                        .checkbox(&mut self.show_all_matches, "show all matches")
                        .on_hover_text("ON: list every verse that fits, best % first.\nOFF: show only the best one. Voice: \"show all matches\" / \"show one match\"")
                        .changed();
                    if changed {
                        save_settings(self);
                    }
                });
                // v0.890.0 — the adjustable gates. v0.856 fixed them (cards from
                // 3 words / 50%), v0.889 fixed them the other way (5 words / 90%,
                // cards nearly gone). In v0.890 the operator chooses, per row:
                // what a CARD needs, what an AUTO-SHOW needs, what a near-tie
                // does, and whether live decodes must hold before a card.
                ui.collapsing("Suggestion gates (words / min %)", |ui| {
                    let mut changed = false;
                    // ---- row 1: what a CARD needs (the suggestive scriptures) ----
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Cards (suggest):");
                        let cur = self.parse_ctx.quote_gates.min_words;
                        for w in [3u8, 4, 5] {
                            if ui.radio(cur == w, format!("{w} words")).clicked() {
                                self.parse_ctx.quote_gates.min_words = w;
                                changed = true;
                            }
                        }
                        let mut pct = self.parse_ctx.quote_gates.min_pct as i32;
                        let pct_before = pct;
                        ui.add(egui::Slider::new(&mut pct, 50..=95).step_by(5.0).text("min %"));
                        if pct != pct_before {
                            self.parse_ctx.quote_gates.min_pct = pct as u8;
                            changed = true;
                        }
                    });
                    ui.small("3 words / 50% = v0.856 (suggests a lot) · 5 words / 90% = v0.889 (quiet). A fragment under 4 words still needs 70%.");
                    // ---- row 2: what an AUTO-SHOW needs ----
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Auto-show:");
                        for w in [3u8, 4, 5] {
                            if ui.radio(self.auto_gates.min_words == w, format!("{w} words")).clicked() {
                                self.auto_gates.min_words = w;
                                changed = true;
                            }
                        }
                        let mut pct = self.auto_gates.min_pct as i32;
                        let pct_before = pct;
                        ui.add(egui::Slider::new(&mut pct, 75..=90).step_by(5.0).text("min %"));
                        if pct != pct_before {
                            self.auto_gates.min_pct = pct as u8;
                            changed = true;
                        }
                    });
                    ui.small("auto-approve quotes must be ON for these to act; 5 words / 90% is the whole-Bible grid winner.");
                    // ---- row 3: the near-tie rule ("91 and 92") ----
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Two verses over the bar (91 & 92):");
                        if ui.radio(!self.auto_gates.near_tie_card, "auto-show the winner").clicked() {
                            self.auto_gates.near_tie_card = false;
                            changed = true;
                        }
                        if ui.radio(self.auto_gates.near_tie_card, "ask me (card)").clicked() {
                            self.auto_gates.near_tie_card = true;
                            changed = true;
                        }
                    });
                    ui.small("ON: the 92 goes straight to the stage. Ask me: both wait as one card (v0.856's 12-point rule).");
                    // ---- row 4: the partial stability hold ----
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Stability hold:");
                        for (ms, label) in [
                            (0u64, "off"),
                            (500, "0.5 s"),
                            (650, "0.65 s"),
                            (1000, "1 s"),
                        ] {
                            if ui.radio(self.quote_hold_ms == ms, label).clicked() {
                                self.quote_hold_ms = crate::partial::clamp_hold_ms(ms);
                                changed = true;
                            }
                        }
                    });
                    ui.small("off = v0.856 (cards appear instantly). On: a partial's verse must survive every decode for the hold (v0.889's anti-flicker).");
                    if changed {
                        save_settings(self);
                    }
                });
                ui.add_space(4.0);

                ScrollArea::vertical()
                    .stick_to_bottom(false)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut done: Vec<(u64, bool)> = vec![]; // (id, approved)
                        let show_all = self.show_all_matches;

                        // ---- uncertain detections (confirm queue) ----
                        for q in &self.confirms {
                            card(ui, PANEL_2, |ui| {
                                let kind_txt = match q.det.kind {
                                    rf_core::DetectionKind::Quotation => "POSSIBLE QUOTATION",
                                    rf_core::DetectionKind::BookOnly => "BOOK ONLY",
                                    _ => "POSSIBLE SCRIPTURE",
                                };
                                ui.label(RichText::new(kind_txt).small().color(BLUE).strong());
                                if q.det.passage.chapter > 0 {
                                    ui.label(
                                        RichText::new(q.det.passage.label())
                                            .size(17.0)
                                            .color(TEXT)
                                            .strong(),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new(self.bible.book_name(q.det.passage.book as usize))
                                            .size(17.0)
                                            .color(TEXT)
                                            .strong(),
                                    );
                                    ui.label(RichText::new("no chapter heard").small().color(MUTED));
                                }
                                if let Some(note) = q.det.note.clone() {
                                    ui.label(RichText::new(note).small().color(MUTED));
                                }
                                if !q.preview.is_empty() {
                                    ui.label(
                                        RichText::new(format!("“{}…”", truncate(&q.preview, 160)))
                                            .small()
                                            .color(MUTED)
                                            .italics(),
                                    );
                                }
                                ui.label(
                                    RichText::new(if q.det.pct > 0 {
                                        format!("accuracy {}%", q.det.pct)
                                    } else {
                                        format!("confidence {:.0}%", q.det.confidence * 100.0)
                                    })
                                    .small()
                                    .color(MUTED),
                                );
                                if !q.det.alt_pct.is_empty() {
                                    // ranked quotation: every possible verse, best % first
                                    let mut rows: Vec<(rf_core::Passage, u8)> = vec![(q.det.passage, q.det.pct)];
                                    for (p, pct) in q.det.alternatives.iter().zip(q.det.alt_pct.iter()) {
                                        rows.push((*p, *pct));
                                    }
                                    let shown = if show_all { rows.len() } else { 1 };
                                    ui.label(RichText::new(format!("{} possible verses \u{2014} tap one to stage it", rows.len())).small().color(MUTED));
                                    for (i, (p, pct)) in rows.iter().take(shown).enumerate() {
                                        if ui
                                            .button(RichText::new(format!("{}.  {}   {}%", i + 1, p.label(), pct)).color(if i == 0 { GOLD } else { TEXT }))
                                            .clicked()
                                        {
                                            done.push((q.id, true));
                                            if i == 0 {
                                                self.pending_approve = Some(q.id);
                                            } else {
                                                self.pending_alt = Some((q.id, *p));
                                            }
                                        }
                                    }
                                    if !show_all && rows.len() > 1 {
                                        ui.label(RichText::new(format!("+{} more hidden (turn on \"show all matches\")", rows.len() - 1)).small().color(MUTED));
                                    }
                                } else if !q.det.alternatives.is_empty() {
                                    ui.label(RichText::new("alternatives:").small().color(MUTED));
                                    for alt in q.det.alternatives.clone() {
                                        if ui
                                            .small_button(RichText::new(alt.label()).color(TEXT))
                                            .clicked()
                                        {
                                            done.push((q.id, true));
                                            self.pending_alt = Some((q.id, alt));
                                        }
                                    }
                                }
                                ui.separator();
                                ui.horizontal(|ui| {
                                    if q.det.passage.chapter > 0
                                        && ui.button(RichText::new("Approve").color(GREEN)).clicked()
                                    {
                                        done.push((q.id, true));
                                        self.pending_approve = Some(q.id);
                                    }
                                    if ui.button("Correct").clicked() {
                                        self.correcting = Some(q.id);
                                        self.correct_input.clear();
                                    }
                                    if ui.button(RichText::new("Dismiss").color(RED)).clicked() {
                                        done.push((q.id, false));
                                        self.pending_dismiss = Some(q.id);
                                    }
                                });
                                if self.correcting == Some(q.id) {
                                    ui.add_space(2.0);
                                    ui.label(RichText::new("type the correct reference:").small().color(MUTED));
                                    let resp = ui.add(
                                        TextEdit::singleline(&mut self.correct_input)
                                            .hint_text("e.g. John 3:16")
                                            .desired_width(180.0),
                                    );
                                    if resp.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                    {
                                        done.push((q.id, false));
                                        self.pending_correct = Some(q.id);
                                    }
                                }
                            });
                        }

                        // ---- honest out-of-range errors ----
                        for (i, e) in self.errors.iter().enumerate() {
                            card(ui, PANEL_2, |ui| {
                                ui.label(RichText::new("COULD NOT RESOLVE").small().color(RED).strong());
                                ui.label(RichText::new(e.message.clone()).color(TEXT));
                                if !e.candidates.is_empty() {
                                    ui.label(RichText::new("did you mean:").small().color(MUTED));
                                    for cand in e.candidates.clone() {
                                        if ui
                                            .small_button(RichText::new(cand.label()).color(GOLD))
                                            .clicked()
                                        {
                                            self.pending_candidate = Some((i, cand));
                                        }
                                    }
                                }
                                if ui.button("Dismiss").clicked() {
                                    self.pending_error_dismiss = Some(i);
                                }
                            });
                        }

                        // ---- suggested content ("three things") ----
                        for (i, s) in self.suggestions.iter().enumerate() {
                            card(ui, PANEL_2, |ui| {
                                ui.label(
                                    RichText::new("SUGGESTED CONTENT")
                                        .small()
                                        .color(GOLD)
                                        .strong(),
                                );
                                ui.label(RichText::new(s.title.clone()).color(TEXT).strong());
                                for item in &s.items {
                                    ui.label(RichText::new(format!("•  {item}")).color(TEXT));
                                }
                                ui.horizontal(|ui| {
                                    if ui.button(RichText::new("Stage").color(GREEN)).clicked() {
                                        self.pending_points = Some(i);
                                    }
                                    if ui.button("Reject").clicked() {
                                        self.pending_points_reject = Some(i);
                                    }
                                });
                            });
                        }

                        if self.confirms.is_empty() && self.errors.is_empty() && self.suggestions.is_empty() {
                            ui.label(
                                RichText::new("Nothing needs review.\nClear detections are staged automatically.")
                                    .color(MUTED),
                            );
                        }

                        // apply queued actions after the read-only pass
                        if let Some(id) = self.pending_approve.take() {
                            if let Some(pos) = self.confirms.iter().position(|q| q.id == id) {
                                let q = self.confirms.remove(pos);
                                if q.det.kind == rf_core::DetectionKind::Quotation {
                                    self.do_stage(q.det.passage, "quotation", q.det.confidence, false);
                                } else {
                                    self.do_stage(q.det.passage, "confirm", q.det.confidence, q.det.kind == rf_core::DetectionKind::ChapterOnly);
                                }
                                self.confirm_ok += 1;
                                self.touches += 1;
                            }
                        }
                        if let Some((id, alt)) = self.pending_alt.take() {
                            if let Some(pos) = self.confirms.iter().position(|q| q.id == id) {
                                let _q = self.confirms.remove(pos);
                                self.do_stage(alt, "confirm", 1.0, false);
                                self.confirm_ok += 1;
                                self.touches += 1;
                            }
                        }
                        if let Some(id) = self.pending_dismiss.take() {
                            self.confirms.retain(|q| q.id != id);
                            self.confirm_dismissed += 1;
                            self.touches += 1;
                        }
                        if let Some(id) = self.pending_correct.take() {
                            let text = self.correct_input.clone();
                            if !text.is_empty() {
                                let outcome = rf_core::parse(
                                    self.bible,
                                    &text,
                                    &rf_core::ParseContext { last: self.parse_ctx.last, pending_book: self.parse_ctx.pending_book, quote_gates: self.parse_ctx.quote_gates },
                                    0.0,
                                );
                                if let Some(det) = outcome
                                    .detections
                                    .first()
                                    .or(outcome.confirms.first())
                                    .cloned()
                                {
                                    if det.passage.chapter > 0 {
                                        self.do_stage(det.passage, "manual", 1.0, false);
                                    }
                                } else if let Some(err) = outcome.errors.first() {
                                    self.errors.push(QueuedError {
                                        id: self.next_id,
                                        message: err.message.clone(),
                                        candidates: err.candidates.clone(),
                                    });
                                    self.next_id += 1;
                                }
                                self.touches += 1;
                            }
                            self.correcting = None;
                            if let Some(pos) = self.confirms.iter().position(|q| q.id == id) {
                                self.confirms.remove(pos);
                            }
                        }
                        if let Some((i, cand)) = self.pending_candidate.take() {
                            if i < self.errors.len() {
                                self.do_stage(cand, "manual", 1.0, false);
                                self.errors.remove(i);
                                self.touches += 1;
                            }
                        }
                        if let Some(i) = self.pending_error_dismiss.take() {
                            if i < self.errors.len() {
                                self.errors.remove(i);
                                self.touches += 1;
                            }
                        }
                        if let Some(i) = self.pending_points.take() {
                            self.stage_points(i);
                        }
                        if let Some(i) = self.pending_points_reject.take() {
                            if i < self.suggestions.len() {
                                self.suggestions.remove(i);
                                self.touches += 1;
                            }
                        }
                    });
            });
    }

    // ------------------------------------------------------------------
    // Bottom: controls (wrapping rows)
    // ------------------------------------------------------------------
    pub fn ui_controls(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("controls")
            .frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(10.0, 6.0)))
            .show(ctx, |ui| {
                // v0.812.0: two compact wrapping rows. Rarely used settings live under "More",
                // so every everyday control is visible at once and nothing is cut off.
                ui.spacing_mut().item_spacing = egui::vec2(5.0, 4.0);
                ui.spacing_mut().button_padding = egui::vec2(7.0, 3.0);

                // ---------------- row 1: service, navigation, stage ----------------
                // The things you press WHILE LIVE are big and first; settings are smaller below them.
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
                    if !self.live {
                        if big_button(ui, "\u{25b6}  Start service", Color32::from_rgb(0x1B, 0x4A, 0x39), GREEN).clicked() {
                            // pre-flight: only a RED line stops to ask; otherwise the service just starts
                            if crate::preflight::worst(&crate::preflight::evaluate(&self.preflight_inputs())) == crate::preflight::Level::Bad {
                                self.preflight_open = true;
                            } else {
                                self.start_service();
                            }
                        }
                        if ui.button("\u{2714} Check").on_hover_text("Pre-flight: microphone, disk, sound, screens, internet, subscription").clicked() {
                            self.preflight_open = true;
                            self.preflight_net = None;
                        }
                    } else if self.x.is_locked() {
                        ui.add_enabled(false, egui::Button::new("\u{1F512} Locked").min_size(egui::vec2(0.0, 34.0))).on_disabled_hover_text(crate::lock::refusal(crate::lock::Guarded::EndService));
                    } else if big_button(ui, "\u{25a0}  End service", Color32::from_rgb(0x55, 0x1F, 0x22), RED).clicked() {
                        self.end_confirm = true;
                    }
                    if ui.add(egui::Button::new(RichText::new(if self.x.runsheet.is_running() { "\u{1F527} Tools \u{25CF}" } else { "\u{1F527} Tools" }).size(14.0)).min_size(egui::vec2(0.0, 34.0))).on_hover_text("Lower thirds, giving QR, announcements, run-sheet, backup, lock ...").clicked() {
                        self.x.open = !self.x.open;
                    }
                    let rec_label = if self.recording { "\u{23f9} Stop rec" } else { "\u{23fa} Record" };
                    if ui.add_enabled(self.live, egui::Button::new(RichText::new(rec_label).size(14.0)).min_size(egui::vec2(0.0, 34.0))).clicked() {
                        self.set_recording(!self.recording);
                    }
                    ui.add_space(6.0);
                    if big_button(ui, "\u{25c0}  Verse", PANEL_2, TEXT).on_hover_text("Previous verse (\u{2190})").clicked() {
                        self.nav_prev_verse();
                    }
                    if big_button(ui, "Verse  \u{25b6}", PANEL_2, TEXT).on_hover_text("Next verse (\u{2192})").clicked() {
                        self.nav_next_verse();
                    }
                    ui.add_space(6.0);
                    let (bo_fill, bo_fg) = if self.blackout { (Color32::from_rgb(0xB8, 0x32, 0x2C), Color32::WHITE) } else { (Color32::from_rgb(0x3A, 0x1D, 0x20), Color32::from_rgb(0xF0, 0x8A, 0x84)) };
                    let bo_text = if self.blackout { "\u{25cf}  BLACKOUT ON" } else { "Blackout" };
                    if big_button(ui, bo_text, bo_fill, bo_fg).on_hover_text("Hide the verse without losing it \u{2014} key B. A new verse brings it back").clicked() {
                        self.blackout = !self.blackout;
                    }
                    if big_button(ui, "Clear", PANEL_2, TEXT).on_hover_text("Key C").clicked() {
                        self.clear_stage();
                    }
                    ui.add_space(6.0);
                    let stage_label = if self.stage_visible { "Close stage" } else { "Open stage" };
                    if big_button(ui, stage_label, PANEL_2, TEXT).on_hover_text("Key F \u{00b7} voice: \"open stage\" / \"close stage\"").clicked() {
                        self.run_named("toggle_stage");
                    }
                    if ui.selectable_label(self.captions_on, "Captions").on_hover_text("Show what the preacher is saying as subtitles on the congregation screen").clicked() {
                        self.captions_on = !self.captions_on;
                    }
                    if ui.selectable_label(self.monitor_open, "Monitor").on_hover_text("A window for the preacher: clock, what is on the screen, what comes next").clicked() {
                        self.monitor_open = !self.monitor_open;
                    }
                    let fs_label = if self.stage_fullscreen { "Exit full screen" } else { "Full screen" };
                    if big_button(ui, fs_label, PANEL_2, TEXT).on_hover_text("Key F11 / Esc \u{00b7} voice: \"full screen\" / \"exit full screen\"").clicked() {
                        self.set_fullscreen(!self.stage_fullscreen);
                    }
                });
                // ---------------- everyday settings (smaller) ----------------
                ui.horizontal_wrapped(|ui| {
                    if ui.button("\u{25c0} ch").on_hover_text("Previous chapter (\u{2191})").clicked() {
                        self.nav_prev_chapter();
                    }
                    if ui.button("ch \u{25b6}").on_hover_text("Next chapter (\u{2193})").clicked() {
                        self.nav_next_chapter();
                    }
                    ui.separator();
                    // which screen the stage goes to (the "output" screen)
                    {
                        let shown = crate::displays::choose(&self.displays, self.output_display.as_deref())
                            .map(|i| format!("Screen {}", i + 1))
                            .unwrap_or_else(|| "Screen: auto".to_string());
                        let mut redetect = false;
                        ComboBox::from_id_source("output_display")
                            .selected_text(shown)
                            .width(86.0)
                            .show_ui(ui, |ui| {
                                let cur = crate::displays::choose(&self.displays, self.output_display.as_deref());
                                for (i, d) in self.displays.clone().iter().enumerate() {
                                    if ui.selectable_label(cur == Some(i), d.label(i)).clicked() {
                                        self.output_display = Some(d.name.clone());
                                        save_settings(self);
                                    }
                                }
                                if self.displays.is_empty() {
                                    ui.label(RichText::new("no screens found").small().color(MUTED));
                                }
                                ui.separator();
                                if ui.button("Refresh screens").clicked() {
                                    redetect = true;
                                }
                            })
                            .response
                            .on_hover_text("Which screen the congregation sees. Default: screen 2 when one is plugged in.");
                        if redetect {
                            self.displays = crate::displays::detect();
                        }
                        if self.stage_fullscreen && !self.stage_report.is_empty() {
                            ui.label(RichText::new(self.stage_report.clone()).small().color(MUTED));
                        }
                    }
                    ui.menu_button("\u{1f3a8} Colours", |ui| {
                        ui.set_min_width(250.0);
                        ui.label(RichText::new("background and text").small().color(MUTED));
                        for (name, bg, fg) in crate::stage::COLOR_PRESETS {
                            let on = self.stage_bg == *bg && self.stage_fg == *fg;
                            let swatch = RichText::new(format!("  Aa  {name}")).color(egui::Color32::from_rgb(fg[0], fg[1], fg[2])).background_color(egui::Color32::from_rgb(bg[0], bg[1], bg[2]));
                            if ui.add(egui::SelectableLabel::new(on, swatch)).clicked() {
                                self.stage_bg = *bg;
                                self.stage_fg = *fg;
                                save_settings(self);
                            }
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("background");
                            if egui::color_picker::color_edit_button_srgb(ui, &mut self.stage_bg).changed() {
                                self.stage_fg = crate::stage::auto_text_for(self.stage_bg);
                                save_settings(self);
                            }
                            ui.label("text");
                            if egui::color_picker::color_edit_button_srgb(ui, &mut self.stage_fg).changed() {
                                save_settings(self);
                            }
                        });
                        ui.label(RichText::new("changing the background picks readable text for you").small().color(MUTED));
                    });
                    if ui.button(format!("\u{266a} Songs ({})", self.present.song_count())).on_hover_text("Your songs: add, see and show them on the stage").clicked() {
                        self.present.open_at(crate::present::Tab::Songs);
                    }
                    if ui.button("Slides & timers").on_hover_text("Slides, backgrounds, timers (pictures and video live in the MEDIA bar below)").clicked() {
                        self.present.open = !self.present.open;
                    }
                    if ui.button("\u{2753} Guide").on_hover_text("How to use the app, step by step (same as Help, H)").clicked() {
                        self.help_visible = !self.help_visible;
                    }
                    if ui.button("Strong's").on_hover_text("Greek and Hebrew words (say: what is the Greek for love)").clicked() {
                        self.strongs.open = !self.strongs.open;
                    }
                    ComboBox::from_id_source("stage_style")
                        .selected_text(self.stage_style.label())
                        .width(96.0)
                        .show_ui(ui, |ui| {
                            for st in StageStyle::ALL {
                                if ui.selectable_label(self.stage_style == st, st.label()).clicked() {
                                    self.stage_style = st;
                                    save_settings(self);
                                }
                            }
                        })
                        .response
                        .on_hover_text("Classic \u{00b7} Lower third (for live streams) \u{00b7} Green screen (chroma key) \u{00b7} Large print");
                    {
                        let label = match self.stage_fade_ms { 0 => "Cut", 1..=500 => "Fade", _ => "Slow fade" };
                        ComboBox::from_id_source("stage_fade")
                            .selected_text(label)
                            .width(84.0)
                            .show_ui(ui, |ui| {
                                for (name, ms) in [("Cut (instant)", 0u32), ("Fade", 350), ("Slow fade", 900)] {
                                    if ui.selectable_label(self.stage_fade_ms == ms, name).clicked() {
                                        self.stage_fade_ms = ms;
                                        save_settings(self);
                                    }
                                }
                            })
                            .response
                            .on_hover_text("How a new verse or picture arrives on the stage");
                    }
                    ui.separator();
                    let names = self.translation_names();
                    let mut chosen: Option<String> = None;
                    ComboBox::from_id_source("translation")
                        .selected_text(self.translation.clone())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for n in &names {
                                if ui.selectable_label(*n == self.translation, n).clicked() {
                                    chosen = Some(n.clone());
                                }
                            }
                            ui.separator();
                            if ui.button("Add Bible versions\u{2026}").clicked() {
                                self.versions_visible = true;
                            }
                        });
                    if let Some(n) = chosen {
                        self.set_translation(&n);
                    }
                });

                // ---------------- row 2: reading, zoom, tools ----------------
                ui.horizontal_wrapped(|ui| {
                    if ui.button("\u{1f50a} Read").on_hover_text("Read the on-stage verse aloud \u{2014} R").clicked() {
                        self.read_aloud();
                    }
                    let speaking = self.tts.speaking();
                    if ui
                        .add_enabled(speaking, egui::Button::new(RichText::new("\u{25a0} Stop").color(if speaking { RED } else { MUTED })))
                        .on_hover_text("Stops the voice at once, even with auto-read on \u{2014} key S, or say \"stop\"")
                        .clicked()
                    {
                        self.stop_reading();
                    }
                    if ui.checkbox(&mut self.auto_read, "auto-read").on_hover_text("Speak every newly staged verse automatically").changed() {
                        if !self.auto_read {
                            self.stop_reading();
                        }
                        save_settings(self);
                    }
                    ui.separator();
                    ui.label(RichText::new("zoom").small().color(MUTED));
                    if ui.button("A\u{2212}").on_hover_text("Smaller (voice: \"zoom out\")").clicked() {
                        self.adjust_zoom(-crate::actions::ZOOM_STEP);
                    }
                    let mut pct = (self.stage_zoom * 100.0).round();
                    if ui.add(egui::Slider::new(&mut pct, 60.0..=400.0).show_value(false)).changed() {
                        self.set_zoom_percent(pct as u16);
                    }
                    if ui.button("A+").on_hover_text("Bigger (voice: \"zoom in\")").clicked() {
                        self.adjust_zoom(crate::actions::ZOOM_STEP);
                    }
                    ComboBox::from_id_source("zoom_presets")
                        .selected_text(format!("{:.0}%", self.stage_zoom * 100.0))
                        .width(64.0)
                        .show_ui(ui, |ui| {
                            for v in [100u16, 150, 200, 250, 300, 350, 400] {
                                let label = match v {
                                    200 => "200% (half max)".to_string(),
                                    400 => "400% (max)".to_string(),
                                    _ => format!("{v}%"),
                                };
                                if ui.selectable_label(false, label).clicked() {
                                    self.set_zoom_percent(v);
                                }
                            }
                        });
                    ui.separator();
                    ui.menu_button("More \u{25be}", |ui| {
                        ui.set_min_width(300.0);
                        if ui.button("\u{1f4be} Save settings\u{2026} (where recordings go)").clicked() {
                            self.save_visible = !self.save_visible;
                            ui.close_menu();
                        }
                        if ui.add_enabled(self.service_dir.is_some(), egui::Button::new("\u{1f4c1} Open this service's folder")).clicked() {
                            if let Some(d) = &self.service_dir {
                                crate::export::open_folder(d);
                            }
                            ui.close_menu();
                        }
                        if ui.button(if self.browser.visible { "Hide scripture browser" } else { "Show scripture browser" }).clicked() {
                            self.browser.visible = !self.browser.visible;
                            save_settings(self);
                            ui.close_menu();
                        }
                        if ui.button("Phone remote & OBS\u{2026}").clicked() {
                            self.web_visible = !self.web_visible;
                            ui.close_menu();
                        }
                        if ui.button("Sermon plan\u{2026}").clicked() {
                            self.plan_visible = !self.plan_visible;
                            ui.close_menu();
                        }
                        ui.separator();
                        ui.label(RichText::new("auto-stage confidence").small().color(MUTED));
                        let floor_pct = format!("{:.0}%", self.floor * 100.0);
                        ui.add(egui::Slider::new(&mut self.floor, 0.5..=0.95).text(floor_pct));
                        ui.separator();
                        ui.label(RichText::new("microphone").small().color(MUTED));
                        ComboBox::from_id_source("device").selected_text(self.selected_device.clone().unwrap_or_else(|| "default".into())).width(240.0).show_ui(ui, |ui| {
                            let devices: Vec<rf_asr::InputDevice> = self.devices.clone();
                            let current = self.selected_device.clone();
                            for d in devices {
                                let label = if d.is_default { format!("{}  (default)", d.name) } else { d.name.clone() };
                                if ui.selectable_label(current.as_deref() == Some(d.name.as_str()), label).clicked() {
                                    self.selected_device = Some(d.name.clone());
                                    save_settings(self);
                                    if self.live || self.engine.is_some() {
                                        self.restart_audio();
                                    }
                                }
                            }
                        });
                        if ui.button("Refresh microphones").clicked() {
                            self.devices = rf_asr::list_input_devices();
                        }
                        ui.separator();
                        ui.label(RichText::new("reading voice").small().color(MUTED));
                        ComboBox::from_id_source("tts_voice")
                            .selected_text(self.voices.iter().find(|v| Some(&v.id) == self.tts_voice.as_ref()).map(|v| v.label()).unwrap_or_else(|| "no TTS voices found".into()))
                            .width(240.0)
                            .show_ui(ui, |ui| {
                                let voices = self.voices.clone();
                                let current = self.tts_voice.clone();
                                for v in &voices {
                                    let star = if v.is_natural() { "\u{2605} " } else { "" };
                                    if ui.selectable_label(current.as_deref() == Some(v.id.as_str()), format!("{star}{}", v.label())).clicked() {
                                        self.tts_voice = Some(v.id.clone());
                                        self.tts.set_voice(Some(v.id.clone()));
                                        save_settings(self);
                                    }
                                }
                            });
                        if ui.button("Manage voices\u{2026}").on_hover_text("Opens the macOS voice download manager").clicked() {
                            #[cfg(target_os = "macos")]
                            let _ = std::process::Command::new("open").arg("x-apple.systempreferences:com.apple.preference.universalaccess?SpokenContent").spawn();
                        }
                        let mut rate = self.tts_rate as f32;
                        if ui.add(egui::Slider::new(&mut rate, 140.0..=240.0).text("rate")).changed() {
                            self.tts_rate = rate as u32;
                            self.tts.set_rate(self.tts_rate);
                            save_settings(self);
                        }
                        if ui
                            .checkbox(&mut self.mute_mic_while_reading, "ignore mic while reading")
                            .on_hover_text("The mic hears the speakers; ON stops the app re-reading its own voice. Turn OFF with headphones.")
                            .changed()
                        {
                            save_settings(self);
                        }
                        ui.separator();
                        if ui.button("Voice commands\u{2026}").clicked() {
                            self.commands_visible = !self.commands_visible;
                        }
                        if ui.button("Bible versions\u{2026}").clicked() {
                            self.versions_visible = true;
                        }
                        if ui.button("Help (H)").clicked() {
                            self.help_visible = !self.help_visible;
                        }
                        if ui.button("About").clicked() {
                            self.about_visible = true;
                        }
                    });
                });

                if let Some(n) = self.save_notice.clone() {
                    ui.label(RichText::new(format!("\u{26a0} {n}")).small().color(GOLD));
                }
                if let Some(err) = self.mic_error.clone() {
                    ui.add_space(4.0);
                    card(ui, Color32::from_rgb(0x33, 0x1A, 0x1B), |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("MICROPHONE").color(RED).strong());
                            ui.label(RichText::new(err.clone()).color(TEXT));
                            if ui.button("Retry").clicked() {
                                self.engine = None;
                                self.asr_tx = None;
                                self.start_audio();
                            }
                        });
                        ui.label(RichText::new("macOS may be blocking access: System Settings \u{2192} Privacy & Security \u{2192} Microphone \u{2192} enable for this app, then Retry.").small().color(MUTED));
                    });
                }
                if let Some(tr) = self.translation_request.clone() {
                    ui.label(RichText::new(format!("{tr} requested \u{2014} add it under Bible versions. Showing {}.", self.translation)).small().color(BLUE));
                }
            });
    }

    // ------------------------------------------------------------------
    // Scripture history: every scripture shown, in order
    // ------------------------------------------------------------------
    /// The newest thing the preacher said (not a voice command), while it is fresh: the caption on the screen.
    pub fn caption_now(&self) -> String {
        if !self.captions_on || !self.live {
            return String::new();
        }
        let now = self.service_ms();
        let Some(line) = self.transcript.iter().rev().find(|l| l.kind != LineKind::Command && !l.text.trim().is_empty()) else { return String::new() };
        crate::stage_extras::caption_text(&line.text, line.service_ms, now)
    }

    /// The preacher's monitor (v0.833.0): a separate window with the clock, the service time, what the congregation
    /// sees now and the next planned scripture. Open it on the screen facing the pulpit.
    pub fn ui_stage_monitor(&mut self, ctx: &egui::Context) {
        if !self.monitor_open {
            return;
        }
        let (now, _) = crate::naming::now_local();
        let next = self.plan.iter().find(|p| !p.used).map(|p| p.label.clone());
        let lines = crate::stage_extras::monitor_lines(
            &now.time_hms(),
            if self.live { Some(self.service_ms()) } else { None },
            &self.stage.label,
            &self.stage.text,
            next.as_deref(),
        );
        let mut close = false;
        let id = egui::ViewportId(egui::Id::new("rf-monitor"));
        let builder = egui::ViewportBuilder::default().with_title(format!("{} \u{2014} Preacher monitor", crate::theme::TITLE)).with_inner_size([900.0, 520.0]);
        ctx.show_viewport_immediate(id, builder, |vctx, _| {
            if vctx.input(|i| i.viewport().close_requested()) {
                close = true;
            }
            egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::BLACK).inner_margin(28.0)).show(vctx, |ui| {
                let h = ui.available_height();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&lines.clock).size(h * 0.16).color(Color32::WHITE).monospace());
                    ui.add_space(30.0);
                    ui.label(RichText::new(&lines.elapsed).size(h * 0.16).color(GREEN).monospace());
                });
                ui.add_space(10.0);
                ui.label(RichText::new("ON THE SCREEN").size(15.0).color(MUTED));
                ui.label(RichText::new(&lines.now_label).size(h * 0.09).color(GOLD).strong());
                ui.label(RichText::new(&lines.now_text).size(h * 0.055).color(Color32::from_gray(210)));
                ui.add_space(12.0);
                ui.label(RichText::new("NEXT").size(15.0).color(MUTED));
                ui.label(RichText::new(&lines.next).size(h * 0.08).color(Color32::from_rgb(120, 190, 255)));
            });
            vctx.request_repaint_after(std::time::Duration::from_millis(500));
        });
        if close {
            self.monitor_open = false;
        }
    }

    pub fn preflight_inputs(&self) -> crate::preflight::Inputs {
        let (root, _) = crate::export::resolve_root(&self.save_root);
        crate::preflight::Inputs {
            mic_count: self.devices.len(),
            mic_selected: self.selected_device.clone(),
            has_model: self.asr_mode.has_model(),
            free_bytes: crate::export::free_bytes(&root),
            ffmpeg: rf_media::convert::find_ffmpeg().is_some(),
            afplay: !cfg!(target_os = "macos") || std::path::Path::new("/usr/bin/afplay").exists(),
            screens: self.displays.len(),
            internet: self.preflight_net,
            access: self.access,
        }
    }

    /// Looks for the internet once at start-up and again whenever the check is opened (never on the UI thread).
    pub fn tick_preflight(&mut self) {
        if let Some(rx) = &self.preflight_rx {
            if let Ok(ok) = rx.try_recv() {
                self.preflight_net = Some(ok);
                self.preflight_rx = None;
            }
        } else if self.preflight_net.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(crate::preflight::internet_reachable());
            });
            self.preflight_rx = Some(rx);
        }
    }

    /// "End the service?": one deliberate extra click, because ending stops the recording and the transcript.
    pub fn ui_end_confirm(&mut self, ctx: &egui::Context) {
        if !self.end_confirm {
            return;
        }
        let mut end = false;
        let mut keep = false;
        egui::Window::new("End the service?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("This stops the microphone, the recording and the live transcript, and saves the service.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if big_button(ui, "\u{25a0}  End service", Color32::from_rgb(0x55, 0x1F, 0x22), RED).clicked() {
                        end = true;
                    }
                    if ui.button("Keep going").clicked() {
                        keep = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            keep = true;
        }
        if end {
            self.end_confirm = false;
            self.end_service();
        } else if keep {
            self.end_confirm = false;
        }
    }

    pub fn ui_preflight_window(&mut self, ctx: &egui::Context) {
        use crate::preflight::{evaluate, worst, Level};
        if !self.preflight_open {
            return;
        }
        let checks = evaluate(&self.preflight_inputs());
        let top = worst(&checks);
        let mut open = true;
        let mut start = false;
        let mut again = false;
        egui::Window::new("Before you start").open(&mut open).collapsible(false).resizable(false).default_width(440.0).show(ctx, |ui| {
            let (headline, col) = match top {
                Level::Ok => ("Everything is ready.", GREEN),
                Level::Warn => ("Ready, with things to know.", GOLD),
                Level::Bad => ("Fix the red items first, or start anyway.", RED),
            };
            ui.label(RichText::new(headline).size(17.0).color(col).strong());
            ui.add_space(6.0);
            for c in &checks {
                ui.horizontal_top(|ui| {
                    let (dot, dc) = match c.level {
                        Level::Ok => ("\u{25cf}", GREEN),
                        Level::Warn => ("\u{25cf}", GOLD),
                        Level::Bad => ("\u{25cf}", RED),
                    };
                    ui.label(RichText::new(dot).color(dc));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(c.name).strong());
                        ui.label(RichText::new(&c.detail).small().color(MUTED));
                    });
                });
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Check again").clicked() {
                    again = true;
                }
                if !self.live {
                    let label = if top == Level::Bad { "Start anyway" } else { "Start service" };
                    if ui.add(egui::Button::new(RichText::new(label).strong())).clicked() {
                        start = true;
                    }
                }
            });
        });
        if again {
            self.preflight_net = None;
            self.devices = rf_asr::list_input_devices();
            self.displays = crate::displays::detect();
        }
        if start {
            open = false;
            self.start_service();
        }
        self.preflight_open = open;
    }

    /// Header chip: who is signed in and whether the subscription is good.
    fn account_badge(&mut self, ui: &mut Ui) {
        use crate::license::{Access, Why};
        let (txt, col) = match (&self.account.status, self.access) {
            (crate::account::Status::SignedOut, _) | (_, Access::SignedOut) => ("Sign in".to_string(), MUTED),
            (_, Access::Active) => ("Subscribed".to_string(), GREEN),
            (_, Access::CheckInSoon { days_left }) => (format!("Go online within {days_left} d"), GOLD),
            (_, Access::PaymentLate { days_left }) => (format!("Renew within {days_left} d"), GOLD),
            (_, Access::FreeMode(Why::SubscriptionEnded)) => ("Subscribe".to_string(), RED),
            (_, Access::FreeMode(_)) => ("Free mode".to_string(), RED),
        };
        if ui.add(egui::Button::new(RichText::new(txt).color(col).size(13.0))).clicked() {
            self.account.open = true;
        }
    }

    /// Every frame: take the server's answer (if any), and work out what the licence rules allow.
    pub fn tick_account(&mut self, ctx: &egui::Context) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        if let Some(t) = self.account.poll(now, self.ticket.as_ref()) {
            crate::account::save_ticket(&crate::account::dir_path(), &t);
            self.ticket = Some(t);
        }
        if let Some(t) = self.ticket.as_mut() {
            crate::license::note_clock(now, t);
        }
        // a live service is never cut: a downgrade waits until it ends
        self.access = crate::license::evaluate(now, self.ticket.as_ref(), &Default::default(), self.live, Some(self.access));
        if self.account.is_working() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }

    pub fn ui_account_window(&mut self, ctx: &egui::Context) {
        use crate::account::Status;
        if !self.account.open {
            return;
        }
        let mut open = true;
        let mut do_in = false;
        let mut do_out = false;
        let mut do_billing = false;
        let mut do_free = false;
        let mut do_code: Option<bool> = None; // Some(false) = send the code, Some(true) = check it
        let mut plan: Option<&'static str> = None;
        egui::Window::new("Your account").open(&mut open).collapsible(false).resizable(false).default_width(340.0).show(ctx, |ui| {
            match self.account.status.clone() {
                Status::SignedIn { email, sub } => {
                    ui.label(RichText::new(email).strong());
                    let paid = self.access != crate::license::Access::FreeMode(crate::license::Why::SubscriptionEnded) && sub.paid_until > 0;
                    ui.label(RichText::new(if paid { "Subscription active" } else { "No active subscription: free Bible display only" }).color(if paid { GREEN } else { GOLD }));
                    ui.add_space(6.0);
                    if !paid {
                        ui.label("RhemaFlow is \u{a3}10 a month. Cancel any time.");
                        if ui.add(egui::Button::new(RichText::new("Subscribe \u{2014} \u{a3}10 / month").strong())).clicked() {
                            plan = Some("monthly");
                        }
                        if ui.button("Annual \u{2014} \u{a3}99 (2 months free)").clicked() {
                            plan = Some("annual");
                        }
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Manage billing").clicked() {
                            do_billing = true;
                        }
                        if ui.button("Sign out").clicked() {
                            do_out = true;
                        }
                    });
                }
                Status::DeviceLimit => {
                    ui.label(RichText::new("This account is already on 2 computers.").color(GOLD).strong());
                    ui.label("Your plan covers 2 computers. Sign out of the others to use this one.");
                    if ui.button("Sign out my other computers").clicked() {
                        do_free = true;
                    }
                    if ui.button("Cancel").clicked() {
                        do_out = true;
                    }
                }
                st => {
                    ui.label(RichText::new(if self.account.create_mode { "Create your account" } else { "Sign in" }).size(18.0).color(GOLD));
                    ui.add(TextEdit::singleline(&mut self.account.email).hint_text("Email").desired_width(300.0));
                    let mut enter = false;
                    if self.account.code_mode {
                        if self.account.code_sent {
                            ui.add(TextEdit::singleline(&mut self.account.code).hint_text("6-digit code from the email").desired_width(300.0));
                        }
                    } else {
                        let r = ui.add(TextEdit::singleline(&mut self.account.password).password(true).hint_text("Password").desired_width(300.0));
                        enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    }
                    match &st {
                        Status::Failed(e) => {
                            ui.label(RichText::new(e).color(RED));
                        }
                        Status::Working(w) => {
                            ui.label(RichText::new(*w).color(MUTED));
                        }
                        Status::Notice(n) => {
                            ui.label(RichText::new(n).color(GREEN));
                        }
                        _ => {}
                    }
                    ui.horizontal(|ui| {
                        let label = if self.account.code_mode { if self.account.code_sent { "Check code" } else { "Email me a code" } } else if self.account.create_mode { "Create account" } else { "Sign in" };
                        if (ui.add_enabled(!self.account.is_working(), egui::Button::new(label)).clicked() || enter) && !self.account.is_working() {
                            if self.account.code_mode {
                                do_code = Some(self.account.code_sent);
                            } else {
                                do_in = true;
                            }
                        }
                        let alt = if self.account.code_mode { "Use a password" } else { "No password? Email me a code" };
                        if ui.link(alt).clicked() {
                            self.account.code_mode = !self.account.code_mode;
                            self.account.code_sent = false;
                        }
                        let sw = if self.account.create_mode { "I already have an account" } else { "Create account" };
                        if ui.link(sw).clicked() {
                            self.account.create_mode = !self.account.create_mode;
                        }
                    });
                    ui.label(RichText::new("Without a subscription the Bible display keeps working.").small().color(MUTED));
                }
            }
        });
        if do_in {
            self.account.sign_in();
        }
        match do_code {
            Some(false) => self.account.send_code(),
            Some(true) => self.account.verify_code(),
            None => {}
        }
        if do_billing {
            self.account.billing();
        }
        if do_free {
            self.account.free_devices();
        }
        if do_out {
            self.account.sign_out();
            self.ticket = None;
            let _ = std::fs::remove_file(crate::account::dir_path().join("ticket.json"));
        }
        if let Some(p) = plan {
            self.account.subscribe(p);
        }
        self.account.open = open;
    }

    /// Scripture history, drawn as a tab in the right dock (was a floating window).
    pub fn ui_history_body(&mut self, ui: &mut egui::Ui) {
        let mut jump: Option<usize> = None;
        let mut back = false;
        let mut fwd = false;
        {
                ui.label(RichText::new("Everything shown so far, newest first. Say \"previous scripture\" to step back, \"next scripture\" to step forward. Keys: [ and ]").small().color(MUTED));
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.timeline.can_back(), egui::Button::new("\u{25c0} Back")).clicked() {
                        back = true;
                    }
                    if ui.add_enabled(self.timeline.can_forward(), egui::Button::new("Forward \u{25b6}")).clicked() {
                        fwd = true;
                    }
                    ui.label(RichText::new(self.timeline.position_label()).small().color(MUTED));
                });
                ui.separator();
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if self.timeline.is_empty() {
                        ui.label(RichText::new("nothing shown yet").color(MUTED));
                    }
                    let cur = self.timeline.pos();
                    for (i, e) in self.timeline.entries().iter().enumerate().rev() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(if i == cur { "\u{25b6}" } else { " " }).color(GREEN));
                            ui.label(RichText::new(format!("{}", i + 1)).small().monospace().color(MUTED));
                            if ui.button(RichText::new(&e.label).color(if i == cur { GOLD } else { TEXT })).clicked() {
                                jump = Some(i);
                            }
                            ui.label(RichText::new(format!("{}  {}", mmss(e.ms), crate::naming::source_label(&e.source))).small().color(MUTED));
                        });
                    }
                });
        }
        if back {
            self.history_back();
        }
        if fwd {
            self.history_forward();
        }
        if let Some(i) = jump {
            self.history_jump(i);
        }
    }

    /// Old entry point: the window no longer exists. Asking for history (button, key, voice) opens the dock tab instead.
    pub fn ui_history_window(&mut self, _ctx: &egui::Context) {
        if self.history_visible {
            self.history_visible = false;
            self.right_tab = crate::order::RightTab::History;
        }
    }

    // ------------------------------------------------------------------
    // Bible versions window
    // ------------------------------------------------------------------
    pub fn ui_versions_window(&mut self, ctx: &egui::Context) {
        if !self.versions_visible {
            return;
        }
        let mut open = self.versions_visible;
        let dir = crate::versions::bibles_dir();
        egui::Window::new("Bible versions")
            .open(&mut open)
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("Built in: KJV and BBE (public domain, always offline).").color(MUTED));
                ui.add_space(4.0);
                ui.label(RichText::new("Installed").strong());
                if self.extra_bibles.is_empty() {
                    ui.label(RichText::new("none yet").small().color(MUTED));
                }
                for (name, _) in self.extra_bibles.clone() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&name).color(GOLD));
                        if ui.small_button("use").clicked() {
                            self.set_translation(&name);
                        }
                    });
                }
                for (name, msg) in &self.version_msgs {
                    if msg.starts_with("failed") {
                        ui.label(RichText::new(format!("{name}: {msg}")).small().color(RED));
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Download a public-domain version (about 8 MB, needs internet once)").strong());
                for (menu, file, desc) in crate::versions::DOWNLOADS {
                    let have = self.extra_bibles.iter().any(|(n, _)| n == menu);
                    let busy = self.version_msgs.iter().any(|(n, m)| n == menu && m.starts_with("downloading"));
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(!have && !busy, |ui| {
                            if ui.button(if have { "installed" } else if busy { "downloading\u{2026}" } else { "Download" }).clicked() {
                                crate::versions::download_async((*menu).to_string(), (*file).to_string(), dir.clone(), self.version_tx.clone());
                                self.version_msgs.retain(|(n, _)| n != menu);
                                self.version_msgs.push(((*menu).to_string(), "downloading\u{2026}".into()));
                            }
                        });
                        ui.label(RichText::new(format!("{menu}  \u{2014} {desc}")).small().color(TEXT));
                    });
                }
                ui.add_space(8.0);
                ui.label(RichText::new("Your own versions (NIV, ESV, NKJV, NLT...)").strong());
                ui.label(RichText::new("Those are licensed: they cannot be bundled or scraped. Get the text from your licence (for example the API.Bible service, with your own key, using tools/fetch_bible.py), or any complete 66-book file, and drop it in the folder below. Accepted: .json (three common layouts) or .tsv/.txt (Book<TAB>Chapter<TAB>Verse<TAB>Text). The file name is the menu name (NIV.json -> NIV). Restart, or press Rescan.").small().color(MUTED));
                ui.label(RichText::new(dir.display().to_string()).monospace().small().color(GOLD));
                ui.horizontal(|ui| {
                    if ui.button("Open folder").clicked() {
                        let _ = std::fs::create_dir_all(&dir);
                        crate::export::open_folder(&dir);
                    }
                    if ui.button("Rescan").clicked() {
                        crate::versions::load_all_async(crate::versions::search_dirs(), self.version_tx.clone());
                    }
                });
                ui.label(RichText::new("Verses are always MATCHED against the KJV wording (what preachers quote), and SHOWN in the version you choose.").small().color(MUTED));
            });
        self.versions_visible = open;
    }

    // ------------------------------------------------------------------
    // Sermon plan window
    // ------------------------------------------------------------------

    /// Two lists side by side: what the preacher PLANS to use (put them in before the service, click to show,
    /// reorder) and what is being called AS THEY SPEAK (everything shown this service, click to show again,
    /// "+ plan" to keep it). Typing any reference shows it straight away: nothing has to be planned.
    pub fn ui_service_scriptures(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("SCRIPTURES FOR THIS SERVICE").size(12.0).color(MUTED).strong());
        let mut stage: Option<usize> = None;
        let mut remove: Option<usize> = None;
        let mut mv: Option<(usize, bool)> = None;
        let mut keep: Option<rf_core::Passage> = None;
        let mut again: Option<rf_core::Passage> = None;
        let mut add_now = false;
        let mut next = false;
        let mut clear_used = false;
        let mut left = |this: &mut Self, ui: &mut Ui| {
            ui.label(RichText::new("PLANNED (click to show)").small().color(GOLD));
            ui.horizontal_wrapped(|ui| {
                let r = ui.add(TextEdit::singleline(&mut this.plan_add_text).hint_text("add: John 3:16, Romans 8:28-30").desired_width((ui.available_width() - 170.0).max(120.0)));
                if ui.button("Add").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    add_now = true;
                }
                let can = this.plan_pos < this.plan.len();
                if ui.add_enabled(can, egui::Button::new(RichText::new("Next \u{25b6}").color(GREEN))).clicked() {
                    next = true;
                }
                if ui.small_button("Reset ticks").clicked() {
                    clear_used = true;
                }
            });
            ScrollArea::vertical().id_source("plan-list").max_height(150.0).auto_shrink([false, false]).show(ui, |ui| {
                if this.plan.is_empty() {
                    ui.label(RichText::new("Nothing planned. Add the scriptures the preacher will use, or leave it empty and call them as you go.").small().color(MUTED));
                }
                let n = this.plan.len();
                for (i, item) in this.plan.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let up_next = i == this.plan_pos;
                        ui.label(RichText::new(if item.used { "\u{2714}" } else if up_next { "\u{25b6}" } else { " " }).color(if up_next { GREEN } else { MUTED }));
                        let col = if item.used { MUTED } else if up_next { GOLD } else { TEXT };
                        if ui.button(RichText::new(&item.label).color(col)).clicked() {
                            stage = Some(i);
                        }
                        if ui.add_enabled(i > 0, egui::Button::new("\u{25b2}").small()).clicked() {
                            mv = Some((i, false));
                        }
                        if ui.add_enabled(i + 1 < n, egui::Button::new("\u{25bc}").small()).clicked() {
                            mv = Some((i, true));
                        }
                        if ui.small_button("\u{2716}").on_hover_text("Take off the list").clicked() {
                            remove = Some(i);
                        }
                    });
                }
            });
        };
        let mut right = |this: &mut Self, ui: &mut Ui| {
            ui.label(RichText::new("AS THEY SPEAK (newest first)").small().color(GOLD));
            ui.label(RichText::new("Anything spoken or typed in the search box shows here. Click to show it again.").small().color(MUTED));
            ScrollArea::vertical().id_source("called-list").max_height(150.0).auto_shrink([false, false]).show(ui, |ui| {
                if this.history.is_empty() {
                    ui.label(RichText::new("No scriptures shown yet this service.").small().color(MUTED));
                }
                for (p, label) in this.history.iter() {
                    ui.horizontal(|ui| {
                        if ui.button(label).clicked() {
                            again = Some(*p);
                        }
                        let planned = this.plan.iter().any(|it| it.label == *label);
                        if !planned && ui.small_button("+ plan").on_hover_text("Keep it on today's list").clicked() {
                            keep = Some(*p);
                        }
                    });
                }
            });
        };
        // two side-by-side lists when there is room; stacked when the column is narrow (they overprinted each other)
        if ui.available_width() >= 560.0 {
            ui.columns(2, |cols| {
                left(self, &mut cols[0]);
                right(self, &mut cols[1]);
            });
        } else {
            left(self, ui);
            ui.add_space(10.0);
            right(self, ui);
        }
        if add_now && !self.plan_add_text.trim().is_empty() {
            let t = std::mem::take(&mut self.plan_add_text);
            if self.plan_add(&t) == 0 {
                self.plan_add_text = t; // nothing found: leave it so it can be fixed
            }
        }
        if next {
            self.plan_next();
        }
        if clear_used {
            for it in self.plan.iter_mut() {
                it.used = false;
            }
            self.plan_pos = 0;
        }
        if let Some(i) = stage {
            self.plan_stage(i);
        }
        if let Some((i, down)) = mv {
            self.plan_move(i, down);
        }
        if let Some(i) = remove {
            self.plan_remove(i);
        }
        if let Some(p) = keep {
            self.plan.push(PlanItem { label: p.label(), passage: p, used: true });
        }
        if let Some(p) = again {
            self.do_stage(p, "history", 1.0, false);
            self.touches += 1;
        }
    }

    pub fn ui_plan_window(&mut self, ctx: &egui::Context) {
        if !self.plan_visible {
            return;
        }
        let mut open = self.plan_visible;
        egui::Window::new("Sermon plan")
            .open(&mut open)
            .default_width(460.0)
            .default_height(420.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("Paste the sermon notes. Every scripture reference is found, in order. Then press Space (or Next) to put the next one on the stage.").small().color(MUTED));
                ui.add(TextEdit::multiline(&mut self.plan_text).desired_rows(5).desired_width(f32::INFINITY).hint_text("e.g.  Open with John 3:16-18, then Romans 8:28, close with Psalm 23"));
                ui.horizontal(|ui| {
                    if ui.button("Find scriptures").clicked() {
                        self.load_plan();
                    }
                    let can = self.plan_pos < self.plan.len();
                    if ui.add_enabled(can, egui::Button::new(RichText::new("Next \u{25b6}").color(GREEN))).clicked() {
                        self.plan_next();
                    }
                    if ui.button("Start over").clicked() {
                        self.plan_pos = 0;
                    }
                    if ui.button("Clear plan").clicked() {
                        self.plan.clear();
                        self.plan_text.clear();
                        self.plan_pos = 0;
                    }
                });
                ui.separator();
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if self.plan.is_empty() {
                        ui.label(RichText::new("no scriptures yet").color(MUTED));
                    }
                    for (i, item) in self.plan.clone().iter().enumerate() {
                        let done = i < self.plan_pos;
                        let next = i == self.plan_pos;
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(if next { "\u{25b6}" } else if done { "\u{2714}" } else { " " }).color(if next { GREEN } else { MUTED }));
                            if ui.button(RichText::new(&item.label).color(if next { GOLD } else if done { MUTED } else { TEXT })).clicked() {
                                self.plan_stage(i);
                            }
                            let preview: String = self.bible.passage(item.passage.book as usize, item.passage.chapter as usize, item.passage.from as usize, item.passage.to as usize).chars().take(70).collect();
                            ui.label(RichText::new(preview).small().color(MUTED));
                        });
                    }
                });
            });
        self.plan_visible = open;
    }

    // ------------------------------------------------------------------
    // Phone remote + OBS window
    // ------------------------------------------------------------------
    pub fn ui_web_window(&mut self, ctx: &egui::Context) {
        if !self.web_visible {
            return;
        }
        let mut open = self.web_visible;
        let mut changed = false;
        egui::Window::new("Phone remote & OBS")
            .open(&mut open)
            .default_width(480.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("A small web server inside this app (off by default). OBS shows the verse as a Browser source; a phone becomes a remote control.").small().color(MUTED));
                ui.add_space(4.0);
                let mut on = self.web.is_some();
                if ui.checkbox(&mut on, "Turn on web output").changed() {
                    self.web_enabled = on;
                    if on {
                        self.web_start();
                    } else {
                        self.web_stop();
                    }
                    changed = true;
                }
                if ui.checkbox(&mut self.web_lan, "allow phones on the same Wi-Fi (otherwise this Mac only)").changed() {
                    changed = true;
                    if self.web.is_some() {
                        self.web_start();
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("port");
                    let mut port = self.web_port as u32;
                    if ui.add(egui::DragValue::new(&mut port).range(1024..=65000)).changed() {
                        self.web_port = port as u16;
                        changed = true;
                    }
                    ui.label("PIN");
                    ui.label(RichText::new(&self.web_pin).monospace().color(GOLD).strong());
                    if ui.small_button("new PIN").clicked() {
                        self.web_pin = crate::web::new_pin();
                        if self.web.is_some() {
                            self.web_start();
                        }
                        changed = true;
                    }
                });
                if let Some(e) = &self.web_error {
                    ui.label(RichText::new(e).color(RED));
                }
                if let Some(w) = &self.web {
                    let host = if self.web_lan { crate::web::lan_ip().unwrap_or_else(|| "this-mac.local".into()) } else { "localhost".into() };
                    let port = w.port();
                    ui.add_space(6.0);
                    ui.label(RichText::new("OBS: add a Browser source, width 1920, height 1080, URL").small().color(MUTED));
                    ui.label(RichText::new(format!("http://{host}:{port}/stage")).monospace().color(GOLD));
                    ui.label(RichText::new("   add ?style=lower (lower third), ?style=chroma (green) or ?zoom=1.3 to override the app").small().color(MUTED));
                    ui.label(RichText::new("Phone remote:").small().color(MUTED));
                    ui.label(RichText::new(format!("http://{host}:{port}/remote")).monospace().color(GOLD));
                    if self.web_lan {
                        crate::studio_ui::draw_qr(ui, &format!("http://{host}:{port}/remote"), 150.0);
                        ui.label(RichText::new("Point the phone's camera at this, open the page, then type the PIN above.").small().color(MUTED));
                    }
                    if !self.web_lan {
                        ui.label(RichText::new("Phones cannot reach it until you tick \"allow phones on the same Wi-Fi\".").small().color(BLUE));
                    } else {
                        ui.label(RichText::new("Anyone on your Wi-Fi can SEE the verse page; only someone with the PIN can control it. Use a trusted network.").small().color(BLUE));
                    }
                }
            });
        if changed {
            save_settings(self);
        }
        self.web_visible = open;
    }

    // ------------------------------------------------------------------
    // Save settings window: where services go, which formats
    // ------------------------------------------------------------------
    pub fn ui_save_window(&mut self, ctx: &egui::Context) {
        if !self.save_visible {
            return;
        }
        let mut open = self.save_visible;
        let mut changed = false;
        egui::Window::new("Save settings \u{2014} recording & transcript")
            .open(&mut open)
            .default_width(520.0)
            .show(ctx, |ui| {
                let (root, _) = crate::export::resolve_root(&self.save_root);
                ui.label(RichText::new("Each day gets a folder with the full date, and every service inside it gets its OWN folder (named by its start time) holding only that service's files:").color(MUTED));
                let (st, _) = crate::naming::now_local();
                let demo_end = st.plus_ms(47 * 60_000 + 27_000);
                let demo_base = crate::naming::final_base(&st, &demo_end, &self.service_title);
                ui.label(RichText::new(format!("{}/{}/{}/", root.display(), crate::naming::day_folder_name(&st), crate::naming::service_folder_name(&st, &self.service_title))).monospace().color(GOLD));
                ui.label(RichText::new(format!("   {}", crate::naming::recording_name(&demo_base, 1))).monospace().small().color(MUTED));
                ui.label(RichText::new(format!("   {}", crate::naming::file_name(&demo_base, "transcript.txt"))).monospace().small().color(MUTED));
                if let Some(free) = crate::export::free_bytes(&root) {
                    let h = free as f64 / crate::export::WAV_BYTES_PER_HOUR as f64;
                    ui.label(RichText::new(format!("{} free on this drive \u{2248} {:.0} hours of recording", crate::export::human_size(free), h)).small().color(if h < 3.0 { RED } else { MUTED }));
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Save location").strong());
                let home = crate::export::home();
                let is_dl = self.save_root.is_empty();
                let docs = crate::export::documents_root().to_string_lossy().to_string();
                let is_docs = self.save_root == docs;
                let is_custom = !is_dl && !is_docs;
                ui.horizontal_wrapped(|ui| {
                    if ui.radio(is_dl, "Downloads / RhemaFlow (default)").clicked() {
                        self.save_root.clear();
                        changed = true;
                    }
                    if ui.radio(is_docs, "Documents / RhemaFlow").clicked() {
                        self.save_root = docs.clone();
                        changed = true;
                    }
                    if ui.radio(is_custom, "Custom folder").clicked() && !is_custom {
                        self.save_root = home.join("RhemaFlow-Services").to_string_lossy().to_string();
                        changed = true;
                    }
                });
                if is_custom || (!is_dl && !is_docs) {
                    ui.horizontal(|ui| {
                        changed |= ui.add(TextEdit::singleline(&mut self.save_root).desired_width(340.0)).changed();
                        if self.folder_pick_rx.is_none() && ui.button("Choose\u{2026}").clicked() {
                            let (tx, rx) = crossbeam_channel::bounded(1);
                            self.folder_pick_rx = Some(rx);
                            std::thread::spawn(move || {
                                let _ = tx.send(crate::export::choose_folder_dialog());
                            });
                        }
                    });
                }
                ui.add_space(8.0);
                ui.label(RichText::new("Service title (goes in the folder name and sermon notes)").strong());
                ui.add(TextEdit::singleline(&mut self.service_title).hint_text("e.g. Faith that moves").desired_width(340.0));
                ui.add_space(8.0);
                ui.label(RichText::new("Transcript formats written when the service ends").strong());
                ui.label(RichText::new("\u{2714} .txt  plain timestamped text \u{2014} always kept (saved live, safe even after a crash)").small().color(MUTED));
                changed |= ui.checkbox(&mut self.formats.srt, ".srt  captions \u{2014} timed subtitles for YouTube / video editors").changed();
                changed |= ui.checkbox(&mut self.formats.md, ".md  sermon notes \u{2014} scripture index with times, then the full transcript").changed();
                ui.label(RichText::new("also always: events.jsonl (every scripture and action, machine-readable) and index.md").small().color(MUTED));
                ui.add_space(8.0);
                ui.label(RichText::new("Recording").strong());
                ui.label(RichText::new("audio.wav (full quality, ~115 MB per hour) is written live.").small().color(MUTED));
                changed |= ui.checkbox(&mut self.compress_audio, "also make a small audio.m4a copy when the service ends (about 1/10 the size, macOS)").changed();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Open save folder").clicked() {
                        crate::export::open_folder(&root);
                    }
                    if self.live {
                        ui.label(RichText::new("changes apply to the next service").small().color(MUTED));
                    }
                });
            });
        if changed {
            save_settings(self);
        }
        self.save_visible = open;
    }

    // ------------------------------------------------------------------
    // "Service saved" window: shown after End service
    // ------------------------------------------------------------------
    pub fn ui_saved_window(&mut self, ctx: &egui::Context) {
        let Some(info) = self.saved.clone() else { return };
        // refresh the file list while the .m4a is still being made
        if info.refreshed.elapsed().as_millis() > 800 {
            if let Some(s) = &mut self.saved {
                s.files = crate::export::list_files(&s.dir, &s.prefix);
                s.refreshed = Instant::now();
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(800));
        }
        let busy = self.compress_busy.load(std::sync::atomic::Ordering::Relaxed);
        let mut keep = true;
        egui::Window::new("Service saved")
            .open(&mut keep)
            .default_width(480.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("Your recording and transcript are here:").color(MUTED));
                ui.label(RichText::new(info.dir.display().to_string()).monospace().color(GOLD));
                if let Some(n) = &info.note {
                    ui.label(RichText::new(format!("\u{26a0} {n}")).small().color(GOLD));
                }
                ui.add_space(6.0);
                egui::Grid::new("saved_files").spacing([16.0, 3.0]).show(ui, |ui| {
                    for (name, size) in &info.files {
                        ui.label(RichText::new(name).monospace());
                        ui.label(RichText::new(crate::export::human_size(*size)).small().color(MUTED));
                        ui.end_row();
                    }
                });
                if busy {
                    ui.label(RichText::new("making audio.m4a\u{2026}").small().color(BLUE));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Open folder").clicked() {
                        crate::export::open_folder(&info.dir);
                    }
                    if ui.button("Copy path").clicked() {
                        ctx.copy_text(info.dir.display().to_string());
                    }
                });
            });
        if !keep {
            self.saved = None;
        }
    }

    // ------------------------------------------------------------------
    // Voice commands window (generated from the same table that matches speech)
    // ------------------------------------------------------------------
    pub fn ui_commands_window(&mut self, ctx: &egui::Context) {
        if !self.commands_visible {
            return;
        }
        let mut open = self.commands_visible;
        egui::Window::new("Voice commands")
            .open(&mut open)
            .default_width(560.0)
            .default_height(520.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("search");
                    ui.add(TextEdit::singleline(&mut self.commands_filter).hint_text("e.g. zoom, stage, read").desired_width(220.0));
                });
                ui.label(RichText::new("Say any of these. Scripture references always win over commands; short phrases only work in short sentences so normal preaching never triggers them.").small().color(MUTED));
                ui.add_space(4.0);
                let q = self.commands_filter.trim().to_ascii_lowercase();
                let all = rf_core::cmdtable::all_phrases();
                let mut groups: Vec<(String, Vec<String>)> = Vec::new();
                for (phrase, label) in all {
                    if !q.is_empty() && !phrase.contains(&q) && !label.to_ascii_lowercase().contains(&q) {
                        continue;
                    }
                    match groups.iter_mut().find(|(l, _)| *l == label) {
                        Some((_, v)) => v.push(phrase),
                        None => groups.push((label, vec![phrase])),
                    }
                }
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for (label, phrases) in &groups {
                        ui.horizontal_top(|ui| {
                            ui.add_sized([170.0, 18.0], egui::Label::new(RichText::new(label).strong().color(GOLD)));
                            let w = ui.available_width();
                            ui.vertical(|ui| {
                                ui.set_max_width(w);
                                ui.add(egui::Label::new(RichText::new(phrases.iter().map(|p| format!("\u{201c}{p}\u{201d}")).collect::<Vec<_>>().join("  \u{00b7}  ")).color(TEXT)).wrap());
                            });
                        });
                        ui.separator();
                    }
                    if groups.is_empty() {
                        ui.label(RichText::new("no command matches that search").color(MUTED));
                    }
                });
            });
        self.commands_visible = open;
    }

    // ------------------------------------------------------------------
    // Center: stage preview + scripture index
    // ------------------------------------------------------------------
    /// The MEDIA bar docked under the stage preview (v0.823.0).
    pub fn ui_media_dock(&mut self, ctx: &egui::Context) {
        if let Some(c) = self.present.dock_ui(ctx) {
            self.show_content(c);
        }
    }

    pub fn ui_center(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG).inner_margin(12.0))
            .show(ctx, |ui| {
                // one scroll for the whole middle: when the window is short it scrolls instead of drawing over the panels below
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let avail = ui.available_size();
                    if avail.x >= 600.0 {
                        // wide: the preview on the left, this service's working scripture lists beside it
                        let gap = 16.0;
                        let left_w = (avail.x * 0.60).min(avail.x - 270.0 - gap).max(260.0);
                        let right_w = avail.x - left_w - gap;
                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(egui::vec2(left_w, 10.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.set_width(left_w);
                                self.center_preview(ui, left_w, (avail.y - 40.0).max(84.0));
                            });
                            ui.add_space(gap);
                            ui.allocate_ui_with_layout(egui::vec2(right_w, 10.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.set_width(right_w);
                                // v0.831.0: the timestamped index moved to the Index tab; the
                                // middle keeps the lists you work from (planned + as-they-speak)
                                self.ui_service_scriptures(ui);
                            });
                        });
                    } else {
                        self.center_preview(ui, avail.x, (avail.y * 0.6).max(110.0));
                        ui.add_space(10.0);
                        self.ui_service_scriptures(ui);
                    }
                });
            });
    }

    /// Layout C (Focus / simple mode): one huge preview, the heard-strip and the
    /// working scripture list underneath. No dock, no media bar — fewest distractions.
    pub fn ui_center_focus(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG).inner_margin(12.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let avail = ui.available_size();
                    self.center_preview(ui, avail.x, (avail.y * 0.62).max(150.0));
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(4.0);
                    self.ui_service_scriptures(ui);
                });
            });
    }

    /// The little copy of what the congregation sees, with a LIVE / BLACKOUT badge, and what was heard.
    fn center_preview(&mut self, ui: &mut Ui, width: f32, max_h: f32) {
        // v0.831.0 fix: cap the preview by the space that is actually VISIBLE right
        // now. The caller's estimate comes from the scroll content, which at short
        // window sizes promises more room than the bottom panels leave — the preview
        // then ran into the Scripture panel (found by ux_probe at 1100x700).
        let used = ui.cursor().top() - ui.clip_rect().top();
        let vis = (ui.clip_rect().height() - used - 14.0).max(46.0);
        let max_h = max_h.min(vis);
        // as wide as the column allows, but never taller than the visible middle (so it cannot run into the panels below)
        let h = (width.min(max_h * 16.0 / 9.0).max(160.0) * 9.0 / 16.0).min(max_h);
        let w = h * 16.0 / 9.0;
        let look = crate::stage::StageLook { style: self.stage_style, blackout: self.blackout, bg: self.stage_bg, fg: self.stage_fg, fade_ms: self.stage_fade_ms };
        ui.horizontal(|ui| {
            ui.label(RichText::new("WHAT THE CONGREGATION SEES").size(12.0).color(MUTED).strong());
            if self.blackout {
                ui.label(RichText::new(" \u{25cf} BLACKOUT ").size(12.0).strong().color(Color32::WHITE).background_color(Color32::from_rgb(0xB8, 0x32, 0x2C)));
            } else if self.stage_visible {
                ui.label(RichText::new(" \u{25cf} ON SCREEN ").size(12.0).strong().color(Color32::BLACK).background_color(GREEN));
            } else {
                ui.label(RichText::new("stage window is closed").size(12.0).color(MUTED));
            }
        });
        ui.add_space(4.0);
        ui.allocate_ui(egui::vec2(w, h), |ui| {
            egui::Frame::none()
                .stroke(egui::Stroke::new(if self.blackout { 2.0 } else { 1.0 }, if self.blackout { RED } else { BORDER }))
                .rounding(8.0)
                .fill(Color32::BLACK)
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(w - 2.0, h - 2.0));
                    let art = self.present.art(&self.stage.kind);
                    crate::stage::draw_stage(ui, &self.stage, false, self.stage_zoom, look, &art);
                });
        });
        // ---- display colour swatches (v0.831.0): one tap under the preview, exactly
        // where the operator looks; the same nine presets as the Colours menu, and the
        // text colour comes with the preset so it is always readable
        ui.add_space(6.0);
        ui.with_layout(egui::Layout::top_down(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.horizontal(|ui| {
                for (name, bg, fg) in crate::stage::COLOR_PRESETS {
                    let on = self.stage_bg == *bg && self.stage_fg == *fg;
                    let btn = egui::Button::new(RichText::new("Aa").size(11.0).strong().color(Color32::from_rgb(fg[0], fg[1], fg[2])))
                        .fill(Color32::from_rgb(bg[0], bg[1], bg[2]))
                        .stroke(if on { egui::Stroke::new(2.0, GOLD) } else { egui::Stroke::new(1.0, BORDER) })
                        .min_size(egui::vec2(30.0, 22.0));
                    if ui.add(btn).on_hover_text(format!("{name} — click to put this on the screen")).clicked() {
                        self.stage_bg = *bg;
                        self.stage_fg = *fg;
                        save_settings(self);
                    }
                }
            });
            ui.label(RichText::new("display colours — readable text is picked for you").small().color(MUTED));
        });
        // ---- interpretation strip (v0.801.2) ----
        if let Some((heard, result)) = self.last_interpret.clone() {
            ui.add_space(8.0);
            card(ui, PANEL_2, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("HEARD").size(12.0).color(MUTED).strong());
                    ui.label(RichText::new(format!("\u{201c}{heard}\u{201d}")).color(TEXT));
                    ui.label(RichText::new("\u{2192}").color(MUTED));
                    ui.label(RichText::new(result).color(GOLD).strong());
                });
            });
        }
    }

    /// The timestamped index of every scripture shown this service + the honesty
    /// stats. v0.831.0: this lives ONLY in the Index tab of the right dock — the
    /// middle panel keeps the working lists instead (kills the old duplicate).
    pub fn center_index_list(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("SCRIPTURE INDEX \u{2014} THIS SERVICE").size(12.0).color(MUTED).strong());
        ui.add_space(4.0);
        if self.index.is_empty() {
            ui.label(RichText::new("Every staged Scripture lands here with its timestamp.").color(MUTED));
        }
        egui::Grid::new("index_grid").spacing([12.0, 4.0]).show(ui, |ui| {
            for e in &self.index {
                ui.label(RichText::new(mmss(e.service_ms)).monospace().color(GOLD));
                ui.label(RichText::new(e.label.clone()).color(TEXT));
                ui.label(RichText::new(e.source).small().color(MUTED));
                ui.end_row();
            }
        });
        if !self.index.is_empty() || self.auto_staged > 0 {
            ui.add_space(6.0);
            ui.label(RichText::new(format!("auto-staged {} \u{b7} operator touched {} \u{b7} avg latency {} ms", self.auto_staged, self.touches, self.avg_latency())).size(12.0).color(MUTED));
        }
    }

    fn avg_latency(&self) -> String {
        if self.latencies.is_empty() {
            "—".into()
        } else {
            let avg: u64 = self.latencies.iter().sum::<u64>() / self.latencies.len() as u64;
            format!("{avg}")
        }
    }

    // ------------------------------------------------------------------
    // Help overlay
    // ------------------------------------------------------------------
    pub fn ui_help(&mut self, ctx: &egui::Context) {
        let mut open = self.help_visible;
        let data_dir = crate::fsutil::data_dir();
        // (title, body): the search box filters these by any word in either
        let sections: Vec<(&str, String)> = vec![
            ("Quick guide: your first service", "1. Check the top bar: \"model ok\" and the mic meter moving when someone speaks.\n2. Press F to open the stage window and drag it to the projector or TV (F11 for full screen).\n3. Start the service (record / go live) and speak a reference such as \"John chapter 3 verse 16\". Approve with Space.\n4. Songs: press \"\u{266a} Songs\" in the top bar. Add words with Paste lyrics, then \"Show on stage\". Next / Previous moves through the slides.\n5. Pictures, videos and sounds: use the MEDIA bar below; the play bar can pause, jump back / forward and loop.\n6. Announcements and countdowns: \"Slides & timers\".\n7. B = blackout, C = clear. Stuck? Press H or F1 any time to open this guide.".into()),
            ("Where things are", "\u{266a} Songs \u{2014} top bar: add, list, edit and show songs\nSlides & timers \u{2014} top bar: slides, background, timers\nMEDIA bar \u{2014} below the transcript: pictures, videos, sounds\nStrong's \u{2014} Greek and Hebrew words\nVoice commands\u{2026} and Bible versions\u{2026} \u{2014} toolbar buttons\nAbout \u{2014} version, folders, copy diagnostics (also in this window, bottom)".into()),
            ("Keyboard shortcuts", "Space \u{2014} approve the first card (or next in the sermon plan)    X \u{2014} dismiss    1-5 \u{2014} pick option\n\u{2190} / \u{2192} or P / N \u{2014} previous / next verse    \u{2191} / \u{2193} \u{2014} previous / next chapter\nB \u{2014} blackout    C \u{2014} clear    R \u{2014} read aloud    S \u{2014} stop reading    F \u{2014} open / close stage\nF11 / Esc \u{2014} leave full screen    / \u{2014} search scripture    H or F1 \u{2014} this help\nCmd+K \u{2014} command palette: type what you want (\"blackout\", \"songs\") and press Enter\nJ / K / L \u{2014} video or sound: back 10 s / play-pause / forward 10 s    PageDown / PageUp \u{2014} next / previous slide\nCmd+S \u{2014} save the song you are editing    Esc \u{2014} close Help, About or the palette (or select no files)\nMEDIA bar: click = show    Cmd+click = add to the selection    Shift+click = range    drag a box = select what it touches    Cmd+A = all\nCmd+C \u{2014} copy the selected files (or the verse on screen)    Cmd+V \u{2014} paste files copied in Finder, a file path, or a song\u{2019}s words    Delete (twice) \u{2014} remove the selected files\nCmd+Q \u{2014} quit (asks first if a service is recording)    Cmd+H \u{2014} hide the app\nLetters only work when you are not typing in a box; click an empty area first.".into()),
            ("Voice commands (say it naturally)", "References: \"Let's turn to John chapter 3 verse 16.\" \u{b7} \"Turn with me to Psalm 23.\" \u{b7} \"Romans 8:28.\"\nNavigation: \"Next.\" / \"Next verse.\" \u{b7} \"Previous.\" / \"Go back.\" \u{b7} \"Next chapter.\"\nAgain & read: \"Last scripture.\" / \"Again.\" \u{b7} \"Read it.\" / \"Stop reading.\"\nStage control: \"Zoom in.\" / \"Bigger.\" \u{b7} \"Zoom out.\" / \"Smaller.\" \u{b7} \"Full screen.\" \u{b7} \"Clear the screen.\"\nQuotations: \"For God so loved the world\u{2026}\" (goes to the review queue)  \u{b7}  Translation: \"in the BBE\"".into()),
            ("Songs and lyrics", "Add: \u{266a} Songs > Paste lyrics (or Import .txt / .pptx, or New song).\nSee: every song is listed with a Search box (title, author or words).\nEdit: press \"\u{270f} Edit words\" next to a song, change the title, words or order, then \"Save song\". If that song is on the slides, they are rebuilt.\nShow: \"Show on stage\", then Next / Previous. Blank line = new slide. A first line like \"Chorus:\" names a part.\nLicence: enter your church CCLI number in the editor; it prints under songs that have a CCLI song number.".into()),
            ("Pictures, videos and sounds", "Click an item in the MEDIA bar to put it on the stage. The play bar has Pause, Back / Forward, a slider (hover for a preview picture), Loop and Mute.\nA video you leave and come back to carries on where it stopped (Restart starts it again). Duplicate (right-click) lists the same file twice.\nPDF and HTML files: drop them on the MEDIA bar like a picture. The document bar has Page back / forward, zoom, Fit page / Fit width and a Reading line that steps down the page line by line (keys: \u{2190} \u{2192} or PageUp / PageDown = page, \u{2193} \u{2191} = reading line, K = reading line on / off).\nJumping in a sound that is not AAC / MP3 needs ffmpeg (brew install ffmpeg).".into()),
            ("Files and folders", format!("Your settings and songs live in: {}\nRecordings, transcripts (.txt / .srt) and sermon notes (.md) go in a dated folder under the location chosen in \"Save settings\" (default Downloads/RhemaFlow).\nBack up the data folder to keep your songs and settings. Extras > Backup makes a zip for you.\nExtra Bible versions: drop a .json or .tsv file in the folder shown under \"Bible versions\u{2026}\".", data_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_else(|| "(not found)".into()))),
            ("Troubleshooting", "No scripture appears: look at the mic meter; check System Settings > Privacy & Security > Microphone for your terminal / app.\nStage is on the wrong screen: close it (F), drag it to the other screen, press F again.\nSound plays but cannot jump: install ffmpeg (brew install ffmpeg) or use an MP4 / M4A.\nA video has no sound: check Mute on the play bar and the Mac's output device.\nApp feels slow: close other apps; very big videos play best from a local disk.\nStill stuck: press About > Copy diagnostics and send that text with your question.".into()),
            ("Read-aloud voices and microphone permission", "Pick a \u{2605} Premium/Enhanced voice in the toolbar. Download more: System Settings > Accessibility > Spoken Content > System Voice > Manage Voices\u{2026}\nmacOS asks once for microphone access: click OK. If missed: System Settings > Privacy & Security > Microphone.".into()),
        ];
        egui::Window::new(format!("{} \u{2014} Help & guide", crate::theme::TITLE))
            .open(&mut open)
            .default_width(600.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("The preacher speaks. RhemaFlow understands.").color(GOLD));
                ui.horizontal(|ui| {
                    ui.label("\u{1f50d} Search help:");
                    ui.add(egui::TextEdit::singleline(&mut self.help_filter).desired_width(280.0).hint_text("e.g. song, sound, folder, shortcut"));
                    if ui.button("Clear").clicked() {
                        self.help_filter.clear();
                    }
                });
                ui.separator();
                let words: Vec<String> = self.help_filter.to_lowercase().split_whitespace().map(String::from).collect();
                egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                    let mut shown = 0;
                    for (title, body) in &sections {
                        let hay = format!("{title} {body}").to_lowercase();
                        if !words.iter().all(|w| hay.contains(w.as_str())) {
                            continue;
                        }
                        shown += 1;
                        ui.label(RichText::new(*title).strong());
                        ui.label(body.as_str());
                        ui.add_space(6.0);
                    }
                    if shown == 0 {
                        ui.label(RichText::new("Nothing matches. Try one word, like \"song\" or \"sound\".").weak());
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Church name on stage:");
                        if ui.text_edit_singleline(&mut self.church_name).changed() {
                            save_settings(self);
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("About / diagnostics").clicked() {
                        self.about_visible = true;
                    }
                    if let Some(d) = &data_dir {
                        if ui.button("Open data folder").clicked() {
                            crate::export::open_folder(d);
                        }
                    }
                });
            });
        self.help_visible = open;
    }

    // ------------------------------------------------------------------
    // About window (version, folders, copy diagnostics)
    // ------------------------------------------------------------------
    /// Cmd/Ctrl+K: type a few letters, press Enter (first match) or click. Every action shows its key.
    pub fn ui_palette(&mut self, ctx: &egui::Context) {
        const ACTIONS: &[(&str, &str, &str)] = &[
            ("approve", "Approve the first scripture card", "Space"),
            ("dismiss", "Dismiss the first card", "X"),
            ("next_verse", "Next verse", "\u{2192} / N"),
            ("prev_verse", "Previous verse", "\u{2190} / P"),
            ("next_chapter", "Next chapter", "\u{2193}"),
            ("prev_chapter", "Previous chapter", "\u{2191}"),
            ("blackout", "Blackout on / off", "B"),
            ("clear", "Clear the screen", "C"),
            ("read", "Read the verse aloud", "R"),
            ("stop_reading", "Stop reading", "S"),
            ("toggle_stage", "Open / close the stage window", "F"),
            ("toggle_fullscreen", "Full screen on / off", "F11"),
            ("next_slide", "Next slide / song part", "PageDown"),
            ("prev_slide", "Previous slide / song part", "PageUp"),
            ("media_toggle", "Play / pause the video or sound", "K"),
            ("media_back", "Back 10 seconds", "J"),
            ("media_fwd", "Forward 10 seconds", "L"),
            ("media_restart", "Restart the video or sound", ""),
            ("zoom_in", "Bigger text", ""),
            ("zoom_out", "Smaller text", ""),
            ("history_back", "Previous scripture shown", "["),
            ("history_forward", "Next scripture shown", "]"),
            ("plan_next", "Next scripture in the sermon plan", "Space"),
            ("songs", "Open Songs", ""),
            ("help", "Help and guide", "H / F1"),
            ("about", "About / copy diagnostics", ""),
        ];
        let mut run: Option<&'static str> = None;
        let mut open = true;
        egui::Window::new("Command palette").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_TOP, [0.0, 80.0]).default_width(440.0).show(ctx, |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.palette_q).desired_width(f32::INFINITY).hint_text("type: next, blackout, songs, sound\u{2026}   (Enter = first, Esc = close)"));
            r.request_focus();
            let words: Vec<String> = self.palette_q.to_lowercase().split_whitespace().map(String::from).collect();
            let hits: Vec<&(&str, &str, &str)> = ACTIONS.iter().filter(|(n, l, _)| words.iter().all(|w| format!("{n} {l}").to_lowercase().contains(w.as_str()))).collect();
            if hits.is_empty() {
                ui.label(RichText::new("No match.").weak());
            }
            for (name, label, keys) in &hits {
                ui.horizontal(|ui| {
                    if ui.button(*label).clicked() {
                        run = Some(*name);
                    }
                    if !keys.is_empty() {
                        ui.label(RichText::new(*keys).small().monospace().color(MUTED));
                    }
                });
            }
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some((n, _, _)) = hits.first() {
                    run = Some(*n);
                }
            }
        });
        if let Some(n) = run {
            self.palette_open = false;
            self.run_named(n);
        } else if !open {
            self.palette_open = false;
        }
    }

    pub fn ui_about(&mut self, ctx: &egui::Context) {
        let mut open = self.about_visible;
        let data_dir = crate::fsutil::data_dir();
        let info = format!(
            "{}\nbuild: {} {}\nos: {} {}\ndata folder: {}\nsongs: {}",
            crate::theme::TITLE,
            if cfg!(debug_assertions) { "debug" } else { "release" },
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            data_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_else(|| "(none)".into()),
            self.present.song_count(),
        );
        egui::Window::new(format!("About {}", crate::theme::TITLE)).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label(RichText::new(crate::theme::TITLE).size(20.0).strong().color(GOLD));
            ui.label("The preacher speaks. RhemaFlow understands.");
            ui.add_space(6.0);
            ui.label(RichText::new(&info).monospace().small());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Copy diagnostics").on_hover_text("Copies the lines above so you can paste them into a message").clicked() {
                    ctx.copy_text(info.clone());
                }
                if let Some(d) = &data_dir {
                    if ui.button("Open data folder").clicked() {
                        crate::export::open_folder(d);
                    }
                }
                if ui.button("Open Help").clicked() {
                    self.help_visible = true;
                }
            });
            ui.add_space(4.0);
            ui.label(RichText::new("Third-party licences are listed in THIRD-PARTY.md.").small().color(MUTED));
        });
        self.about_visible = open;
    }
}

// ---------------------------------------------------------------------------
// small drawing helpers
// ---------------------------------------------------------------------------

fn badge(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::none()
        .fill(color.gamma_multiply(0.16))
        .rounding(4.0)
        .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.7)))
        .inner_margin(egui::Margin::symmetric(8.0, 3.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().strong().color(color));
        });
}

fn card(ui: &mut Ui, fill: Color32, add: impl FnOnce(&mut Ui)) {
    egui::Frame::none()
        .fill(fill)
        .rounding(8.0)
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .inner_margin(10.0)
        .outer_margin(egui::Margin {
            bottom: 2.0,
            ..Default::default()
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// A large button for things pressed while live (comfortable to hit on a trackpad in a dark booth).
fn big_button(ui: &mut Ui, text: &str, fill: Color32, fg: Color32) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).size(15.0).strong().color(fg)).fill(fill).min_size(egui::vec2(0.0, 34.0)))
}

fn draw_meter(ui: &mut Ui, level: f32, speaking: bool) {
    let bars = 7;
    let active = ((level * 40.0).clamp(0.0, 1.0) * bars as f32).round() as usize;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(70.0, 14.0), egui::Sense::hover());
    let bw = 7.0;
    let gap = 3.0;
    for i in 0..bars {
        let x = rect.left() + i as f32 * (bw + gap);
        let h = 4.0 + i as f32 * 1.6;
        let color = if i < active {
            if speaking && i >= 5 {
                GOLD
            } else {
                GREEN
            }
        } else {
            Color32::from_rgb(0x33, 0x3A, 0x44)
        };
        let y = rect.center().y - h / 2.0;
        ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(bw, h)), 2.0, color);
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
