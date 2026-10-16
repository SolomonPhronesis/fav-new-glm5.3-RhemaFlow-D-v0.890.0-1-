//! The 66 canonical books of the KJV with spoken aliases.
//!
//! Aliases cover: canonical names, common shortenings, numeric/ordinal/roman
//! prefixes for the numbered books, and the frequent spoken variants
//! ("revelations", "song of songs", "psalm" vs "psalms", ...).
//!
//! `freq` is a preaching-frequency weight (0-9) used as a tie-break when a
//! fuzzy match is ambiguous: between two equally-distant candidates the more
//! preached book wins ("john" beats "joel" for a garbled "jon").

pub struct BookInfo {
    pub name: &'static str,
    pub abbr: &'static str,
    pub freq: u8,
    pub aliases: &'static [&'static str],
}

macro_rules! book {
    ($name:literal, $abbr:literal, $freq:literal, [$($alias:literal),* $(,)?]) => {
        BookInfo { name: $name, abbr: $abbr, freq: $freq, aliases: &[$name, $($alias),*] }
    };
}

pub static BOOKS: [BookInfo; 66] = [
    book!("Genesis", "Gen", 7, ["gen", "book of genesis"]),
    book!("Exodus", "Exo", 5, ["exod", "ex", "book of exodus"]),
    book!("Leviticus", "Lev", 2, ["lev", "levidicus", "leviticus"]),
    book!("Numbers", "Num", 2, ["num", "book of numbers"]),
    book!("Deuteronomy", "Deu", 3, ["deut", "deuteronomy", "dueteronomy"]),
    book!("Joshua", "Jos", 4, ["josh", "joshuas", "book of joshua"]),
    book!("Judges", "Jud", 3, ["judg", "book of judges"]),
    book!("Ruth", "Rut", 3, ["ruth", "book of ruth"]),
    book!("1 Samuel", "1Sa", 4, ["first samuel", "one samuel", "1st samuel", "i samuel", "1 samuel", "first sam", "1st sam", "one sam"]),
    book!("2 Samuel", "2Sa", 3, ["second samuel", "two samuel", "2nd samuel", "ii samuel", "2 samuel", "second sam", "2nd sam", "two sam"]),
    book!("1 Kings", "1Ki", 3, ["first kings", "one kings", "1st kings", "i kings", "1 kings", "first kins", "firstkins"]),
    book!("2 Kings", "2Ki", 3, ["second kings", "two kings", "2nd kings", "ii kings", "2 kings", "second kins"]),
    book!("1 Chronicles", "1Ch", 2, ["first chronicles", "one chronicles", "1st chronicles", "i chronicles", "1 chronicles", "first cron", "first chronicles"]),
    book!("2 Chronicles", "2Ch", 2, ["second chronicles", "two chronicles", "2nd chronicles", "ii chronicles", "2 chronicles", "second cron"]),
    book!("Ezra", "Ezr", 2, ["ezra", "ezras"]),
    book!("Nehemiah", "Neh", 2, ["neh", "nehemias", "nyhemiah"]),
    book!("Esther", "Est", 3, ["est", "esther", "esters"]),
    book!("Job", "Job", 2, ["job", "jobe", "jobs"]),
    book!("Psalms", "Psa", 8, ["psalm", "psalms", "psalmes", "book of psalms", "book of psalm"]),
    book!("Proverbs", "Pro", 5, ["prov", "proverbs", "proverb", "book of proverbs"]),
    book!("Ecclesiastes", "Ecc", 1, ["eccl", "ecclesiastes", "eccleasiastes", "qoheleth"]),
    book!("Song of Solomon", "Sng", 1, ["song of songs", "song of solomon", "canticles", "canticle of canticles", "songs of solomon", "the song of solomon"]),
    book!("Isaiah", "Isa", 7, ["isa", "isaih", "isaiah", "esaias", "izaiah"]),
    book!("Jeremiah", "Jer", 4, ["jer", "jeremia", "jeremy", "jerimiah"]),
    book!("Lamentations", "Lam", 1, ["lam", "lamentations", "lamente"]),
    book!("Ezekiel", "Eze", 2, ["eze", "ezekiel", "ezechiel", "ezekiels"]),
    book!("Daniel", "Dan", 4, ["dan", "daniel", "daniels", "book of daniel"]),
    book!("Hosea", "Hos", 1, ["hos", "hosea", "hosei"]),
    book!("Joel", "Joe", 1, ["joel", "joels", "jole"]),
    book!("Amos", "Amo", 1, ["amos", "amoses", "amos"]),
    book!("Obadiah", "Oba", 0, ["obad", "obadiah", "obadia"]),
    book!("Jonah", "Jon", 3, ["jonah", "jona", "jonus", "johah"]),
    book!("Micah", "Mic", 1, ["mic", "micah", "micha", "mica"]),
    book!("Nahum", "Nah", 0, ["nah", "nahum", "nayhum"]),
    book!("Habakkuk", "Hab", 1, ["hab", "habakkuk", "habakuk", "habbakuk"]),
    book!("Zephaniah", "Zep", 0, ["zeph", "zephaniah", "zepaniah"]),
    book!("Haggai", "Hag", 0, ["hag", "haggai", "hagai"]),
    book!("Zechariah", "Zec", 1, ["zech", "zechariah", "zachariah", "zecharia"]),
    book!("Malachi", "Mal", 1, ["mal", "malachi", "malaki", "malachia"]),
    book!("Matthew", "Mat", 8, ["matt", "matthew", "mathew", "st matthew", "saint matthew", "gospel of matthew"]),
    book!("Mark", "Mar", 7, ["mark", "marks", "st mark", "saint mark", "gospel of mark"]),
    book!("Luke", "Luk", 7, ["luke", "lukes", "st luke", "saint luke", "gospel of luke"]),
    book!("John", "Joh", 9, ["john", "johns", "st john", "saint john", "gospel of john", "jon"]),
    book!("Acts", "Act", 5, ["acts", "act", "book of acts", "acts of the apostles", "the acts"]),
    book!("Romans", "Rom", 7, ["rom", "romans", "roman", "book of romans"]),
    book!("1 Corinthians", "1Co", 6, ["first corinthians", "one corinthians", "1st corinthians", "i corinthians", "1 corinthians", "first corinth", "1st corinth", "one corinth", "first cor"]),
    book!("2 Corinthians", "2Co", 4, ["second corinthians", "two corinthians", "2nd corinthians", "ii corinthians", "2 corinthians", "second corinth", "2nd corinth", "two corinth", "second cor"]),
    book!("Galatians", "Gal", 4, ["gal", "galatians", "galations", "galatia"]),
    book!("Ephesians", "Eph", 4, ["eph", "ephesians", "efesians", "ephesia"]),
    book!("Philippians", "Php", 3, ["phil", "philippians", "philipians", "phillipians", "filipians"]),
    book!("Colossians", "Col", 2, ["col", "colossians", "colosians", "colassians"]),
    book!("1 Thessalonians", "1Th", 2, ["first thessalonians", "one thessalonians", "1st thessalonians", "i thessalonians", "1 thessalonians", "first thess", "1st thess"]),
    book!("2 Thessalonians", "2Th", 1, ["second thessalonians", "two thessalonians", "2nd thessalonians", "ii thessalonians", "2 thessalonians", "second thess", "2nd thess"]),
    book!("1 Timothy", "1Ti", 3, ["first timothy", "one timothy", "1st timothy", "i timothy", "1 timothy", "first tim", "1st tim"]),
    book!("2 Timothy", "2Ti", 3, ["second timothy", "two timothy", "2nd timothy", "ii timothy", "2 timothy", "second tim", "2nd tim"]),
    book!("Titus", "Tit", 2, ["titus", "titu", "titus"]),
    book!("Philemon", "Phm", 1, ["philemon", "filemon", "philem"]),
    book!("Hebrews", "Heb", 5, ["heb", "hebrews", "hebrew", "book of hebrews"]),
    book!("James", "Jam", 5, ["jam", "james", "james", "st james"]),
    book!("1 Peter", "1Pe", 3, ["first peter", "one peter", "1st peter", "i peter", "1 peter", "first pet", "1st pet"]),
    book!("2 Peter", "2Pe", 2, ["second peter", "two peter", "2nd peter", "ii peter", "2 peter", "second pet", "2nd pet"]),
    book!("1 John", "1Jo", 4, ["first john", "one john", "1st john", "i john", "1 john", "first jon"]),
    book!("2 John", "2Jo", 1, ["second john", "two john", "2nd john", "ii john", "2 john"]),
    book!("3 John", "3Jo", 1, ["third john", "three john", "3rd john", "iii john", "3 john"]),
    book!("Jude", "Jud", 1, ["jude", "jud", "juda", "judas"]),
    book!("Revelation", "Rev", 5, ["rev", "revelation", "revelations", "the revelation", "book of revelation", "book of revelations"]),
];

