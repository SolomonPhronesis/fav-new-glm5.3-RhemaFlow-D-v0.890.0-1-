//! rf-core — the Scripture understanding engine.
//!
//! Pure logic, no I/O, no UI: everything here is unit-tested against the
//! acceptance criteria in PRD v2.0 §6/§7 and every locked regression
//! utterance. UI and audio layers sit on top of this crate.

pub mod books;
pub mod cmdtable;
pub mod commit;
pub mod commands;
pub mod evalset;
pub mod learn;
pub mod lexicon;
pub mod livetext;
pub mod parser;
pub mod repair;
pub mod soundtag;
pub mod stream;
pub mod wer;

pub use parser::{
    parse, normalize_for_commands, Command, Detection, DetectionKind, ParseContext, ParseOutcome,
    Passage, RecentLine, RefError, Suggestion, TRANSLATIONS, pick_recent_pieces,
};
pub use commands::is_bare_stop;
pub use commit::{clean_tail, CommitTracker, Flush};
pub use lexicon::Lexicon;
pub use livetext::{FinishReport, LiveText, LiveView, Strategy as LiveStrategy};
pub use repair::{repair_line, Repair};
pub use soundtag::{contains_sound_tags, strip_sound_tags};
pub use stream::{end_hint, Agreement, AgreementView, EndHint};
pub use wer::{normalize_words, word_error_rate, Wer};
