//! The live runner: what the congregation sees, and how the operator moves it.
//!
//! Live position is tracked by **item id**, not list index, so reordering or
//! deleting other items while live can never change what is on screen.

use crate::model::{Item, ItemKind, Service};
use serde::{Deserialize, Serialize};

const UNDO_DEPTH: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Output {
    /// Show slide `slide` of item `item_id`.
    Content { item_id: u64, slide: usize },
    /// Empty screen with the theme background.
    Clear,
    /// Church logo / holding slide.
    Logo,
    /// Pure black (projector-off look).
    Black,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Slides can be edited. The default.
    Edit,
    /// Locked for the live service: structural edits are refused.
    Show,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunError {
    /// Service is in Show mode; switch to Edit to change it.
    Locked,
    NoSuchItem(u64),
    NoSuchSlide { item_id: u64, slide: usize },
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Locked => write!(f, "service is locked (Show mode)"),
            RunError::NoSuchItem(id) => write!(f, "no item with id {id}"),
            RunError::NoSuchSlide { item_id, slide } => {
                write!(f, "item {item_id} has no slide {slide}")
            }
        }
    }
}
impl std::error::Error for RunError {}

pub struct Runner {
    service: Service,
    output: Output,
    mode: Mode,
    /// Last Content position, so "un-clear" returns to it.
    last_content: Option<(u64, usize)>,
    undo: Vec<Output>,
}

impl Runner {
    /// Starts on **Clear**. A crashed service is never auto-shown to the
    /// congregation; the operator chooses `resume()`.
    pub fn new(service: Service) -> Self {
        Runner { service, output: Output::Clear, mode: Mode::Edit, last_content: None, undo: Vec::new() }
    }

    pub fn service(&self) -> &Service {
        &self.service
    }
    pub fn output(&self) -> Output {
        self.output
    }
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// The position saved before the last shutdown/crash, if still valid.
    pub fn resume_point(&self) -> Option<Output> {
        match self.service.last_live {
            Some(Output::Content { item_id, slide }) if self.valid(item_id, slide) => {
                self.service.last_live
            }
            Some(o @ (Output::Logo | Output::Black)) => Some(o),
            _ => None,
        }
    }

    /// Re-show the saved position. Returns false if there is nothing valid.
    pub fn resume(&mut self) -> bool {
        match self.resume_point() {
            Some(o) => {
                self.apply(o);
                true
            }
            None => false,
        }
    }

    // ---- live actions (allowed in both modes: the show must always be drivable)

    pub fn go(&mut self, item_id: u64, slide: usize) -> Result<(), RunError> {
        let item = self.service.item(item_id).ok_or(RunError::NoSuchItem(item_id))?;
        if slide >= item.slide_count() {
            return Err(RunError::NoSuchSlide { item_id, slide });
        }
        self.apply(Output::Content { item_id, slide });
        Ok(())
    }

    /// Advance one slide, flowing into the next non-empty item.
    /// From Clear/Logo/Black it goes to the first slide of the service.
    /// Returns false (and changes nothing) at the very end.
    pub fn next(&mut self) -> bool {
        let target = match self.output {
            Output::Content { item_id, slide } => self.step(item_id, slide, true),
            _ => self.first_position(),
        };
        self.move_to(target)
    }

    /// Back one slide. At the very start it changes nothing.
    pub fn prev(&mut self) -> bool {
        let target = match self.output {
            Output::Content { item_id, slide } => self.step(item_id, slide, false),
            _ => self.last_content.filter(|(i, s)| self.valid(*i, *s)),
        };
        self.move_to(target)
    }

    pub fn clear(&mut self) {
        self.apply(Output::Clear);
    }
    pub fn logo(&mut self) {
        self.apply(Output::Logo);
    }
    pub fn black(&mut self) {
        self.apply(Output::Black);
    }

    /// Come back from Clear/Logo/Black to the last shown slide.
    pub fn restore_content(&mut self) -> bool {
        let t = self.last_content.filter(|(i, s)| self.valid(*i, *s));
        self.move_to(t)
    }

