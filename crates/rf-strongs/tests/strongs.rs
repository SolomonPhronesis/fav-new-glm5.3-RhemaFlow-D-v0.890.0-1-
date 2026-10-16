//! v0.816.0 Strong's. Written BEFORE the code.
use rf_strongs::{ascii_fold, normalize_number, Lexicon, TaggedBible, CURATED};

fn greek() -> &'static Lexicon {
    use std::sync::OnceLock;
    static L: OnceLock<Lexicon> = OnceLock::new();
    L.get_or_init(Lexicon::greek)
}

const HEBREW_MINI: &str = r#"{"schema":1,"language":"H","source":"test","license":"test only","entries":{
 "H7965":{"l":"שָׁלוֹם","t":"shâlôm","p":"shaw-lome'","v":"from H7999","d":"safe, i.e. well, happy, friendly; welfare","k":"peace, welfare, well"},
 "H430":{"l":"אֱלֹהִים","t":"ʼĕlôhîym","p":"el-o-heem'","v":"plural of H433","d":"gods in the ordinary sense","k":"God, god, judge"}}}"#;

#[test]
fn the_greek_dictionary_is_built_in_and_complete() {
    let g = greek();
    assert!(g.len() >= 5500, "{}", g.len());
    let e = g.get("G3056").expect("logos");
    assert_eq!((e.lemma.as_str(), ascii_fold(&e.translit).as_str()), ("λόγος", "logos"));
    assert!(e.kjv.contains("word") && e.definition.contains("something said"), "{e:?}");
    assert!(e.pron.contains("log"));
    assert!(g.licenses().iter().any(|l| l.contains("CC0")), "{:?}", g.licenses());
}

#[test]
fn numbers_are_understood_in_every_spelling() {
    assert_eq!(normalize_number("G3056").as_deref(), Some("G3056"));
    assert_eq!(normalize_number("g3056").as_deref(), Some("G3056"));
    assert_eq!(normalize_number("G03056").as_deref(), Some("G3056"));
    assert_eq!(normalize_number("strong:H07225").as_deref(), Some("H7225"));
    assert_eq!(normalize_number("  h 430 ").as_deref(), Some("H430"));
    assert_eq!(normalize_number("3056").as_deref(), Some("3056"), "a bare number stays bare; the lexicon decides");
    assert_eq!(normalize_number("love"), None);
    assert_eq!(normalize_number("G"), None);
    assert_eq!(normalize_number(""), None);
    let g = greek();
    assert_eq!(g.get("g3056").unwrap().number, "G3056");
    assert_eq!(g.get("3056").unwrap().number, "G3056", "a bare number means Greek when only Greek has it");
    assert!(g.get("G999999").is_none());
}

#[test]
fn what_is_the_greek_for_word_gives_logos_then_rhema() {
    let r = greek().search_english("word", 'G', 5);
    let nums: Vec<&str> = r.iter().map(|e| e.number.as_str()).collect();
    assert_eq!(&nums[..2], ["G3056", "G4487"], "{nums:?}");
}

#[test]
fn what_is_the_greek_for_love_gives_the_three_famous_words_first() {
    let r = greek().search_english("love", 'G', 6);
    let nums: Vec<&str> = r.iter().map(|e| e.number.as_str()).collect();
    assert_eq!(&nums[..3], ["G26", "G25", "G5368"], "{nums:?}");
    for e in &r {
        assert!(e.kjv.to_lowercase().contains("love") || e.definition.to_lowercase().contains("love"), "{} does not mention love", e.number);
    }
}

#[test]
fn english_endings_are_forgiven() {
    let g = greek();
    for w in ["words", "Word", "WORD!", " word "] {
        assert_eq!(g.search_english(w, 'G', 3)[0].number, "G3056", "{w:?}");
    }
    let loved: Vec<&str> = g.search_english("loved", 'G', 4).iter().map(|e| e.number.as_str()).collect();
    assert!(loved.contains(&"G25") && loved.contains(&"G26"), "{loved:?}");
    assert!(g.search_english("believing", 'G', 5).iter().any(|e| e.number == "G4100"), "believing -> pisteuo");
    assert!(g.search_english("spirits", 'G', 5).iter().any(|e| e.number == "G4151"));
}

