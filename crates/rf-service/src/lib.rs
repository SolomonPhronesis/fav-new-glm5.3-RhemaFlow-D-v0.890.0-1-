//! rf-service — the presentation foundation (roadmap Phase 1).
//!
//! Pure logic, no UI, no audio: a **Service** (ordered list of items), the
//! **Runner** that decides what the congregation sees and can never leave the
//! operator stranded (Clear / Logo / Black panic keys, undo, edit-vs-show
//! lock), and a **crash-safe store** (atomic writes, rolling backups,
//! schema versioning, corrupt-file recovery, debounced autosave).
//!
//! Stage 2 (v0.815.0) adds: `slides` (text and .pptx to slides), `songs` (library, arrangement,
//! search), `timer` (countdowns, stopwatch, clock) and `media` (picture/video rules, fit maths,
//! a video's own clock, background darkening).
//!
//! Kept free of egui on purpose: everything here is unit-testable on any
//! machine, and the operator UI and the audience renderer both sit on top of
//! it (see roadmap ADR-001 — render stack).

pub mod library;
pub mod media;
pub mod model;
pub mod runner;
pub mod slides;
pub mod songs;
pub mod store;
pub mod timer;

pub use model::{Align, Item, ItemKind, Service, Slide, Theme, SCHEMA_VERSION};
pub use runner::{Mode, Output, RunError, Runner};
pub use store::{load, save, Autosave, LoadOutcome, StoreError};
