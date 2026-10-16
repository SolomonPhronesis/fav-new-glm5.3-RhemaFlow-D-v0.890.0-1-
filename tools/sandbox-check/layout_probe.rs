//! Headless layout probe (scratch, not shipped): runs real frames at the user's window
//! size and reads back where every piece of text was painted.
use super::*;
use egui::epaint::Shape;

fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Pos2)> {
    let mut v = Vec::new();
    fn collect(s: &Shape, v: &mut Vec<(String, egui::Pos2)>) {
        match s {
            Shape::Text(t) => v.push((t.galley.text().to_string(), t.pos)),
            Shape::Vec(list) => list.iter().for_each(|x| collect(x, v)),
            _ => {}
        }
    }
    for cs in &out.shapes {
        collect(&cs.shape, &mut v);
    }
    v
}

fn frame(app: &mut app::RhemaFlowApp, ctx: &egui::Context, w: f32, h: f32) -> egui::FullOutput {
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h)));
    ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame);
    })
}

#[test]
fn probe() {
    std::env::set_var("HOME", std::env::temp_dir().join("rf-probe-home"));
    for (w, h) in [(1190.0_f32, 770.0_f32), (1000.0, 640.0), (1440.0, 900.0)] {
        let ctx = egui::Context::default();
        let cc = eframe::CreationContext { egui_ctx: ctx.clone(), _p: Default::default() };
        let mut app = app::RhemaFlowApp::new(&cc, state::AsrMode::Offline, "".into());
        let mut out = frame(&mut app, &ctx, w, h);
        for _ in 0..4 {
            out = frame(&mut app, &ctx, w, h);
        }
        let t = texts(&out);
        eprintln!("=== window {w}x{h}");
        let at = |s: &str| t.iter().find(|x| x.0 == s || x.0.contains(s)).map(|x| (x.1.x.round() as i32, x.1.y.round() as i32));
        let g = t.iter().find(|x| x.0 == "Genesis").unwrap().1;
        let e = t.iter().find(|x| x.0 == "Exodus").unwrap().1;
        assert!((g.x - e.x).abs() < 3.0 && e.y > g.y + 10.0, "books are a vertical list: {g:?} {e:?}");
        eprintln!("  books: Genesis {:?} Exodus {:?}  (vertical list)", (g.x as i32, g.y as i32), (e.x as i32, e.y as i32));
        eprintln!("  verse 1 text at {:?}", at("In the beginning God created"));
        eprintln!("  chapters '1' {:?} '2' {:?}", at("1"), at("2"));
        let out_of: Vec<_> = t.iter().filter(|x| x.1.x > w - 4.0 || x.1.y > h - 4.0).map(|x| (x.0.clone(), x.1.x as i32, x.1.y as i32)).collect();
        eprintln!("  painted outside the window: {out_of:?}");
        for label in ["Start service", "Open stage", "Full screen", "Blackout", "Read", "A+", "More", "SCRIPTURE", "NEEDS YOUR REVIEW", "TRANSCRIPT"] {
            eprintln!("  {label:?} at {:?}", at(label));
        }
        // how many text rows do the controls use? (distinct y of the control buttons)
        let mut ys: Vec<i32> = ["Start service", "Open stage", "Read", "A+"].iter().filter_map(|l| at(l).map(|p| p.1)).collect();
        ys.sort();
        ys.dedup();
        eprintln!("  control rows (distinct y): {ys:?}");
    }
}
