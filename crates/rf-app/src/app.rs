//! Operator Command Centre — one place to understand what RhemaFlow is doing
//! (PRD §8). Layout: header (state) · left (transcript) · center (stage
//! preview + scripture index) · right (review queue) · bottom (controls).

use crate::actions::{self};
use crate::recorder;
use crate::stage;
use crate::state::*;
use crate::theme;
use crossbeam_channel::Receiver;
use rf_asr::capture::AudioEngine;
use rf_asr::InputDevice;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct RhemaFlowApp {
    pub bible: &'static rf_bible::Bible,

    // service state
    pub live: bool,
    pub recording: bool,
    pub started_at: Option<Instant>,

    // stage
    pub stage: StageContent,
    pub stage_visible: bool,
    /// live captions on the congregation screen, and the preacher's stage monitor (v0.833.0)
    pub captions_on: bool,
    pub monitor_open: bool,
    /// bumps every time anything is staged (the scripture browser follows it)
    pub stage_seq: u64,
    /// stage look + blackout + recent scriptures + sermon plan (v0.812.0)
    pub stage_style: StageStyle,
    pub blackout: bool,
    pub history: Vec<(rf_core::Passage, String)>,
    /// every scripture shown, in order, with a position (previous / next scripture)
    pub timeline: crate::history::ScriptureHistory,
    pub history_visible: bool,
    pub plan: Vec<PlanItem>,
    pub plan_pos: usize,
    pub plan_text: String,
    pub plan_add_text: String,
    pub plan_visible: bool,
    /// extra Bible versions from the Bibles folder / downloads
    pub extra_bibles: Vec<(String, &'static rf_bible::Bible)>,
    pub version_rx: Receiver<crate::versions::VersionEvent>,
    pub version_tx: crossbeam_channel::Sender<crate::versions::VersionEvent>,
    pub version_msgs: Vec<(String, String)>,
    pub versions_visible: bool,
    /// built-in web output: OBS page + phone remote
    pub web: Option<crate::web::WebServer>,
    pub web_enabled: bool,
    pub web_lan: bool,
    pub web_port: u16,
    pub web_pin: String,
    pub web_error: Option<String>,
    pub web_visible: bool,
    pub web_key: u64,
    pub stage_fullscreen: bool,
    pub church_name: String,

    // content
    pub transcript: Vec<TranscriptLine>,
    pub index: Vec<ScriptureIndexEntry>,
    pub confirms: Vec<QueuedDetection>,
    pub errors: Vec<QueuedError>,
    pub suggestions: Vec<rf_core::Suggestion>,
    pub parse_ctx: rf_core::ParseContext,
    pub next_id: u64,
    pub correcting: Option<u64>,
    pub correct_input: String,
    // queued card actions (set in the read-only UI pass, applied after)
    pub pending_approve: Option<u64>,
    pub pending_alt: Option<(u64, rf_core::Passage)>,
    pub pending_dismiss: Option<u64>,
    pub pending_correct: Option<u64>,
    pub pending_candidate: Option<(usize, rf_core::Passage)>,
    pub pending_error_dismiss: Option<usize>,
    pub pending_points: Option<usize>,
    pub pending_points_reject: Option<usize>,

    // settings
    pub floor: f32,
    /// this speaker's learned mishearing repairs (see `rhemaflow-d calibrate`)
    pub lexicon: rf_core::Lexicon,
    /// when update() last ran; a long gap means the OS throttled the window (App Nap / occluded)
    pub last_update: Instant,
    pub translation: String,
    pub translation_request: Option<String>,

    // read-aloud (TTS) + stage zoom (v0.801.2)
    pub tts: crate::tts::Speaker,
    pub tts_voice: Option<String>,
    pub tts_rate: u32,
    pub auto_read: bool,
    pub voices: Vec<crate::tts::Voice>,
    pub stage_zoom: f32,
    /// quotation settings (v0.811.0)
    pub auto_approve_quotes: bool,
    pub show_all_matches: bool,
    /// ignore the mic while the app is reading aloud (it would hear its own voice)
    pub mute_mic_while_reading: bool,
    /// auto-read is held off until this moment after the operator pressed Stop
    pub read_stop_until: Option<Instant>,
    /// last fullscreen state sent to the stage window (None = window not open yet)
    pub fs_sent: Option<bool>,
    /// connected screens + which one the stage uses (by name) + last placement sent to the window
    pub displays: Vec<crate::displays::Display>,
    pub output_display: Option<String>,
    pub out_sent: Option<(bool, i32, i32, i32, i32)>,
    /// what the stage window was last ASKED to be, how many corrections were tried, and what it really is (v0.821.0)
    pub stage_cmd: ((f32, f32), (f32, f32)),
    pub stage_tries: u8,
    pub stage_report: String,
    /// stage colours (v0.814.0)
    pub stage_bg: [u8; 3],
    pub stage_fg: [u8; 3],
    /// 0 = cut, 350 = fade, 900 = slow fade
    pub stage_fade_ms: u32,
    /// exit-full-screen watchdog: when we asked to leave, retries made, and window re-open flag
    pub fs_exit_at: Option<Instant>,
    pub fs_retries: u8,
    pub stage_reopen: bool,
    /// What the app understood from the last utterance (heard → result).
    pub last_interpret: Option<(String, String)>,

    // audio
    pub asr_mode: AsrMode,
    pub engine: Option<AudioEngine>,
    pub mic_error: Option<String>,
    pub devices: Vec<InputDevice>,
    pub selected_device: Option<String>,
    pub asr_tx: Option<crossbeam_channel::Sender<actions::UiEvent>>,
    pub asr_worker: Option<std::thread::JoinHandle<()>>,
    pub ui_rx: Receiver<actions::UiEvent>,
    pub rec_audio_rx: Option<Receiver<Vec<f32>>>,
    pub last_level: f32,
    pub speaking: bool,
    /// (message, when-shown) for the transient "heard something too brief" note
    pub short_utterance_note: Option<(String, Instant)>,
    /// the command just used, lit on a copy of the menu bar for 0.5 s: (menu, command, keys, when) (v0.845.0)
    /// Cut / Copy / Paste / Select All from the Mac menu, handed to the NEXT frame before any window is drawn (v0.845.0)
    pub pending_edit_events: Vec<egui::Event>,
    pub menu_flash: Option<(&'static str, &'static str, &'static str, Instant)>,
    /// learning from corrections (v0.821.0)
    pub learner: rf_core::learn::Learner,
    pub edit_line: Option<(usize, String)>,
    /// a shelf item the phone asked for (shown next frame)
    pub pending_media: Option<u64>,
    /// provisional text for speech still in progress, and when it arrived
    pub live_partial: Option<(String, Instant)>,
    /// append-only live strip: solid (committed) + dim (tentative) words of the utterance in progress
    pub live_text: rf_core::LiveText,
    /// set while a streaming engine is configured; tells the live strip which kind of text it is getting
    pub stream_up: Option<std::sync::Arc<crate::actions::StreamShared>>,
    pub live_view: rf_core::LiveView,
    /// quotations already raised from a live partial this utterance
    pub partial_quote_labels: Vec<(String, Instant)>,
    /// v0.890.0 — the auto-stage bars (word floor, % bar, near-tie mode), saved in settings.json
    pub auto_gates: crate::state::AutoGates,
    /// v0.890.0 — the partial stability hold in ms (0 = off, v0.856 snappiness)
    pub quote_hold_ms: u64,
    /// v0.890.0 — per-utterance candidate stability tracker for the hold
    pub quote_hold: crate::partial::QuoteHold,
    pub last_mic_restart: Option<Instant>,
    /// the mic was lost mid-service and re-opening it is still being retried
    pub mic_lost: bool,
    last_seen_dropped_count: u64,

    // recorder + logs
    pub recorder: Option<recorder::Recorder>,
    pub event_log: Option<recorder::EventLog>,
    pub transcript_log: Option<recorder::TranscriptLog>,
    pub service_dir: Option<PathBuf>,

    // saving + export (v0.811.0)
    pub save_root: String,
    pub formats: crate::export::Formats,
    pub compress_audio: bool,
    pub service_title: String,
    pub save_visible: bool,
    pub saved: Option<SavedInfo>,
    pub compress_busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub folder_pick_rx: Option<Receiver<Option<String>>>,
    pub export_lines: Vec<crate::export::Line>,
    pub export_cites: Vec<crate::export::Cite>,
    pub commands_visible: bool,
    pub commands_filter: String,
    /// persistent save-related warning (low disk, folder fallback)
    pub save_notice: Option<String>,
    /// wall-clock moment service time 0 happened (local time) + zone name, and file naming
    pub service_start: Option<(crate::naming::LocalStamp, String)>,
    pub live_base: String,
    pub rec_paths: Vec<PathBuf>,
    /// scripture browser (v0.812.0)
    pub browser: crate::browser::BrowserState,
    /// pictures, video, slides, songs, backgrounds, timers (v0.815.0)
    pub present: crate::present::Present,
    /// the service order (right-hand panel, "Order" tab)
    pub order: crate::order::Order,
    pub right_tab: crate::order::RightTab,
    /// A/B/C window layout (v0.831.0: Operator console / Dock-first / Focus)
    pub layout: LayoutMode,
    /// the transcript panel is hidden in layouts A and C (v0.841.0; View > Hide transcript)
    pub hide_transcript: bool,
    pub account: crate::account::Account,
    pub preflight_open: bool,
    /// asking "End the service?" before a stray click ends it (v0.840.0)
    pub end_confirm: bool,
    pub preflight_net: Option<bool>,
    pub preflight_rx: Option<std::sync::mpsc::Receiver<bool>>,
    pub ticket: Option<crate::license::Ticket>,
    pub access: crate::license::Access,
    pub present_bg_tried: bool,
    /// Greek & Hebrew (Strong's) panel (v0.816.0)
    pub strongs: crate::strongs_ui::StrongsState,
    /// service time (ms) of the last piece that produced a quotation: older pieces are not re-used
    pub quote_consumed_ms: u64,
    /// v0.853.0: when the stage last changed — the quotation repeat window
    /// ("said it three times, got three cards") measures from here
    pub last_stage_at: Option<std::time::Instant>,

    // metrics (PRD §24)
    pub auto_staged: usize,
    pub confirm_ok: usize,
    pub confirm_dismissed: usize,
    pub touches: usize,
    pub latencies: Vec<u64>,

    // ui ephemeral
    pub sim_input: String,
    pub demo_idx: usize,
    pub help_visible: bool,
    pub about_visible: bool,
    pub palette_open: bool,
    pub palette_q: String,
    pub last_title: String,
    pub help_filter: String,
    pub scroll_to_end: bool,
    /// v0.835.0 extras (lower thirds, run-sheet, lock, backup ...)
    pub x: crate::extras::Extras,
}

impl RhemaFlowApp {
    pub fn new(cc: &eframe::CreationContext<'_>, asr_mode: AsrMode, church_name: String) -> Self {
        let (version_tx, version_rx) = crossbeam_channel::unbounded();
        crate::versions::load_all_async(crate::versions::search_dirs(), version_tx.clone());
        let live_strategy = if matches!(asr_mode, AsrMode::Stream(_)) { rf_core::LiveStrategy::AppendOnly } else { rf_core::LiveStrategy::Agreement };
        theme::apply(&cc.egui_ctx);
        load_stage_fonts(&cc.egui_ctx);
        let devices = rf_asr::list_input_devices();
        let selected_device = devices.iter().find(|d| d.is_default).map(|d| d.name.clone());
        let settings = load_settings();
        // v0.890.0: the suggestion gates ride on the live ParseContext — apply
        // the saved settings once at startup (the settings window re-applies on
        // change). Default is v0.856 exactly when nothing was saved yet.
        let quote_gates = rf_core::parser::QuoteGates::clamped(
            settings.suggest_min_words.unwrap_or(3),
            settings.suggest_min_pct.unwrap_or(rf_core::parser::MIN_QUOTE_PCT),
        );
        let (tx, rx) = crossbeam_channel::unbounded();
        let voices = crate::tts::list_voices();
        let tts_voice = settings
            .tts_voice
            .or_else(|| crate::tts::default_voice(&voices));
        let tts_rate = settings.tts_rate.unwrap_or(175);
        RhemaFlowApp {
            x: crate::extras::Extras::load(),
            bible: {
                let b = rf_bible::Bible::embedded();
                // build the garbled-quotation index in the background so the first one does not pause the screen
                std::thread::spawn(move || b.warm());
                b
            },
            live: false,
            recording: false,
            started_at: None,
            stage: StageContent::default(),
            stage_visible: false,
            captions_on: false,
            monitor_open: false,
            stage_seq: 0,
            stage_style: StageStyle::from_key(settings.stage_style.as_deref().unwrap_or("classic")),
            blackout: false,
            history: Vec::new(),
            timeline: crate::history::ScriptureHistory::default(),
            history_visible: false,
            plan: Vec::new(),
            plan_pos: 0,
            plan_text: String::new(),
            plan_add_text: String::new(),
            plan_visible: false,
            extra_bibles: Vec::new(),
            version_rx,
            version_tx,
            version_msgs: Vec::new(),
            versions_visible: false,
            web: None,
            web_enabled: settings.web_enabled.unwrap_or(false),
            web_lan: settings.web_lan.unwrap_or(false),
            web_port: settings.web_port.unwrap_or(8787),
            web_pin: settings.web_pin.clone().filter(|p| p.len() == 4).unwrap_or_else(crate::web::new_pin),
            web_error: None,
            web_visible: false,
            web_key: u64::MAX,
            stage_fullscreen: false,
            church_name: settings.church_name.or(Some(church_name)).unwrap_or_default(),
            transcript: vec![],
            index: vec![],
            confirms: vec![],
            errors: vec![],
            suggestions: vec![],
            parse_ctx: rf_core::ParseContext { quote_gates, ..rf_core::ParseContext::default() },
            lexicon: crate::calibrate::load_lexicon(),
            last_update: Instant::now(),
            next_id: 1,
            correcting: None,
            correct_input: String::new(),
            pending_approve: None,
            pending_alt: None,
            pending_dismiss: None,
            pending_correct: None,
            pending_candidate: None,
            pending_error_dismiss: None,
            pending_points: None,
            pending_points_reject: None,
            floor: settings.floor.unwrap_or(0.75),
            translation: settings.translation.unwrap_or_else(|| "KJV".into()),
            translation_request: None,
            tts: crate::tts::Speaker::new(tts_voice.clone(), tts_rate),
            tts_voice,
            tts_rate,
            auto_read: settings.auto_read.unwrap_or(false),
            voices,
            stage_zoom: settings.stage_zoom.unwrap_or(1.0).clamp(stage::ZOOM_MIN, stage::ZOOM_MAX),
            auto_approve_quotes: settings.auto_approve_quotes.unwrap_or(false),
            show_all_matches: settings.show_all_matches.unwrap_or(true),
            // v0.890.0 — the adjustable gates. Defaults = v0.856's measured grid
            // winner (suggest 3 words / 50%, auto 5 words / 90%) + the near-tie
            // rule the operator asked for (92 auto-shows). The env var wins for
            // the hold only when the setting never saved a value.
            auto_gates: crate::state::AutoGates {
                min_words: settings.auto_min_words.unwrap_or(rf_core::parser::AUTO_MIN_WORDS),
                min_pct: settings.auto_stage_pct.unwrap_or(rf_core::parser::AUTO_STAGE_PCT),
                near_tie_card: settings.near_tie_card.unwrap_or(false),
            },
            quote_hold_ms: settings.quote_hold_ms.unwrap_or_else(crate::partial::hold_ms_from_env),
            quote_hold: crate::partial::QuoteHold::default(),
            mute_mic_while_reading: settings.mute_mic_while_reading.unwrap_or(true),
            read_stop_until: None,
            fs_sent: None,
            displays: crate::displays::detect(),
            output_display: settings.output_display.clone(),
            out_sent: None,
            stage_cmd: ((0.0, 0.0), (0.0, 0.0)),
            stage_tries: 0,
            stage_report: String::new(),
            stage_bg: settings.stage_bg.unwrap_or([0, 0, 0]),
            stage_fg: settings.stage_fg.unwrap_or([255, 255, 255]),
            stage_fade_ms: settings.stage_fade_ms.unwrap_or(350),
            fs_exit_at: None,
            fs_retries: 0,
            stage_reopen: false,
            last_interpret: None,
            asr_mode,
            engine: None,
            mic_error: None,
            devices,
            selected_device: settings.device.or(selected_device),
            asr_tx: Some(tx),
            asr_worker: None,
            ui_rx: rx,
            rec_audio_rx: None,
            last_level: 0.0,
            speaking: false,
            short_utterance_note: None,
            menu_flash: None,
            pending_edit_events: Vec::new(),
            learner: load_learner(),
            edit_line: None,
            pending_media: None,
            live_partial: None,
            live_text: rf_core::LiveText::new(live_strategy),
            stream_up: None,
            live_view: rf_core::LiveView::default(),
            partial_quote_labels: Vec::new(),
            last_mic_restart: None,
            mic_lost: false,
            last_seen_dropped_count: 0,
            recorder: None,
            event_log: None,
            transcript_log: None,
            service_dir: None,
            save_root: settings.save_root.clone().unwrap_or_default(),
            formats: crate::export::Formats {
                txt: settings.save_txt.unwrap_or(true),
                srt: settings.save_srt.unwrap_or(true),
                md: settings.save_md.unwrap_or(true),
            },
            compress_audio: settings.compress_audio.unwrap_or(true),
            service_title: String::new(),
            save_visible: false,
            saved: None,
            compress_busy: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            folder_pick_rx: None,
            export_lines: Vec::new(),
            export_cites: Vec::new(),
            commands_visible: false,
            commands_filter: String::new(),
            save_notice: None,
            service_start: None,
            live_base: String::new(),
            rec_paths: Vec::new(),
            browser: crate::browser::BrowserState::new(settings.browser_visible.unwrap_or(true)),
            layout: LayoutMode::from_key(settings.layout.as_deref().unwrap_or("console")),
            hide_transcript: settings.hide_transcript.unwrap_or(false),
            present_bg_tried: false,
            order: crate::order::Order::open(settings_path().and_then(|p| p.parent().map(|d| d.join("service-order.json"))).unwrap_or_else(|| std::env::temp_dir().join("rhemaflow-service-order.json"))),
            right_tab: crate::order::RightTab::from_key(settings.right_tab.as_deref().unwrap_or("order")),
            account: crate::account::Account::new(crate::account::dir_path()),
            preflight_open: false,
            end_confirm: false,
            preflight_net: None,
            preflight_rx: None,
            ticket: crate::account::load_ticket(&crate::account::dir_path()),
            access: crate::license::Access::SignedOut,
            strongs: crate::strongs_ui::StrongsState::new(settings_path().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(std::env::temp_dir)),
            quote_consumed_ms: 0,
            // v0.853.0: when the stage last changed — the quotation repeat
            // window ("said it three times, got three cards") measures from here
            last_stage_at: None,
            present: {
                let mut p = crate::present::Present::new(settings_path().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(std::env::temp_dir));
                p.bg_adds = true; // adding files runs on a helper thread: the window never freezes
                p
            },
            auto_staged: 0,
            confirm_ok: 0,
            confirm_dismissed: 0,
            touches: 0,
            latencies: vec![],
            sim_input: String::new(),
            demo_idx: 0,
            help_visible: false,
            about_visible: false,
            palette_open: false,
            palette_q: String::new(),
            last_title: String::new(),
            help_filter: String::new(),
            scroll_to_end: false,
        }
    }

    fn poll_events(&mut self, ctx: &egui::Context) {
        let mut repaint = false;
        while let Ok(ev) = self.ui_rx.try_recv() {
            match ev {
                actions::UiEvent::Transcript { service_ms, text, latency_ms } => {
                    self.live_partial = None; // the finished text replaces the provisional
                    self.live_view = rf_core::LiveView::default();
                    self.handle_parse(&text, service_ms, latency_ms);
                    self.scroll_to_end = true;
                    repaint = true;
                }
                actions::UiEvent::Partial { service_ms, text } => {
                    self.handle_partial(&text, service_ms);
                    self.scroll_to_end = true;
                    repaint = true;
                }
                actions::UiEvent::Notice(msg) => {
                    self.short_utterance_note = Some((msg, Instant::now()));
                    repaint = true;
                }
                actions::UiEvent::Correction { service_ms, text, engine } => {
                    // ClearSpeech final pass (v0.843.0): the high-accuracy engine's
                    // version of the last utterance replaces the streaming draft.
                    self.live_partial = None;
                    self.handle_correction(&text, service_ms, &engine);
                    self.scroll_to_end = true;
                    repaint = true;
                }
            }
        }
        self.check_mic_health();
        // recorder tap
        if self.recording {
            if let (Some(rx), Some(rec)) = (&self.rec_audio_rx, &self.recorder) {
                let mut got = false;
                while let Ok(chunk) = rx.try_recv() {
                    rec.push(&chunk);
                    got = true;
                }
                if got {
                    repaint = true;
                }
            }
        }
        // mic meter
        if let Some(eng) = &self.engine {
            use std::sync::atomic::Ordering::Relaxed;
            self.last_level = f32::from_bits(eng.level.load(Relaxed));
            self.speaking = eng.speaking.load(Relaxed);
            let dropped = eng.dropped_count.load(Relaxed);
            if dropped > self.last_seen_dropped_count {
                self.last_seen_dropped_count = dropped;
                let ms = eng.last_dropped_ms.load(Relaxed);
                self.short_utterance_note = Some((
                    format!("heard ~{ms}ms — too brief/quiet to catch, try a touch louder or closer"),
                    Instant::now(),
                ));
            }
            repaint = true;
        }
        if repaint || self.live {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
    }

    /// Full screen on/off for the congregation stage. Turning it ON opens the stage
    /// window first if it is closed (a voice "full screen" must always do something).
    pub fn set_fullscreen(&mut self, on: bool) {
        if on {
            self.stage_visible = true;
            if self.displays.is_empty() {
                self.displays = crate::displays::detect(); // a screen may have been plugged in since launch
            }
        }
        self.stage_fullscreen = on;
    }

    fn poll_folder_pick(&mut self) {
        if let Some(rx) = &self.folder_pick_rx {
            if let Ok(res) = rx.try_recv() {
                if let Some(path) = res {
                    self.save_root = path;
                    save_settings(self);
                }
                self.folder_pick_rx = None;
            }
        }
    }

    /// Cut / Copy / Paste / Select All while no text box has the cursor (v0.844.0).
    /// Light up `id` on the menu-bar copy for half a second (v0.845.0).
    pub fn flash_menu(&mut self, id: &str) {
        if let Some((menu, name, keys)) = crate::macmenu::describe(id) {
            self.menu_flash = Some((menu, name, keys, Instant::now()));
        }
    }

    /// A strip across the top of the window that copies the menu bar for 0.5 s, with the menu that was used and its
    /// command lit in gold, so you SEE "Edit > Copy" fire when you press Cmd+C.
    fn draw_menu_flash(&mut self, ctx: &egui::Context) {
        const FLASH: Duration = Duration::from_millis(500);
        let Some((menu, name, keys, at)) = self.menu_flash else { return };
        if at.elapsed() >= FLASH {
            self.menu_flash = None;
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(50));
        egui::Area::new(egui::Id::new("menu-flash")).order(egui::Order::Tooltip).fixed_pos(egui::pos2(0.0, 0.0)).interactable(false).show(ctx, |ui| {
            let w = ctx.screen_rect().width();
            egui::Frame::none().fill(egui::Color32::from_rgb(0x1A, 0x1F, 0x28)).inner_margin(egui::Margin::symmetric(12.0, 4.0)).show(ui, |ui| {
                ui.set_min_width(w - 24.0);
                ui.horizontal(|ui| {
                    for t in crate::macmenu::MENU_TITLES {
                        if t == menu {
                            ui.label(egui::RichText::new(t).strong().color(theme::BG).background_color(theme::GOLD));
                        } else {
                            ui.label(egui::RichText::new(t).color(theme::MUTED));
                        }
                        ui.add_space(6.0);
                    }
                    ui.add_space(16.0);
                    ui.label(egui::RichText::new(format!("{menu}  \u{25b8}  {name}   {keys}")).strong().color(theme::BG).background_color(theme::GOLD));
                });
            });
        });
    }

    fn edit_outside_text(&mut self, ctx: &egui::Context, id: &str, paste_text: Option<String>) {
        let say = |app: &mut Self, m: String| app.short_utterance_note = Some((m, Instant::now()));
        match id {
            "copy" => self.copy_anything(ctx),
            "paste" => self.paste_anything(ctx, paste_text),
            "cut" => say(self, "Cut is not used for files. Cmd+C copies them; Delete removes them from RhemaFlow (it asks twice).".into()),
            "select_all" => {
                let m = self.present.dock_select_all();
                say(self, m);
            }
            _ => {}
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::Key::*;
        // "/" jumps to the search box (even though it is a typing key)
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(Slash)) {
            self.browser.visible = true;
            self.browser.focus_search = true;
        }
        if ctx.input(|i| i.key_pressed(F1)) {
            self.help_visible = !self.help_visible;
        }
        // Cmd/Ctrl+K: command palette (works even while typing)
        if ctx.input(|i| i.modifiers.command && i.key_pressed(K)) {
            self.flash_menu("palette");
            self.palette_open = !self.palette_open;
            self.palette_q.clear();
        }
        // Cmd/Ctrl+S: save the song being edited
        if let Some(id) = self.present.song_edit_shortcut(ctx) {
            self.flash_menu(id);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(S)) && self.present.save_song_shortcut() {
            self.flash_menu("save_song");
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, S));
        }
        // Esc closes the help / about / palette windows first (it still leaves full screen otherwise)
        if ctx.input(|i| i.key_pressed(Escape)) && (self.help_visible || self.about_visible || self.palette_open) {
            self.help_visible = false;
            self.about_visible = false;
            self.palette_open = false;
        }
        let typing = ctx.wants_keyboard_input();
        // Where there is no Mac menu (other systems) the window sees Cmd+C / V / X / A itself.
        if !typing && !crate::macmenu::NATIVE {
            let found: Vec<(&'static str, Option<String>)> = ctx.input(|i| {
                let mut v = Vec::new();
                for e in &i.events {
                    match e {
                        egui::Event::Copy => v.push(("copy", None)),
                        egui::Event::Cut => v.push(("cut", None)),
                        egui::Event::Paste(t) => v.push(("paste", Some(t.clone()))),
                        _ => {}
                    }
                }
                if i.modifiers.command && i.key_pressed(A) {
                    v.push(("select_all", None));
                }
                v
            });
            for (id, text) in found {
                self.flash_menu(id);
                self.edit_outside_text(ctx, id, text);
            }
        }
        let mut run: Vec<&'static str> = Vec::new();
        ctx.input(|i| {
            if typing {
                return;
            }
            // Cmd+C, Cmd+X, Cmd+V... are copy / cut / paste, never the one-letter "clear" / "dismiss" keys
            if i.modifiers.command {
                return;
            }
            let shift = i.modifiers.shift;
            // operator keys (v0.812.0): hands stay on the keyboard
            if i.key_pressed(Space) {
                run.push(if self.confirms.iter().any(|q| q.det.passage.chapter > 0) { "approve" } else { "plan_next" });
            }
            if i.key_pressed(X) {
                run.push("dismiss");
            }
            for (k, name) in [(Num1, "pick_1"), (Num2, "pick_2"), (Num3, "pick_3"), (Num4, "pick_4"), (Num5, "pick_5")] {
                if i.key_pressed(k) {
                    run.push(name);
                }
            }
            if i.key_pressed(ArrowRight) || i.key_pressed(N) {
                run.push("next_verse");
            }
            if i.key_pressed(ArrowLeft) || i.key_pressed(P) {
                run.push("prev_verse");
            }
            if i.key_pressed(ArrowDown) || (i.key_pressed(M) && !shift) {
                run.push("next_chapter");
            }
            if i.key_pressed(ArrowUp) || (i.key_pressed(M) && shift) {
                run.push("prev_chapter");
            }
            if i.key_pressed(B) {
                run.push("blackout");
            }
            if i.key_pressed(C) {
                run.push("clear");
            }
            if i.key_pressed(R) {
                run.push("read");
            }
            if i.key_pressed(S) {
                run.push("stop_reading");
            }
            if i.key_pressed(F) {
                run.push("toggle_stage");
            }
            if i.key_pressed(H) {
                run.push("help");
            }
            // media keys (like a video player): J back 10 s, K play / pause, L forward 10 s; PageUp / PageDown = slides
            if !i.modifiers.command {
                if i.key_pressed(J) {
                    run.push("media_back");
                }
                if i.key_pressed(L) {
                    run.push("media_fwd");
                }
                if i.key_pressed(K) {
                    run.push("media_toggle");
                }
                if i.key_pressed(PageDown) {
                    run.push("next_slide");
                }
                if i.key_pressed(PageUp) {
                    run.push("prev_slide");
                }
            }
            if i.key_pressed(OpenBracket) {
                run.push("history_back");
            }
            if i.key_pressed(CloseBracket) {
                run.push("history_forward");
            }
            if i.key_pressed(Escape) {
                run.push("exit_fullscreen");
            }
            if i.key_pressed(F11) {
                run.push("toggle_fullscreen");
            }
        });
        for name in run {
            self.run_named(name);
        }
    }
}

