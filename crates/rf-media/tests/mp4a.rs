//! A file whose SOUND header is damaged (the "mp4a box contains a box with a larger size" error) must
//! still open for its picture. Built from a real ffmpeg file with its mp4a box size made too big.
use rf_media::convert::find_ffmpeg;
use rf_media::VideoReader;
use std::process::Command;

#[test]
fn a_damaged_sound_header_does_not_stop_the_picture() {
    let Some(ff) = find_ffmpeg() else { eprintln!("SKIPPED: no ffmpeg"); return };
    let d = std::env::temp_dir().join(format!("rf-mp4a-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let good = d.join("good.mp4");
    assert!(Command::new(ff).args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=320x180:rate=25:duration=1", "-f", "lavfi", "-i", "sine=duration=1", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"]).arg(&good).status().unwrap().success());
    let mut bytes = std::fs::read(&good).unwrap();
    let at = bytes.windows(4).position(|w| w == b"mp4a").expect("has an mp4a box") - 4;
    let size = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    bytes[at..at + 4].copy_from_slice(&(size + 5000).to_be_bytes()); // now larger than its parent
    let bad = d.join("bad.mp4");
    std::fs::write(&bad, &bytes).unwrap();
    let v = VideoReader::open(&bad).expect("the picture still opens");
    assert_eq!(v.info().frame_count, 25);
}
