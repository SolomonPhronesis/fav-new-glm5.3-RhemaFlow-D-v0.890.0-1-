//! rf-studio: the "replace the big presentation programs" feature set.
//!
//! Everything here is plain functions and plain data, written from the behaviour a church needs
//! (our own words and our own code; nothing is taken from any other program). No window, no
//! sound card, no clock: callers pass the time in, so every rule is tested without a screen.
//!
//! | # | Module | What it gives the operator |
//! |---|--------|----------------------------|
//! | 1 | [`import`] | Songs from OpenLyrics, OpenSong, ChordPro and plain text files |
//! | 2 | [`chords`] | Chord charts: transpose, capo, Nashville numbers, key detection |
//! | 3 | [`parallel`] | Two or three Bible versions on one slide |
//! | 4 | [`fit`] | Auto-split and balanced line breaks, best font size for a box |
//! | 5 | [`transition`] | Cut, fade, dip, push, wipe, zoom (per-frame maths) |
//! | 6 | [`layout`] | Slide templates with several boxes, and the editor rules |
//! | 7 | [`vars`] | `{date}`, `{time}`, `{countdown:Name}`... filled in at show time |
//! | 8 | [`macros`] | One button runs a script: wait, go to, black, sound, alert |
//! | 9 | [`cues`] | Slides that advance by themselves, loop or ping-pong |
//! | 10 | [`playlist`] | Walk-in music: shuffle, repeat, crossfade, fill N minutes |
//! | 11 | [`mediaset`] | Trim, loop, speed, fit mode, colour filters, fade curves |
//! | 12 | [`stagemon`] | Confidence-monitor layouts for the preacher, band and tech |
//! | 13 | [`alerts`] | Nursery calls and messages with expiry and a ticker |
//! | 14 | [`teleprompter`] | Sermon notes that scroll at the speaker's pace |
//! | 15 | [`library_tools`] | Tags, usage, duplicate finder, merge, find and replace |
//! | 16 | [`servicetpl`] | Service templates and "copy last Sunday" |
//! | 17 | [`hotkeys`] | Your own shortcuts, with clash checks |
//! | 18 | [`goto`] | "chorus", "v2", "+2", "last" jump targets and group colours |
//! | 19 | [`staging`] | Preview-then-Take (safe mode) with undo |
//! | 20 | [`bilingual`] | Lyrics with a translation line under each line |

pub mod alerts;
pub mod bilingual;
pub mod chords;
pub mod cues;
pub mod fit;
pub mod goto;
pub mod hotkeys;
pub mod import;
pub mod layout;
pub mod library_tools;
pub mod macros;
pub mod mediaset;
pub mod parallel;
pub mod playlist;
pub mod servicetpl;
pub mod stagemon;
pub mod staging;
pub mod teleprompter;
pub mod transition;
pub mod vars;
pub mod xml;
