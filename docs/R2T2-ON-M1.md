# Confucius4-R2T2 → RhemaFlow on Apple Silicon (v0.807)

## 1. Did I watch the demo? What does it show?

Yes. The video is in their repo (`resources/gpt-vs-r2t2.mp4`, 101 s, 960×540). I cannot hear audio, so I looked at a few dozen frames (an overview sheet of the whole clip plus 3-frames-per-second slices of two stretches). It shows two laptops. The left one plays a studio presenter video (the audio source). The right one, labelled R2T2.ai, prints the transcript while the presenter talks.

What the frames show:
- Text appears **word by word, continuously**, a few words per second, about as fast as he speaks.
- **Nothing already on screen is rewritten.** New words fade in (grey, then solid) at the end and the paragraph scrolls up. That is the "append-only" claim, visible.
- It switches between English and Spanish mid-sentence without a restart.
- The voice is a clean studio recording played through a laptop speaker. **It does not show a measured latency.** The only number is the README's: 200–600 ms average, by their own "chunk-wise fuzzy latency" metric.

Honest limit: this is a staged demo with clean audio. It proves the *behaviour* (live, append-only, no flicker). It says nothing about a reverberant sanctuary or a pastor who says "Habakkuk".

## 2. How it actually works (from their code, not guesses)

| Idea | What the code does |
|---|---|
| Base | Qwen3-ASR-1.7B (audio encoder + LLM decoder), fine-tuned → ~2 B params, BF16 ≈ 4.1 GB |
| Chunks | audio fed every **160 ms** (configurable 80 ms–2 s); first chunk gets +160 ms lookahead |
| Each step | **re-encodes all audio since the utterance began** and asks the LLM to continue from the text already committed. Prompt = chat template + `context` + committed prefix. Greedy, 1–4 new tokens per step (1 token ≈ 80 ms of audio) |
| Append-only | the last `unfixed_token_num` (=1) tokens are held back as "unsettled"; everything before them is emitted and is fed back as the prefix next time, so it can never change. Training ("Longest Stable Prefix", stable-prefix data, forced alignment) is what makes the model willing to emit only what it is sure of |
| Hotwords | `context` string is part of the prompt, natively. **This is how scripture book names and the sermon notes can be injected.** |
| Cost control | VAD ends the utterance and resets state; a hard reset caps the re-feed cost; `max_new_tokens` adapts up when nothing new came out |
| Guards | tail-repetition / hallucination loop detector forces a reset |
| Protocol | WebSocket, int16 PCM frames in, JSON `{text, reset}` out, `text` incremental |

Two consequences that matter for us:
1. **It is not a small model.** Every 160 ms step re-runs a ~1.7 B decoder prefill over the whole utterance so far. Their reference runs on a CUDA GPU with vLLM. The reference llama.cpp path appears to clear the KV cache on each call (`llama_memory_clear` in `native_ext.cpp`; I did not trace every branch), so cost likely grows with utterance length.
2. **"93%" is not in their README.** Their 160 ms English WERs average **6.1 %** across the nine sets (my arithmetic: 11.37, 9.60, 2.13, 4.88, 3.00, 3.07, 9.36, 3.34, 8.40) ⇒ ~93.9 % accuracy. That figure is a mean over read speech, TED, earnings calls and meetings. It is not a pulpit.

## 3. Licence (read before shipping anything)

- Code: Apache-2.0. Weights: **NetEase Youdao Model Use License**. Free below 100 M MAU / RMB 1 B revenue; Chinese-language version prevails; PRC law, CIETAC arbitration.
- GGUF/quantised/Core ML conversions are **Derivative Works**: keep the licence text, and a distributed derivative must state the "not endorsed" disclaimer (§4.1a).
- §3.4(c): you may not use the model or its outputs to improve any AI model except R2T2 itself or non-commercial ones. **Do not train a model on R2T2 transcripts.** The personal lexicon (a word-pair table) is not a model, but keep an eye on this if it ever becomes one.
- Therefore RhemaFlow **does not bundle the weights**. The user downloads them after accepting the licence.

## 4. What I built (v0.807.0)