impl eframe::App for RhemaFlowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Evidence for "it stopped listening": if the UI was not drawn for >1.5 s while
        // live, the OS throttled this window (App Nap, occluded, screen saver). Audio keeps
        // running on its own threads, but nothing reaches the screen until the UI wakes.
        let gap_ms = self.last_update.elapsed().as_millis() as u64;
        self.last_update = Instant::now();
        if self.live && gap_ms > 1500 {
            let sm = self.engine.as_ref().map(|e| e.service_ms.load(std::sync::atomic::Ordering::Relaxed)).unwrap_or(0);
            if let Some(log) = &mut self.event_log {
                log.append(sm, "ui_stall", &format!("window not drawn for {gap_ms} ms (OS throttling?)"));
            }
            self.short_utterance_note = Some((format!("screen was paused by macOS for {:.1}s - audio kept recording", gap_ms as f32 / 1000.0), Instant::now()));
        }
        theme::apply(ctx);
        if !self.pending_edit_events.is_empty() {
            let evs = std::mem::take(&mut self.pending_edit_events);
            ctx.input_mut(|i| i.events.extend(evs));
        }
        self.present.last_backup_unix = self.x.p.last_backup as i64; // for the Sunday check
        self.tts.poll();
        if self.tts.speaking() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        self.poll_folder_pick();
        self.poll_versions();
        self.web_tick();
        self.poll_events(ctx);
        self.handle_shortcuts(ctx);

        // the window title carries the version, and says LIVE / REC while a service runs
        {
            let t = format!("{}{}", if self.live { "\u{25cf} LIVE \u{b7} " } else { "" }, theme::TITLE);
            if t != self.last_title {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(t.clone()));
                self.last_title = t;
            }
        }
        if self.palette_open {
            self.ui_palette(ctx);
        }

        self.present_frame(ctx);
        self.x_tick(ctx);
        self.order_collect();
        self.order.tick();
        if !crate::macmenu::NATIVE {
            self.ui_menubar(ctx);
        }
        for id in crate::macmenu::take_events() {
            self.flash_menu(&id);
            // v0.844.0: with NO text box active, Cut / Copy / Paste / Select All work on the MEDIA bar's files and on songs
            if matches!(id.as_str(), "cut" | "copy" | "paste" | "select_all") && !ctx.wants_keyboard_input() {
                self.edit_outside_text(ctx, &id, None);
                continue;
            }
            // Cut / Copy / Paste / Select All go to the text box that has the cursor (v0.840.0)
            if let Some(evs) = crate::macmenu::clipboard_events(&id, crate::macmenu::read_clipboard) {
                // v0.845.0: the Songs window is drawn BEFORE this point, so events added now were never seen by its text
                // boxes (Select All / Copy / Paste did nothing there). Hold them for the start of the next frame instead.
                self.pending_edit_events.extend(evs);
                ctx.request_repaint();
                continue;
            }
            self.menu_action(ctx, &id);
        }
        self.draw_menu_flash(ctx);
        self.ui_header(ctx);
        // A/B/C window layouts (v0.831.0). A: transcript left + tabbed dock right.
        // B: transcript hidden, wider dock. C: focus — big preview, transcript right.
        match self.layout {
            LayoutMode::Console => {
                if !self.hide_transcript {
                    self.ui_transcript(ctx, false);
                }
                self.ui_queue(ctx, false);
                self.ui_controls(ctx);
                self.ui_media_dock(ctx);
                self.ui_browser(ctx);
                self.ui_center(ctx);
            }
            LayoutMode::DockFirst => {
                self.ui_queue(ctx, true);
                self.ui_controls(ctx);
                self.ui_media_dock(ctx);
                self.ui_browser(ctx);
                self.ui_center(ctx);
            }
            LayoutMode::Focus => {
                if !self.hide_transcript {
                    self.ui_transcript(ctx, true);
                }
                self.ui_controls(ctx);
                self.ui_browser(ctx);
                self.ui_center_focus(ctx);
            }
        }

        // ---- stage window (immediate viewport) ----
        // v0.814.0: "full screen" = a borderless window sized exactly to the chosen screen
        // (screen 2 by default). No macOS native full screen, so no separate Space to get stuck in.
        if !self.stage_visible {
            self.out_sent = None;
        }
        if self.stage_visible {
            let stage = self.stage.clone();
            let art = self.present.art(&stage.kind);
            let church = self.church_name.clone();
            let caption = self.x.caption_for_stage(&self.caption_now());
            let output = self.stage_fullscreen;
            let zoom = self.stage_zoom * self.x.p.access.zoom_factor();
            let (abg, afg) = self.x.p.access.stage_colors(self.stage_bg, self.stage_fg);
            let look = stage::StageLook { style: self.stage_style, blackout: self.blackout, bg: abg, fg: afg, fade_ms: self.stage_fade_ms };
            let ov = self.x.overlays(self.blackout);
            let mut closed = false;
            let mut leave = false;
            let vp_id = egui::ViewportId(egui::Id::new("rf-stage"));
            let target = crate::displays::choose(&self.displays, self.output_display.as_deref()).map(|i| self.displays[i].clone());
            let placement = target.as_ref().map(|d| crate::displays::place(d, output));
            let key = placement.map(|((x, y), (w, h))| (output, x as i32, y as i32, w as i32, h as i32));
            let mut builder = egui::ViewportBuilder::default()
                .with_title(format!("{} \u{2014} Stage", theme::TITLE))
                .with_inner_size([960.0, 540.0])
                .with_decorations(!output)
                .with_window_level(if output { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal });
            if let Some(((x, y), (w, h))) = placement {
                builder = builder.with_position([x, y]).with_inner_size([w, h]);
            }
            // changes after the window exists are sent as commands, once
            if self.out_sent.is_some() && self.out_sent != key {
                ctx.send_viewport_cmd_to(vp_id, egui::ViewportCommand::Decorations(!output));
                ctx.send_viewport_cmd_to(vp_id, egui::ViewportCommand::WindowLevel(if output { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal }));
                if let Some(((x, y), (w, h))) = placement {
                    ctx.send_viewport_cmd_to(vp_id, egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
                    ctx.send_viewport_cmd_to(vp_id, egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
                } else if !output {
                    ctx.send_viewport_cmd_to(vp_id, egui::ViewportCommand::InnerSize(egui::vec2(960.0, 540.0)));
                }
            }
            let newkey = key.or(Some((output, 0, 0, 0, 0)));
            if self.out_sent != newkey {
                // a new placement: forget old corrections and start from the plain numbers
                self.stage_tries = 0;
                self.stage_report.clear();
                if let Some((p, s)) = placement {
                    self.stage_cmd = (p, s);
                }
            }
            self.out_sent = newkey;
            let mut got: (Option<egui::Rect>, Option<egui::Rect>) = (None, None);
            ctx.show_viewport_immediate(vp_id, builder, |vp_ctx, _class| {
                if vp_ctx.input(|i| i.viewport().close_requested()) {
                    closed = true;
                }
                got = vp_ctx.input(|i| (i.viewport().inner_rect, i.viewport().outer_rect));
                // ways out from inside the stage window itself: Esc, F11, double-click
                if output
                    && vp_ctx.input(|i| {
                        i.key_pressed(egui::Key::F11) || i.key_pressed(egui::Key::Escape) || i.pointer.button_double_clicked(egui::PointerButton::Primary)
                    })
                {
                    leave = true;
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(egui::Color32::from_rgb(look.bg[0], look.bg[1], look.bg[2])))
                    .show(vp_ctx, |ui| {
                        stage::draw_stage(ui, &stage, true, zoom, look, &art);
                        crate::extras::paint_overlays(ui, ui.max_rect(), &ov, look.fg, look.bg, true);
                        if !caption.is_empty() {
                            // live captions: a soft dark strip near the bottom, large readable text
                            let r = ui.max_rect();
                            let w = (r.width() * 0.82).min(1500.0);
                            let size = (r.height() * 0.045).clamp(18.0, 56.0);
                            let galley = ui.painter().layout(caption.clone(), egui::FontId::proportional(size), egui::Color32::WHITE, w - 40.0);
                            let h = galley.size().y + 24.0;
                            let box_rect = egui::Rect::from_center_size(egui::pos2(r.center().x, r.bottom() - h * 0.5 - r.height() * 0.06), egui::vec2(w, h));
                            ui.painter().rect_filled(box_rect, 10.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 175));
                            ui.painter().galley(egui::pos2(box_rect.left() + 20.0, box_rect.top() + 12.0), galley, egui::Color32::WHITE);
                        }
                        if !church.is_empty() {
                            ui.painter().text(
                                egui::pos2(ui.available_rect_before_wrap().right() - 24.0, ui.available_rect_before_wrap().bottom() - 24.0),
                                egui::Align2::RIGHT_BOTTOM,
                                &church,
                                egui::FontId::new(15.0, egui::FontFamily::Name(stage::SERIF.into())),
                                egui::Color32::from_rgba_unmultiplied(150, 156, 168, 170),
                            );
                        }
                    });
                // a small exit button that appears when the mouse moves (hidden while still)
                if output && vp_ctx.input(|i| i.pointer.time_since_last_movement() < 3.0) {
                    egui::Area::new(egui::Id::new("rf-exit-fs"))
                        .anchor(egui::Align2::RIGHT_TOP, [-16.0, 16.0])
                        .show(vp_ctx, |ui| {
                            if ui.button("Exit full screen  (Esc)").clicked() {
                                leave = true;
                            }
                        });
                }
            });
            // look at what the window REALLY is and fix it if it is not the whole screen
            if let (true, Some(((px, py), (pw, ph))), (Some(inner), Some(outer))) = (output, placement, got) {
                let sz = crate::displays::corrected_size(self.stage_cmd.1, (pw, ph), (inner.width(), inner.height()));
                let ps = crate::displays::corrected_pos(self.stage_cmd.0, (px, py), (outer.min.x, outer.min.y));
                if sz.is_none() && ps.is_none() {
                    self.stage_report = format!("Stage window: {}\u{00d7}{} at {},{}. Fills the screen.", inner.width() as i32, inner.height() as i32, outer.min.x as i32, outer.min.y as i32);
                } else if self.stage_tries < 8 {
                    self.stage_tries += 1;
                    if let Some(s) = sz {
                        self.stage_cmd.1 = s;
                    }
                    if let Some(p) = ps {
                        self.stage_cmd.0 = p;
                    }
                    let (cp, cs) = self.stage_cmd;
                    ctx.send_viewport_cmd_to(vp_id, egui::ViewportCommand::OuterPosition(egui::pos2(cp.0, cp.1)));
                    ctx.send_viewport_cmd_to(vp_id, egui::ViewportCommand::InnerSize(egui::vec2(cs.0, cs.1)));
                    ctx.request_repaint();
                } else {
                    self.stage_report = format!("Stage window is {}\u{00d7}{} but the screen is {}\u{00d7}{} (tried {} times).", inner.width() as i32, inner.height() as i32, pw as i32, ph as i32, self.stage_tries);
                }
            }
            if leave {
                self.stage_fullscreen = false;
            }
            if closed {
                self.stage_visible = false;
                self.stage_fullscreen = false;
            }
        }

        // ---- saved / settings / commands windows ----
        self.ui_save_window(ctx);
        self.ui_versions_window(ctx);
        self.ui_plan_window(ctx);
        self.ui_history_window(ctx);
        self.tick_account(ctx);
        self.tick_preflight();
        self.ui_stage_monitor(ctx);
        self.ui_preflight_window(ctx);
        self.ui_end_confirm(ctx);
        self.ui_account_window(ctx);
        self.ui_web_window(ctx);
        self.ui_saved_window(ctx);
        self.ui_commands_window(ctx);

        // ---- help overlay ----
        if self.help_visible {
            self.ui_help(ctx);
        }
        if self.about_visible {
            self.ui_about(ctx);
        }
        self.ui_extras(ctx);
        self.x_screens(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.recording {
            self.set_recording(false);
        }
        self.asr_tx = None;
        self.engine = None;
        self.x_mark_clean_exit();
    }
}

