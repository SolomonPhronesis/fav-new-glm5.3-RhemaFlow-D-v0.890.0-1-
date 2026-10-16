# Stage 2 behaviour spec: pictures, video, slides, songs, backgrounds, timers (desktop)

*Written first, in our own words, before any test or code (rule: `LEGAL-AND-CLEAN-ROOM.md`).
The only things this spec is built from are (1) what our own web version already does
(`web/src/app.js`, `web/src/core.js`), (2) what a church operator needs, (3) our own notes.
No other program's code, files, icons, names or screens were looked at.*

Every numbered rule below is checked by at least one automated test (the test names say which).

---

## 0. What "on the stage" means

The stage is the thing the congregation sees (screen 2). Until now it could show:
nothing, a scripture, or a points list. Stage 2 adds three more things it can show:

| Stage kind | What the people see | Where it comes from |
|---|---|---|
| **Slide** | big centred words (announcement, one part of a song) | Slides tab, Songs tab |
| **Media** | a picture or a playing video, fitted to the screen | Media tab |
| **Countdown / timer / clock** | big numbers and a title | Timers tab |

Rules that hold for all of them (same as scripture, so nobody has to learn new habits):
* S0.1 **Blackout (B)** hides everything but keeps it; pressing B again brings it back. A video keeps running underneath.
* S0.2 **Clear (C)** or the voice command "clear the screen" empties the stage and **stops** any video (and its sound). A **timer is only taken off the screen**: it keeps running in the Timers tab until you press Reset (so a sermon stopwatch survives a scripture being shown).
* S0.3 Putting a scripture on the stage replaces a slide/media/timer, and the other way round. Only one thing is on the stage at a time.
* S0.4 The operator's small preview shows exactly what screen 2 shows.
* S0.5 Nothing here may ever block the screen. If a file is missing or broken the stage stays as it was and the operator sees a plain-words message.

## 1. Slides (announcements and lyrics)

Input is plain text typed or pasted into the Slides tab.

* S1.1 A **blank line** starts a new slide. Several blank lines in a row are still one break. Windows line endings (CRLF) behave like normal ones.
* S1.2 If the **first line** of a slide ends with a colon, is 1 to 30 characters before the colon, has no other colon, **and the slide has more lines**, it is a **label** ("Chorus:"). The label is not shown to the congregation; the operator sees it on the slide's button. A lone line such as `Note:` is text, not a label.
* S1.3 Empty or whitespace-only slides are dropped. Empty input gives no slides.
* S1.4 **Next / Previous** step through the slides. At the last slide "next" does nothing and says "Last slide"; at the first, "previous" says "First slide".
* S1.5 While a slide is on the stage, the existing keys and words **next verse / previous verse** (arrow keys, "next") step through slides instead (same trick as the web version), and **next slide / previous slide** work by voice too (short sentences only, like "next verse").
* S1.6 Text is shown centred, one source line per screen line, shrunk (never cropped) until it fits, and the Zoom setting applies.
* S1.7 **Import**: a `.txt` file is read as text (UTF-8, with or without BOM; invalid bytes become a replacement mark (¿-style) rather than failing). A `.pptx` file gives its words: one slide per PowerPoint slide in slide-number order (slide 2 before slide 10), one line per paragraph, only the words (no pictures, no animation). Entities such as `&amp;` are turned back into characters.

## 2. Song library

* S2.1 A **song** has a title, an optional author, **sections** (each with a label and words) and an **arrangement**: the order the sections are sung, for example `Verse 1, Chorus, Verse 2, Chorus, Chorus`. Empty arrangement = the sections in the order they were written.
* S2.2 Song text is typed like slides: blocks separated by blank lines, a first line ending in a colon is the section label. A block with no label is named `Verse 1`, `Verse 2`... counting only the unlabelled blocks.
* S2.3 Arrangement labels match section labels ignoring upper/lower case and extra spaces. A label that matches nothing is skipped (and reported to the operator), never a crash.
* S2.4 **Slides from a song** = the sections in arrangement order. A chorus used three times makes three slides. A section with more lines than the **lines per slide** setting (default 4, 0 = never split) is split into pieces; the 2nd piece is labelled `Chorus (2)` and so on.
* S2.5 **Search** (case and punctuation do not matter): empty search lists every song A to Z. Otherwise every search word must appear in the title, author or lyrics. Order of results: title starts with the search first, then title contains it, then lyrics-only matches; ties are alphabetical.
* S2.6 The library is one plain JSON file (`~/.rhemaflow/songs.json`), written safely (temporary file then rename, like the service file) so a crash can never leave half a library. A missing file is an empty library. A damaged file is reported and the damaged copy is kept next to it (`songs.json.damaged`), never silently thrown away.
* S2.7 Song ids are never reused after a delete.
* S2.8 No song text is shipped with the app. The church types or imports its own words. (Many modern songs are protected by copyright; the church must hold its own licence, e.g. CCLI in the UK. The app says so in the Songs tab.)

