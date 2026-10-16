//! Microphone capture via cpal: device selection, level meter, VAD feeding
//! and recorder tap — all on a dedicated worker thread so the cpal callback
//! stays minimal.

use crate::resampler::{resample, to_mono};
use crate::vad::{PartialPacer, Vad};
use crate::agc::Agc;
use crate::vad::EndHintCell;
use crate::{Utterance, WHISPER_RATE};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct InputDevice {
    pub name: String,
    pub is_default: bool,
}

pub fn list_input_devices() -> Vec<InputDevice> {
    let host = cpal::default_host();
    let default = host
        .default_input_device()
        .map(|d| d.name().unwrap_or_default());
    let mut out: Vec<InputDevice> = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            let name = d.name().unwrap_or_else(|_| "unknown".into());
            out.push(InputDevice {
                is_default: Some(&name) == default.as_ref(),
                name,
            });
        }
    }
    out
}

/// Everything the UI needs to observe the live audio path.
/// Is the microphone stream still delivering audio?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicHealth {
    Ok,
    /// the OS reported a stream error (device removed/switched, etc.)
    Errored,
    /// no audio callbacks for too long — the stream died without an error
    Stalled,
}

/// Pure classification (unit-tested). All times are on the engine's clock
/// (milliseconds, including its start offset).
///  - `now_ms`: current time; `started_ms`: when this stream was opened
///  - `last_cb_ms`: time of the newest audio callback (0 = none yet)
pub fn classify_mic_health(now_ms: u64, started_ms: u64, last_cb_ms: u64, errored: bool) -> MicHealth {
    /// A live input stream delivers callbacks continuously, even in total silence.
    const STALL_MS: u64 = 2_500;
    /// Allow for slow start-up — e.g. the macOS microphone-permission prompt.
    const FIRST_AUDIO_GRACE_MS: u64 = 15_000;
    if errored {
        return MicHealth::Errored;
    }
    if last_cb_ms == 0 {
        return if now_ms.saturating_sub(started_ms) > FIRST_AUDIO_GRACE_MS { MicHealth::Stalled } else { MicHealth::Ok };
    }
    if now_ms.saturating_sub(last_cb_ms) > STALL_MS {
        MicHealth::Stalled
    } else {
        MicHealth::Ok
    }
}

/// What the audio thread hands to a streaming-ASR bridge (see `set_streaming`).
pub enum StreamTap {
    /// 16 kHz mono audio, in capture order
    Audio(Vec<f32>),
    /// our VAD heard the speaker stop: ask the engine to finish the utterance
    Flush,
}

pub struct AudioEngine {
    /// audio + utterance-end markers for a streaming engine (only while enabled)
    pub stream_rx: Receiver<StreamTap>,
    stream_flag: Arc<AtomicBool>,
    /// engine-clock time (ms) the newest audio callback arrived; 0 = none yet
    pub last_callback_ms: Arc<AtomicU64>,
    /// set when the OS reports a stream error
    pub stream_error: Arc<AtomicBool>,
    /// set by the ASR worker while it is mid-decode (interim work backs off)
    pub asr_busy: Arc<AtomicBool>,
    /// endpointing advice from the ASR worker (see `rf_core::end_hint`)
    pub end_hint: Arc<EndHintCell>,
    /// how long the ASR worker's last interim decode took; paces interim work
    pub partial_decode_ms: Arc<AtomicU32>,
    started_ms: u64,
    /// f32 bits of the current input RMS (0..~0.5), for the meter
    pub level: Arc<AtomicU32>,
    /// true while VAD considers the speaker active
    pub speaking: Arc<AtomicBool>,
    /// samples per second of the raw device stream
    pub device_rate: Arc<AtomicU32>,
    /// milliseconds since the engine started (worker-updated clock)
    pub service_ms: Arc<AtomicU64>,
    /// utterances discarded as too short/quiet (never reached the ASR engine)
    pub dropped_count: Arc<AtomicU64>,
    /// wall-clock span of the most recently discarded utterance, in ms
    pub last_dropped_ms: Arc<AtomicU32>,
    /// completed utterances for the ASR worker
    pub utterance_rx: Receiver<Utterance>,
    /// 16 kHz mono chunks for the recorder (only sent while recording)
    pub rec_rx: Receiver<Vec<f32>>,
    stop: Arc<AtomicBool>,
    rec_flag: Arc<AtomicBool>,
    rec_tx: Sender<Vec<f32>>,
    worker: Option<std::thread::JoinHandle<()>>,
    /// keeps the cpal stream alive; dropping it stops the stream
    _stream: Option<Box<dyn FnOnce()>>,
}

