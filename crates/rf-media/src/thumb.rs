//! Video preview pictures (v0.835.0): the shelf thumbnail and the picture under the scrub bar.
//! Works on a video of any size (the old code gave up above 150 MB, so big videos had no thumbnail),
//! and falls back to ffmpeg when the built-in decoder cannot read the file (HEVC phone videos).

use crate::{decode_image, Rgba, VideoReader};
use std::path::Path;

/// Nearest-neighbour shrink so the preview is small.
pub fn shrink(img: &Rgba, max_side: u32) -> Rgba {
    let (w, h) = (img.width.max(1), img.height.max(1));
    let k = (w.max(h) as f32 / max_side.max(1) as f32).max(1.0);
    let (nw, nh) = (((w as f32 / k) as u32).max(1), ((h as f32 / k) as u32).max(1));
    let mut pixels = Vec::with_capacity((nw * nh * 4) as usize);
    for y in 0..nh {
        for x in 0..nw {
            let (sx, sy) = (((x as f32 * k) as u32).min(w - 1), ((y as f32 * k) as u32).min(h - 1));
            let i = ((sy * w + sx) * 4) as usize;
            pixels.extend_from_slice(&img.pixels[i..i + 4]);
        }
    }
    Rgba { width: nw, height: nh, pixels }
}

/// Average brightness 0..255 of a picture (sampled).
pub fn brightness(img: &Rgba) -> u32 {
    let n = (img.pixels.len() / 4).max(1);
    let step = (n / 400).max(1);
    let (mut sum, mut cnt) = (0u64, 0u64);
    for i in (0..n).step_by(step) {
        sum += img.pixels[i * 4] as u64 + img.pixels[i * 4 + 1] as u64 + img.pixels[i * 4 + 2] as u64;
        cnt += 3;
    }
    (sum / cnt.max(1)) as u32
}

/// The picture shown at `ms` (the one the player would show after a jump there).
pub fn frame_at(reader: &mut VideoReader, ms: u64) -> Option<Rgba> {
    reader.seek_to(ms).ok()?;
    while let Some(p) = reader.peek_pts() {
        if p >= ms {
            break;
        }
        reader.next_frame(false).ok()?;
    }
    loop {
        let f = reader.next_frame(true).ok()??;
        if !f.image.pixels.is_empty() {
            return Some(f.image);
        }
    }
}

/// One picture from the video at `ms` using ffmpeg (any format ffmpeg reads).
pub fn ffmpeg_frame(path: &Path, ms: u64, max_side: u32) -> Option<Rgba> {
    let ff = crate::convert::find_ffmpeg()?;
    let out = std::env::temp_dir().join(format!("rf-thumb-{}-{}.png", std::process::id(), ms));
    let ok = std::process::Command::new(ff)
        .args(["-y", "-v", "error", "-ss", &format!("{}.{:03}", ms / 1000, ms % 1000), "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-vf", &format!("scale='min({max_side},iw)':-2")])
        .arg(&out)
        .stdin(std::process::Stdio::null())
        .status()
        .ok()?
        .success();
    let img = if ok { std::fs::read(&out).ok().and_then(|b| decode_image(&b, max_side).ok()) } else { None };
    let _ = std::fs::remove_file(&out);
    img
}

/// A representative picture for the shelf plus the video's length. Tries a moment ~8% in (past fades and
/// black openings), then the first pictures, then ffmpeg. Never gives up because of file size.
pub fn video_thumbnail(path: &Path, max_side: u32) -> Option<(Rgba, u64)> {
    let mut duration = 0;
    if let Ok(mut r) = VideoReader::open(path) {
        duration = r.info().duration_ms;
        let mut best: Option<Rgba> = None;
        for at in [duration / 12, 1500.min(duration / 2), 0] {
            if let Some(img) = frame_at(&mut r, at) {
                let bright = brightness(&img) > 14;
                best = Some(img);
                if bright {
                    break;
                }
            }
        }
        if let Some(img) = best {
            return Some((shrink(&img, max_side), duration));
        }
    }
    ffmpeg_frame(path, 1000, max_side).map(|i| (i, duration))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrink_keeps_shape_and_brightness_reads() {
        let img = Rgba { width: 400, height: 200, pixels: vec![200; 400 * 200 * 4] };
        let s = shrink(&img, 100);
        assert_eq!((s.width, s.height), (100, 50));
        assert!(brightness(&s) > 100);
    }
}
