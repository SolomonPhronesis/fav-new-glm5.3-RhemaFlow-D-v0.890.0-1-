//! Diagnostic: stream ONE continuous, unbroken utterance through the real
//! pipeline the way the live app does — VAD, interim snapshots (PartialPacer),
//! fast Whisper decode, quotation matching — and report *when* each thing
//! appears, versus when the old "wait for the pause" design would have.
//!
//! Audio time is simulated at real-time pace; a decode keeps the worker
//! "busy" for as long as it actually took, so a slow machine is modelled
//! honestly (this sandbox is single-core; an M-series Mac is far quicker).
//!
//! Usage: cargo run --example live_stream_probe -- CLIP.wav

use rf_asr::vad::{PartialPacer, Vad};
use rf_asr::{AsrEngine, WhisperAsr};
use rf_core::parser::{parse_full, ParseContext};
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let model = std::env::var("RF_WHISPER_MODEL").unwrap_or_else(|_| "models/ggml-tiny.en.bin".into());
    let mut asr = WhisperAsr::load(&model)?;
    let bible = rf_bible::Bible::embedded();
    let ctx = ParseContext::default();
    let path = std::env::args().nth(1).expect("CLIP.wav");
    let samples = rf_asr::wav::load_16k_mono(&path)?;
    println!("clip: {:.1}s", samples.len() as f32 / 16_000.0);

    let mut vad = Vad::new();
    let mut pacer = PartialPacer::new();
    let mut t_ms: u64 = 0;
    let mut busy_until_ms: u64 = 0;
    let mut first_quote_ms: Option<u64> = None;
    let mut partials = 0;

    let mut tail = samples.clone();
    tail.extend(std::iter::repeat(0.0f32).take(16_000 * 2)); // trailing quiet ends the utterance
    for chunk in tail.chunks(320) {
        t_ms += 20;
        let finals = vad.feed(chunk);
        if !finals.is_empty() {
            let t0 = Instant::now();
            let text: String = finals.iter().flat_map(|u| asr.transcribe(u)).map(|s| s.text).collect::<Vec<_>>().join(" ");
            println!("[{:>5.1}s] FINAL   ({:.1}s decode) {:?}", t_ms as f32 / 1000.0, t0.elapsed().as_secs_f32(), text);
            pacer.reset();
            continue;
        }
        if let Some(snap) = vad.in_progress() {
            let busy = t_ms < busy_until_ms;
            if pacer.should_emit(snap.len(), busy, 0) {
                let t0 = Instant::now();
                let text: String = asr.transcribe_fast(snap).into_iter().map(|s| s.text).collect::<Vec<_>>().join(" ");
                let d = t0.elapsed();
                busy_until_ms = t_ms + d.as_millis() as u64;
                partials += 1;
                let out = parse_full(bible, &text, &ctx, 0.75, &[], None);
                let hit = out.confirms.iter().find(|c| c.kind == rf_core::DetectionKind::Quotation);
                println!(
                    "[{:>5.1}s] partial ({:.1}s decode) {:?}{}",
                    t_ms as f32 / 1000.0,
                    d.as_secs_f32(),
                    text.trim(),
                    hit.map(|h| format!("   ==> CONFIRM QUEUE: {}", h.passage.label())).unwrap_or_default()
                );
                if hit.is_some() && first_quote_ms.is_none() {
                    first_quote_ms = Some(t_ms);
                }
            }
        }
    }
    println!("\n{partials} interim decode(s).");
    match first_quote_ms {
        Some(ms) => println!("quotation first surfaced at {:.1}s (while still speaking).", ms as f32 / 1000.0),
        None => println!("quotation never surfaced from an interim."),
    }
    Ok(())
}
