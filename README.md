# RhemaFlow-D v0.839.0 (see CHANGELOG.md)

**The preacher speaks. RhemaFlow understands.**

An intelligent church presentation and sermon-intelligence desktop app for
**Apple Silicon (M1) Macs**, written in Rust. RhemaFlow listens to the live
service, recognizes spoken Scripture references and quotations, retrieves the
passage from the embedded full KJV, and puts it on the congregation stage —
automatically, with the media operator always in control.

100% offline on the live path: speech recognition, parsing, stage display and
recording all run locally. Nothing leaves the machine. No network required
during a service.

---

## Quick start

```bash
./run.sh
```

First run installs what's missing (Rust toolchain if needed, cmake if needed,
the 77 MB English whisper model), builds the app (~5–10 min once, seconds
afterwards) and launches it. Later runs are just:

```bash
./run.sh          # straight in
```

Flags:

| Flag | Meaning |
|---|---|
| `--demo` | Built-in demo ASR — no model download needed |
| `--no-asr` | No speech recognition — simulator + mic meter only |
| `--engine E` | Speech engine: `whisper` (default) or `parakeet` |
| `--model PATH` | Use a specific ggml model (engine is guessed from the file name) |
| `--metal` | Build with the Apple GPU (Metal) backend — opt-in, see *Speech engines* |
| `--church NAME` | Church name shown on the stage window |
| `--yes` | Never prompt during setup |

Full test plan and a step-by-step demo script: see **TESTING.md**.
Design process and rationale: **docs/UIUX-PROCESS.md**. Accuracy engineering
and benchmark vs Siri/Google/Alexa: **docs/ACCURACY.md**. 115-item roadmap:
**docs/IDEAS-100.md**.

---

## What's new in v0.801.2 (voice commands · natural read-aloud · accuracy)

**Voice commands now work as spoken — including bare words:**

| Say | Effect |
|---|---|
| “Next.” / “Next verse.” | advance one verse |
| “Previous.” / “Go back.” | previous verse |
| “Next chapter.” / “Back a chapter.” | chapter navigation |
| “Last scripture.” / “Again.” | re-stage the most recent passage |
| “Read it.” / “Stop reading.” | read the on-stage verse aloud (natural TTS) |
| “Zoom in.” / “Bigger.” / “Zoom out.” / “Reset zoom.” | stage text size |
| “Full screen.” / “Normal screen.” | stage fullscreen |
| “Clear the screen.” | blank the stage |
| “Verse 18.” / “Romans 8:28.” / “Psalm 23.” | stage that passage |
| “For God so loved the world…” | quotation → review queue |

Guard rails: “next is John 3” stages John 3 (commands never eat references);
voice can never stop listening or recording (operator-only, by design).

**Natural read-aloud (not robotic):** pick a ★ Premium/Enhanced macOS voice
(Ava, Zoe, Tom, Daniel…) in the new toolbar — free, fully offline after a
one-time download; the “Manage voices…” button opens the download manager.
Male/female options, rate slider, optional auto-read of every new verse.

**Accuracy upgrades (details in docs/ACCURACY.md):** beam search (5) +
domain-vocabulary prompt + deterministic decode + non-speech suppression in
whisper; two-tier fuzzy command matcher; BBE added as a second built-in
translation with a translation-framework API (`from_translation()`).

**Operator feedback:** new “HEARD → RESULT” strip shows exactly what the app
understood; sermon services now also write a human-readable `index.md`
scripture index next to the recording.

---

## What's inside (PRD v2.0 M0 scope — all shipped)

| PRD | Feature | Status in v0.801.2 |
|---|---|---|
| §6.1 | Voice-activated Scripture trigger — structural parser | ✅ filler-tolerant, digit-split, fuzzy book match |
| §6.1 | Verse-count validation | ✅ full KJV (31,102 verses) embedded; honest out-of-range messages with "did you mean" suggestions |
| §6.2 | Scripture quotation recognition | ✅ leading-words match → confirm queue (never auto-staged) |
| §6.3 | Natural context: bare "verse 17" / "that verse" | ✅ resolves against the on-stage chapter |
| §6.4 | Translation management | ✅ KJV + BBE built-in; translation-framework API (`from_translation()`); licensed requests captured honestly (NIV/AMPC ready) |
| §6.5 | Confidence + operator review | ✅ confirm queue, adjustable auto-stage floor, Approve / Correct / Dismiss |
| §6.6 | Smart presentation | ✅ stage window, auto-fitting serif layout, fade-in, chapter-only badge |
| §7.1 | Live transcription | ✅ offline whisper (tiny.en), timestamped |
| §7.2 | Scripture index | ✅ every staged verse logged with service timestamp |
| §7.4 | "Three things" sermon points | ✅ strict enumerated pattern → operator-approved stage card |
| §8 | Operator command centre | ✅ transcript, stage preview, review queue, index, metrics on one screen |
| §11 | Recording | ✅ WAV (16 kHz mono, ~115 MB/h) + events.jsonl + transcript.txt per service |
| §15 | Offline operation | ✅ zero network on the live path |
| §19 | Companion mode | ✅ stage window works alongside any existing presentation software |
| §21 | Privacy | ✅ local-first, no telemetry, church owns the files |

Voice: full command table above. Keyboard: N / P / M / B verses+chapters,
R read aloud, F stage window, H help, Esc leave fullscreen.

Not in v0.801.2 (per PRD roadmap): sermon archive search (M2), service modes and
templates (M2), branding/styles editor (M2), accounts/cloud/sharing (M3).
Pushing back on scope is deliberate — see `docs/PRD-v2-review.md`.

