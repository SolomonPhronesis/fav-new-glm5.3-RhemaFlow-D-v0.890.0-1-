//! rf-asr — microphone capture, voice-activity detection and offline ASR
//! (Whisper and NVIDIA Parakeet, one shared whisper.cpp build).
//!
//! Live-path rule (PRD v2.0 §1): everything here works with the network
//! cable pulled. Audio never leaves the machine — the whisper model runs
//! locally, the recording is written locally.

pub mod agc;
pub mod capture;
pub mod engines;
pub mod hpf;
pub mod micreport;
#[cfg(feature = "native")]
mod native;
pub mod resampler;
pub mod stream_client;
pub mod testsig;
pub mod vad;
pub mod wav;

pub use capture::{classify_mic_health, list_input_devices, AudioEngine, InputDevice, MicHealth, StreamTap};
pub use engines::{MockAsr, ParakeetAsr, WhisperAsr};
pub use vad::{EndHintCell, FinishedMeta};

/// One transcript segment produced by the ASR engine for an utterance.
#[derive(Clone, Debug)]
pub struct Segment {
    pub text: String,
    /// offset inside the utterance, milliseconds
    pub start_ms: u64,
    pub end_ms: u64,
}

/// Pluggable offline ASR engine (whisper / parakeet in production, mock in tests/demo).
pub trait AsrEngine: Send {
    fn transcribe(&mut self, samples: &[f32]) -> Vec<Segment>;
    /// Quicker, less exhaustive decode for interim results while someone is
    /// still speaking. Defaults to the full decode for engines with no
    /// cheaper mode.
    fn transcribe_fast(&mut self, samples: &[f32]) -> Vec<Segment> {
        self.transcribe(samples)
    }
    fn name(&self) -> &str;
}

/// A completed spoken utterance, resampled to 16 kHz mono.
#[derive(Clone)]
pub struct Utterance {
    pub samples: Vec<f32>,
    /// milliseconds since service start when the utterance ended
    pub service_ms: u64,
    /// true for an interim snapshot of speech still in progress (decode it
    /// fast, treat the text as provisional); false for a finished utterance
    pub partial: bool,
    /// Speculative decode requested by a pause (>=100 ms of silence after real
    /// speech): decode at full quality, then the result may be reused as the
    /// final if no further speech arrives (`partial` is also true).
    pub pause: bool,
    /// Which utterance this belongs to; a snapshot and its final share it.
    pub seq: u32,
    /// Frames (10 ms each) of actual speech so far; equal on a snapshot and
    /// its final means no new speech arrived in between.
    pub speech_frames: u32,
    /// ClearSpeech final pass (v0.852.0): true when this utterance should be
    /// re-decoded by the high-accuracy offline engine while the streaming
    /// service is up, and the result used to CORRECT the live transcript
    /// (a `Correction` event) rather than to stage it as a fresh Transcript.
    pub correct: bool,
}

pub const WHISPER_RATE: u32 = 16_000;

/// True when the native (whisper.cpp + Parakeet) engines are compiled in.
pub const fn native_compiled() -> bool {
    cfg!(feature = "native")
}

/// Which real speech engine to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineKind {
    Whisper,
    Parakeet,
}

impl EngineKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "whisper" | "whisper.cpp" | "whispercpp" => Some(Self::Whisper),
            "parakeet" | "parakeet-tdt" | "nvidia" => Some(Self::Parakeet),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Whisper => "whisper",
            Self::Parakeet => "parakeet",
        }
    }

    /// Guess the engine from a model file name (`ggml-parakeet-…` → Parakeet).
    pub fn guess_from_model_path(path: &str) -> Self {
        if path.to_ascii_lowercase().contains("parakeet") {
            Self::Parakeet
        } else {
            Self::Whisper
        }
    }

    pub fn load(self, model_path: &str) -> anyhow::Result<Box<dyn AsrEngine>> {
        Ok(match self {
            Self::Whisper => Box::new(WhisperAsr::load(model_path)?),
            Self::Parakeet => Box::new(ParakeetAsr::load(model_path)?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_kind_parsing_and_guessing() {
        assert_eq!(EngineKind::parse("Whisper"), Some(EngineKind::Whisper));
        assert_eq!(EngineKind::parse("parakeet-tdt"), Some(EngineKind::Parakeet));
        assert_eq!(EngineKind::parse("nope"), None);
        assert_eq!(EngineKind::guess_from_model_path("models/ggml-parakeet-tdt-0.6b-v3-f16.bin"), EngineKind::Parakeet);
        assert_eq!(EngineKind::guess_from_model_path("models/ggml-tiny.en.bin"), EngineKind::Whisper);
    }
}
