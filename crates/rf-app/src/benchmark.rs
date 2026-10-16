//! `rhemaflow-d benchmark AUDIO.wav [options]` — compare speech engines on the
//! SAME audio: load time, decode time, real-time factor, word error rate
//! against a reference transcript, and which scripture reference the parser
//! actually extracts from each transcript (the metric that matters here).
//!
//! No GUI, no microphone. Engines whose model file is missing are skipped.

use rf_asr::{AsrEngine, EngineKind};
use rf_core::word_error_rate;
use std::time::Instant;

const USAGE: &str = "\
USAGE: rhemaflow-d benchmark AUDIO.wav [options]

  --ref FILE            reference transcript (text file) → enables WER
  --ref-text \"…\"        reference transcript given inline
  --whisper MODEL       whisper ggml model   (default: ./models/ggml-tiny.en.bin)
  --parakeet MODEL      parakeet ggml model  (default: ./models/ggml-parakeet-*.bin)
  --stream-url URL      also run a streaming service (Confucius4-R2T2 protocol, see
                        docs/R2T2-ON-M1.md); audio is sent in 160 ms frames in real time
  --stream-speed X      send audio X times faster than real time (default 1.0)
  --mix-noise DB        mix steady white noise under the audio at this speech-to-noise ratio
  --mix-music DB        mix a synthetic worship-band chord bed under the audio at this ratio
                        (the same mixed audio goes to EVERY engine, so rows are comparable)
  --extra LABEL FILE    add another transcript (e.g. macOS dictation) to the
                        WER/detection comparison; repeatable

Any audio format hound reads (WAV) is resampled to 16 kHz mono automatically.";

struct Row {
    label: String,
    load_s: Option<f64>,
    decode_s: Option<f64>,
    rtf: Option<f64>,
    text: String,
    /// streaming only: ms from the start of the audio to the first word
    first_word_ms: Option<u64>,
    /// streaming only: ms from the END of the audio until the last word arrived
    after_end_ms: Option<u64>,
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let mut wav: Option<String> = None;
    let mut reference: Option<String> = None;
    let mut whisper: Option<String> = None;
    let mut parakeet: Option<String> = None;
    let mut extras: Vec<(String, String)> = Vec::new();
    let mut stream_url: Option<String> = None;
    let mut stream_speed = 1.0f64;
    let mut mix_noise: Option<f32> = None;
    let mut mix_music: Option<f32> = None;

    let mut i = 0;
    while i < args.len() {
        let next = |i: &mut usize| -> anyhow::Result<String> {
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| anyhow::anyhow!("missing value after {}\n\n{USAGE}", args[*i - 1]))
        };
        match args[i].as_str() {
            "--ref" => {
                let f = next(&mut i)?;
                reference = Some(std::fs::read_to_string(&f).map_err(|e| anyhow::anyhow!("cannot read reference '{f}': {e}"))?);
            }
            "--ref-text" => reference = Some(next(&mut i)?),
            "--whisper" => whisper = Some(next(&mut i)?),
            "--parakeet" => parakeet = Some(next(&mut i)?),
            "--stream-url" => stream_url = Some(next(&mut i)?),
            "--stream-speed" => stream_speed = next(&mut i)?.parse().map_err(|_| anyhow::anyhow!("--stream-speed needs a number"))?,
            "--mix-noise" => mix_noise = Some(next(&mut i)?.parse().map_err(|_| anyhow::anyhow!("--mix-noise needs dB, e.g. 10"))?),
            "--mix-music" => mix_music = Some(next(&mut i)?.parse().map_err(|_| anyhow::anyhow!("--mix-music needs dB, e.g. 10"))?),
            "--extra" => {
                let label = next(&mut i)?;
                let f = next(&mut i)?;
                let text = std::fs::read_to_string(&f).map_err(|e| anyhow::anyhow!("cannot read '{f}': {e}"))?;
                extras.push((label, text));
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other if other.starts_with("--") => anyhow::bail!("unknown option {other}\n\n{USAGE}"),
            other => wav = Some(other.to_string()),
        }
        i += 1;
    }
    let wav = wav.ok_or_else(|| anyhow::anyhow!("no audio file given\n\n{USAGE}"))?;
    if !rf_asr::native_compiled() {
        anyhow::bail!("this build has no native speech engines (built without the `native` feature)");
    }