// ---------------------------------------------------------------------------
// fonts + settings persistence
// ---------------------------------------------------------------------------

pub fn load_stage_fonts(ctx: &egui::Context) {
    load_stage_fonts_with(ctx, crate::access::StageFont::Serif)
}

pub fn load_stage_fonts_with(ctx: &egui::Context, which: crate::access::StageFont) {
    let mut fonts = egui::FontDefinitions::default();
    // egui's normal text fonts lack the arrows, triangles and bullets the buttons use (they showed as empty boxes);
    // the built-in "Hack" font has them, so it becomes the last fallback for normal text too
    if let Some(p) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        if !p.iter().any(|n| n == "Hack") {
            p.push("Hack".to_owned());
        }
    }
    let regular = include_bytes!("../assets/LiberationSerif-Regular.ttf");
    let bold = include_bytes!("../assets/LiberationSerif-Bold.ttf");
    fonts.font_data.insert(
        "liberation_serif".into(),
        egui::FontData::from_static(regular),
    );
    fonts.font_data.insert(
        "liberation_serif_bold".into(),
        egui::FontData::from_static(bold),
    );
    let mut stack: Vec<String> = vec!["liberation_serif".into(), "liberation_serif_bold".into()];
    match which {
        crate::access::StageFont::Dyslexic => {
            fonts.font_data.insert("opendyslexic".into(), egui::FontData::from_static(include_bytes!("../assets/OpenDyslexic-Regular.otf")));
            stack.insert(0, "opendyslexic".into());
        }
        crate::access::StageFont::Hyperlegible => {
            fonts.font_data.insert("atkinson".into(), egui::FontData::from_static(include_bytes!("../assets/AtkinsonHyperlegible-Regular.ttf")));
            stack.insert(0, "atkinson".into());
        }
        _ => {}
    }
    fonts.families.insert(egui::FontFamily::Name(stage::SERIF.into()), stack);
    ctx.set_fonts(fonts);
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Settings {
    church_name: Option<String>,
    floor: Option<f32>,
    translation: Option<String>,
    device: Option<String>,
    tts_voice: Option<String>,
    tts_rate: Option<u32>,
    auto_read: Option<bool>,
    stage_zoom: Option<f32>,
    auto_approve_quotes: Option<bool>,
    show_all_matches: Option<bool>,
    // v0.890.0 — the adjustable suggestion/auto-show gates (see ui.rs)
    suggest_min_words: Option<u8>,
    suggest_min_pct: Option<u8>,
    auto_min_words: Option<u8>,
    auto_stage_pct: Option<u8>,
    near_tie_card: Option<bool>,
    quote_hold_ms: Option<u64>,
    mute_mic_while_reading: Option<bool>,
    save_root: Option<String>,
    save_txt: Option<bool>,
    save_srt: Option<bool>,
    save_md: Option<bool>,
    compress_audio: Option<bool>,
    browser_visible: Option<bool>,
    stage_style: Option<String>,
    web_enabled: Option<bool>,
    web_lan: Option<bool>,
    web_port: Option<u16>,
    web_pin: Option<String>,
    output_display: Option<String>,
    stage_bg: Option<[u8; 3]>,
    stage_fg: Option<[u8; 3]>,
    #[serde(default)]
    stage_fade_ms: Option<u32>,
    #[serde(default)]
    layout: Option<String>,
    #[serde(default)]
    hide_transcript: Option<bool>,
    #[serde(default)]
    right_tab: Option<String>,
}

