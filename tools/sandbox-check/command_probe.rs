//! Headless end-to-end probe (scratch, not shipped in the app): builds the real app and speaks to it.
use super::*;
use crate::state::{StageKind, StageStyle};

fn new_app() -> (app::RhemaFlowApp, egui::Context) {
    let home = std::env::temp_dir().join(format!("rf-cmd-probe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let ctx = egui::Context::default();
    let cc = eframe::CreationContext { egui_ctx: ctx.clone(), _p: Default::default() };
    let app = app::RhemaFlowApp::new(&cc, state::AsrMode::Offline, "".into());
    (app, ctx)
}

fn stage_label(app: &app::RhemaFlowApp) -> String {
    format!("{:?}:{}", app.stage.kind, app.stage.label)
}

/// The operator's real pieces and timestamps (seconds) from the screenshot of 2026-10-08.
const PIECES: [(u64, &str); 9] = [
    (194, "If any man being Christ."), (201, "He is."), (204, "A new."), (207, "Creation"), (210, "Old things."),
    (213, "I'm passed away."), (215, "Behold."), (218, "All things."), (220, "I'll become new."),
];

#[test]
fn the_operators_nine_garbled_pieces_end_up_on_the_screen() {
    let (mut app, _ctx) = new_app();
    app.auto_approve_quotes = true;
    for (s, t) in PIECES {
        app.handle_parse(t, s * 1000, 100);
        println!("{s:>4}s {t:28} -> stage {:30} queue {}", stage_label(&app), app.confirms.len());
    }
    let staged = app.stage.kind == StageKind::Scripture && app.stage.label.contains("2 Corinthians 5:17");
    let queued = app.confirms.iter().any(|c| format!("{:?}", c.det.passage).contains("5") );
    println!("final stage: {}  confirms: {}", stage_label(&app), app.confirms.len());
    assert!(staged || queued, "2 Corinthians 5:17 was neither staged nor queued: {}", stage_label(&app));
    assert!(staged, "with auto-approve on it should have gone straight to the screen: {}", stage_label(&app));
    // the spent pieces must not bring it back a second time
    let seq = app.stage_seq;
    app.handle_parse("so let us all pray together now", 230_000, 100);
    assert_eq!(app.stage_seq, seq, "nothing new should happen after");
}

#[test]
fn the_whole_sentence_said_at_once_still_works() {
    let (mut app, _ctx) = new_app();
    app.auto_approve_quotes = true;
    app.handle_parse("If anyone be in Christ he is a new creature", 5_000, 100);
    assert!(app.stage.label.contains("2 Corinthians 5:17"), "{}", stage_label(&app));
}

#[test]
fn ordinary_talk_never_reaches_the_screen() {
    let (mut app, _ctx) = new_app();
    app.auto_approve_quotes = true;
    let talk = ["so today I want to talk to you about", "what it means to follow Jesus", "in everyday life and in our families", "and I want you to turn to your neighbour", "and tell them God loves them", "the music team will lead us now"];
    for (i, t) in talk.iter().enumerate() {
        app.handle_parse(t, 10_000 + i as u64 * 3_000, 100);
    }
    assert_eq!(app.stage.kind, StageKind::Blank, "{}", stage_label(&app));
}

fn say(app: &mut app::RhemaFlowApp, t: &str, n: &mut u64) {
    *n += 20_000;
    app.handle_parse(t, *n, 100);
}

#[test]
fn every_new_voice_command_does_its_job() {
    let (mut app, _ctx) = new_app();
    app.mute_mic_while_reading = false;
    let mut t = 0u64;
    say(&mut app, "John 3:16", &mut t);
    assert!(app.stage.label.contains("John 3:16"), "{}", stage_label(&app));

    say(&mut app, "read till verse 20", &mut t);
    let p = app.parse_ctx.last.unwrap();
    assert_eq!((p.chapter, p.from, p.to), (3, 16, 20), "read till verse 20");
    assert!(app.stage.label.contains("16"), "{}", stage_label(&app));

    say(&mut app, "John 3:16", &mut t);
    say(&mut app, "read the chapter", &mut t);
    let p = app.parse_ctx.last.unwrap();
    assert_eq!((p.from, p.to), (16, 36), "to the end of John 3");

    say(&mut app, "final verse", &mut t);
    let p = app.parse_ctx.last.unwrap();
    assert_eq!((p.from, p.to), (36, 36), "last verse of the chapter");

    say(&mut app, "read until verse five", &mut t);
    assert!(app.short_utterance_note.as_ref().unwrap().0.contains("comes before"), "{:?}", app.short_utterance_note);

    say(&mut app, "go dark", &mut t);
    assert!(app.blackout);
    say(&mut app, "screen on", &mut t);
    assert!(!app.blackout);

    for (said, want) in [("in the KJV", "KJV"), ("B B E", "BBE"), ("K J V", "KJV"), ("basic english", "BBE"), ("King James Version", "KJV")] {
        say(&mut app, said, &mut t);
        assert_eq!(app.translation, want, "{said}");
    }
    let before = app.translation.clone();
    say(&mut app, "next translation", &mut t);
    assert_ne!(app.translation, before);
    say(&mut app, "previous translation", &mut t);
    assert_eq!(app.translation, before);
    say(&mut app, "N I V", &mut t);
    assert_eq!(app.translation, before, "NIV is not installed: nothing changes");
    assert!(app.short_utterance_note.as_ref().unwrap().0.contains("NIV"), "{:?}", app.short_utterance_note);

    say(&mut app, "show history", &mut t);
    assert!(app.history_visible);
    say(&mut app, "hide history", &mut t);
    assert!(!app.history_visible);
    say(&mut app, "hide the browser", &mut t);
    assert!(!app.browser.visible);
    say(&mut app, "show the browser", &mut t);
    assert!(app.browser.visible);
    say(&mut app, "open media", &mut t);
    assert!(app.present.dock_open);
    say(&mut app, "close media", &mut t);
    assert!(!app.present.dock_open);

    say(&mut app, "lower third", &mut t);
    assert_eq!(app.stage_style, StageStyle::LowerThird);
    say(&mut app, "green screen", &mut t);
    assert_eq!(app.stage_style, StageStyle::ChromaGreen);
    say(&mut app, "large print", &mut t);
    assert_eq!(app.stage_style, StageStyle::LargePrint);
    say(&mut app, "classic look", &mut t);
    assert_eq!(app.stage_style, StageStyle::Classic);

    say(&mut app, "what is the greek for love", &mut t);
    assert!(app.strongs.open);
    assert_eq!(app.strongs.selected.as_deref(), Some("G26"));
    say(&mut app, "hide strongs", &mut t);
    assert!(!app.strongs.open);
    say(&mut app, "show strongs", &mut t);
    assert!(app.strongs.open);

    say(&mut app, "start a five minute countdown", &mut t);
    assert_eq!(app.stage.kind, StageKind::Countdown);
    assert_eq!(app.stage.text, "05:00");
    say(&mut app, "pause the timer", &mut t);
    say(&mut app, "resume the timer", &mut t);
    say(&mut app, "stop the timer", &mut t);
    assert_eq!(app.stage.kind, StageKind::Blank, "stopping the timer takes it off the screen");
    say(&mut app, "show the clock", &mut t);
    assert_eq!(app.stage.kind, StageKind::Countdown);
    say(&mut app, "show the timer", &mut t);
    assert_eq!(app.stage.kind, StageKind::Countdown);

    say(&mut app, "put the stage on screen nine", &mut t);
    say(&mut app, "put the stage on screen two", &mut t);
    assert!(app.short_utterance_note.as_ref().unwrap().0.contains("screen"), "{:?}", app.short_utterance_note);

    say(&mut app, "dismiss all", &mut t);
    assert!(app.confirms.is_empty());
}