    let mut samples = rf_asr::wav::load_16k_mono(&wav)?;
    if let Some(db) = mix_noise {
        let bg = rf_asr::testsig::noise(samples.len(), 11, 0.05);
        samples = rf_asr::testsig::mix_at_snr(&samples, &bg, db);
        println!("Mixed in   : white noise at {db} dB speech-to-noise");
    }
    if let Some(db) = mix_music {
        let bg = rf_asr::testsig::music(samples.len(), 0.05);
        samples = rf_asr::testsig::mix_at_snr(&samples, &bg, db);
        println!("Mixed in   : chord-bed music at {db} dB speech-to-music");
    }
    let audio_s = samples.len() as f64 / rf_asr::WHISPER_RATE as f64;
    println!("Audio      : {wav}  ({audio_s:.1} s, 16 kHz mono)");
    if let Some(r) = &reference {
        println!("Reference  : {} words", rf_core::normalize_words(r).len());
    } else {
        println!("Reference  : none (pass --ref FILE or --ref-text to get WER)");
    }
    println!();

    let mut rows: Vec<Row> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for (kind, explicit) in [(EngineKind::Whisper, whisper), (EngineKind::Parakeet, parakeet)] {
        let Some(model) = crate::find_model(kind, explicit.as_deref()) else {
            skipped.push(match kind {
                EngineKind::Whisper => "whisper: no model found (run ./run.sh, or pass --whisper PATH)".to_string(),
                EngineKind::Parakeet => "parakeet: no model found (run ./run.sh model-parakeet, or pass --parakeet PATH)".to_string(),
            });
            continue;
        };
        eprintln!("[benchmark] loading {} …", model);
        let t = Instant::now();
        let mut eng: Box<dyn AsrEngine> = match kind.load(&model) {
            Ok(e) => e,
            Err(e) => {
                skipped.push(format!("{}: {e}", kind.label()));
                continue;
            }
        };
        let load_s = t.elapsed().as_secs_f64();
        // warm-up on the first second so one-time setup isn't billed to the decode
        let warm = &samples[..samples.len().min(rf_asr::WHISPER_RATE as usize)];
        let _ = eng.transcribe(warm);
        eprintln!("[benchmark] transcribing with {} …", kind.label());
        let t = Instant::now();
        let segs = eng.transcribe(&samples);
        let decode_s = t.elapsed().as_secs_f64();
        let text = segs.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join(" ");
        rows.push(Row { label: kind.label().to_string(), load_s: Some(load_s), decode_s: Some(decode_s), rtf: Some(decode_s / audio_s.max(1e-9)), text, first_word_ms: None, after_end_ms: None });
    }
    if let Some(url) = &stream_url {
        eprintln!("[benchmark] streaming {audio_s:.1} s of audio to {url} at {stream_speed}x real time …");
        match run_stream(url, &samples, stream_speed) {
            Ok(r) => rows.push(r),
            Err(e) => skipped.push(format!("streaming service: {e}")),
        }
    }
    for (label, text) in extras {
        rows.push(Row { label, load_s: None, decode_s: None, rtf: None, text: text.trim().to_string(), first_word_ms: None, after_end_ms: None });
    }

    if rows.is_empty() {
        for s in &skipped {
            println!("skipped — {s}");
        }
        anyhow::bail!("nothing to compare: no engine had a model and no --extra transcript was given");
    }

