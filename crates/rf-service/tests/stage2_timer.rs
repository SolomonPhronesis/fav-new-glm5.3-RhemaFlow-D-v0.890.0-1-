//! Stage 2 spec section 5 (timers and clocks). Written BEFORE the code.
use rf_service::timer::{format_time_of_day, parse_time_of_day, Phase, Timer, TimerKind};

const MIN: u64 = 60_000;

#[test]
fn s5_1_a_countdown_rounds_up_to_the_next_whole_second() {
    let mut t = Timer::new(TimerKind::CountDown { total_ms: 5 * MIN }, "Service starts in");
    assert_eq!(t.display(0, 0), "05:00", "before start it shows the full time");
    t.start(1_000, 0);
    assert_eq!(t.display(1_000, 0), "05:00");
    assert_eq!(t.display(1_001, 0), "05:00", "299.999 s left rounds up to 5:00 (never shows 04:59 early)");
    assert_eq!(t.display(2_000, 0), "04:59");
    assert_eq!(t.display(1_000 + 5 * MIN - 1, 0), "00:01");
    assert_eq!(t.display(1_000 + 5 * MIN, 0), "00:00");
    assert_eq!(t.display(1_000 + 9 * MIN, 0), "00:00", "never negative");
}

#[test]
fn s5_1_from_one_hour_it_shows_hours() {
    let mut t = Timer::new(TimerKind::CountDown { total_ms: 75 * MIN }, "x");
    t.start(0, 0);
    assert_eq!(t.display(0, 0), "1:15:00");
    assert_eq!(t.display(15 * MIN, 0), "1:00:00");
    assert_eq!(t.display(15 * MIN + 1_000, 0), "59:59");
}

#[test]
fn s5_5_pause_resume_and_reset() {
    let mut t = Timer::new(TimerKind::CountDown { total_ms: 2 * MIN }, "x");
    t.start(0, 0);
    t.pause(30_000);
    assert_eq!(t.phase(999_999, 0), Phase::Paused);
    assert_eq!(t.display(999_999, 0), "01:30", "frozen while paused");
    t.resume(1_000_000);
    assert_eq!(t.display(1_010_000, 0), "01:20");
    t.reset();
    assert_eq!(t.display(2_000_000, 0), "02:00");
    assert_eq!(t.phase(2_000_000, 0), Phase::Idle);
}

#[test]
fn s5_5_and_s5_6_warning_in_the_last_minute_then_done_and_it_stays_at_zero() {
    let mut t = Timer::new(TimerKind::CountDown { total_ms: 3 * MIN }, "x");
    assert_eq!(t.phase(0, 0), Phase::Idle);
    t.start(0, 0);
    assert_eq!(t.phase(1, 0), Phase::Running);
    assert_eq!(t.phase(2 * MIN - 1, 0), Phase::Running);
    assert_eq!(t.phase(2 * MIN + 1, 0), Phase::Warning);
    assert_eq!(t.phase(3 * MIN, 0), Phase::Done);
    assert_eq!(t.phase(10 * MIN, 0), Phase::Done);
    assert_eq!(t.display(10 * MIN, 0), "00:00");
}

#[test]
fn s5_3_a_stopwatch_counts_up_and_rounds_down() {
    let mut t = Timer::new(TimerKind::StopWatch, "Sermon");
    assert_eq!(t.display(5_000, 0), "00:00");
    t.start(10_000, 0);
    assert_eq!(t.display(10_999, 0), "00:00");
    assert_eq!(t.display(11_000, 0), "00:01");
    assert_eq!(t.display(10_000 + 61 * 1_000, 0), "01:01");
    assert_eq!(t.display(10_000 + 3_600_000 + 5_000, 0), "1:00:05");
    t.pause(70_000);
    assert_eq!(t.display(999_999, 0), "01:00");
    assert_eq!(t.phase(999_999, 0), Phase::Paused);
}

#[test]
fn s5_2_countdown_to_a_clock_time_follows_the_wall_clock_and_never_wraps_to_tomorrow() {
    let ten_thirty = parse_time_of_day("10:30").unwrap();
    let mut t = Timer::new(TimerKind::UntilClock { target_secs: ten_thirty }, "Service starts");
    let wall = |h: u64, m: u64, s: u64| (h * 3600 + m * 60 + s) * 1000;
    t.start(0, wall(10, 0, 0));
    assert_eq!(t.display(123, wall(10, 0, 0)), "30:00");
    assert_eq!(t.display(999_999_999, wall(10, 29, 0)), "01:00", "uses the wall clock, not the app's own timer");
    assert_eq!(t.phase(0, wall(10, 29, 30)), Phase::Warning);
    assert_eq!(t.phase(0, wall(10, 30, 0)), Phase::Done);
    assert_eq!(t.display(0, wall(10, 45, 0)), "00:00", "already passed: finished, not 23 hours");
    assert_eq!(t.display(0, wall(8, 30, 0)), "2:00:00");
}

#[test]
fn s5_4_the_clock_shows_the_time_of_day() {
    let t24 = Timer::new(TimerKind::Clock { twelve_hour: false }, "");
    let t12 = Timer::new(TimerKind::Clock { twelve_hour: true }, "");
    let wall = |h: u64, m: u64| (h * 3600 + m * 60) * 1000 + 25_000;
    assert_eq!(t24.display(0, wall(9, 5)), "09:05");
    assert_eq!(t24.display(0, wall(0, 0)), "00:00");
    assert_eq!(t12.display(0, wall(9, 5)), "9:05 AM");
    assert_eq!(t12.display(0, wall(0, 7)), "12:07 AM");
    assert_eq!(t12.display(0, wall(12, 0)), "12:00 PM");
    assert_eq!(t12.display(0, wall(23, 59)), "11:59 PM");
    assert_eq!(format_time_of_day(37_800, false), "10:30");
}

#[test]
fn s5_8_times_are_typed_in_many_ways_and_bad_input_is_none() {
    for (text, secs) in [
        ("10:30", 10 * 3600 + 30 * 60), ("9:05", 9 * 3600 + 5 * 60), ("0930", 9 * 3600 + 30 * 60), ("9.05", 9 * 3600 + 5 * 60),
        ("9:05pm", 21 * 3600 + 5 * 60), ("9:05 PM", 21 * 3600 + 5 * 60), ("12am", 0), ("12pm", 12 * 3600), ("7am", 7 * 3600),
        ("  23:59 ", 23 * 3600 + 59 * 60), ("00:00", 0),
    ] {
        assert_eq!(parse_time_of_day(text), Some(secs as u32), "{text:?}");
    }
    for bad in ["", "abc", "25:00", "10:75", "24:00", "13pm", "0am", "10:", ":30", "1:2:3", "10:30:99"] {
        assert_eq!(parse_time_of_day(bad), None, "{bad:?} must be refused");
    }
}
