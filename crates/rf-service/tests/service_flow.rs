use rf_service::*;
use std::fs;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rf-service-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// The roadmap's Phase 1 "done when" service: 10 items.
fn ten_item_service() -> Service {
    let mut s = Service::new("Sunday 10:30");
    s.add_text_item("Welcome", "Welcome to church\n\nPlease silence your phones");
    s.add_text_item("Prayer", "Let us pray");
    s.add_item("Reading", ItemKind::Scripture { reference: "Psalm 23".into(), translation: "KJV".into() });
    s.add_item("Countdown", ItemKind::Timer { seconds: 300, label: "Starting soon".into() });
    s.add_text_item("Notices", "Youth camp\n\nFoodbank\n\nCoffee morning");
    s.add_item("Walk-in", ItemKind::Media { path: "media/walkin.mp4".into() });
    s.add_item("Song 1", ItemKind::Song { song_id: "s1".into(), arrangement: vec!["V1".into(), "C".into(), "V2".into(), "C".into()] });
    s.add_item("Sermon text", ItemKind::Scripture { reference: "John 3:16".into(), translation: "KJV".into() });
    s.add_text_item("Offering", "Give online or in the plate");
    s.add_text_item("Blessing", "Go in peace");
    s
}

fn total_positions(s: &Service) -> usize {
    s.items.iter().map(|i| i.slide_count()).sum()
}

#[test]
fn keyboard_only_walk_visits_every_position_once_then_stops() {
    let svc = ten_item_service();
    let expected = total_positions(&svc);
    let mut r = Runner::new(svc);
    assert_eq!(r.output(), Output::Clear, "starts blank");
    let mut seen = 0;
    while r.next() {
        seen += 1;
        assert!(matches!(r.output(), Output::Content { .. }));
    }
    assert_eq!(seen, expected);
    // at the very end, next() is a no-op, not a wraparound
    let before = r.output();
    assert!(!r.next());
    assert_eq!(r.output(), before);
    // and we can walk all the way back
    let mut back = 0;
    while r.prev() {
        back += 1;
    }
    assert_eq!(back, expected - 1);
}

#[test]
fn empty_items_are_skipped_in_navigation() {
    let mut s = Service::new("t");
    let a = s.add_text_item("A", "one");
    s.add_item("Empty", ItemKind::Slides { slides: vec![] });
    let c = s.add_text_item("C", "three");
    let mut r = Runner::new(s);
    r.go(a, 0).unwrap();
    assert!(r.next());
    assert_eq!(r.output(), Output::Content { item_id: c, slide: 0 });
    assert!(r.prev());
    assert_eq!(r.output(), Output::Content { item_id: a, slide: 0 });
}

#[test]
fn panic_keys_and_restore() {
    let mut r = Runner::new(ten_item_service());
    r.next();
    let shown = r.output();
    r.black();
    assert_eq!(r.output(), Output::Black);
    r.logo();
    assert_eq!(r.output(), Output::Logo);
    r.clear();
    assert_eq!(r.output(), Output::Clear);
    assert!(r.restore_content());
    assert_eq!(r.output(), shown, "un-clear returns to the last slide");
}

#[test]
fn undo_is_ten_deep_and_safe_when_target_was_deleted() {
    let mut r = Runner::new(ten_item_service());
    for _ in 0..15 {
        r.next();
    }
    let mut undone = 0;
    while r.undo() {
        undone += 1;
    }
    assert_eq!(undone, 10, "undo depth is capped at 10");

    // undo into an item that has since been deleted falls back to Clear
    let mut s = Service::new("t");
    let a = s.add_text_item("A", "one");
    let b = s.add_text_item("B", "two");
    let mut r = Runner::new(s);
    r.go(a, 0).unwrap();
    r.go(b, 0).unwrap();
    r.remove_item(a).unwrap(); // a is in the undo stack
    assert!(r.undo());
    assert_eq!(r.output(), Output::Clear);
}

#[test]
fn show_mode_blocks_edits_but_not_driving_the_show() {
    let mut r = Runner::new(ten_item_service());
    let first = r.service().items[0].id;
    r.set_mode(Mode::Show);
    assert_eq!(r.add_text_item("x", "y"), Err(RunError::Locked));
    assert_eq!(r.remove_item(first), Err(RunError::Locked));
    assert_eq!(r.move_item(first, 3), Err(RunError::Locked));
    assert_eq!(r.edit_slide_text(first, 0, "typo"), Err(RunError::Locked));
    assert!(r.next(), "navigation still works while locked");
    r.clear();
    r.set_mode(Mode::Edit);
    assert!(r.edit_slide_text(first, 0, "fixed").is_ok());
}

#[test]
fn reordering_or_deleting_other_items_never_changes_what_is_live() {
    let mut r = Runner::new(ten_item_service());
    let live = r.service().items[2].id; // Reading
    r.go(live, 0).unwrap();
    let out = r.output();
    let first = r.service().items[0].id;
    r.move_item(first, 9).unwrap();
    r.move_item(live, 0).unwrap();
    assert_eq!(r.output(), out);
    let other = r.service().items[5].id;
    r.remove_item(other).unwrap();
    assert_eq!(r.output(), out);
    // deleting the live item itself blanks the screen rather than dangling
    r.remove_item(live).unwrap();
    assert_eq!(r.output(), Output::Clear);
}

#[test]
fn bad_positions_are_errors_not_panics() {
    let mut r = Runner::new(ten_item_service());
    assert_eq!(r.go(9999, 0), Err(RunError::NoSuchItem(9999)));
    let id = r.service().items[1].id;
    assert!(matches!(r.go(id, 50), Err(RunError::NoSuchSlide { .. })));
}

