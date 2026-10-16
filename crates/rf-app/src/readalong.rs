//! Read-along (v0.835.0): the whole chapter on the screen with the verse being read picked out, moving down as the
//! preacher reads. The chapter is shown from the Bible; which verse is being read is worked out by comparing the last
//! words heard with the next few verses. It only ever moves forward by itself; the operator can move it either way.

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| !w.is_empty()).map(|w| w.to_lowercase().replace('\'', "")).collect()
}

/// Small words that say little about WHICH verse is being read.
const FILLER: &[&str] = &["the", "and", "of", "to", "a", "in", "that", "he", "it", "is", "was", "for", "i", "his", "unto", "shall", "ye", "with", "not", "but", "they", "them", "be", "my", "me", "thou", "thy"];

fn significant(w: &[String]) -> Vec<&String> {
    w.iter().filter(|x| !FILLER.contains(&x.as_str())).collect()
}

/// How much of the first part of `verse` appears in `heard` (0.0 to 1.0), counting meaningful words only.
pub fn score(verse: &str, heard: &str) -> f32 {
    let v = words(verse);
    let h = words(heard);
    let mut vs: Vec<&String> = Vec::new();
    for w in significant(&v) {
        if !vs.contains(&w) {
            vs.push(w);
        }
        if vs.len() == 8 {
            break;
        }
    }
    if vs.is_empty() || h.is_empty() {
        return 0.0;
    }
    let hit = vs.iter().filter(|w| h.contains(w)).count();
    hit as f32 / vs.len() as f32
}

/// Given the verse now highlighted and what was just heard, the verse that should be highlighted.
/// Looks at the current verse and the next three; needs more than half of a verse's opening meaningful words, and a better fit than the verse already shown, to move.
pub fn advance(verses: &[String], current: usize, heard: &str) -> usize {
    if verses.is_empty() {
        return 0;
    }
    let current = current.min(verses.len() - 1);
    let mut best = current;
    // the verse being read now is the one the words fit best: a later verse must fit clearly better
    let mut best_score = score(&verses[current], heard).max(0.549);
    for j in (current + 1)..verses.len().min(current + 4) {
        let s = score(&verses[j], heard);
        if s > best_score {
            best = j;
            best_score = s;
        }
    }
    best
}

/// The lines for the screen: "16  For God so loved the world..." with verse numbers.
pub fn lines(verses: &[String], first_verse_number: usize) -> Vec<String> {
    verses.iter().enumerate().map(|(i, v)| format!("{}  {}", first_verse_number + i, v.trim())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn john() -> Vec<String> {
        [
            "For God so loved the world, that he gave his only begotten Son",
            "For God sent not his Son into the world to condemn the world",
            "He that believeth on him is not condemned: but he that believeth not is condemned already",
            "And this is the condemnation, that light is come into the world",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    #[test]
    fn it_moves_forward_when_the_next_verse_starts_being_read() {
        let v = john();
        assert_eq!(advance(&v, 0, "for God so loved the world that he gave his only begotten son"), 0, "still on the first verse");
        assert_eq!(advance(&v, 0, "for God sent not his son into the world"), 1);
        assert_eq!(advance(&v, 1, "he that believeth on him is not condemned"), 2);
    }

    #[test]
    fn it_can_skip_a_verse_the_preacher_jumped_but_never_goes_back_or_off_the_end() {
        let v = john();
        assert_eq!(advance(&v, 0, "he that believeth on him is not condemned"), 2);
        assert_eq!(advance(&v, 2, "for God so loved the world"), 2, "never back by itself");
        assert_eq!(advance(&v, 3, "anything"), 3);
        assert_eq!(advance(&[], 5, "x"), 0);
        assert_eq!(advance(&v, 99, "x"), 3);
    }

    #[test]
    fn chatter_and_filler_do_not_move_it() {
        let v = john();
        assert_eq!(advance(&v, 0, "and the of to a in that he it is was for"), 0);
        assert_eq!(advance(&v, 0, "let us turn to the next page"), 0);
        assert_eq!(advance(&v, 0, ""), 0);
    }

    #[test]
    fn lines_carry_verse_numbers() {
        let l = lines(&john(), 16);
        assert!(l[0].starts_with("16  For God so loved") && l[3].starts_with("19  And this"));
    }
}
