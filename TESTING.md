# Testing RhemaFlow-D v0.801.2 on your M1 Mac

Everything below was written for the machine in front of you: an Apple
Silicon Mac, a microphone, and Terminal. Budget 15 minutes for the first
setup (mostly one-time downloads) and 5 minutes for the demo.

---

## Part 1 — One-time setup (first run only)

1. Open **Terminal** (Applications → Utilities → Terminal).
2. Unzip the download if you haven't:

   ```bash
   cd ~/Downloads
   unzip RhemaFlow-D-v0.801.2.zip
   cd RhemaFlow-D-v0.801.2
   ```

3. Run:

   ```bash
   ./run.sh
   ```

   The script will, in order:
   - verify Xcode Command Line Tools (macOS offers to install them if missing —
     after that finishes, re-run `./run.sh`)
   - install Rust if missing (one-time, ~1 min)
   - install cmake if missing (via Homebrew, if you have it)
   - download the 77 MB English speech model (one-time, checksum-verified)
   - build the app (one-time, ~5–10 min)
   - launch RhemaFlow-D

Everything after the first run is offline and instant: `./run.sh`.

> **No internet at the venue?** Do the first run at home today. After setup
> the live path needs no network at all.

---

## Part 2 — Microphone test (2 minutes)

1. When the app opens, look at the **header**: you want the green `model ok`
   chip and the small 7-bar meter next to the word "mic".
2. macOS will ask once: *"<Terminal> would like to access the microphone"* →
   click **OK**.
   - Missed it or clicked Don't Allow? → System Settings → **Privacy &
     Security → Microphone** → enable your terminal app → back in the app,
     press **Retry** on the red banner.
3. Speak at normal volume. The meter bars should move and the
   **"● listening"** dot should appear next to TRANSCRIPT while you talk.
4. If the meter moves but recognition feels off, pick a specific input in the
   bottom-right device dropdown (e.g. your USB mic instead of the Mac mics) —
   it takes effect within two seconds, no restart.

**No mic at all?** The app still runs: the SIMULATOR box (bottom of the
centre panel) lets you type utterances that go through the exact same
pipeline. The demo cannot be blocked by a mic problem.

---

## Part 3 — The five-minute demo (the script for tomorrow)

1. **Start.** Click **▶ Start service**. The header shows the gold `LIVE`
   badge and the red pulsing `REC` badge; the service timer starts.
2. **The core moment.** Say, naturally:

   > "Let's turn to John chapter 3 verse 16."

   Within a couple of seconds the verse appears in the big preview card and
   in the left transcript (flagged bright). The SCRIPTURE INDEX underneath
   logs `John 3:16 · voice` with the timestamp.
3. **Navigation by voice.** Say:

   > "Next verse."

4. **Quotation recognition.** Say:

   > "For God so loved the world that he gave his only begotten Son…"

   A **POSSIBLE QUOTATION** card appears on the right — never auto-staged.
   Click **Approve**. (This is PRD §6.5: uncertainty is visible, control is
   yours.)
5. **Context jump.** Say:

   > "Verse 18."

   It jumps inside the on-stage chapter — no book needed (PRD §6.3).
6. **The honest failure.** Say:

   > "First Corinthians chapter five verse seventeen."

   The app refuses to guess: a red **COULD NOT RESOLVE** card says
   *"1 Corinthians 5 has only 13 verses (no verse 17) — did you mean
   2 Corinthians 5:17?"* — and offers the corrected verse as a button.
   Click it. This is the anti-silent-failure guarantee from the PRD, live.
7. **The congregation screen.** Press **F** (or the Open stage button). The
   black stage window opens — that's what a projector would show. Press
   **F11**-style fullscreen from its green button, or just drag it to the
   second display. Say "next verse" and watch the fade-in.
8. **End.** Click **■ End service**. Find the artefacts:

   ```bash
   ls ~/RhemaFlow-Services/<timestamp>/
   # audio.wav  transcript.txt  events.jsonl
   ```

   `audio.wav` is the recording; `events.jsonl` is the machine-readable
   Scripture index with timestamps.

---

