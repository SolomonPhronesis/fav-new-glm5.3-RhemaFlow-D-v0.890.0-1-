//! Energy-based voice activity detector with an adaptive noise floor.
//!
//! Deliberately simple and deterministic: it must behave identically in a
//! sanctuary, a home office and a regression test. Whisper handles the rest.
//!
//! Frame = 10 ms (160 samples @ 16 kHz).

use crate::WHISPER_RATE;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

const FRAME: usize = 160; // 10 ms @ 16 kHz
const SPEECH_START_FRAMES: usize = 3; // 30 ms above threshold -> speech
const SPEECH_END_FRAMES: usize = 80; // 800 ms below threshold -> utterance end
const PREROLL_FRAMES: usize = 50; // 500 ms of pre-roll kept
const MIN_SPEECH_FRAMES: usize = 25; // 250 ms of actual speech required
const MAX_UTTERANCE_FRAMES: usize = 30 * 100; // 30 s force-flush
/// Continuous speech is cut at its quietest moment once it has run this long,
/// so a 60 s monologue becomes ~12 s pieces that each get a full transcript
/// instead of one 30 s hard cut in the middle of a word.
/// Frames of history used to tell a steady background (music, fan, hum) from speech.
const HIST_FRAMES: usize = 100;
/// A background counts as steady when the frame-to-frame variation of its level
/// (standard deviation / mean over the last second) stays below this. Measured on
/// real audio: steady fan 0.04, a worship-band chord bed 0.19, human speech ~1.0.
const STEADY_CV: f32 = 0.3;
/// ...and has stayed steady for this many extra frames (so a held vowel is not mistaken for a bed).
const STEADY_HOLD_FRAMES: usize = 100;
const MAX_FLOOR: f32 = 0.08;
/// Trailing quiet frames kept on a finished utterance (200 ms).
const TAIL_KEEP_FRAMES: usize = 20;
const SOFT_CUT_FRAMES: usize = 12 * 100;
/// ...searching only the part of the piece after this point (never a tiny first piece).
const SOFT_CUT_SEARCH_FROM: usize = 8 * 100;

// A short bare command ("next.", "read it.", "full screen.") pays the same
// 800ms trailing-silence wait as a whole scripture reference before it's
// even handed to Whisper — that's most of what made single-word commands
// feel laggy/"cut off". Scripture readers legitimately pause 500-800ms+
// between clauses ("...chapter five... verse seventeen"), so SPEECH_END_FRAMES
// can't just be lowered across the board without chopping references more
// often. Instead: an utterance that is STILL short (well under what a real
// reference takes to speak) when silence starts gets a much shorter fuse.
// Once it grows past this, it falls back to the full, safe 800ms tail.
const SHORT_UTTERANCE_SPEECH_FRAMES_CAP: usize = 150; // 1.5 s of accumulated speech
const SHORT_SPEECH_END_FRAMES: usize = 40; // 400 ms below threshold -> end (short case)

// ---- semantic endpointing (v0.806) --------------------------------------
// While the speaker is paused the ASR worker decodes what was said so far and
// asks the reference parser how long to keep waiting (rf_core::end_hint).
// A finished "John chapter three verse sixteen" needs ~350 ms; an unfinished
// "…verse" needs ~1100 ms. The VAD takes that advice within these bounds.
const HINT_MIN_FRAMES: usize = 25; // never end sooner than 250 ms of silence
const HINT_MAX_FRAMES: usize = 120; // never wait longer than 1.2 s
/// A pause this long (100 ms) triggers one speculative full-quality decode.
const PAUSE_SNAPSHOT_FRAMES: usize = 10;
/// New speech beyond this many frames after the advised snapshot makes the advice stale.
const HINT_STALE_TOLERANCE_FRAMES: usize = 5;

/// Lock-free mailbox from the ASR worker to the audio thread: the newest
/// endpointing advice, tagged with the utterance it was computed for.
#[derive(Default)]
pub struct EndHintCell {
    packed: AtomicU64, // 0 = none
    speech_frames: AtomicU32,
}

impl EndHintCell {
    pub fn new() -> Self {
        Self::default()
    }
    /// `seq`: the utterance the advice belongs to (from `Utterance::seq`).
    pub fn set(&self, silence_ms: u32, seq: u32, speech_frames: u32) {
        self.speech_frames.store(speech_frames, Ordering::Relaxed);
        // bit 63 marks "present"; low 32 = seq; bits 32..62 = silence_ms
        let v = (1u64 << 63) | ((silence_ms as u64 & 0x7FFF_FFFF) << 32) | seq as u64;
        self.packed.store(v, Ordering::Release);
    }
    /// (silence_ms, seq, speech_frames)
    pub fn get(&self) -> Option<(u32, u32, u32)> {
        let v = self.packed.load(Ordering::Acquire);
        if v >> 63 == 0 {
            return None;
        }
        Some((((v >> 32) & 0x7FFF_FFFF) as u32, v as u32, self.speech_frames.load(Ordering::Relaxed)))
    }
    pub fn clear(&self) {
        self.packed.store(0, Ordering::Release);
    }
}

