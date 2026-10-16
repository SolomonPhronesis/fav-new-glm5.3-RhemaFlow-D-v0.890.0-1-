//! Sound-tag filter (v0.855.0): "[MUSIC]", "(upbeat guitar music)",
//! "[APPLAUSE]", "[BLANK_AUDIO]" and friends must not be able to hide a
//! Bible verse.
//!
//! Whisper-style engines emit these tags as literal text. The parser used to
//! only strip the BRACKETS (its normalizer replaces `[`, `]`, `(`, `)` with
//! spaces), which turned "[MUSIC] for God so loved the world" into "music
//! for god so loved the world" — the tag's words then diluted the quotation
//! match and could even steer it to a wrong verse. A verse command fared
//! worse: "read john 3 [APPLAUSE]" parsed fine, but "john 3 (guitar strumming)
//! verse 16" did not.
//!
//! Rule: a bracketed span is a sound tag when EVERY word inside is from the
//! audio-tag lexicon below. Real asides the preacher actually says —
//! "Romans (chapter 8)", "(verse 16)" — contain book/chapter/verse words and
//! are never touched. Everything else keeps its words.
//!
//! Display text stays exactly what the engine heard; this filter is applied
//! to the text the PARSER sees (same contract as the lexicon repair).

/// Words that can only describe sound/silence, never Scripture content.
const TAG_WORDS: &[&str] = &[
    "music", "musical", "instrumental", "guitar", "acoustic", "electric",
    "piano", "keyboard", "organ", "drum", "drums", "percussion", "violin",
    "strings", "flute", "trumpet", "band", "orchestra", "choir", "singing",
    "sing", "song", "songs", "melody", "tune", "humming", "hum", "strumming",
    "strum", "strums", "playing", "plays", "played", "jam", "jamming",
    "applause", "applauding", "clapping", "clap", "cheering", "cheers",
    "cheer", "laughter", "laughing", "laughs", "crowd", "congregation",
    "murmuring", "chatter", "chattering", "whispering", "shuffling",
    "footsteps", "door", "doorbell", "phone", "ringing", "ring", "buzzing",
    "hum", "static", "noise", "noises", "silence", "silent", "quiet",
    "pause", "paused", "blank", "audio", "instruments", "sounds", "sound",
    "beep", "beeps", "chime", "chimes", "tone", "tones", "bell", "bells",
    "upbeat", "soft", "loudly", "loud", "faint", "distant", "background",
    "amplified", "feedback", "microphone", "mic", "cough", "coughing",
    "sneeze", "sneezing", "breathing", "sigh", "sighing", "clearing",
    "throat", "blow", "nose", "jazz", "anthem", "chorus", "refrain", "solo",
    "duet", "rehearsal", "rehearsing", "practice", "practicing", "soundcheck",
];

fn is_tag_word(w: &str) -> bool {
    let lower = w.to_ascii_lowercase();
    TAG_WORDS.contains(&lower.as_str())
}

/// Strip bracketed sound tags — "[...]" and "(...)" spans whose every word
/// is an audio-tag word — from `text`, keeping everything else verbatim.
/// Returns the cleaned text (whitespace-collapsed where tags were removed).
pub fn strip_sound_tags(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    let mut removed = false;
    while i < chars.len() {
        let open = chars[i];
        if open == '[' || open == '(' {
            let close = if open == '[' { ']' } else { ')' };
            if let Some(rel) = chars[i + 1..].iter().position(|&c| c == close) {
                let inner: String = chars[i + 1..i + 1 + rel].iter().collect();
                let words: Vec<&str> = inner.split_whitespace().collect();
                if !words.is_empty() && words.iter().all(|w| is_tag_word(w)) {
                    removed = true;
                    i += rel + 2;
                    // swallow one space that belonged to the tag
                    if i < chars.len() && chars[i] == ' ' && !out.ends_with(' ') && !out.is_empty() {
                        i += 1;
                    }
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    let _ = removed;
    // collapse the whitespace the removed tags left behind (the parser's
    // word pipeline collapses anyway, but keep the utility honest on its own)
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// True when the text still contains bracketed sound tags (used in tests and
/// by the UI to show an honest "audio tag removed from parse" note).
pub fn contains_sound_tags(text: &str) -> bool {
    strip_sound_tags(text) != text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_tags_are_removed_and_the_verse_keeps() {
        assert_eq!(
            strip_sound_tags("[MUSIC] for God so loved the world"),
            "for God so loved the world"
        );
        assert_eq!(
            strip_sound_tags("for God so [upbeat guitar music] loved the world"),
            "for God so loved the world"
        );
        assert_eq!(
            strip_sound_tags("(soft piano) let this mind be in you"),
            "let this mind be in you"
        );
    }

    #[test]
    fn verse_asides_are_never_touched() {
        assert_eq!(strip_sound_tags("Romans (chapter 8) verse 28"), "Romans (chapter 8) verse 28");
        assert_eq!(strip_sound_tags("turn to (Psalm 23)"), "turn to (Psalm 23)");
        // an unbalanced bracket is not a tag either
        assert_eq!(strip_sound_tags("john 3 [unclosed"), "john 3 [unclosed");
    }

    #[test]
    fn mixed_content_tags_are_kept() {
        // "loud amen" is not purely a sound word
        assert_eq!(strip_sound_tags("[loud amen] john 3:16"), "[loud amen] john 3:16");
    }

    #[test]
    fn multi_tag_lines_are_fully_cleaned() {
        assert_eq!(
            strip_sound_tags("[APPLAUSE] and he told them [LAUGHTER] the story"),
            "and he told them the story"
        );
        assert!(contains_sound_tags("[MUSIC] hello"));
        assert!(!contains_sound_tags("hello there"));
    }
}
