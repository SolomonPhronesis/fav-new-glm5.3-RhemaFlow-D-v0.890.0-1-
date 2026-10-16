//! Headless screenshot probe (scratch, not shipped): runs real frames of the real UI, rasterizes
//! egui's own output (triangles + font atlas) in software and writes PPM files to $RF_SHOTS
//! (default /tmp/rf-shots). Convert with PIL. It is a rough render (nearest-neighbour text, no GPU),
//! good enough to SEE layout, colour, spacing and overlaps. It is NOT a pixel-exact Mac screenshot.
use super::*;
use egui::epaint::{ImageData, Primitive};
use std::collections::HashMap;

struct Tex {
    w: usize,
    h: usize,
    px: Vec<egui::Color32>,
}

fn apply_deltas(texs: &mut HashMap<egui::TextureId, Tex>, out: &egui::FullOutput) {
    for (id, d) in &out.textures_delta.set {
        let (w, h, px): (usize, usize, Vec<egui::Color32>) = match &d.image {
            ImageData::Color(c) => (c.size[0], c.size[1], c.pixels.clone()),
            ImageData::Font(f) => (f.size[0], f.size[1], f.srgba_pixels(None).collect()),
        };
        match d.pos {
            None => {
                texs.insert(*id, Tex { w, h, px });
            }
            Some([x0, y0]) => {
                if let Some(t) = texs.get_mut(id) {
                    for yy in 0..h {
                        for xx in 0..w {
                            let (tx, ty) = (x0 + xx, y0 + yy);
                            if tx < t.w && ty < t.h {
                                t.px[ty * t.w + tx] = px[yy * w + xx];
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render(ctx: &egui::Context, out: &egui::FullOutput, texs: &HashMap<egui::TextureId, Tex>, w: usize, h: usize) -> Vec<u8> {
    let mut img = vec![[0.0_f32; 3]; w * h];
    let prims = ctx.tessellate(out.shapes.clone(), 1.0);
    for cp in &prims {
        let Primitive::Mesh(m) = &cp.primitive else { continue };
        let Some(tex) = texs.get(&m.texture_id) else { continue };
        let clip = cp.clip_rect;
        for tri in m.indices.chunks_exact(3) {
            let v = [m.vertices[tri[0] as usize], m.vertices[tri[1] as usize], m.vertices[tri[2] as usize]];
            let minx = v.iter().map(|p| p.pos.x).fold(f32::MAX, f32::min).max(clip.min.x).max(0.0).floor() as usize;
            let maxx = v.iter().map(|p| p.pos.x).fold(f32::MIN, f32::max).min(clip.max.x).min(w as f32 - 1.0).ceil() as usize;
            let miny = v.iter().map(|p| p.pos.y).fold(f32::MAX, f32::min).max(clip.min.y).max(0.0).floor() as usize;
            let maxy = v.iter().map(|p| p.pos.y).fold(f32::MIN, f32::max).min(clip.max.y).min(h as f32 - 1.0).ceil() as usize;
            let (a, b, c) = (v[0].pos, v[1].pos, v[2].pos);
            let den = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
            if den.abs() < 1e-6 {
                continue;
            }
            for y in miny..=maxy {
                for x in minx..=maxx {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let l0 = ((b.y - c.y) * (px - c.x) + (c.x - b.x) * (py - c.y)) / den;
                    let l1 = ((c.y - a.y) * (px - c.x) + (a.x - c.x) * (py - c.y)) / den;
                    let l2 = 1.0 - l0 - l1;
                    if l0 < -0.001 || l1 < -0.001 || l2 < -0.001 {
                        continue;
                    }
                    let u = l0 * v[0].uv.x + l1 * v[1].uv.x + l2 * v[2].uv.x;
                    let vv = l0 * v[0].uv.y + l1 * v[1].uv.y + l2 * v[2].uv.y;
                    let tx = ((u * tex.w as f32) as usize).min(tex.w - 1);
                    let ty = ((vv * tex.h as f32) as usize).min(tex.h - 1);
                    let t = tex.px[ty * tex.w + tx];
                    let col = |i: usize| (l0 * v[0].color.to_array()[i] as f32 + l1 * v[1].color.to_array()[i] as f32 + l2 * v[2].color.to_array()[i] as f32) / 255.0;
                    let tc = t.to_array();
                    let s = [col(0) * tc[0] as f32 / 255.0, col(1) * tc[1] as f32 / 255.0, col(2) * tc[2] as f32 / 255.0];
                    let sa = col(3) * tc[3] as f32 / 255.0;
                    let d = &mut img[y * w + x];
                    for i in 0..3 {
                        d[i] = s[i] + d[i] * (1.0 - sa); // premultiplied "over"
                    }
                }
            }
        }
    }
    let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in img {
        for i in 0..3 {
            ppm.push((p[i].clamp(0.0, 1.0) * 255.0) as u8);
        }
    }
    ppm
}

fn frame(app: &mut app::RhemaFlowApp, ctx: &egui::Context, w: f32, h: f32) -> egui::FullOutput {
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h)));
    ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame);
    })
}

/// Writes one screenshot per window size. Run: cargo test -p rf-app --ignore-rust-version shot -- --nocapture
#[test]
fn shot() {
    let dir = std::env::var("RF_SHOTS").unwrap_or("/tmp/rf-shots".into());
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("HOME", std::env::temp_dir().join("rf-shot-home"));
    for (w, h) in [(1440usize, 820usize), (1100, 700)] {
        let ctx = egui::Context::default();
        let cc = eframe::CreationContext { egui_ctx: ctx.clone(), _p: Default::default() };
        let mut app = app::RhemaFlowApp::new(&cc, state::AsrMode::Offline, "".into());
        let mut texs = HashMap::new();
        let mut out = frame(&mut app, &ctx, w as f32, h as f32);
        apply_deltas(&mut texs, &out);
        for _ in 0..4 {
            out = frame(&mut app, &ctx, w as f32, h as f32);
            apply_deltas(&mut texs, &out);
        }
        let ppm = render(&ctx, &out, &texs, w, h);
        std::fs::write(format!("{dir}/shot_{w}x{h}.ppm"), ppm).unwrap();
        eprintln!("wrote {dir}/shot_{w}x{h}.ppm");
    }
}
