//! v0.818.0 microphone check. Written BEFORE the code. Real speech (jfk-16k.wav, 11 s) plus deliberately broken copies of it.
use rf_asr::micreport::{analyze, Level, MicReport};
use rf_asr::testsig;
use rf_asr::wav::load_16k_mono;

const RATE: u32 = 16_000;

fn jfk() -> Vec<f32> {
    load_16k_mono(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/jfk-16k.wav")).unwrap()
}
fn scale(x: &[f32], g: f32) -> Vec<f32> {
    x.iter().map(|s| (s * g).clamp(-1.0, 1.0)).collect()
}
fn level(r: &MicReport, name: &str) -> Level {
    r.findings.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("no finding {name:?} in {:?}", r.findings.iter().map(|f| f.name).collect::<Vec<_>>())).level
}
fn has(r: &MicReport, name: &str) -> bool {
    r.findings.iter().any(|f| f.name == name)
}
/// Add a steady tone at `hz` with the given RMS.
fn add_tone(x: &[f32], hz: f32, rms: f32) -> Vec<f32> {
    x.iter().enumerate().map(|(i, s)| s + rms * 2f32.sqrt() * (2.0 * std::f32::consts::PI * hz * i as f32 / RATE as f32).sin()).collect()
}

#[test]
fn real_speech_at_a_sensible_level_is_fine() {
    let r = analyze(&jfk(), RATE); // a real recording of a speaker, as it is
    println!("{}", r.to_text());
    assert!(r.seconds > 10.0 && r.seconds < 12.0);
    assert_eq!(level(&r, "Loudness"), Level::Good, "{}", r.to_text());
    assert_eq!(level(&r, "Distortion"), Level::Good);
    assert_ne!(r.verdict, Level::Bad, "{}", r.to_text());
    assert!(r.speech_seconds > 4.0, "{}", r.speech_seconds);
}

#[test]
fn a_very_quiet_mic_is_found_and_the_advice_says_what_to_do() {
    let r = analyze(&scale(&jfk(), 0.03), RATE);
    assert_eq!(level(&r, "Loudness"), Level::Bad, "{}", r.to_text());
    let f = r.findings.iter().find(|f| f.name == "Loudness").unwrap();
    assert!(f.advice.to_lowercase().contains("closer") && f.advice.to_lowercase().contains("gain"), "{}", f.advice);
    assert_eq!(r.verdict, Level::Bad);
}

#[test]
fn a_clipping_mic_is_found() {
    let r = analyze(&scale(&jfk(), 60.0), RATE);
    assert_eq!(level(&r, "Distortion"), Level::Bad, "{}", r.to_text());
    let f = r.findings.iter().find(|f| f.name == "Distortion").unwrap();
    assert!(f.advice.to_lowercase().contains("gain") || f.advice.to_lowercase().contains("lower"), "{}", f.advice);
}

#[test]
fn a_noisy_room_is_found_by_how_far_the_voice_stands_above_it() {
    let speech = jfk();
    let quiet_room = analyze(&speech, RATE);
    // background noise only 8 dB below the voice (a fan, a crowd)
    let bg = testsig::noise(speech.len(), 3, 1.0);
    let noisy = testsig::mix_at_snr(&speech, &bg, 8.0);
    let r = analyze(&noisy, RATE);
    assert_eq!(level(&r, "Background noise"), Level::Bad, "{}", r.to_text());
    assert!(level(&quiet_room, "Background noise") != Level::Bad, "{}", quiet_room.to_text());
    let f = r.findings.iter().find(|f| f.name == "Background noise").unwrap();
    assert!(f.advice.to_lowercase().contains("closer") || f.advice.to_lowercase().contains("lapel") || f.advice.to_lowercase().contains("headset"), "{}", f.advice);
}

/// A stand-in voice: half-second bursts of noise at -20 dBFS over a very quiet floor (-66 dBFS).
fn synthetic_voice(seconds: usize) -> Vec<f32> {
    let n = seconds * RATE as usize;
    let floor = testsig::noise(n, 11, 0.0005);
    let burst = testsig::noise(n, 12, 0.1);
    (0..n).map(|i| floor[i] + if (i / (RATE as usize / 2)) % 2 == 0 { burst[i] } else { 0.0 }).collect()
}

