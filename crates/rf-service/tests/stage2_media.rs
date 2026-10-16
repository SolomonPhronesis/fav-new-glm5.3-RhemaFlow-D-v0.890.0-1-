//! Stage 2 spec sections 3 and 4 (the maths and the rules; decoding is tested in rf-media).
use rf_service::media::{classify, dim_alpha, fit_rect, Classified, FitMode, MediaKind, PlaybackClock, DEFAULT_DIM_PCT, MAX_DIM_PCT};

#[test]
fn s3_1_what_is_a_picture_what_is_a_video_and_what_is_refused_with_advice() {
    for p in ["a.png", "/x/Photo.JPG", "b.jpeg", "c.gif", "d.BMP"] {
        assert_eq!(classify(p), Classified::Ok(MediaKind::Image), "{p}");
    }
    for p in ["a.mp4", "/x/Film.MP4", "b.m4v"] {
        assert_eq!(classify(p), Classified::Ok(MediaKind::Video), "{p}");
    }
    for p in ["a.mp3", "/x/Walk In.MP3", "b.wav", "c.m4a", "d.aac", "e.aiff", "f.flac"] {
        assert_eq!(classify(p), Classified::Ok(MediaKind::Audio), "{p}");
    }
    for p in ["a.ogg", "a.opus", "a.wma"] {
        match classify(p) {
            Classified::Unsupported(msg) => assert!(msg.to_lowercase().contains("convert"), "{p}: {msg}"),
            other => panic!("{p} should be Unsupported, got {other:?}"),
        }
    }
    for p in ["a.mov", "a.mkv", "a.avi", "a.webm", "a.webp", "a.heic", "a.hevc"] {
        match classify(p) {
            Classified::Unsupported(msg) => {
                assert!(msg.to_lowercase().contains("convert") || msg.to_lowercase().contains("save"), "{p}: advice missing: {msg}");
            }
            other => panic!("{p} should be Unsupported, got {other:?}"),
        }
    }
    assert_eq!(classify("notes.txt"), Classified::Unknown);
    assert_eq!(classify("noextension"), Classified::Unknown);
    assert_eq!(classify(""), Classified::Unknown);
}

#[test]
fn s3_3_fit_keeps_the_whole_picture_centred() {
    // a wide 2:1 picture on a 16:9 screen: full width, bands top and bottom
    let r = fit_rect(2000.0, 1000.0, 1920.0, 1080.0, FitMode::Fit);
    assert!((r.w - 1920.0).abs() < 0.01 && (r.h - 960.0).abs() < 0.01);
    assert!((r.x).abs() < 0.01 && (r.y - 60.0).abs() < 0.01);
    // a tall picture: full height, bands left and right
    let r = fit_rect(1000.0, 2000.0, 1920.0, 1080.0, FitMode::Fit);
    assert!((r.h - 1080.0).abs() < 0.01 && (r.w - 540.0).abs() < 0.01);
    assert!((r.x - 690.0).abs() < 0.01 && r.y.abs() < 0.01);
    // same shape as the screen: fills it exactly
    let r = fit_rect(1280.0, 720.0, 1920.0, 1080.0, FitMode::Fit);
    assert!((r.w - 1920.0).abs() < 0.01 && (r.h - 1080.0).abs() < 0.01);
}

#[test]
fn s3_3_fill_covers_the_screen_and_may_overflow_equally_on_both_sides() {
    let r = fit_rect(2000.0, 1000.0, 1920.0, 1080.0, FitMode::Fill);
    assert!((r.h - 1080.0).abs() < 0.01 && (r.w - 2160.0).abs() < 0.01);
    assert!((r.x + 120.0).abs() < 0.01, "centred: 120 off each side, got x={}", r.x);
    assert!(r.y.abs() < 0.01);
    assert!(r.w >= 1920.0 && r.h >= 1080.0);
}

#[test]
fn s3_3_stretch_ignores_the_shape_and_bad_sizes_do_not_panic() {
    let r = fit_rect(10.0, 10.0, 1920.0, 1080.0, FitMode::Stretch);
    assert_eq!((r.x, r.y, r.w, r.h), (0.0, 0.0, 1920.0, 1080.0));
    for mode in [FitMode::Fit, FitMode::Fill, FitMode::Stretch] {
        let r = fit_rect(0.0, 0.0, 100.0, 100.0, mode);
        assert!(r.w.is_finite() && r.h.is_finite() && r.x.is_finite() && r.y.is_finite());
        let r = fit_rect(100.0, 100.0, 0.0, 0.0, mode);
        assert!(r.w.is_finite() && r.h.is_finite());
    }
}

#[test]
fn s3_4_the_playback_clock_runs_on_wall_time_and_pause_holds_the_position() {
    let mut c = PlaybackClock::new(10_000, false);
    assert_eq!(c.position_ms(5_000), 0, "not started: at the start");
    c.play(1_000);
    assert_eq!(c.position_ms(1_500), 500);
    assert_eq!(c.position_ms(11_000), 10_000 - 0, "ten seconds later it is ten seconds in (clamped at the end)");
    c.restart(20_000);
    assert!(c.is_playing());
    c.pause(20_700);
    assert_eq!(c.position_ms(99_000), 700);
    assert!(!c.is_playing());
    c.play(100_000);
    assert_eq!(c.position_ms(100_300), 1_000);
}

#[test]
fn s3_6_without_loop_the_last_frame_stays_and_it_says_ended() {
    let mut c = PlaybackClock::new(2_000, false);
    c.play(0);
    assert!(!c.ended(1_999));
    assert!(c.ended(2_000));
    assert_eq!(c.position_ms(50_000), 2_000);
    // restart works after the end
    c.restart(60_000);
    assert!(!c.ended(60_001));
    assert_eq!(c.position_ms(60_500), 500);
}

#[test]
fn s3_6_with_loop_it_wraps_and_never_ends() {
    let mut c = PlaybackClock::new(2_000, true);
    c.play(0);
    assert_eq!(c.position_ms(1_999), 1_999);
    assert_eq!(c.position_ms(2_000), 0);
    assert_eq!(c.position_ms(5_500), 1_500);
    assert!(!c.ended(1_000_000));
    // a zero-length clip does not divide by zero
    let mut z = PlaybackClock::new(0, true);
    z.play(0);
    assert_eq!(z.position_ms(1_000), 0);
}

#[test]
fn s3_6_switching_loop_on_while_playing_keeps_the_place() {
    let mut c = PlaybackClock::new(2_000, false);
    c.play(0);
    c.set_looping(true, 1_500);
    assert_eq!(c.position_ms(1_800), 1_800);
    assert_eq!(c.position_ms(2_200), 200);
}

#[test]
fn s4_3_darken_is_limited_so_the_background_never_disappears() {
    assert_eq!(DEFAULT_DIM_PCT, 45);
    assert_eq!(MAX_DIM_PCT, 85);
    assert_eq!(dim_alpha(0), 0);
    assert_eq!(dim_alpha(100), dim_alpha(85), "anything above 85 is treated as 85");
    assert_eq!(dim_alpha(50), 128);
    assert!(dim_alpha(85) < 255 && dim_alpha(85) > 200);
    assert!(dim_alpha(10) < dim_alpha(20));
}

#[test]
fn v0840_pdf_and_html_are_documents() {
    for p in ["a.pdf", "/x/Notes.PDF", "b.html", "c.HTM", "d.xhtml"] {
        assert_eq!(classify(p), Classified::Ok(MediaKind::Document), "{p}");
    }
}
