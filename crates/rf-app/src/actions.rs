//! Actions: parsing pipeline, staging, navigation, metrics.

use crate::app::{save_settings, RhemaFlowApp};
use crate::recorder::{EventLog, Recorder, TranscriptLog};
use crate::state::*;
use rf_asr::AsrEngine;
use rf_core::parser::{DetectionKind, ParseContext, Passage};
use std::time::Instant;

/// One zoom step for the A-/A+ buttons and "zoom in/out" (range is 60%..400%).
pub const ZOOM_STEP: f32 = 0.2;

pub enum UiEvent {
    Transcript {
        service_ms: u64,
        text: String,
        latency_ms: u64,
    },
    /// Provisional text for speech that is still in progress.
    Partial { service_ms: u64, text: String },
    /// A short operator-visible status note (e.g. "speech engine recovered").
    Notice(String),
    /// ClearSpeech final pass (v0.843.0): the offline high-accuracy engine
    /// re-decoded a finished utterance; `text` is its corrected transcript
    /// and replaces the draft line that the streaming service produced.
    Correction { service_ms: u64, text: String, engine: String },
}

impl RhemaFlowApp {
    pub fn service_ms(&self) -> u64 {
        self.started_at
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    /// The single entry point every spoken/typed utterance flows through.
    pub fn handle_parse(&mut self, text: &str, service_ms: u64, latency_ms: u64) {
        self.studio_voice(text);
        let fixed_by_learning = self.learner.apply(text.trim());
        let trimmed = fixed_by_learning.as_str();
        if trimmed.is_empty() {
            return;
        }
        // While the app is reading a verse aloud the mic hears the speakers. That
        // echo used to be parsed as a NEW scripture / quotation, which (with
        // auto-read on) restarted the reading - "stop reading" never seemed to
        // work. So while reading (and for a moment after) only a stop request is
        // accepted; everything else heard is our own voice.
        if self.mute_mic_while_reading && (self.tts.echo_active() || self.present.video_sound_playing()) {
            if self.is_stop_request(trimmed) {
                self.stop_reading();
            } else {
                self.short_utterance_note = Some(("ignored while reading aloud (mic hears the speaker) \u{2014} say \"stop\"".to_string(), Instant::now()));
            }
            return;
        }
        // The finished text is authoritative. Tell the log whether the permanent (solid)
        // words shown live turned out to disagree with it - the honest cost of never rewriting.
        let rep = self.live_text.finish(trimmed);
        if rep.disagreed_words > 0 {
            if let Some(log) = &mut self.event_log {
                log.append(service_ms, "live_disagree", &format!("{} of {} live words differed from the final text", rep.disagreed_words, rep.committed_words));
            }
        }

        self.parse_and_stage(trimmed, service_ms, latency_ms, false);
    }

    /// ClearSpeech final pass (v0.843.0): the high-accuracy offline engine
    /// (Parakeet TDT, else Whisper) re-decoded an utterance the streaming
    /// service already transcribed. Replace the draft transcript line with
    /// the corrected text and re-parse it, so a reference misheard over the
    /// music gets staged correctly a moment later.
    ///
    /// Voice commands are NOT re-run from a correction: the draft already
    /// executed them, and a double "next verse" would visibly skip a verse
    /// mid-service. Scripture staging is idempotent (staging the same passage
    /// twice shows the same slide), so re-staging the corrected reference is
    /// always safe — and it is exactly what fixes the misheard verse.
    pub fn handle_correction(&mut self, text: &str, service_ms: u64, engine: &str) {
        let fixed_by_learning = self.learner.apply(text.trim());
        let trimmed = fixed_by_learning.as_str();
        if trimmed.is_empty() {
            return;
        }
        // Only a recent draft line can be the one being corrected. Anything
        // older (or absent — e.g. the stream silently died right then) is
        // treated as a brand-new utterance instead.
        let fresh = self
            .transcript
            .last()
            .is_some_and(|l| service_ms >= l.service_ms && service_ms - l.service_ms < 8_000);
        if !fresh {
            self.handle_parse(trimmed, service_ms, 0);
            return;
        }
        if self.transcript.last().is_some_and(|l| l.text == trimmed) {
            return; // the draft was already right
        }
        self.transcript.pop();
        self.export_lines.pop();
        self.parse_and_stage(trimmed, service_ms, 0, true);
        self.short_utterance_note =
            Some((format!("final pass ({engine}) corrected the last line"), Instant::now()));
    }

    /// Shared tail of every transcript path: parse the finished text, run
    /// commands, stage Scripture, queue reviews and colour the line.
    /// `correction` = the ClearSpeech final pass re-decoding an utterance
    /// (voice commands suppressed, see handle_correction).
    fn parse_and_stage(&mut self, trimmed: &str, service_ms: u64, latency_ms: u64, correction: bool) {
        // A scripture quotation can be split across a natural pause into two
        // utterances ("For God so" … "loved the world…") — offer a short,
        // recent, unflagged tail as fallback context. Capped in count and
        // time so it can only bridge a genuine, immediate pause, never an
        // unrelated earlier remark.
        let recent_owned = self.recent_quote_context(service_ms);
        let recent: Vec<&str> = recent_owned.iter().map(String::as_str).collect();

        // Quotation matching stays on the canonical KJV bible by default; if
        // the operator has switched the on-stage translation, also SUGGEST
        // a match against that translation's own wording when KJV finds
        // nothing — still only ever to the confirm queue, same as any other
        // quotation, so the operator approves knowingly rather than the app
        // silently switching translations underneath them.
        let alt_bible = self.text_bible();
        let alt = (self.translation != "KJV").then_some((alt_bible, self.translation.as_str()));
        // Repair this speaker's known mishearings before parsing (display text
        // stays exactly what the engine heard).
        let repaired = self.lexicon.apply(trimmed);
        let parse_text: &str = if self.lexicon.is_empty() { trimmed } else { repaired.as_str() };
        let outcome = rf_core::parser::parse_full(self.bible, parse_text, &self.parse_ctx, self.floor, &recent, alt);
        let flagged = !outcome.is_empty();
        if outcome.detections.iter().chain(outcome.confirms.iter()).any(|d| d.kind == DetectionKind::Quotation) {
            self.quote_consumed_ms = service_ms; // these words have done their job
        }
        let n_confirms = outcome.confirms.len();
        let n_errors = outcome.errors.len();

        // Transparency: a reasonably long utterance that produced nothing at
        // all gets a brief, visible note instead of a silent gap — the same
        // idea as the VAD's "too short/quiet" note, for "I heard you, but
        // didn't recognize a reference, quotation or command in it."
        if !flagged {
            let word_count = trimmed.split_whitespace().count();
            let has_digit = trimmed.chars().any(|c| c.is_ascii_digit());
            if word_count >= 6 && !has_digit {
                self.short_utterance_note = Some((
                    "heard that — no scripture or command recognized".to_string(),
                    Instant::now(),
                ));
            }
        }

        // colour-code the line in the blue/green transcript panel (v0.831.0):
        // green = this utterance was about a scripture, blue = a voice command.
        // Scripture wins when an utterance contains both ("next is John 3" stages John 3).
        let kind = if !outcome.detections.is_empty()
            || !outcome.confirms.is_empty()
            || !outcome.errors.is_empty()
            || !outcome.suggestions.is_empty()
        {
            LineKind::Verse
        } else if !outcome.commands.is_empty() {
            LineKind::Command
        } else {
            LineKind::Plain
        };

        // v0.855.0 — verse-anchored repair of the line the operator sees
        // (screenshot 1's "to the gods" for "to the wisdom of the just").
        // While a verse is on stage and this utterance is plain speech (no
        // command, no detection, no error), the transcript line is repaired
        // against THAT verse's own text: dropped words filled, garbled
        // endings replaced by the sentence's real words. The export log and
        // transcript log keep the raw ASR text — the repair is display-only,
        // and the interpretation strip says exactly what was done.
        let mut repaired_note: Option<String> = None;
        let plain_speech = outcome.commands.is_empty()
            && outcome.detections.is_empty()
            && outcome.confirms.is_empty()
            && outcome.errors.is_empty()
            && outcome.suggestions.is_empty();
        let mut display_text = if !plain_speech || correction {
            trimmed.to_string()
        } else if let Some(p) = self.parse_ctx.last.filter(|p| p.chapter > 0) {
            match rf_core::repair::repair_line(self.bible, &p, trimmed) {
                Some(r) if r.filled + r.dropped > 0 => {
                    repaired_note = Some(format!(
                        "repaired against {}: {} words filled, {} noise words dropped",
                        p.label(),
                        r.filled,
                        r.dropped
                    ));
                    r.text
                }
                _ => trimmed.to_string(),
            }
        } else {
            trimmed.to_string()
        };
        // v0.890.0 — the KJV post-correction for lines with NO verse on stage
        // (the STT benchmark's top text-layer fix, docs/STT-V0890.md): a
        // finished line that strongly matches exactly one verse is re-rendered
        // from that verse for the DISPLAY. The v0.855 staged-verse repair above
        // still runs first — this catches the reads the operator never named.
        // Same transparency: the note names the verse; logs keep the raw ASR.
        if repaired_note.is_none() && plain_speech && !correction {
            if let Some((p, r)) = rf_core::repair::repair_from_match(self.bible, trimmed) {
                if r.filled + r.dropped > 0 {
                    repaired_note = Some(format!(
                        "repaired against {}: {} words filled, {} noise words dropped",
                        p.label(),
                        r.filled,
                        r.dropped
                    ));
                    display_text = r.text;
                }
            }
        }
        let kind = if repaired_note.is_some() { LineKind::Verse } else { kind };

        self.transcript.push(TranscriptLine {
            service_ms,
            text: display_text,
            flagged,
            kind,
        });
        if self.transcript.len() > 500 {
            self.transcript.remove(0);
        }
        self.export_lines.push(crate::export::Line { ms: service_ms, text: trimmed.to_string() });
        let prefix = match &self.service_start {
            Some((st, _)) => crate::naming::line_prefix(st, service_ms),
            None => format!("[{}]", crate::export::hms(service_ms)),
        };
        if let Some(log) = &mut self.transcript_log {
            log.append(&prefix, trimmed);
            log.flush();
        }

        // ---- voice commands ----
        // Gating rule (v0.801.2): navigation commands only fire when the SAME
        // utterance did not also contain Scripture ("next is John 3" should
        // stage John 3, not advance first). Read-aloud is the exception: it
        // may combine with a reference ("read John 3:16" stages then reads).
        // v0.853.0: a QUOTATION confirm no longer blocks navigation — "The
        // evidence of things not seen. Next verse." is the preacher finishing
        // the verse and asking for the next one; the advance must run. And a
        // utterance that DOES navigate drops its quotation re-match (it would
        // only re-stage what was just read aloud).
        let nav_blocked = rf_core::parser::nav_blocked_by(&outcome);
        let had_nav = outcome.commands.iter().any(|c| {
            matches!(
                c,
                rf_core::Command::NextVerse
                    | rf_core::Command::PrevVerse
                    | rf_core::Command::NextChapter
                    | rf_core::Command::PrevChapter
                    | rf_core::Command::LastScripture
            )
        });
        for cmd in &outcome.commands {
            if correction {
                continue; // the draft utterance already ran its commands
            }
            let is_nav = matches!(
                cmd,
                rf_core::Command::NextVerse
                    | rf_core::Command::PrevVerse
                    | rf_core::Command::NextChapter
                    | rf_core::Command::PrevChapter
                    | rf_core::Command::LastScripture
            );
            if is_nav && nav_blocked {
                continue;
            }
            self.apply_command(cmd);
        }

        // ---- auto-stage detections (the last one wins the stage) ----
        let mut staged_label: Option<String> = None;
        if let Some(det) = outcome.detections.last() {
            let chapter_only = det.kind == DetectionKind::ChapterOnly;
            self.do_stage(det.passage, "voice", det.confidence, chapter_only);
            staged_label = Some(det.passage.label());
            self.auto_staged += 1;
            self.latencies.push(latency_ms);
            for extra in outcome.detections.iter().rev().skip(1) {
                self.index.push(ScriptureIndexEntry {
                    service_ms,
                    label: extra.passage.label(),
                    source: "voice:parallel",
                });
            }
        }

        // ---- operator review queue ----
        self.partial_quote_labels.retain(|(_, t)| t.elapsed().as_secs() < 30);
        for det in &outcome.confirms {
            // A quotation already raised from a live partial (or still pending)
            // must not be queued a second time when the finished utterance lands.
            if det.kind == DetectionKind::Quotation {
                // v0.853.0: a navigation utterance is about MOVING on, not about
                // re-staging what was just read aloud ("...the evidence of things
                // not seen. Next verse." already matches Heb 11:1 on stage).
                if had_nav {
                    continue;
                }
                let label = det.passage.label();
                let raised = self.partial_quote_labels.iter().any(|(l, _)| *l == label);
                let pending_idx = self
                    .confirms
                    .iter()
                    .position(|q| q.det.kind == DetectionKind::Quotation && q.det.passage.label() == label);
                // v0.890.0 (from v0.889): a card raised from a weaker live
                // partial (87%) must not block the finished line (93%) from
                // staging — the 2 Timothy 3:15 bug. decide_raise lets the
                // stronger finished line upgrade or replace the card.
                match crate::partial::decide_raise(pending_idx.is_some(), raised, should_auto_approve_g(det, self.auto_approve_quotes, &self.auto_gates)) {
                    crate::partial::Raise::Skip => continue,
                    crate::partial::Raise::Upgrade => {
                        if let Some(i) = pending_idx {
                            self.confirms.remove(i);
                        }
                    }
                    crate::partial::Raise::Queue => {}
                }
                // v0.853.0: the same quotation while its verse is already on
                // stage (the preacher repeated it, or said it again after a
                // stumble — the operator said it THREE times in one service)
                // must not stack a third card for the same passage.
                if crate::state::quotation_repeat_suppressed(
                    &det.passage,
                    self.parse_ctx.last,
                    self.last_stage_at,
                    std::time::Instant::now(),
                ) {
                    continue;
                }
            }
            // v0.853.0: the interpretation strip shows BOTH sides — what was
            // actually heard and what it matched — instead of echoing the
            // passage label back as if it had been heard that way.
            self.queue_confirm(det, trimmed);
        }
        self.partial_quote_labels.clear(); // the utterance is finished
        self.quote_hold.reset(); // v0.890.0: the hold tracks ONE utterance only

        // ---- v0.853.0: remember the book that is waiting for its chapter ----
        // "We go to Psalms." + "23 verse 1." must complete as Psalms 23:1.
        for det in outcome.confirms.iter().chain(outcome.detections.iter()) {
            if det.kind == DetectionKind::BookOnly && det.confidence >= 0.55 {
                self.parse_ctx.pending_book = Some(det.passage.book);
            }
        }

        // ---- honest out-of-range errors ----
        for err in &outcome.errors {
            self.errors.push(QueuedError {
                id: self.next_id,
                message: err.message.clone(),
                candidates: err.candidates.clone(),
            });
            self.next_id += 1;
        }

        // ---- "three things" suggestions ----
        for sug in &outcome.suggestions {
            self.suggestions.push(sug.clone());
        }

        if let Some(log) = &mut self.event_log {
            if !outcome.detections.is_empty() {
                log.append(service_ms, "parse", "auto-stage");
            } else if n_confirms > 0 {
                log.append(service_ms, "parse", "confirm-queued");
            } else if n_errors > 0 {
                log.append(service_ms, "parse", "out-of-range");
            }
            log.flush();
        }

        // ---- interpretation strip (what the operator just heard → result) ----
        let mut interpret = interpret_outcome(&outcome, staged_label.as_deref());
        if let Some(note) = repaired_note {
            if !interpret.is_empty() {
                interpret.push_str(" — ");
            }
            interpret.push_str(&note);
        }
        self.last_interpret = Some((
            truncate_chars(trimmed, 90),
            interpret,
        ));
    }

    /// The newest pieces of speech to join together for a quotation said in short bits
    /// (v0.816.0: up to 14 pieces from the last 45 seconds, bridging pauses of up to 9 s; pieces
    /// that already produced a quotation are spent). The rules live in `rf_core::pick_recent_pieces`.
    fn recent_quote_context(&self, service_ms: u64) -> Vec<String> {
        let lines: Vec<rf_core::RecentLine> =
            self.transcript.iter().map(|l| rf_core::RecentLine { ms: l.service_ms, text: l.text.clone(), flagged: l.flagged }).collect();
        rf_core::pick_recent_pieces(&lines, service_ms, self.quote_consumed_ms)
    }

    fn queue_confirm(&mut self, det: &rf_core::Detection, heard: &str) {
        // "1 Kings" heard four times in a row (kings-and-priests, "unpiced") used
        // to make four identical cards - one pending card per book is enough.
        if det.kind == DetectionKind::BookOnly
            && self.confirms.iter().any(|q| q.det.kind == DetectionKind::BookOnly && q.det.passage.book == det.passage.book)
        {
            return;
        }
        // The operator's choice: wait for approval (default) or let a clear,
        // unambiguous quotation go straight to the stage.
        if should_auto_approve_g(det, self.auto_approve_quotes, &self.auto_gates) {
            self.do_stage(det.passage, "quotation (auto)", det.confidence, false);
            self.auto_staged += 1;
            // v0.853.0: show BOTH what was heard and what it matched, so a
            // mis-transcription that still landed the right verse is honest
            // ("let this mind being you" -> "Philippians 2:5 (auto)").
            self.last_interpret = Some((
                truncate_chars(heard, 90),
                format!("auto-approved quotation: {}", det.passage.label()),
            ));
            return;
        }
        let preview = match det.kind {
            DetectionKind::BookOnly => String::new(),
            _ => self
                .bible
                .passage(
                    det.passage.book as usize,
                    det.passage.chapter as usize,
                    det.passage.from as usize,
                    det.passage.to as usize,
                )
                .chars()
                .take(220)
                .collect(),
        };
        self.confirms.push(QueuedDetection { id: self.next_id, det: det.clone(), preview });
        self.next_id += 1;
    }

    /// True for "stop reading" in any of its spoken forms, or a bare "stop".
    pub fn is_stop_request(&self, text: &str) -> bool {
        if rf_core::is_bare_stop(text) {
            return true;
        }
        let o = rf_core::parse(self.bible, text, &rf_core::ParseContext::default(), 1.1);
        o.commands.contains(&rf_core::Command::StopReading)
    }

    // ---- the review queue by voice or keyboard ----
    fn first_reviewable(&self) -> Option<usize> {
        self.confirms.iter().position(|q| q.det.passage.chapter > 0)
    }

    pub fn approve_first(&mut self) {
        if let Some(i) = self.first_reviewable() {
            let q = self.confirms.remove(i);
            let src = if q.det.kind == DetectionKind::Quotation { "quotation" } else { "confirm" };
            self.do_stage(q.det.passage, src, q.det.confidence, q.det.kind == DetectionKind::ChapterOnly);
            self.confirm_ok += 1;
            self.touches += 1;
        }
    }

    pub fn dismiss_first(&mut self) {
        if !self.confirms.is_empty() {
            self.confirms.remove(0);
            self.confirm_dismissed += 1;
            self.touches += 1;
        }
    }

    /// "option two": 1 = the best match, 2.. = the other candidates, best first.
    pub fn pick_match(&mut self, n: u8) {
        let Some(i) = self.first_reviewable() else { return };
        let q = self.confirms[i].clone();
        let passage = if n <= 1 { Some(q.det.passage) } else { q.det.alternatives.get(n as usize - 2).copied() };
        if let Some(p) = passage {
            self.confirms.remove(i);
            self.do_stage(p, "quotation", 1.0, false);
            self.confirm_ok += 1;
            self.touches += 1;
        }
    }

    /// Provisional text for speech still in progress: shown live, and — only
    /// for scripture *quotations* (approval-only anyway) — raised for
    /// approval as soon as enough words are in, instead of after the pause.
    /// Everything else waits for the finished utterance; see `partial.rs`.
    pub fn handle_partial(&mut self, text: &str, service_ms: u64) {
        let t = text.trim();
        if t.is_empty() {
            return;
        }
        if self.mute_mic_while_reading && (self.tts.echo_active() || self.present.video_sound_playing()) {
            if self.is_stop_request(t) {
                self.stop_reading(); // stop the moment the word is heard, not after the pause
            }
            return;
        }
        let recent_owned = self.recent_quote_context(service_ms);
        let recent: Vec<&str> = recent_owned.iter().map(String::as_str).collect();
        let alt_bible = self.text_bible();
        let alt = (self.translation != "KJV").then_some((alt_bible, self.translation.as_str()));
        let repaired = self.lexicon.apply(t);
        let parse_text: &str = if self.lexicon.is_empty() { t } else { repaired.as_str() };
        let outcome = rf_core::parser::parse_full(self.bible, parse_text, &self.parse_ctx, self.floor, &recent, alt);
        if !crate::partial::worth_showing(&outcome) {
            return; // "[BLANK_AUDIO]", "[MUSIC]"… — not text to put on screen
        }
        // Append-only only while the streaming engine is the one talking; if Whisper has
        // taken over, its interim text can change, so use agreement instead.
        let streaming_now = self.stream_up.as_ref().map_or(false, |s| s.is_up());
        self.live_text.set_strategy(if streaming_now || (self.stream_up.is_none() && matches!(self.asr_mode, AsrMode::Stream(_))) { rf_core::LiveStrategy::AppendOnly } else { rf_core::LiveStrategy::Agreement });
        let view = self.live_text.update(t);
        self.live_partial = Some((view.full(), Instant::now()));
        self.live_view = view;

        self.partial_quote_labels.retain(|(_, at)| at.elapsed().as_secs() < 30);
        // v0.890.0: the STABILITY HOLD is now a SETTING (default OFF = v0.856
        // snappiness). When it is on, every quotation candidate of this decode
        // is fed to the hold; only those that have been the same candidate in
        // every decode for >= quote_hold_ms may be raised. A candidate that
        // appeared in one decode and moved in the next is never acted on.
        let cands: Vec<&rf_core::Detection> =
            crate::partial::early_quote_confirms(&outcome, &[]).into_iter().collect();
        let cand_labels: Vec<String> = cands.iter().map(|d| d.passage.label()).collect();
        let ready = if self.quote_hold_ms > 0 {
            self.quote_hold.observe(&cand_labels, Instant::now(), std::time::Duration::from_millis(self.quote_hold_ms))
        } else {
            self.quote_hold.reset();
            cand_labels
        };
        let mut fresh: Vec<rf_core::Detection> = Vec::new();
        for det in cands {
            let label = det.passage.label();
            if !ready.contains(&label) {
                continue;
            }
            // v0.890.0 (from v0.889): raised/pending labels are decided by
            // decide_raise, not skipped — a card waiting from a weaker decode
            // must not block this one (it upgrades when strong enough).
            let raised = self.partial_quote_labels.iter().any(|(l, _)| *l == label);
            let pending_idx = self
                .confirms
                .iter()
                .position(|q| q.det.kind == DetectionKind::Quotation && q.det.passage.label() == label);
            match crate::partial::decide_raise(pending_idx.is_some(), raised, should_auto_approve_g(det, self.auto_approve_quotes, &self.auto_gates)) {
                crate::partial::Raise::Skip => {}
                crate::partial::Raise::Upgrade => {
                    if let Some(i) = pending_idx {
                        self.confirms.remove(i);
                    }
                    fresh.push(det.clone());
                }
                crate::partial::Raise::Queue => fresh.push(det.clone()),
            }
        }
        for det in &fresh {
            self.partial_quote_labels.push((det.passage.label(), Instant::now()));
            self.queue_confirm(det, t);
            if let Some(log) = &mut self.event_log {
                log.append(service_ms, "parse", "confirm-queued (live partial)");
                log.flush();
            }
        }
    }

    /// Keep listening no matter what: if the microphone stream has errored or
    /// silently stopped delivering audio (device unplugged or switched, macOS
    /// audio hiccup, sleep/wake…), re-open it — retrying until it comes back.
    pub fn check_mic_health(&mut self) {
        if !self.live {
            return;
        }
        if self.last_mic_restart.map(|t| t.elapsed().as_secs() < 3).unwrap_or(false) {
            return; // don't thrash
        }
        let unhealthy = match &self.engine {
            Some(eng) => eng.health() != rf_asr::MicHealth::Ok,
            None => self.mic_lost, // a previous restart failed — keep trying
        };
        if !unhealthy {
            return;
        }
        self.last_mic_restart = Some(Instant::now());
        eprintln!("[rf-app] microphone stalled or errored — re-opening it");
        self.mic_lost = true;
        self.restart_audio();
        let offset = self.service_ms();
        if self.engine.is_some() {
            self.mic_lost = false;
            self.mic_error = None;
            self.short_utterance_note = Some(("mic reconnected — listening again".to_string(), Instant::now()));
            if let Some(log) = &mut self.event_log {
                log.append(offset, "mic", "reconnected");
                log.flush();
            }
        } else {
            self.mic_error = Some(format!(
                "microphone unavailable — retrying… ({})",
                self.mic_error.clone().unwrap_or_default()
            ));
        }
    }

    pub fn apply_command(&mut self, cmd: &rf_core::Command) {
        use rf_core::Command::*;
        match cmd {
            NextVerse => self.nav_next_verse(),
            PrevVerse => self.nav_prev_verse(),
            NextSlide => self.slide_step(1),
            PrevSlide => self.slide_step(-1),
            MediaPlay => self.run_named("media_play"),
            MediaPause => self.run_named("media_pause"),
            MediaRestart => self.run_named("media_restart"),
            NextChapter => self.nav_next_chapter(),
            PrevChapter => self.nav_prev_chapter(),
            PrevScripture => self.history_back(),
            NextScripture => {
                if self.timeline.can_forward() {
                    self.history_forward();
                } else {
                    self.nav_next_verse();
                }
            }
            LastScripture => {
                if let Some(p) = self.parse_ctx.last {
                    self.do_stage(p, "again", 1.0, false);
                }
            }
            ReadAloud => self.read_aloud(),
            StopReading => self.stop_reading(),
            ZoomIn => self.adjust_zoom(ZOOM_STEP),
            ZoomOut => self.adjust_zoom(-ZOOM_STEP),
            ZoomTo(p) => self.set_zoom_percent(*p),
            OpenStage => self.stage_visible = true,
            CloseStage => {
                self.stage_visible = false;
                self.stage_fullscreen = false;
            }
            FirstVerse => {
                if let Some(p) = self.parse_ctx.last {
                    self.do_stage(Passage::single(p.book, p.chapter, 1), "nav", 1.0, false);
                }
            }
            ApproveQuote => self.approve_first(),
            DismissQuote => self.dismiss_first(),
            PickMatch(n) => self.pick_match(*n),
            AutoApprove(on) => {
                self.auto_approve_quotes = *on;
                save_settings(self);
            }
            ShowAllMatches(on) => {
                self.show_all_matches = *on;
                save_settings(self);
            }
            AutoRead(on) => {
                self.auto_read = *on;
                if !*on {
                    self.stop_reading();
                }
                save_settings(self);
            }
            ReadFaster => self.change_rate(15),
            ReadSlower => self.change_rate(-15),
            Help(on) => self.commands_visible = *on,
            ZoomReset => self.adjust_zoom_reset(),
            ClearStage => self.clear_stage(),
            Fullscreen => self.set_fullscreen(true),
            ExitFullscreen => self.set_fullscreen(false),
            StopListening => {
                // operator-visible: voice cannot stop listening (safety)
                self.transcript.push(TranscriptLine {
                    service_ms: self.service_ms(),
                    text: "voice asked to stop listening — ignored (operator-only control)"
                        .into(),
                    flagged: false,
                    kind: LineKind::Command,
                });
            }
            StartRecording => self.set_recording(true),
            StopRecording => self.set_recording(false),
            Translation(tr) => {
                // any translation that is installed (KJV, BBE and the extra ones); the rest is only noted
                let found = self.translation_names().into_iter().find(|n| n.eq_ignore_ascii_case(tr));
                match found {
                    Some(name) => {
                        self.translation_request = None;
                        self.set_translation(&name);
                        self.short_utterance_note = Some((format!("translation: {name}"), Instant::now()));
                    }
                    None => {
                        self.translation_request = Some((*tr).to_string());
                        self.short_utterance_note = Some((format!("{tr} is not installed (licensed Bibles need a licence: see docs/BIBLE-VERSIONS.md)"), Instant::now()));
                        let ms = self.service_ms();
                        if let Some(log) = &mut self.event_log {
                            log.append(ms, "translation-request", tr);
                        }
                    }
                }
            }
            // ---- v0.816.0 ----
            Blackout(on) => self.blackout = *on,
            SwitchTranslation(dir) => {
                let names = self.translation_names();
                if let Some(i) = names.iter().position(|n| n.eq_ignore_ascii_case(&self.translation)) {
                    let next = (i as i32 + *dir as i32).rem_euclid(names.len() as i32) as usize;
                    let name = names[next].clone();
                    self.set_translation(&name);
                    self.short_utterance_note = Some((format!("translation: {name}"), Instant::now()));
                }
            }
            LastVerse => {
                if self.stage.kind == StageKind::Scripture {
                    if let Some(p) = self.parse_ctx.last {
                        let last = self.text_bible().verse_count(p.book as usize, p.chapter as usize);
                        if last > 0 {
                            self.do_stage(Passage::single(p.book, p.chapter, last as u16), "voice", 1.0, false);
                        }
                    }
                }
            }
            ReadUntil(n) => self.read_until(*n),
            History(on) => self.history_visible = *on,
            Browser(on) => self.browser.visible = *on,
            MediaWindow(on) => self.present.dock_open = *on,
            DismissAll => {
                let n = self.confirms.len();
                self.confirms.clear();
                self.short_utterance_note = Some((format!("dismissed {n} card{}", if n == 1 { "" } else { "s" }), Instant::now()));
            }
            Look(l) => {
                self.stage_style = match *l {
                    "lower_third" => StageStyle::LowerThird,
                    "chroma" => StageStyle::ChromaGreen,
                    "large_print" => StageStyle::LargePrint,
                    _ => StageStyle::Classic,
                };
            }
            StageScreen(n) => match self.displays.get((*n as usize).saturating_sub(1)) {
                Some(d) => {
                    self.output_display = Some(d.name.clone());
                    self.short_utterance_note = Some((format!("stage on screen {n}"), Instant::now()));
                }
                None => {
                    self.short_utterance_note = Some((format!("there is no screen {n} (found {})", self.displays.len()), Instant::now()));
                }
            },
            TimerStart(m) => {
                self.present.timer_ui.choice = crate::present::TimerChoice::CountDown;
                self.present.timer_ui.minutes = *m as u32;
                self.present.timer_start();
                if let Some(c) = self.present.stage_timer() {
                    self.show_content(c);
                }
            }
            TimerPause => self.present.timer_pause(),
            TimerResume => self.present.timer_resume(),
            TimerStop => {
                self.present.timer_reset();
                if self.stage.kind == StageKind::Countdown {
                    self.clear_stage();
                }
            }
            ShowTimer => {
                if let Some(c) = self.present.stage_timer() {
                    self.show_content(c);
                }
            }
            ShowClock => {
                let c = self.present.stage_clock();
                self.show_content(c);
            }
            Strongs(on) => {
                self.strongs.open = *on;
            }
            OriginalFor { lang, word } => self.strongs.lookup_word(*lang, word),
        }
    }

    /// "Read till verse 20": speak from the verse on screen up to verse `n` of the same chapter
    /// (0 = to the end of the chapter). The range goes on the stage so everyone can follow.
    pub fn read_until(&mut self, n: u16) {
        let note = |app: &mut Self, m: String| app.short_utterance_note = Some((m, Instant::now()));
        if self.stage.kind != StageKind::Scripture {
            return note(self, "nothing on the screen to read from".to_string());
        }
        let Some(p) = self.parse_ctx.last else { return };
        let count = self.text_bible().verse_count(p.book as usize, p.chapter as usize) as u16;
        if count == 0 {
            return;
        }
        let to = if n == 0 { count } else { n.min(count) };
        if to < p.from {
            return note(self, format!("verse {to} comes before verse {} which is on the screen", p.from));
        }
        if n > count {
            note(self, format!("this chapter ends at verse {count}"));
        }
        self.do_stage(Passage { book: p.book, chapter: p.chapter, from: p.from, to }, "voice", 1.0, false);
        self.read_aloud();
    }

    // ------------------------------------------------------------------
    // Read aloud (natural TTS, v0.801.2)
    // ------------------------------------------------------------------

    /// Read the on-stage passage aloud. Ranges are spoken verse-by-verse
    /// without the inline verse numbers (hearing "16" before each verse is
    /// unnatural).
    pub fn read_aloud(&mut self) {
        let text = match self.stage.kind {
            StageKind::Scripture => {
                if let Some(p) = self.parse_ctx.last {
                    let bible = self.text_bible();
                    let mut t = String::new();
                    for v in p.from..=p.to {
                        let verse = bible.verse(
                            p.book as usize,
                            p.chapter as usize,
                            v as usize,
                        );
                        if verse.is_empty() {
                            continue;
                        }
                        if !t.is_empty() {
                            t.push(' ');
                        }
                        t.push_str(verse);
                    }
                    if t.is_empty() {
                        self.stage.text.clone()
                    } else {
                        t
                    }
                } else {
                    self.stage.text.clone()
                }
            }
            StageKind::Points => format!("{}. {}", self.stage.label, self.stage.text),
            // lyrics, pictures, video and timers are not read aloud
            StageKind::Blank | StageKind::Slide | StageKind::Media | StageKind::Countdown => return,
        };
        let label = self.stage.label.clone();
        let spoken = if label.is_empty() {
            text
        } else {
            format!("{label}. {text}")
        };
        if let Err(e) = self.tts.speak(&spoken) {
            self.mic_error = Some(format!("read-aloud: {e}"));
        }
    }

    /// Stop the voice at once. Also holds auto-read off for a few seconds so a
    /// verse staged a moment later does not start talking again behind the
    /// operator's back (the Stop button used to lose that race).
    pub fn stop_reading(&mut self) {
        self.tts.stop();
        self.read_stop_until = Some(Instant::now() + std::time::Duration::from_secs(4));
    }

    pub fn change_rate(&mut self, delta: i32) {
        self.tts_rate = (self.tts_rate as i32 + delta).clamp(120, 260) as u32;
        self.tts.set_rate(self.tts_rate);
        save_settings(self);
    }

    pub fn set_zoom_percent(&mut self, pct: u16) {
        self.stage_zoom = (pct as f32 / 100.0).clamp(crate::stage::ZOOM_MIN, crate::stage::ZOOM_MAX);
        save_settings(self);
    }

    pub fn adjust_zoom(&mut self, delta: f32) {
        self.stage_zoom = (self.stage_zoom + delta).clamp(crate::stage::ZOOM_MIN, crate::stage::ZOOM_MAX);
        save_settings(self);
    }

    pub fn adjust_zoom_reset(&mut self) {
        self.stage_zoom = 1.0;
        save_settings(self);
    }

    // ------------------------------------------------------------------
    // Multi-translation engine (v0.801.2): parsing/versification is always
    // KJV-canonical; the on-stage TEXT follows the operator's selection.
    // ------------------------------------------------------------------
    pub fn text_bible(&self) -> &'static rf_bible::Bible {
        match self.translation.as_str() {
            "KJV" => rf_bible::Bible::embedded(),
            "BBE" => rf_bible::Bible::embedded_bbe(),
            other => self
                .extra_bibles
                .iter()
                .find(|(n, _)| n == other)
                .map(|(_, b)| *b)
                .unwrap_or_else(rf_bible::Bible::embedded),
        }
    }

