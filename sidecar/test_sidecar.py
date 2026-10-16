"""Protocol + guard tests for r2t2_server.py (mock backend; no model needed).
Run:  python3 -m unittest sidecar/test_sidecar.py -v
"""
import asyncio, json, subprocess, sys, time, unittest, os, socket
import numpy as np
sys.path.insert(0, os.path.dirname(__file__))
import r2t2_server as S


def free_port():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p


class Guards(unittest.TestCase):
    def test_repetition_loop_is_cut(self):
        self.assertEqual(S.clean_tail("amen okay okay okay okay okay okay okay"), "amen okay")
        self.assertEqual(S.clean_tail("praise the lord praise the lord"), "praise the lord praise the lord")
        self.assertEqual(S.clean_tail("one two three"), "one two three")

    def test_planner_grows_when_slow_and_shrinks_when_fast(self):
        p = S.ChunkPlanner(160)
        for _ in range(8):
            p.note(400)  # 2.5x slower than real time
        self.assertGreater(p.chunk_ms, 160)
        big = p.chunk_ms
        for _ in range(20):
            p.note(20)
        self.assertLess(p.chunk_ms, big)

    def test_planner_never_leaves_the_supported_range(self):
        p = S.ChunkPlanner(160)
        for _ in range(100):
            p.note(10_000)
        self.assertEqual(p.chunk_ms, 960)
        for _ in range(100):
            p.note(1)
        self.assertEqual(p.chunk_ms, 160)


class Reset(unittest.TestCase):
    loud = [0.1] * 60

    def test_never_resets_early_or_mid_speech(self):
        self.assertFalse(S.should_reset(5 * S.SR, 0.01, self.loud))        # too early, even if quiet
        self.assertFalse(S.should_reset(10 * S.SR, 0.1, self.loud))        # past soft limit but still loud speech

    def test_resets_at_a_breath_after_the_soft_limit(self):
        self.assertTrue(S.should_reset(10 * S.SR, 0.02, self.loud))

    def test_resets_regardless_at_the_hard_limit(self):
        self.assertTrue(S.should_reset(17 * S.SR, 0.1, self.loud))

    def test_needs_some_history_before_judging_quiet(self):
        self.assertFalse(S.should_reset(10 * S.SR, 0.0, [0.1] * 3))


class ParentWatch(unittest.TestCase):
    def test_sidecar_exits_when_its_parent_process_ends(self):
        parent = subprocess.Popen(["sleep", "3"])
        port = free_port()
        child = subprocess.Popen([sys.executable, os.path.join(os.path.dirname(__file__), "r2t2_server.py"),
                                  "--backend", "mock", "--port", str(port), "--parent-pid", str(parent.pid)],
                                 stdout=subprocess.PIPE)
        child.stdout.readline()
        self.assertIsNone(child.poll(), "running while the parent lives")
        parent.wait()
        t0 = time.time()
        while child.poll() is None and time.time() - t0 < 8:
            time.sleep(0.2)
        self.assertIsNotNone(child.poll(), "must exit after the parent is gone")
        self.assertEqual(child.returncode, 0)


class Shim(unittest.TestCase):
    def test_vllm_stand_in_provides_sampling_params(self):
        saved = sys.modules.pop("vllm", None)
        try:
            S.install_vllm_shim()
            from vllm import SamplingParams  # the exact line R2T2 executes per chunk
            sp = SamplingParams(temperature=0.0, max_tokens=3, skip_special_tokens=True)
            self.assertEqual(sp.max_tokens, 3)
        finally:
            sys.modules.pop("vllm", None)
            if saved is not None:
                sys.modules["vllm"] = saved


class Protocol(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.port = free_port()
        cls.proc = subprocess.Popen([sys.executable, os.path.join(os.path.dirname(__file__), "r2t2_server.py"),
                                     "--backend", "mock", "--port", str(cls.port)], stdout=subprocess.PIPE)
        cls.proc.stdout.readline()  # banner

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate(); cls.proc.wait(5)

    def run_session(self, seconds, tail):
        import websockets

        async def go():
            texts, resets = [], 0
            async with websockets.connect(f"ws://127.0.0.1:{self.port}/asr_stream_api_v1") as ws:
                frame = (np.zeros(2560, dtype="<i2")).tobytes()
                for _ in range(int(seconds / 0.16)):
                    await ws.send(frame)
                await ws.send(tail)
                try:
                    while True:
                        m = json.loads(await asyncio.wait_for(ws.recv(), 3))
                        self.assertEqual(m["status"], "success")
                        texts.append(m["msg"]["text"]); resets += bool(m["msg"]["reset"])
                        if resets and tail == S.EOS:
                            pass
                except (asyncio.TimeoutError, Exception):
                    pass
            return "".join(texts), resets
        return asyncio.run(go())

    def test_text_is_incremental_and_eos_flushes_the_rest(self):
        text, resets = self.run_session(2.0, S.EOS)  # 2 s of audio => 5 words so far
        self.assertEqual(text.split(), "turn with me to john chapter three verse sixteen".split())
        self.assertEqual(resets, 1)

    def test_flush_ends_the_utterance_and_the_connection_stays_usable(self):
        import websockets

        async def go():
            async with websockets.connect(f"ws://127.0.0.1:{self.port}/asr_stream_api_v1") as ws:
                frame = np.zeros(2560, dtype="<i2").tobytes()
                out = []
                for round_ in range(2):
                    for _ in range(13):
                        await ws.send(frame)  # ~2 s
                    await ws.send(S.FLUSH)
                    got, reset = "", False
                    while not reset:
                        m = json.loads(await asyncio.wait_for(ws.recv(), 3))["msg"]
                        got += m["text"]; reset = m["reset"]
                    out.append(got.split())
                return out
        a, b = asyncio.run(go())
        self.assertEqual(a, b, "second utterance starts from scratch after FLUSH")
        self.assertEqual(a[0], "turn")


class Context(unittest.TestCase):
    """v0.854: the RF_CONTEXT hotword extension."""

    def test_mock_backend_stores_hotwords(self):
        m = S.MockBackend()
        self.assertEqual(m.context, "")
        m.set_context("Habakkuk chapter two")
        self.assertEqual(m.context, "Habakkuk chapter two")

    def test_rf_context_frame_reaches_the_backend_and_session_survives(self):
        import websockets

        async def go():
            backend = S.MockBackend()

            async def handler(ws):
                await S.handle(ws, lambda: backend, 160)

            port = free_port()
            async with websockets.serve(handler, "127.0.0.1", port):
                async with websockets.connect(f"ws://127.0.0.1:{port}/asr_stream_api_v1") as ws:
                    await ws.send(S.CONTEXT + " Habakkuk chapter two")
                    frame = np.zeros(2560, dtype="<i2").tobytes()
                    for _ in range(5):
                        await ws.send(frame)
                    await ws.send(S.FLUSH)
                    reset = False
                    while not reset:
                        m = json.loads(await asyncio.wait_for(ws.recv(), 3))
                        self.assertEqual(m["status"], "success")
                        reset = m["msg"]["reset"]
            return backend

        backend = asyncio.run(go())
        self.assertEqual(backend.context, "Habakkuk chapter two",
                         "the RF_CONTEXT control frame must update the engine's hotwords")


if __name__ == "__main__":
    unittest.main()
