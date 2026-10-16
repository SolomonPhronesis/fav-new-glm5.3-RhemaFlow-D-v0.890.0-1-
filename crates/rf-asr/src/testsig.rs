//! Synthetic background signals for repeatable robustness tests and for
//! `rhemaflow-d benchmark --mix-noise / --mix-music`. They are STAND-INS:
//! real rooms, real worship bands and real HVAC are messier. Use them to compare
//! engines against each other under identical conditions, not to predict a sanctuary.

pub fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|s| s * s).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

/// Deterministic white noise in [-1, 1], scaled to the given RMS.
pub fn noise(n: usize, seed: u64, level_rms: f32) -> Vec<f32> {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1;
    let mut v: Vec<f32> = (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 11) as f32 / (1u64 << 53) as f32 * 2.0 - 1.0
        })
        .collect();
    scale_to(&mut v, level_rms);
    v
}

/// "Worship band": a sustained 5-note chord with a 2 Hz pulse.
pub fn music(n: usize, level_rms: f32) -> Vec<f32> {
    let mut v: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / 16_000.0;
            let pulse = 0.6 + 0.4 * (t * 2.0 * std::f32::consts::PI * 2.0).sin().abs();
            [220.0f32, 277.2, 329.6, 440.0, 554.4].iter().map(|f| (t * f * 2.0 * std::f32::consts::PI).sin()).sum::<f32>() * pulse
        })
        .collect();
    scale_to(&mut v, level_rms);
    v
}

fn scale_to(v: &mut [f32], target: f32) {
    let r = rms(v);
    if r > 0.0 {
        v.iter_mut().for_each(|s| *s *= target / r);
    }
}

/// Mix `background` under `speech` so that speech RMS / background RMS equals `snr_db`.
/// Output length = speech length (background is repeated/cut as needed).
pub fn mix_at_snr(speech: &[f32], background: &[f32], snr_db: f32) -> Vec<f32> {
    if background.is_empty() || speech.is_empty() {
        return speech.to_vec();
    }
    let sp = rms(speech);
    let target_bg = sp / 10f32.powf(snr_db / 20.0);
    let bg_rms = rms(background).max(1e-9);
    let k = target_bg / bg_rms;
    speech.iter().enumerate().map(|(i, s)| s + background[i % background.len()] * k).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speechlike(n: usize) -> Vec<f32> {
        (0..n).map(|i| (i as f32 * 0.05).sin() * 0.2 * (0.5 + 0.5 * (i as f32 / 4000.0).sin())).collect()
    }

    #[test]
    fn noise_and_music_hit_their_requested_level() {
        assert!((rms(&noise(48_000, 1, 0.02)) - 0.02).abs() < 1e-4);
        assert!((rms(&music(48_000, 0.05)) - 0.05).abs() < 1e-4);
    }

    #[test]
    fn mixing_gives_the_requested_snr() {
        let s = speechlike(48_000);
        for snr in [20.0f32, 10.0, 5.0, 0.0] {
            let m = mix_at_snr(&s, &noise(48_000, 7, 0.1), snr);
            let diff: Vec<f32> = m.iter().zip(&s).map(|(a, b)| a - b).collect();
            let measured = 20.0 * (rms(&s) / rms(&diff)).log10();
            assert!((measured - snr).abs() < 0.05, "asked {snr} dB got {measured}");
        }
    }

    #[test]
    fn short_backgrounds_loop_and_empty_ones_are_ignored() {
        let s = speechlike(10_000);
        assert_eq!(mix_at_snr(&s, &noise(100, 2, 0.1), 10.0).len(), 10_000);
        assert_eq!(mix_at_snr(&s, &[], 10.0), s);
    }

    #[test]
    fn noise_is_deterministic_per_seed() {
        assert_eq!(noise(1000, 5, 0.1), noise(1000, 5, 0.1));
        assert_ne!(noise(1000, 5, 0.1), noise(1000, 6, 0.1));
    }
}
