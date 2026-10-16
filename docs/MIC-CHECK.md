# Making the microphone and transcript more accurate (v0.818.0)

**The one idea to remember:** speech recognition can only be as good as the sound it is given. In a church the problem is usually the *sound*, not the software. So measure the sound first.

## Run the microphone check

1. Record **30 seconds** of the preacher (or a volunteer reading aloud) from the real microphone setup, at normal volume. Any recorder works; save as WAV.
2. `cargo run --release -p rf-asr --example mic_report -- sunday.wav`
3. Read the list. `[ OK ]` fine, `[CHECK]` worth fixing, `[ FIX ]` fix before anything else. Each line says what to do.

It checks: is there a voice, how loud, is it clipped (cut off at the loud parts), how far the voice stands above the background, mains hum (50 Hz in the UK) **judged by how loud it is next to the voice**, and a constant offset. Exit code 0 / 1 / 3 for good / check / fix, so a script can use it.

*What I tested:* real speech (the 1961 recording in `crates/rf-asr/tests/fixtures`) plus deliberately broken copies (too quiet, clipped, noisy, hummed, silent, too short). An independent FFT (numpy) confirmed that recording really has a 60 Hz hum about 35 dB under the voice, so the check correctly leaves it alone: a hum only matters when it competes with the voice. *What I could not test:* your real room and microphones; the numbers (for example "voice at -20 dB", "25 dB above the background") are the usual rules of thumb, not measured on your church.

## The top 5 ways to improve accuracy, best first, and what is already done

| # | Way | Why it matters | Status |
|---|---|---|---|
| 1 | **Fix the sound source.** Take the feed from the mixing desk (a line or aux out into a USB interface) or give the preacher a headset or lapel microphone. A room microphone hears the echo, the crowd and the fans. | Nothing in software repairs a voice that is 6 dB above the noise. This beats every model upgrade. | Check it with `mic_report` (new). Hardware, not code. |
| 2 | **Use a bigger speech model for the Mac's chip** (small.en, distil-large, or large-v3-turbo with Metal) and measure the speed. | More accurate on unusual words and accents. Costs speed and memory. | Model choice exists; **I could not run Whisper here**, so I cannot say which size is best on your Mac. Test on your own recording. |
| 3 | **Give the engine the right words.** Book names, the church name, the pastor's and local names, in the prompt. | Names are what recognition gets wrong. | Book names and Bible phrases are already in the prompt. **Church and pastor names are not: good next job.** |
| 4 | **Don't cut speech at every breath.** Keep joined phrases together and use the Bible text to repair what is heard. | Your screenshot showed one verse arriving as nine 1-3 word pieces with wrong words. | **Done in 0.816.0** (garbled-quote matcher + 45-second memory). Segment length tuning still needs real audio. |
| 5 | **Learn from the operator's corrections and keep real recordings to measure against.** When the operator fixes a detection, remember it for next time; keep a few Sundays of audio with the correct words typed so every change is measured (`compare.sh`, the WER tool). | You cannot improve what you cannot measure; a church has the same mishearings every week. | Measuring tools exist. **Learning from corrections is not built (good next job).** |

Honest note: a cloud speech engine (Deepgram, Google...) can be more accurate with custom word lists, but needs internet, costs money per hour and sends sermon audio away. It does not fit "works offline", so I would offer it only as an optional paid choice later.