    let bible = rf_bible::Bible::embedded();
    println!("{:<18} {:>7} {:>8} {:>7} {:>8}  {:<9} {}", "ENGINE", "LOAD", "DECODE", "RTF", "WER", "S/D/I", "SCRIPTURE DETECTED");
    for r in &rows {
        let fmt = |v: Option<f64>, unit: &str| v.map(|x| format!("{x:.2}{unit}")).unwrap_or_else(|| "-".into());
        let (wer, sdi) = match &reference {
            Some(refr) => {
                let w = word_error_rate(refr, &r.text);
                (format!("{:.1}%", w.rate() * 100.0), format!("{}/{}/{}", w.substitutions, w.deletions, w.insertions))
            }
            None => ("-".into(), "-".into()),
        };
        let out = rf_core::parse(bible, &r.text, &rf_core::ParseContext::default(), 0.75);
        let mut found: Vec<String> = out.detections.iter().map(|d| d.passage.label()).collect();
        found.extend(out.confirms.iter().map(|d| format!("{}?", d.passage.label())));
        let found = if found.is_empty() { "none".to_string() } else { found.join(", ") };
        println!("{:<18} {:>7} {:>8} {:>7} {:>8}  {:<9} {}", r.label, fmt(r.load_s, "s"), fmt(r.decode_s, "s"), fmt(r.rtf, "x"), wer, sdi, found);
    }
    for r in &rows {
        if let (Some(f), Some(a)) = (r.first_word_ms, r.after_end_ms) {
            println!("\n[{}] first word {f} ms after the audio started; last word {a} ms after the audio ENDED", r.label);
        }
    }
    println!("\nRTF = decode time / audio length (below 1.0x is faster than real time).");
    println!("Whisper is primed with a scripture vocabulary prompt; Parakeet has no equivalent input.\n");
    for r in &rows {
        println!("[{}] {}", r.label, r.text);
    }
    for s in &skipped {
        println!("\nskipped — {s}");
    }
    Ok(())
}

/// Send `samples` to a streaming service the way the app does and record when text arrives.
fn run_stream(url: &str, samples: &[f32], speed: f64) -> anyhow::Result<Row> {
    use rf_asr::stream_client::{StreamClient, StreamEvent, FRAME_SAMPLES};
    let client = StreamClient::start(url);
    // wait for the connection
    let t_wait = Instant::now();
    loop {
        match client.events.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(StreamEvent::Connected) => break,
            Ok(StreamEvent::Disconnected(why)) if t_wait.elapsed().as_secs() > 6 => anyhow::bail!("cannot connect: {why}"),
            _ if t_wait.elapsed().as_secs() > 8 => anyhow::bail!("cannot connect to {url}"),
            _ => {}
        }
    }
    let frame_dur = std::time::Duration::from_secs_f64(FRAME_SAMPLES as f64 / rf_asr::WHISPER_RATE as f64 / speed.max(0.01));
    let start = Instant::now();
    let mut text = String::new();
    let (mut first_word, mut last_word) = (None::<u64>, 0u64);
    let mut note = |ev: StreamEvent, text: &mut String, now: u64| {
        if let StreamEvent::Text(t) = ev {
            if !t.trim().is_empty() {
                first_word.get_or_insert(now);
                last_word = now;
            }
            text.push_str(&t);
        }
    };
    let mut sent_until = start;
    for frame in samples.chunks(FRAME_SAMPLES) {
        let mut f = frame.to_vec();
        f.resize(FRAME_SAMPLES, 0.0);
        client.send_audio(f);
        sent_until += frame_dur;
        while Instant::now() < sent_until {
            if let Ok(ev) = client.events.recv_timeout(std::time::Duration::from_millis(5)) {
                note(ev, &mut text, start.elapsed().as_millis() as u64);
            }
        }
    }
    let audio_end = start.elapsed().as_millis() as u64;
    client.flush();
    let drain_until = Instant::now() + std::time::Duration::from_secs(8);
    while Instant::now() < drain_until {
        match client.events.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(StreamEvent::Reset) => break,
            Ok(ev) => note(ev, &mut text, start.elapsed().as_millis() as u64),
            Err(_) => {}
        }
    }
    Ok(Row {
        label: "streaming (R2T2)".into(),
        load_s: None,
        decode_s: None,
        rtf: None,
        text: text.split_whitespace().collect::<Vec<_>>().join(" "),
        first_word_ms: first_word,
        after_end_ms: Some(last_word.saturating_sub(audio_end)),
    })
}
