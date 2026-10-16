//! Regression tests for "it stops hearing me after a second".
//! REAL human speech (JFK + two scripture fixtures) through the production
//! chain: AGC -> Vad -> PartialPacer, exactly as the capture thread runs it.
use rf_asr::agc::Agc;
use rf_asr::vad::{PartialPacer, Vad};

fn fixture(name: &str) -> Vec<f32> {
    let p = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    rf_asr::wav::load_16k_mono(&p).expect("fixture")
}
fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|s| s * s).sum::<f32>() / x.len().max(1) as f32).sqrt()
}
fn gap(ms: usize) -> Vec<f32> {
    vec![0.0; 16 * ms]
}
/// deterministic pseudo-random in [-1, 1]
fn noise(n: usize, seed: u64) -> Vec<f32> {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 11) as f32 / (1u64 << 53) as f32 * 2.0 - 1.0
        })
        .collect()
}
/// "Worship band": sustained chords with a pulse, plus a bass line.
fn music(n: usize, level_rms: f32) -> Vec<f32> {
    let mut v: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / 16_000.0;
            let chord = [220.0f32, 277.2, 329.6, 440.0, 554.4];
            let pulse = 0.6 + 0.4 * (t * 2.0 * std::f32::consts::PI * 2.0).sin().abs();
            chord.iter().map(|f| (t * f * 2.0 * std::f32::consts::PI).sin()).sum::<f32>() * pulse
        })
        .collect();
    let r = rms(&v);
    v.iter_mut().for_each(|s| *s *= level_rms / r);
    v
}
fn mix(a: &[f32], b: &[f32]) -> Vec<f32> {
    (0..a.len().max(b.len())).map(|i| a.get(i).copied().unwrap_or(0.0) + b.get(i).copied().unwrap_or(0.0)).collect()
}

struct Result {
    finals: usize,
    final_secs: Vec<f32>,
    speaking_secs: f32,
    first_final_at: Option<f32>,
    /// speaking time after the first 10 s (steady state, ignoring the start-up transient)
    late_speaking_secs: f32,
    /// (start, end) of every returned utterance, in samples of the audio passed in
    ranges: Vec<(u64, u64)>,
}

/// The capture thread's loop, in 20 ms steps.
fn run(audio: &[f32], use_agc: bool) -> Result {
    let mut vad = Vad::new();
    let mut agc = Agc::new();
    let mut pacer = PartialPacer::new();
    let mut r = Result { finals: 0, final_secs: vec![], speaking_secs: 0.0, first_final_at: None, late_speaking_secs: 0.0, ranges: vec![] };
    let lead = gap(1000);
    let tail = gap(1300);
    let all: Vec<f32> = lead.iter().chain(audio.iter()).chain(tail.iter()).copied().collect();
    for (i, chunk) in all.chunks(320).enumerate() {
        let t = i as f32 * 0.02 - 1.0;
        let c = if use_agc { agc.process(chunk) } else { chunk.to_vec() };
        for (u, meta) in vad.feed_with_meta(&c) {
            r.ranges.push((meta.start_sample.saturating_sub(16_000), meta.end_sample.saturating_sub(16_000)));
            r.finals += 1;
            r.final_secs.push(u.len() as f32 / 16_000.0);
            r.first_final_at.get_or_insert(t);
            pacer.reset();
        }
        if let Some(s) = vad.in_progress() {
            let _ = pacer.should_emit(s.len(), false, 0);
        }
        if vad.speaking() {
            r.speaking_secs += 0.02;
            if t > 10.0 {
                r.late_speaking_secs += 0.02;
            }
        }
    }
    r
}

/// Share of the clearly-spoken 10 ms frames of `clean` (placed `offset` samples into
/// the tested stream) that ended up inside a returned utterance.
fn coverage(clean: &[f32], offset: usize, ranges: &[(u64, u64)]) -> f32 {
    let (mut spoken, mut covered) = (0usize, 0usize);
    for (i, f) in clean.chunks_exact(160).enumerate() {
        if rms(f) > 0.03 {
            spoken += 1;
            let pos = (offset + i * 160 + 80) as u64;
            if ranges.iter().any(|(a, b)| pos >= *a && pos < *b) {
                covered += 1;
            }
        }
    }
    covered as f32 / spoken.max(1) as f32
}

fn jfk() -> Vec<f32> {
    fixture("jfk-16k.wav")
}

/// Remove pauses longer than 60 ms so the speaker never really stops.
fn no_pauses(x: &[f32]) -> Vec<f32> {
    let mut out = Vec::new();
    let mut quiet_run = 0usize;
    for f in x.chunks(160) {
        if rms(f) < 0.01 {
            quiet_run += 1;
            if quiet_run <= 6 {
                out.extend_from_slice(f);
            }
        } else {
            quiet_run = 0;
            out.extend_from_slice(f);
        }
    }
    out
}

#[test]
fn quiet_microphones_are_not_chopped_into_fragments() {
    let clean = run(&jfk(), true);
    println!("\nlevel      finals  speaking   (clean: {} finals, {:.1}s)", clean.finals, clean.speaking_secs);
    for gain in [1.0f32, 0.25, 0.1, 0.05] {
        let q: Vec<f32> = jfk().iter().map(|s| s * gain).collect();
        let with = run(&q, true);
        let without = run(&q, false);
        println!("x{gain:<5} RMS {:.4}  AGC: {} finals {:.1}s   no AGC: {} finals {:.1}s", rms(&q), with.finals, with.speaking_secs, without.finals, without.speaking_secs);
        let cov_with = coverage(&jfk(), 0, &with.ranges);
        let cov_without = coverage(&jfk(), 0, &without.ranges);
        println!("        share of clearly-spoken audio that reached the recogniser: AGC {:.0}%   no AGC {:.0}%", cov_with * 100.0, cov_without * 100.0);
        assert!(cov_with >= if gain < 0.07 { 0.85 } else { 0.95 }, "x{gain}: only {:.0}% of the speech reached the recogniser", cov_with * 100.0);
        assert!(cov_with + 0.001 >= cov_without, "AGC must never make it worse");
        assert!(with.finals <= clean.finals + 3, "x{gain}: speech shattered into {} pieces (clean {})", with.finals, clean.finals);
    }
}

