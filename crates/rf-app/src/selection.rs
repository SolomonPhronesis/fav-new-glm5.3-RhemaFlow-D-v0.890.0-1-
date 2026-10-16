//! Selecting files in the MEDIA bar (v0.844.0).
//!
//! The rules are plain functions with tests; `present.rs` only draws and calls them.
//!
//! * plain click          -> that one file is selected (and shown on screen, as it always was)
//! * Cmd+click            -> add / remove that file from the selection (nothing is shown)
//! * Shift+click          -> everything between the last click and this one (nothing is shown)
//! * drag a box           -> everything the box touches (hold Cmd to add to what is selected)
//! * Cmd+A                -> everything in the folder you are looking at
//! * Esc                  -> select nothing

use std::collections::HashSet;

/// What a click on a file means, from the keys held at that moment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClickKind {
    /// select only this one, and show it on screen
    Plain,
    /// add / remove this one
    Toggle,
    /// extend from the anchor to this one
    Range,
}

pub fn click_kind(command: bool, shift: bool) -> ClickKind {
    if command {
        ClickKind::Toggle
    } else if shift {
        ClickKind::Range
    } else {
        ClickKind::Plain
    }
}

/// Everything between `anchor` and `to` in the list as it is on screen (either direction).
/// With no anchor, or an anchor that is not in the list any more, only `to`.
pub fn range_between(ids: &[u64], anchor: Option<u64>, to: u64) -> Vec<u64> {
    let (Some(a), Some(b)) = (anchor.and_then(|a| ids.iter().position(|x| *x == a)), ids.iter().position(|x| *x == to)) else {
        return vec![to];
    };
    let (lo, hi) = (a.min(b), a.max(b));
    ids[lo..=hi].to_vec()
}

/// Apply one click to the selection. Returns the new anchor.
pub fn apply_click(sel: &mut HashSet<u64>, ids: &[u64], anchor: Option<u64>, clicked: u64, kind: ClickKind) -> Option<u64> {
    match kind {
        ClickKind::Plain => {
            sel.clear();
            sel.insert(clicked);
            Some(clicked)
        }
        ClickKind::Toggle => {
            if !sel.remove(&clicked) {
                sel.insert(clicked);
            }
            Some(clicked)
        }
        ClickKind::Range => {
            // a range replaces the selection, like Finder; the anchor stays where it was
            sel.clear();
            sel.extend(range_between(ids, anchor, clicked));
            Some(anchor.filter(|a| ids.contains(a)).unwrap_or(clicked))
        }
    }
}

/// Which items does a dragged box touch? `rects` are (id, left, top, right, bottom) in one coordinate space.
pub fn box_hits(rects: &[(u64, [f32; 4])], a: [f32; 2], b: [f32; 2]) -> Vec<u64> {
    let (l, r) = (a[0].min(b[0]), a[0].max(b[0]));
    let (t, bt) = (a[1].min(b[1]), a[1].max(b[1]));
    rects.iter().filter(|(_, q)| q[0] <= r && q[2] >= l && q[1] <= bt && q[3] >= t).map(|(id, _)| *id).collect()
}

/// A box smaller than this is a click on empty space, not a drag.
pub const MIN_BOX: f32 = 4.0;

pub fn is_real_drag(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() >= MIN_BOX || (a[1] - b[1]).abs() >= MIN_BOX
}

/// The selection once some files are gone from the list (removed, or hidden by a search).
pub fn keep_existing(sel: &mut HashSet<u64>, exists: impl Fn(u64) -> bool) {
    sel.retain(|id| exists(*id));
}

/// Move the keyboard cursor by `delta` places, staying inside the list. `None` cursor starts at the first.
pub fn step_cursor(ids: &[u64], cursor: Option<u64>, delta: i64) -> Option<u64> {
    if ids.is_empty() {
        return None;
    }
    let cur = cursor.and_then(|c| ids.iter().position(|x| *x == c));
    let next = match cur {
        None => 0,
        Some(c) => (c as i64 + delta).clamp(0, ids.len() as i64 - 1) as usize,
    };
    Some(ids[next])
}

/// "3 selected" / "1 selected": the words shown in the bar.
pub fn count_text(n: usize) -> String {
    format!("{n} selected")
}

/// What a paste of text means. Files come first (checked by the caller); this decides between a song and nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum PastedText {
    Empty,
    /// several lines of words: a song
    Song,
    /// one line: not enough to make a song of
    OneLine,
}