fn corrections_path() -> Option<PathBuf> {
    settings_path().and_then(|p| p.parent().map(|d| d.join("corrections.tsv")))
}

/// Load what was learned from earlier corrections, and tell Whisper the names.
fn load_learner() -> rf_core::learn::Learner {
    let l = corrections_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| rf_core::learn::Learner::from_text(&t)).unwrap_or_default();
    rf_asr::engines::set_extra_prompt(&l.prompt_words(200));
    l
}

impl RhemaFlowApp {
    /// The operator fixed a transcript line: learn from it, save, refresh Whisper's names.
    pub fn correct_line(&mut self, idx: usize, fixed: &str) {
        let Some(line) = self.transcript.get(idx).cloned() else { return };
        let fixed = fixed.trim();
        if fixed.is_empty() || fixed == line.text {
            return;
        }
        let learned = self.learner.record(&line.text, fixed);
        self.transcript[idx].text = fixed.to_string();
        if let Some(l) = self.export_lines.iter_mut().rev().find(|l| l.ms == line.service_ms) {
            l.text = fixed.to_string();
        }
        if learned {
            self.save_learner();
        }
        self.short_utterance_note = Some((if learned { "fixed, and remembered (after the same fix twice it is applied by itself)".into() } else { "fixed".into() }, Instant::now()));
    }