    /// Names for the translation menu: built-ins, then every loaded extra version.
    pub fn translation_names(&self) -> Vec<String> {
        let mut v = vec!["KJV".to_string(), "BBE".to_string()];
        v.extend(self.extra_bibles.iter().map(|(n, _)| n.clone()));
        v
    }

    /// Switch version and redraw the verse that is on stage in the new text.
    pub fn set_translation(&mut self, name: &str) {
        self.translation = name.to_string();
        if self.stage.kind == StageKind::Scripture {
            if let Some(p) = self.parse_ctx.last {
                let text = self.text_bible().passage(p.book as usize, p.chapter as usize, p.from as usize, p.to as usize);
                if !text.is_empty() {
                    self.stage.text = text;
                    self.stage.translation = self.translation.clone();
                    self.stage.updated = Some(Instant::now());
                    self.stage_seq += 1; // the browser and web output refresh too
                }
            }
        }
        save_settings(self);
    }

    pub fn poll_versions(&mut self) {
        while let Ok(ev) = self.version_rx.try_recv() {
            use crate::versions::VersionEvent::*;
            match ev {
                Loaded(name, b) => {
                    self.extra_bibles.retain(|(n, _)| *n != name);
                    self.extra_bibles.push((name.clone(), b));
                    self.extra_bibles.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));
                    self.version_msgs.retain(|(n, _)| *n != name);
                    self.version_msgs.push((name, "ready".into()));
                }
                Failed(name, why) => {
                    self.version_msgs.retain(|(n, _)| *n != name);
                    self.version_msgs.push((name, format!("failed: {why}")));
                }
                Downloading(name) => {
                    self.version_msgs.retain(|(n, _)| *n != name);
                    self.version_msgs.push((name, "downloading\u{2026}".into()));
                }
                Done(_) => {}
            }
        }
    }

    // ------------------------------------------------------------------
    // Operator commands by name: used by keys, the phone remote and the buttons
    // ------------------------------------------------------------------
    pub fn run_named(&mut self, name: &str) {
        // a PDF or HTML page on the stage takes the page / line / zoom keys first (v0.840.0)
        if self.present.has_document() {
            if let Some(a) = crate::docview::action_for_command(name) {
                if self.present.doc_act(a) {
                    return;
                }
            }
        }
        match name {
            // from the phone: show a shelf item (needs the window context, so it is picked up next frame)
            n if n.starts_with("media_") => self.pending_media = n[6..].parse().ok(),
            "next_verse" => self.nav_next_verse(),
            "prev_verse" => self.nav_prev_verse(),
            "next_chapter" => self.nav_next_chapter(),
            "prev_chapter" => self.nav_prev_chapter(),
            "clear" => self.clear_stage(),
            "next_slide" => self.slide_step(1),
            "prev_slide" => self.slide_step(-1),
            "media_play" => {
                if !self.present.media_play() {
                    self.short_utterance_note = Some(("no video on the stage".to_string(), Instant::now()));
                }
            }
            "media_pause" => {
                self.present.media_pause();
            }
            "media_restart" => {
                self.present.media_restart();
            }
            "blackout" => self.blackout = !self.blackout,
            "approve" => self.approve_first(),
            "dismiss" => self.dismiss_first(),
            "pick_1" => self.pick_match(1),
            "pick_2" => self.pick_match(2),
            "pick_3" => self.pick_match(3),
            "pick_4" => self.pick_match(4),
            "pick_5" => self.pick_match(5),
            "plan_next" => self.plan_next(),
            "history_back" => self.history_back(),
            "history_forward" => self.history_forward(),
            "read" => self.read_aloud(),
            "stop_reading" => self.stop_reading(),
            "zoom_in" => self.adjust_zoom(ZOOM_STEP),
            "zoom_out" => self.adjust_zoom(-ZOOM_STEP),
            "toggle_stage" => {
                self.stage_visible = !self.stage_visible;
                if !self.stage_visible {
                    self.stage_fullscreen = false;
                }
            }
            "toggle_fullscreen" => self.set_fullscreen(!self.stage_fullscreen),
            "exit_fullscreen" => self.set_fullscreen(false),
            "help" => self.help_visible = !self.help_visible,
            "media_back" => {
                self.present.media_skip(-10_000);
            }
            "media_fwd" => {
                self.present.media_skip(10_000);
            }
            "media_toggle" => {
                if let Some(t) = self.present.transport() {
                    if t.playing {
                        self.present.media_pause();
                    } else {
                        self.present.media_play();
                    }
                }
            }
            "about" => self.about_visible = true,
            "songs" => self.present.open_at(crate::present::Tab::Songs),
            // v0.835.0 actions are done by extras.rs on the next frame (they need the window context)
            n if crate::extras::is_x_action(n) => self.x.queue.push(n.to_string()),
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Sermon plan: scriptures the preacher will read, staged one key-press at a time
    // ------------------------------------------------------------------
    pub fn load_plan(&mut self) {
        self.plan = plan_from_text(self.bible, &self.plan_text);
        self.plan_pos = 0;
    }

    /// Add scriptures typed or pasted ("John 3:16, Romans 8:28-30") to the end of today's list.
    /// Returns how many were found.
    pub fn plan_add(&mut self, text: &str) -> usize {
        let found = plan_from_text(self.bible, text);
        let n = found.len();
        self.plan.extend(found);
        n
    }

    pub fn plan_remove(&mut self, i: usize) {
        plan_remove_at(&mut self.plan, &mut self.plan_pos, i);
    }

    pub fn plan_move(&mut self, i: usize, down: bool) {
        plan_move_at(&mut self.plan, &mut self.plan_pos, i, down);
    }

    pub fn plan_stage(&mut self, i: usize) {
        if let Some(item) = self.plan.get(i).cloned() {
            self.plan[i].used = true;
            self.plan_pos = i + 1;
            self.do_stage(item.passage, "plan", 1.0, false);
            self.touches += 1;
        }
    }

    pub fn plan_next(&mut self) {
        if self.plan_pos < self.plan.len() {
            self.plan_stage(self.plan_pos);
        }
    }

    // ------------------------------------------------------------------
    // Web output (OBS page + phone remote)
    // ------------------------------------------------------------------
    pub fn web_start(&mut self) {
        self.web = None;
        self.web_error = None;
        match crate::web::WebServer::start(self.web_port, self.web_lan, &self.web_pin) {
            Ok(w) => {
                self.web = Some(w);
                self.web_key = u64::MAX; // force the first push
            }
            Err(e) => self.web_error = Some(format!("cannot start on port {}: {e}", self.web_port)),
        }
    }

    pub fn web_stop(&mut self) {
        self.web = None;
    }

    pub fn web_tick(&mut self) {
        let Some(w) = &self.web else { return };
        // commands from phones
        for c in w.take_commands() {
            self.run_named(&c);
        }
        let Some(w) = &self.web else { return };
        let reviewable: Vec<(u64, String, String)> = self
            .confirms
            .iter()
            .filter(|q| q.det.passage.chapter > 0)
            .take(3)
            .map(|q| (q.id, q.det.passage.label(), if q.det.pct > 0 { format!("{}%", q.det.pct) } else { format!("{:.0}%", q.det.confidence * 100.0) }))
            .collect();
        let media: Vec<(u64, String, bool)> = self.present.shelf.entries.iter().map(|e| (e.id, e.name.clone(), !std::path::Path::new(&e.path).exists())).collect();
        // only push when something changed
        let key = self.stage_seq
            ^ ((media.iter().map(|m| m.0 * (1 + m.2 as u64)).sum::<u64>() + media.len() as u64) << 58)
            ^ ((self.blackout as u64) << 40)
            ^ ((self.stage_style as u64) << 44)
            ^ ((self.stage_bg[0] as u64 | (self.stage_bg[1] as u64) << 8 | (self.stage_bg[2] as u64) << 16 | (self.stage_fg[0] as u64) << 24 | (self.stage_fg[1] as u64) << 32) << 5)
            ^ (((self.stage_zoom * 100.0) as u64) << 20)
            ^ ((self.tts.speaking() as u64) << 50)
            ^ ((reviewable.iter().map(|r| r.0).sum::<u64>() + reviewable.len() as u64) << 52);
        if key == self.web_key {
            return;
        }
        self.web_key = key;
        let st = self.stage.clone();
        let (style, blackout, zoom, reading) = (self.stage_style.key().to_string(), self.blackout, self.stage_zoom, self.tts.speaking());
        let (bg, fg) = (self.stage_bg, self.stage_fg);
        w.update(|s| {
            s.seq = key;
            s.kind = match st.kind {
                StageKind::Scripture => "scripture",
                StageKind::Points => "points",
                StageKind::Slide => "slide",
                StageKind::Countdown => "countdown",
                // pictures and video are not sent to the OBS page in this version (spec S7.1)
                StageKind::Media => "media",
                StageKind::Blank => "blank",
            }
            .into();
            s.label = st.label;
            s.text = st.text;
            s.translation = st.translation;
            s.style = style;
            s.bg = format!("#{:02x}{:02x}{:02x}", bg[0], bg[1], bg[2]);
            s.fg = format!("#{:02x}{:02x}{:02x}", fg[0], fg[1], fg[2]);
            s.blackout = blackout;
            s.zoom = zoom;
            s.reading = reading;
            s.review = reviewable;
            s.media = media;
        });
    }

    // ------------------------------------------------------------------
    // Staging
    // ------------------------------------------------------------------

    pub fn do_stage(&mut self, p: Passage, source: &'static str, conf: f32, chapter_only: bool) {
        if p.chapter == 0 {
            return;
        }
        let text = self.text_bible().passage(
            p.book as usize,
            p.chapter as usize,
            p.from as usize,
            p.to as usize,
        );
        if text.is_empty() {
            return;
        }
        self.stage = StageContent {
            kind: StageKind::Scripture,
            label: p.label(),
            text,
            translation: self.translation.clone(),
            chapter_only,
            updated: Some(Instant::now()),
        };
        self.parse_ctx.last = Some(p);
        // v0.853.0: the new book is also the book "waiting for its chapter" —
        // "Joshua 1:8" on stage, then "We go to Psalms." replaces the pending
        // book, then "23 verse 1." completes the NEWEST book mentioned.
        self.parse_ctx.pending_book = Some(p.book);
        self.last_stage_at = Some(Instant::now());
        self.stage_seq += 1;
        self.blackout = false; // a new verse always comes back on screen
        {
            let label = p.label();
            self.history.retain(|(_, l)| *l != label);
            self.history.insert(0, (p, label.clone()));
            self.history.truncate(14);
            // the ordered list behind "previous scripture" (not when we are only walking it)
            if source != "history" {
                let ms = self.service_ms();
                self.timeline.push(p, label, ms, source, source == "nav");
            }
        }
        let ms = self.service_ms();
        self.index.push(ScriptureIndexEntry {
            service_ms: ms,
            label: p.label(),
            source,
        });
        // BUG (fixed): `truncate(len().max(200))` can never remove anything —
        // len().max(200) is always >= len(), and Vec::truncate only ever
        // shrinks. The on-screen Scripture Index list grew without bound for
        // the life of a service. cap_oldest keeps only the most recent 200
        // (the full, permanent record is unaffected — every entry still
        // reaches events.jsonl on disk via the log write below, regardless
        // of this in-memory display cap).
        crate::state::cap_oldest(&mut self.index, 200);
        self.export_cites.push(crate::export::Cite { ms, label: p.label(), source: source.to_string(), text: self.stage.text.clone() });
        if let Some(log) = &mut self.event_log {
            let detail = format!("{} conf={conf:.2}", p.label());
            log.append(ms, "scripture", &detail);
            log.flush();
        }
        // auto-read: speak newly staged scripture when the operator enabled it
        let held = self.read_stop_until.map_or(false, |t| Instant::now() < t);
        if self.auto_read && !held {
            self.read_aloud();
        }
    }

    pub fn stage_points(&mut self, si: usize) {
        if let Some(sug) = self.suggestions.get(si).cloned() {
            self.stage = StageContent {
                kind: StageKind::Points,
                label: sug.title,
                text: sug.items.join("\n"),
                translation: String::new(),
                chapter_only: false,
                updated: Some(Instant::now()),
            };
            self.touches += 1;
            self.suggestions.remove(si);
            let ms = self.service_ms();
            if let Some(log) = &mut self.event_log {
                log.append(ms, "points", "suggestion approved");
            }
        }
    }

    /// Put something from the Media & slides window on the stage (spec S0.3: it replaces whatever was there).
    pub fn show_content(&mut self, c: StageContent) {
        self.stage = c;
        self.stage_seq += 1;
    }

    pub fn slide_step(&mut self, dir: i32) {
        if let Some(c) = self.present.step_slide(dir) {
            self.show_content(c);
        }
    }

    /// Per-frame work for Stage 2: drop-in files, the window, video frames, a running countdown.
    pub fn present_frame(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            self.present.dock_open = true;
            // dropped on a folder in the media window? then the files go into that folder
            let onto = ctx.input(|i| i.pointer.latest_pos()).and_then(|pos| self.present.folder_at(pos));
            for p in dropped {
                if p.is_dir() {
                    self.present.add_directory(ctx, &p);
                } else {
                    self.present.target_folder = onto.clone();
                    self.present.route_path(ctx, &p, crate::present::PickFor::Media);
                    self.present.target_folder = None;
                }
            }
        }
        if !self.present_bg_tried {
            self.present_bg_tried = true;
            self.present.restore_background(ctx);
        }
        if let Some(id) = self.pending_media.take() {
            if let Some(c) = self.present.stage_entry(ctx, id) {
                self.show_content(c);
            }
        }
        if let Some(c) = self.present.ui(ctx) {
            self.show_content(c);
        }
        // the Greek & Hebrew panel knows which verse is on the screen
        let verse = match (&self.stage.kind, self.parse_ctx.last) {
            (StageKind::Scripture, Some(p)) => Some((p.book as usize, p.chapter as usize, p.from as usize, p.to as usize, self.stage.text.clone())),
            _ => None,
        };
        if let Some(c) = self.strongs.ui(ctx, verse) {
            self.show_content(c);
        }
        let mut st = std::mem::take(&mut self.stage);
        let changed = self.present.tick(ctx, &mut st);
        self.stage = st;
        if changed {
            self.stage_seq += 1; // the OBS page only updates when this number moves
        }
        if let Some(m) = self.present.note_for_app.take() {
            if !self.present.open {
                self.short_utterance_note = Some((m, Instant::now()));
            }
        }
    }

    pub fn clear_stage(&mut self) {
        self.stage = StageContent::default();
        self.stage.updated = Some(Instant::now());
    }

    // ------------------------------------------------------------------
    // Navigation with chapter rollover (PRD M0)
    // ------------------------------------------------------------------

    /// "Previous scripture": one step back through everything shown so far.
    pub fn history_back(&mut self) {
        if let Some(e) = self.timeline.back() {
            self.do_stage(e.passage, "history", 1.0, false);
        }
    }

    pub fn history_forward(&mut self) {
        if let Some(e) = self.timeline.forward() {
            self.do_stage(e.passage, "history", 1.0, false);
        }
    }

    pub fn history_jump(&mut self, i: usize) {
        if let Some(e) = self.timeline.jump(i) {
            self.do_stage(e.passage, "history", 1.0, false);
        }
    }

    pub fn nav_next_verse(&mut self) {
        if self.stage.kind == StageKind::Slide {
            self.slide_step(1); // spec S1.5: the same keys and words step slides
            return;
        }
        if self.stage.kind != StageKind::Scripture {
            return;
        }
        if let Some(p) = self.parse_ctx.last {
            let b = p.book as usize;
            let ch = p.chapter as usize;
            let next = if (p.to as usize) < self.bible.verse_count(b, ch) {
                Passage::single(p.book, p.chapter, p.to + 1)
            } else if ch + 1 < self.bible.chapter_count(b) {
                Passage::single(p.book, p.chapter + 1, 1)
            } else {
                return;
            };
            self.do_stage(next, "nav", 1.0, false);
        }
    }

    pub fn nav_prev_verse(&mut self) {
        if self.stage.kind == StageKind::Slide {
            self.slide_step(-1);
            return;
        }
        if self.stage.kind != StageKind::Scripture {
            return;
        }
        if let Some(p) = self.parse_ctx.last {
            let b = p.book as usize;
            let ch = p.chapter as usize;
            let prev = if p.from > 1 {
                Passage::single(p.book, p.chapter, p.from - 1)
            } else if ch > 1 {
                let vc = self.bible.verse_count(b, ch - 1) as u16;
                Passage::single(p.book, p.chapter - 1, vc)
            } else {
                return;
            };
            self.do_stage(prev, "nav", 1.0, false);
        }
    }

    pub fn nav_next_chapter(&mut self) {
        if let Some(p) = self.parse_ctx.last {
            let b = p.book as usize;
            let ch = (p.chapter as usize) + 1;
            if ch <= self.bible.chapter_count(b) {
                self.do_stage(Passage::single(p.book, ch as u16, 1), "nav", 1.0, true);
            }
        }
    }

    pub fn nav_prev_chapter(&mut self) {
        if let Some(p) = self.parse_ctx.last {
            if p.chapter > 1 {
                self.do_stage(
                    Passage::single(p.book, p.chapter - 1, 1),
                    "nav",
                    1.0,
                    true,
                );
            }
        }
    }

    // ------------------------------------------------------------------
    // Service lifecycle
    // ------------------------------------------------------------------

    pub fn start_service(&mut self) {
        if self.live {
            return;
        }
        // 1. folder = the full date; files = date + start time (v0.812.0, local clock)
        let (start, tz) = crate::naming::now_local();
        let (root, note) = crate::export::resolve_root(&self.save_root);
        let dir = root.join(crate::naming::day_folder_name(&start)).join(crate::naming::service_folder_name(&start, &self.service_title));
        let base = crate::naming::live_base(&start, &self.service_title);
        self.save_notice = note;
        if let Some(free) = crate::export::free_bytes(&root) {
            let hours = free as f64 / crate::export::WAV_BYTES_PER_HOUR as f64;
            if hours < 3.0 {
                let msg = format!(
                    "low disk space: {} free on the save drive (about {:.1} h of recording)",
                    crate::export::human_size(free),
                    hours
                );
                self.save_notice = Some(match self.save_notice.take() {
                    Some(n) => format!("{n}; {msg}"),
                    None => msg,
                });
            }
        }
        self.saved = None;
        self.export_lines.clear();
        self.export_cites.clear();
        self.rec_paths.clear();
        self.service_dir = Some(dir.clone());
        self.live_base = base.clone();
        self.service_start = Some((start, tz.clone()));
        self.event_log = EventLog::create_at(&dir.join(crate::naming::file_name(&base, "events.jsonl")), Some(start)).ok();
        let header = crate::naming::live_header(&start, &tz, &self.service_title, &self.church_name, crate::theme::TITLE);
        self.transcript_log = TranscriptLog::create_at(&dir.join(crate::naming::file_name(&base, "transcript.txt")), &header).ok();
        if let Some(log) = &mut self.event_log {
            log.append(0, "service-start", &format!("{} {}", start.long_date(), start.time_hms()));
        }

        // 2. audio engine
        self.start_audio();

        // 3. reset service state
        self.live = true;
        self.started_at = Some(Instant::now());
        self.transcript.clear();
        self.index.clear();
        self.confirms.clear();
        self.errors.clear();
        self.suggestions.clear();
        self.auto_staged = 0;
        self.touches = 0;
        self.confirm_ok = 0;
        self.confirm_dismissed = 0;
        self.latencies.clear();
        self.parse_ctx = ParseContext::default();
        self.set_recording(true); // recording defaults ON for a live service
    }

    pub fn end_service(&mut self) {
        if !self.live {
            return;
        }
        self.stop_reading();
        let duration = self.service_ms();
        let (end, _) = crate::naming::now_local();
        self.set_recording(false); // closes the recording file cleanly first
        self.live = false;
        self.asr_tx = None; // stop ASR worker
        self.engine = None; // release mic
        self.rec_audio_rx = None;
        if let Some(log) = &mut self.event_log {
            log.append(duration, "service-end", &format!("{} {}", end.long_date(), end.time_hms()));
            log.flush();
        }
        self.stage_visible = false;
        self.stage_fullscreen = false;

        // Save everything under its final name and tell the operator where it is.
        if let (Some(dir), Some((start, tz))) = (self.service_dir.clone(), self.service_start.clone()) {
            let live_base = self.live_base.clone();
            let base = crate::naming::final_base(&start, &end, &self.service_title);
            // release the live files, then rename start-only names -> start-to-end names
            self.event_log = None;
            self.transcript_log = None;
            let mut recording_files = Vec::new();
            let mut wavs = Vec::new();
            for (i, old) in self.rec_paths.clone().iter().enumerate() {
                let new = dir.join(crate::naming::recording_name(&base, i as u32 + 1));
                let target = if std::fs::rename(old, &new).is_ok() { new } else { old.clone() };
                recording_files.push(target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
                wavs.push(target);
            }
            for what in ["events.jsonl", "transcript.txt"] {
                let _ = std::fs::rename(dir.join(crate::naming::file_name(&live_base, what)), dir.join(crate::naming::file_name(&base, what)));
            }
            let meta = crate::naming::ServiceMeta {
                title: self.service_title.clone(),
                church: self.church_name.clone(),
                app_version: crate::theme::TITLE.to_string(),
                start,
                end,
                tz_name: tz,
                duration_ms: duration,
                translation: self.translation.clone(),
                engine: self.engine_label(),
                mic: self.selected_device.clone().unwrap_or_else(|| "default".into()),
                folder: dir.display().to_string(),
                recording_files,
                auto_staged: self.auto_staged as u32,
                approved: self.confirm_ok as u32,
                dismissed: self.confirm_dismissed as u32,
            };
            let written = crate::naming::write_transcripts(&dir, &base, self.formats, &meta, &self.export_lines, &self.export_cites);
            // note what was saved (the events file is closed, so append once by hand)
            if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(dir.join(crate::naming::file_name(&base, "events.jsonl"))) {
                use std::io::Write as _;
                let rec = serde_json::json!({"t_ms": duration, "kind": "saved", "detail": written.join(", "), "clock": end.time_hms(), "date": end.long_date()});
                let _ = writeln!(f, "{rec}");
            }
            // a small .m4a copy of each recording (about a tenth of the size) - slow, so off the UI thread
            if self.compress_audio && wavs.iter().any(|w| w.exists()) {
                let busy = self.compress_busy.clone();
                busy.store(true, std::sync::atomic::Ordering::Relaxed);
                std::thread::spawn(move || {
                    for w in wavs {
                        if w.exists() {
                            let _ = crate::export::compress_to_m4a(&w);
                        }
                    }
                    busy.store(false, std::sync::atomic::Ordering::Relaxed);
                });
            }
            self.saved = Some(SavedInfo {
                files: crate::export::list_files(&dir, &base),
                prefix: base,
                dir,
                refreshed: Instant::now(),
                note: self.save_notice.clone(),
            });
        }
    }

    /// Short description of the speech engine for the transcript header.
    pub fn engine_label(&self) -> String {
        match &self.asr_mode {
            AsrMode::Stream(url) => format!("streaming service {url} (Whisper offline as backup)"),
            _ => "Whisper (offline, on this Mac)".to_string(),
        }
    }

    pub fn set_recording(&mut self, on: bool) {
        if on {
            if !self.live || self.recording {
                return;
            }
            if let Some(dir) = self.service_dir.clone() {
                let part = self.rec_paths.len() as u32 + 1;
                let path = dir.join(crate::naming::recording_name(&self.live_base, part));
                match Recorder::start_at(&path) {
                    Ok(rec) => {
                        if let Some(eng) = &self.engine {
                            eng.set_recording(true);
                        }
                        self.rec_paths.push(path);
                        self.recorder = Some(rec);
                        self.recording = true;
                        let ms = self.service_ms();
                        if let Some(log) = &mut self.event_log {
                            log.append(ms, "recording", &format!("start (part {part})"));
                        }
                    }
                    Err(e) => eprintln!("[rf-app] recorder start failed: {e}"),
                }
            }
        } else if self.recording {
            if let Some(eng) = &self.engine {
                eng.set_recording(false);
            }
            if let Some(rec) = self.recorder.take() {
                let secs = rec.seconds_written();
                rec.finish();
                let ms = self.service_ms();
                if let Some(log) = &mut self.event_log {
                    log.append(ms, "recording", &format!("stop {secs:.0}s"));
                    log.flush();
                }
            }
            self.recording = false;
        }
    }

    pub fn start_audio(&mut self) {
        self.start_audio_with_offset(0);
    }

    /// Re-open the microphone (device switched, or the stream died) without
    /// disturbing the rest of the service: the transcript clock keeps counting
    /// from where it was, and — the part that used to be missed — recording is
    /// switched back on for the new stream, with the time the mic was down
    /// filled with silence so `audio.wav` stays lined up with the Scripture
    /// Index timestamps.
    pub fn restart_audio(&mut self) {
        let offset = if self.live { self.service_ms() } else { 0 };
        // keep whatever the old stream had already captured
        if self.recording {
            if let (Some(rx), Some(rec)) = (&self.rec_audio_rx, &self.recorder) {
                while let Ok(chunk) = rx.try_recv() {
                    rec.push(&chunk);
                }
            }
        }
        self.engine = None; // drops the old stream
        self.rec_audio_rx = None;
        self.asr_tx = None;
        self.start_audio_with_offset(offset);
        if self.recording {
            if let Some(rec) = &self.recorder {
                rec.pad_silence_to_ms(offset);
            }
            if let Some(eng) = &self.engine {
                eng.set_recording(true);
            }
        }
    }

    pub fn start_audio_with_offset(&mut self, offset_ms: u64) {
        self.mic_error = None;
        match rf_asr::AudioEngine::start_with_offset(self.selected_device.as_deref(), offset_ms) {
            Ok(eng) => {
                self.rec_audio_rx = Some(eng.rec_rx.clone());
                self.engine = Some(eng);
                self.devices = rf_asr::list_input_devices();
                self.spawn_asr_worker();
            }
            Err(e) => {
                self.mic_error = Some(format!("{e}"));
                // with a real speech model configured, keep retrying (mic
                // permission granted late, device plugged in…)
                self.mic_lost = self.asr_mode.has_model();
                self.spawn_asr_worker(); // still allow simulator-driven parsing
            }
        }
    }

    pub fn spawn_asr_worker(&mut self) {
        let (tx, rx) = crossbeam_channel::unbounded::<UiEvent>();
        self.ui_rx = rx;
        self.asr_tx = Some(tx.clone());
        // Streaming engine primary + built-in offline engine as a standby that
        // only works while the stream is down or silent (see failover.rs).
        //
        // v0.843.0 ClearSpeech: the SAME high-accuracy offline engine
        // (Parakeet TDT 0.6B → Whisper large-v3-turbo → base.en → tiny.en,
        // best model available) ALSO re-decodes every finished utterance
        // while the stream is up, and its better transcript replaces the
        // streaming draft a moment later. One engine instance serves both
        // jobs — they never decode at the same time (failover runs only when
        // the stream is down, the final pass only when it is up) — so the
        // model is loaded once and shared behind a mutex.
        let mut gated_rx: Option<crossbeam_channel::Receiver<rf_asr::Utterance>> = None;
        let mut standby: Option<(rf_asr::EngineKind, String)> = None;
        let mut asr_shared: Option<std::sync::Arc<std::sync::Mutex<Box<dyn AsrEngine>>>> = None;
        if let (Some(engine), AsrMode::Stream(url)) = (&self.engine, &self.asr_mode) {
            let shared = std::sync::Arc::new(StreamShared::new());
            self.stream_up = Some(shared.clone());
            standby = crate::find_final_model();
            if let Some((kind, path)) = &standby {
                match kind.load(path) {
                    Ok(e) => {
                        asr_shared = Some(std::sync::Arc::new(std::sync::Mutex::new(e)));
                    }
                    Err(e) => {
                        eprintln!("[rf-app] {} load failed: {e}", kind.label());
                        self.mic_error = Some(format!("{} model failed: {e}", kind.label()));
                        standby = None;
                    }
                }
            }
            if let Some(shared_asr) = asr_shared.clone() {
                let label = standby.as_ref().map(|(k, _)| k.label()).unwrap_or("offline engine").to_string();
                let (gtx, grx) = crossbeam_channel::unbounded::<rf_asr::Utterance>();
                let (ftx, frx) = crossbeam_channel::unbounded::<rf_asr::Utterance>();
                let final_on = std::env::var("RF_FINAL_PASS").map(|v| !(v == "off" || v == "0")).unwrap_or(true)
                    && rf_asr::native_compiled();
                if final_on {
                    let _ = tx.send(UiEvent::Notice(format!(
                        "ClearSpeech final pass on — {label} corrects the live transcript (RF_FINAL_PASS=off disables)"
                    )));
                    spawn_final_pass(frx, shared_asr, tx.clone(), label);
                    spawn_failover_router(engine.utterance_rx.clone(), gtx, Some(ftx), shared.clone(), tx.clone());
                } else {
                    drop(ftx); // no final pass: the router behaves exactly like v0.842
                    spawn_failover_router(engine.utterance_rx.clone(), gtx, None, shared.clone(), tx.clone());
                }
                gated_rx = Some(grx);
            }
            self.asr_worker = spawn_stream_bridge(engine, url, tx.clone(), self.floor, shared, standby.is_none());
            if standby.is_none() {
                let _ = tx.send(UiEvent::Notice("no Whisper model found: no backup if the streaming engine stops".into()));
                return;
            }
        }
        if let Some(engine) = &self.engine {
            let utx = gated_rx.unwrap_or_else(|| engine.utterance_rx.clone());
            let asr_shared = match asr_shared {
                Some(a) => a, // streaming mode: engine already loaded and shared with the final pass
                None => {
                    // push-to-talk / demo mode: load the app's own engine as before
                    let asr: Box<dyn AsrEngine> = match &self.asr_mode {
                        AsrMode::Whisper(path) => match rf_asr::EngineKind::Whisper.load(path) {
                            Ok(e) => e,
                            Err(e) => {
                                eprintln!("[rf-app] whisper load failed: {e}");
                                self.mic_error = Some(format!("whisper model failed: {e}"));
                                Box::new(rf_asr::MockAsr::new(vec![]))
                            }
                        },
                        AsrMode::Parakeet(path) => match rf_asr::EngineKind::Parakeet.load(path) {
                            Ok(e) => e,
                            Err(e) => {
                                eprintln!("[rf-app] parakeet load failed: {e}");
                                self.mic_error = Some(format!("parakeet model failed: {e}"));
                                Box::new(rf_asr::MockAsr::new(vec![]))
                            }
                        },
                        AsrMode::Mock => Box::new(rf_asr::MockAsr::demo()),
                        AsrMode::Offline | AsrMode::Stream(_) => Box::new(rf_asr::MockAsr::new(vec![])),
                    };
                    std::sync::Arc::new(std::sync::Mutex::new(asr))
                }
            };
            let busy = engine.asr_busy.clone();
            let hint_cell = engine.end_hint.clone();
            let decode_ms_cell = engine.partial_decode_ms.clone();
            let worker = std::thread::Builder::new()
                .name("rf-asr-worker".into())
                .spawn(move || {
                    use std::sync::atomic::Ordering::Relaxed;
                    let asr = asr_shared; // shared with the ClearSpeech final pass (v0.843.0)
                    let bible = rf_bible::Bible::embedded();
                    let hint_ctx = rf_core::ParseContext::default();
                    // A full-quality decode made during a pause. If the speaker
                    // adds nothing more, it IS the final transcript: no second decode.
                    let mut pause_cache: Option<(u32, u32, Vec<rf_asr::Segment>)> = None;
                    while let Ok(u) = utx.recv() {
                        // an interim snapshot is stale the moment anything newer is queued
                        if u.partial && !utx.is_empty() {
                            continue;
                        }
                        if !u.partial {
                            if let Some((seq, frames, segs)) = pause_cache.take() {
                                if seq == u.seq && frames == u.speech_frames {
                                    for seg in segs {
                                        let ev = UiEvent::Transcript { service_ms: u.service_ms, text: seg.text, latency_ms: 0 };
                                        if tx.send(ev).is_err() {
                                            return;
                                        }
                                    }
                                    continue;
                                }
                            }
                        } else if !u.pause {
                            pause_cache = None; // speech resumed: the old pause decode is stale
                        }
                        busy.store(true, Relaxed);
                        let t0 = Instant::now();
                        // A panic inside a decode must never silently kill the
                        // worker (that would look exactly like "stopped listening").
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let mut g = asr.lock().unwrap_or_else(|p| p.into_inner());
                            if u.partial && !u.pause {
                                g.transcribe_fast(&u.samples)
                            } else {
                                g.transcribe(&u.samples)
                            }
                        }));
                        busy.store(false, Relaxed);
                        let segs = match result {
                            Ok(s) => s,
                            Err(_) => {
                                eprintln!("[rf-asr] a decode panicked — recovered, still listening");
                                let _ = tx.send(UiEvent::Notice("speech engine hiccup — recovered, still listening".into()));
                                continue;
                            }
                        };
                        if u.partial {
                            if !u.pause {
                                decode_ms_cell.store(t0.elapsed().as_millis().min(u32::MAX as u128) as u32, Relaxed);
                            }
                            let text = segs.iter().map(|s| s.text.trim()).collect::<Vec<_>>().join(" ");
                            // Ask the reference parser how long to keep waiting in silence.
                            match rf_core::end_hint(bible, &text, &hint_ctx, 0.75) {
                                Some(h) => hint_cell.set(h.silence_ms, u.seq, u.speech_frames),
                                None => hint_cell.clear(),
                            }
                            if u.pause {
                                pause_cache = Some((u.seq, u.speech_frames, segs));
                            }
                            if !text.is_empty() && tx.send(UiEvent::Partial { service_ms: u.service_ms, text }).is_err() {
                                break;
                            }
                            continue;
                        }
                        for seg in segs {
                            let ev = UiEvent::Transcript {
                                service_ms: u.service_ms,
                                text: seg.text,
                                latency_ms: t0.elapsed().as_millis() as u64,
                            };
                            if tx.send(ev).is_err() {
                                break;
                            }
                        }
                    }
                })
                .ok();
            self.asr_worker = worker;
        }
    }
}