#[test]
fn theme_validation_rejects_invisible_text() {
    let mut t = Theme::default();
    assert!(t.validate().is_ok());
    t.text_rgb = t.background_rgb;
    assert!(t.validate().is_err());
    let mut t = Theme::default();
    t.font_size_pt = 2.0;
    assert!(t.validate().is_err());
}

#[test]
fn save_load_roundtrip_and_resume_after_kill() {
    let dir = tmp("resume");
    let path = dir.join("service.json");
    let mut r = Runner::new(ten_item_service());
    for _ in 0..7 {
        r.next();
    }
    let live = r.output();
    // autosave happens, then the process is "killed" (we just drop the runner)
    save(&path, r.service()).unwrap();
    drop(r);

    let out = load(&path).unwrap();
    assert!(out.recovered_from.is_none());
    let mut r2 = Runner::new(out.service);
    assert_eq!(r2.output(), Output::Clear, "a relaunch never auto-shows to the congregation");
    assert_eq!(r2.resume_point(), Some(live));
    assert!(r2.resume());
    assert_eq!(r2.output(), live);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn corrupt_main_file_recovers_from_newest_backup() {
    let dir = tmp("corrupt");
    let path = dir.join("service.json");
    let mut s = ten_item_service();
    save(&path, &s).unwrap();
    s.title = "second save".into();
    save(&path, &s).unwrap(); // first version now in .bak.1
    s.title = "third save".into();
    save(&path, &s).unwrap(); // second version now in .bak.1

    // simulate a torn/garbled file
    fs::write(&path, b"{ \"schema_version\": 1, \"title\": ").unwrap();
    let out = load(&path).unwrap();
    assert!(out.recovered_from.is_some());
    assert_eq!(out.service.title, "second save", "newest good backup wins");

    // garbage everywhere → a clear error, not a panic
    for n in 1..=store::BACKUP_COUNT {
        let mut p = path.as_os_str().to_owned();
        p.push(format!(".bak.{n}"));
        let _ = fs::write(PathBuf::from(p), b"nope");
    }
    assert!(matches!(load(&path), Err(StoreError::Unrecoverable(_))));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn backups_rotate_to_a_fixed_window_and_unchanged_saves_dont_churn() {
    let dir = tmp("rotate");
    let path = dir.join("service.json");
    let mut s = Service::new("v0");
    for i in 0..12 {
        s.title = format!("v{i}");
        save(&path, &s).unwrap();
        save(&path, &s).unwrap(); // identical second save: must be a no-op
    }
    let count = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".bak."))
        .count();
    assert_eq!(count, store::BACKUP_COUNT);
    // .bak.1 is the immediately previous version (v10), not a duplicate of v11
    let b1: Service = serde_json::from_slice(&fs::read(dir.join("service.json.bak.1")).unwrap()).unwrap();
    assert_eq!(b1.title, "v10");
    // no temp files left behind
    assert!(!fs::read_dir(&dir).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains(".tmp-")));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn newer_schema_is_refused_and_not_masked_by_old_backups() {
    let dir = tmp("toonew");
    let path = dir.join("service.json");
    let s = Service::new("old");
    save(&path, &s).unwrap();
    let mut s2 = Service::new("new");
    s2.title = "changed".into();
    save(&path, &s2).unwrap(); // creates a .bak.1 that IS readable
    let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    v["schema_version"] = serde_json::Value::from(SCHEMA_VERSION + 1);
    fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(matches!(load(&path), Err(StoreError::TooNew { .. })));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn migration_chain_runs_in_order_and_unknown_gaps_fail() {
    fn v1_to_v2(v: &mut serde_json::Value) {
        let t = v["title"].take();
        v["name"] = t; // pretend v2 renamed the field
    }
    fn v2_to_v3(v: &mut serde_json::Value) {
        v["extra"] = serde_json::Value::from(true);
    }
    let steps: &[(u32, fn(&mut serde_json::Value))] = &[(1, v1_to_v2), (2, v2_to_v3)];
    let input = serde_json::json!({ "schema_version": 1, "title": "hello" });
    let out = store::migrate_with(input.clone(), 3, steps).unwrap();
    assert_eq!(out["schema_version"], 3);
    assert_eq!(out["name"], "hello");
    assert_eq!(out["extra"], true);
    // missing step
    assert!(store::migrate_with(input, 3, &steps[..1]).is_err());
    // missing version field
    assert!(store::migrate_with(serde_json::json!({"title": "x"}), 1, &[]).is_err());
}

#[test]
fn autosave_debounces_but_never_starves() {
    let mut a = Autosave::new(800, 5_000);
    assert!(!a.should_save(0));
    a.mark_dirty(1_000);
    assert!(!a.should_save(1_500), "still inside debounce");
    assert!(a.should_save(1_800));
    a.saved();
    assert!(!a.should_save(9_999));
    // continuous typing: a change every 500 ms never lets debounce expire…
    let mut t = 10_000;
    a.mark_dirty(t);
    let mut saved_at = None;
    for _ in 0..30 {
        t += 500;
        a.mark_dirty(t);
        if a.should_save(t) {
            saved_at = Some(t);
            break;
        }
    }
    // …but max_wait forces a save within 5 s of the first change
    assert_eq!(saved_at, Some(15_000));
}

#[test]
fn service_file_is_plain_documented_json() {
    let s = ten_item_service();
    let v: serde_json::Value = serde_json::to_value(&s).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["items"][2]["kind"]["type"], "scripture");
    assert_eq!(v["items"][2]["kind"]["reference"], "Psalm 23");
    // scripture items carry a reference, never translation text
    assert!(v["items"][2]["kind"].get("text").is_none());
}