    pub fn save_learner(&self) {
        rf_asr::engines::set_extra_prompt(&self.learner.prompt_words(200));
        if let Some(p) = corrections_path() {
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let tmp = p.with_extension("tsv.tmp");
            if std::fs::write(&tmp, self.learner.to_text()).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".rhemaflow").join("settings.json"))
}

fn load_settings() -> Settings {
    if let Some(p) = settings_path() {
        if let Ok(text) = std::fs::read_to_string(p) {
            return serde_json::from_str(&text).unwrap_or_default();
        }
    }
    Settings::default()
}

pub fn save_settings(app: &RhemaFlowApp) {
    if let Some(p) = settings_path() {
        let s = Settings {
            church_name: Some(app.church_name.clone()).filter(|s| !s.is_empty()),
            floor: Some(app.floor),
            translation: Some(app.translation.clone()),
            device: app.selected_device.clone(),
            tts_voice: app.tts_voice.clone(),
            tts_rate: Some(app.tts_rate),
            auto_read: Some(app.auto_read),
            stage_zoom: Some(app.stage_zoom),
            auto_approve_quotes: Some(app.auto_approve_quotes),
            show_all_matches: Some(app.show_all_matches),
            // v0.890.0 — the adjustable gates saved in settings.json
            suggest_min_words: Some(app.parse_ctx.quote_gates.min_words),
            suggest_min_pct: Some(app.parse_ctx.quote_gates.min_pct),
            auto_min_words: Some(app.auto_gates.min_words),
            auto_stage_pct: Some(app.auto_gates.min_pct),
            near_tie_card: Some(app.auto_gates.near_tie_card),
            quote_hold_ms: Some(app.quote_hold_ms),
            mute_mic_while_reading: Some(app.mute_mic_while_reading),
            save_root: Some(app.save_root.clone()).filter(|s| !s.is_empty()),
            save_txt: Some(app.formats.txt),
            save_srt: Some(app.formats.srt),
            save_md: Some(app.formats.md),
            compress_audio: Some(app.compress_audio),
            browser_visible: Some(app.browser.visible),
            stage_style: Some(app.stage_style.key().to_string()),
            web_enabled: Some(app.web_enabled),
            web_lan: Some(app.web_lan),
            web_port: Some(app.web_port),
            web_pin: Some(app.web_pin.clone()),
            output_display: app.output_display.clone(),
            stage_bg: Some(app.stage_bg),
            stage_fg: Some(app.stage_fg),
            stage_fade_ms: Some(app.stage_fade_ms),
            layout: Some(app.layout.key().to_string()),
            hide_transcript: Some(app.hide_transcript),
            right_tab: Some(app.right_tab.key().to_string()),
        };
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        std::fs::write(p, serde_json::to_string_pretty(&s).unwrap_or_default()).ok();
    }
}
