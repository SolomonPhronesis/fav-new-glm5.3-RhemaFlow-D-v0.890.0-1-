//! Feature 10: walk-in and offering music. Shuffle that does not repeat a song until all have played, repeat modes,
//! crossfades, "fill the next 8 minutes", and "what is playing and what is next".

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub path: String,
    pub title: String,
    pub dur_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Repeat {
    Off,
    All,
    One,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub tracks: Vec<Track>,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub crossfade_ms: u32,
}

/// One entry of the timetable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub track: usize,
    pub start_ms: u64,
    pub end_ms: u64,
    /// Fade this one out over its last `fade_out_ms` (the crossfade into the next).
    pub fade_out_ms: u32,
    pub fade_in_ms: u32,
}

/// A small, steady random number generator (so a shuffle can be repeated from its seed in tests).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn shuffled(n: usize, rng: &mut Rng) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        v.swap(i, rng.below(i + 1));
    }
    v
}

impl Playlist {
    pub fn new(name: &str) -> Playlist {
        Playlist { name: name.to_string(), tracks: Vec::new(), shuffle: false, repeat: Repeat::All, crossfade_ms: 0 }
    }
    pub fn total_ms(&self) -> u64 {
        self.tracks.iter().map(|t| t.dur_ms).sum()
    }

    /// One pass through the list: in order, or shuffled.
    pub fn one_pass(&self, seed: u64) -> Vec<usize> {
        if self.shuffle {
            shuffled(self.tracks.len(), &mut Rng(seed ^ 0x9E37_79B9_7F4A_7C15))
        } else {
            (0..self.tracks.len()).collect()
        }
    }

    /// The order to play until `limit` tracks have been listed (repeat All keeps going; Off stops after one pass; One repeats the first).
    /// With shuffle, every song plays once before any plays again, and the last song of one round is never the first of the next.
    pub fn order(&self, seed: u64, limit: usize) -> Vec<usize> {
        let n = self.tracks.len();
        if n == 0 || limit == 0 {
            return Vec::new();
        }
        match self.repeat {
            Repeat::One => {
                let first = self.one_pass(seed)[0];
                vec![first; limit]
            }
            Repeat::Off => self.one_pass(seed).into_iter().take(limit).collect(),
            Repeat::All => {
                let mut out: Vec<usize> = Vec::new();
                let mut round = 0u64;
                while out.len() < limit {
                    let mut pass = if self.shuffle { shuffled(n, &mut Rng(seed.wrapping_add(round.wrapping_mul(0x1234_5678_9ABC_DEF1)) ^ 0x9E37_79B9_7F4A_7C15)) } else { (0..n).collect() };
                    if self.shuffle && n > 1 {
                        if let Some(last) = out.last() {
                            if pass[0] == *last {
                                pass.swap(0, 1);
                            }
                        }
                    }
                    out.extend(pass);
                    round += 1;
                }
                out.truncate(limit);
                out
            }
        }
    }

    /// Start times with crossfades: each song starts `crossfade` before the one before it ends (never more than half of the shorter song).
    pub fn schedule(&self, order: &[usize]) -> Vec<Slot> {
        let mut out: Vec<Slot> = Vec::new();
        let mut t = 0u64;
        for (k, &i) in order.iter().enumerate() {
            let Some(tr) = self.tracks.get(i) else { continue };
            let mut fade_in = 0u32;
            if let Some(prev) = out.last_mut() {
                let prev_len = prev.end_ms - prev.start_ms;
                let f = (self.crossfade_ms as u64).min(prev_len / 2).min(tr.dur_ms / 2) as u32;
                prev.fade_out_ms = f;
                fade_in = f;
                t = prev.end_ms - f as u64;
            }
            let _ = k;
            out.push(Slot { track: i, start_ms: t, end_ms: t + tr.dur_ms, fade_out_ms: 0, fade_in_ms: fade_in });
            t += tr.dur_ms;
        }
        out
    }

    /// Music for a gap of `target_ms` (the walk-in while people sit down). Plays whole songs; the last one is faded early
    /// so it ends exactly on time. Returns the slots and the length of the last fade.
    pub fn fill(&self, target_ms: u64, seed: u64) -> Vec<Slot> {
        if self.tracks.is_empty() || target_ms == 0 || self.total_ms() == 0 {
            return Vec::new();
        }
        let mut pl = self.clone();
        pl.repeat = Repeat::All;
        let max = (target_ms / self.tracks.iter().map(|t| t.dur_ms.max(1)).min().unwrap_or(1)) as usize + 3;
        let order = pl.order(seed, max.min(10_000));
        let mut slots = pl.schedule(&order);
        let mut keep = Vec::new();
        for s in slots.drain(..) {
            if s.start_ms >= target_ms {
                break;
            }
            keep.push(s);
        }
        if let Some(last) = keep.last_mut() {
            if last.end_ms > target_ms {
                // end early with a 3 second fade (or half the song if it is short)
                let len = target_ms - last.start_ms;
                last.end_ms = target_ms;
                last.fade_out_ms = 3000.min(len as u32 / 2);
            }
        }
        keep
    }

