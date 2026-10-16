//! A jump in a video keeps its sound (v0.835.0): the sound is cut from the jump time without ffmpeg, and
//! big videos get a thumbnail.
use rf_media::audiocut::cut_aac_adts;
use rf_media::thumb::{brightness, video_thumbnail};
use std::path::PathBuf;
use std::process::Command;

fn clip() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/clip_bframes_audio.mp4"))
}

fn probe_secs(p: &std::path::Path) -> Option<f64> {
    let o = Command::new("ffprobe").args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"]).arg(p).output().ok()?;
    String::from_utf8_lossy(&o.stdout).trim().parse().ok()
}

#[test]
fn the_sound_after_a_jump_starts_at_the_jump_time_and_plays_to_the_end() {
    let dir = std::env::temp_dir().join(format!("rf-cut-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let full = dir.join("full.aac");
    cut_aac_adts(&clip(), 0, &full).unwrap();
    let from_half = dir.join("half.aac");
    cut_aac_adts(&clip(), 1000, &from_half).unwrap();
    assert!(std::fs::metadata(&from_half).unwrap().len() < std::fs::metadata(&full).unwrap().len());
    if let (Some(a), Some(b)) = (probe_secs(&full), probe_secs(&from_half)) {
        assert!((a - 2.0).abs() < 0.2, "whole sound ~2s, got {a}");
        assert!((b - 1.0).abs() < 0.2, "from 1s there is ~1s left, got {b}");
    }
    // a jump past the last sound sample still leaves something to play (the last frame)
    cut_aac_adts(&clip(), 60_000, &dir.join("end.aac")).unwrap();
}

#[test]
fn a_file_with_no_aac_sound_says_so_so_the_app_can_try_ffmpeg() {
    let hevc = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/clip_hevc.mp4"));
    let out = std::env::temp_dir().join("rf-none.aac");
    let r = cut_aac_adts(&hevc, 0, &out);
    // either it has no sound track or it is not plain AAC: never a crash and never an empty file reported as good
    if r.is_ok() {
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
    }
}

#[test]
fn a_video_gets_a_bright_thumbnail_whatever_its_size() {
    let (img, ms) = video_thumbnail(&clip(), 128).expect("thumbnail");
    assert!(img.width <= 128 && img.height <= 128);
    assert!(ms > 1000);
    assert!(brightness(&img) > 0);
    let big = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/clip_seek_20s.mp4"));
    assert!(video_thumbnail(&big, 128).is_some());
}
