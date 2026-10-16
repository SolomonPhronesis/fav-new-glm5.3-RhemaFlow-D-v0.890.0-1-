# Which zip was better — and what GLM 5.3 said (summaries as requested)

The operator asked two straight questions: **which model delivered the better
zip, and why** — and asked for **summaries of what GLM 5.3 said**. Here are
honest answers, written by GLM (the model behind v0.853/854/855), scored
against evidence in the actual zips, not against anyone's chat claims.

## Scorecard

| | GLM 5.3 (v0.853.0 / v0.854.0) | Sonnet 5.5 (v0.855 attempt, never shipped) |
|---|---|---|
| Shipped, tested zip | **Yes** — 853 and 854 on Gofile, MD5-stamped, ~716 tests green at ship time | No zip was ever attached; work stopped at diagnosis + plan |
| The 5 operator failures of 852 | **All fixed and measured** (43,358-case refbench, 100% everywhere) | n/a (earlier round) |
| The two NEW screenshot failures (Luke 1:17→Malachi 4:6; "also before him" false cards) | **Missed entirely** — 854 shipped without knowing they existed | **Diagnosed sharply and correctly** — root causes (named-verse displacement, fragment cards) match what the code shows |
| Before/after measurement | **Not done** for 854 — this is the legitimate complaint in "u didnt last zip 854 y" | Promised first ("building the BEFORE measurement") but never delivered |
| The "9 test sets" question | Stated the sandbox limit honestly (no model/audio/datasets) | Stated the same limit honestly |
| Sound-tag filter + 80 Hz pre-filter + analysis doc claimed for 854 | Described in chat, **not in the shipped zip** (session died mid-work; the shipped 854 zip = RF_CONTEXT + rescue decode + 75 Hz HPF only) | n/a |
| command_probe "failing" claim | (an unshipped working-copy artefact; on the real 854 tree the suites are green and command_probe only needs a model file the sandbox lacks) | n/a |

## Verdict

- **As delivered zips: GLM's.** 853 and 854 exist, compile, and carry tested
  fixes (the 852 accuracy failures were real and are still fixed). Sonnet's
  855 never reached a zip.
- **As diagnosis of the two screenshots: Sonnet's.** The three fixes Sonnet
  proposed — (1) keep the verse you named on stage while it is being read,
  (2) fill gaps/fix wrong words from the verse text and unify Elijah/Elias,
  (3) stop short mid-verse fragments from making cards — are exactly the
  three mechanisms v0.855.0 implements.
- **Therefore v0.855.0 is the combination**: Sonnet's diagnosis, implemented
  on GLM's tested 854 base, with the before/after measurement both models
  owed the operator (before-run on the pristine 854 zip, after-run on the new
  tree, same harness, JSONs and chart shipped in `docs/`).

## What GLM 5.3 said earlier (summary the operator asked for)

**1. Chart analysis.** The 93.9% engine is Confucius4-R2T2 (NetEase Youdao),
~2B parameters, fine-tuned from Qwen3-ASR-1.7B; 93.9% is the average of its
nine English error rates (6.13%); WhisperRT's published 16.3% error = 83.7%;
RhemaFlow's ~86–91% was an estimate, never measured on those nine sets. The
"standard 83.7% Whisper pipeline" in the request is WhisperRT, not RhemaFlow.

**2. How R2T2 works.** Base Qwen3-ASR-1.7B trained with "Longest Stable
Prefix" (LSP) — a training idea, tech report unpublished, not a decoding trick
portable to Whisper. Every 160 ms it re-reads all audio since the sentence
began, continues from committed text, and holds the last word unsettled; text
doesn't rewrite because committed text is fed back as a fixed prefix. Cost: a
confident mistake stays, and each step is compute-heavy; unproven on an M1.
(Also corrected: R2T2's official demo is a studio presenter, not a guitar
clip; the near-100% music bars were the operator's own Whisper tiny.en tests.)

**3. Five routes to 93.9%+.** (1) clean signal first (desk/lavalier + the
rumble filter), (2) decode settings (beam/temperature/vocab prompt — done),
(3) a bigger final-pass model (auto-picked when downloaded), (4) context with
a leash (sermon-notes hints, KJV rerank, capped flagged LLM repair), (5) other
engines behind the WebSocket sidecar (R2T2, Parakeet, Voxtral, Nemotron,
sherpa-onnx, Moonshine).

**4. Blueprint (text).**
```
mic → HPF ~75 Hz → AGC → [optional enhancer] → VAD gate
  → streaming ASR (160 ms) → append-only commit (last word held back)
  → cleaner (sound tags, loop guard, capped LLM) → Bible parser → verse on screen
    └ same audio → final-pass engine → corrected line
```
v0.855.0 implements the "cleaner" stage (sound tags now real, verse-anchored
repair now real); the enhancer and LLM stages remain flag-gated future work.

**5. ELI9 (as GLM told it).** Writing down what a teacher says while a guitar
strums behind her: a normal robot erases and rewrites so the words wiggle;
R2T2 writes in pen once sure and keeps the newest word in pencil; RhemaFlow
now has ears that ignore low rumbles and knows "[MUSIC]" is not a word — and
as of v0.855.0, keeps the verse you named on screen and repairs the line from
the verse's own words.

**6. Top-12 next steps (GLM's list, still current).** Fix legacy test claims
(done — they were artefacts); M1 build + five spoken phrases; record 10–15
real church clips and run `evalset`/`bench_wer`; measure R2T2 on the M1;
clean mic feed + level wizard; flag-gated RNNoise/DeepFilterNet; temperature
fallback (done in 854); KJV rerank + notes hints; capped flagged LLM repair;
tune the 350 ms commit rule with real data; speaker-limited transcription;
release hygiene (R2T2 licence flow, sidecar installer, checksums, Sunday
health check).