#[test]
fn a_word_with_no_greek_gives_nothing_and_odd_input_is_safe() {
    let g = greek();
    assert!(g.search_english("xylophone", 'G', 5).is_empty());
    assert!(g.search_english("", 'G', 5).is_empty());
    assert!(g.search_english("   ", 'G', 5).is_empty());
    assert!(g.search_english("1234", 'G', 5).is_empty());
    assert!(g.search_english(&"a".repeat(5000), 'G', 5).is_empty());
    assert!(g.search_english("love", 'G', 0).is_empty());
    assert!(g.search_english("love", 'G', 2).len() <= 2);
    assert!(g.search_english("peace", 'H', 5).is_empty(), "no Hebrew installed");
}

#[test]
fn the_first_picks_are_real_entries_with_the_right_word() {
    let g = greek();
    let mut checked = 0;
    for (english, picks) in CURATED {
        assert!(!english.is_empty() && !picks.is_empty());
        for (number, translit) in *picks {
            if number.starts_with('G') {
                let e = g.get(number).unwrap_or_else(|| panic!("{english}: {number} missing"));
                assert_eq!(ascii_fold(&e.translit), *translit, "{english}: {number} is {} not {translit}", e.translit);
                checked += 1;
            }
        }
    }
    assert!(checked >= 45, "{checked} Greek first-picks checked");
}

#[test]
fn search_by_sound_by_greek_letters_and_by_number() {
    let g = greek();
    assert_eq!(g.search("agape", 3)[0].number, "G26");
    assert_eq!(g.search("ἀγάπη", 3)[0].number, "G26");
    assert_eq!(g.search("λόγος", 3)[0].number, "G3056");
    assert_eq!(g.search("logos", 3)[0].number, "G3056");
    assert_eq!(g.search("G3056", 3)[0].number, "G3056");
    assert_eq!(g.search("3056", 3)[0].number, "G3056");
    assert_eq!(g.search("love", 3)[0].number, "G26", "an English word falls through to the English search");
    assert!(g.search("", 3).is_empty());
}

#[test]
fn hebrew_can_be_added_and_is_searched_with_greek() {
    let mut g = Lexicon::greek();
    let h = Lexicon::from_json(HEBREW_MINI).unwrap();
    g.merge(h);
    assert!(g.has_language('H') && g.has_language('G'));
    assert_eq!(g.get("H7965").unwrap().lemma, "שָׁלוֹם");
    assert_eq!(g.search_english("peace", 'H', 3)[0].number, "H7965");
    assert_eq!(g.search_english("God", 'H', 3)[0].number, "H430");
    let both: Vec<String> = g.search_english("peace", '?', 6).iter().map(|e| e.number.clone()).collect();
    assert!(both.contains(&"H7965".to_string()) && both.contains(&"G1515".to_string()), "{both:?}");
    assert!(g.licenses().iter().any(|l| l.contains("test only")));
    assert_eq!(g.search("shalom", 3)[0].number, "H7965", "diacritics are ignored");
}

