# Mic pipeline v0.806 — faster, and measured on YOUR voice

## What you asked for, stated as testable numbers

| Goal | Metric we can actually measure |
|---|---|
| "Transcribes in 1 s" | time from the speaker's last word until the finished text exists (**end-of-speech → text**) |
| "90% accurate on my words" | word accuracy = 1 − WER on sentences **you** read, via `rhemaflow-d calibrate` |

Neither number can be proven from a desk without your voice, your mic and your Mac. This release makes the pipeline *capable* of both and ships the tool that tells you the truth.

## What changed (and why it should help)

1. **Semantic endpointing** (`rf_core::end_hint`). The old pipeline waited a fixed 400/800 ms of silence. Now the ASR worker reads the interim text with the real Scripture parser:
   - finished reference ("John chapter three verse sixteen") → end after ~350 ms
   - unfinished ("…chapter three verse", "…twenty") → wait up to ~1.1 s so the verse number isn't lost
   - chapter only → ~650 ms; bare command ("next verse") → 300 ms
   - ordinary sermon speech → unchanged default
   - lifecycle commands (stop listening/recording) are never shortened.
2. **Decode during the pause.** A pause of ≥100 ms triggers one full-quality decode while the silence timer is still running. If the speaker adds nothing, that result *is* the final transcript — no second decode (matched by utterance id + speech-frame count).
3. **Adaptive interim cadence.** First interim text after ~1.0 s of audio (was 1.4 s); interim decodes every 0.4–1.0 s depending on how fast your machine actually decodes (never faster than 2× the measured decode time).
4. **LocalAgreement** (`rf_core::Agreement`): library for "words two consecutive decodes agree on", the technique from the whisper-streaming line of work. Not yet used to act on spoken references early (see "Not done").
5. **Personal lexicon** (`rf_core::Lexicon`). `rhemaflow-d calibrate` records how the engine mis-hears *you* ("Habakkuk" → "have a cook") and repairs it before parsing. Safety rules: a pattern must be seen twice before it is applied; common words are never triggers; competing repairs need a 2:1 winner; display text is never rewritten.

## Simulated effect (model, not measurement)

`cargo test -p rf-app -- --nocapture latency_sim` runs the real VAD/pacer/end-hint code against a scripted decoder with an assumed decode time:

| assumed decode | old: last word → text | new |
|---|---|---|
| 100 ms | 900 ms | 350 ms |
| 250 ms | 1050 ms | 360 ms |
| 500 ms | 1300 ms | 610 ms |
| 800 ms | 1600 ms | 900 ms |

It also shows a mid-reference breath ("…verse" <0.9 s> "sixteen") staying one utterance instead of being cut at 800 ms.

**Reading it honestly:** ≤1 s is reachable when your decode takes ≲ 600 ms for a short utterance. That depends on the model and chip — `tiny.en` on CPU may do it; larger models need `--metal`. `calibrate` prints your real decode times.

## How to find out if you hit 90% (10 minutes)

```bash
./run.sh --yes                       # builds once
./target/release/rhemaflow-d calibrate          # read 30 sentences when prompted
./target/release/rhemaflow-d calibrate          # run it a SECOND time (repairs activate after 2 sightings)
```
Read: **word accuracy (all)** — your headline number; **held-out before/after** — what the lexicon adds on lines it never saw; **decode time avg / p95** — your latency budget.

If accuracy is under 90%, the lexicon will not rescue a weak model. The biggest lever is the model: compare `--model models/ggml-base.en.bin`, small.en, or `--engine parakeet` using the same calibrate run (identical sentences, so the numbers are comparable).

## Not done (deliberately, with reasons)

- **Acting on spoken references from interim text.** Needs real-voice data on how often interim numbers flip; `Agreement` is ready, wiring it without data would trade accuracy for speed blindly.
- **Streaming transducer engine** (true word-by-word output). Needs a new model + runtime; no model could be downloaded or validated from the build environment.
- **Worker-side parse context.** The endpoint advisor uses a default context, so bare "verse seventeen" (relative to what is on stage) gets the default timing, not the fast path.
- **Live-mic and Metal behaviour** remain untested on a real Mac.