pub fn classify_pasted_text(text: &str) -> PastedText {
    let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
    match lines {
        0 => PastedText::Empty,
        1 => PastedText::OneLine,
        _ => PastedText::Song,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDS: [u64; 6] = [10, 11, 12, 13, 14, 15];

    #[test]
    fn keys_decide_what_a_click_means() {
        assert_eq!(click_kind(false, false), ClickKind::Plain);
        assert_eq!(click_kind(true, false), ClickKind::Toggle);
        assert_eq!(click_kind(false, true), ClickKind::Range);
        assert_eq!(click_kind(true, true), ClickKind::Toggle, "Cmd wins, like Finder");
    }

    #[test]
    fn a_plain_click_selects_only_that_file() {
        let mut sel: HashSet<u64> = [10, 11].into();
        let anchor = apply_click(&mut sel, &IDS, None, 13, ClickKind::Plain);
        assert_eq!(sel, HashSet::from([13]));
        assert_eq!(anchor, Some(13));
    }

    #[test]
    fn cmd_click_adds_and_removes_one_file() {
        let mut sel: HashSet<u64> = [10].into();
        apply_click(&mut sel, &IDS, Some(10), 12, ClickKind::Toggle);
        assert_eq!(sel, HashSet::from([10, 12]));
        apply_click(&mut sel, &IDS, Some(12), 10, ClickKind::Toggle);
        assert_eq!(sel, HashSet::from([12]));
    }

    #[test]
    fn shift_click_selects_the_run_between_in_either_direction_and_keeps_the_anchor() {
        let mut sel: HashSet<u64> = HashSet::new();
        let anchor = apply_click(&mut sel, &IDS, Some(11), 14, ClickKind::Range);
        assert_eq!(sel, HashSet::from([11, 12, 13, 14]));
        assert_eq!(anchor, Some(11));
        let anchor = apply_click(&mut sel, &IDS, anchor, 10, ClickKind::Range);
        assert_eq!(sel, HashSet::from([10, 11]));
        assert_eq!(anchor, Some(11));
    }

    #[test]
    fn shift_click_with_no_anchor_selects_just_that_file() {
        let mut sel: HashSet<u64> = HashSet::new();
        assert_eq!(apply_click(&mut sel, &IDS, None, 12, ClickKind::Range), Some(12));
        assert_eq!(sel, HashSet::from([12]));
        // an anchor that was removed or filtered away behaves the same
        assert_eq!(range_between(&IDS, Some(99), 12), vec![12]);
    }

    #[test]
    fn a_dragged_box_selects_what_it_touches() {
        let rects = vec![(1, [0.0, 0.0, 50.0, 30.0]), (2, [60.0, 0.0, 110.0, 30.0]), (3, [0.0, 40.0, 50.0, 70.0]), (4, [60.0, 40.0, 110.0, 70.0])];
        // top-left to the middle of item 2 and nothing below
        assert_eq!(box_hits(&rects, [5.0, 5.0], [80.0, 20.0]), vec![1, 2]);
        // dragged backwards (bottom-right to top-left) gives the same answer
        assert_eq!(box_hits(&rects, [80.0, 20.0], [5.0, 5.0]), vec![1, 2]);
        // a box that touches the corner of one item still counts
        assert_eq!(box_hits(&rects, [49.0, 29.0], [55.0, 35.0]), vec![1]);
        // empty space
        assert!(box_hits(&rects, [200.0, 200.0], [300.0, 300.0]).is_empty());
        // everything
        assert_eq!(box_hits(&rects, [-10.0, -10.0], [500.0, 500.0]).len(), 4);
    }

    #[test]
    fn a_tiny_box_is_a_click_not_a_drag() {
        assert!(!is_real_drag([10.0, 10.0], [12.0, 12.0]));
        assert!(is_real_drag([10.0, 10.0], [20.0, 10.0]));
        assert!(is_real_drag([10.0, 10.0], [10.0, 30.0]));
    }

    #[test]
    fn removed_files_leave_the_selection() {
        let mut sel: HashSet<u64> = [10, 11, 12].into();
        keep_existing(&mut sel, |id| id != 11);
        assert_eq!(sel, HashSet::from([10, 12]));
    }

    #[test]
    fn the_cursor_steps_and_stops_at_the_ends() {
        assert_eq!(step_cursor(&IDS, None, 1), Some(10));
        assert_eq!(step_cursor(&IDS, Some(10), 1), Some(11));
        assert_eq!(step_cursor(&IDS, Some(15), 1), Some(15));
        assert_eq!(step_cursor(&IDS, Some(11), -3), Some(10));
        assert_eq!(step_cursor(&IDS, Some(11), 3), Some(14));
        assert_eq!(step_cursor(&[], Some(11), 1), None);
        // a cursor that is no longer listed starts again at the first
        assert_eq!(step_cursor(&IDS, Some(99), 1), Some(10));
    }

    #[test]
    fn pasted_text_is_a_song_only_when_it_has_several_lines() {
        assert_eq!(classify_pasted_text(""), PastedText::Empty);
        assert_eq!(classify_pasted_text("  \n \n"), PastedText::Empty);
        assert_eq!(classify_pasted_text("John 3:16"), PastedText::OneLine);
        assert_eq!(classify_pasted_text("Amazing grace\nhow sweet the sound"), PastedText::Song);
        assert_eq!(count_text(3), "3 selected");
    }
}
