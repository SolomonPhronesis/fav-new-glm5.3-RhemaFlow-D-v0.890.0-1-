//! How fast can THIS computer decode a video? Run it on the Mac that will run the service:
//!   cargo run --release -p rf-media --example decode_speed -- /path/to/video.mp4
//! A video needs to decode comfortably faster than its own frame rate (30 fps video: want 60+).
use std::time::Instant;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: decode_speed <video.mp4>");
        std::process::exit(2);
    };
    let mut r = match rf_media::VideoReader::open(std::path::Path::new(&path)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let i = r.info().clone();
    println!("{}x{}, {} frames, {:.1} fps, {:.1} s", i.width, i.height, i.frame_count, i.fps, i.duration_ms as f32 / 1000.0);
    for (name, convert) in [("decode only", false), ("decode + make RGBA pictures", true)] {
        r.rewind().unwrap();
        let t = Instant::now();
        let mut n = 0u32;
        while let Some(_f) = r.next_frame(convert).unwrap() {
            n += 1;
        }
        let secs = t.elapsed().as_secs_f32();
        println!("{name:30} {n} frames in {secs:.2} s = {:.0} frames per second ({:.1}x real time)", n as f32 / secs, (n as f32 / secs) / i.fps);
    }
}