/// Identity of a finished utterance, so a speculative decode can be reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinishedMeta {
    pub seq: u32,
    pub speech_frames: u32,
    /// where the returned audio sat in the stream fed to the VAD (samples @ 16 kHz)
    pub start_sample: u64,
    pub end_sample: u64,
}

pub struct Vad {
    seq: u32,
    end_hint: Option<(usize, usize)>, // (silence frames, speech frames when advised)
    finished_meta: Option<FinishedMeta>,
    floor: f32,
    in_speech: bool,
    speech_run: usize,
    silence_run: usize,
    utterance: Vec<f32>,
    preroll: VecDeque<f32>,
    frame_buf: Vec<f32>,
    /// RMS of each 10 ms frame of `utterance` (for choosing a quiet cut point)
    frame_rms: Vec<f32>,
    /// RMS of the last second of frames, whatever the state (steady-background detection)
    hist: VecDeque<f32>,
    steady_run: usize,
    /// frames left during which a steady background (music bed, fan) is considered to be present;
    /// refreshed whenever the background is observed steady, so speech over it does not reset it
    bg_known_frames: usize,
    /// total samples consumed so far (stream position)
    fed_samples: u64,
    frames_in_utterance: usize,
    speech_frames_in_utterance: usize,
    /// most recent frame RMS, for the UI level meter
    pub last_rms: f32,
    /// utterances discarded as too short/quiet (never sent to ASR) since creation
    pub dropped_count: u64,
    /// wall-clock span of the most recently discarded utterance, in ms
    pub last_dropped_ms: u64,
}

impl Default for Vad {
    fn default() -> Self {
        Self::new()
    }
}

impl Vad {
    pub fn new() -> Self {
        Vad {
            seq: 0,
            end_hint: None,
            finished_meta: None,
            floor: 0.003,
            in_speech: false,
            speech_run: 0,
            silence_run: 0,
            utterance: Vec::new(),
            preroll: VecDeque::with_capacity(PREROLL_FRAMES * FRAME),
            frame_buf: Vec::new(),
            frame_rms: Vec::new(),
            hist: VecDeque::with_capacity(HIST_FRAMES + 1),
            steady_run: 0,
            bg_known_frames: 0,
            fed_samples: 0,
            frames_in_utterance: 0,
            speech_frames_in_utterance: 0,
            last_rms: 0.0,
            dropped_count: 0,
            last_dropped_ms: 0,
        }
    }

    /// Level needed to START hearing speech.
    fn threshold(&self) -> f32 {
        // Over a known-steady background (music bed, fan) its fluctuation is small,
        // so a smaller multiple is safe and lets speech ~6 dB above it start.
        let mult = if self.bg_known_frames > 0 { 1.8 } else { 3.0 };
        (self.floor * mult).max(self.floor + 0.012)
    }

    /// Level needed to KEEP hearing speech once it has started. Lower than the
    /// start level (hysteresis) so soft word endings and quiet syllables do not
    /// count as silence - the cause of speech being chopped into fragments.
    fn stay_threshold(&self) -> f32 {
        // NOT lowered over a steady background: its own beats must stay below this
        // level, or an endless run of "speech" frames would stop the utterance ending.
        (self.floor * 1.8).max(self.floor + 0.005)
    }

    /// Feed samples; returns completed utterances.
    pub fn feed(&mut self, samples: &[f32]) -> Vec<Vec<f32>> {
        self.feed_with_meta(samples).into_iter().map(|(u, _)| u).collect()
    }

    /// Like [`feed`](Self::feed), but each utterance carries its identity.
    pub fn feed_with_meta(&mut self, samples: &[f32]) -> Vec<(Vec<f32>, FinishedMeta)> {
        let mut done = Vec::new();
        self.frame_buf.extend_from_slice(samples);

        while self.frame_buf.len() >= FRAME {
            let frame: Vec<f32> = self.frame_buf.drain(..FRAME).collect();
            let rms = {
                let sum: f32 = frame.iter().map(|s| s * s).sum();
                (sum / FRAME as f32).sqrt()
            };
            self.last_rms = rms;
            if let Some(u) = self.step_frame(frame, rms) {
                if let Some(meta) = self.finished_meta.take() {
                    done.push((u, meta));
                }
            }
        }
        done
    }

