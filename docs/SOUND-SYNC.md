# Keeping sound and picture matched (v0.839.0)

**Problem.** The sound was a separate program (`afplay`). It took about half a second to start, could not start part-way in (every jump had to cut a new copy of the sound), and could not say where it was, so the picture and sound drifted and the operator had to nudge a number. With the window behind others, macOS also slowed the app, so the picture froze while the sound played on, and caught up slowly when you clicked back.

## Top 5 ways to fix it

| # | Way | Verdict |
|---|---|---|
| 1 | Keep `afplay` and add a fixed delay (what 0.836 to 0.838 did) | Works only if the delay is right on every Mac and file. Needs hand tuning. **Rejected as the main fix** (kept as the fallback). |
| 2 | Measure `afplay`'s start-up delay at run time and compensate | Cannot hear when it really starts; guesses again. Rejected. |
| 3 | Decode the sound ourselves with a Rust library and play with `cpal` | Best control, but the AAC decoders are MPL-licensed, which the licence rules forbid, and it is a lot of new code. Rejected. |
| 4 | **Play the sound inside the app with macOS's own `AVAudioPlayer`, and make the sound the master clock; the picture follows its true position** | Starts at once, jumps instantly (no copy to cut), reports its real position, no extra library. **Chosen.** |
| 5 | Re-package the file so sound and picture are one stream | Needs ffmpeg and a long wait per file. Rejected. |

**What was built (way 4).** `macaudio.rs` talks to `AVAudioPlayer`. `sound.rs` uses it first and falls back to `afplay` if the Mac cannot open the file. Four times a second the picture's position is compared with the sound's; if they differ by more than 90 ms the picture is pulled to the sound (`Player::sync_to`). Jumping a video no longer holds the picture. The Sound sync box only matters in the fallback. `RHEMAFLOW_AFPLAY=1` forces the old way for testing.

**The frozen picture.** Three layers: `LSAppNapIsDisabled` in the app's Info.plist and a `defaults write ... NSAppSleepDisabled` in `run.sh`; the app asks macOS for a "user initiated, latency critical" activity at start; and while a video plays a helper thread asks for a redraw every frame (`KEEP_AWAKE`), because the stage window is drawn from the main window's redraw.

NOT heard or seen on a Mac in this version: compile, then play a video, jump, click away, click back.