impl AudioEngine {
    /// Open `device_name` (None = system default) and start the pipeline.
    pub fn start(device_name: Option<&str>) -> anyhow::Result<Self> {
        Self::start_with_offset(device_name, 0)
    }

    /// Current stream health — see [`classify_mic_health`].
    pub fn health(&self) -> MicHealth {
        classify_mic_health(
            self.service_ms.load(Ordering::Relaxed),
            self.started_ms,
            self.last_callback_ms.load(Ordering::Relaxed),
            self.stream_error.load(Ordering::Relaxed),
        )
    }

    /// Like [`start`], but the engine clock begins at `offset_ms` — used when
    /// the mic is re-opened mid-service so transcript timestamps keep
    /// counting up instead of jumping back to zero.
    pub fn start_with_offset(device_name: Option<&str>, offset_ms: u64) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = match device_name {
            Some(name) => {
                let mut found = None;
                for d in host.input_devices()?.into_iter() {
                    if d.name().ok().as_deref() == Some(name) {
                        found = Some(d);
                        break;
                    }
                }
                found.ok_or_else(|| anyhow::anyhow!("input device not found: {name}"))?
            }
            None => host
                .default_input_device()
                .ok_or_else(|| anyhow::anyhow!("no default input device — check macOS microphone permission"))?,
        };

        let cfg = device.default_input_config()?;
        let rate = cfg.sample_rate().0;
        let channels = cfg.channels() as usize;
        eprintln!(
            "[rf-asr] input device \"{}\" — {} Hz, {} ch, {:?}",
            device.name().unwrap_or_default(),
            rate,
            channels,
            cfg.sample_format()
        );

        let start = Instant::now();
        let last_callback_ms = Arc::new(AtomicU64::new(0));
        let stream_error = Arc::new(AtomicBool::new(false));
        let asr_busy = Arc::new(AtomicBool::new(false));
        let end_hint = Arc::new(EndHintCell::new());
        let partial_decode_ms = Arc::new(AtomicU32::new(0));
        let raw: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let level = Arc::new(AtomicU32::new(0));
        let stream_err_level = level.clone();
        let raw_cb = raw.clone();

