//! Feature 19: preview, then Take. In "safe mode" a click only puts something in the preview; nothing reaches the
//! congregation until the operator presses Take. Outside safe mode a click goes straight out. Undo returns to what was
//! showing before. Works for anything: slides, songs, verses.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Put in the preview, not on the screen.
    Previewed,
    /// On the screen now.
    Live,
    /// Nothing to do.
    Nothing,
}

#[derive(Clone, Debug)]
pub struct Staging<T: Clone + PartialEq> {
    preview: Option<T>,
    live: Option<T>,
    history: Vec<Option<T>>,
    pub safe_mode: bool,
}

const MAX_HISTORY: usize = 50;

impl<T: Clone + PartialEq> Staging<T> {
    pub fn new(safe_mode: bool) -> Staging<T> {
        Staging { preview: None, live: None, history: Vec::new(), safe_mode }
    }
    pub fn preview(&self) -> Option<&T> {
        self.preview.as_ref()
    }
    pub fn live(&self) -> Option<&T> {
        self.live.as_ref()
    }
    /// Is something waiting in the preview that is not what is live?
    pub fn is_pending(&self) -> bool {
        self.preview.is_some() && self.preview != self.live
    }

    fn put_live(&mut self, v: Option<T>) {
        if self.history.len() >= MAX_HISTORY {
            self.history.remove(0);
        }
        self.history.push(self.live.take());
        self.live = v;
    }

    /// The operator picked something.
    pub fn select(&mut self, item: T) -> Outcome {
        if self.safe_mode {
            self.preview = Some(item);
            Outcome::Previewed
        } else {
            if self.live.as_ref() == Some(&item) {
                return Outcome::Nothing;
            }
            self.put_live(Some(item));
            self.preview = None;
            Outcome::Live
        }
    }

    /// The Take button. Sends the preview out; the preview empties so it cannot be taken twice by accident.
    pub fn take(&mut self) -> Outcome {
        match self.preview.take() {
            Some(p) => {
                if self.live.as_ref() == Some(&p) {
                    return Outcome::Nothing;
                }
                self.put_live(Some(p));
                Outcome::Live
            }
            None => Outcome::Nothing,
        }
    }

    /// Drop the preview without sending it.
    pub fn cancel(&mut self) -> bool {
        self.preview.take().is_some()
    }

    /// Blank the screen (clear / black). Can be undone.
    pub fn blank(&mut self) -> bool {
        if self.live.is_none() {
            return false;
        }
        self.put_live(None);
        true
    }

    /// Go back to what was on the screen before the last change.
    pub fn undo(&mut self) -> Option<Option<&T>> {
        let prev = self.history.pop()?;
        self.live = prev;
        Some(self.live.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_safe_mode_a_click_goes_straight_out() {
        let mut s = Staging::new(false);
        assert_eq!(s.select("A"), Outcome::Live);
        assert_eq!(s.live(), Some(&"A"));
        assert_eq!(s.select("A"), Outcome::Nothing);
        assert_eq!(s.take(), Outcome::Nothing);
    }

    #[test]
    fn in_safe_mode_nothing_goes_out_until_take() {
        let mut s = Staging::new(true);
        s.select("A");
        s.take();
        assert_eq!(s.select("B"), Outcome::Previewed);
        assert_eq!(s.live(), Some(&"A"));
        assert!(s.is_pending());
        assert_eq!(s.take(), Outcome::Live);
        assert_eq!(s.live(), Some(&"B"));
        assert!(!s.is_pending());
        assert_eq!(s.take(), Outcome::Nothing, "a second Take does nothing");
    }

    #[test]
    fn picking_again_replaces_the_preview_and_cancel_drops_it() {
        let mut s = Staging::new(true);
        s.select(1);
        s.select(2);
        assert_eq!(s.preview(), Some(&2));
        assert!(s.cancel());
        assert!(!s.cancel());
        assert_eq!(s.take(), Outcome::Nothing);
        assert_eq!(s.live(), None);
    }

    #[test]
    fn taking_what_is_already_live_does_nothing() {
        let mut s = Staging::new(true);
        s.select("A");
        s.take();
        s.select("A");
        assert!(!s.is_pending());
        assert_eq!(s.take(), Outcome::Nothing);
    }

    #[test]
    fn undo_steps_back_through_changes_including_a_blank() {
        let mut s = Staging::new(false);
        s.select("A");
        s.select("B");
        assert!(s.blank());
        assert!(!s.blank());
        assert_eq!(s.undo(), Some(Some(&"B")));
        assert_eq!(s.undo(), Some(Some(&"A")));
        assert_eq!(s.undo(), Some(None)); // before A there was nothing
        assert_eq!(s.undo(), None);
    }

    #[test]
    fn history_is_limited() {
        let mut s = Staging::new(false);
        for i in 0..200 {
            s.select(i);
        }
        let mut n = 0;
        while s.undo().is_some() {
            n += 1;
        }
        assert_eq!(n, MAX_HISTORY);
    }
}
