//! `rhemaflow-d calibrate` — learn how the engine mis-hears YOUR voice, and
//! measure the word accuracy you actually get.
//!
//! You read ~30 short pulpit sentences into the microphone. For each one we
//! keep what you were asked to say and what the engine heard. Then:
//!   * word accuracy is reported (the number that matters: is it >= 90%?),
//!   * the pairs are split — even lines teach the lexicon, odd lines test it —
//!     so the "after" figure is honest (measured on sentences it never saw),
//!   * the lexicon is saved to `~/RhemaFlow-Services/lexicon.tsv`, and the app
//!     loads it on every start.
//!
//! `--wav-dir DIR` does the same from recordings instead of the microphone:
//! DIR holds `01.wav` + `01.txt`, `02.wav` + `02.txt`, …

use rf_asr::{AsrEngine, EngineKind, WHISPER_RATE};
use rf_core::{word_error_rate, Lexicon};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const USAGE: &str = "\
USAGE: rhemaflow-d calibrate [options]

  --model PATH     ggml model (default: ./models/ggml-tiny.en.bin)
  --engine E       whisper (default) | parakeet
  --wav-dir DIR    use DIR/NN.wav + DIR/NN.txt instead of the microphone
  --reset          forget the saved lexicon first
  --lines N        how many prompts to read (default 30, max 30)

Speak each sentence naturally, at your normal pulpit pace, then pause.";

/// Sentences that stress what a church actually says: book names that engines
/// garble, number words, ranges, commands and ordinary preaching.
pub const SCRIPT: &[&str] = &[
    "Turn with me to Habakkuk chapter two verse four.",
    "First Thessalonians chapter five verse sixteen.",
    "Let us read Romans eight verse twenty eight.",
    "Open your Bibles to Second Corinthians chapter five verse seventeen.",
    "Psalm one hundred and nineteen verse one hundred and five.",
    "John chapter three verse sixteen.",
    "Go to Ephesians chapter six, verses ten through eighteen.",
    "Philippians chapter four verse thirteen.",
    "Next verse.",
    "Previous verse.",
    "Go back a chapter.",
    "Read it aloud.",
    "Zoom in.",
    "Clear the screen.",
    "Habakkuk wrote down the vision and made it plain.",
    "The Thessalonians were known for their faith and their love.",
    "Matthew chapter twenty eight, verses nineteen and twenty.",
    "Deuteronomy chapter six verse five.",
    "Ecclesiastes chapter three verse one.",
    "Revelation chapter twenty one verse four.",
    "Hebrews chapter eleven verse one.",
    "Isaiah fifty three verse five.",
    "Jeremiah twenty nine verse eleven.",
    "Proverbs chapter three, verses five and six.",
    "Galatians five twenty two.",
    "Colossians chapter three verse twenty three.",
    "Luke chapter fifteen verse twenty.",
    "There are three things I want you to remember this morning.",
    "And the Lord said unto Moses, stretch out thine hand.",
    "Let us pray.",
];

pub fn lexicon_path() -> PathBuf {
    crate::recorder::services_root().join("lexicon.tsv")
}

/// Load the saved lexicon (empty if none or unreadable).
pub fn load_lexicon() -> Lexicon {
    std::fs::read_to_string(lexicon_path()).map(|s| Lexicon::from_tsv(&s)).unwrap_or_default()
}

struct Sample {
    said: String,
    heard: String,
    audio_ms: u64,
    decode_ms: u64,
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let mut model = String::from("./models/ggml-tiny.en.bin");
    let mut kind = EngineKind::Whisper;
    let mut wav_dir: Option<PathBuf> = None;
    let mut reset = false;
    let mut lines = SCRIPT.len();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--model" => {
                i += 1;
                model = args.get(i).cloned().ok_or_else(|| anyhow::anyhow!("missing value after --model"))?;
                kind = EngineKind::guess_from_model_path(&model);
            }
            "--engine" => {
                i += 1;
                let v = args.get(i).cloned().unwrap_or_default();
                kind = EngineKind::parse(&v).ok_or_else(|| anyhow::anyhow!("unknown engine '{v}'"))?;
            }
            "--wav-dir" => {
                i += 1;
                wav_dir = args.get(i).map(PathBuf::from);
            }
            "--reset" => reset = true,
            "--lines" => {
                i += 1;
                lines = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(lines).clamp(4, SCRIPT.len());
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => anyhow::bail!("unknown option '{other}'\n\n{USAGE}"),
        }
        i += 1;
    }
    if !rf_asr::native_compiled() {
        anyhow::bail!("this build has no speech engine compiled in");
    }
    println!("Loading {} model {model} …", kind.label());
    let mut engine = kind.load(&model)?;
    // warm-up so the first timing isn't the model's cold start
    let _ = engine.transcribe(&vec![0.0f32; WHISPER_RATE as usize]);

    let samples = match wav_dir {
        Some(dir) => from_wavs(&mut *engine, &dir)?,
        None => from_microphone(&mut *engine, lines)?,
    };
    if samples.len() < 4 {
        anyhow::bail!("only {} usable samples — need at least 4 to learn anything", samples.len());
    }
    report_and_save(&samples, reset)
}

fn transcribe_text(engine: &mut dyn AsrEngine, audio: &[f32]) -> (String, u64) {
    let t0 = Instant::now();
    let segs = engine.transcribe(audio);
    let text = segs.iter().map(|s| s.text.trim()).collect::<Vec<_>>().join(" ");
    (text, t0.elapsed().as_millis() as u64)
}

