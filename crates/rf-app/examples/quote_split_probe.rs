//! Diagnostic: reproduces a scripture quotation split across two utterances
//! by a natural pause, through the REAL pipeline — VAD, Whisper, and
//! `parse_with_recent` exactly as `RhemaFlowApp::handle_parse` calls it.
//! Not shipped as a product feature; a debugging/verification aid.
//!
//! Usage: cargo run --example quote_split_probe -- FIRST.wav SECOND.wav

use rf_asr::{AsrEngine, WhisperAsr};
use rf_core::parser::ParseContext;

fn transcribe(asr: &mut WhisperAsr, path: &str) -> String {
    let samples = rf_asr::wav::load_16k_mono(path).expect("load wav");
    let mut vad = rf_asr::vad::Vad::new();
    let mut utterances: Vec<Vec<f32>> = Vec::new();
    for chunk in samples.chunks(1600) {
        utterances.extend(vad.feed(chunk));
    }
    utterances.extend(vad.feed(&vec![0.0f32; 16_000]));
    let mut text = String::new();
    for u in &utterances {
        for seg in asr.transcribe(u) {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(seg.text.trim());
        }
    }
    text
}

fn main() -> anyhow::Result<()> {
    let model = std::env::var("RF_WHISPER_MODEL").unwrap_or_else(|_| "models/ggml-tiny.en.bin".into());
    let mut asr = WhisperAsr::load(&model)?;
    let bible = rf_bible::Bible::embedded();
    let ctx = ParseContext::default();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let (first_path, second_path) = (&args[0], &args[1]);

    let t1 = transcribe(&mut asr, first_path);
    println!("utterance 1 ({first_path}) whisper -> {t1:?}");
    let out1 = rf_core::parser::parse_with_recent(bible, &t1, &ctx, 0.75, &[]);
    println!("  alone -> detections={} confirms={} commands={}", out1.detections.len(), out1.confirms.len(), out1.commands.len());
    if !out1.is_empty() {
        println!("  (unexpected: utterance 1 alone already matched something — {out1:?})");
    }

    let t2 = transcribe(&mut asr, second_path);
    println!("utterance 2 ({second_path}) whisper -> {t2:?}");
    let out2_alone = rf_core::parse(bible, &t2, &ctx, 0.75);
    println!(
        "  alone (no recent context) -> detections={} confirms={}",
        out2_alone.detections.len(),
        out2_alone.confirms.len()
    );

    let out2_with_recent = rf_core::parser::parse_with_recent(bible, &t2, &ctx, 0.75, &[t1.as_str()]);
    println!(
        "  WITH utterance 1 as recent context -> detections={} confirms={}",
        out2_with_recent.detections.len(),
        out2_with_recent.confirms.len()
    );
    for d in &out2_with_recent.confirms {
        println!("  -> CONFIRM: {} (kind={:?}, confidence={:.2}, note={:?})", d.passage.label(), d.kind, d.confidence, d.note);
    }
    if out2_with_recent.confirms.is_empty() && out2_with_recent.detections.is_empty() {
        println!("  -> DID NOT RESOLVE");
    }
    Ok(())
}