    /// Undo the last live change (up to 10 deep). Safe if the target vanished.
    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else { return false };
        let restored = match prev {
            Output::Content { item_id, slide } if !self.valid(item_id, slide) => Output::Clear,
            o => o,
        };
        self.set_output(restored);
        true
    }

    // ---- structural edits (Edit mode only)

    pub fn add_item(&mut self, title: &str, kind: ItemKind) -> Result<u64, RunError> {
        self.guard()?;
        Ok(self.service.add_item(title, kind))
    }

    pub fn add_text_item(&mut self, title: &str, body: &str) -> Result<u64, RunError> {
        self.guard()?;
        Ok(self.service.add_text_item(title, body))
    }

    /// Removing the item that is live clears the screen; any other removal
    /// leaves the output untouched because live state is keyed by id.
    pub fn remove_item(&mut self, item_id: u64) -> Result<(), RunError> {
        self.guard()?;
        let idx = self.service.index_of(item_id).ok_or(RunError::NoSuchItem(item_id))?;
        self.service.items.remove(idx);
        if matches!(self.output, Output::Content { item_id: live, .. } if live == item_id) {
            self.apply(Output::Clear);
        }
        if matches!(self.last_content, Some((i, _)) if i == item_id) {
            self.last_content = None;
        }
        Ok(())
    }

    pub fn move_item(&mut self, item_id: u64, new_index: usize) -> Result<(), RunError> {
        self.guard()?;
        let idx = self.service.index_of(item_id).ok_or(RunError::NoSuchItem(item_id))?;
        let item = self.service.items.remove(idx);
        let at = new_index.min(self.service.items.len());
        self.service.items.insert(at, item);
        Ok(())
    }

    pub fn rename_item(&mut self, item_id: u64, title: &str) -> Result<(), RunError> {
        self.guard()?;
        let t = title.trim();
        let item = self.service.items.iter_mut().find(|i| i.id == item_id).ok_or(RunError::NoSuchItem(item_id))?;
        if !t.is_empty() {
            item.title = t.to_string();
        }
        Ok(())
    }

    /// Operator note and planned minutes. These do not change what is on screen, so they are allowed
    /// even while the list is locked for the service.
    pub fn set_item_notes(&mut self, item_id: u64, notes: &str) -> Result<(), RunError> {
        let item = self.service.items.iter_mut().find(|i| i.id == item_id).ok_or(RunError::NoSuchItem(item_id))?;
        item.notes = notes.to_string();
        Ok(())
    }

    pub fn set_item_minutes(&mut self, item_id: u64, minutes: Option<u32>) -> Result<(), RunError> {
        let item = self.service.items.iter_mut().find(|i| i.id == item_id).ok_or(RunError::NoSuchItem(item_id))?;
        item.minutes = minutes.map(|m| m.min(600));
        Ok(())
    }

    /// Put back a whole earlier list (undo / redo of list edits). If the item that is live is gone, or
    /// its slide no longer exists, the screen is cleared rather than left pointing at nothing.
    pub fn restore_items(&mut self, items: Vec<Item>) -> Result<(), RunError> {
        self.guard()?;
        self.service.items = items;
        if let Output::Content { item_id, slide } = self.output {
            if !self.valid(item_id, slide) {
                self.apply(Output::Clear);
            }
        }
        if matches!(self.last_content, Some((i, s)) if !self.valid(i, s)) {
            self.last_content = None;
        }
        Ok(())
    }

    /// Replace the text of one slide. Cannot change slide count, so the live
    /// position stays valid.
    pub fn edit_slide_text(&mut self, item_id: u64, slide: usize, text: &str) -> Result<(), RunError> {
        self.guard()?;
        let item = self
            .service
            .items
            .iter_mut()
            .find(|i| i.id == item_id)
            .ok_or(RunError::NoSuchItem(item_id))?;
        match &mut item.kind {
            ItemKind::Slides { slides } if slide < slides.len() => {
                slides[slide].text = text.to_string();
                Ok(())
            }
            _ => Err(RunError::NoSuchSlide { item_id, slide }),
        }
    }

    // ---- internals

    fn guard(&self) -> Result<(), RunError> {
        if self.mode == Mode::Show { Err(RunError::Locked) } else { Ok(()) }
    }

    fn valid(&self, item_id: u64, slide: usize) -> bool {
        self.service.item(item_id).map_or(false, |i| slide < i.slide_count())
    }

    fn move_to(&mut self, target: Option<(u64, usize)>) -> bool {
        match target {
            Some((item_id, slide)) => {
                self.apply(Output::Content { item_id, slide });
                true
            }
            None => false,
        }
    }

    /// Record undo + change output.
    fn apply(&mut self, new: Output) {
        if new == self.output {
            return;
        }
        self.undo.push(self.output);
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.set_output(new);
    }

    fn set_output(&mut self, new: Output) {
        self.output = new;
        if let Output::Content { item_id, slide } = new {
            self.last_content = Some((item_id, slide));
        }
        self.service.last_live = Some(new);
    }

    fn first_position(&self) -> Option<(u64, usize)> {
        self.service.items.iter().find(|i| i.slide_count() > 0).map(|i| (i.id, 0))
    }

    fn step(&self, item_id: u64, slide: usize, forward: bool) -> Option<(u64, usize)> {
        let idx = self.service.index_of(item_id)?;
        let items: &[Item] = &self.service.items;
        if forward {
            if slide + 1 < items[idx].slide_count() {
                return Some((item_id, slide + 1));
            }
            items[idx + 1..].iter().find(|i| i.slide_count() > 0).map(|i| (i.id, 0))
        } else {
            if slide > 0 {
                return Some((item_id, slide - 1));
            }
            items[..idx].iter().rev().find(|i| i.slide_count() > 0).map(|i| (i.id, i.slide_count() - 1))
        }
    }
}
