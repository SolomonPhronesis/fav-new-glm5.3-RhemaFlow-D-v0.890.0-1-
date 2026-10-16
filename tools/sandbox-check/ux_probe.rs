//! Window-size probe: at laptop sizes, does the stage preview run into the panel below it?
use super::*;
use egui::epaint::Shape;
fn walk(s: &Shape, t: &mut Vec<(String, egui::Pos2)>, r: &mut Vec<egui::Rect>) {
    match s {
        Shape::Text(x) => t.push((x.galley.text().to_string(), x.pos)),
        Shape::Rect(x) => r.push(x.rect),
        Shape::Vec(l) => l.iter().for_each(|y| walk(y, t, r)),
        _ => {}
    }
}
#[test]
fn the_stage_preview_never_runs_into_the_scripture_panel_at_laptop_sizes() {
    std::env::set_var("HOME", std::env::temp_dir().join("rf-ux-home"));
    for (w, h) in [(1512.0_f32, 900.0_f32), (1440.0, 820.0), (1280.0, 720.0), (1100.0, 700.0)] {
        let ctx = egui::Context::default();
        let cc = eframe::CreationContext { egui_ctx: ctx.clone(), _p: Default::default() };
        let mut app = app::RhemaFlowApp::new(&cc, state::AsrMode::Offline, "".into());
        let mut out = egui::FullOutput::default();
        for _ in 0..5 {
            let mut raw = egui::RawInput::default();
            raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h)));
            out = ctx.run(raw, |c| eframe::App::update(&mut app, c, &mut eframe::Frame));
        }
        let (mut t, mut r) = (vec![], vec![]);
        for cs in &out.shapes { walk(&cs.shape, &mut t, &mut r); }
        let stage = r.iter().filter(|x| x.width() > 150.0 && (x.width() / x.height() - 16.0 / 9.0).abs() < 0.05).max_by(|a, b| a.width().partial_cmp(&b.width()).unwrap());
        let y = |s: &str| t.iter().find(|x| x.0.starts_with(s)).map(|x| x.1.y);
        let (sb, sy) = (stage.map(|s| s.bottom()), y("SCRIPTURE"));
        eprintln!("UX {w}x{h}: preview {:?} bottom={:?} | SCRIPTURE y={:?} MEDIA y={:?}", stage.map(|s| (s.width().round(), s.height().round())), sb, sy, y("MEDIA"));
        if let (Some(b), Some(s)) = (sb, sy) {
            assert!(b <= s + 1.0, "{w}x{h}: the preview ends at {b} but the Scripture panel starts at {s}");
        }
        assert!(stage.map_or(false, |s| s.width() >= 150.0), "{w}x{h}: preview missing or tiny");
    }
}
