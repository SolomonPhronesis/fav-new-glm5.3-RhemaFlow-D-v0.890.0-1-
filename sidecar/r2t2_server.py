#!/usr/bin/env python3
"""RhemaFlow streaming-ASR sidecar — speaks the Confucius4-R2T2 WebSocket protocol.

    client -> server : binary frames of 16 kHz mono int16 LE PCM (160 ms = 2560 samples)
                       text "YOUDAO_ONETIME_ASR_STREAM_EOS"  = end of audio
                       text "RF_FLUSH"  = (RhemaFlow extension) speaker stopped, finish now
                       text "RF_CONTEXT <words>"  = (RhemaFlow extension, v0.854) update the
                            hotwords (R2T2 `context` slot: book names + learned words).
                            Applies from the NEXT utterance on — R2T2 reads the context when
                            a streaming state is created, and resetting mid-utterance would
                            drop buffered audio.
    server -> client : {"status":"success","requestId":..,"msg":{"text":"<new words>",
                        "reset":false,"asr_cost_ms":..}}   text is APPEND-ONLY
                       {"msg":{"text":"<last words>","reset":true}} = utterance finished

Backends
    mock   deterministic fake engine; used by the tests (no model needed)
    llama  the official R2T2 streaming algorithm (r2t2.R2T2ASRModel) decoded by the
           llama.cpp backend that ships in the R2T2 repo (r2t2_llama, LlamaNative).
           This is the path intended for Apple Silicon (Metal). It needs the steps in
           docs/R2T2-ON-M1.md and has NOT been run on a Mac by the author of this file.

Streaming-loop rules copied from the reference server (ws_server.py, Apache-2.0):
    * first chunk = chunk + lookahead; later chunks = chunk
    * max_new_tokens starts at (chunk+lookahead)/1280 samples-per-token and adapts
    * only the *fixed* text (all but the last `unfixed_token_num` tokens) is emitted
    * a hard reset bounds the cost of re-feeding all audio each chunk
Additions here: adaptive chunk size from measured cost, RF_FLUSH, repetition guard.
"""
import argparse
import asyncio
import json
import os
import re
import sys
import time
import uuid

import numpy as np

EOS = "YOUDAO_ONETIME_ASR_STREAM_EOS"
FLUSH = "RF_FLUSH"
CONTEXT = "RF_CONTEXT"
SR = 16000
SAMPLES_PER_TOKEN = 1280  # one audio token = 80 ms
# R2T2 re-reads the whole utterance every step, so cost grows with its length. A reset
# restarts that cost. Never reset in the middle of a word: after SOFT seconds, reset at the
# first quiet chunk (a breath between words); only after HARD seconds reset regardless.
SOFT_RESET_SEC = float(os.environ.get("RF_SOFT_RESET_SEC", "8"))
HARD_RESET_SEC = float(os.environ.get("RF_HARD_RESET_SEC", "16"))


# ----------------------------------------------------------------- guards ----
def clean_tail(text: str, repeats: int = 5) -> str:
    """Cut a decoder loop ("okay okay okay okay okay ...") off the end."""
    words = text.split()
    norm = [re.sub(r"\W", "", w).lower() for w in words]
    for k in range(1, 9):
        if len(words) < k * repeats:
            continue
        tail = norm[-k:]
        if not any(tail):
            continue
        reps = 1
        while len(words) >= (reps + 1) * k and norm[-(reps + 1) * k : -reps * k] == tail:
            reps += 1
        if reps >= repeats:
            return " ".join(words[: len(words) - (reps - 1) * k])
    return text


def should_reset(utterance_samples: int, chunk_rms: float, recent_rms: list) -> bool:
    """True when it is a good moment to restart the decoder (see SOFT/HARD above)."""
    secs = utterance_samples / SR
    if secs >= HARD_RESET_SEC:
        return True
    if secs < SOFT_RESET_SEC or len(recent_rms) < 10:
        return False
    typical = float(np.median(recent_rms))
    return chunk_rms < 0.35 * typical  # clearly quieter than the speech around it


class ChunkPlanner:
    """Pick the chunk size from measured cost: never let decoding fall behind audio.

    R2T2 supports chunks from 80 ms to 2 s. A small chunk is lower latency but costs
    one full re-feed per chunk; if the machine can't keep up, grow the chunk.
    """

    SIZES_MS = (160, 240, 320, 480, 640, 960)

    def __init__(self, start_ms: int = 160):
        self.idx = min(range(len(self.SIZES_MS)), key=lambda i: abs(self.SIZES_MS[i] - start_ms))
        self.costs = []

    @property
    def chunk_ms(self) -> int:
        return self.SIZES_MS[self.idx]

    def note(self, cost_ms: float) -> None:
        self.costs.append(cost_ms)
        self.costs = self.costs[-6:]
        if len(self.costs) < 4:
            return
        avg = sum(self.costs) / len(self.costs)
        if avg > 0.8 * self.chunk_ms and self.idx < len(self.SIZES_MS) - 1:
            self.idx += 1
            self.costs.clear()
        elif avg < 0.3 * self.chunk_ms and self.idx > 0:
            self.idx -= 1
            self.costs.clear()


