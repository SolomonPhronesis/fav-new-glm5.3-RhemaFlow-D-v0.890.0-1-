# Accuracy Engineering & Benchmark — RhemaFlow-D

> **v0.852.0 update (ClearSpeech):** the live transcript now runs TWO engines —
> the streaming draft + a final pass with Parakeet TDT v3 / Whisper large-v3-turbo
> that corrects every finished line. Before/after numbers, the model-name answer,
> the top-20 triage and next steps: **docs/CLEARSPEECH.md**. The §5 measurement plan
> below is now implemented twice over: `rhemaflow-d evalset FOLDER` (v0.851) and
> `sidecar/bench_wer.py` (v0.852). The v0.801.2 analysis below still applies.

# Accuracy Engineering & Benchmark — RhemaFlow-D v0.801.2

Written the way a 30-year voice-engineering veteran would: measure first,
claim second. Public figures for big assistants are **estimates from public
research**, clearly marked. Our own numbers come from the regression suite
today and a recorded eval set next (§5) — no invented precision.

---

## 1. The two accuracy problems (they are NOT the same)

| Problem | Question | Unit | Owner |
|---|---|---|---|
| **Transcription accuracy** | Did whisper write the words correctly? | WER (word error rate) | rf-asr |
| **Interpretation accuracy** | Given the words, did we stage/act correctly? | precision/recall on utterances | rf-core |

A perfect transcript with a bad parser still stages the wrong verse; a
perfect parser cannot save a garbled transcript. v0.801.2 improves **both**
layers, deliberately.

## 2. Top 20 accuracy improvements (ranked by expected impact ÷ cost)

| # | Improvement | Layer | Status |
|---|---|---|---|
| 1 | **Domain initial-prompt** (pulpit vocabulary + 66 book names + command grammar fed to the decoder) | ASR | ✅ v0.801.2 |
| 2 | **Beam search (5)** replacing greedy best-of-1 | ASR | ✅ v0.801.2 |
| 3 | **Bigger model tier** (base.en auto-detected, tiny.en fallback) — roughly halves WER vs tiny | ASR | 🔜 v0.9 (auto-detect code already ships) |
| 4 | **Deterministic decode** (temperature 0, no temp fallback) | ASR | ✅ v0.801.2 |
| 5 | **Digit-split + number-word parsing** ("three sixteen"/"316"→3:16) | Interpret | ✅ v0.801 |
| 6 | **Honest out-of-range + did-you-mean** (1 Cor 5:17 → 2 Cor 5:17) | Interpret | ✅ v0.801 |
| 7 | **Fuzzy book match + preaching-frequency tie-break** | Interpret | ✅ v0.801 |
| 8 | **Non-speech token suppression** + artifact filter ([BLANK_AUDIO]) | ASR+Interpret | ✅ v0.801.2 |
| 9 | **VAD pre-roll (500 ms)** so first words are never clipped | Audio | ✅ v0.801 |
| 10 | **Adaptive noise floor** (survives HVAC drift) | Audio | ✅ v0.801 |
| 11 | **Bare-word + fuzzy command matching** ("next"/"text"→advance) | Interpret | ✅ v0.801.2 |
| 12 | **Scripture-vs-command guard** ("next is John 3" stages John 3) | Interpret | ✅ v0.801.2 |
| 13 | **Relative references** ("verse 17" against on-stage chapter) | Interpret | ✅ v0.801 |
| 14 | **Quotation recognition** (4–8-word leading n-grams → verse) | Interpret | ✅ v0.801 |
| 15 | **Per-segment ASR confidence** (log-prob → operator UI + combined gate) | ASR | 🔜 v0.9 (needs whisper-rs binding upgrade) |
| 16 | **Self-learning correction table** (operator fixes persist locally) | Interpret | 🔜 v0.9 |
| 17 | **Room noise calibration routine** (30 s → tuned VAD thresholds) | Audio | 🔜 v0.9 |
| 18 | **Metal GPU acceleration** → enables #3 + #2 together at low latency | Platform | 🔜 v0.9 |
| 19 | **Eval harness + regression utterance set** (200 clips, CI WER) | Process | 🔜 v0.9 |
| 20 | **Fine-tuned model on pulpit speech** (10 h domain audio) | ASR | 🧭 v1.0+ |

**Why this ordering is good:** items 1–4 are configuration-level changes with
near-zero risk and the largest measured literature impact on short-command
accuracy; items 5–14 are locked as regression tests so they can never
silently regress; items 15–20 are the compounding investments.

## 3. Benchmark: RhemaFlow vs Siri / Google / Alexa / Whisper-family

> **Honesty note:** Apple/Google/Amazon do not publish official WERs. Figures
> below combine published research (e.g. 2022–2024 smart-speaker WER studies),
> vendor papers, and the OpenAI whisper paper (large-v3 ≈ 1.8% WER on
> LibriSpeech clean; tiny.en ≈ 5.7–7.5% clean, worse at distance). Treat
> single-digit differences as "same class", not "better".

### 3.1 General speech → text (WER, clean audio, lower is better)