/// State shared between the stream bridge and the failover router. All times are ms since `t0`.
pub struct StreamShared {
    pub t0: Instant,
    pub up: std::sync::atomic::AtomicBool,
    /// u64::MAX = no text yet
    pub last_text_ms: std::sync::atomic::AtomicU64,
}

impl StreamShared {
    pub fn new() -> Self {
        StreamShared { t0: Instant::now(), up: false.into(), last_text_ms: u64::MAX.into() }
    }
    pub fn now_ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }
    pub fn last_text(&self) -> Option<u64> {
        match self.last_text_ms.load(std::sync::atomic::Ordering::Relaxed) {
            u64::MAX => None,
            v => Some(v),
        }
    }
    pub fn is_up(&self) -> bool {
        self.up.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// ClearSpeech final pass worker (v0.843.0): re-decodes finished utterances
/// with the high-accuracy offline engine (Parakeet TDT / Whisper) while the
/// streaming service is up, and reports each corrected transcript to the UI.
/// The streaming draft stays on screen until this better text arrives —
/// typically 0.3–2 s after the speaker stops (M1 Pro, 0.6B model, Metal).
/// Shares ONE engine instance with the failover worker behind a mutex: the
/// two jobs never decode at the same time (up → final pass, down → failover).
fn spawn_final_pass(
    rx: crossbeam_channel::Receiver<rf_asr::Utterance>,
    asr: std::sync::Arc<std::sync::Mutex<Box<dyn AsrEngine>>>,
    tx: crossbeam_channel::Sender<UiEvent>,
    engine_label: String,
) {
    let _ = std::thread::Builder::new().name("rf-finalpass".into()).spawn(move || {
        while let Ok(u) = rx.recv() {
            if u.partial || u.samples.is_empty() {
                continue;
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut g = asr.lock().unwrap_or_else(|p| p.into_inner());
                g.transcribe(&u.samples)
            }));
            let Ok(segs) = result else {
                eprintln!("[rf-finalpass] a correction decode panicked — recovered");
                continue;
            };
            let text = segs.iter().map(|s| s.text.trim()).collect::<Vec<_>>().join(" ").trim().to_string();
            if text.is_empty() {
                continue; // nothing usable — the draft stands
            }
            let ev = UiEvent::Correction { service_ms: u.service_ms, text, engine: engine_label.clone() };
            if tx.send(ev).is_err() {
                return;
            }
        }
    });
}

