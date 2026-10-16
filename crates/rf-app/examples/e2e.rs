//! End-to-end verification: WAV file -> whisper ASR -> parser -> verse text.
//! Run: cargo run --release -p rf-app --example e2e -- <model.ggml> <audio.wav>

use rf_asr::{resampler::resample, AsrEngine};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: e2e <model> <wav>");
        std::process::exit(2);
    }
    let (model, wav_path) = (&args[1], &args[2]);

    // 1. read + resample audio
    let mut reader = hound::WavReader::open(wav_path)?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / 32768.0)
            .collect(),
    };
    let mono = if spec.channels > 1 {
        rf_asr::resampler::to_mono(&samples, spec.channels as usize)
    } else {
        samples
    };
    let s16 = resample(&mono, spec.sample_rate, rf_asr::WHISPER_RATE);
    eprintln!(
        "[e2e] audio: {} Hz -> 16 kHz, {} samples ({:.2} s)",
        spec.sample_rate,
        s16.len(),
        s16.len() as f32 / 16_000.0
    );

    // 2. transcribe
    let t0 = std::time::Instant::now();
    let mut asr = rf_asr::WhisperAsr::load(model)?;
    let segments = asr.transcribe(&s16);
    let asr_ms = t0.elapsed().as_millis();
    let text: String = segments.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join(" ");
    eprintln!("[e2e] ASR ({asr_ms} ms): \"{text}\"");
    if text.trim().is_empty() {
        anyhow::bail!("ASR produced no text");
    }

    // 3. parse
    let bible = rf_bible::Bible::embedded();
    let outcome = rf_core::parse(bible, &text, &rf_core::ParseContext::default(), 0.75);
    println!("TRANSCRIPT: {text}");
    println!("DETECTIONS: {:?}", outcome.detections.iter().map(|d| d.passage.label()).collect::<Vec<_>>());
    println!("CONFIRMS:   {:?}", outcome.confirms.iter().map(|d| d.passage.label()).collect::<Vec<_>>());
    println!("ERRORS:     {:?}", outcome.errors.iter().map(|e| e.message.clone()).collect::<Vec<_>>());
    println!("COMMANDS:   {:?}", outcome.commands);
    if let Some(d) = outcome.detections.first() {
        let p = d.passage;
        println!(
            "STAGED: {} = {}",
            p.label(),
            truncate(&bible.passage(p.book as usize, p.chapter as usize, p.from as usize, p.to as usize), 90)
        );
    }
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}