    /// Identity of the utterance currently being collected.
    pub fn seq(&self) -> u32 {
        self.seq
    }

    pub fn speech_frames(&self) -> usize {
        self.speech_frames_in_utterance
    }

    /// Apply endpointing advice. Ignored unless it was computed for the
    /// utterance in progress (`seq`), so advice can never leak to the next one.
    pub fn set_end_hint(&mut self, silence_ms: u32, seq: u32, speech_frames: u32) {
        if seq == self.seq && self.in_speech {
            let frames = ((silence_ms / 10) as usize).clamp(HINT_MIN_FRAMES, HINT_MAX_FRAMES);
            self.end_hint = Some((frames, speech_frames as usize));
        }
    }

    /// The utterance so far, offered once per pause (>=100 ms of silence
    /// after real speech) for a speculative full-quality decode. Returns
    /// (samples, speech_frames, seq).
    pub fn pause_snapshot(&self) -> Option<(&[f32], u32, u32)> {
        (self.in_speech
            && self.speech_frames_in_utterance >= MIN_SPEECH_FRAMES
            && self.silence_run >= PAUSE_SNAPSHOT_FRAMES)
            .then(|| (self.utterance.as_slice(), self.speech_frames_in_utterance as u32, self.seq))
    }

    /// Raise the noise floor to a steady background even while "in speech".
    /// Without this, music or a fan that is already playing when the app starts
    /// (or that gets louder mid-service) is heard as one endless utterance,
    /// because the floor only used to adapt while silent.
    fn track_steady_background(&mut self, rms: f32) {
        self.hist.push_back(rms);
        if self.hist.len() > HIST_FRAMES {
            self.hist.pop_front();
        }
        if self.hist.len() < HIST_FRAMES {
            return;
        }
        let n = self.hist.len() as f32;
        let mean = self.hist.iter().sum::<f32>() / n;
        let var = self.hist.iter().map(|r| (r - mean) * (r - mean)).sum::<f32>() / n;
        let cv = var.sqrt() / mean.max(1e-6);
        if cv < STEADY_CV {
            self.steady_run += 1;
        } else {
            self.steady_run = 0;
        }
        self.bg_known_frames = self.bg_known_frames.saturating_sub(1);
        if self.steady_run >= STEADY_HOLD_FRAMES {
            self.bg_known_frames = 3000; // remember it for 30 s, through the speech over it
        }
        if self.in_speech && self.steady_run >= STEADY_HOLD_FRAMES && mean > self.floor * 1.2 {
            // aim the floor at the background's typical level, so the "stay"
            // level (1.8x floor) sits above nearly all of its beats
            self.floor += (mean.min(MAX_FLOOR) - self.floor) * 0.03;
        }
    }

