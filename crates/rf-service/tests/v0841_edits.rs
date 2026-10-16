//! v0.841.0: notes, planned minutes, rename and whole-list restore (the base of undo / redo).
use rf_service::{ItemKind, Mode, Output, Runner, Service};

fn runner() -> (Runner, u64, u64, u64) {
    let mut r = Runner::new(Service::new("t"));
    let a = r.add_text_item("Welcome", "one\n\ntwo").unwrap();
    let b = r.add_item("Reading", ItemKind::Scripture { reference: "John 3:16".into(), translation: "ASV".into() }).unwrap();
    let c = r.add_item("Timer", ItemKind::Timer { seconds: 60, label: "x".into() }).unwrap();
    (r, a, b, c)
}

#[test]
fn notes_minutes_and_rename_work_and_old_files_still_load() {
    let (mut r, a, _, _) = runner();
    r.rename_item(a, "  Welcome and notices ").unwrap();
    r.rename_item(a, "   ").unwrap(); // blank is ignored
    r.set_item_notes(a, "start after the prayer").unwrap();
    r.set_item_minutes(a, Some(5)).unwrap();
    r.set_item_minutes(a, Some(99999)).unwrap();
    let it = r.service().item(a).unwrap();
    assert_eq!(it.title, "Welcome and notices");
    assert_eq!(it.notes, "start after the prayer");
    assert_eq!(it.minutes, Some(600));
    // a file written by v0.840 has neither field: it must still load
    let old = r#"{"id":7,"title":"Old","kind":{"type":"media","path":"/x.png"}}"#;
    let item: rf_service::Item = serde_json::from_str(old).unwrap();
    assert_eq!((item.notes.as_str(), item.minutes), ("", None));
}

#[test]
fn notes_are_allowed_while_locked_but_rename_is_not() {
    let (mut r, a, _, _) = runner();
    r.set_mode(Mode::Show);
    assert!(r.set_item_notes(a, "ok").is_ok());
    assert!(r.rename_item(a, "no").is_err());
}

#[test]
fn restoring_a_list_clears_the_screen_only_if_the_live_item_vanished() {
    let (mut r, a, b, c) = runner();
    let before = r.service().items.clone();
    r.go(b, 0).unwrap();
    r.remove_item(c).unwrap();
    r.restore_items(before.clone()).unwrap(); // c is back; b is still live
    assert_eq!(r.output(), Output::Content { item_id: b, slide: 0 });
    let without_b: Vec<_> = before.iter().filter(|i| i.id != b).cloned().collect();
    r.restore_items(without_b).unwrap();
    assert_eq!(r.output(), Output::Clear);
    let _ = a;
}
