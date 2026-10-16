//! Automatic gain control for the speech path.
//!
//! WHY: the voice detector and the speech models both behave best when the
//! speaker arrives at a roughly constant level. A built-in Mac mic at a low
//! input volume delivers a pastor at RMS ~0.01-0.03; the detector's fixed
//! margin (0.012) then sees the soft half of every sentence as silence and
//! chops the speech into fragments ("it stops hearing me after a second").
//! Measured on real speech in `tests/continuous_speech.rs`.
//!
//! HOW: a slow-release / fast-attack gain follower that
//!   * only learns from blocks that look like speech (well above a
//!     minimum-statistics noise floor), so room noise and music beds are not
//!     pumped up;
//!   * is limited to +24 dB, so a silent mic cannot turn hiss into "speech";
//!   * soft-limits peaks so a sudden loud sound cannot clip.
//! The raw (un-levelled) audio is still what gets recorded to audio.wav.

const BLOCK: usize = 160; // 10 ms @ 16 kHz
const TARGET_RMS: f32 = 0.08; // about -22 dBFS: comfortable for VAD and ASR
const MAX_GAIN: f32 = 16.0; // +24 dB
/// Never attenuate: a loud speaker or music bed is fine as it is (the soft limiter
/// catches peaks), and turning the gain DOWN makes quiet words fall below the
/// detector's levels. Found by tracing speech over a music bed: gain fell to 0.3
/// and 45% of the speech was lost.
const MIN_GAIN: f32 = 1.0;
const FLOOR_WINDOW_BLOCKS: usize = 300; // 3 s minimum-statistics window
const SPEECH_RATIO: f32 = 2.5; // "speech-like" = this many times the floor
const GAIN_UP_TAU_BLOCKS: f32 = 30.0; // ~0.3 s to raise
const GAIN_DOWN_TAU_BLOCKS: f32 = 5.0; // ~50 ms to lower
const LEVEL_TAU_BLOCKS: f32 = 25.0; // speech level estimate, ~0.25 s
/// Real microphones always hiss a little; a "floor" of exactly zero (a noise
/// gate, a muted channel) must not make ordinary room noise look like speech.
const MIN_FLOOR: f32 = 0.0015;
const MIN_SPEECH_RMS: f32 = 0.004;
/// Speech wobbles at syllable rate; a fan, hum or sustained chord does not.
/// Measured (std/mean of 10 ms levels): fan 0.04, chord bed 0.19, speech ~1.0.
const MIN_MODULATION_CV: f32 = 0.4;
const MODULATION_WINDOW_BLOCKS: usize = 50;
/// The first speech after start-up is learned quickly, so the first words are not lost.
const FAST_LEARN_BLOCKS: u32 = 150;
const RELAX_TAU_BLOCKS: f32 = 800.0; // without speech, gain drifts back to 1 over ~8 s

pub struct Agc {
    gain: f32,
    speech_level: f32,
    recent: std::collections::VecDeque<f32>, // per-block RMS, last 3 s
    speech_blocks: u32,
    enabled: bool,
}

impl Default for Agc {
    fn default() -> Self {
        Self::new()
    }
}

impl Agc {
    pub fn new() -> Self {
        // RF_AGC=0 turns it off, for A/B comparisons.
        let enabled = std::env::var("RF_AGC").map(|v| v != "0").unwrap_or(true);
        Agc { gain: 1.0, speech_level: TARGET_RMS, speech_blocks: 0, recent: std::collections::VecDeque::with_capacity(FLOOR_WINDOW_BLOCKS + 1), enabled }
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }

    fn floor(&self) -> f32 {
        self.recent.iter().copied().fold(f32::MAX, f32::min).clamp(MIN_FLOOR, 1.0)
    }

    /// How much the level wobbles over the last half second (std/mean; 0 = perfectly steady)
    fn modulation(&self) -> f32 {
        let n = self.recent.len().min(MODULATION_WINDOW_BLOCKS);
        if n < 10 {
            return 0.0;
        }
        let v: Vec<f32> = self.recent.iter().rev().take(n).copied().collect();
        let mean = v.iter().sum::<f32>() / n as f32;
        let var = v.iter().map(|r| (r - mean) * (r - mean)).sum::<f32>() / n as f32;
        var.sqrt() / mean.max(1e-6)
    }