---

## Architecture

```
crates/
  rf-bible   KJV + BBE embedded (build-validated) + quotation index + translation framework
  rf-core    structural Scripture parser + two-tier voice-command engine (pure logic)
  rf-asr     cpal mic capture → VAD → resampler → Whisper or Parakeet (one shared whisper.cpp build)
  rf-app     eframe UI: Command Centre + Stage + natural TTS read-aloud + sermon index
```

Design choices a senior engineer would defend:

- **Structural parsing, not word whitelists.** Any number of filler words is
  tolerated between book / chapter / verse because no word list covers natural
  speech (PRD §6.1). Fuzzy book match uses edit distance with a
  preaching-frequency tie-break; a fuzzy anchor never outranks a nearby exact
  one ("to john" → "john").
- **Silent failure is treated as a defect** (PRD §2). A verse that doesn't
  exist is never staged silently: the operator gets the honest message plus
  the closest real candidates ("1 Corinthians 5 has only 13 verses (no verse
  17) — did you mean 2 Corinthians 5:17?").
- **The audio callback stays tiny.** Capture, VAD, resampling and whisper run
  on worker threads; the UI thread only repaints.
- **ASR = whisper.cpp, called through a thin C++ shim.** "100% Rust" is honored
  at the source level — every line of app logic is Rust; whisper.cpp is a C++
  inference core reached over FFI, the same way cpal reaches CoreAudio and
  winit reaches Cocoa. `crates/rf-asr/build.rs` builds ONE pinned whisper.cpp
  tree (vendored under `third_party/`) that provides both the Whisper and the
  NVIDIA Parakeet engines, so there is a single copy of ggml in the binary.
  The Rust side never declares whisper's large parameter structs: two small C++
  files in `crates/rf-asr/native/` are compiled against the real headers and
  expose only pointers, ints and C strings. A pure-Rust ASR core (candle)
  exists but is slower, less battle-tested, and adds model-format risk the day
  before a service. This trade-off is documented deliberately, not hidden.
- **Locked regression tests** (`cargo test --workspace`) cover every PRD acceptance criterion plus
  the v0.801.2 voice-command grammar; every future user-reported mishear
  becomes a new test (PRD §23).

## Speech engines (Whisper · NVIDIA Parakeet) and the benchmark

Two offline engines share one native build. **Whisper** (`tiny.en`) stays the
default — it is the path this app has been exercised on end to end.
**Parakeet TDT 0.6B v3** is an opt-in alternative.

```bash
./run.sh model-parakeet          # one-time ~1.2 GB download, offline afterwards
./run.sh --engine parakeet       # run the app on Parakeet
./run.sh --metal                 # optional Apple-GPU build (untested — see below)
```

What each engine is, honestly:

| | Whisper (default) | Parakeet TDT 0.6B v3 |
|---|---|---|
| Model size | 77 MB (`tiny.en`) | ~1.2 GB |
| Vocabulary priming | ✅ scripture prompt (book names, commands) | ❌ no equivalent input |
| Latency model | whole utterance after the speaker pauses | same — the C API decodes whole chunks, **not** word-by-word |
| Reported accuracy | see docs/ACCURACY.md | ~2.6% WER LibriSpeech test-clean (NVIDIA / Open ASR Leaderboard) |

Parakeet is a transducer model, but the API bundled in whisper.cpp
(`parakeet_full` / `parakeet_chunk`) is chunk-based, so in RhemaFlow it plugs
into the same "speaker pauses → transcribe the utterance" pipeline as Whisper.
Expect potentially better accuracy and speed — not earlier detection. Whether it
actually beats primed Whisper on *scripture* speech (book names!) is a
question for the benchmark, not an assumption:

```bash
./run.sh benchmark sermon-clip.wav --ref sermon-clip.txt
```

It runs every engine that has a model installed on the same audio and prints
load time, decode time, real-time factor, word error rate against your reference
transcript, and — most usefully — which scripture reference the parser extracts
from each transcript. `--extra "Label" transcript.txt` adds any other
transcript to the same comparison (for example text produced by macOS
dictation) so you can compare against tools this app cannot call itself.

**Not verified in development:** the Parakeet model itself (only whisper.cpp's
tiny CI stub model was run — the real ~1.2 GB weights were not reachable), and
the Metal / Core ML backends (no Apple hardware). The CPU path is what has been
run. Numbers in the table above for Parakeet are the published figures, not
measurements from this repository.

## Service artefacts

Each live service writes to `~/RhemaFlow-Services/<timestamp>/`:

- `audio.wav` — 16 kHz mono PCM recording (only while recording is on)
- `transcript.txt` — timestamped live transcript
- `events.jsonl` — machine-readable Scripture index + timeline events
- `index.md` — human-readable sermon Scripture index (written at service end)

Settings persist in `~/.rhemaflow/settings.json`.

## Troubleshooting

- **No mic prompt appeared.** System Settings → Privacy & Security →
  Microphone → enable your terminal app, then press "Retry" in the app.
- **"no model" chip in the header.** Run `./run.sh` again and accept the model
  download, or use `--demo`.
- **Whisper mishears a reference.** Use the Correct button — and please send
  us the phrase so it becomes a locked regression test.
- **cmake missing.** `brew install cmake` (or `pip3 install --user cmake`).

— Built as a complete, testable vertical slice of PRD v2.0's M0: the
trust-building core a church could actually run a service on.
