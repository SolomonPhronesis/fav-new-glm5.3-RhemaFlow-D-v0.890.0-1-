# RhemaFlow Web (single file)

Open `rhemaflow-web.html` in **Chrome, Edge or Safari**. KJV built in (1.7 MB).

**For reliable microphone access serve it** (opened as a plain file, Chrome re-asks for the mic every time and may refuse it):
```
cd web && python3 -m http.server 8000      # then open http://localhost:8000/rhemaflow-web.html
```

What it does: listens (browser speech recognition), understands "John chapter 3 verse 16" and quoted words ("and have made us kings and priests" -> Revelation 1:6), shows the verse on a stage window, lets you search by book / reference / words, click through books-chapters-verses, plan a sermon, change zoom 60-400%, blackout, read aloud, and saves date-named files (`Sunday 4th October 2026 - 01h27m43s to 02h15m10s - transcript.txt`, `.srt`, `sermon notes.md`, recording).

Same rules as the desktop app: `src/core.js` is a port of the Rust matching, auto-approve, voice commands, naming and transcript logic, tested in Node.

## Honest limits
* **Speech recognition is the browser's:** Chrome and Edge send audio to the vendor's servers (needs internet; less accurate for Scripture than the desktop app's Whisper/R2T2). Safari 14.1+ works; Firefox has no speech recognition (use the search box and keys).
* Browsers allow full screen and popups only after a click, so voice "open stage" / "full screen" may be refused (a message says so); click the stage window and press F. "Exit full screen" by voice works.
* Not the desktop app's offline engine, 66-book accuracy tuning, or R2T2 streaming.

## Develop
```
python3 build.py                         # writes rhemaflow-web.html
node --test test/core.test.js            # 18 logic tests
npm i jsdom && node --test test/dom.test.js   # 14 page tests (drives the real page, with a fake speech engine)
```
Host it anywhere static (GitHub Pages, any web server) over **https** (needed for the microphone).

Layout check in a real browser: `pip install playwright && playwright install chromium && python3 test/layout_check.py $PWD/rhemaflow-web.html /tmp/shot.png 1181 706`
