//! Spec S3.4 to S3.6 for the Player, on the real clip (2 seconds, 60 frames).
//! These tests use real time, so the margins are generous (a busy machine must not make them flaky).
use rf_media::Player;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn clip() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/clip_bframes_audio.mp4"))
}
fn red(p: &Player) -> i32 {
    let Some((_, f)) = p.latest() else { return -1 }; // not decoded yet
    let o = ((f.image.height as usize / 2) * f.image.width as usize + f.image.width as usize / 2) * 4;
    f.image.pixels[o] as i32
}
fn wait_until(ms: u64, mut cond: impl FnMut() -> bool) -> bool {
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(ms) {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    cond()
}

#[test]
fn opens_paused_on_the_first_picture() {
    let p = Player::open(&clip(), false).unwrap();
    assert!(wait_until(3000, || p.latest().is_some()), "first frame shows up");
    let s = p.state();
    assert!(!s.playing && !s.ended && s.position_ms == 0 && s.error.is_none());
    assert!(red(&p) < 20, "first frame is the first colour, got {}", red(&p));
    assert!((1900..=2100).contains(&s.duration_ms));
}

#[test]
fn s3_4_play_runs_on_its_own_clock_and_pause_keeps_the_frame() {
    let p = Player::open(&clip(), false).unwrap();
    wait_until(3000, || p.latest().is_some());
    p.play();
    std::thread::sleep(Duration::from_millis(700));
    let s = p.state();
    assert!(s.playing);
    assert!((400..=1100).contains(&s.position_ms), "position {}", s.position_ms);
    let r = red(&p);
    assert!((24..=150).contains(&r), "after ~0.7 s the picture should be around frame 21 (red ~84), got {r}");
    p.pause();
    std::thread::sleep(Duration::from_millis(80));
    let (serial, _) = p.latest().unwrap();
    let pos = p.state().position_ms;
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(p.state().position_ms, pos, "paused: the position does not move");
    assert_eq!(p.latest().unwrap().0, serial, "paused: no new pictures");
    assert!(!p.state().playing);
}

#[test]
fn s3_5_restart_goes_back_to_the_start_and_plays() {
    let p = Player::open(&clip(), false).unwrap();
    p.play();
    std::thread::sleep(Duration::from_millis(900));
    assert!(red(&p) > 40);
    p.restart();
    assert!(wait_until(1500, || red(&p) < 40), "back near the first colour");
    assert!(p.state().playing);
}

#[test]
fn s3_6_without_loop_the_last_frame_stays_and_it_says_ended() {
    let p = Player::open(&clip(), false).unwrap();
    p.play();
    assert!(wait_until(4500, || p.state().ended), "reaches the end");
    assert!(wait_until(1500, || red(&p) >= 215), "last frame is on screen, red {}", red(&p));
    let (serial, _) = p.latest().unwrap();
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(p.latest().unwrap().0, serial, "the last frame just stays (no flash, no repaint)");
    // pressing play at the end starts again from the top
    p.play();
    assert!(wait_until(1500, || red(&p) < 100), "started again, red {}", red(&p));
    assert!(!p.state().ended);
}

#[test]
fn s3_6_with_loop_it_comes_back_round_and_never_ends() {
    let p = Player::open(&clip(), true).unwrap();
    p.play();
    assert!(wait_until(3000, || red(&p) >= 200), "reaches the end of the first pass");
    assert!(wait_until(1500, || red(&p) < 60), "...and starts again with no help, red {}", red(&p));
    let s = p.state();
    assert!(s.playing && !s.ended && s.looping);
    // and it keeps going rather than rushing through: after wrapping, the colour is still low a moment later
    std::thread::sleep(Duration::from_millis(150));
    assert!(red(&p) < 120, "must not fast-forward after the wrap, red {}", red(&p));
}

#[test]
fn s3_10_a_bad_file_is_a_plain_error() {
    let dir = std::env::temp_dir().join(format!("rf-media-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("bad.mp4");
    std::fs::write(&f, b"not a video").unwrap();
    assert!(Player::open(&f, false).err().unwrap().contains("MP4"));
    assert!(Player::open(&dir.join("missing.mp4"), false).is_err());
}

#[test]
fn dropping_a_playing_player_stops_its_thread_quickly() {
    let p = Player::open(&clip(), true).unwrap();
    p.play();
    std::thread::sleep(Duration::from_millis(100));
    let t = Instant::now();
    drop(p);
    assert!(t.elapsed() < Duration::from_secs(2), "drop took {:?}", t.elapsed());
}

#[test]
fn seek_jumps_forward_and_back_and_shows_that_picture_even_when_paused() {
    let p = Player::open(&clip(), false).unwrap();
    wait_until(3000, || p.latest().is_some());
    let r0 = red(&p);
    p.seek(1500);
    assert!(wait_until(3000, || red(&p) > r0 + 60), "paused, but the picture at 1.5 s appears (first {r0}, now {})", red(&p));
    let s = p.state();
    assert!(!s.playing && (1500..=1600).contains(&s.position_ms), "{s:?}");
    let late = red(&p);
    p.seek(400);
    assert!(wait_until(3000, || red(&p) < late - 40), "going back shows an earlier picture");
    assert!((400..=500).contains(&p.state().position_ms));
    // skip is relative; and it never goes below zero or past the end
    p.skip(-10_000);
    assert_eq!(p.state().position_ms, 0);
    p.skip(60_000);
    let s = p.state();
    assert!(s.position_ms <= s.duration_ms && s.ended, "{s:?}");
}

#[test]
fn seek_while_playing_keeps_playing_from_the_new_time() {
    let p = Player::open(&clip(), false).unwrap();
    p.play();
    std::thread::sleep(Duration::from_millis(200));
    p.seek(1000);
    std::thread::sleep(Duration::from_millis(300));
    let s = p.state();
    assert!(s.playing, "{s:?}");
    assert!((1200..=1700).contains(&s.position_ms), "{s:?}");
    assert!(red(&p) > 60);
}
