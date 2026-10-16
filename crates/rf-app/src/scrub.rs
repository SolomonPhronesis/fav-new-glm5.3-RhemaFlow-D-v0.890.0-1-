//! Thumbnail under the video scrub bar (v0.835.0): while you hover or drag along the bar, a small picture of the
//! video at that moment follows the pointer, so you can find the right spot without guessing from the time.
//!
//! One helper thread keeps the video open and makes pictures on request (newest request first, older ones that
//! were overtaken are dropped), so dragging never freezes the window. Pictures are cached by moment.

use rf_media::Rgba;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

/// Size of a time bucket: pictures are made for whole buckets (about 100 per video, between 1 and 10 s).
pub fn bucket_ms(duration_ms: u64) -> u64 {
    (duration_ms / 120).clamp(1_000, 10_000)
}

/// The bucket a moment falls in (as its start time).
pub fn bucket_of(ms: u64, duration_ms: u64) -> u64 {
    let b = bucket_ms(duration_ms);
    (ms / b) * b
}

/// Time under the pointer: `frac` is 0.0 at the left end of the bar and 1.0 at the right.
pub fn time_at(frac: f32, duration_ms: u64) -> u64 {
    (frac.clamp(0.0, 1.0) as f64 * duration_ms as f64) as u64
}

/// Of the buckets already made, the one nearest to `want` (so something sensible shows while the exact one is being made).
pub fn nearest(done: &[u64], want: u64) -> Option<u64> {
    done.iter().copied().min_by_key(|b| b.abs_diff(want))
}

pub struct Scrub {
    path: Option<PathBuf>,
    duration_ms: u64,
    tx: Option<Sender<u64>>,
    rx: Option<Receiver<(u64, Rgba)>>,
    textures: HashMap<u64, egui::TextureHandle>,
    asked: HashSet<u64>,
    path_id: u64,
}

impl Default for Scrub {
    fn default() -> Self {
        Scrub { path: None, duration_ms: 0, tx: None, rx: None, textures: HashMap::new(), asked: HashSet::new(), path_id: 0 }
    }
}

const MAX_CACHED: usize = 240;

impl Scrub {
    /// Call every frame the bar is on screen. Starts a helper for a new video, drops the old one.
    pub fn ensure(&mut self, path: &str, duration_ms: u64) {
        if self.path.as_deref().map(|p| p.to_string_lossy() == path).unwrap_or(false) {
            return;
        }
        self.stop();
        self.path_id += 1;
        let p = PathBuf::from(path);
        let (req_tx, req_rx) = channel::<u64>();
        let (img_tx, img_rx) = channel::<(u64, Rgba)>();
        let pp = p.clone();
        std::thread::spawn(move || {
            let Ok(mut reader) = rf_media::VideoReader::open(&pp) else { return };
            while let Ok(mut ms) = req_rx.recv() {
                while let Ok(newer) = req_rx.try_recv() {
                    ms = newer; // the pointer has moved on: only the newest request matters
                }
                if let Some(img) = rf_media::thumb::frame_at(&mut reader, ms) {
                    if img_tx.send((ms, rf_media::thumb::shrink(&img, 240))).is_err() {
                        return;
                    }
                }
            }
        });
        self.path = Some(p);
        self.duration_ms = duration_ms;
        self.tx = Some(req_tx);
        self.rx = Some(img_rx);
    }

    pub fn stop(&mut self) {
        self.path = None;
        self.tx = None;
        self.rx = None;
        self.textures.clear();
        self.asked.clear();
    }

    /// Ask for the picture at `ms` (nothing happens if it is already made or asked for).
    pub fn want(&mut self, ms: u64) {
        let b = bucket_of(ms, self.duration_ms);
        if self.textures.contains_key(&b) || !self.asked.insert(b) {
            return;
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(b);
        }
    }

    /// Take finished pictures into textures.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        while let Ok((b, img)) = rx.try_recv() {
            if self.textures.len() >= MAX_CACHED {
                self.textures.clear();
                self.asked.clear();
            }
            let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.pixels);
            self.textures.insert(b, ctx.load_texture(format!("scrub-{}-{b}", self.path_id), ci, egui::TextureOptions::LINEAR));
            ctx.request_repaint();
        }
    }

    /// The picture to show for `ms`: its own, or the nearest one made so far.
    pub fn picture(&self, ms: u64) -> Option<egui::TextureHandle> {
        let b = bucket_of(ms, self.duration_ms);
        if let Some(t) = self.textures.get(&b) {
            return Some(t.clone());
        }
        let done: Vec<u64> = self.textures.keys().copied().collect();
        nearest(&done, b).and_then(|k| self.textures.get(&k).cloned())
    }

    pub fn is_ready(&self, ms: u64) -> bool {
        self.textures.contains_key(&bucket_of(ms, self.duration_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_are_about_a_hundred_per_video_and_between_one_and_ten_seconds() {
        assert_eq!(bucket_ms(20_000), 1_000, "short video: every second");
        assert_eq!(bucket_ms(22 * 60_000), 10_000, "a 22 minute video: every 10 s");
        assert_eq!(bucket_ms(10 * 60_000), 5_000);
        assert_eq!(bucket_of(13_999, 20_000), 13_000);
        assert_eq!(bucket_of(83_000, 22 * 60_000), 80_000);
    }

    #[test]
    fn the_pointer_position_maps_to_a_time_inside_the_video() {
        assert_eq!(time_at(0.0, 100_000), 0);
        assert_eq!(time_at(0.5, 100_000), 50_000);
        assert_eq!(time_at(1.7, 100_000), 100_000);
        assert_eq!(time_at(-1.0, 100_000), 0);
    }

    #[test]
    fn while_the_exact_picture_is_being_made_the_nearest_one_shows() {
        assert_eq!(nearest(&[0, 10_000, 40_000], 33_000), Some(40_000));
        assert_eq!(nearest(&[], 5), None);
    }

    #[test]
    fn a_real_video_yields_pictures_through_the_helper_thread() {
        let clip = concat!(env!("CARGO_MANIFEST_DIR"), "/../rf-media/tests/fixtures/clip_seek_20s.mp4");
        let ctx = egui::Context::default();
        let mut s = Scrub::default();
        s.ensure(clip, 20_000);
        s.want(13_400);
        s.want(13_900); // same bucket: asked only once
        let t = std::time::Instant::now();
        while !s.is_ready(13_400) && t.elapsed().as_secs() < 10 {
            s.poll(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(s.is_ready(13_400), "a picture arrived");
        let tex = s.picture(13_999).unwrap();
        assert!(tex.size()[0] <= 240 && tex.size()[0] > 0);
        s.stop();
        assert!(s.picture(0).is_none());
    }
}
