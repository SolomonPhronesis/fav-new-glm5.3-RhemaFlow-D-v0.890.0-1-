//! ASR engines: two offline native engines that share one whisper.cpp/ggml
//! build (Whisper, NVIDIA Parakeet TDT) and a deterministic mock (tests +
//! no-model demo mode).

use crate::{AsrEngine, Segment, WHISPER_RATE};

#[cfg(feature = "native")]
use std::ffi::{CStr, CString};
#[cfg(feature = "native")]
use std::os::raw::c_char;

/// Threads handed to the native decoders.
#[cfg(feature = "native")]
const N_THREADS: i32 = 4;

/// GPU only when built with `--features metal` (opt-in; CPU is the default).
#[cfg(feature = "native")]
const USE_GPU: i32 = if cfg!(feature = "metal") { 1 } else { 0 };

#[cfg(feature = "native")]
fn cpath(model_path: &str) -> anyhow::Result<CString> {
    CString::new(model_path).map_err(|_| anyhow::anyhow!("model path contains a NUL byte"))
}

#[cfg(feature = "native")]
fn err_string(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

#[cfg(feature = "native")]
unsafe fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().trim().to_string()
    }
}

// ---------------------------------------------------------------------------
// Whisper (production default)
// ---------------------------------------------------------------------------

/// Domain-vocabulary prompt (v0.801.2 accuracy upgrade #1).
///
/// Whisper conditions its first-token distribution on this text, which
/// biases the decoder toward pulpit vocabulary: book names it would
/// otherwise garble ("Habakkuk", "Thessalonians"), the command words the
/// operator relies on ("next verse", "previous"), and the reference
/// grammar ("chapter … verse …"). This is the cheapest large accuracy win
/// that exists for an on-device model: zero latency cost, fully offline.
/// (Whisper-only: Parakeet has no prompt input, so it gets no such priming —
/// which is exactly what `rhemaflow-d benchmark` is for measuring.)
#[cfg(feature = "native")]
const DOMAIN_PROMPT: &str = "\
Let us turn to John chapter 3 verse 16. Next verse. Previous verse. \
Again. Read it aloud. Zoom in. Full screen. Last scripture. \
Genesis Exodus Leviticus Deuteronomy Judges Psalms Proverbs Isaiah \
Jeremiah Ezekiel Daniel Matthew Mark Luke John Acts Romans \
First Corinthians Second Corinthians Galatians Ephesians Philippians \
Colossians First Thessalonians Second Thessalonians Hebrews James \
First Peter Second Peter Jude Revelation. \
For God so loved the world that he gave his only begotten Son.";


/// Words learned from the operator's corrections (church and pastor names), added to Whisper's prompt.
static EXTRA_PROMPT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Set the learned words (kept short: the prompt has a small budget; no control characters).
pub fn set_extra_prompt(words: &str) {
    let clean: String = words.chars().filter(|c| !c.is_control()).take(200).collect();
    if let Ok(mut g) = EXTRA_PROMPT.lock() {
        *g = clean;
    }
}
pub fn extra_prompt() -> String {
    EXTRA_PROMPT.lock().map(|g| g.clone()).unwrap_or_default()
}
/// The full prompt: the built-in one, then "Names:" and the learned words.
pub fn full_prompt(base: &str, extra: &str) -> String {
    if extra.trim().is_empty() { base.to_string() } else { format!("{base} Names: {}.", extra.trim()) }
}

/// The 66 book names in their spoken/canonical form — the hotword budget for
/// streaming engines. (Kept separate from DOMAIN_PROMPT, which also carries a
/// verse sample and command words that a streaming `context` slot wastes.)
pub const BOOK_NAMES: &str = "Genesis Exodus Leviticus Numbers Deuteronomy Joshua Judges Ruth \
First Samuel Second Samuel First Kings Second Kings First Chronicles Second Chronicles \
Ezra Nehemiah Esther Job Psalms Proverbs Ecclesiastes Isaiah Jeremiah Lamentations \
Ezekiel Daniel Hosea Joel Amos Obadiah Jonah Micah Nahum Habakkuk Zephaniah Haggai \
Zechariah Malachi Matthew Mark Luke John Acts Romans First Corinthians Second Corinthians \
Galatians Ephesians Philippians Colossians First Thessalonians Second Thessalonians \
First Timothy Second Timothy Titus Philemon Hebrews James First Peter Second Peter \
First John Second John Third John Jude Revelation";

/// Compact hotword string for a streaming engine's context slot (the R2T2
/// sidecar's `RF_CONTEXT` / `--context`): the 66 book names plus the words
/// learned from the operator's corrections. v0.854.0 accuracy upgrade — the
/// decoder is biased toward exactly the vocabulary a pulpit uses, at zero
/// latency cost.
pub fn stream_context() -> String {
    let extra = extra_prompt();
    let mut s = String::from(BOOK_NAMES);
    if !extra.trim().is_empty() {
        s.push(' ');
        s.push_str(extra.trim());
    }
    s.chars().filter(|c| !c.is_control()).take(600).collect()
}