fn from_microphone(engine: &mut dyn AsrEngine, lines: usize) -> anyhow::Result<Vec<Sample>> {
    let audio = rf_asr::AudioEngine::start(None)?;
    println!("\nRead each line aloud, then pause. (Ctrl-C to stop early.)\n");
    let mut out = Vec::new();
    for (n, line) in SCRIPT.iter().take(lines).enumerate() {
        println!("[{}/{}]  {}", n + 1, lines, line);
        while audio.utterance_rx.try_recv().is_ok() {} // drop anything stale
        let deadline = Instant::now() + Duration::from_secs(25);
        let utt = loop {
            match audio.utterance_rx.recv_timeout(Duration::from_millis(200)) {
                Ok(u) if !u.partial => break Some(u),
                Ok(_) => {}
                Err(_) if Instant::now() > deadline => break None,
                Err(_) => {}
            }
        };
        let Some(u) = utt else {
            println!("   (nothing heard — skipped)\n");
            continue;
        };
        let (heard, decode_ms) = transcribe_text(engine, &u.samples);
        println!("   heard: {heard}\n");
        out.push(Sample { said: (*line).to_string(), heard, audio_ms: u.samples.len() as u64 * 1000 / WHISPER_RATE as u64, decode_ms });
    }
    Ok(out)
}

fn from_wavs(engine: &mut dyn AsrEngine, dir: &PathBuf) -> anyhow::Result<Vec<Sample>> {
    let mut stems: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("wav"))
        .collect();
    stems.sort();
    let mut out = Vec::new();
    for wav in stems {
        let txt = wav.with_extension("txt");
        let Ok(said) = std::fs::read_to_string(&txt) else {
            println!("skip {} — no matching .txt", wav.display());
            continue;
        };
        let audio = rf_asr::wav::load_16k_mono(&wav.to_string_lossy())?;
        let (heard, decode_ms) = transcribe_text(engine, &audio);
        println!("{}: {heard}", wav.file_name().unwrap().to_string_lossy());
        out.push(Sample { said: said.trim().to_string(), heard, audio_ms: audio.len() as u64 * 1000 / WHISPER_RATE as u64, decode_ms });
    }
    Ok(out)
}

fn accuracy(pairs: &[(&str, &str)]) -> f64 {
    let (mut errors, mut words) = (0usize, 0usize);
    for (said, heard) in pairs {
        let w = word_error_rate(said, heard);
        errors += w.errors();
        words += w.ref_words;
    }
    if words == 0 { 0.0 } else { 100.0 * (1.0 - errors as f64 / words as f64) }
}

fn report_and_save(samples: &[Sample], reset: bool) -> anyhow::Result<()> {
    let raw: Vec<(&str, &str)> = samples.iter().map(|s| (s.said.as_str(), s.heard.as_str())).collect();
    let acc_all = accuracy(&raw);

    // honest before/after: learn from even-numbered samples, test on odd ones
    let mut trial = Lexicon::new();
    for (n, s) in samples.iter().enumerate() {
        if n % 2 == 0 {
            trial.learn(&s.said, &s.heard);
        }
    }
    let test: Vec<&Sample> = samples.iter().enumerate().filter(|(n, _)| n % 2 == 1).map(|(_, s)| s).collect();
    let before: Vec<(&str, &str)> = test.iter().map(|s| (s.said.as_str(), s.heard.as_str())).collect();
    let repaired: Vec<String> = test.iter().map(|s| trial.apply(&s.heard)).collect();
    let after: Vec<(&str, &str)> = test.iter().zip(&repaired).map(|(s, r)| (s.said.as_str(), r.as_str())).collect();

    let audio_s: f64 = samples.iter().map(|s| s.audio_ms as f64).sum::<f64>() / 1000.0;
    let decode_s: f64 = samples.iter().map(|s| s.decode_ms as f64).sum::<f64>() / 1000.0;
    let mut dec: Vec<u64> = samples.iter().map(|s| s.decode_ms).collect();
    dec.sort();
    let p95 = dec[((dec.len() as f64 * 0.95).ceil() as usize).saturating_sub(1).min(dec.len() - 1)];

    println!("\n================ calibration report ================");
    println!("samples                 : {}", samples.len());
    println!("word accuracy (all)     : {acc_all:.1}%   (target 90%+)");
    println!("held-out, before lexicon: {:.1}%", accuracy(&before));
    println!("held-out, after lexicon : {:.1}%   (learned on other lines only)", accuracy(&after));
    println!("decode time             : avg {:.0} ms, p95 {p95} ms   (real-time factor {:.2})", 1000.0 * decode_s / samples.len() as f64, decode_s / audio_s.max(0.001));
    println!("====================================================");

    // the saved lexicon learns from EVERYTHING
    let path = lexicon_path();
    let mut lex = if reset { Lexicon::new() } else { load_lexicon() };
    let mut learned = 0;
    for s in samples {
        learned += lex.learn(&s.said, &s.heard);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, lex.to_tsv())?;
    println!("recorded {learned} mishearing pattern(s) → {}", path.display());
    println!("A pattern is only applied after it has been seen twice, so run calibrate a second time (or read the lines twice) to activate repairs.");
    if acc_all < 90.0 {
        println!("\nBelow 90%: the lexicon cannot fix a weak model. Try a larger model (base.en / small.en, or --engine parakeet) and run calibrate again to compare.");
    }
    Ok(())
}
