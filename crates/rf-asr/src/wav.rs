//! WAV file → 16 kHz mono f32, the format every engine expects.

use crate::{resampler, WHISPER_RATE};

/// Read any PCM/float WAV, downmix to mono and resample to 16 kHz.
pub fn load_16k_mono(path: &str) -> anyhow::Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path).map_err(|e| anyhow::anyhow!("cannot open '{path}': {e}"))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, _) => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
        (hound::SampleFormat::Int, 16) => reader.samples::<i16>().filter_map(|s| s.ok()).map(|s| s as f32 / 32768.0).collect(),
        (hound::SampleFormat::Int, b) if b <= 32 => {
            let scale = (1u64 << (b - 1)) as f32;
            reader.samples::<i32>().filter_map(|s| s.ok()).map(|s| s as f32 / scale).collect()
        }
        (_, b) => anyhow::bail!("unsupported WAV bit depth: {b}"),
    };
    let mono = if spec.channels > 1 { resampler::to_mono(&samples, spec.channels as usize) } else { samples };
    Ok(resampler::resample(&mono, spec.sample_rate, WHISPER_RATE))
}
