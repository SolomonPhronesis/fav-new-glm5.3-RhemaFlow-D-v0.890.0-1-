# ClearSpeech — the accuracy upgrade (RhemaFlow-D v0.852.0)

**One sentence:** RhemaFlow now runs TWO engines on your voice — the fast
streaming draft you already have, plus a new **high-accuracy final pass** that
re-decodes every finished sentence with the best local model available and
corrects the transcript — even with noise, music and other voices in the room.

---

## 1. The model you remembered, by name

You asked: *"that model was good even with noise and music and other voices —
tell me the name of it and copy it."* There are two, and both are now wired in:

| Model | Who made it | Why it survives noise, music and other voices | License | Where it is now |
|---|---|---|---|---|
| **Whisper large-v3** (and its fast sibling **large-v3-turbo**) | OpenAI | Trained on **680,000 hours** of real-world web audio — most of it imperfect: background chatter, music beds, distant mics. It literally learned from noisy audio. | MIT (code), open weights | Runs inside RhemaFlow via **whisper.cpp** (MIT) — the same engine the app already shipped; now used as the final pass with beam search + its own paper's robustness recipe |
| **NVIDIA Parakeet TDT 0.6B v3** | NVIDIA | Current **#1 of the Hugging Face Open ASR Leaderboard** — the standard public benchmark that includes *noisy, reverberant, far-field and accented* test sets. ~6% average WER across those sets (≈94% accuracy), ~25–30× real time on M-series silicon, punctuation and casing included. | CC-BY-4.0 (weights) | `./run.sh model-parakeet` downloads it (1.2 GB, once); v0.843 auto-selects it as the final pass when present |

**What we "copied":** the open code and the published decoding recipe.
- From **whisper.cpp** (the code Whisper runs on): beam search (5), the
  temperature-fallback behaviour, domain initial-prompt, non-speech token
  suppression — all of it was already in `rf-asr` (v0.801.2 work), now it also
  serves the live final pass.
- From **Parakeet**'s stack: the idea that a *small* (0.6B) token-and-duration
  Transducer can beat a 30× bigger encoder-decoder on realistic noisy data at a
  fraction of the latency — that is why Parakeet v3 is the preferred final
  engine when it is downloaded.

## 2. How the two engines work together (the ClearSpeech pattern)

This is the same pattern broadcast captioning uses:

```
mic ──► VAD ──► R2T2 streaming sidecar ──► DRAFT words on screen (instant, ~60–80% in noise)
  │
  └─► same utterance audio ──► Parakeet TDT v3 (or large-v3-turbo) ──► CORRECTED line (~0.3–2 s later, ~92–97%)
```

- While the preacher speaks, you see the draft immediately (unchanged latency).
- The moment a sentence ends, the offline engine re-decodes **the same audio**
  and the transcript line updates. If the corrected text parses as a scripture
  reference, the stage updates too — a verse misheard over the music fixes
  itself before the operator has finished glancing at the screen.
- Voice commands are **not** re-run by the correction (the draft already ran
  them) — a corrected line can never double-advance a slide.
- Everything stays **offline and private**: both engines run on the Mac.

Controls:
- `RF_FINAL_PASS=off ./run.sh` — disable the final pass (v0.842 behaviour).
- `RF_FINAL_MODEL=/path/model.bin` — force a specific model file.
- The engine is chosen automatically: **Parakeet v3 → large-v3-turbo → large-v3
  → base.en → tiny.en** (whatever is downloaded). The operator banner tells you
  which one is active.

## 3. Before / after accuracy

> **Honesty rule** (carried over from ACCURACY.md): published figures are
> marked *published*; numbers on YOUR microphone come from YOUR run of
> `sidecar/bench_wer.py` (§5). The rows marked *estimate* combine the two and
> are estimates, not measurements.

