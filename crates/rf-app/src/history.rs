//! The list of scriptures shown so far, in order (v0.814.0).
//!
//! "Previous scripture" walks BACK through it one step at a time (and again, and again);
//! "next scripture" walks forward again. Like the back button in a browser, except nothing
//! is ever thrown away: showing something new always goes on the end of the list.
//!
//! "Next verse / previous verse" would flood the list (every press is a new verse), so while
//! you are reading along inside the same chapter at the newest entry, those steps just move
//! that entry on instead of adding a new one.

use rf_core::Passage;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub passage: Passage,
    pub label: String,
    /// milliseconds into the service when it was shown
    pub ms: u64,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct ScriptureHistory {
    entries: Vec<Entry>,
    /// index of the entry on screen now (meaningless while empty)
    pos: usize,
    cap: usize,
}

impl Default for ScriptureHistory {
    fn default() -> Self {
        ScriptureHistory { entries: Vec::new(), pos: 0, cap: 200 }
    }
}

impl ScriptureHistory {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.pos = 0;
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn pos(&self) -> usize {
        self.pos
    }
    pub fn current(&self) -> Option<&Entry> {
        self.entries.get(self.pos)
    }
    pub fn can_back(&self) -> bool {
        !self.entries.is_empty() && self.pos > 0
    }
    pub fn can_forward(&self) -> bool {
        !self.entries.is_empty() && self.pos + 1 < self.entries.len()
    }

    /// Something was shown by voice, card, click or plan. `nav` = it came from next/previous
    /// verse or chapter, which only moves the newest entry when still in the same chapter.
    pub fn push(&mut self, passage: Passage, label: String, ms: u64, source: &str, nav: bool) {
        let at_head = self.entries.is_empty() || self.pos + 1 == self.entries.len();
        if let Some(last) = self.entries.last_mut() {
            if at_head && last.passage == passage {
                self.pos = self.entries.len() - 1;
                return; // the same thing again
            }
            if nav && at_head && last.passage.book == passage.book && last.passage.chapter == passage.chapter {
                last.passage = passage;
                last.label = label;
                last.source = source.to_string();
                return; // reading along: move the entry, do not add one
            }
        }
        self.entries.push(Entry { passage, label, ms, source: source.to_string() });
        if self.entries.len() > self.cap {
            self.entries.remove(0);
        }
        self.pos = self.entries.len() - 1;
    }

    /// One step back. Returns the entry to show, or None at the beginning.
    pub fn back(&mut self) -> Option<Entry> {
        if self.can_back() {
            self.pos -= 1;
            return self.current().cloned();
        }
        None
    }

    /// One step forward (only after going back).
    pub fn forward(&mut self) -> Option<Entry> {
        if self.can_forward() {
            self.pos += 1;
            return self.current().cloned();
        }
        None
    }

    /// Jump straight to an entry (clicking it in the list).
    pub fn jump(&mut self, i: usize) -> Option<Entry> {
        if i < self.entries.len() {
            self.pos = i;
            return self.current().cloned();
        }
        None
    }

    /// "3 of 7"
    pub fn position_label(&self) -> String {
        if self.entries.is_empty() {
            "empty".to_string()
        } else {
            format!("{} of {}", self.pos + 1, self.entries.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(book: u8, chapter: u16, verse: u16) -> Passage {
        Passage { book, chapter, from: verse, to: verse }
    }
    fn names(h: &ScriptureHistory) -> Vec<String> {
        h.entries().iter().map(|e| e.label.clone()).collect()
    }
    fn push(h: &mut ScriptureHistory, pp: Passage, nav: bool) {
        h.push(pp, pp.label(), 0, if nav { "nav" } else { "voice" }, nav);
    }

    #[test]
    fn previous_scripture_walks_back_one_at_a_time_and_forward_again() {
        let mut h = ScriptureHistory::default();
        push(&mut h, p(42, 3, 16), false); // John 3:16
        push(&mut h, p(44, 8, 28), false); // Romans 8:28
        push(&mut h, p(18, 23, 1), false); // Psalms 23:1
        assert_eq!(h.back().unwrap().label, "Romans 8:28");
        assert_eq!(h.back().unwrap().label, "John 3:16", "and the one before that");
        assert!(h.back().is_none(), "start of the list");
        assert_eq!(h.forward().unwrap().label, "Romans 8:28");
        assert_eq!(h.forward().unwrap().label, "Psalms 23:1");
        assert!(h.forward().is_none());
        assert_eq!(h.position_label(), "3 of 3");
    }

    #[test]
    fn showing_something_new_goes_on_the_end_and_nothing_is_lost() {
        let mut h = ScriptureHistory::default();
        push(&mut h, p(42, 3, 16), false);
        push(&mut h, p(44, 8, 28), false);
        push(&mut h, p(18, 23, 1), false);
        h.back();
        h.back(); // now on John 3:16
        push(&mut h, p(65, 1, 6), false); // Revelation 1:6
        assert_eq!(names(&h), vec!["John 3:16", "Romans 8:28", "Psalms 23:1", "Revelation 1:6"]);
        assert_eq!(h.pos(), 3);
        assert_eq!(h.back().unwrap().label, "Psalms 23:1", "back from the newest goes to what was shown just before");
    }

    #[test]
    fn next_verse_while_reading_along_moves_one_entry_instead_of_flooding() {
        let mut h = ScriptureHistory::default();
        push(&mut h, p(42, 3, 16), false);
        for v in 17..=25 {
            push(&mut h, p(42, 3, v), true);
        }
        assert_eq!(h.len(), 1, "{:?}", names(&h));
        assert_eq!(h.current().unwrap().label, "John 3:25");
        // crossing into another chapter IS a new entry
        push(&mut h, p(42, 4, 1), true);
        assert_eq!(h.len(), 2);
        // and a spoken reference is always a new entry
        push(&mut h, p(42, 4, 5), false);
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn navigating_from_the_middle_of_the_list_adds_instead_of_overwriting() {
        let mut h = ScriptureHistory::default();
        push(&mut h, p(42, 3, 16), false);
        push(&mut h, p(44, 8, 28), false);
        h.back(); // John 3:16, not the newest
        push(&mut h, p(42, 3, 17), true); // next verse from here
        assert_eq!(names(&h), vec!["John 3:16", "Romans 8:28", "John 3:17"]);
    }

    #[test]
    fn the_same_scripture_twice_in_a_row_is_one_entry_and_jump_works() {
        let mut h = ScriptureHistory::default();
        push(&mut h, p(42, 3, 16), false);
        push(&mut h, p(42, 3, 16), false);
        assert_eq!(h.len(), 1);
        push(&mut h, p(44, 8, 28), false);
        push(&mut h, p(18, 23, 1), false);
        assert_eq!(h.jump(0).unwrap().label, "John 3:16");
        assert_eq!(h.position_label(), "1 of 3");
        assert!(h.jump(9).is_none());
        assert!(!h.can_back() && h.can_forward());
    }

    #[test]
    fn empty_history_is_safe() {
        let mut h = ScriptureHistory::default();
        assert!(h.back().is_none() && h.forward().is_none() && h.current().is_none());
        assert_eq!(h.position_label(), "empty");
    }

    #[test]
    fn the_list_is_capped() {
        let mut h = ScriptureHistory::default();
        for v in 1..=300u16 {
            push(&mut h, p(0, 1 + v % 40, 1 + v % 7), false);
            push(&mut h, p(1, 1 + v % 40, 1 + (v + 3) % 7), false);
        }
        assert!(h.len() <= 200);
        assert_eq!(h.pos(), h.len() - 1);
    }
}