## Part 3.5 — Voice commands & read-aloud (new in v0.801.2, 3 minutes)

With a passage on stage (from Part 3, step 2):

1. Say **"Next."** — verse advances. Say **"Previous."** — it goes back.
2. Say **"Read it."** — the verse is read aloud with a natural system voice.
   Say **"Stop reading."** to stop.
3. Say **"Zoom in."** twice — stage text grows. **"Reset zoom."** restores.
4. Say **"Last scripture."** — the most recent passage re-appears.
5. Say **"Full screen."** — the stage window goes fullscreen.

If a bare word like "next" doesn't fire: check the HEARD → RESULT strip under
the stage preview — it shows exactly what the app heard and what it did with
it. If it heard something odd, say the phrase again; the fuzzy matcher
tolerates common mishears ("text" → next).

**Natural voices (one-time, 2 minutes):** press the new "Manage voices…"
button (or System Settings → Accessibility → Spoken Content → System Voice →
Manage Voices…) and download **Ava (Premium)** (female) and **Tom (Premium)**
(male) — they are free and fully offline after download. Then pick the ★
Premium voice in the toolbar. That is the "natural, not robot" answer.

## Part 4 — What the test suite already proves

Before this zip existed, the codebase passed **47 automated regression
tests**, including every acceptance criterion in PRD v2.0 §6:

- spoken forms: "john three sixteen", "John chapter 3 verse 16",
  "First Corinthians chapter 13", "turn with me to Psalm 23",
  "1st Corinthians five, for seventeen", merged digits ("john 316"),
  ranges ("verses 16 through 18"), two references in one sentence
- all 15 numbered books in every prefix form ("second samuel", "2 samuel",
  "ii samuel", "two samuel")
- fuzzy books ("jhn", "epheseeans") and the exact-beats-fuzzy rule
- quotations land in the confirm queue, never auto-staged
- `[BLANK_AUDIO]` / `[MUSIC]` produce no candidates and no errors
- out-of-range chapters and verses produce honest messages with useful
  suggestions
- "there are three things I want you to remember…" → one suggestion card,
  nothing staged
- ordinary speech ("good morning everyone…") produces nothing

You can re-run the suite anytime:

```bash
cargo test --workspace
```

And you can verify the ASR engine against any WAV file:

```bash
cargo run --release -p rf-app --example e2e -- models/ggml-tiny.en.bin myaudio.wav
```

---

## Part 5 — Honest limits of v0.801.2

- **Whisper tiny.en is good, not magic.** Strong accents, distant room mics
  and worship-band noise hurt it. For the demo, use a normal speaking voice
  0.5–2 m from the mic — or a USB/lapel mic. The parser's honest-error path
  covers the rest without ever showing a wrong verse.
- **KJV only** for stage text; requesting "the NIV" records the request and
  tells the operator licensed texts arrive in M1 (per PRD §6.4 licensing).
- No sermon archive UI, service modes, or branding editor — M2 scope,
  deliberately not faked here.

---

## Stage 2 (v0.815.0): what to try on your Mac

I could not run any of this on a Mac. Please try these and tell me what you see (each takes about a minute).

1. `./run.sh` (the first build downloads and compiles new libraries; it needs internet once and Xcode command line tools).
2. Press **Media & slides** (next to Clear). Drag a **picture** onto the window, press **Show**. Check screen 2 and the small preview.
3. Add an **MP4** from your phone. Show it: does it play smoothly? Does it have sound? Say "pause video", "play video", "restart video". Does the app stop listening while it makes sound?
4. Terminal: `cargo run --release -p rf-media --example decode_speed -- yourvideo.mp4` and tell me the two "frames per second" numbers.
5. **Slides** tab: type two slides, press Make slides, press Next. Try the arrow keys and "next slide".
6. **Songs** tab: New song, paste words with "Chorus:", set the order, Save, Use, then Next. Drop a real `.pptx` from PowerPoint onto the window.
7. **Background** tab: pick a picture, show a slide, move the Darken slider.
8. **Timers** tab: Countdown 1 minute, Start, Show on stage; then show a scripture and back. Check the OBS page (`/stage`) shows the countdown.