    /// What is playing at `elapsed_ms`, and what follows.
    pub fn at(&self, slots: &[Slot], elapsed_ms: u64) -> (Option<usize>, Option<usize>) {
        let now = slots.iter().position(|s| elapsed_ms >= s.start_ms && elapsed_ms < s.end_ms);
        match now {
            Some(i) => (Some(slots[i].track), slots.get(i + 1).map(|s| s.track)),
            None => (None, slots.iter().find(|s| s.start_ms > elapsed_ms).map(|s| s.track)),
        }
    }
}

pub fn clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pl(durs: &[u64]) -> Playlist {
        let mut p = Playlist::new("walk-in");
        for (i, d) in durs.iter().enumerate() {
            p.tracks.push(Track { path: format!("t{i}.mp3"), title: format!("Track {i}"), dur_ms: *d });
        }
        p
    }

    #[test]
    fn in_order_repeat_all_cycles() {
        let p = pl(&[1000, 1000, 1000]);
        assert_eq!(p.order(1, 7), vec![0, 1, 2, 0, 1, 2, 0]);
    }

    #[test]
    fn repeat_off_plays_once_and_repeat_one_repeats() {
        let mut p = pl(&[1000, 1000, 1000]);
        p.repeat = Repeat::Off;
        assert_eq!(p.order(1, 10), vec![0, 1, 2]);
        p.repeat = Repeat::One;
        assert_eq!(p.order(1, 3), vec![0, 0, 0]);
        assert!(pl(&[]).order(1, 5).is_empty());
    }

    #[test]
    fn shuffle_plays_every_song_before_any_repeats_and_never_back_to_back() {
        let mut p = pl(&[1000; 6]);
        p.shuffle = true;
        for seed in 0..40u64 {
            let o = p.order(seed, 24);
            for round in o.chunks(6) {
                let mut r = round.to_vec();
                r.sort();
                assert_eq!(r, vec![0, 1, 2, 3, 4, 5], "seed {seed}");
            }
            assert!(o.windows(2).all(|w| w[0] != w[1]), "seed {seed}: {o:?}");
        }
        assert_ne!(p.order(1, 6), p.order(2, 6));
        assert_eq!(p.order(5, 12), p.order(5, 12)); // same seed, same order
    }

    #[test]
    fn crossfade_overlaps_but_never_more_than_half_a_song() {
        let mut p = pl(&[10_000, 10_000, 1_000]);
        p.crossfade_ms = 4000;
        let s = p.schedule(&[0, 1, 2]);
        assert_eq!(s[1].start_ms, 6000);
        assert_eq!(s[0].fade_out_ms, 4000);
        assert_eq!(s[1].fade_in_ms, 4000);
        assert_eq!(s[2].fade_in_ms, 500); // the 1 s song only allows half
        assert_eq!(s[2].start_ms, s[1].end_ms - 500);
        assert!(s.windows(2).all(|w| w[1].start_ms >= w[0].start_ms));
    }

    #[test]
    fn no_crossfade_means_back_to_back() {
        let p = pl(&[3000, 2000]);
        let s = p.schedule(&[0, 1]);
        assert_eq!((s[1].start_ms, s[1].end_ms), (3000, 5000));
        assert_eq!(s[0].fade_out_ms, 0);
        assert!(p.schedule(&[9]).is_empty()); // a bad index is skipped
    }

    #[test]
    fn filling_a_gap_ends_exactly_on_time() {
        let p = pl(&[200_000, 180_000, 240_000]);
        let slots = p.fill(8 * 60_000, 3);
        let last = slots.last().unwrap();
        assert_eq!(last.end_ms, 480_000);
        assert!(last.fade_out_ms > 0 && last.fade_out_ms <= 3000);
        assert!(slots.windows(2).all(|w| w[0].end_ms <= w[1].start_ms + 1));
        assert!(p.fill(0, 1).is_empty());
        assert!(pl(&[]).fill(60_000, 1).is_empty());
        // a gap shorter than one song: one song, cut short
        let short = p.fill(30_000, 1);
        assert_eq!(short.len(), 1);
        assert_eq!(short[0].end_ms, 30_000);
    }

    #[test]
    fn now_and_next_follow_the_clock() {
        let p = pl(&[1000, 2000]);
        let s = p.schedule(&[0, 1]);
        assert_eq!(p.at(&s, 500), (Some(0), Some(1)));
        assert_eq!(p.at(&s, 1500), (Some(1), None));
        assert_eq!(p.at(&s, 9999), (None, None));
        let gap = vec![Slot { track: 0, start_ms: 5000, end_ms: 6000, fade_out_ms: 0, fade_in_ms: 0 }];
        assert_eq!(p.at(&gap, 100), (None, Some(0)));
    }

    #[test]
    fn totals_and_clock_text() {
        assert_eq!(pl(&[60_000, 90_500]).total_ms(), 150_500);
        assert_eq!(clock(150_500), "2:30");
    }
}