/// Sits between the voice detector and the offline standby. While the streaming
/// engine is healthy the standby receives nothing; otherwise it gets everything.
///
/// v0.843.0 ClearSpeech: while the stream is healthy, every finished utterance
/// ALSO goes to the final pass (`to_final`), where the high-accuracy offline
/// engine re-decodes it and corrects the streaming draft. Utterances released
/// later by the failover detector (stream proved broken) do NOT go to the
/// final pass: the standby already stages them as full Transcripts.
fn spawn_failover_router(
    from_vad: crossbeam_channel::Receiver<rf_asr::Utterance>,
    to_whisper: crossbeam_channel::Sender<rf_asr::Utterance>,
    to_final: Option<crossbeam_channel::Sender<rf_asr::Utterance>>,
    shared: std::sync::Arc<StreamShared>,
    ui: crossbeam_channel::Sender<UiEvent>,
) {
    let _ = std::thread::Builder::new().name("rf-failover".into()).spawn(move || {
        let mut fo = crate::failover::Failover::new(2000);
        let mut held: std::collections::VecDeque<(u64, rf_asr::Utterance)> = std::collections::VecDeque::new();
        let mut next_id = 0u64;
        let mut was_up = false;
        loop {
            let got = from_vad.recv_timeout(std::time::Duration::from_millis(100));
            let now = shared.now_ms();
            fo.observe(shared.is_up(), shared.last_text());
            match got {
                Ok(u) => {
                    if u.partial {
                        if !fo.is_up() && to_whisper.send(u).is_err() {
                            return;
                        }
                    } else {
                        next_id += 1;
                        let speech_ms = u.samples.len() as u64 * 1000 / rf_asr::WHISPER_RATE as u64;
                        if fo.is_up() {
                            // ClearSpeech final pass (v0.843.0): stream healthy —
                            // the draft came from the service; ask the offline
                            // engine for a corrected version of the same audio.
                            if let Some(fx) = &to_final {
                                let mut c = u.clone();
                                c.correct = true;
                                let _ = fx.send(c);
                            }
                        }
                        if fo.on_final(next_id, speech_ms, now) {
                            if to_whisper.send(u).is_err() {
                                return;
                            }
                        } else {
                            held.push_back((next_id, u));
                            while held.len() > 8 {
                                held.pop_front();
                            }
                        }
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
            }
            for id in fo.tick(now) {
                if let Some(pos) = held.iter().position(|(i, _)| *i == id) {
                    if let Some((_, u)) = held.remove(pos) {
                        if to_whisper.send(u).is_err() {
                            return;
                        }
                    }
                }
            }
            // utterances that were verified healthy can be forgotten
            let up = fo.is_up();
            if up != was_up {
                let _ = ui.send(UiEvent::Notice(if up {
                    "streaming speech engine active".to_string()
                } else {
                    "streaming engine down or silent - backup Whisper is listening".to_string()
                }));
                was_up = up;
            }
        }
    });
}

/// Run the streaming-service bridge on its own thread (see `stream_bridge`).
fn spawn_stream_bridge(
    engine: &rf_asr::AudioEngine,
    url: &str,
    tx: crossbeam_channel::Sender<UiEvent>,
    floor: f32,
    shared: std::sync::Arc<StreamShared>,
    drain_utterances: bool,
) -> Option<std::thread::JoinHandle<()>> {
    use crate::stream_bridge::{Bridge, Out};
    use rf_asr::stream_client::{StreamClient, StreamEvent};
    use rf_asr::StreamTap;

    engine.set_streaming(true);
    let tap = engine.stream_rx.clone();
    let utterances = engine.utterance_rx.clone(); // our own VAD still runs; its decodes are unused here
    let service_ms = engine.service_ms.clone();
    let url = url.to_string();
    std::thread::Builder::new()
        .name("rf-stream-bridge".into())
        .spawn(move || {
            let client = StreamClient::start(&url);
            let mut bridge = Bridge::new(floor);
            let t0 = shared.t0;
            let emit = |outs: Vec<Out>, service_ms_now: u64| -> bool {
                for o in outs {
                    let ev = match o {
                        Out::Partial(text) => UiEvent::Partial { service_ms: service_ms_now, text },
                        Out::Transcript { text, wait_ms, .. } => UiEvent::Transcript { service_ms: service_ms_now, text, latency_ms: wait_ms },
                        Out::Notice(m) => UiEvent::Notice(m),
                    };
                    if tx.send(ev).is_err() {
                        return false;
                    }
                }
                true
            };
            loop {
                let now = t0.elapsed().as_millis() as u64;
                let sm = service_ms.load(std::sync::atomic::Ordering::Relaxed);
                // audio (and VAD utterance-end markers) → service
                loop {
                    match tap.try_recv() {
                        Ok(StreamTap::Audio(a)) => client.send_audio(a),
                        Ok(StreamTap::Flush) => client.flush(),
                        Err(crossbeam_channel::TryRecvError::Empty) => break,
                        Err(crossbeam_channel::TryRecvError::Disconnected) => return, // mic engine gone
                    }
                }
                if drain_utterances {
                    while utterances.try_recv().is_ok() {}
                }
                // service → parser
                while let Ok(ev) = client.events.try_recv() {
                    match &ev {
                        StreamEvent::Connected => shared.up.store(true, std::sync::atomic::Ordering::Relaxed),
                        StreamEvent::Disconnected(_) => shared.up.store(false, std::sync::atomic::Ordering::Relaxed),
                        StreamEvent::Text(t) if !t.trim().is_empty() => shared.last_text_ms.store(now, std::sync::atomic::Ordering::Relaxed),
                        _ => {}
                    }
                    if !emit(bridge.on_event(ev, now), sm) {
                        return;
                    }
                }
                if !emit(bridge.tick(now), sm) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        })
        .ok()
}

fn truncate_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n).collect();
        format!("{t}…")
    }
}

/// One-line operator-readable summary of what the parser did with an
/// utterance — feeds the "HEARD → RESULT" strip in the Command Centre.
fn interpret_outcome(o: &rf_core::ParseOutcome, staged: Option<&str>) -> String {
    if let Some(label) = staged {
        return format!("staged {label}");
    }
    if !o.commands.is_empty() {
        let names: Vec<String> = o.commands.iter().map(command_name).collect();
        return format!("command: {}", names.join(" + "));
    }
    if !o.errors.is_empty() {
        return "out of range — see review queue".into();
    }
    if !o.confirms.is_empty() {
        return "queued for review — confirm on the right".into();
    }
    "no Scripture or command recognized".into()
}

fn command_name(c: &rf_core::Command) -> String {
    c.label()
}
