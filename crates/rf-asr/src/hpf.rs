//! High-pass front-end filter (v0.854.0): remove DC drift and low rumble
//! before the AGC and the ASR engine.
//!
//! Why: sanctuary audio carries energy the models cannot use but the rest of
//! the chain pays for — HVAC rumble, traffic, footfalls, mic-stand thumps, and
//! the DC offset every cheap ADC adds. Left in, it eats the AGC's gain budget
//! (the leveller mistakes rumble for speech floor) and shifts the VAD's
//! silence floor. A linear-phase-free 2nd-order Butterworth high-pass at
//! 75 Hz is inaudible to speech (fundamental of the lowest male voices ≈ 85 Hz,
//! and ASR does not use pitch), costs ~7 multiply-adds per sample, and is
//! fully deterministic — no lookahead, no latency.
//!
//! Implementation: RBJ biquad ("Cookbook formulae for audio EQ", high-pass,
//! Q = 1/√2 Butterworth), coefficients computed in f64, state in f32 — the
//! same trade-off whisper.cpp makes in its own pre-emphasis.

/// 2nd-order Butterworth high-pass.
pub struct Hpf {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Hpf {
    /// `cutoff_hz` −3 dB point, `rate` sample rate (e.g. 75 Hz @ 16 kHz).
    pub fn new(cutoff_hz: f32, rate: f32) -> Self {
        let f0 = cutoff_hz.max(1.0).min(rate * 0.45) as f64;
        let fs = rate.max(1.0) as f64;
        let w0 = std::f64::consts::TAU * f0 / fs;
        // Butterworth Q = 1/sqrt(2)
        let alpha = w0.sin() / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
        let cosw = w0.cos();
        let a0 = 1.0 + alpha;
        let (b0, b1, b2) = ((1.0 + cosw) / 2.0 / a0, -(1.0 + cosw) / a0, (1.0 + cosw) / 2.0 / a0);
        let (a1, a2) = (-2.0 * cosw / a0, (1.0 - alpha) / a0);
        Self { b0: b0 as f32, b1: b1 as f32, b2: b2 as f32, a1: a1 as f32, a2: a2 as f32, x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 }
    }

    /// Filter `samples`; output length always equals input length.
    pub fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        let mut out = Vec::with_capacity(samples.len());
        for &x in samples {
            let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
            self.x2 = self.x1;
            self.x1 = x;
            self.y2 = self.y1;
            self.y1 = y;
            out.push(y);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, secs: f32, amp: f32) -> Vec<f32> {
        (0..(secs * 16_000.0) as usize)
            .map(|i| amp * (std::f32::consts::TAU * freq * i as f32 / 16_000.0).sin())
            .collect()
    }
    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    #[test]
    fn dc_offset_is_removed() {
        let mut h = Hpf::new(75.0, 16_000.0);
        let out = h.process(&vec![0.5; 16_000]); // a full second of constant offset
        let tail = &out[out.len() - 1000..];
        assert!(tail.iter().all(|s| s.abs() < 1e-3), "max |y| = {}", tail.iter().fold(0.0f32, |a, b| a.max(b.abs())));
    }

    #[test]
    fn rumble_is_attenuated_and_speech_band_passes() {
        let mut h = Hpf::new(75.0, 16_000.0);
        // 30 Hz rumble: below cutoff -> strongly attenuated after settling
        let out = h.process(&tone(30.0, 1.0, 0.5));
        let ratio = rms(&out[out.len() / 2..]) / rms(&tone(30.0, 1.0, 0.5)[out.len() / 2..]);
        assert!(ratio < 0.25, "30 Hz should be attenuated, ratio = {ratio}");
        // 400 Hz: well inside the speech band -> essentially unchanged
        let mut h = Hpf::new(75.0, 16_000.0);
        let out = h.process(&tone(400.0, 1.0, 0.5));
        let ratio = rms(&out[out.len() / 2..]) / rms(&tone(400.0, 1.0, 0.5)[out.len() / 2..]);
        assert!((0.9..=1.1).contains(&ratio), "400 Hz should pass, ratio = {ratio}");
    }

    #[test]
    fn length_is_preserved_and_output_is_finite() {
        let mut h = Hpf::new(75.0, 16_000.0);
        let x = tone(120.0, 0.3, 0.2);
        let out = h.process(&x);
        assert_eq!(out.len(), x.len());
        assert!(out.iter().all(|s| s.is_finite()));
        // empty input -> empty output
        assert!(h.process(&[]).is_empty());
    }
}