    fn step_frame(&mut self, frame: Vec<f32>, rms: f32) -> Option<Vec<f32>> {
        self.fed_samples += FRAME as u64;
        self.track_steady_background(rms);
        let above = rms > self.threshold();

        if !self.in_speech {
            // adapt noise floor slowly while silent
            self.floor = self.floor * 0.995 + rms * 0.005;
            self.preroll.extend(frame);
            while self.preroll.len() > PREROLL_FRAMES * FRAME {
                self.preroll.pop_front();
            }
            if above {
                self.speech_run += 1;
                if self.speech_run >= SPEECH_START_FRAMES {
                    self.in_speech = true;
                    self.frame_rms.clear();
                    let pre: Vec<f32> = self.preroll.drain(..).collect();
                    for f in pre.chunks(FRAME) {
                        self.frame_rms.push((f.iter().map(|s| s * s).sum::<f32>() / f.len() as f32).sqrt());
                    }
                    self.utterance.extend(pre);
                    self.utterance.extend(std::iter::repeat(0.0).take(0));
                    self.frames_in_utterance = SPEECH_START_FRAMES;
                    self.speech_frames_in_utterance = SPEECH_START_FRAMES;
                    self.speech_run = 0;
                    self.silence_run = 0;
                }
            } else {
                self.speech_run = 0;
            }
            return None;
        }

        // in speech: judged against the lower "stay" level
        let above = rms > self.stay_threshold();
        self.utterance.extend(frame);
        self.frame_rms.push(rms);
        self.frames_in_utterance += 1;
        if above {
            self.silence_run = 0;
            self.speech_frames_in_utterance += 1;
        } else {
            self.silence_run += 1;
        }

        // Short utterances (likely a bare command) end on a much shorter
        // trailing silence than a longer, in-progress phrase (likely
        // Scripture) — see the constants above.
        let default_end = if self.speech_frames_in_utterance <= SHORT_UTTERANCE_SPEECH_FRAMES_CAP {
            SHORT_SPEECH_END_FRAMES
        } else {
            SPEECH_END_FRAMES
        };
        // Semantic advice from the ASR worker wins, unless the speaker has
        // said more since the advice was computed.
        let end_threshold = match self.end_hint {
            Some((frames, at)) if self.speech_frames_in_utterance <= at + HINT_STALE_TOLERANCE_FRAMES => frames,
            _ => default_end,
        };
        // Long continuous speech: cut at its quietest moment, keep listening.
        if self.silence_run < end_threshold && self.utterance.len() / FRAME >= SOFT_CUT_FRAMES {
            return self.soft_cut();
        }
        if self.silence_run >= end_threshold || self.frames_in_utterance >= MAX_UTTERANCE_FRAMES {
            // BUG (fixed): `self.silence_run` used to be zeroed on the next
            // two lines *before* being read here, so `trim` was always
            // computed from 0 and never removed anything — every utterance
            // carried its full trailing SPEECH_END_FRAMES (≤800 ms) of
            // near-silence into the ASR call. Capture the run length first.
            let trailing_silence_frames = self.silence_run.min(end_threshold);
            self.in_speech = false;
            self.speech_run = 0;
            self.silence_run = 0;
            self.frame_rms.clear();
            let mut u = std::mem::take(&mut self.utterance);
            let start_sample = self.fed_samples.saturating_sub(u.len() as u64);
            // trim trailing silence
            // keep 200 ms of the tail: word-final sounds like the 's' in "Americans" are
            // faint and were being trimmed away (measured: "fellow America.")
            let trim = trailing_silence_frames.saturating_sub(TAIL_KEEP_FRAMES) * FRAME;
            u.truncate(u.len().saturating_sub(trim));
            let end_sample = start_sample + u.len() as u64;
            self.frames_in_utterance = 0;
            let speech_frames = self.speech_frames_in_utterance;
            self.speech_frames_in_utterance = 0;
            self.finished_meta = Some(FinishedMeta { seq: self.seq, speech_frames: speech_frames as u32, start_sample, end_sample });
            self.seq = self.seq.wrapping_add(1);
            self.end_hint = None;
            if speech_frames >= MIN_SPEECH_FRAMES {
                Some(u)
            } else {
                // Discarded as noise (a cough, a mic bump, or genuinely
                // quiet/soft speech that rarely cleared the threshold).
                // This used to be completely silent — the operator got no
                // signal that anything was heard at all. Record it so the
                // UI can say so instead of leaving a dead pause unexplained.
                self.dropped_count += 1;
                self.last_dropped_ms = (u.len() as u64 * 1000) / WHISPER_RATE as u64;
                None
            }
        } else {
            None
        }
    }

    /// Split a long utterance at its quietest 10 ms frame (searching after
    /// `SOFT_CUT_SEARCH_FROM`). The first part is returned as a finished
    /// utterance; the rest stays in progress, so the speaker is never
    /// interrupted and no audio is lost or duplicated.
    fn soft_cut(&mut self) -> Option<Vec<f32>> {
        let n = self.frame_rms.len().min(self.utterance.len() / FRAME);
        let from = SOFT_CUT_SEARCH_FROM.min(n.saturating_sub(1));
        // latest frame among the quietest (ties go to the later one)
        let mut idx = from;
        for i in from..n {
            if self.frame_rms[i] <= self.frame_rms[idx] {
                idx = i;
            }
        }
        let cut = idx * FRAME;
        let start_sample = self.fed_samples.saturating_sub(self.utterance.len() as u64);
        let rest = self.utterance.split_off(cut);
        let first = std::mem::replace(&mut self.utterance, rest);
        let rest_rms = self.frame_rms.split_off(idx);
        let first_rms = std::mem::replace(&mut self.frame_rms, rest_rms);
        let stay = self.stay_threshold();
        let first_speech = first_rms.iter().filter(|r| **r > stay).count();
        let rest_speech = self.frame_rms.iter().filter(|r| **r > stay).count();
        self.frames_in_utterance = self.frame_rms.len();
        self.speech_frames_in_utterance = rest_speech;
        self.silence_run = self.frame_rms.iter().rev().take_while(|r| **r <= stay).count();
        self.finished_meta = Some(FinishedMeta { seq: self.seq, speech_frames: first_speech as u32, start_sample, end_sample: start_sample + cut as u64 });
        self.seq = self.seq.wrapping_add(1);
        self.end_hint = None;
        if first_speech >= MIN_SPEECH_FRAMES {
            Some(first)
        } else {
            self.finished_meta = None;
            None
        }
    }