#[cfg(feature = "native")]
pub struct WhisperAsr {
    raw: *mut crate::native::RfWhisper,
    prompt: CString,
    extra_seen: String,
    model: String,
}

// The handle is only ever used from one thread at a time (the ASR worker).
#[cfg(feature = "native")]
unsafe impl Send for WhisperAsr {}

#[cfg(feature = "native")]
impl WhisperAsr {
    pub fn load(model_path: &str) -> anyhow::Result<Self> {
        let path = cpath(model_path)?;
        let mut err = [0u8; 256];
        let raw = unsafe { crate::native::rf_whisper_load(path.as_ptr(), USE_GPU, err.as_mut_ptr() as *mut c_char, err.len()) };
        if raw.is_null() {
            anyhow::bail!("failed to load whisper model '{model_path}': {}", err_string(&err));
        }
        eprintln!("[rf-asr] whisper model loaded: {model_path}");
        Ok(Self { raw, prompt: CString::new(DOMAIN_PROMPT).unwrap(), extra_seen: String::new(), model: model_path.to_string() })
    }
}

#[cfg(feature = "native")]
impl Drop for WhisperAsr {
    fn drop(&mut self) {
        unsafe { crate::native::rf_whisper_free(self.raw) }
    }
}

#[cfg(feature = "native")]
impl WhisperAsr {
    fn run(&mut self, samples: &[f32], beam: i32) -> Vec<Segment> {
        // learned names may have changed since the last utterance
        let extra = extra_prompt();
        if extra != self.extra_seen {
            if let Ok(c) = CString::new(full_prompt(DOMAIN_PROMPT, &extra)) {
                self.prompt = c;
            }
            self.extra_seen = extra;
        }
        if samples.len() < WHISPER_RATE as usize / 3 || samples.len() > i32::MAX as usize {
            return vec![]; // < 0.33 s — nothing useful
        }
        let n = unsafe {
            crate::native::rf_whisper_transcribe(self.raw, samples.as_ptr(), samples.len() as i32, self.prompt.as_ptr(), beam, N_THREADS)
        };
        if n < 0 {
            eprintln!("[rf-asr] whisper inference error (code {n})");
            return vec![];
        }
        let mut out = Vec::new();
        for i in 0..n {
            let (text, t0, t1) = unsafe {
                (
                    cstr(crate::native::rf_whisper_segment_text(self.raw, i)),
                    crate::native::rf_whisper_segment_t0_ms(self.raw, i),
                    crate::native::rf_whisper_segment_t1_ms(self.raw, i),
                )
            };
            if !text.is_empty() {
                out.push(Segment { text, start_ms: t0.max(0) as u64, end_ms: t1.max(0) as u64 });
            }
        }
        out
    }
}

#[cfg(feature = "native")]
impl AsrEngine for WhisperAsr {
    fn transcribe(&mut self, samples: &[f32]) -> Vec<Segment> {
        // v0.801.2: beam search (5) instead of greedy — measurably fewer word
        // errors on short commands, ~same latency on M-series.
        self.run(samples, 5)
    }

    /// Greedy decode for interim results: several times cheaper than beam
    /// search, and the finished utterance is always re-decoded properly.
    fn transcribe_fast(&mut self, samples: &[f32]) -> Vec<Segment> {
        self.run(samples, 1)
    }

    fn name(&self) -> &str {
        &self.model
    }
}

// ---------------------------------------------------------------------------
// NVIDIA Parakeet TDT (alternate engine)
// ---------------------------------------------------------------------------

#[cfg(feature = "native")]
pub struct ParakeetAsr {
    raw: *mut crate::native::RfParakeet,
    model: String,
}

#[cfg(feature = "native")]
unsafe impl Send for ParakeetAsr {}

#[cfg(feature = "native")]
impl ParakeetAsr {
    pub fn load(model_path: &str) -> anyhow::Result<Self> {
        let path = cpath(model_path)?;
        let mut err = [0u8; 256];
        let raw = unsafe { crate::native::rf_parakeet_load(path.as_ptr(), USE_GPU, err.as_mut_ptr() as *mut c_char, err.len()) };
        if raw.is_null() {
            anyhow::bail!("failed to load parakeet model '{model_path}': {}", err_string(&err));
        }
        eprintln!("[rf-asr] parakeet model loaded: {model_path}");
        Ok(Self { raw, model: model_path.to_string() })
    }
}