| Piece | Where | Tested |
|---|---|---|
| App ⇄ service client, R2T2 protocol, reconnect, drains last words | `rf-asr/stream_client.rs` | 5 Rust tests incl. in-process server + **cross-language run against the Python sidecar** |
| Mic audio tap + "speaker stopped" flush | `rf-asr/capture.rs` (`StreamTap`) | compiles; exercised via bridge tests, not with a live mic |
| Append-only commit tracker: act on a finished reference ~350 ms after the last word, never early on an unfinished one, tail-loop guard | `rf-core/commit.rs` | 8 tests |
| Bridge into the existing parser/UI | `rf-app/stream_bridge.rs` + `--stream-url` | 4 tests |
| Mac sidecar speaking the protocol, adaptive chunk size, loop guard, vLLM stand-in | `sidecar/r2t2_server.py` | 6 Python tests with a mock backend |
| Mac build/setup script | `sidecar/setup_mac.sh` | **syntax-checked only** |

**Not tested, because it cannot be from here:** the real model, Metal, the native llama.cpp extension build on macOS, a real microphone, and real latency/accuracy numbers.

Needed on macOS that the official docs do not mention (found by reading the code):
- `pip install -e repo` pulls `qwen-asr[vllm]` → fails on Mac. Use `--no-deps`.
- `streaming_transcribe` executes `from vllm import SamplingParams` on every call → the sidecar installs a stand-in module.
- Shipped native libs are Linux/CUDA, with `$ORIGIN` rpaths → rebuild with Metal and `@loader_path`.
- Defaults `n_threads=32`, `n_ctx=32768` are server values → the sidecar uses 4 threads (M1 has 4 performance cores) and 8192.

## 5. Will an M1 keep up? (estimate, not measurement)

Each step must finish inside its chunk or text falls behind. Rough prefill cost ≈ 2 × 1.7 B × tokens. A 3-second utterance is ~38 audio tokens + ~40 prompt/prefix tokens ≈ 0.27 TFLOP per step. An M1 GPU sustains very roughly 0.5–1 TFLOP/s in this kind of workload, so **~0.3–0.6 s per step**, i.e. slower than a 160 ms chunk. Growth with utterance length makes it worse until the next VAD reset.

So expect the sidecar's `ChunkPlanner` to settle at **320–640 ms chunks** on an M1, giving roughly 0.5–1 s behind the speaker, not 200 ms. An M2/M3/M4 Pro/Max should do better. If the M1 is the 8 GB model, use the 8-bit GGUF (~2.3 GB) and close other apps. These are my estimates; `rhemaflow-d calibrate` and the sidecar's reported `asr_cost_ms` will give the real numbers in one session.

Community ports exist that I have **not** verified: audio.cpp (native C++, Metal, GGUF Q8/Q4_K_M, a streaming mode), and a Core ML / Neural Engine conversion (reports LibriSpeech test-clean WER 2.4 %, needs macOS 15+). Both are third-party derivative works. If the llama.cpp route is too slow on your Mac, they are the next things to try; the sidecar protocol means RhemaFlow would not change.

## 6. Run it

```bash
# one-time (Apple Silicon, ~20 min, ~6 GB disk)
brew install cmake python@3.12
./sidecar/setup_mac.sh          # follow its step 5 to download the weights after accepting the licence

# every Sunday
~/RhemaFlow-R2T2/run_sidecar.sh                       # terminal 1
./run.sh --stream-url ws://127.0.0.1:8272/asr_stream_api_v1   # terminal 2
```
No Mac handy? `python3 sidecar/r2t2_server.py --backend mock` plus `--stream-url` exercises the whole path with a fake engine.

## 7. What is still needed to hit "93%, under 1 s" for a pulpit

1. A real run on your Mac to learn the true step cost and chunk size (step 6).
2. **Pass the book names and the sermon's references as `--context`** so the decoder is biased toward them (R2T2 supports this natively; I have not measured the gain).
3. Run `rhemaflow-d calibrate` against this engine too. The 30-sentence script gives *your* accuracy; the 93.9 % is theirs on other speakers.
4. Mic and room: a close/lav mic matters more than the model. A sanctuary with reverb will cost more WER than any engine choice.
5. Decide the fallback: if the sidecar is down, `--model` Whisper keeps working (the app already supports it).
6. Feed `Agreement`/`CommitTracker` real data to tune the 350 ms release rule.
