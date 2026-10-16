//! Headless probe for v0.841.0 (scratch, not shipped): runs real frames of the real UI and reads back what was
//! painted: dock tabs on one line at narrow widths, the order panel's details and time strip, hide-transcript,
//! paste-a-reference, and copy-verse.
use super::*;
use crate::state::{LayoutMode, StageKind};
use rf_service::ItemKind;
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

fn new_app(ctx: &egui::Context) -> app::RhemaFlowApp {
    let home = std::env::temp_dir().join(format!("rf-v0841-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let cc = eframe::CreationContext { egui_ctx: ctx.clone(), _p: Default::default() };
    app::RhemaFlowApp::new(&cc, state::AsrMode::Offline, "".into())
}

fn settle(app: &mut app::RhemaFlowApp, ctx: &egui::Context, w: f32, h: f32) -> Vec<(String, egui::Pos2)> {
    for _ in 0..4 {
        let _ = frame(app, ctx, w, h);
    }
    painted(&frame(app, ctx, w, h))
}

#[test]
fn the_dock_tabs_stay_on_one_line_at_every_laptop_size() {
    for (w, h) in [(1000.0, 700.0), (1100.0, 700.0), (1280.0, 720.0), (1440.0, 900.0)] {
        for layout in [LayoutMode::Console, LayoutMode::DockFirst] {
            let ctx = egui::Context::default();
            let mut app = new_app(&ctx);
            app.layout = layout;
            let t = settle(&mut app, &ctx, w, h);
            let tabs: Vec<_> = t.iter().filter(|x| x.1.y < 130.0 && x.1.x > w * 0.5 && ["Order", "Review", "Rev", "Strong's", "G/H", "History", "Hist", "Index", "Idx", "Ord"].iter().any(|k| x.0 == *k || x.0.starts_with(&format!("{k} ")))).collect();
            assert!(tabs.len() >= 5, "{w}x{h} {layout:?}: expected the five tabs, saw {:?}", tabs.iter().map(|x| &x.0).collect::<Vec<_>>());
            let y0 = tabs[0].1.y;
            assert!(tabs.iter().all(|x| (x.1.y - y0).abs() < 2.0), "{w}x{h} {layout:?}: tabs wrapped onto a second line: {:?}", tabs.iter().map(|x| (&x.0, x.1.y)).collect::<Vec<_>>());
            assert!(tabs.iter().all(|x| x.1.x < w), "{w}x{h}: a tab was painted outside the window");
        }
    }
}

#[test]
fn hide_transcript_hides_it_and_the_choice_is_remembered() {
    let ctx = egui::Context::default();
    let mut app = new_app(&ctx);
    assert!(settle(&mut app, &ctx, 1280.0, 800.0).iter().any(|x| x.0 == "TRANSCRIPT"));
    app.menu_action(&ctx, "toggle_transcript");
    assert!(settle(&mut app, &ctx, 1280.0, 800.0).iter().all(|x| x.0 != "TRANSCRIPT"), "hidden");
    let home = std::env::var("HOME").unwrap();
    let saved = std::fs::read_to_string(std::path::Path::new(&home).join(".rhemaflow").join("settings.json")).unwrap_or_default();
    assert!(saved.contains("\"hide_transcript\": true") && saved.contains("\"right_tab\""), "remembered: {saved}");
    app.menu_action(&ctx, "toggle_transcript");
    assert!(settle(&mut app, &ctx, 1280.0, 800.0).iter().any(|x| x.0 == "TRANSCRIPT"), "back again");
}

#[test]
fn the_order_panel_shows_details_the_time_strip_and_what_is_next() {
    let ctx = egui::Context::default();
    let mut app = new_app(&ctx);
    let a = app.order.add("Welcome", ItemKind::Timer { seconds: 60, label: "x".into() }).unwrap();
    let b = app.order.add("Offering", ItemKind::Timer { seconds: 60, label: "y".into() }).unwrap();
    app.order.set_minutes(a, Some(5));
    app.order.set_notes(b, "ushers first");
    app.order.runner.go(a, 0).unwrap();
    app.right_tab = crate::order::RightTab::Order;
    app.order.expanded = Some(b);
    let t = settle(&mut app, &ctx, 1280.0, 800.0);
    let has = |s: &str| t.iter().any(|x| x.0.contains(s));
    assert!(has("Undo edit") && has("Redo edit") && has("Copy run sheet") && has("Undo screen"), "buttons");
    assert!(has("left"), "the countdown for the item with planned minutes: {:?}", t.iter().map(|x| &x.0).filter(|s| s.contains("left") || s.contains("screen")).collect::<Vec<_>>());
    assert!(has("Next: Offering"), "what comes next");
    assert!(has("Left 5 min of 5 min") && has("(1 without a time)"), "the total");
    assert!(has("Private note (never shown on the stage)") && has("Duplicate"), "the open details");
    assert!(has("5 min") && has("note"), "the small markers in the row");
}

#[test]
fn pasting_a_reference_jumps_to_it_and_bad_text_says_so() {
    let ctx = egui::Context::default();
    let mut app = new_app(&ctx);
    let n = app.clip_go_to_text("  John 3:16-17 \n");
    assert_eq!(app.stage.kind, StageKind::Scripture, "{n}");
    assert!(app.stage.label.contains("John 3:16"), "{}", app.stage.label);
    assert!(n.starts_with("Jumped to"), "{n}");
    let before = app.stage.label.clone();
    assert!(app.clip_go_to_text("").contains("empty"));
    assert!(app.clip_go_to_text("hello there everyone").starts_with("Not a reference"));
    assert_eq!(app.stage.label, before, "nothing changed on the stage");
    let n = app.clip_go_to_text("Romans 8");
    assert!(n.contains("Opened") && app.browser.visible, "{n}");
    assert_eq!(app.stage.label, before, "a whole chapter is never thrown on the stage by a paste");
}

#[test]
fn copy_verse_copies_what_is_on_the_stage_and_says_when_nothing_is() {
    let ctx = egui::Context::default();
    let mut app = new_app(&ctx);
    app.clip_copy_verse(&ctx, crate::clip::CopyKind::Text);
    assert!(app.short_utterance_note.as_ref().unwrap().0.contains("no scripture"));
    app.clip_go_to_text("John 3:16");
    let mut copied = String::new();
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)));
    let out = ctx.run(raw, |ctx| app.clip_copy_verse(ctx, crate::clip::CopyKind::WithReference));
    copied.push_str(&out.platform_output.copied_text);
    assert!(copied.contains("John 3:16") && copied.to_lowercase().contains("loved"), "{copied}");
}
