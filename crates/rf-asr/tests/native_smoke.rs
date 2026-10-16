//! Integration tests for the native engines (Whisper + Parakeet on one shared
//! whisper.cpp/ggml build). Tests that need a model file skip with a message
//! when it isn't present, so a fresh checkout still passes.

#![cfg(feature = "native")]

use rf_asr::{AsrEngine, EngineKind, ParakeetAsr, WhisperAsr};
use std::path::PathBuf;

const VENDOR: &str = "third_party/whisper.cpp-307869af285d";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn vendor_model(name: &str) -> Option<String> {
    let p = root().join(VENDOR).join("models").join(name);
    p.exists().then(|| p.display().to_string())
}
fn real_whisper_model() -> Option<String> {
    let p = root().join("models/ggml-tiny.en.bin");
    p.exists().then(|| p.display().to_string())
}
fn fixture(name: &str) -> Option<String> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    p.exists().then(|| p.display().to_string())
}
fn silence(secs: f32) -> Vec<f32> {
    vec![0.0; (16_000.0 * secs) as usize]
}

#[test]
fn garbage_model_files_are_errors_not_crashes() {
    let dir = std::env::temp_dir().join("rf-asr-garbage-model");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("not-a-model.bin");
    std::fs::write(&f, b"this is definitely not a ggml model").unwrap();
    let f = f.display().to_string();
    assert!(WhisperAsr::load(&f).is_err(), "whisper must reject a junk file");
    assert!(ParakeetAsr::load(&f).is_err(), "parakeet must reject a junk file");
    assert!(WhisperAsr::load("/no/such/model.bin").is_err());
    assert!(ParakeetAsr::load("/no/such/model.bin").is_err());
}

#[test]
fn parakeet_ci_stub_model_loads_and_runs() {
    let Some(m) = vendor_model("for-tests-ggml-parakeet-tdt.bin") else {
        eprintln!("SKIP: parakeet CI stub model not vendored");
        return;
    };
    let mut p = ParakeetAsr::load(&m).expect("stub model must load");
    // A stub model has random weights: we assert "runs to completion", not text.
    let _ = p.transcribe(&silence(2.0));
    let _ = p.transcribe(&silence(1.0));
}

#[test]
fn whisper_ci_stub_model_loads_and_runs() {
    let Some(m) = vendor_model("for-tests-ggml-tiny.en.bin") else {
        eprintln!("SKIP: whisper CI stub model not vendored");
        return;
    };
    let mut w = WhisperAsr::load(&m).expect("stub model must load");
    let _ = w.transcribe(&silence(2.0));
}

#[test]
fn both_engines_coexist_in_one_process() {
    // The whole reason for ONE shared ggml build: both engines alive at once,
    // interleaved, in the same process.
    let (Some(pm), Some(wm)) = (vendor_model("for-tests-ggml-parakeet-tdt.bin"), vendor_model("for-tests-ggml-tiny.en.bin")) else {
        eprintln!("SKIP: stub models not vendored");
        return;
    };
    let mut p = ParakeetAsr::load(&pm).unwrap();
    let mut w = WhisperAsr::load(&wm).unwrap();
    for _ in 0..2 {
        let _ = w.transcribe(&silence(1.0));
        let _ = p.transcribe(&silence(1.0));
    }
}

#[test]
fn too_short_audio_is_ignored() {
    let Some(m) = vendor_model("for-tests-ggml-tiny.en.bin") else { return };
    let mut w = WhisperAsr::load(&m).unwrap();
    assert!(w.transcribe(&[]).is_empty());
    assert!(w.transcribe(&silence(0.1)).is_empty());
}

#[test]
fn engine_kind_loads_through_trait_object() {
    let Some(m) = vendor_model("for-tests-ggml-parakeet-tdt.bin") else { return };
    let mut e = EngineKind::Parakeet.load(&m).unwrap();
    assert!(e.name().contains("parakeet"));
    let _ = e.transcribe(&silence(1.0));
}

#[test]
fn real_whisper_model_transcribes_jfk() {
    let (Some(m), Some(wav)) = (real_whisper_model(), fixture("jfk-16k.wav")) else {
        eprintln!("SKIP: models/ggml-tiny.en.bin or fixture missing");
        return;
    };
    let samples = rf_asr::wav::load_16k_mono(&wav).unwrap();
    let mut w = WhisperAsr::load(&m).unwrap();
    let text: String = w.transcribe(&samples).iter().map(|s| s.text.clone()).collect::<Vec<_>>().join(" ").to_lowercase();
    eprintln!("jfk transcript: {text}");
    assert!(text.contains("ask not what your country can do for you"), "got: {text}");
}

#[test]
fn segment_times_are_milliseconds() {
    let (Some(m), Some(wav)) = (real_whisper_model(), fixture("jfk-16k.wav")) else { return };
    let samples = rf_asr::wav::load_16k_mono(&wav).unwrap();
    let dur_ms = (samples.len() as u64 * 1000) / 16_000;
    let mut w = WhisperAsr::load(&m).unwrap();
    let segs = w.transcribe(&samples);
    assert!(!segs.is_empty());
    let end = segs.iter().map(|s| s.end_ms).max().unwrap();
    // Whisper may end its last segment at the end of its fixed 30 s window, so
    // the upper bound is 30 s, not the clip length. The lower bound is what
    // proves the unit: raw 10 ms values would read ~1100 for this 11 s clip.
    assert!(end >= dur_ms * 9 / 10 && end <= 30_000, "end_ms={end} dur_ms={dur_ms}");
    assert!(segs.iter().all(|s| s.start_ms <= s.end_ms));
}