#[test]
fn sixty_seconds_of_continuous_talking_is_transcribed_in_pieces_not_lost() {
    let mut talk = Vec::new();
    while talk.len() < 16_000 * 62 {
        talk.extend(no_pauses(&jfk()));
        talk.extend(gap(60));
    }
    let r = run(&talk, true);
    let talk_secs = talk.len() as f32 / 16_000.0;
    println!("\n60 s non-stop: input {talk_secs:.0}s -> finals {} lengths {:?} first at {:?}", r.finals, r.final_secs.iter().map(|s| (s * 10.0).round() / 10.0).collect::<Vec<_>>(), r.first_final_at);
    assert!(r.first_final_at.unwrap() <= 13.5, "first text must arrive within ~13 s of non-stop talking");
    assert!(r.final_secs.iter().all(|s| *s <= 14.0), "no piece may exceed ~14 s: {:?}", r.final_secs);
    let delivered: f32 = r.final_secs.iter().sum();
    assert!(delivered >= talk_secs * 0.85, "delivered only {delivered:.1}s of {talk_secs:.1}s");
}

#[test]
fn background_noise_does_not_trigger_or_hide_the_speaker() {
    let speech = jfk();
    let sp_rms = rms(&speech);
    for snr_db in [20.0f32, 10.0] {
        let n_rms = sp_rms / 10f32.powf(snr_db / 20.0);
        let n: Vec<f32> = noise(speech.len() + 32_000, 7).iter().map(|s| s * n_rms * 1.7).collect();
        let noisy: Vec<f32> = mix(&speech, &n);
        let clean = run(&speech, true);
        let r = run(&noisy, true);
        println!("noise SNR {snr_db} dB: heard {:.1}s of {:.1}s clean, finals {}", r.speaking_secs, clean.speaking_secs, r.finals);
        let cov = coverage(&speech, 0, &r.ranges);
        println!("   share of clearly-spoken audio that reached the recogniser: {:.0}%", cov * 100.0);
        assert!(cov >= 0.95, "SNR {snr_db}: only {:.0}% of the speech reached the recogniser", cov * 100.0);
        let _ = &clean;
    }
    // noise only (a fan / HVAC at a realistic level): must stay "not speaking"
    for level in [0.003f32, 0.01, 0.03] {
        let n: Vec<f32> = noise(16_000 * 20, 3).iter().map(|s| s * level * 1.7).collect();
        let r = run(&n, true);
        println!("noise only, RMS {level}: speaking {:.2}s of 20s ({:.2}s after the first 10 s)", r.speaking_secs, r.late_speaking_secs);
        assert!(r.late_speaking_secs < 0.2, "steady noise at {level} still heard as speech after 10 s: {:.1}s", r.late_speaking_secs);
        assert!(r.speaking_secs < 4.0, "noise at {level} was heard as speech for {:.1}s", r.speaking_secs);
    }
}

#[test]
fn steady_background_music_is_learned_and_speech_over_it_is_still_heard() {
    // music alone: after a short warm-up the floor learns it
    let m = music(16_000 * 25, 0.03);
    let r = run(&m, true);
    println!("music only: speaking {:.1}s of 25s ({:.1}s after the first 10 s), finals {}", r.speaking_secs, r.late_speaking_secs, r.finals);
    assert!(r.late_speaking_secs < 0.5, "steady music still heard as speech after 10 s: {:.1}s", r.late_speaking_secs);
    assert!(r.speaking_secs < 8.0, "music was treated as speech for {:.1}s", r.speaking_secs);

    // speech 10 dB above the music, starting after the music has been playing
    let speech = jfk();
    let bed = music(16_000 * 20 + speech.len(), rms(&speech) / 3.2);
    let mut mixed = bed.clone();
    for (i, s) in speech.iter().enumerate() {
        mixed[16_000 * 8 + i] += *s;
    }
    let r = run(&mixed, true);
    let clean = run(&speech, true);
    println!("speech over music (10 dB): finals {} total speaking {:.1}s (clean speech {:.1}s)", r.finals, r.speaking_secs, clean.speaking_secs);
    println!("   utterance ranges (s, relative to the music start): {:?}", r.ranges.iter().map(|(a, b)| (*a as f32 / 16000.0, *b as f32 / 16000.0)).collect::<Vec<_>>());
    println!("   speech occupies {:.1}s .. {:.1}s", 8.0, 8.0 + speech.len() as f32 / 16000.0);
    let cov = coverage(&speech, 8 * 16_000, &r.ranges);
    println!("   share of clearly-spoken audio that reached the recogniser: {:.0}%", cov * 100.0);
    // Honest limit of an energy-based detector: at 10 dB over a loud music bed it
    // keeps ~85-90% of the speech (100% with no music). A neural VAD, or the
    // streaming engine (which never gates its audio on the VAD), does better.
    assert!(cov >= 0.8, "speech over music: only {:.0}% reached the recogniser", cov * 100.0);
    let _ = clean;
}

