//! Offline WAV recording (16 kHz mono) + per-service artefacts.
//!
//! Service folder layout (local-first, PRD §15/§21):
//!   ~/RhemaFlow-Services/<timestamp>/
//!     audio.wav        — 16 kHz mono PCM (only while recording is on)
//!     transcript.txt   — timestamped live transcript
//!     events.jsonl     — machine-readable scripture/timeline events
//! Budget: ~115 MB/hour (PRD allows ≤ ~300 MB/hour).

use anyhow::Context;
use crossbeam_channel::Sender;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

pub const RATE: u32 = 16_000;

pub struct Recorder {
    #[allow(dead_code)]
    pub dir: PathBuf,
    #[allow(dead_code)]
    pub audio_path: PathBuf,
    tx: Option<Sender<Vec<f32>>>,
    written: Arc<AtomicU64>,
    /// samples handed to the writer so far (updated synchronously, unlike `written`)
    pushed: Arc<AtomicU64>,
    handle: Option<JoinHandle<()>>,
}

impl Recorder {
    /// Start recording into the file `audio_path` (v0.812.0: the caller chooses a
    /// date-and-time name; a second Stop/Record gets its own file instead of
    /// truncating the first). The writer thread ends when this handle is dropped
    /// or [`finish`](Self::finish) is called.
    pub fn start_at(audio_path: &Path) -> anyhow::Result<Self> {
        let dir = audio_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("cannot create service dir {}", dir.display()))?;
        let audio_path = audio_path.to_path_buf();
        let (tx, rx) = crossbeam_channel::unbounded::<Vec<f32>>();
        let written = Arc::new(AtomicU64::new(0));
        let written_w = written.clone();
        let path_w = audio_path.clone();

        let handle = std::thread::Builder::new()
            .name("rf-recorder".into())
            .spawn(move || {
                let spec = WavSpec {
                    channels: 1,
                    sample_rate: RATE,
                    bits_per_sample: 16,
                    sample_format: SampleFormat::Int,
                };
                let mut writer = match WavWriter::create(&path_w, spec) {
                    Ok(w) => w,
                    Err(e) => {
                        eprintln!("[rf-rec] cannot open {}: {e}", path_w.display());
                        return;
                    }
                };
                let mut frames: u64 = 0;
                for chunk in rx {
                    for s in chunk {
                        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
                        if writer.write_sample(v).is_err() {
                            break;
                        }
                        frames += 1;
                    }
                    written_w.store(frames, Ordering::Relaxed);
                }
                if let Err(e) = writer.finalize() {
                    eprintln!("[rf-rec] finalize error: {e}");
                }
                eprintln!("[rf-rec] wrote {} ({} frames)", path_w.display(), frames);
            })?;

        Ok(Recorder {
            dir: dir.to_path_buf(),
            audio_path,
            tx: Some(tx),
            written,
            pushed: Arc::new(AtomicU64::new(0)),
            handle: Some(handle),
        })
    }

    pub fn push(&self, chunk: &[f32]) {
        if !chunk.is_empty() {
            if let Some(tx) = &self.tx {
                tx.send(chunk.to_vec()).ok();
                self.pushed.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            }
        }
    }

    /// Pad with silence until the recording is `target_ms` long. Used when the
    /// microphone is re-opened mid-service: whatever wasn't captured while it
    /// was down becomes silence, so `audio.wav` stays aligned with the service
    /// clock and the Scripture Index timestamps still point at the right moment.
    pub fn pad_silence_to_ms(&self, target_ms: u64) {
        let mut need = silence_needed(self.pushed.load(Ordering::Relaxed), target_ms);
        while need > 0 {
            let n = need.min(RATE as usize); // 1 s at a time — never one huge allocation
            self.push(&vec![0.0f32; n]);
            need -= n;
        }
    }

    pub fn seconds_written(&self) -> f32 {
        self.written.load(Ordering::Relaxed) as f32 / RATE as f32
    }

    /// Close the WAV cleanly and wait for the writer thread.
    pub fn finish(mut self) {
        self.tx = None;
        if let Some(h) = self.handle.take() {
            h.join().ok();
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(h) = self.handle.take() {
            h.join().ok();
        }
    }
}

/// How many silent samples bring a recording that currently holds
/// `pushed_samples` up to `target_ms` of audio. Zero if already there.
pub fn silence_needed(pushed_samples: u64, target_ms: u64) -> usize {
    (target_ms * (RATE as u64) / 1000).saturating_sub(pushed_samples) as usize
}

/// Append-only JSONL event log (scripture index + timeline events).
pub struct EventLog {
    file: std::fs::File,
    #[allow(dead_code)]
    pub path: PathBuf,
    start: Option<crate::naming::LocalStamp>,
}

