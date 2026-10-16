//! END-TO-END, MEASURED: real speech -> (quiet mic / music / noise) -> AGC -> VAD
//! -> real Whisper tiny.en -> word accuracy against the known text.
//! One speaker (JFK), one model (tiny.en), CPU. It shows what the new front end
//! changes; it is NOT a claim about your voice - run `rhemaflow-d calibrate` for that.
#![cfg(feature = "native")]

use rf_asr::agc::Agc;
use rf_asr::vad::Vad;
use rf_asr::{AsrEngine, WhisperAsr};
use std::path::PathBuf;

const TEXT: &str = "and so my fellow americans ask not what your country can do for you ask what you can do for your country";

fn model() -> Option<String> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models/ggml-tiny.en.bin");
    p.exists().then(|| p.display().to_string())
}
fn jfk() -> Vec<f32> {
    let p = format!("{}/tests/fixtures/jfk-16k.wav", env!("CARGO_MANIFEST_DIR"));
    rf_asr::wav::load_16k_mono(&p).unwrap()
}
fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|s| s * s).sum::<f32>() / x.len().max(1) as f32).sqrt()
}
fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect()
}
/// word accuracy = 1 - WER (edit distance over words)
fn accuracy(reference: &[String], hyp: &[String]) -> f32 {
    let (n, m) = (reference.len(), hyp.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..=n {
        d[i][0] = i;
    }
    for j in 0..=m {
        d[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let c = (reference[i - 1] != hyp[j - 1]) as usize;
            d[i][j] = (d[i - 1][j - 1] + c).min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
        }
    }
    (1.0 - d[n][m] as f32 / n as f32).max(0.0)
}

/// the capture thread's chain, then real Whisper on each finished utterance
fn transcribe(engine: &mut WhisperAsr, audio: &[f32], use_agc: bool) -> (String, usize) {
    let mut vad = Vad::new();
    let mut agc = Agc::new();
    let mut utts = Vec::new();
    let mut all = vec![0.0f32; 16_000];
    all.extend_from_slice(audio);
    all.extend(vec![0.0f32; 20_000]);
    for chunk in all.chunks(320) {
        let c = if use_agc { agc.process(chunk) } else { chunk.to_vec() };
        for (u, _) in vad.feed_with_meta(&c) {
            utts.push(u);
        }
    }
    let text = utts.iter().map(|u| engine.transcribe(u).iter().map(|s| s.text.trim().to_string()).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join(" ");
    (text, utts.len())
}

#[test]
fn measured_word_accuracy_through_the_whole_front_end() {
    let Some(m) = model() else {
        eprintln!("skipped: models/ggml-tiny.en.bin not present");
        return;
    };
    let mut engine = WhisperAsr::load(&m).unwrap();
    let reference = words(TEXT);
    println!("\nlevel / condition                       pieces  word accuracy");
    let mut results = Vec::new();
    for (label, gain) in [("normal mic", 1.0f32), ("quiet mic x0.1", 0.1), ("very quiet mic x0.05", 0.05)] {
        if let Ok(only) = std::env::var("RF_ACC_ONLY") {
            if !label.contains(&only) {
                continue;
            }
        }
        let a: Vec<f32> = jfk().iter().map(|s| s * gain).collect();
        for use_agc in [false, true] {
            let (text, pieces) = transcribe(&mut engine, &a, use_agc);
            let acc = accuracy(&reference, &words(&text));
            println!("{label:<24} AGC {:<5}           {pieces:>3}    {:>5.1}%   {:?}", if use_agc { "on" } else { "off" }, acc * 100.0, text.trim());
            results.push((label, use_agc, acc));
        }
    }
    // the front end must never make things worse, and must fix the quiet case
    if std::env::var("RF_ACC_ONLY").is_ok() {
        return;
    }
    let get = |l: &str, agc: bool| results.iter().find(|r| r.0 == l && r.1 == agc).unwrap().2;
    assert!(get("normal mic", true) >= get("normal mic", false) - 0.05);
    assert!(get("quiet mic x0.1", true) >= get("quiet mic x0.1", false));
    assert!(get("very quiet mic x0.05", true) >= get("very quiet mic x0.05", false));
    assert!(get("very quiet mic x0.05", true) >= 0.8, "with the new front end even a very quiet mic should stay above 80% on this clip");
}

/// MEASURED: the same clip with steady noise and a music bed mixed in at several
/// speech-to-background ratios, through AGC -> VAD -> real Whisper tiny.en.
/// Filter with RF_NOISE_ONLY=<substring> (each decode is slow on a 1-core CPU).
#[test]
fn measured_accuracy_with_noise_and_music() {
    use rf_asr::testsig::{mix_at_snr, music, noise};
    let Some(m) = model() else {
        eprintln!("skipped: models/ggml-tiny.en.bin not present");
        return;
    };
    let mut engine = WhisperAsr::load(&m).unwrap();
    let reference = words(TEXT);
    let clean = jfk();
    let n = clean.len();
    let cases: Vec<(String, Vec<f32>)> = vec![
        ("clean".into(), clean.clone()),
        ("noise 20 dB".into(), mix_at_snr(&clean, &noise(n, 11, 0.05), 20.0)),
        ("noise 10 dB".into(), mix_at_snr(&clean, &noise(n, 11, 0.05), 10.0)),
        ("noise 5 dB".into(), mix_at_snr(&clean, &noise(n, 11, 0.05), 5.0)),
        ("music 20 dB".into(), mix_at_snr(&clean, &music(n, 0.05), 20.0)),
        ("music 10 dB".into(), mix_at_snr(&clean, &music(n, 0.05), 10.0)),
        ("music 5 dB".into(), mix_at_snr(&clean, &music(n, 0.05), 5.0)),
    ];
    println!("\ncondition        pieces  accuracy   decode_s  audio_s  RTF");
    for (label, audio) in cases {
        if let Ok(only) = std::env::var("RF_NOISE_ONLY") {
            if !only.split(',').any(|o| label == o.trim()) {
                continue;
            }
        }
        let t = std::time::Instant::now();
        let (text, pieces) = transcribe(&mut engine, &audio, true);
        let dec = t.elapsed().as_secs_f32();
        let secs = audio.len() as f32 / 16_000.0;
        let acc = accuracy(&reference, &words(&text));
        println!("{label:<16} {pieces:>5}   {:>6.1}%   {dec:>7.1}  {secs:>6.1}  {:.2}   {:?}", acc * 100.0, dec / secs, text.trim());
    }
    let _ = rms(&clean);
}

/// Records REAL interim hypotheses (what the app's live path sees while someone is
/// still talking) so the live-text logic can be tested on genuine Whisper output.
///   RF_REC_COND=clean|noise10  RF_REC_STEPS=0,1,2  cargo test -p rf-asr --release --test pipeline_accuracy record_interims -- --ignored
#[test]
#[ignore]
fn record_interims() {
    use rf_asr::testsig::{mix_at_snr, noise};
    use std::io::Write;
    let Some(m) = model() else { return };
    let cond = std::env::var("RF_REC_COND").unwrap_or_else(|_| "clean".into());
    let steps: Vec<usize> = std::env::var("RF_REC_STEPS").unwrap_or_else(|_| "0".into()).split(',').filter_map(|s| s.trim().parse().ok()).collect();
    let mut engine = WhisperAsr::load(&m).unwrap();
    let clean = jfk();
    let audio = if cond == "noise10" { mix_at_snr(&clean, &noise(clean.len(), 11, 0.05), 10.0) } else { clean };
    let ends = [2.0f32, 3.5, 5.0, 6.5, 8.0, 9.5, 11.0];
    let path = format!("{}/tests/fixtures/jfk-interims.jsonl", env!("CARGO_MANIFEST_DIR"));
    for s in steps {
        let end = ((ends[s] * 16_000.0) as usize).min(audio.len());
        let t = std::time::Instant::now();
        let text = engine.transcribe_fast(&audio[..end]).iter().map(|x| x.text.trim().to_string()).collect::<Vec<_>>().join(" ");
        let line = serde_json::json!({"cond": cond, "step": s, "secs": ends[s], "decode_s": t.elapsed().as_secs_f32(), "text": text});
        println!("{line}");
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
        writeln!(f, "{line}").unwrap();
    }
}
