# Why it "stopped listening after 1 second", and how to make it always listen (v0.808)

## 1. The 93.9% question (I owe you a correction)
You were right to push back. The README never prints "93" or "93.9" as a headline. But their own accuracy table does contain it: the nine English test sets at 160 ms average **6.1% word errors**, and 100 − 6.1 = **93.9% correct**. So the figure is real, and I was wrong to make it sound like it was not there. It comes from clean test recordings (books, TED talks, meetings), not from a church.

## 2. What was really going wrong (MEASURED, not guessed)
I ran real human speech through the real voice detector.

| Mic level | Before | What happened |
|---|---|---|
| normal | fine | speech kept |
| quiet (x0.1) | 4 pieces, 8% of clearly-spoken audio never reached the recogniser (re-measured on v0.809 without the auto-volume) | detector thought soft words were silence and cut the sentence |
| very quiet (x0.05) | 2 pieces, **43% of the speech lost** (re-measured on v0.809 without the auto-volume) | "stops hearing me" |

**Cause 1: a fixed loudness rule.** The detector needed the voice to be a fixed amount louder than the room, and it used the same rule to *stay* in speech as to *start*. A built-in Mac mic at a low volume sits near that line, so soft words, endings and quieter syllables counted as silence. After 0.4 s of "silence" it ended the sentence. That is the "it stops after about a second".

**Cause 2: no way to handle long talking.** A non-stop talker was only cut by a hard 30-second chop in the middle of a word, and the live text only ever showed the last 14 seconds.

**Cause 3 (not proven, now logged): the Mac pausing the app.** macOS can slow down a window nobody is looking at (App Nap) or let the Mac / projector sleep. I cannot test that from here. So v0.808 (a) keeps the Mac and display awake with `caffeinate` and (b) writes a `ui_stall` line to the service's `events.jsonl` and shows a note on screen if the window was not drawn for 1.5 s while live. If it happens again, that line tells us it was the Mac and not the microphone.

## 3. What changed (all in v0.808.0)
1. **Auto-volume (AGC).** Quiet voices are lifted up to +24 dB; it never turns anything down; it ignores steady noise and music beds (it only learns from sound that wobbles like speech). The recorded `audio.wav` stays untouched.
2. **Hysteresis.** Starting to hear speech takes a bit more than *continuing* to hear it, so soft word endings stay inside the sentence.
3. **Steady-background learning.** A fan, hum or music bed that is already playing is recognised as steady and no longer counts as one never-ending "utterance".
4. **Soft cut for long talking.** After about 12 s of non-stop speech it cuts at the quietest moment (a breath between words) and keeps listening. Each piece gets its own full transcript, so a 60-second sermon passage becomes about six pieces with nothing lost.
5. **Streaming sidecar:** resets at a breath too, not mid-word; sends the book names as hints.

## 4. Results (MEASURED on this machine, JFK speech, Whisper tiny.en on CPU)
Word accuracy for the same sentence through the whole front end:

| Mic | Before | After |
|---|---|---|
| normal | 100% | 100% |
| quiet (x0.1) | 86.4% | 95.5% |
| very quiet (x0.05) | 50.0% | 95.5% |

Other measured results (`cargo test -p rf-asr --test continuous_speech -- --nocapture`):
- 100% of clearly-spoken audio reaches the recogniser at every mic level, with noise 10 and 20 dB below the voice.
- Steady fan-type noise at three levels and a steady music bed are treated as silence after a few seconds.
- Speech 10 dB over a loud music bed: about 92% reaches the recogniser (re-measured on v0.809). **That is the honest limit of this kind of detector.** Beating it needs a neural voice detector or the streaming engine (it listens to everything).
- 64 s of non-stop talking: six pieces of about 11 s, first text after 11.7 s, none lost.

**Limits:** one speaker, one clip, one small model, CPU. Not your voice, not a sanctuary, not an M1. Run `rhemaflow-d calibrate` to get your own numbers.

## 5. "Full language model or a model just for English and the KJV?"
Plain answer: **use the big streaming model for the ears, and teach the app the KJV on top.** Training a new speech model from nothing needs thousands of hours of recorded speech and rented GPUs; it cannot be done here and would almost certainly be worse than R2T2.

| Option | Good | Bad | Verdict |
|---|---|---|---|
| A. R2T2 as it is (English + many languages) | already 93.9% on clean test sets, append-only, hints supported | big (2 B), M1 may need 320-640 ms chunks | **use now** |
| B. A smaller English-only Whisper (base.en / small.en) | small, fast, runs on M1 today | not streaming, ~1 s+ behind | fallback |
| C. Fine-tune R2T2 (or Qwen3-ASR) on KJV speech | best for Habakkuk, thee/thou, 66 books | needs GPU + hundreds of hours of real recordings of YOUR speakers; R2T2 licence allows tuning R2T2 itself | **later, only with real recordings** |

What gives most of C's benefit **today**, without training:
- Book names as hints to the model (`sidecar/context_books.txt`).
- The app already snaps what it hears to real KJV verses and 66-book names (the parser and quote matcher).
- Your personal lexicon from `calibrate` (fixes *your* mishearings).

## 6. Do-it steps, explained like you are 9
Think of the app as a person with ears.

1. **Make the ears the right loudness.** Some people whisper, some shout. The new auto-volume is like a friend who turns the radio up when the singer is quiet. *(done)*
2. **Don't think "quiet" means "finished".** Before, if you got a bit soft, the app thought you were done and wrote down half a sentence. Now it only decides you stopped after a real pause. *(done)*
3. **Take breaths in a smart place.** When you talk for a very long time, the app writes down what you said every few seconds, always at a tiny pause so it never cuts a word in half. *(done)*
4. **Don't mix up music with talking.** If a fan or music is steady, the app learns "that's just background" and listens for the voice on top. *(done, with the limit above)*
5. **Stay awake.** The Mac is told "don't nap while church is on". *(done; logs it if the Mac still pauses the window)*
6. **Learn the funny words.** Run `calibrate` and read 30 sentences. The app remembers that when it hears "have a cook" you meant "Habakkuk". *(tool done; you need to run it)*
7. **Get the big brain.** Install R2T2 with `sidecar/setup_mac.sh`. It writes words as you say them and never changes them, like the video. *(code done; needs you to run it on your Mac)*
8. **Test with real people.** Record 10 minutes each from 3 different speakers (accents!), with the real mic in the real room, once with music and once without. Read them with `calibrate --wav-dir`. This is how we find out the real accuracy. *(you do)*
9. **Only then teach the big brain the KJV** (fine-tune), if the numbers are still under 93%. *(later)*

## 7. Do this on your Mac, in this order
```bash
# 1. the fix for "stops listening" is in the normal app already
./run.sh --yes
#    Level meter now shows your REAL mic level (not the levelled one).
#    If you ever see "screen was paused by macOS", the Mac did it; tell me.

# 2. measure yourself (10 minutes)
./target/release/rhemaflow-d calibrate        # read 30 sentences; then run it again

# 3. optional: the streaming engine from the video
./sidecar/setup_mac.sh
~/RhemaFlow-R2T2/run_sidecar.sh
./run.sh --stream-url ws://127.0.0.1:8272/asr_stream_api_v1

# 4. if the screen still freezes when the window is hidden (App Nap), try this once (unverified):
defaults write NSGlobalDomain NSAppSleepDisabled -bool YES
```