## 3. Pictures and video

* S3.1 **Pictures:** PNG, JPEG, GIF (first frame) and BMP. **Video:** MP4 / M4V files that contain **H.264** video. Anything else (MOV, MKV, AVI, WebM, HEVC/H.265 inside an MP4, WebP, HEIC) is refused with a sentence that says what to do ("Convert it to an MP4 with H.264 video, for example with the free HandBrake app").
* S3.2 Pictures are never loaded larger than 4096 pixels on the long side (bigger ones are scaled down once when loaded) so a 40-megapixel phone photo cannot freeze the app.
* S3.3 **Fit modes:** *Fit* (whole picture visible, empty bands in the stage colour), *Fill* (screen fully covered, edges cropped), *Stretch*. Default Fit. The maths keeps the picture's shape in Fit and Fill, centred.
* S3.4 **Video plays on its own clock**, so a slow computer drops frames rather than falling behind: after 10 seconds of wall time the video is at 10 seconds.
* S3.5 **Play / Pause / Restart** by button and by voice ("play video", "pause video", "restart video"). Pause keeps the frame on screen. Restart goes to 0:00 and plays.
* S3.6 **Loop**: when on, the video starts again at the end with no gap on screen. When off, the **last frame stays** (no black flash) and the operator sees "ended".
* S3.7 Frames are shown in **presentation order**, not file order (videos with B-frames store frames out of order).
* S3.8 A video's **sound track is skipped over** correctly (the file has a second track); the picture still plays.
* S3.9 **Sound:** the H.264 decoder only makes pictures. Sound is played by macOS's own `afplay` tool (a program that is already on every Mac, so no extra library). It starts and pauses with the video. **Mute** is a tick-box. While a video with sound is playing the microphone is ignored, exactly like while reading aloud, so the film's own voices cannot be mistaken for the preacher. *(Not testable without a Mac: see "Not verified".)*
* S3.10 A damaged or cut-off file gives an error message, never a crash.
* S3.11 The media list remembers file **paths** (not copies) between runs; a path that no longer exists is shown greyed with "missing" and cannot be put on the stage.

## 4. Background loops behind slides

* S4.1 A background is nothing (the stage colour), a **picture**, or a **video loop**. The video loop always loops and is always silent.
* S4.2 It sits behind slide and countdown text. It is **not** shown behind scriptures (scripture text is the main thing; keeping it plain stays the safe default).
* S4.3 **Darken** slider, 0 to 85 %: a black layer between the background and the words so text stays readable. Default 45 %. Never more than 85 % (a fully black layer would just be "no background").
* S4.4 The words get a soft shadow so they stay readable on bright pictures.
* S4.5 Changing the background does not interrupt the slide on screen.

## 5. Timers and clocks

Four kinds, one at a time on the stage:

* S5.1 **Countdown** for a length of time ("Service starts in" 5 minutes). Shows `mm:ss`; from one hour up it shows `h:mm:ss`. It counts **up to the next whole second** so "00:00" appears only when time is really over (same as the web version).
* S5.2 **Countdown to a clock time** ("until 10:30"). It follows the real clock, so it stays right even if the app was busy. If that time has already passed it shows 00:00 and is finished (it never jumps to tomorrow).
* S5.3 **Stopwatch** (count up), e.g. sermon length. Shows `mm:ss`, from one hour `h:mm:ss`. Rounds **down**.
* S5.4 **Clock** (time of day), 24-hour or 12-hour.
* S5.5 Countdowns can be **paused, resumed and reset**. A finished countdown stays on screen at 00:00 until cleared; it does not vanish by itself and does not flash.
* S5.6 In the operator panel a countdown shows a **warning** in its last 60 seconds and **done** at zero. (The congregation sees no colour change: nothing flashes.)
* S5.7 The title ("Service starts in") is shown above the numbers on the stage.
* S5.8 `HH:MM` input accepts `10:30`, `9:05`, `0930`, `9.05`, `9:05pm`, `12am`. Bad input gives no time (and a message), never a crash.

