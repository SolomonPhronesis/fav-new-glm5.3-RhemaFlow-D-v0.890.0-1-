//! End-to-end checks of the service order inside the real app (headless).
use super::*;

fn new_app() -> (app::RhemaFlowApp, egui::Context) {
    std::env::set_var("HOME", std::env::temp_dir().join(format!("rf-order-home-{}", std::process::id())));
    let ctx = egui::Context::default();
    let cc = eframe::CreationContext { egui_ctx: ctx.clone(), _p: Default::default() };
    (app::RhemaFlowApp::new(&cc, state::AsrMode::Offline, "".into()), ctx)
}

fn frame(app: &mut app::RhemaFlowApp, ctx: &egui::Context) -> egui::FullOutput {
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0)));
    ctx.run(raw, |ctx| eframe::App::update(app, ctx, &mut eframe::Frame))
}

fn all_text(out: &egui::FullOutput) -> String {
    fn collect(s: &egui::epaint::Shape, v: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Text(t) => v.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(l) => l.iter().for_each(|x| collect(x, v)),
            _ => {}
        }
    }
    let mut v = vec![];
    for cs in &out.shapes {
        collect(&cs.shape, &mut v);
    }
    v.join(" | ")
}

#[test]
fn a_dragged_media_item_lands_in_the_order_and_shows_when_clicked_and_the_panel_draws() {
    let (mut app, ctx) = new_app();
    app.right_tab = order::RightTab::Order;
    // a scripture on the stage can be added, kept, and shown again later
    app.do_stage(rf_core::Passage { book: 42, chapter: 3, from: 16, to: 16 }, "test", 1.0, false);
    assert_eq!(app.stage.kind, state::StageKind::Scripture);
    let label = app.stage.label.clone();
    app.order.runner.add_item(&label, rf_service::ItemKind::Scripture { reference: label.clone(), translation: "KJV".into() }).unwrap();
    app.clear_stage();
    assert_eq!(app.stage.kind, state::StageKind::Blank);
    assert!(app.order.runner.next());
    app.order_apply(&ctx);
    assert_eq!(app.stage.kind, state::StageKind::Scripture, "Next shows the scripture again");
    assert!(app.stage.text.to_lowercase().contains("god so loved"), "{}", app.stage.text);
    // Black blanks the output but Clear/Undo bring things back
    app.order.runner.black();
    app.order_apply(&ctx);
    assert!(app.blackout);
    assert!(app.order.runner.undo());
    app.order_apply(&ctx);
    assert!(!app.blackout);
    // slides and a timer in the list
    app.order.runner.add_text_item("Notices", "Welcome\n\nTea after the service").unwrap();
    app.order.runner.add_item("Countdown", rf_service::ItemKind::Timer { seconds: 120, label: "Starting soon".into() }).unwrap();
    assert!(app.order.runner.next());
    app.order_apply(&ctx);
    assert_eq!(app.stage.kind, state::StageKind::Slide);
    assert_eq!(app.stage.text, "Welcome");
    assert!(app.order.runner.next());
    app.order_apply(&ctx);
    assert_eq!(app.stage.text, "Tea after the service");
    assert!(app.order.runner.next());
    app.order_apply(&ctx);
    assert_eq!(app.stage.kind, state::StageKind::Countdown);
    // the panel draws without trouble, lists every item and marks the live one
    let mut out = frame(&mut app, &ctx);
    for _ in 0..3 {
        out = frame(&mut app, &ctx);
    }
    let t = all_text(&out);
    for want in ["SERVICE ORDER", "Next", "Clear", "Black", "Undo", "Notices", "Countdown", "ON SCREEN", &label] {
        assert!(t.contains(want), "panel is missing {want:?}: {t}");
    }
}

#[test]
fn a_missing_file_in_the_order_does_not_blank_the_screen_and_says_why() {
    let (mut app, ctx) = new_app();
    app.do_stage(rf_core::Passage { book: 42, chapter: 3, from: 16, to: 16 }, "test", 1.0, false);
    let id = app.order.runner.add_item("Gone", rf_service::ItemKind::Media { path: "/nowhere/gone.mp4".into() }).unwrap();
    app.order.runner.go(id, 0).unwrap();
    app.order_apply(&ctx);
    assert_eq!(app.stage.kind, state::StageKind::Scripture, "the old content stays");
    assert!(app.present.msg.as_ref().map_or(false, |m| m.0.contains("missing")), "{:?}", app.present.msg);
    assert_eq!(app.order.live(), None, "the list does not claim the missing file is on screen");
}

#[test]
fn a_sound_file_goes_on_the_shelf_into_the_order_and_a_bad_one_is_explained() {
    let (mut app, ctx) = new_app();
    let d = std::env::temp_dir().join(format!("rf-order-sound-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let mp3 = d.join("Walk In.mp3");
    std::fs::write(&mp3, b"ID3 not really music but the name is right").unwrap();
    app.present.route_path(&ctx, &mp3, present::PickFor::Media);
    // wait for the helper thread
    for _ in 0..200 {
        app.present.poll_jobs();
        if app.present.jobs_pending() == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(app.present.shelf.entries.len(), 1, "an mp3 is accepted onto the shelf: {:?}", app.present.msg);
    let id = app.present.shelf.entries[0].id;
    app.order_add(present::OrderAdd::Media(id));
    assert_eq!(app.order.items().len(), 1);
    assert_eq!(order::kind_word(&app.order.items()[0].kind), "sound");
    // an ogg is refused with advice
    let ogg = d.join("x.ogg");
    std::fs::write(&ogg, b"OggS").unwrap();
    app.present.route_path(&ctx, &ogg, present::PickFor::Media);
    assert!(app.present.msg.as_ref().map_or(false, |m| m.0.to_lowercase().contains("convert")), "{:?}", app.present.msg);
}
