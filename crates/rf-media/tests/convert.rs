//! Real conversions with the real ffmpeg (skipped, with a note, when ffmpeg is not installed).
use rf_media::convert::{convert, find_ffmpeg, Target};
use rf_media::VideoReader;
use std::path::{Path, PathBuf};
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rf-conv-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn make(args: &[&str], out: &Path) -> bool {
    let ff = find_ffmpeg().unwrap();
    Command::new(ff).args(["-y", "-hide_banner", "-loglevel", "error"]).args(args).arg(out).status().map(|s| s.success()).unwrap_or(false)
}

fn has_stream(path: &Path, word: &str) -> bool {
    let out = Command::new(find_ffmpeg().unwrap()).args(["-hide_banner", "-i"]).arg(path).output().unwrap();
    String::from_utf8_lossy(&out.stderr).contains(word)
}

#[test]
fn an_iphone_style_hevc_video_becomes_a_playable_h264_mp4_with_its_sound() {
    if find_ffmpeg().is_none() {
        eprintln!("SKIPPED: ffmpeg is not installed here");
        return;
    }
    let d = dir("hevc");
    let src = d.join("Baptism 2026.mp4");
    assert!(make(&["-f", "lavfi", "-i", "testsrc=size=640x360:rate=25:duration=2", "-f", "lavfi", "-i", "sine=frequency=440:duration=2", "-c:v", "libx265", "-pix_fmt", "yuv420p", "-tag:v", "hvc1", "-c:a", "aac", "-shortest"], &src));
    // the app cannot play it as it is (this is the "error playing" the operator saw)
    let e = VideoReader::open(&src).err().expect("HEVC must be refused by the player");
    assert!(e.contains("H.265") || e.contains("does not recognise"), "{e}");
    let mut last = 0.0f32;
    let out = convert(&src, &d.join("converted"), Target::Video, &mut |p| last = p).expect("conversion works");
    assert!(last >= 1.0, "progress ends at 100%");
    assert_eq!(out.extension().unwrap(), "mp4");
    let v = VideoReader::open(&out).expect("the converted file plays");
    assert!(v.info().width == 640 && v.info().frame_count >= 45, "{:?}", v.info());
    assert!(has_stream(&out, "Audio: aac"), "the sound came along");
    // converting the same file again reuses the result
    let again = convert(&src, &d.join("converted"), Target::Video, &mut |_| {}).unwrap();
    assert_eq!(again, out);
}

#[test]
fn a_mov_with_no_sound_and_a_4k_picture_converts_and_is_scaled_down() {
    if find_ffmpeg().is_none() {
        eprintln!("SKIPPED: ffmpeg is not installed here");
        return;
    }
    let d = dir("mov");
    let src = d.join("film.mov");
    assert!(make(&["-f", "lavfi", "-i", "testsrc=size=3840x2160:rate=10:duration=1", "-c:v", "libx264", "-pix_fmt", "yuv444p"], &src));
    let out = convert(&src, &d.join("converted"), Target::Video, &mut |_| {}).expect("converts without sound");
    let v = VideoReader::open(&out).expect("plays");
    assert_eq!(v.info().width, 1920, "4K is brought down to 1920 so it plays smoothly");
    assert!(!has_stream(&out, "Audio:"));
}

#[test]
fn a_sound_file_ffmpeg_can_read_becomes_an_m4a() {
    if find_ffmpeg().is_none() {
        eprintln!("SKIPPED: ffmpeg is not installed here");
        return;
    }
    let d = dir("aud");
    let src = d.join("Walk In.opus");
    if !make(&["-f", "lavfi", "-i", "sine=frequency=330:duration=2", "-c:a", "libopus"], &src) {
        eprintln!("SKIPPED: this ffmpeg cannot make opus test files");
        return;
    }
    let out = convert(&src, &d.join("converted"), Target::Audio, &mut |_| {}).expect("converts");
    assert_eq!(out.extension().unwrap(), "m4a");
    assert!(has_stream(&out, "Audio: aac"));
    assert!(!has_stream(&out, "Video:"));
}

#[test]
fn a_damaged_file_gives_a_plain_reason_and_leaves_nothing_behind() {
    if find_ffmpeg().is_none() {
        eprintln!("SKIPPED: ffmpeg is not installed here");
        return;
    }
    let d = dir("bad");
    let src = d.join("broken.mov");
    std::fs::write(&src, b"this is not a video at all").unwrap();
    let e = convert(&src, &d.join("converted"), Target::Video, &mut |_| {}).err().expect("must fail");
    assert!(e.contains("did not work"), "{e}");
    let left: Vec<_> = std::fs::read_dir(d.join("converted")).unwrap().filter_map(|e| e.ok()).collect();
    assert!(left.is_empty(), "no half-made file is left: {left:?}");
}