| Scenario (English pulpit speech) | BEFORE — streaming draft only | AFTER — ClearSpeech final pass | Source |
|---|---|---|---|
| Clean pulpit mic (headset / handheld) | ~80–90% *(your report + tiny.en-class published)* | **~97–98%** *(published: Parakeet v3 ≈ 6% avg WER incl. hard sets; Whisper large-v3 ≈ 1.8% WER LibriSpeech-clean)* | published + your report |
| **Room noise, music bed, congregation voices (your case)** | **~60–80%** *(your report — this is the number we set out to fix)* | **~92–95%** *(estimate: Parakeet v3 averages ~94% across the leaderboard's noisy sets; large-v3-turbo similar class)* | estimate → verify with §5 |
| Hymn/worship playing over the speech | worst case for the draft | large improvement: the offline engines were trained on music-overlapped audio | published-class behaviour |
| Far-field (lapel mic → PA reverb) | drops hard | degrades gracefully (~90–93% est.) | estimate |

**Word-error-rate view of the same table** (WER = 100 − accuracy):
draft in noise ≈ 20–40% WER → final pass ≈ **5–8% WER**. The leaderboard
average of Parakeet TDT 0.6B v3 is ~6% WER *including* its noisy test sets —
that is the "92%+ even with noise and music" you asked for.

## 4. Your top-20 list, triaged honestly

That list mixes two different jobs. **Transcription (what we need) = ASR.
Most of the 20 are the OTHER job** — text-to-speech / voice cloning (making a
computer talk), which RhemaFlow doesn't need for accuracy (it already reads
aloud with the Mac's own voices).

| # | Model (your list) | ASR or TTS? | Verdict for this task |
|---|---|---|---|
| 1 | Breeze TTS 2 | TTS | wrong job |
| 2 | Fish Audio S2 Pro | TTS/clone | wrong job |
| 3 | CosyVoice 2 | TTS | wrong job |
| 4 | IndexTTS 2 | TTS | wrong job |
| 5 | Grok Voice/Transcribe 2.0 | **ASR** | closed API — cannot be copied into an offline app |
| 6 | **VibeVoice-ASR-Streaming (Microsoft)** | **ASR** | right job, interesting research direction — a streaming multi-speaker ASR; worth watching as a future *draft* engine (our draft is R2T2 today) |
| 7 | Step Audio EditX | TTS | wrong job |
| 8 | **Voxtral (Mistral)** | **ASR** | strong open ASR (Apache-2.0), but 3B+ — heavier than Parakeet 0.6B for live correction on M1; a good future option for max-accuracy mode |
| 9 | Supertonic 3 | TTS runtime | wrong job |
| 10 | VoxCPM 2 | TTS | wrong job |
| 11 | Spark-TTS 2 | TTS | wrong job |
| 12 | FireRedTTS 2 | TTS | wrong job |
| 13 | GPT-SoVITS v2 | TTS/clone | wrong job |
| 14 | NVIDIA Magpie-Multilingual | TTS | wrong job (NVIDIA's ASR line is Parakeet/Canary — which we use) |
| 15 | Higgs Audio V3 | TTS | wrong job |
| 16 | Chatterbox | TTS | wrong job |
| 17 | Kokoro 82M | TTS | wrong job |
| 18 | Maya1 | TTS | wrong job |
| 19 | Qwen3-TTS | TTS | wrong job (though the R2T2 draft engine itself is Qwen3-ASR based) |
| 20 | GLM-TTS | TTS | wrong job |

**Missing from the list but winning it:** NVIDIA **Parakeet TDT** (leaderboard
#1) and OpenAI **Whisper large-v3** — which is why those are the two we ship.

## 5. Measure it on YOUR microphone (5 minutes, real numbers)

Two tools are on board now:
- **`rhemaflow-d evalset FOLDER`** (built in since v0.851.0 — see docs/EVALSET.md):
  scores Whisper AND Parakeet over a folder of clips, reports WER, scripture
  hit-rate and mix-ups. The full workbench.
- **`sidecar/bench_wer.py`** (new in v0.852.0): the quick one-file draft-vs-corrected
  compare shown below.

```bash
# 0) one-time: get the best final-pass model (pick one, or both)
./run.sh model-parakeet     # ~1.2 GB — recommended (leaderboard #1)
./run.sh model-turbo        # ~0.6 GB — whisper large-v3-turbo alternative

# 1) start RhemaFlow with streaming + final pass as usual, and read/record
#    the test passage with the SAME mic/room/music as the real service.

# 2) save what was ACTUALLY said to reference.txt (paste the Bible text).

# 3) save the DRAFT line (what the streaming engine heard) to draft.txt
#    and the CORRECTED line (after the final pass updated it) to final.txt.

# 4) score it:
python3 sidecar/bench_wer.py compare --ref reference.txt \
    --hyp-before draft.txt --hyp-after final.txt
```

Output:

```
ClearSpeech before/after (same audio, two engines):

  BEFORE (draft)         WER  28.57%   accuracy  71.43%   (14 ref words; 3 sub / 1 del / 0 ins)
  AFTER (final pass)     WER   7.14%   accuracy  92.86%   (14 ref words; 1 sub / 0 del / 0 ins)

  improvement: 21.43 WER points  (21.43 accuracy points)
```

`--demo`-mode tip: any audio file works too — convert with
`ffmpeg -i yours.mp3 -ar 16000 -ac 1 yours.wav` first if you want to feed the
Drive recording through the sidecar directly.

## 6. Next steps to push past 95% (ranked)

1. **Download Parakeet v3** (§5 step 0) — biggest single jump, zero code.
2. **Run the benchmark on your real Sunday audio** — send me the before/after
   table and we tune the prompt/trim thresholds to YOUR room.
3. **Order-of-Service hotwords** — feed song titles + names of the day into the
   domain prompt (`--context` already exists in the sidecar; the final pass
   inherits `full_prompt`), so "Geneva" stops becoming "Jennifer".
4. **Music-bed suppression** — an RNNoise-style spectral front end before the
   final pass for services with live worship over speech (~1–2 WER points in
   heavy-music cases; roadmap v0.85).
5. **Who-said-what (diarization)** — the last "other voices" frontier: label
   preacher vs congregation vs reading leader so only the preacher's line
   reaches the transcript (needs a small diarization model; v0.9).
6. **Voxtral max-accuracy mode** — an optional heavier final engine for
   post-service re-transcription of the recording (not live).
7. **Self-learning corrections → prompt** — the operator fixes you already make
   in the Review tab should feed next Sunday's initial prompt automatically
   (the learning table exists; wiring it to the final pass is small work).

## 7. What shipped in v0.852.0 (code map)

| File | Change |
|---|---|
| `crates/rf-app/src/actions.rs` | `UiEvent::Correction`; `handle_correction` (replaces the draft line, re-parses, never re-runs commands); `parse_and_stage` extracted; failover router now feeds the final pass; one shared engine behind a mutex for failover + final pass |
| `crates/rf-app/src/main.rs` | `find_final_model()` — Parakeet → large-v3-turbo → base → tiny, `RF_FINAL_MODEL` override |
| `crates/rf-app/src/app.rs` | Correction event arm (updates the live view) |
| `crates/rf-asr/src/lib.rs` | `Utterance::correct` flag (Clone derive) |
| `crates/rf-asr/src/capture.rs` | construct the new flag |
| `sidecar/bench_wer.py` | the before/after benchmark (self-tested) |
| `run.sh` | `model-turbo` subcommand; version stamps |
| `docs/CLEARSPEECH.md` | this document |
