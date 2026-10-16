//! Headless probe for the v0.831.0 layout work (scratch, not shipped): the A/B/C
//! layout switch, the blue/green transcript panel, the colour swatches under the
//! preview, and the Index-tab de-duplication. Runs real frames of the real UI and
//! reads back what was painted where.
use super::*;
use crate::state::{LayoutMode, LineKind};
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

fn frame(app: &mut app::RhemaFlowApp, ctx: &egui::Context, w: f32, h: f32) -> egui::FullOutput {
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h)));
    ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame);
    })
}

fn painted(out: &egui::FullOutput) -> Vec<(String, egui::Pos2)> {
    fn collect(s: &egui::epaint::Shape, v: &mut Vec<(String, egui::Pos2)>) {
        match s {
            egui::epaint::Shape::Text(t) => v.push((t.galley.text().to_string(), t.pos)),
            egui::epaint::Shape::Vec(list) => list.iter().for_each(|x| collect(x, v)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for cs in &out.shapes {
        collect(&cs.shape, &mut v);
    }
    v
}

/// a fresh app with its OWN home dir, so parallel tests never share settings
fn new_app(ctx: &egui::Context) -> (app::RhemaFlowApp, std::path::PathBuf) {
    let home = std::env::temp_dir().join(format!("rf-v0831-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let cc = eframe::CreationContext { egui_ctx: ctx.clone(), _p: Default::default() };
    (app::RhemaFlowApp::new(&cc, state::AsrMode::Offline, "".into()), home)
}

const W: f32 = 1280.0;
const H: f32 = 800.0;

#[test]
fn transcript_kinds_are_colour_coded() {
    let ctx = egui::Context::default();
    let (mut app, _home) = new_app(&ctx);
    // a plain sentence: no scripture, no command, 6+ words, no digits
    app.handle_parse("we are gathered here this morning friends", 1_000, 10);
    assert_eq!(app.transcript.last().unwrap().kind, LineKind::Plain, "ordinary speech stays plain");
    // a voice command line goes blue
    app.handle_parse("Next.", 2_000, 10);
    assert_eq!(app.transcript.last().unwrap().kind, LineKind::Command, "a command goes blue");
    // a scripture line goes green (staged or queued — either way it was about a verse)
    app.handle_parse("Let's turn to John chapter 3 verse 16.", 3_000, 10);
    assert_eq!(app.transcript.last().unwrap().kind, LineKind::Verse, "a scripture goes green");
}

#[test]
fn layouts_a_b_c_paint_the_right_panels() {
    // ---- A: transcript LEFT, dock RIGHT, nine swatches ----
    let ctx = egui::Context::default();
    let (mut app, _home) = new_app(&ctx);
    for _ in 0..4 {
        let _ = frame(&mut app, &ctx, W, H);
    }
    let t = painted(&frame(&mut app, &ctx, W, H));
    let tr = t.iter().find(|x| x.0 == "TRANSCRIPT").expect("layout A shows the transcript");
    assert!(tr.1.x < 320.0, "layout A transcript sits on the LEFT, got x={}", tr.1.x);
    let swatches = t.iter().filter(|x| x.0 == "Aa").count();
    assert_eq!(swatches, 9, "layout A shows the nine display colour swatches under the preview, got {swatches}");

    // ---- B: transcript HIDDEN, dock wider ----
    let ctx = egui::Context::default();
    let (mut app, _home) = new_app(&ctx);
    app.layout = LayoutMode::DockFirst;
    for _ in 0..4 {
        let _ = frame(&mut app, &ctx, W, H);
    }
    let t = painted(&frame(&mut app, &ctx, W, H));
    assert!(t.iter().all(|x| x.0 != "TRANSCRIPT"), "layout B hides the transcript");

    // ---- C: transcript RIGHT, big preview, no dock ----
    let ctx = egui::Context::default();
    let (mut app, _home) = new_app(&ctx);
    app.layout = LayoutMode::Focus;
    for _ in 0..4 {
        let _ = frame(&mut app, &ctx, W, H);
    }
    let t = painted(&frame(&mut app, &ctx, W, H));
    let tr = t.iter().find(|x| x.0 == "TRANSCRIPT").expect("layout C shows the transcript");
    assert!(tr.1.x > W * 0.5, "layout C transcript sits on the RIGHT, got x={}", tr.1.x);
    // the dock (and its NEEDS YOUR REVIEW strip) is gone in C
    assert!(t.iter().all(|x| x.0 != "NEEDS YOUR REVIEW"), "layout C hides the dock");
}

#[test]
fn layout_choice_is_saved_and_unknown_keys_fall_back() {
    let ctx = egui::Context::default();
    let (mut app, home) = new_app(&ctx);
    app.layout = LayoutMode::Focus;
    crate::app::save_settings(&app);
    let txt = std::fs::read_to_string(home.join(".rhemaflow/settings.json")).unwrap();
    assert!(txt.contains("\"focus\""), "settings.json records the chosen layout: {txt}");
    // unknown keys fall back to layout A, never to a blank screen
    assert_eq!(LayoutMode::from_key("nonsense"), LayoutMode::Console);
}

#[test]
fn index_tab_owns_the_index_middle_keeps_the_lists() {
    // the de-duplication: with the Index tab forward, the timestamped index
    // appears once (in the tab) and the middle keeps the working lists —
    // before v0.831.0 the same index grid was painted in BOTH places.
    let ctx = egui::Context::default();
    let (mut app, _home) = new_app(&ctx);
    app.right_tab = crate::order::RightTab::Index;
    for _ in 0..4 {
        let _ = frame(&mut app, &ctx, W, H);
    }
    let t = painted(&frame(&mut app, &ctx, W, H));
    let n_index = t.iter().filter(|x| x.0.contains("SCRIPTURE INDEX")).count();
    assert_eq!(n_index, 1, "the timestamped index appears exactly once (Index tab), got {n_index}");
    let n_lists = t.iter().filter(|x| x.0 == "PLANNED (click to show)").count();
    assert_eq!(n_lists, 1, "the working scripture lists stay in the middle, got {n_lists}");
}