#[test]
fn mains_hum_is_found_and_graded_by_how_close_it_is_to_the_voice_at_50_and_60_hz() {
    let voice = synthetic_voice(12);
    let clean = analyze(&voice, RATE);
    assert!(!has(&clean, "Hum"), "false alarm on a clean signal: {}", clean.to_text());
    for hz in [50.0, 60.0] {
        // (hum rms, expected). The voice is at -20 dB.
        let loud = analyze(&add_tone(&voice, hz, 0.03), RATE); // -30 dB: 10 dB under the voice
        assert_eq!(level(&loud, "Hum"), Level::Bad, "{hz}: {}", loud.to_text());
        let f = loud.findings.iter().find(|f| f.name == "Hum").unwrap();
        assert!(f.value.contains(&format!("{hz:.0}")), "{}", f.value);
        assert!(f.advice.to_lowercase().contains("cable") && f.advice.to_lowercase().contains("ground"), "{}", f.advice);
        let mild = analyze(&add_tone(&voice, hz, 0.008), RATE); // -42 dB: 22 under the voice
        assert_eq!(level(&mild, "Hum"), Level::Warn, "{hz}: {}", mild.to_text());
        let faint = analyze(&add_tone(&voice, hz, 0.002), RATE); // -54 dB: 34 under the voice
        assert!(!has(&faint, "Hum"), "{hz}: faint hum must not be reported: {}", faint.to_text());
    }
}

#[test]
fn hum_on_a_real_recording_is_found_when_it_competes_with_the_voice() {
    let r = analyze(&add_tone(&jfk(), 50.0, 0.03), RATE);
    assert_eq!(level(&r, "Hum"), Level::Bad, "{}", r.to_text());
}

#[test]
fn a_hum_far_quieter_than_the_voice_is_not_a_problem_and_is_not_reported() {
    // The 1961 recording used here REALLY has 60 Hz hum (confirmed with an independent FFT). It sits
    // about 35 dB under the voice and is harmless: the check must not cry wolf.
    let clean = analyze(&jfk(), RATE);
    assert!(!has(&clean, "Hum"), "false alarm: {}", clean.to_text());
    let faint = analyze(&add_tone(&jfk(), 50.0, 0.004), RATE); // -48 dBFS
    assert!(!has(&faint, "Hum"), "{}", faint.to_text());
}

#[test]
fn a_constant_offset_is_found() {
    let x: Vec<f32> = jfk().iter().map(|s| s + 0.08).collect();
    let r = analyze(&x, RATE);
    assert_ne!(level(&r, "DC offset"), Level::Good, "{}", r.to_text());
}

#[test]
fn silence_and_a_muted_mic_say_so_plainly() {
    let r = analyze(&vec![0.0; 10 * RATE as usize], RATE);
    assert_eq!(r.verdict, Level::Bad);
    assert!(has(&r, "Speech"), "{}", r.to_text());
    let f = r.findings.iter().find(|f| f.name == "Speech").unwrap();
    assert!(f.advice.to_lowercase().contains("mute") || f.advice.to_lowercase().contains("plugged"), "{}", f.advice);
    // only a little hiss, no voice
    let hiss = analyze(&testsig::noise(10 * RATE as usize, 5, 0.002), RATE);
    assert_eq!(level(&hiss, "Speech"), Level::Bad, "{}", hiss.to_text());
}

#[test]
fn a_recording_that_is_too_short_to_judge_says_so() {
    let r = analyze(&jfk()[..RATE as usize * 2], RATE);
    assert_eq!(r.verdict, Level::Bad);
    assert!(r.findings.iter().any(|f| f.name == "Length"), "{}", r.to_text());
    let empty = analyze(&[], RATE);
    assert_eq!(empty.verdict, Level::Bad);
}

#[test]
fn odd_numbers_never_panic() {
    for x in [vec![f32::NAN; 40_000], vec![f32::INFINITY; 40_000], vec![1.0; 40_000], vec![-1.0, 1.0].repeat(30_000)] {
        let r = analyze(&x, RATE);
        let _ = r.to_text();
    }
    let _ = analyze(&jfk(), 8_000);
    let _ = analyze(&jfk(), 48_000);
    let _ = analyze(&jfk(), 0);
}

#[test]
fn the_report_reads_in_plain_english() {
    let r = analyze(&scale(&jfk(), 0.03), RATE);
    let t = r.to_text();
    assert!(t.contains("Loudness") && t.contains("Verdict"), "{t}");
    assert!(!t.contains("NaN") && !t.contains("inf"), "{t}");
}