/// Index of a book by canonical name (exact). Panics on unknown names —
/// only called with compile-time constants.
pub fn idx(name: &str) -> usize {
    BOOKS
        .iter()
        .position(|b| b.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("unknown book name: {name}"))
}

/// Every alias LOWERCASED next to its book index, built once (v0.853.0).
///
/// Why this exists: `match_book_at` compares lowercased speech, but the
/// static aliases carry their canonical capitals ("Exodus", "Judges"). A
/// case-sensitive edit distance charged every capitalized alias one full
/// edit — `similarity("exodus", "Exodus")` = 0.833 instead of 1.0 — which
/// quietly dropped several books' chapter-only confidence under the 0.75
/// auto-stage floor ("exodus 5" became a confirm card instead of staging).
/// The static alias strings stay capitalized for display; this is the
/// matching view.
pub fn aliases_lower() -> &'static [(u8, &'static str)] {
    use std::sync::OnceLock;
    static LOWER: OnceLock<Vec<(u8, &'static str)>> = OnceLock::new();
    LOWER.get_or_init(|| {
        let mut v: Vec<(u8, &'static str)> = Vec::with_capacity(512);
        for (bi, b) in BOOKS.iter().enumerate() {
            for a in b.aliases {
                if a.chars().all(|c| c.is_ascii_lowercase() || c == ' ') {
                    v.push((bi as u8, a));
                } else {
                    // one leaked lowercase copy per capitalized alias,
                    // computed once, lives for the process — the app is a
                    // long-running desktop program, never a request handler
                    let lowered: &'static str =
                        Box::leak(a.to_ascii_lowercase().into_boxed_str());
                    v.push((bi as u8, lowered));
                }
            }
        }
        v
    })
}

/// Normalized similarity in [0,1]: 1 - levenshtein/max_len.
pub fn similarity(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut prev: Vec<u16> = (0..=b.len() as u16).collect();
    let mut cur = vec![0u16; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i as u16;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    let dist = prev[b.len()] as f32;
    1.0 - dist / a.len().max(b.len()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixty_six_books() {
        assert_eq!(BOOKS.len(), 66);
        // canonical names must be unique
        let mut names: Vec<&str> = BOOKS.iter().map(|b| b.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 66);
    }

    #[test]
    fn aliases_lower_covers_every_book() {
        // v0.853.0: the lowercase matching view must exist for all 66 books,
        // and the canonical names must be present in it — the case-sensitive
        // comparison used to charge "exodus" an edit against "Exodus" and
        // quietly push chapter-only confidence below the auto-stage floor.
        let lower = aliases_lower();
        assert_eq!(lower.len(), BOOKS.iter().map(|b| b.aliases.len()).sum::<usize>());
        for (bi, b) in BOOKS.iter().enumerate() {
            assert!(
                lower.iter().any(|(i, a)| *i as usize == bi && *a == b.name.to_ascii_lowercase()),
                "canonical name missing (lowercased) for book {}: {:?}",
                bi,
                b.name
            );
        }
    }

    #[test]
    fn similarity_basics() {
        assert!(similarity("john", "john") > 0.999);
        assert!(similarity("jhn", "john") > 0.7);
        assert!(similarity("mark", "luke") < 0.5);
    }
}