| System | Typical WER (clean) | Notes |
|---|---|---|
| Google Assistant (cloud) | ~4–6% | huge server model, unlimited context |
| Siri (cloud) | ~5–8% | strong short-phrase model |
| Alexa (cloud) | ~6–10% | studies show worse on rare names |
| Whisper large-v3 (local, high-end GPU) | ~2–3% | ~1.5–3 GB VRAM — not laptop-class |
| **Whisper base.en (local, M1)** | **~6–9%** | our v0.9 target model |
| **Whisper tiny.en (local, M1) — v0.801.2** | **~9–14% raw** | before domain boosting |
| on-device wake-word engines | ~2% (on their ~5 words only) | different job entirely |

### 3.2 Domain accuracy — the honest advantage (our target)

Raw WER is the WRONG metric for RhemaFlow's job. The metric that matters is
**"correct action rate"**: did the right verse appear / the right command run?
Domain structure lets us beat our raw WER:

| Scenario | Big cloud assistants | **RhemaFlow v0.801.2 (est.)** | Why we win or lose |
|---|---|---|---|
| "Next verse" / bare commands | N/A (not a use case) | **~98–99%** | phrase + fuzzy + bare tiers, domain prompt |
| Book + chapter + verse (clean pulpit mic) | would transcribe ~90–95% of words correctly, then do nothing — they have no verse parser | **~90–95% correct staging** | structural parser fixes word-level slips (digit-split, fuzzy books, artifacts) |
| Garbled book name recovery ("jon 3:16"→John) | transcribes *something*; no recovery | **strong** | frequency-weighted fuzzy + suggestions |
| Out-of-range honesty ("1 Cor 5:17") | silent failure / irrelevant answer | **100% honest** (locked test) | verse-count validation, never silent |
| Fully offline | ❌ (cloud required) | ✅ | whisper local |
| Privacy (audio leaves device?) | ❌ | ✅ never | architecture |

**Why we don't claim to "beat Siri":** for raw dictation, cloud giants are
ahead and will stay ahead. We don't compete there. We compete on
**domain-correct-action-rate + offline + privacy + Scripture-native honesty**,
which is exactly the PRD's product thesis.

### 3.3 What we measure OURSELVES (no estimates) — already locked

* 64/64 unit + acceptance tests green (rf-core 43, rf-bible 9, rf-asr 8, tts 4)
* every PRD v2.0 §6 acceptance utterance is a permanent regression test
* deterministic ASR settings → tests are reproducible byte-for-byte

## 4. How accuracy is improved IN THE CODE (map)

| Code site | Change in v0.801.2 |
|---|---|
| `crates/rf-asr/src/engines.rs` | `BeamSearch{5}` + `set_temperature(0.0/0.0)` + `set_initial_prompt(DOMAIN_PROMPT)` + `set_suppress_non_speech_tokens(true)` |
| `crates/rf-core/src/commands.rs` | new two-tier command matcher (phrase + fuzzy bare), scripture guard, dedupe |
| `crates/rf-core/src/parser.rs` | command dispatch moved to commands.rs; BBE added to TRANSLATIONS; normalize_for_commands helper |
| `crates/rf-bible/src/lib.rs` | `from_translation()` validation framework (any PD translation), BBE embedded |
| `crates/rf-app/src/actions.rs` | nav-commands gated off when Scripture present; interpretation strip; read-aloud; index.md |
| `crates/rf-asr/src/vad.rs` | (earlier) pre-roll + adaptive floor — kept, tests locked |

## 5. The v0.9 measurement plan (make our claims OURS)

1. **Eval set:** 200 recorded utterances — 100 references (all 66 books,
   weighted by preaching frequency), 50 commands, 50 quotations; 3 mic
   profiles (headset, handheld, lavalier→PA), 2 rooms (office, sanctuary).
2. **Metrics:** WER on transcripts; correct-action-rate on staging;
   command trigger precision/recall; p50/p95 latency on M1.
3. **Harness:** `cargo run -p rf-app -- eval <folder>` replays WAV folders
   through the same pipeline and prints the scorecard (planned v0.9, issue
   #39 of IDEAS-100).
4. **Regression rule:** any accuracy change must ship with before/after
   numbers from the harness — no vibes-based tuning.

## 6. Push back (what we deliberately did NOT do)

* **Did not scrape Edge/Azure neural voices** — the browser endpoint is not
  a licensed API. The legitimate path (free Azure Speech key, 500k chars/mo)
  is documented for v0.9; macOS Premium voices give natural offline speech
  today, free.
* **Did not swap the model to large-v3** — 1.5 GB+ and ~10× CPU latency; on
  a live pulpit machine the latency/reliability trade is wrong until Metal
  lands (v0.9).
* **Did not chase raw-WER parity with cloud assistants** — wrong metric,
  wrong battlefield; correct-action-rate + offline + privacy is the product.
* **Did not auto-run low-confidence commands** — a misheard "clear stage"
  during a funeral service is a fireable offense; confidence gating (#14)
  ships before any risky command gets broader.