    /// True while the detector considers the speaker active (UI indicator).
    /// The utterance captured so far, while speech is still ongoing — for
    /// interim (partial) decoding, so text and quotation detection can appear
    /// while the speaker is still talking instead of only after they pause.
    /// `None` until it holds real speech (not just a noise blip), and `None`
    /// again once the speaker has gone quiet (nothing new to decode).
    pub fn in_progress(&self) -> Option<&[f32]> {
        const LIVE_SILENCE_FRAMES: usize = 20; // 200 ms
        (self.in_speech
            && self.speech_frames_in_utterance >= MIN_SPEECH_FRAMES
            && self.silence_run <= LIVE_SILENCE_FRAMES)
            .then(|| self.utterance.as_slice())
    }

    pub fn speaking(&self) -> bool {
        self.in_speech
    }

    /// (noise floor, start level, stay level, background-known) - for diagnostics and tests
    pub fn levels(&self) -> (f32, f32, f32, bool) {
        (self.floor, self.threshold(), self.stay_threshold(), self.bg_known_frames > 0)
    }
}

/// Decides when to hand the ASR worker an interim snapshot of an utterance
/// that is still being spoken. Pure logic (no threads/devices) so it can be
/// tested directly.
#[derive(Default)]
pub struct PartialPacer {
    last_len: usize,
    last_pause_speech: Option<usize>,
    /// how long the ASR worker's last interim decode took (0 = unknown)
    decode_ms: u32,
}

impl PartialPacer {
    /// Don't decode less than 1.4 s (includes the 0.5 s pre-roll): tiny
    /// clips give unreliable text.
    pub const MIN_SAMPLES: usize = 16_000;
    /// Longest step between interim decodes (1.0 s); the step shrinks toward
    /// `FASTEST_STEP_SAMPLES` when the engine proves it can keep up.
    pub const STEP_SAMPLES: usize = 16_000;
    /// Shortest step (0.4 s).
    pub const FASTEST_STEP_SAMPLES: usize = 6_400;

    pub fn new() -> Self {
        Self::default()
    }

    /// `snap_len`: samples in the in-progress utterance. `asr_busy`: the ASR
    /// worker is mid-decode. `queue_len`: jobs already waiting for it. Never
    /// piles interim work on a worker that is behind.
    pub fn should_emit(&mut self, snap_len: usize, asr_busy: bool, queue_len: usize) -> bool {
        if asr_busy || queue_len > 0 {
            return false;
        }
        if snap_len < Self::MIN_SAMPLES || snap_len < self.last_len + self.step_samples() {
            return false;
        }
        self.last_len = snap_len;
        true
    }

    /// Report how long the last interim decode took. A decode that takes
    /// 100 ms lets us ask again every ~0.4 s; one that takes 800 ms keeps
    /// the old 1 s cadence. Never less than twice the decode time.
    pub fn note_decode_ms(&mut self, ms: u32) {
        self.decode_ms = ms;
    }

    fn step_samples(&self) -> usize {
        if self.decode_ms == 0 {
            return Self::STEP_SAMPLES;
        }
        let twice_decode = (self.decode_ms as usize * 2) * (WHISPER_RATE as usize / 1000);
        twice_decode.clamp(Self::FASTEST_STEP_SAMPLES, Self::STEP_SAMPLES)
    }

    /// Once per distinct pause (keyed by how much speech preceded it): hand
    /// the worker a full-quality speculative decode. Same back-pressure rules.
    pub fn should_emit_pause(&mut self, speech_frames: usize, asr_busy: bool, queue_len: usize) -> bool {
        if asr_busy || queue_len > 0 || self.last_pause_speech == Some(speech_frames) {
            return false;
        }
        self.last_pause_speech = Some(speech_frames);
        true
    }

