//! Stage 2 spec section 2 (song library). Written BEFORE the code.
use rf_service::songs::{load_library, save_library, Library, Song};

const GRACE: &str = "Verse 1:\nAmazing grace how sweet the sound\nThat saved a wretch like me\n\nChorus:\nMy chains are gone\nI have been set free";

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("rf-songs-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.join("songs.json")
}

#[test]
fn s2_2_sections_are_parsed_and_unlabelled_blocks_are_named_verse_n() {
    let s = Song::from_text("Mixed", "Words one\n\nChorus:\nSing it\n\nWords two");
    let labels: Vec<&str> = s.sections.iter().map(|x| x.label.as_str()).collect();
    assert_eq!(labels, vec!["Verse 1", "Chorus", "Verse 2"], "only unlabelled blocks are counted");
    assert_eq!(s.sections[0].text, "Words one");
}

#[test]
fn s2_1_empty_arrangement_means_written_order() {
    let s = Song::from_text("Grace", GRACE);
    let (secs, unknown) = s.ordered();
    assert_eq!(secs.iter().map(|x| x.label.as_str()).collect::<Vec<_>>(), vec!["Verse 1", "Chorus"]);
    assert!(unknown.is_empty());
}

#[test]
fn s2_4_a_repeated_chorus_makes_a_slide_each_time() {
    let mut s = Song::from_text("Grace", GRACE);
    s.set_arrangement_text("Verse 1, Chorus, Chorus");
    let (slides, unknown) = s.slides(0);
    let labels: Vec<&str> = slides.iter().map(|x| x.label.as_str()).collect();
    assert_eq!(labels, vec!["Verse 1", "Chorus", "Chorus"]);
    assert!(unknown.is_empty());
}

#[test]
fn s2_3_arrangement_matches_labels_ignoring_case_and_spaces_and_reports_unknown() {
    let mut s = Song::from_text("Grace", GRACE);
    s.set_arrangement_text("  verse   1 ,CHORUS, Bridge ,, chorus");
    let (secs, unknown) = s.ordered();
    assert_eq!(secs.len(), 3, "verse 1, chorus, chorus");
    assert_eq!(unknown, vec!["Bridge".to_string()]);
    assert_eq!(s.arrangement, vec!["verse 1", "CHORUS", "Bridge", "chorus"], "blank entries dropped, text otherwise kept");
}

#[test]
fn s2_4_long_sections_are_split_into_pieces_with_numbered_labels() {
    let s = Song::from_text("Long", "Chorus:\nl1\nl2\nl3\nl4\nl5\nl6\nl7");
    let (slides, _) = s.slides(3);
    let got: Vec<(&str, &str)> = slides.iter().map(|x| (x.label.as_str(), x.text.as_str())).collect();
    assert_eq!(got, vec![("Chorus", "l1\nl2\nl3"), ("Chorus (2)", "l4\nl5\nl6"), ("Chorus (3)", "l7")]);
    // 0 means never split
    assert_eq!(s.slides(0).0.len(), 1);
    // a section that already fits is left alone
    assert_eq!(s.slides(7).0.len(), 1);
}

#[test]
fn s2_1_song_text_round_trips() {
    let s = Song::from_text("Grace", GRACE);
    let again = Song::from_text("Grace", &s.to_text());
    assert_eq!(s.sections, again.sections);
}

#[test]
fn s2_5_search_ranks_title_start_then_title_contains_then_lyrics() {
    let mut lib = Library::new();
    lib.add(Song::from_text("Grace Alone", "Verse 1:\nonly by faith"));
    lib.add(Song::from_text("Amazing Grace", GRACE));
    lib.add(Song::from_text("Faith Of Our Fathers", "Verse 1:\nliving still\nin spite of grace"));
    lib.add(Song::from_text("Zeal", "Verse 1:\nnothing here"));
    let titles = |q: &str| lib.search(q).iter().map(|s| s.title.clone()).collect::<Vec<_>>();
    assert_eq!(titles("grace"), vec!["Grace Alone", "Amazing Grace", "Faith Of Our Fathers"]);
    assert_eq!(titles("  GRACE!! "), vec!["Grace Alone", "Amazing Grace", "Faith Of Our Fathers"], "case and punctuation do not matter");
    assert_eq!(titles(""), vec!["Amazing Grace", "Faith Of Our Fathers", "Grace Alone", "Zeal"], "empty search = everything A to Z");
}

#[test]
fn s2_5_every_search_word_must_match_somewhere() {
    let mut lib = Library::new();
    lib.add(Song::from_text("Amazing Grace", GRACE));
    lib.add(Song::from_text("Other", "Verse 1:\nsweet things"));
    let t = |q: &str| lib.search(q).iter().map(|s| s.title.clone()).collect::<Vec<_>>();
    assert_eq!(t("sweet sound"), vec!["Amazing Grace"], "words from the lyrics find the song");
    assert_eq!(t("sweet sound wretch"), vec!["Amazing Grace"]);
    assert!(t("sweet banana").is_empty());
    assert!(t("nothing like this").is_empty());
}

#[test]
fn s2_7_ids_are_never_reused_after_a_delete() {
    let mut lib = Library::new();
    let a = lib.add(Song::from_text("A", "x"));
    let b = lib.add(Song::from_text("B", "y"));
    assert!(lib.remove(b));
    assert!(!lib.remove(b), "second delete says nothing was deleted");
    let c = lib.add(Song::from_text("C", "z"));
    assert!(c != a && c != b);
    assert!(lib.get(a).is_some() && lib.get(b).is_none());
}

#[test]
fn s2_6_the_library_is_saved_and_loaded_and_missing_means_empty() {
    let path = tmp("roundtrip");
    let loaded = load_library(&path);
    assert!(loaded.library.songs().is_empty() && loaded.warning.is_none(), "no file = empty, no scary message");
    let mut lib = Library::new();
    let id = lib.add(Song::from_text("Amazing Grace", GRACE));
    lib.get_mut(id).unwrap().author = "John Newton".into();
    lib.get_mut(id).unwrap().set_arrangement_text("Verse 1, Chorus, Chorus");
    save_library(&path, &lib).unwrap();
    let back = load_library(&path);
    assert!(back.warning.is_none());
    assert_eq!(back.library.get(id).unwrap().author, "John Newton");
    assert_eq!(back.library.get(id).unwrap().arrangement, vec!["Verse 1", "Chorus", "Chorus"]);
    // ids keep going up after a reload
    let mut l2 = back.library;
    assert!(l2.add(Song::from_text("New", "q")) > id);
    // it is plain readable JSON
    let json = std::fs::read_to_string(&path).unwrap();
    assert!(json.contains("Amazing Grace") && json.contains("schema_version"));
}

#[test]
fn s2_6_a_damaged_library_is_kept_aside_and_reported_never_thrown_away() {
    let path = tmp("damaged");
    std::fs::write(&path, b"{ this is not json").unwrap();
    let loaded = load_library(&path);
    assert!(loaded.library.songs().is_empty());
    let w = loaded.warning.expect("the operator is told");
    assert!(w.contains("songs.json.damaged"), "{w}");
    assert_eq!(std::fs::read(path.with_file_name("songs.json.damaged")).unwrap(), b"{ this is not json");
    // saving afterwards works and does not lose the kept copy
    save_library(&path, &Library::new()).unwrap();
    assert!(path.with_file_name("songs.json.damaged").exists());
}