impl EventLog {
    /// `start` = the wall-clock moment of service time 0, so every record also
    /// carries the real clock time ("clock": "01:28:46").
    pub fn create_at(path: &Path, start: Option<crate::naming::LocalStamp>) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(EventLog { file, path: path.to_path_buf(), start })
    }

    pub fn append(&mut self, service_ms: u64, kind: &str, detail: &str) {
        let mut rec = serde_json::json!({
            "t_ms": service_ms,
            "kind": kind,
            "detail": detail,
        });
        if let Some(st) = &self.start {
            let at = st.plus_ms(service_ms);
            rec["clock"] = serde_json::json!(at.time_hms());
            rec["date"] = serde_json::json!(at.long_date());
        }
        writeln!(self.file, "{rec}").ok();
    }

    pub fn flush(&mut self) {
        self.file.flush().ok();
    }
}

/// Append transcript lines as they arrive.
pub struct TranscriptLog {
    file: std::fs::File,
    #[allow(dead_code)]
    pub path: PathBuf,
}

impl TranscriptLog {
    /// Open (or continue) the live transcript file; a new file starts with `header`
    /// (the day, date and start time).
    pub fn create_at(path: &Path, header: &str) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let is_new = !path.exists() || std::fs::metadata(path).map(|m| m.len() == 0).unwrap_or(true);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        if is_new {
            file.write_all(header.as_bytes()).ok();
        }
        Ok(TranscriptLog { file, path: path.to_path_buf() })
    }

    /// `prefix` is e.g. `[00:01:03 | 01:28:46]`.
    pub fn append(&mut self, prefix: &str, text: &str) {
        let line = format!("{prefix} {text}\n");
        self.file.write_all(line.as_bytes()).ok();
    }

    pub fn flush(&mut self) {
        self.file.flush().ok();
    }
}

/// Default service folder root: ~/RhemaFlow-Services
pub fn services_root() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("RhemaFlow-Services");
    }
    PathBuf::from("./RhemaFlow-Services")
}

#[allow(dead_code)] // superseded in v0.812.0 by the sermon notes file (naming.rs)
/// Human-readable sermon Scripture index (v0.801.2 — PRD feature 3:
/// "recordings tagged with a digital index of scriptures cited").
/// Written once when the service ends, next to audio.wav/transcript.txt.
pub fn write_service_index(
    dir: &Path,
    entries: &[crate::state::ScriptureIndexEntry],
    church: &str,
    duration_ms: u64,
) -> std::io::Result<()> {
    use std::fmt::Write as _;
    let mut md = String::new();
    let _ = writeln!(md, "# Scripture index");
    let _ = writeln!(md);
    if !church.trim().is_empty() {
        let _ = writeln!(md, "**{church}** — ");
    }
    let mins = duration_ms / 60_000;
    let secs = (duration_ms % 60_000) / 1000;
    let _ = writeln!(md, "Service length: {mins:02}:{secs:02}  \nArtefacts: `audio.wav` · `transcript.txt` · `events.jsonl` (machine-readable)\n");
    if entries.is_empty() {
        let _ = writeln!(md, "_No Scripture was staged during this service._");
    } else {
        let _ = writeln!(md, "| Time | Scripture | Source |");
        let _ = writeln!(md, "|------|-----------|--------|");
        for e in entries {
            let s = e.service_ms / 1000;
            let _ = writeln!(
                md,
                "| {:02}:{:02}:{:02} | {} | {} |",
                s / 3600,
                (s / 60) % 60,
                s % 60,
                e.label,
                e.source
            );
        }
    }
    std::fs::write(dir.join("index.md"), md)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_needed_covers_exactly_the_gap() {
        // 5 s recorded, service clock says 9 s -> 4 s of silence
        assert_eq!(silence_needed(5 * 16_000, 9_000), 4 * 16_000);
        assert_eq!(silence_needed(9 * 16_000, 9_000), 0, "already caught up");
        assert_eq!(silence_needed(12 * 16_000, 9_000), 0, "never negative");
    }

    #[test]
    fn a_mic_gap_becomes_silence_so_the_wav_stays_aligned_with_the_service_clock() {
        let dir = std::env::temp_dir().join(format!("rf-rec-gap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let rec = Recorder::start_at(&dir.join("gap test - recording.wav")).unwrap();
        rec.push(&vec![0.2f32; 16_000]); // 1 s of audio…
        rec.pad_silence_to_ms(4_000); //  …mic down until t = 4 s…
        rec.push(&vec![0.2f32; 16_000]); //  …then 1 s more after recovery
        let path = rec.audio_path.clone();
        rec.finish();
        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.len(), 5 * 16_000, "1 s + 3 s of padding + 1 s");
        let samples: Vec<i16> = reader.into_samples::<i16>().map(|s| s.unwrap()).collect();
        assert!(samples[16_000..4 * 16_000].iter().all(|&s| s == 0), "the gap is true silence");
        assert!(samples[4 * 16_000..].iter().all(|&s| s != 0), "audio resumes at t = 4 s");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