    /// Call when an utterance finishes (or speech stops): the next one starts fresh.
    pub fn reset(&mut self) {
        self.last_len = 0;
        self.last_pause_speech = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_speech(seconds: f32, amp: f32) -> Vec<f32> {
        let n = (seconds * 16_000.0) as usize;
        // syllable-rate amplitude wobble (4 Hz), like real speech; a perfectly
        // steady tone is deliberately treated as a background hum by the VAD
        (0..n)
            .map(|i| {
                let t = i as f32 / 16_000.0;
                let env = 0.6 + 0.4 * (t * 2.0 * std::f32::consts::PI * 4.0).sin();
                (i as f32 * 0.05).sin() * amp * env
            })
            .collect()
    }

    fn silence(seconds: f32) -> Vec<f32> {
        vec![0.0; (seconds * 16_000.0) as usize]
    }

    #[test]
    fn detects_utterance_between_silences() {
        let mut vad = Vad::new();
        let mut out = Vec::new();
        out.extend(vad.feed(&silence(1.0)));
        out.extend(vad.feed(&sine_speech(1.2, 0.2)));
        out.extend(vad.feed(&silence(1.2)));
        assert_eq!(out.len(), 1, "one utterance expected");
        let u = &out[0];
        // utterance should be roughly preroll (0.5s) + speech (1.2s), i.e.
        // ~27.2k samples — NOT that plus the 0.8s trailing-silence window
        // that triggered the cutoff (~39.5k). The old bound of 40_000 here
        // was loose enough to pass even with the trim bug present (the
        // untrimmed length landed at ~39_520, just under it) — that's how
        // the bug shipped unnoticed. 30_000 is tight enough to actually
        // catch a regression.
        assert!(u.len() > 12_000, "len={}", u.len());
        assert!(u.len() < 30_000, "len={} (trailing silence not trimmed?)", u.len());
    }

    #[test]
    fn trailing_silence_is_actually_trimmed() {
        // Direct regression test for the trim-uses-zeroed-value bug: an
        // utterance followed by exactly the cutoff window of silence must
        // come back close to the spoken length, not spoken + 0.8s of pad.
        let mut vad = Vad::new();
        vad.feed(&silence(1.0));
        let mut out = Vec::new();
        out.extend(vad.feed(&sine_speech(1.0, 0.2)));
        out.extend(vad.feed(&silence(1.5))); // > 800ms cutoff, well clear of it
        assert_eq!(out.len(), 1);
        let u = &out[0];
        // preroll (0.5s) + speech (1.0s) = 1.5s = 24_000 samples. Before the
        // fix this would additionally include the 0.8s trailing window
        // (+12_800 samples, landing at ~36_800).
        assert!(
            u.len() < 27_000,
            "len={} — looks like trailing silence leaked back in (buggy trim)",
            u.len()
        );
    }

    #[test]
    fn short_command_commits_after_the_short_tail_not_the_full_one() {
        // A brief command ("next.", "read it.") should NOT need to wait out
        // the full 800ms scripture-grade tail before it commits.
        let mut vad = Vad::new();
        vad.feed(&silence(1.0));
        vad.feed(&sine_speech(0.6, 0.2)); // well under the 1.5s short-utterance cap
        // Feed the short-tail window worth of silence in small steps and stop
        // the instant an utterance comes back, so we can measure exactly how
        // much trailing silence it actually needed.
        let mut ms_needed = 0u32;
        let mut got = Vec::new();
        for _ in 0..150 {
            // up to 1.5s, in 10ms steps
            let out = vad.feed(&silence(0.01));
            ms_needed += 10;
            if !out.is_empty() {
                got = out;
                break;
            }
        }
        assert!(!got.is_empty(), "utterance never committed");
        assert!(
            ms_needed <= 500,
            "short command took {ms_needed}ms of trailing silence to commit — expected ~400ms, not the full 800ms scripture tail"
        );
    }

    #[test]
    fn long_utterance_still_gets_the_full_safe_tail() {
        // A longer, in-progress phrase (a scripture reference read with
        // natural pauses) must NOT be cut on the short 400ms fuse — that
        // would reintroduce premature chopping. It should still need close
        // to the full 800ms.
        let mut vad = Vad::new();
        vad.feed(&silence(1.0));
        vad.feed(&sine_speech(2.0, 0.2)); // well over the 1.5s short-utterance cap
        let mut ms_needed = 0u32;
        let mut got = Vec::new();
        for _ in 0..150 {
            let out = vad.feed(&silence(0.01));
            ms_needed += 10;
            if !out.is_empty() {
                got = out;
                break;
            }
        }
        assert!(!got.is_empty(), "utterance never committed");
        assert!(
            ms_needed >= 700,
            "a long utterance committed after only {ms_needed}ms — the short 400ms fuse should not apply here"
        );
    }

    #[test]
    fn short_or_quiet_bursts_are_recorded_as_dropped() {
        // A burst that starts speech (>= 30ms above threshold) but never
        // accumulates 250ms of above-threshold audio must still be
        // discarded (anti-noise behavior is unchanged) — but it must no
        // longer vanish silently: dropped_count/last_dropped_ms should
        // reflect it, so the UI has something to show the operator.
        let mut vad = Vad::new();
        vad.feed(&silence(1.0));
        assert_eq!(vad.dropped_count, 0);
        vad.feed(&sine_speech(0.05, 0.3)); // 50ms burst — below MIN_SPEECH_FRAMES
        let out = vad.feed(&silence(1.5));
        assert!(out.is_empty(), "a too-short burst must still not reach the ASR engine");
        assert_eq!(vad.dropped_count, 1, "the drop must be counted");
        assert!(vad.last_dropped_ms > 0, "a duration must be recorded for the UI");
    }

    #[test]
    fn silence_alone_produces_nothing() {
        let mut vad = Vad::new();
        assert!(vad.feed(&silence(3.0)).is_empty());
    }

    #[test]
    fn tiny_burst_discarded() {
        let mut vad = Vad::new();
        vad.feed(&silence(1.0));
        let out = vad.feed(&sine_speech(0.05, 0.3)); // 50 ms — below min
        out.into_iter().for_each(|_| {});
        let out2 = vad.feed(&silence(1.5));
        assert!(out2.is_empty() || out2.iter().all(|u| u.is_empty()));
    }

    #[test]
    fn in_progress_is_available_only_while_speech_is_ongoing() {
        let mut vad = Vad::new();
        vad.feed(&silence(1.0));
        assert!(vad.in_progress().is_none(), "nothing yet");
        vad.feed(&sine_speech(0.05, 0.3));
        assert!(vad.in_progress().is_none(), "a blip is not real speech yet");
        let mut vad = Vad::new();
        vad.feed(&silence(1.0));
        vad.feed(&sine_speech(1.5, 0.2));
        let snap_len = vad.in_progress().expect("real speech is ongoing").len();
        assert!(snap_len > 16_000, "len={snap_len}");
        vad.feed(&sine_speech(1.0, 0.2));
        assert!(vad.in_progress().unwrap().len() > snap_len, "must grow as speech continues");
        // once the speaker goes quiet there is nothing new to decode
        vad.feed(&silence(0.3));
        assert!(vad.in_progress().is_none());
    }

    #[test]
    fn pacer_waits_for_enough_audio_then_steps() {
        let mut p = PartialPacer::new();
        assert!(!p.should_emit(10_000, false, 0), "too short to decode reliably");
        assert!(p.should_emit(23_000, false, 0), "first interim once long enough");
        assert!(!p.should_emit(30_000, false, 0), "needs a full step of new audio");
        assert!(p.should_emit(39_500, false, 0));
        p.reset();
        assert!(p.should_emit(23_000, false, 0), "a new utterance starts fresh");
    }

    #[test]
    fn pacer_never_piles_work_on_a_busy_or_backlogged_worker() {
        let mut p = PartialPacer::new();
        assert!(!p.should_emit(60_000, true, 0), "worker mid-decode");
        assert!(!p.should_emit(60_000, false, 1), "work already queued");
        assert!(p.should_emit(60_000, false, 0), "the moment it's free it proceeds");
    }

    /// Feed silence in 10 ms steps until an utterance comes back; returns ms needed.
    fn silence_until_final(vad: &mut Vad, max_ms: u32) -> Option<u32> {
        for ms in (10..=max_ms).step_by(10) {
            if !vad.feed(&silence(0.01)).is_empty() {
                return Some(ms);
            }
        }
        None
    }

    fn speech_then_hint(speech_s: f32, hint_ms: Option<u32>) -> Option<u32> {
        let mut vad = Vad::new();
        vad.feed(&silence(1.0));
        vad.feed(&sine_speech(speech_s, 0.2));
        if let Some(ms) = hint_ms {
            let sf = vad.speech_frames() as u32;
            vad.set_end_hint(ms, vad.seq(), sf);
        }
        silence_until_final(&mut vad, 1500)
    }

    #[test]
    fn a_finished_reference_ends_sooner_than_the_default_800ms() {
        let default = speech_then_hint(2.5, None).unwrap();
        let hinted = speech_then_hint(2.5, Some(350)).unwrap();
        assert!((790..=810).contains(&default), "default tail {default}");
        assert!((340..=360).contains(&hinted), "hinted tail {hinted}");
    }

    #[test]
    fn an_unfinished_reference_waits_longer_than_the_default() {
        let hinted = speech_then_hint(2.5, Some(1100)).unwrap();
        assert!((1090..=1110).contains(&hinted), "{hinted}");
    }

    #[test]
    fn hints_are_clamped_to_safe_bounds() {
        assert!(speech_then_hint(2.5, Some(10)).unwrap() >= 250, "never below 250 ms");
        assert!(speech_then_hint(2.5, Some(9000)).unwrap() <= 1210, "never above 1.2 s");
    }

    #[test]
    fn a_stale_hint_is_ignored_once_more_speech_arrives() {
        let mut vad = Vad::new();
        vad.feed(&silence(1.0));
        vad.feed(&sine_speech(2.0, 0.2));
        let sf = vad.speech_frames() as u32;
        vad.set_end_hint(300, vad.seq(), sf);
        vad.feed(&sine_speech(1.0, 0.2)); // the speaker kept going
        let ms = silence_until_final(&mut vad, 1500).unwrap();
        assert!(ms >= 790, "stale 300 ms advice must not cut them off (got {ms})");
    }

    #[test]
    fn advice_for_one_utterance_never_leaks_into_the_next() {
        let mut vad = Vad::new();
        vad.feed(&silence(1.0));
        vad.feed(&sine_speech(2.5, 0.2));
        let (old_seq, sf) = (vad.seq(), vad.speech_frames() as u32);
        vad.set_end_hint(300, old_seq, sf);
        assert!(silence_until_final(&mut vad, 1500).unwrap() <= 310);
        // next utterance: a late-arriving hint tagged with the OLD seq is dropped
        vad.feed(&sine_speech(2.5, 0.2));
        vad.set_end_hint(300, old_seq, vad.speech_frames() as u32);
        assert!(silence_until_final(&mut vad, 1500).unwrap() >= 790);
    }

    #[test]
    fn hint_cell_roundtrips_and_clears() {
        let c = EndHintCell::new();
        assert_eq!(c.get(), None);
        c.set(350, 7, 240);
        assert_eq!(c.get(), Some((350, 7, 240)));
        c.clear();
        assert_eq!(c.get(), None);
    }

    #[test]
    fn a_pause_offers_exactly_one_snapshot_and_it_matches_the_final() {
        let mut vad = Vad::new();
        let mut pacer = PartialPacer::new();
        vad.feed(&silence(1.0));
        vad.feed(&sine_speech(2.0, 0.2));
        assert!(vad.pause_snapshot().is_none(), "still speaking: no pause yet");
        vad.feed(&silence(0.15)); // a 150 ms pause
        let (snap, sf, seq) = vad.pause_snapshot().expect("pause snapshot");
        assert!(snap.len() > 30_000);
        assert!(pacer.should_emit_pause(sf as usize, false, 0));
        assert!(!pacer.should_emit_pause(sf as usize, false, 0), "only once per pause");
        // no new speech → the final carries the same identity (reusable)
        let fin = {
            let mut got = None;
            for _ in 0..200 {
                let mut r = vad.feed_with_meta(&silence(0.01));
                if let Some(x) = r.pop() {
                    got = Some(x);
                    break;
                }
            }
            got.expect("final")
        };
        assert_eq!(fin.1.seq, seq);
        assert_eq!(fin.1.speech_frames, sf, "same speech_frames ⇒ safe to reuse the snapshot's text");
    }

    #[test]
    fn more_speech_after_a_pause_changes_the_identity_so_the_snapshot_is_not_reused() {
        let mut vad = Vad::new();
        vad.feed(&silence(1.0));
        vad.feed(&sine_speech(1.5, 0.2));
        vad.feed(&silence(0.15));
        let (_, sf, _) = vad.pause_snapshot().unwrap();
        vad.feed(&sine_speech(1.0, 0.2)); // "…verse sixteen" after a breath
        let mut fin = None;
        for _ in 0..200 {
            if let Some(x) = vad.feed_with_meta(&silence(0.01)).pop() {
                fin = Some(x);
                break;
            }
        }
        assert!(fin.unwrap().1.speech_frames > sf);
    }

    #[test]
    fn interim_cadence_adapts_to_how_fast_the_engine_is() {
        let mut slow = PartialPacer::new();
        slow.note_decode_ms(900);
        assert!(slow.should_emit(20_000, false, 0));
        assert!(!slow.should_emit(20_000 + 10_000, false, 0), "slow engine: still ~1 s apart");
        assert!(slow.should_emit(20_000 + 16_000, false, 0));

        let mut fast = PartialPacer::new();
        fast.note_decode_ms(80);
        assert!(fast.should_emit(17_000, false, 0), "first interim after only ~1.06 s of audio");
        assert!(!fast.should_emit(17_000 + 3_000, false, 0));
        assert!(fast.should_emit(17_000 + 6_400, false, 0), "fast engine: new text every 0.4 s");
    }
}
