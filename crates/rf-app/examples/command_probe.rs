//! Diagnostic: feed a WAV file through the REAL pipeline — VAD segmentation,
//! then Whisper transcription, then the command parser — and report exactly
//! where a short voice command survives or dies. Not shipped as a product
//! feature; a debugging aid for "next/previous/read/fullscreen aren't firing."
//!
//! Usage: cargo run --example command_probe -- FILE.wav [FILE2.wav ...]

use rf_asr::{AsrEngine, WhisperAsr};

fn main() -> anyhow::Result<()> {
    let model = std::env::var("RF_WHISPER_MODEL").unwrap_or_else(|_| "models/ggml-tiny.en.bin".into());
    let mut asr = WhisperAsr::load(&model)?;
    let bible = rf_bible::Bible::embedded();
    let ctx = rf_core::parser::ParseContext::default();

    for path in std::env::args().skip(1) {
        let samples = rf_asr::wav::load_16k_mono(&path)?;
        println!("=== {path} ({:.2}s @16kHz) ===", samples.len() as f32 / 16_000.0);

        // 1) VAD — feed the whole clip through as a live mic stream would.
        let mut vad = rf_asr::vad::Vad::new();
        let mut utterances: Vec<Vec<f32>> = Vec::new();
        for chunk in samples.chunks(1600) {
            // 100ms chunks, like the real capture worker's ~20ms polling would
            // eventually deliver in aggregate
            utterances.extend(vad.feed(chunk));
        }
        // flush: real mic streams always end on live silence, so force it
        utterances.extend(vad.feed(&vec![0.0f32; 16_000])); // 1s flush
        println!(
            "  VAD: {} utterance(s) kept, {} dropped (too short/quiet)",
            utterances.len(),
            vad.dropped_count
        );
        if utterances.is_empty() && vad.dropped_count > 0 {
            println!("  -> DIED IN VAD: discarded as too short/quiet ({}ms)", vad.last_dropped_ms);
            continue;
        }
        if utterances.is_empty() {
            println!("  -> DIED IN VAD: no speech onset detected at all");
            continue;
        }

        // 2) Whisper — transcribe each surviving utterance.
        for (i, u) in utterances.iter().enumerate() {
            let segs = asr.transcribe(u);
            let text: String = segs.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join(" ");
            println!("  utterance[{i}] ({:.0}ms audio) whisper -> {:?}", u.len() as f32 / 16.0, text.trim());
            if text.trim().is_empty() {
                println!("    -> DIED IN WHISPER: empty transcript");
                continue;
            }

            // 3) Parser — does it produce a Command?
            let out = rf_core::parse(bible, text.trim(), &ctx, 0.75);
            if out.commands.is_empty() {
                println!("    -> DIED IN PARSER: no command recognized from {:?}", text.trim());
            } else {
                println!("    -> COMMAND(S): {:?}", out.commands);
            }
        }
    }
    Ok(())
}