    /// Level `samples` (any length). Output length equals input length.
    pub fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        if !self.enabled {
            return samples.to_vec();
        }
        let mut out = Vec::with_capacity(samples.len());
        for block in samples.chunks(BLOCK) {
            let rms = (block.iter().map(|s| s * s).sum::<f32>() / block.len() as f32).sqrt();
            self.recent.push_back(rms);
            if self.recent.len() > FLOOR_WINDOW_BLOCKS {
                self.recent.pop_front();
            }
            let floor = self.floor();
            let speech_like = rms > floor * SPEECH_RATIO && rms > MIN_SPEECH_RMS && self.modulation() >= MIN_MODULATION_CV;
            if speech_like {
                let fast = self.speech_blocks < FAST_LEARN_BLOCKS;
                self.speech_blocks = self.speech_blocks.saturating_add(1);
                let tau = if fast { 6.0 } else { LEVEL_TAU_BLOCKS };
                self.speech_level += (rms - self.speech_level) * (1.0 / tau);
            }
            let desired = (TARGET_RMS / self.speech_level.max(1e-4)).clamp(MIN_GAIN, MAX_GAIN);
            let start_gain = self.gain;
            if speech_like || desired < self.gain {
                let up = if self.speech_blocks < FAST_LEARN_BLOCKS { 10.0 } else { GAIN_UP_TAU_BLOCKS };
                let tau = if desired < self.gain { GAIN_DOWN_TAU_BLOCKS } else { up };
                self.gain += (desired - self.gain) * (1.0 / tau);
            } else {
                // no speech in this block: slowly return toward unity
                self.gain += (1.0 - self.gain) * (1.0 / RELAX_TAU_BLOCKS);
                self.speech_level += (TARGET_RMS - self.speech_level) * (1.0 / (RELAX_TAU_BLOCKS * 4.0));
            }
            let end_gain = self.gain;
            let n = block.len() as f32;
            for (i, s) in block.iter().enumerate() {
                let g = start_gain + (end_gain - start_gain) * (i as f32 / n);
                out.push(soft_limit(s * g));
            }
        }
        out
    }
}

/// Transparent below 0.8, smoothly squashed toward +-1 above it.
fn soft_limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.8 {
        x
    } else {
        x.signum() * (0.8 + 0.2 * ((a - 0.8) / 0.2).tanh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rms: f32, secs: f32) -> Vec<f32> {
        // a tone with syllable-like on/off modulation so some blocks are quieter
        let n = (secs * 16_000.0) as usize;
        (0..n)
            .map(|i| {
                let env = 0.55 + 0.45 * (i as f32 / 16_000.0 * 2.0 * std::f32::consts::PI * 3.0).sin();
                (i as f32 * 0.07).sin() * rms * 1.4142 * env / 0.78
            })
            .collect()
    }
    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    #[test]
    fn quiet_speakers_are_lifted_to_a_workable_level_and_loud_ones_left_alone() {
        for input in [0.008f32, 0.02] {
            let mut agc = Agc::new();
            let _warm = agc.process(&tone(input, 4.0));
            let out = rms(&agc.process(&tone(input, 2.0)));
            assert!((0.05..0.13).contains(&out), "input {input} -> output {out}");
        }
        for input in [0.08f32, 0.2] {
            let mut agc = Agc::new();
            let _warm = agc.process(&tone(input, 4.0));
            assert!(agc.gain() >= 1.0 && agc.gain() < 1.25, "input {input} must be (almost) left alone, gain {}", agc.gain());
        }
    }

    #[test]
    fn steady_noise_is_not_pumped_up() {
        let mut agc = Agc::new();
        let noise: Vec<f32> = (0..16_000u64 * 20).map(|i| ((i * 7919 % 2003) as f32 / 2003.0 - 0.5) * 0.006).collect();
        let out = agc.process(&noise);
        assert!(agc.gain() <= 1.05, "gain rose to {} on pure noise", agc.gain());
        assert!(rms(&out) < 0.01);
    }

    #[test]
    fn gain_never_exceeds_24db_and_output_never_clips() {
        let mut agc = Agc::new();
        let out = agc.process(&tone(0.0025, 10.0));
        assert!(agc.gain() <= MAX_GAIN + 1e-3);
        assert!(out.iter().all(|s| s.abs() <= 1.0));
        let loud: Vec<f32> = (0..16_000).map(|i| (i as f32 * 0.05).sin() * 3.0).collect();
        assert!(agc.process(&loud).iter().all(|s| s.abs() <= 1.0), "limiter must hold");
    }

    #[test]
    fn output_length_always_equals_input_length() {
        let mut agc = Agc::new();
        for n in [0usize, 1, 159, 160, 161, 1000, 4097] {
            assert_eq!(agc.process(&vec![0.01; n]).len(), n);
        }
    }

    #[test]
    fn a_sudden_shout_pulls_the_gain_down_fast() {
        let mut agc = Agc::new();
        agc.process(&tone(0.01, 5.0)); // learned a quiet speaker: high gain
        let high = agc.gain();
        agc.process(&tone(0.3, 0.3));
        assert!(agc.gain() < high * 0.6, "{} -> {}", high, agc.gain());
        assert!(agc.gain() >= 1.0, "must never attenuate below unity");
    }
}