## 6. Voice commands and keys added

| Say | Does |
|---|---|
| "play video", "start the video" | play |
| "pause video", "pause the video" | pause |
| "restart video", "replay the video" | restart |
| "next slide", "previous slide" | step slides |
| (existing) "next", "next verse", arrow keys | step slides when a slide is on the stage |

All are *short-utterance* commands (they only fire in a short sentence), so a preacher saying "...and then they played the video of..." does not start anything.

## 7. OBS page and phone remote

* S7.1 Slides and countdowns appear on the OBS page as text (lines kept). Pictures and video are **not** sent to the OBS page in this version (it would need a media server); the page shows the stage empty and says so in the docs.

## 8. The video library choice (and why)

We needed something that turns an H.264 video file into pictures, under a licence that lets us sell closed software. Everything in the final tree is MIT, Apache-2.0, BSD-2-Clause, Zlib, 0BSD or Unlicense; **no GPL, AGPL or MPL**.

| Option | Licence | Verdict |
|---|---|---|
| **`openh264` crate + `mp4` crate** (chosen) | BSD-2-Clause (Cisco's OpenH264 source) + MIT | Decodes the format almost every church video uses (H.264 in an MP4). Builds from source on any machine, works offline, no Mac-only parts. **Picture only, no sound.** |
| FFmpeg via `ffmpeg-next` | FFmpeg is LGPL 2.1+ or GPL depending on how it is built; most builds people have are GPL | **Rejected.** Dynamic linking an LGPL build can work, but one wrong build flag makes it GPL, and it is hard to ship on a Mac. Too easy to get wrong. |
| GStreamer | LGPL | Rejected: needs a large separate install. |
| `symphonia` (sound) | MPL-2.0 | Not on the allowed list (MPL is "weak copyleft"). Not used. We play sound with macOS `afplay` instead. |
| `rodio` | MIT/Apache, but it uses `symphonia` to decode | Same reason. |
| Apple AVFoundation (system) | Apple's | Excellent on a Mac but needs Objective-C bindings and cannot be tested here. A good later swap. |
| Pure-Rust H.264 decoders | various | None I could find is complete and maintained enough to bet a church service on. |

**Patents (H.264 is patented; this is not legal advice).** The *source code* licence (BSD) and the *patent* licence are different things. From the web (checked 7 Oct 2026): the H.264 patent pool run by Via LA lists the first 100,000 units as US$0 and 100,001 to 5 million at US$0.20 each; Cisco's own pre-built OpenH264 file is covered by Cisco's licence at no cost to you but only if you use *that file* and show the words "OpenH264 Video Codec provided by Cisco Systems, Inc."; people who **compile OpenH264 from source** (what we do, so the app builds anywhere offline) are told they should arrange their own licence. **Decision for you:** for a free or personal build this is not a worry; **before you sell it widely, either (a) ask Via LA (info@via-la.com) to confirm the 100,000-free tier for you and sign up, or (b) switch the one line `OpenH264API::from_source()` to Cisco's downloadable file (the crate supports it) and keep the Cisco credit line.** The credit line is already shown at the bottom of the Media tab and in `THIRD-PARTY.md`.

## 9. Not in this stage (said plainly)

* The **service order** (a saved list mixing scriptures, songs, slides, media, timers; `rf-service` already has the model and the live runner) is **not yet wired to the screen**. You build the running order by clicking items in each tab. Wiring the runner is a good first job for Stage 5/7.
* Video **sound** goes straight to the Mac's default output. Choosing a different audio device for video is not done.
* No transitions, no per-slide fonts or themes, no "stage display" for the preacher (Stage 5), no NDI (Stage 5).
* No video in the OBS page (see S7.1).

## 10. Not verified here (needs a real Mac)

1. How it looks on a real Mac window and on screen 2 (the drawing is tested headlessly: the right text and image shapes are painted, nothing outside the window).
2. Real video playback speed and smoothness (decode speed measured on this Linux machine only), the OpenH264 build on Apple Silicon, `afplay` sound, microphone muting during video.
3. The file picker (`osascript choose file`) and drag-and-drop of files onto the window.
4. Opening a `.pptx` made by PowerPoint/Keynote/Google Slides. Tested on files made by python-pptx only.
5. Any H.264 file from a camera/phone that is interlaced, 10-bit or uses unusual settings. Tested on one x264 clip with B-frames.