#[test]
fn a_damaged_lexicon_file_is_a_plain_error() {
    assert!(Lexicon::from_json("not json").unwrap_err().contains("Strong"));
    assert!(Lexicon::from_json("{}").is_err());
    assert!(Lexicon::from_json(r#"{"schema":9,"language":"G","entries":{}}"#).unwrap_err().contains("version"));
}

#[test]
fn folding_removes_accents() {
    assert_eq!(ascii_fold("agápē"), "agape");
    assert_eq!(ascii_fold("ʼĕlôhîym"), "elohiym");
    assert_eq!(ascii_fold("PSYCHĒ"), "psyche");
}

// ------------------------------------------------------------ tagged Bible (way 1)

/// ILLUSTRATIVE test data written for this test (NOT a real module): the shape of a CrossWire-style OSIS file.
const OSIS: &str = r#"<?xml version="1.0"?><osis><osisText osisIDWork="test">
<div type="book" osisID="John"><chapter osisID="John.1">
<verse osisID="John.1.1">In the <w lemma="strong:G746">beginning</w> was the <w lemma="strong:G3056" morph="robinson:N-NSM">Word</w>, and the <w lemma="strong:G3056">Word</w> was with <w lemma="strong:G2316 lemma.TR:θεός">God</w>.</verse>
</chapter></div>
<div type="book" osisID="2Cor"><chapter osisID="2Cor.5">
<verse osisID="2Cor.5.17">Therefore if any man be in <w lemma="strong:G5547">Christ</w>, he is a <w lemma="strong:G2537">new</w> <w lemma="strong:G2937">creature</w>:</verse>
</chapter></div>
<div type="book" osisID="Gen"><chapter osisID="Gen.1">
<verse sID="Gen.1.1" osisID="Gen.1.1"/>In the <w lemma="strong:H07225">beginning</w> <w lemma="strong:H0430">God</w> created<verse eID="Gen.1.1"/>
</chapter></div>
<div type="book" osisID="Foo"><chapter osisID="Foo.1"><verse osisID="Foo.1.1">ignored</verse></chapter></div>
</osisText></osis>"#;

#[test]
fn a_tagged_bible_gives_each_word_its_number() {
    let t = TaggedBible::from_osis(OSIS).unwrap();
    let v = t.verse(42, 1, 1).expect("John 1:1");
    let tagged: Vec<(&str, Vec<&str>)> = v.iter().filter(|w| !w.strongs.is_empty()).map(|w| (w.text.as_str(), w.strongs.iter().map(String::as_str).collect())).collect();
    assert_eq!(tagged, vec![("beginning", vec!["G746"]), ("Word", vec!["G3056"]), ("Word", vec!["G3056"]), ("God", vec!["G2316"])]);
    // untagged words are kept too, one by one, so the whole verse can be shown
    let plain: Vec<&str> = v.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(plain.join(" "), "In the beginning was the Word and the Word was with God", "punctuation is not a word");
    assert_eq!(t.verse(46, 5, 17).unwrap().iter().filter(|w| !w.strongs.is_empty()).count(), 3);
    assert_eq!(t.len(), 3, "the unknown book is skipped");
}

#[test]
fn old_testament_numbers_lose_their_zero_padding_and_milestone_verses_work() {
    let t = TaggedBible::from_osis(OSIS).unwrap();
    let v = t.verse(0, 1, 1).expect("Genesis 1:1 in sID/eID style");
    let nums: Vec<&str> = v.iter().flat_map(|w| w.strongs.iter().map(String::as_str)).collect();
    assert_eq!(nums, vec!["H7225", "H430"]);
}

#[test]
fn a_tagged_bible_that_is_not_one_is_a_plain_error() {
    assert!(TaggedBible::from_osis("").unwrap_err().to_lowercase().contains("osis"));
    assert!(TaggedBible::from_osis("<osis><osisText></osisText></osis>").unwrap_err().to_lowercase().contains("no verses"));
    assert!(TaggedBible::from_osis("just some words").is_err());
    assert!(TaggedBible::from_osis(OSIS).unwrap().verse(0, 99, 99).is_none());
}

#[test]
fn which_numbers_are_in_a_verse_and_which_word_asked_for() {
    let t = TaggedBible::from_osis(OSIS).unwrap();
    assert_eq!(t.numbers_in(42, 1, 1), vec!["G746", "G3056", "G2316"], "in order, each once");
    assert!(t.numbers_in(0, 77, 1).is_empty());
}

#[test]
fn a_hebrew_file_made_by_the_install_tool_loads_and_the_first_picks_hold() {
    // tests/hebrew_sample.json = six entries cut from the real output of tools/fetch_strongs.py
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hebrew_sample.json")).unwrap();
    let mut g = Lexicon::greek();
    g.merge(Lexicon::from_json(&json).expect("the tool's output is readable"));
    assert!(g.has_language('H') && g.has_language('G'));
    assert_eq!(g.get("H7965").unwrap().lemma, "שָׁלוֹם");
    assert_eq!(g.search_english("peace", 'H', 3)[0].number, "H7965");
    assert_eq!(g.search_english("beginning", 'H', 3)[0].number, "H7225");
    assert_eq!(g.search_english("word", 'H', 3)[0].number, "H1697");
    assert_eq!(g.search("shalowm", 3)[0].number, "H7965");
    assert_eq!(g.search("H430", 1)[0].number, "H430");
    // every Hebrew first pick that is in the sample has the spelling we wrote down
    for (_, picks) in CURATED {
        for (number, translit) in *picks {
            if number.starts_with('H') {
                if let Some(e) = g.get(number) {
                    assert_eq!(ascii_fold(&e.translit), *translit, "{number}");
                }
            }
        }
    }
    let both: Vec<String> = g.search_english("peace", '?', 4).iter().map(|e| e.number.clone()).collect();
    assert_eq!(both[0], "G1515", "Greek first when both exist");
    assert!(both.contains(&"H7965".to_string()));
}
