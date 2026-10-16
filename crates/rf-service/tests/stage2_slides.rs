//! Stage 2 spec section 1 (slides) and S1.7 (import). Written BEFORE the code.
use rf_service::slides::{slides_from_text, text_from_bytes, text_from_pptx};

fn texts(v: &[rf_service::slides::ParsedSlide]) -> Vec<&str> {
    v.iter().map(|s| s.text.as_str()).collect()
}

#[test]
fn s1_1_a_blank_line_starts_a_new_slide() {
    let s = slides_from_text("Welcome to church\n\nPlease sit\nand pray\n\n\n\nAnnouncements");
    assert_eq!(texts(&s), vec!["Welcome to church", "Please sit\nand pray", "Announcements"]);
}

#[test]
fn s1_1_windows_line_endings_work_like_normal_ones() {
    let s = slides_from_text("One\r\n\r\nTwo line a\r\nTwo line b\r\n\r\nThree");
    assert_eq!(texts(&s), vec!["One", "Two line a\nTwo line b", "Three"]);
}

#[test]
fn s1_1_a_line_with_only_spaces_counts_as_blank() {
    let s = slides_from_text("A\n   \t \nB");
    assert_eq!(texts(&s), vec!["A", "B"]);
}

#[test]
fn s1_2_first_line_ending_in_a_colon_is_a_label() {
    let s = slides_from_text("Chorus:\nAmazing grace\nhow sweet\n\nVerse 2:\nTwas grace");
    assert_eq!(s[0].label, "Chorus");
    assert_eq!(s[0].text, "Amazing grace\nhow sweet");
    assert_eq!(s[1].label, "Verse 2");
    assert_eq!(s[1].text, "Twas grace");
}

#[test]
fn s1_2_a_lone_line_ending_in_a_colon_is_text_not_a_label() {
    let s = slides_from_text("Note:");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].label, "");
    assert_eq!(s[0].text, "Note:");
}

#[test]
fn s1_2_a_long_or_double_colon_first_line_is_not_a_label() {
    let long = format!("{}:\nbody", "x".repeat(31));
    let s = slides_from_text(&long);
    assert_eq!(s[0].label, "");
    assert!(s[0].text.starts_with("xxx"));
    // exactly 30 characters is still allowed
    let ok = format!("{}:\nbody", "y".repeat(30));
    assert_eq!(slides_from_text(&ok)[0].label.len(), 30);
    // a colon inside the line means it is a sentence, not a label
    let s = slides_from_text("Time: 10:30:\nbody");
    assert_eq!(s[0].label, "");
    assert_eq!(s[0].text, "Time: 10:30:\nbody");
}

#[test]
fn s1_3_empty_and_blank_input_give_no_slides() {
    assert!(slides_from_text("").is_empty());
    assert!(slides_from_text("  \n\n \r\n\t\n").is_empty());
}

#[test]
fn s1_3_words_in_other_scripts_survive() {
    let s = slides_from_text("Aleluya, gloire à Dieu\n\nЛюбовь Божья");
    assert_eq!(texts(&s), vec!["Aleluya, gloire à Dieu", "Любовь Божья"]);
}

#[test]
fn s1_7_text_files_are_read_with_or_without_a_bom_and_never_fail() {
    assert_eq!(text_from_bytes(b"\xEF\xBB\xBFHello\r\nthere"), "Hello\nthere");
    assert_eq!(text_from_bytes("café".as_bytes()), "café");
    // invalid UTF-8 becomes a replacement character, not an error
    let t = text_from_bytes(b"ok \xFF\xFE end");
    assert!(t.starts_with("ok ") && t.ends_with(" end"));
}

#[test]
fn s1_7_pptx_gives_one_slide_per_powerpoint_slide_in_slide_number_order() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample.pptx")).unwrap();
    let text = text_from_pptx(&bytes).expect("a real .pptx opens");
    let slides = slides_from_text(&text);
    assert_eq!(slides.len(), 12, "12 slides in, 12 slides out: {text}");
    // slide 2 must come before slide 10 (numeric order, not alphabetical)
    let order: Vec<String> = slides.iter().map(|s| s.text.lines().next().unwrap().to_string()).collect();
    let expected: Vec<String> = (1..=12).map(|n| format!("Slide {n}")).collect();
    assert_eq!(order, expected);
    // paragraphs become lines, entities are turned back into characters
    assert_eq!(slides[0].text, "Slide 1\nTom & Jerry <Welcome>\nTea after church");
    assert_eq!(slides[1].text, "Slide 2\nSecond slide body");
}

#[test]
fn s1_7_a_file_that_is_not_a_pptx_is_a_plain_error() {
    let e = text_from_pptx(b"this is not a zip file").unwrap_err();
    assert!(e.to_lowercase().contains("powerpoint") || e.to_lowercase().contains("pptx"), "{e}");
    // a zip that is not a presentation
    let e2 = text_from_pptx(&[0x50, 0x4B, 0x05, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap_err();
    assert!(!e2.is_empty());
}