# --------------------------------------------------------------- backends ----
class MockBackend:
    """Says one scripted word per 400 ms of audio. Append-only, no revisions."""

    SCRIPT = "turn with me to john chapter three verse sixteen".split()

    def __init__(self):
        self.context = ""
        self.reset()

    def reset(self):
        self.samples = 0
        self.emitted = 0

    def set_context(self, context: str):
        """Store the hotwords (mock ignores them for output; kept for tests)."""
        self.context = context

    def feed(self, pcm: np.ndarray, chunk_ms: int):
        self.samples += len(pcm)
        due = min(len(self.SCRIPT), self.samples // int(0.4 * SR))
        new = self.SCRIPT[self.emitted : due]
        self.emitted = due
        return "".join(w + " " for w in new)

    def finish(self):
        rest = self.SCRIPT[self.emitted :] if self.samples else []
        self.reset()
        return "".join(w + " " for w in rest)


def install_vllm_shim() -> None:
    """R2T2's streaming_transcribe() does `from vllm import SamplingParams` on every call.
    vLLM cannot be installed on macOS, and the llama.cpp adapter only reads `.max_tokens`
    from the object, so a tiny stand-in module is enough. A real vLLM wins if present."""
    import types

    try:
        import vllm  # noqa: F401
        return
    except ImportError:
        pass

    class SamplingParams:
        def __init__(self, temperature=0.0, max_tokens=None, skip_special_tokens=True, **_):
            self.temperature, self.max_tokens, self.skip_special_tokens = temperature, max_tokens, skip_special_tokens

    mod = types.ModuleType("vllm")
    mod.SamplingParams = SamplingParams
    sys.modules["vllm"] = mod


class LlamaBackend:
    """Official R2T2 streaming algorithm + llama.cpp decoder (Metal on Apple Silicon)."""

    def __init__(self, processor_path: str, gguf_dir: str, language, context: str, unfixed: int, threads: int):
        sys.path.insert(0, os.environ.get("R2T2_REPO", "."))
        install_vllm_shim()
        from r2t2_llama import R2T2LlamaASRModel  # noqa: E402  (from the R2T2 repo)

        self.asr = R2T2LlamaASRModel.LlamaNative(
            processor_path=processor_path,
            gguf_dir=gguf_dir,
            n_ctx=8192,  # reference default is 32768; far more than one utterance needs
            n_batch=2048,
            n_threads=threads,  # M1: 4 performance cores
            use_gpu=True,
            n_gpu_layers=-1,
            max_new_tokens=4,
        )
        self.language, self.context, self.unfixed = language, context, unfixed
        self.max_new = 2
        self.last_fixed = ""
        self.state = None
        self.first = True
        self.reset()

    def reset(self):
        self.state = self.asr.init_streaming_state(
            context=self.context, language=self.language, unfixed_chunk_num=0,
            unfixed_token_num=self.unfixed, chunk_size_sec=0.16,
        )
        self.last_fixed = ""
        self.first = True
        self.max_new = 2

    def feed(self, pcm: np.ndarray, chunk_ms: int):
        step = int(chunk_ms / 1000 * SR)
        self.state.chunk_size_sec = chunk_ms / 1000.0
        self.state.chunk_size_samples = step
        if self.first:  # the reference gives the first chunk one extra chunk of lookahead
            self.state.chunk_size_sec = 2 * chunk_ms / 1000.0
            self.state.chunk_size_samples = 2 * step
            self.first = False
        _, fixed = self.asr.streaming_transcribe(pcm, self.state, int(self.max_new))
        fixed = fixed.split("|")[0]
        new = fixed[len(self.last_fixed):] if fixed.startswith(self.last_fixed) else ""
        if new:
            self.last_fixed = fixed
            self.max_new = max(1, step // SAMPLES_PER_TOKEN)
        else:
            self.max_new = min(32, self.max_new + 0.5)  # nothing new: allow a longer run next time
        return new

    def set_context(self, context: str):
        """Update hotwords. R2T2 reads the context when a streaming state is
        created (at every reset), so the new words apply from the next
        utterance on. Never reset here: mid-utterance we would drop audio."""
        if context != self.context:
            self.context = context

    def finish(self):
        full = self.asr.finish_streaming_transcribe(self.state, 4) or ""
        full = full.split("|")[0]
        rest = full[len(self.last_fixed):] if full.startswith(self.last_fixed) else ""
        self.reset()
        return rest


# ----------------------------------------------------------------- server ----
async def handle(ws, make_backend, first_chunk_ms: int):
    request_id = str(uuid.uuid4())
    backend = make_backend()
    planner = ChunkPlanner(first_chunk_ms)
    buf = np.zeros(0, dtype=np.float32)
    utterance_samples = 0
    emitted = ""
    recent_rms = []  # per-chunk RMS of the current utterance

    async def send(text: str, reset: bool, cost_ms: float = 0.0):
        text = clean_tail(emitted + text)[len(emitted):] if text else text
        msg = {"status": "success", "requestId": request_id,
               "msg": {"text": text, "reset": reset, "asr_cost_ms": round(cost_ms, 1)}}
        await ws.send(json.dumps(msg))

    async def finish_utterance(cost_ms=0.0):
        nonlocal buf, utterance_samples, emitted
        loop = asyncio.get_running_loop()
        rest = await loop.run_in_executor(None, backend.finish)
        buf = np.zeros(0, dtype=np.float32)
        utterance_samples = 0
        emitted = ""
        recent_rms.clear()
        await send(rest, True, cost_ms)

    async for data in ws:
        if isinstance(data, str):
            if data == EOS:
                await finish_utterance()
                await ws.close()
                return
            if data == FLUSH:
                await finish_utterance()
                continue
            if data.startswith(CONTEXT):  # "RF_CONTEXT <words>" — hotword update
                backend.set_context(data[len(CONTEXT):].strip())
            continue
        pcm = np.frombuffer(data, dtype="<i2").astype(np.float32) / 32768.0
        buf = np.concatenate([buf, pcm])
        step = int(planner.chunk_ms / 1000 * SR)
        while len(buf) >= step:
            seg, buf = buf[:step], buf[step:]
            utterance_samples += len(seg)
            seg_rms = float(np.sqrt(np.mean(seg * seg))) if len(seg) else 0.0
            recent_rms.append(seg_rms)
            del recent_rms[:-80]
            t0 = time.perf_counter()
            loop = asyncio.get_running_loop()
            new = await loop.run_in_executor(None, backend.feed, seg, planner.chunk_ms)
            cost = (time.perf_counter() - t0) * 1000
            planner.note(cost)
            if new:
                emitted += new
            await send(new, False, cost)
            if should_reset(utterance_samples, seg_rms, recent_rms):  # bound the re-feed cost
                await finish_utterance(cost)


async def watch_parent(pid: int):
    """Exit when the app that started us is gone, so a crashed app never leaves a
    2-billion-parameter model running in the background."""
    while True:
        await asyncio.sleep(2)
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            print("[r2t2-sidecar] parent process ended - exiting", flush=True)
            os._exit(0)
        except PermissionError:
            pass


async def serve(args):
    import websockets

    if args.parent_pid:
        asyncio.create_task(watch_parent(args.parent_pid))

    if args.backend == "mock":
        make = MockBackend
    else:
        lang = None if args.language in ("", "None") else args.language
        shared = LlamaBackend(args.processor_path, args.gguf_dir, lang, args.context, args.unfixed_token_num, args.threads)
        make = lambda: shared  # one model, one active stream (single-user desktop app)  # noqa: E731

    async def route(ws):
        path = getattr(getattr(ws, "request", None), "path", "/asr_stream_api_v1")
        if not path.startswith("/asr_stream_api_v1"):
            await ws.close(code=1008, reason="unknown path")
            return
        await handle(ws, make, args.chunk_ms)

    async with websockets.serve(route, args.host, args.port, max_size=2**22):
        print(f"[r2t2-sidecar] backend={args.backend} ws://{args.host}:{args.port}/asr_stream_api_v1", flush=True)
        await asyncio.Future()


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--backend", choices=["mock", "llama"], default="llama")
    p.add_argument("--host", default="127.0.0.1")  # local only: this has no authentication
    p.add_argument("--port", type=int, default=8272)
    p.add_argument("--processor-path", help="HF dir of Confucius4-R2T2 (tokenizer/processor files)")
    p.add_argument("--gguf-dir", help="dir with the language-model GGUF and the mmproj*.gguf")
    p.add_argument("--language", default="English")
    p.add_argument("--context", default="", help="hotwords / topic prompt, e.g. book names")
    p.add_argument("--unfixed-token-num", type=int, default=1)
    p.add_argument("--chunk-ms", type=int, default=160)
    p.add_argument("--threads", type=int, default=4)
    p.add_argument("--parent-pid", type=int, default=0, help="exit when this process ends (set by the app)")
    args = p.parse_args()
    if args.backend == "llama" and not (args.processor_path and args.gguf_dir):
        p.error("--backend llama needs --processor-path and --gguf-dir (see docs/R2T2-ON-M1.md)")
    asyncio.run(serve(args))


if __name__ == "__main__":
    main()