#[cfg(feature = "native")]
impl Drop for ParakeetAsr {
    fn drop(&mut self) {
        unsafe { crate::native::rf_parakeet_free(self.raw) }
    }
}

#[cfg(feature = "native")]
impl AsrEngine for ParakeetAsr {
    fn transcribe(&mut self, samples: &[f32]) -> Vec<Segment> {
        if samples.len() < WHISPER_RATE as usize / 3 || samples.len() > i32::MAX as usize {
            return vec![];
        }
        let n = unsafe { crate::native::rf_parakeet_transcribe(self.raw, samples.as_ptr(), samples.len() as i32, N_THREADS) };
        if n < 0 {
            eprintln!("[rf-asr] parakeet inference error (code {n})");
            return vec![];
        }
        let mut out = Vec::new();
        for i in 0..n {
            let (text, t0, t1) = unsafe {
                (
                    cstr(crate::native::rf_parakeet_segment_text(self.raw, i)),
                    crate::native::rf_parakeet_segment_t0_ms(self.raw, i),
                    crate::native::rf_parakeet_segment_t1_ms(self.raw, i),
                )
            };
            if !text.is_empty() {
                out.push(Segment { text, start_ms: t0.max(0) as u64, end_ms: t1.max(0) as u64 });
            }
        }
        out
    }

    fn name(&self) -> &str {
        &self.model
    }
}

// ---------------------------------------------------------------------------
// Builds without the `native` feature: same names, load() reports why.
// ---------------------------------------------------------------------------

#[cfg(not(feature = "native"))]
pub struct WhisperAsr(());
#[cfg(not(feature = "native"))]
pub struct ParakeetAsr(());

#[cfg(not(feature = "native"))]
impl WhisperAsr {
    pub fn load(_model_path: &str) -> anyhow::Result<Self> {
        anyhow::bail!("rf-asr was built without the `native` feature — no whisper engine")
    }
}
#[cfg(not(feature = "native"))]
impl ParakeetAsr {
    pub fn load(_model_path: &str) -> anyhow::Result<Self> {
        anyhow::bail!("rf-asr was built without the `native` feature — no parakeet engine")
    }
}
#[cfg(not(feature = "native"))]
impl AsrEngine for WhisperAsr {
    fn transcribe(&mut self, _samples: &[f32]) -> Vec<Segment> {
        vec![]
    }
    fn name(&self) -> &str {
        "whisper (not compiled in)"
    }
}
#[cfg(not(feature = "native"))]
impl AsrEngine for ParakeetAsr {
    fn transcribe(&mut self, _samples: &[f32]) -> Vec<Segment> {
        vec![]
    }
    fn name(&self) -> &str {
        "parakeet (not compiled in)"
    }
}

// ---------------------------------------------------------------------------
// Mock (tests / demo mode)
// ---------------------------------------------------------------------------

pub struct MockAsr {
    phrases: Vec<String>,
    idx: usize,
}

impl MockAsr {
    pub fn new(phrases: Vec<String>) -> Self {
        MockAsr { phrases, idx: 0 }
    }

    pub fn demo() -> Self {
        MockAsr::new(vec![
            "Let's turn to John chapter 3 verse 16.".into(),
            "For God so loved the world that he gave his only begotten Son.".into(),
            "Next verse please.".into(),
            "Now look at Romans 8:28.".into(),
            "Turn with me to Psalm 23.".into(),
        ])
    }
}

impl AsrEngine for MockAsr {
    fn transcribe(&mut self, _samples: &[f32]) -> Vec<Segment> {
        if self.phrases.is_empty() {
            return vec![];
        }
        let text = self.phrases[self.idx % self.phrases.len()].clone();
        self.idx += 1;
        vec![Segment { text, start_ms: 0, end_ms: 500 }]
    }

    fn name(&self) -> &str {
        "mock-asr"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_cycles_phrases() {
        let mut m = MockAsr::demo();
        let s1 = m.transcribe(&[]);
        assert!(s1[0].text.contains("John"));
        let s2 = m.transcribe(&[]);
        assert!(s2[0].text.contains("God so loved"));
    }

    #[test]
    fn mock_empty_is_safe() {
        let mut m = MockAsr::new(vec![]);
        assert!(m.transcribe(&[]).is_empty());
    }
}

#[cfg(test)]
mod prompt_tests {
    use super::*;
    #[test]
    fn learned_names_join_the_prompt_safely() {
        assert_eq!(full_prompt("Base.", ""), "Base.");
        assert_eq!(full_prompt("Base.", " Adeoye, Redeemer's Hall "), "Base. Names: Adeoye, Redeemer's Hall.");
        set_extra_prompt(&format!("Ade\0oye{}", "x".repeat(500)));
        let e = extra_prompt();
        assert!(!e.contains('\0') && e.chars().count() <= 200);
        set_extra_prompt("");
    }
}