        let sample_fmt = cfg.sample_format();
        let mk_err = |flag: Arc<AtomicBool>| {
            move |e: cpal::StreamError| {
                eprintln!("[rf-asr] stream error: {e}");
                flag.store(true, Ordering::Relaxed);
            }
        };
        // heartbeat: every audio callback stamps the time, so a stream that
        // dies silently is noticed instead of looking like "it stopped listening"
        let stream = match sample_fmt {
            cpal::SampleFormat::F32 => {
                let hb = last_callback_ms.clone();
                device.build_input_stream(
                    &cfg.into(),
                    move |d: &[f32], _: &cpal::InputCallbackInfo| {
                        hb.store(offset_ms + start.elapsed().as_millis() as u64 + 1, Ordering::Relaxed);
                        let mono = to_mono(d, channels);
                        let sum: f32 = mono.iter().map(|s| s * s).sum();
                        let rms = (sum / mono.len().max(1) as f32).sqrt();
                        stream_err_level.store(rms.to_bits(), Ordering::Relaxed);
                        raw_cb.lock().unwrap().extend(mono);
                    },
                    mk_err(stream_error.clone()),
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                let hb = last_callback_ms.clone();
                let lv = stream_err_level.clone();
                device.build_input_stream(
                    &cfg.into(),
                    move |d: &[i16], _: &cpal::InputCallbackInfo| {
                        hb.store(offset_ms + start.elapsed().as_millis() as u64 + 1, Ordering::Relaxed);
                        let f: Vec<f32> = d.iter().map(|s| *s as f32 / 32768.0).collect();
                        let mono = to_mono(&f, channels);
                        let sum: f32 = mono.iter().map(|s| s * s).sum();
                        let rms = (sum / mono.len().max(1) as f32).sqrt();
                        lv.store(rms.to_bits(), Ordering::Relaxed);
                        raw_cb.lock().unwrap().extend(mono);
                    },
                    mk_err(stream_error.clone()),
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let hb = last_callback_ms.clone();
                let lv = stream_err_level.clone();
                device.build_input_stream(
                    &cfg.into(),
                    move |d: &[u16], _: &cpal::InputCallbackInfo| {
                        hb.store(offset_ms + start.elapsed().as_millis() as u64 + 1, Ordering::Relaxed);
                        let f: Vec<f32> =
                            d.iter().map(|s| (*s as f32 - 32768.0) / 32768.0).collect();
                        let mono = to_mono(&f, channels);
                        let sum: f32 = mono.iter().map(|s| s * s).sum();
                        let rms = (sum / mono.len().max(1) as f32).sqrt();
                        lv.store(rms.to_bits(), Ordering::Relaxed);
                        raw_cb.lock().unwrap().extend(mono);
                    },
                    mk_err(stream_error.clone()),
                    None,
                )?
            }
            other => anyhow::bail!("unsupported sample format: {other:?}"),
        };
        stream.play()?;

        let (utx, urx) = unbounded::<Utterance>();
        let (rtx, rrx) = unbounded::<Vec<f32>>();
        let stop = Arc::new(AtomicBool::new(false));
        let rec_flag = Arc::new(AtomicBool::new(false));
        let (stx, srx) = unbounded::<StreamTap>();
        let stream_flag = Arc::new(AtomicBool::new(false));
        let stream_flag_w = stream_flag.clone();
        let device_rate = Arc::new(AtomicU32::new(rate));

        // keep the stream alive: park it in a guard that we drop on stop
        let stop_for_guard = stop.clone();
        let stream_guard = Box::new(move || {
            stop_for_guard.store(true, Ordering::Relaxed);
            drop(stream);
        });

        let worker_stop = stop.clone();
        let rec_flag_w = rec_flag.clone();
        let level_w = level.clone();
        let speaking = Arc::new(AtomicBool::new(false));
        let speaking_w = speaking.clone();
        let rtx_w = rtx.clone();
        let asr_busy_w = asr_busy.clone();
        let end_hint_w = end_hint.clone();
        let decode_ms_w = partial_decode_ms.clone();
        let service_ms = Arc::new(AtomicU64::new(offset_ms));
        let service_ms_w = service_ms.clone();
        let dropped_count = Arc::new(AtomicU64::new(0));
        let dropped_count_w = dropped_count.clone();
        let last_dropped_ms = Arc::new(AtomicU32::new(0));
        let last_dropped_ms_w = last_dropped_ms.clone();

        let worker = std::thread::Builder::new()
            .name("rf-audio-worker".into())
            .spawn(move || {
                let mut vad = Vad::new();
                let mut pacer = PartialPacer::new();
                let mut agc = Agc::new();
                // v0.855.0: the 75 Hz rumble high-pass runs before AGC/VAD by
                // default; RF_PREFILTER=0 turns it off (e.g. to A/B the effect
                // on a known-good desk feed). Everything else is unchanged.
                let prefilter_on = std::env::var("RF_PREFILTER").map(|v| v != "0").unwrap_or(true);
                let mut hpf = crate::hpf::Hpf::new(75.0, WHISPER_RATE as f32);
                let mut raw_level = 0.0f32;
                let mut last_ui = Instant::now();
                loop {
                    if worker_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    service_ms_w.store(offset_ms + start.elapsed().as_millis() as u64, Ordering::Relaxed);

                    let chunk: Vec<f32> = {
                        let mut buf = raw.lock().unwrap();
                        std::mem::take(&mut *buf)
                    };
                    if chunk.is_empty() && !vad.speaking() {
                        if last_ui.elapsed().as_millis() > 100 {
                            last_ui = Instant::now();
                            let rms = f32::from_bits(level_w.load(Ordering::Relaxed));
                            // decay meter while silent
                            level_w.store((rms * 0.8).to_bits(), Ordering::Relaxed);
                            speaking_w.store(false, Ordering::Relaxed);
                        }
                        continue;
                    }

                    let mono16 = resample(&chunk, rate, WHISPER_RATE);

                    // recorder tap: everything captured while recording
                    if rec_flag_w.load(Ordering::Relaxed) && !mono16.is_empty() {
                        let _ = rtx_w.send(mono16.clone());
                    }

                    if !mono16.is_empty() {
                        raw_level = (mono16.iter().map(|s| s * s).sum::<f32>() / mono16.len() as f32).sqrt();
                    }
                    // Everything below (VAD, interim snapshots, streaming engine) hears
                    // the RUMBLE-CLEANED, LEVELLED audio; the recorder above keeps the raw audio.
                    // v0.854: high-pass first, so DC drift and HVAC rumble (< ~75 Hz) never
                    // reach the AGC's gain decision or the models' silence floor.
                    // v0.855: RF_PREFILTER=0 bypasses the high-pass (A/B testing).
                    let mono16 = if prefilter_on { hpf.process(&mono16) } else { mono16 };
                    let mono16 = agc.process(&mono16);

                    // Advice from the ASR worker on how long to wait in silence.
                    if let Some((ms, seq, sf)) = end_hint_w.get() {
                        vad.set_end_hint(ms, seq, sf);
                    }
                    pacer.note_decode_ms(decode_ms_w.load(Ordering::Relaxed));

                    let streaming = stream_flag_w.load(Ordering::Relaxed);
                    if streaming && !mono16.is_empty() {
                        let _ = stx.send(StreamTap::Audio(mono16.clone()));
                    }
                    let mut got_final = false;
                    for (u, meta) in vad.feed_with_meta(&mono16) {
                        got_final = true;
                        if streaming {
                            let _ = stx.send(StreamTap::Flush);
                        }
                        let _ = utx.send(Utterance {
                            samples: u,
                            service_ms: service_ms_w.load(Ordering::Relaxed),
                            partial: false,
                            pause: false,
                            seq: meta.seq,
                            speech_frames: meta.speech_frames,
                            correct: false,
                        });
                    }
                    // Interim snapshots: while speech is still going, periodically
                    // hand the ASR worker what's been said so far, so live text and
                    // quotation detection don't have to wait for a pause. Backs off
                    // whenever the worker is busy or behind.
                    if got_final {
                        pacer.reset();
                        end_hint_w.clear();
                    } else if let Some((snap, sf, seq)) = vad.pause_snapshot() {
                        // The speaker paused: decode what we have at full quality
                        // NOW, while the silence timer is still running. If no more
                        // speech arrives, that result is the final transcript.
                        if pacer.should_emit_pause(sf as usize, asr_busy_w.load(Ordering::Relaxed), utx.len()) {
                            let _ = utx.send(Utterance {
                                samples: snap.to_vec(),
                                service_ms: service_ms_w.load(Ordering::Relaxed),
                                partial: true,
                                pause: true,
                                seq,
                                speech_frames: sf,
                                correct: false,
                            });
                        }
                    } else if let Some(snap) = vad.in_progress() {
                        if pacer.should_emit(snap.len(), asr_busy_w.load(Ordering::Relaxed), utx.len()) {
                            const MAX_PARTIAL: usize = 16_000 * 14; // decode at most the last 14 s
                            let tail = &snap[snap.len().saturating_sub(MAX_PARTIAL)..];
                            let _ = utx.send(Utterance {
                                samples: tail.to_vec(),
                                service_ms: service_ms_w.load(Ordering::Relaxed),
                                partial: true,
                                pause: false,
                                seq: vad.seq(),
                                speech_frames: vad.speech_frames() as u32,
                                correct: false,
                            });
                        }
                    } else if !vad.speaking() {
                        pacer.reset();
                    }
                    // vad.feed() may have silently discarded a too-short/quiet
                    // burst inside step_frame; publish that so the UI can say
                    // something instead of an unexplained dead pause.
                    if vad.dropped_count > dropped_count_w.load(Ordering::Relaxed) {
                        dropped_count_w.store(vad.dropped_count, Ordering::Relaxed);
                        last_dropped_ms_w.store(vad.last_dropped_ms as u32, Ordering::Relaxed);
                    }
                    speaking_w.store(vad.speaking(), Ordering::Relaxed);
                    if last_ui.elapsed().as_millis() > 60 {
                        last_ui = Instant::now();
                        level_w.store(raw_level.to_bits(), Ordering::Relaxed); // meter shows the real mic level
                    }
                }
            })?;

        Ok(AudioEngine {
            stream_rx: srx,
            stream_flag,
            last_callback_ms,
            stream_error,
            asr_busy,
            end_hint,
            partial_decode_ms,
            started_ms: offset_ms,
            level,
            speaking,
            device_rate,
            service_ms,
            dropped_count,
            last_dropped_ms,
            utterance_rx: urx,
            rec_rx: rrx,
            stop,
            rec_flag,
            rec_tx: rtx,
            worker: Some(worker),
            _stream: Some(stream_guard),
        })
    }

    /// Start/stop copying audio to `stream_rx` for a streaming ASR engine.
    pub fn set_streaming(&self, on: bool) {
        self.stream_flag.store(on, Ordering::Relaxed);
    }

    pub fn set_recording(&self, on: bool) {
        self.rec_flag.store(on, Ordering::Relaxed);
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.rec_tx.send(Vec::new()).ok(); // unblock
        if let Some(w) = self.worker.take() {
            w.join().ok();
        }
        if let Some(g) = self._stream.take() {
            g();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_stream_with_recent_audio_is_ok() {
        assert_eq!(classify_mic_health(10_000, 0, 9_900, false), MicHealth::Ok);
    }

    #[test]
    fn a_stream_error_is_reported_immediately() {
        assert_eq!(classify_mic_health(10_000, 0, 9_990, true), MicHealth::Errored);
    }

    #[test]
    fn silent_death_is_detected_after_a_gap_in_callbacks() {
        assert_eq!(classify_mic_health(10_000, 0, 7_400, false), MicHealth::Stalled);
        assert_eq!(classify_mic_health(10_000, 0, 7_600, false), MicHealth::Ok, "2.4 s gap is still fine");
    }

    #[test]
    fn slow_first_audio_gets_a_grace_period_but_not_forever() {
        // e.g. the macOS microphone-permission prompt is still showing
        assert_eq!(classify_mic_health(8_000, 0, 0, false), MicHealth::Ok);
        assert_eq!(classify_mic_health(16_000, 0, 0, false), MicHealth::Stalled);
    }

    #[test]
    fn a_restart_offset_does_not_trigger_a_false_stall() {
        // engine re-opened at 500 s into the service; clock includes the offset
        assert_eq!(classify_mic_health(500_400, 500_000, 0, false), MicHealth::Ok);
        assert_eq!(classify_mic_health(500_900, 500_000, 500_850, false), MicHealth::Ok);
    }
}
