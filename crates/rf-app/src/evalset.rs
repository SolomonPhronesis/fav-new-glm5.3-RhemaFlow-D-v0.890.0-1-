//! `rhemaflow-d evalset FOLDER [options]` - score the speech engines on a whole folder of REAL clips (v0.851.0).
//!
//! For every `name.wav` in the folder:
//!   name.txt     what was actually said (required; clips without it are skipped)
//!   name.expect  optional: the scripture the clip should produce, e.g. "John 3:16"
//! Every engine that has a model transcribes every clip. The result is the overall word error rate, how often the
//! right scripture was found, the worst clips, and the mix-ups that keep coming back (with a list of words worth
//! teaching). A report is written to FOLDER/evalset-report.md and FOLDER/vocabulary-suggestions.txt.

use rf_asr::{AsrEngine, EngineKind};
use rf_core::evalset::{align, confusions, report, summarize, vocabulary_suggestions, Clip};
use rf_core::word_error_rate;
use std::path::{Path, PathBuf};

const USAGE: &str = "\
USAGE: rhemaflow-d evalset FOLDER [options]

  FOLDER contains pairs:  clip.wav + clip.txt (what was said)  [+ clip.expect: the scripture it should find]

  --whisper MODEL     whisper ggml model   (default: the one RhemaFlow found)
  --parakeet MODEL    parakeet ggml model
  --threshold X       scripture confidence for a sure detection (default 0.75)

Writes FOLDER/evalset-report.md and FOLDER/vocabulary-suggestions.txt.";

/// The clips in `dir`: (name, wav path, what was said, expected scripture). Clips without a .txt are listed in `skipped`.
pub fn find_clips(dir: &Path) -> (Vec<(String, PathBuf, String, Option<String>)>, Vec<String>) {
    let mut wavs: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map_or(false, |x| x.to_string_lossy().to_lowercase() == "wav")).collect())
        .unwrap_or_default();
    wavs.sort();
    let (mut clips, mut skipped) = (Vec::new(), Vec::new());
    for w in wavs {
        let name = w.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        match std::fs::read_to_string(w.with_extension("txt")) {
            Ok(said) if !said.trim().is_empty() => {
                let expect = std::fs::read_to_string(w.with_extension("expect")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                clips.push((name, w, said, expect));
            }
            _ => skipped.push(name),
        }
    }
    (clips, skipped)
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let (mut folder, mut whisper, mut parakeet, mut threshold) = (None::<String>, None::<String>, None::<String>, 0.75f32);
    let mut i = 0;
    while i < args.len() {
        let next = |i: &mut usize| -> anyhow::Result<String> {
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| anyhow::anyhow!("missing value after {}\n\n{USAGE}", args[*i - 1]))
        };
        match args[i].as_str() {
            "--whisper" => whisper = Some(next(&mut i)?),
            "--parakeet" => parakeet = Some(next(&mut i)?),
            "--threshold" => threshold = next(&mut i)?.parse().map_err(|_| anyhow::anyhow!("--threshold needs a number such as 0.75"))?,
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other if other.starts_with("--") => anyhow::bail!("unknown option {other}\n\n{USAGE}"),
            other => folder = Some(other.to_string()),
        }
        i += 1;
    }
    let folder = PathBuf::from(folder.ok_or_else(|| anyhow::anyhow!("no folder given\n\n{USAGE}"))?);
    if !rf_asr::native_compiled() {
        anyhow::bail!("this build has no native speech engines (built without the `native` feature)");
    }
    let (clips, skipped) = find_clips(&folder);
    if clips.is_empty() {
        anyhow::bail!("no clips found in {}: each clip.wav needs a clip.txt next to it with the words that were said", folder.display());
    }
    println!("{} clip(s) in {}{}", clips.len(), folder.display(), if skipped.is_empty() { String::new() } else { format!("  (no .txt for: {})", skipped.join(", ")) });

    let bible = rf_bible::Bible::embedded();
    let mut audio: Vec<Vec<f32>> = Vec::new();
    for (name, wav, _, _) in &clips {
        audio.push(rf_asr::wav::load_16k_mono(&wav.to_string_lossy()).map_err(|e| anyhow::anyhow!("cannot read {name}.wav: {e}"))?);
    }

    let mut doc = format!("# RhemaFlow evalset\n\nfolder: {}\nclips: {}\n\n", folder.display(), clips.len());
    let mut vocab_all: Vec<String> = Vec::new();
    let mut ran = 0;
    for (kind, explicit) in [(EngineKind::Whisper, whisper), (EngineKind::Parakeet, parakeet)] {
        let Some(model) = crate::find_model(kind, explicit.as_deref()) else {
            println!("skipped {}: no model found", kind.label());
            continue;
        };
        eprintln!("[evalset] loading {} ...", model);
        let mut eng: Box<dyn AsrEngine> = match kind.load(&model) {
            Ok(e) => e,
            Err(e) => {
                println!("skipped {}: {e}", kind.label());
                continue;
            }
        };
        let mut results: Vec<Clip> = Vec::new();
        for (n, (name, _, said, expect)) in clips.iter().enumerate() {
            eprintln!("[evalset] {} {}/{}: {name}", kind.label(), n + 1, clips.len());
            let heard = eng.transcribe(&audio[n]).iter().map(|s| s.text.clone()).collect::<Vec<_>>().join(" ");
            let out = rf_core::parse(bible, &heard, &rf_core::ParseContext::default(), threshold);
            let mut detected: Vec<String> = out.detections.iter().map(|d| d.passage.label()).collect();
            detected.extend(out.confirms.iter().map(|d| format!("{}?", d.passage.label())));
            results.push(Clip { name: name.clone(), wer: word_error_rate(said, &heard), confusions: confusions(&align(said, &heard)), expected: expect.clone(), detected });
        }
        let s = summarize(&results);
        println!("\n{}", report(kind.label(), &s));
        doc.push_str(&report(kind.label(), &s));
        doc.push('\n');
        for w in vocabulary_suggestions(&s, 2) {
            if !vocab_all.contains(&w) {
                vocab_all.push(w);
            }
        }
        ran += 1;
    }
    if ran == 0 {
        anyhow::bail!("no engine had a model (run ./run.sh, or pass --whisper PATH)");
    }
    std::fs::write(folder.join("evalset-report.md"), &doc)?;
    if !vocab_all.is_empty() {
        std::fs::write(folder.join("vocabulary-suggestions.txt"), vocab_all.join("\n") + "\n")?;
        println!("{} word(s) worth teaching are in vocabulary-suggestions.txt (add them to the vocabulary, or read them in `rhemaflow-d calibrate`).", vocab_all.len());
    }
    println!("report written to {}", folder.join("evalset-report.md").display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_are_paired_with_their_words_and_expected_scripture() {
        let d = std::env::temp_dir().join(format!("rf-evalset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for n in ["a", "b", "c"] {
            std::fs::write(d.join(format!("{n}.wav")), b"x").unwrap();
        }
        std::fs::write(d.join("a.txt"), "turn to john three sixteen\n").unwrap();
        std::fs::write(d.join("a.expect"), " John 3:16 \n").unwrap();
        std::fs::write(d.join("b.txt"), "next verse").unwrap();
        std::fs::write(d.join("notes.txt"), "not a clip").unwrap();
        let (clips, skipped) = find_clips(&d);
        assert_eq!(clips.len(), 2);
        assert_eq!((clips[0].0.as_str(), clips[0].3.as_deref()), ("a", Some("John 3:16")));
        assert_eq!(clips[1].3, None);
        assert_eq!(skipped, vec!["c".to_string()]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
