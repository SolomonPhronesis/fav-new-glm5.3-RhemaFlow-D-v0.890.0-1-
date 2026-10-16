//! Failover between a streaming engine (R2T2 sidecar) and the built-in Whisper.
//!
//! Rules, all driven by evidence the app already has:
//!   * stream not connected                        -> Whisper handles everything;
//!   * stream connected                            -> Whisper stays idle (no wasted CPU/GPU,
//!                                                    no heat competing with the stream engine);
//!   * stream "connected" but silent through a real utterance (hung model, wrong device)
//!     -> that utterance is handed to Whisper after a short grace period, and Whisper
//!        keeps working until the stream produces text again.
//! Pure logic with an injected clock (ms) so every rule is unit-tested.

pub struct Failover {
    grace_ms: u64,
    up: bool,
    last_text_ms: Option<u64>,
    stalled_at: Option<u64>,
    pending: Vec<Pending>,
}

struct Pending {
    id: u64,
    deadline: u64,
    window_start: u64,
    speech_ms: u64,
}

/// Utterances shorter than this may legitimately produce no text (a cough, a click),
/// so silence from the stream during them is not evidence it is broken.
const MIN_SPEECH_FOR_STALL_MS: u64 = 800;

impl Failover {
    pub fn new(grace_ms: u64) -> Self {
        Failover { grace_ms, up: false, last_text_ms: None, stalled_at: None, pending: Vec::new() }
    }

    /// Tell it what the stream bridge currently reports.
    pub fn observe(&mut self, connected: bool, last_text_ms: Option<u64>) {
        self.up = connected;
        self.last_text_ms = last_text_ms;
        if let Some(sa) = self.stalled_at {
            if last_text_ms.map_or(false, |t| t > sa) {
                self.stalled_at = None; // it is talking again
            }
        }
        if !connected {
            self.pending.clear(); // those are handled by `on_final` going forward; held audio is flushed by tick
        }
    }

    pub fn is_up(&self) -> bool {
        self.up && self.stalled_at.is_none()
    }

    /// A finished utterance from the voice detector. `true` = give it to Whisper now.
    pub fn on_final(&mut self, id: u64, speech_ms: u64, now: u64) -> bool {
        if !self.is_up() {
            return true;
        }
        self.pending.push(Pending { id, deadline: now + self.grace_ms, window_start: now.saturating_sub(speech_ms + 1000), speech_ms });
        false
    }

    /// Ids of withheld utterances that the stream never produced text for.
    pub fn tick(&mut self, now: u64) -> Vec<u64> {
        let mut forward = Vec::new();
        let last = self.last_text_ms;
        let mut stalled = false;
        self.pending.retain(|p| {
            if p.deadline > now {
                return true;
            }
            let healthy = last.map_or(false, |t| t >= p.window_start);
            if !healthy {
                forward.push(p.id);
                if p.speech_ms >= MIN_SPEECH_FOR_STALL_MS {
                    stalled = true;
                }
            }
            false
        });
        if stalled {
            self.stalled_at = Some(now);
        }
        forward
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_connected_means_whisper_handles_everything() {
        let mut f = Failover::new(2000);
        assert!(!f.is_up());
        assert!(f.on_final(1, 3000, 10_000), "stream never connected: forward at once");
    }

    #[test]
    fn a_healthy_stream_keeps_whisper_idle() {
        let mut f = Failover::new(2000);
        f.observe(true, Some(9_000));
        assert!(f.is_up());
        assert!(!f.on_final(1, 3000, 10_000), "withheld");
        f.observe(true, Some(10_200)); // text arrived during the utterance
        assert!(f.tick(12_500).is_empty(), "stream produced words: nothing to hand over");
        assert!(f.is_up());
    }

    #[test]
    fn a_connected_but_silent_stream_is_detected_and_its_utterance_rescued() {
        let mut f = Failover::new(2000);
        f.observe(true, Some(1_000)); // last words long before
        assert!(!f.on_final(7, 3000, 20_000));
        assert!(f.tick(21_000).is_empty(), "still inside the grace period");
        assert_eq!(f.tick(22_100), vec![7], "no text for 3 s of speech: hand it to Whisper");
        assert!(!f.is_up(), "and keep using Whisper");
        assert!(f.on_final(8, 2000, 30_000), "next utterances go straight to Whisper");
        // the stream recovers
        f.observe(true, Some(31_000));
        assert!(f.is_up());
        assert!(!f.on_final(9, 2000, 40_000));
    }

    #[test]
    fn a_short_blip_without_text_is_rescued_but_does_not_declare_the_stream_dead() {
        let mut f = Failover::new(2000);
        f.observe(true, None);
        assert!(!f.on_final(1, 400, 5_000));
        assert_eq!(f.tick(7_100), vec![1]);
        assert!(f.is_up(), "a cough proves nothing");
    }

    #[test]
    fn disconnect_switches_to_whisper_immediately_and_reconnect_switches_back() {
        let mut f = Failover::new(2000);
        f.observe(true, Some(100));
        assert!(!f.on_final(1, 3000, 1_000));
        f.observe(false, Some(100));
        assert!(!f.is_up());
        assert!(f.on_final(2, 3000, 2_000));
        f.observe(true, Some(100));
        assert!(f.is_up(), "connected again, nothing stalled");
    }

    #[test]
    fn text_from_before_the_utterance_does_not_count_as_healthy() {
        let mut f = Failover::new(2000);
        f.observe(true, Some(5_000));
        assert!(!f.on_final(1, 2000, 30_000)); // window starts at 27_000
        assert_eq!(f.tick(32_100), vec![1]);
    }
}
