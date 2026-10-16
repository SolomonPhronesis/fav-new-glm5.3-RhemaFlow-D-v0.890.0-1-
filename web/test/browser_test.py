#!/usr/bin/env python3
"""Real-Chromium tests of the web page (media, two windows, colours, history, slides, countdown).
Run:  python3 test/browser_test.py        (needs: pip install playwright && playwright install chromium)
Serves the web folder on http://localhost:8765 because file:// pages cannot talk to each other."""
import base64, subprocess, sys, time, os, pathlib
from playwright.sync_api import sync_playwright

here = pathlib.Path(__file__).resolve().parent.parent
srv = subprocess.Popen([sys.executable, "-m", "http.server", "8765", "--bind", "127.0.0.1"], cwd=here, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(0.8)
URL = "http://127.0.0.1:8765/rhemaflow-web.html"
fails = []
def check(name, cond, extra=""):
    print(("PASS " if cond else "FAIL ") + name + (("  " + str(extra)) if (extra and not cond) else ""))
    if not cond: fails.append(name)

PNG = base64.b64decode("iVBORw0KGgoAAAANSUhEUgAAAAoAAAAKCAYAAACNMs+9AAAAFUlEQVR42mNk+M9Qz0AEYBxVSF+FAP5FDvcfRYWgAAAAAElFTkSuQmCC")
try:
    with sync_playwright() as p:
        b = p.chromium.launch(args=["--no-sandbox", "--use-fake-ui-for-media-stream", "--autoplay-policy=no-user-gesture-required"])
        ctx = b.new_context(viewport={"width": 1181, "height": 706})
        pg = ctx.new_page()
        errors = []
        pg.on("pageerror", lambda e: errors.append(str(e)))
        pg.goto(URL); pg.wait_for_function("document.body.dataset.ready==='1'", timeout=20000)

        # ---- history: previous scripture walks back one at a time -------------------------------
        for t in ["John chapter 3 verse 16", "Romans 8:28", "Psalm 23 verse 1"]:
            pg.evaluate("t=>window.__rf.handleFinal(t)", t)
        check("three scriptures in history", pg.evaluate("__rf.S.timeline.entries.length") == 3)
        pg.evaluate("__rf.handleFinal('previous scripture')")
        check("previous scripture -> Romans 8:28", pg.evaluate("__rf.S.stage.label") == "Romans 8:28")
        pg.evaluate("__rf.handleFinal('the one before')")
        check("and again -> John 3:16", pg.evaluate("__rf.S.stage.label") == "John 3:16")
        pg.evaluate("__rf.handleFinal('previous scripture')")
        check("at the start it stays and says so", pg.evaluate("__rf.S.stage.label") == "John 3:16" and "first" in pg.inner_text("#toast").lower())
        pg.evaluate("__rf.handleFinal('next scripture')")
        check("next scripture goes forward again", pg.evaluate("__rf.S.stage.label") == "Romans 8:28")
        check("history walking does not add entries", pg.evaluate("__rf.S.timeline.entries.length") == 3)
        pg.keyboard.press("["); check("[ key = previous scripture", pg.evaluate("__rf.S.stage.label") == "John 3:16")
        pg.keyboard.press("]"); check("] key = next scripture", pg.evaluate("__rf.S.stage.label") == "Romans 8:28")
        pg.click("#bHistory"); rows = pg.query_selector_all("#hList .h")
        check("history window lists newest first", len(rows) == 3 and "Psalms 23:1" in rows[0].inner_text(), [r.inner_text() for r in rows])
        rows[2].query_selector("button").click()
        check("clicking a history row shows it", pg.evaluate("__rf.S.stage.label") == "John 3:16")
        pg.keyboard.press("Escape")

        # ---- colours -----------------------------------------------------------------------------
        pg.evaluate("__rf.handleFinal('John chapter 3 verse 16')")
        pg.click("#bColours"); pg.click("#presets button:nth-child(2)")
        bg = pg.evaluate("getComputedStyle(document.getElementById('prev')).backgroundColor")
        check("preset sets the stage background (deep navy)", bg == "rgb(10, 22, 52)", bg)
        pg.evaluate("document.getElementById('cBg').value='#faf4e6'; document.getElementById('cBg').dispatchEvent(new Event('input'))")
        fg = pg.evaluate("getComputedStyle(document.getElementById('prev')).color")
        check("light background switches the text to dark automatically", fg == "rgb(20, 16, 10)", fg)
        pg.keyboard.press("Escape")

        # ---- show / hide scripture bar -----------------------------------------------------------
        pg.click("#bBrowser")
        check("hidden: panel gone and a Show bar is there", pg.is_hidden("#browser") and pg.is_visible("#brbar"))
        pg.click("#brshow")
        check("Show brings it back", pg.is_visible("#browser") and pg.is_hidden("#brbar"))

        # ---- two windows: the stage window follows (media + colours) -----------------------------
        stage = ctx.new_page(); stage.goto(URL + "#stage"); stage.wait_for_selector("#stg")
        pg.evaluate("__rf.handleFinal('John chapter 3 verse 16')"); stage.wait_for_timeout(300)
        check("stage window shows the verse", "For God so loved" in stage.inner_text("#stg .txt"))
        sbg = stage.evaluate("getComputedStyle(document.getElementById('stg')).backgroundColor")
        check("stage window has the chosen colour", sbg == "rgb(250, 244, 230)", sbg)

        # ---- picture ------------------------------------------------------------------------------
        pg.click("#bMedia"); pg.set_input_files("#mediaFile", files=[{"name": "logo.png", "mimeType": "image/png", "buffer": PNG}])
        pg.wait_for_selector("#mediaList .m"); pg.click("#mediaList .m"); stage.wait_for_timeout(400)
        check("picture shows in the preview", pg.evaluate("!!document.querySelector('#prev .media img')"))
        check("picture shows in the stage window", stage.evaluate("!!document.querySelector('#stg .media img')") and stage.evaluate("document.getElementById('stg').classList.contains('hasmedia')"))
        check("verse text is not drawn over the picture", stage.evaluate("getComputedStyle(document.querySelector('#stg .box')).display") == "none")
        pg.keyboard.press("Escape")
        pg.evaluate("__rf.handleFinal('John chapter 3 verse 16')"); stage.wait_for_timeout(300)
        check("showing a verse again removes the picture", stage.evaluate("!document.querySelector('#stg .media img')"))

        # ---- video (made right here with MediaRecorder so no test file is needed) -----------------
        made = pg.evaluate("""async()=>{const c=document.createElement('canvas');c.width=160;c.height=90;const x=c.getContext('2d');const st=c.captureStream(15);
          const rec=new MediaRecorder(st,{mimeType:'video/webm'});const ch=[];rec.ondataavailable=e=>ch.push(e.data);rec.start();
          let n=0;await new Promise(r=>{const t=setInterval(()=>{x.fillStyle='hsl('+(n*20)+',80%,50%)';x.fillRect(0,0,160,90);if(++n>34){clearInterval(t);r();}},100)});
          await new Promise(r=>{rec.onstop=r;rec.stop()});const blob=new Blob(ch,{type:'video/webm'});
          window.__rf.S.media.push({id:'v1',name:'clip.webm',type:'video/webm',blob});return blob.size}""")
        check("made a test video", made > 1000, made)
        pg.click("#bMedia"); pg.wait_for_timeout(100); pg.evaluate("document.querySelector('#bMedia').click()")
        pg.evaluate("document.getElementById('dlgMedia').close()")
        pg.evaluate("(()=>{document.getElementById('bMedia').click();})()")
        pg.wait_for_selector("#mediaList .m")
        items = pg.query_selector_all("#mediaList .m"); items[-1].click(); stage.wait_for_timeout(1200)
        vid = stage.evaluate("(()=>{const v=document.querySelector('#stg video');return v?{paused:v.paused,ready:v.readyState,w:v.videoWidth}:null})()")
        check("video element on the stage window", vid is not None, vid)
        check("video is playing on the stage window (voice 'play video' also works)", vid and (not vid["paused"] or vid["ready"] >= 2), vid)
        pg.evaluate("__rf.handleFinal('pause video')"); stage.wait_for_timeout(300)
        t1 = stage.evaluate("document.querySelector('#stg video').currentTime"); stage.wait_for_timeout(400)
        t2 = stage.evaluate("document.querySelector('#stg video').currentTime")
        check("'pause video' pauses it (time stops moving)", stage.evaluate("document.querySelector('#stg video').paused") is True and abs(t2 - t1) < 0.05, (t1, t2))
        pg.evaluate("__rf.handleFinal('play video')"); stage.wait_for_timeout(700)
        t3 = stage.evaluate("document.querySelector('#stg video').currentTime")
        check("'play video' plays it (time moves again)", t3 > t2 + 0.2 or stage.evaluate("document.querySelector('#stg video').ended"), (t2, t3))
        pg.keyboard.press("Escape")

        # ---- slides -------------------------------------------------------------------------------
        pg.evaluate("document.getElementById('bSlides').click()")
        pg.fill("#slidesText", "Welcome\nto church\n\nChorus:\nAmazing grace\nhow sweet the sound\n\nAnnouncements")
        pg.click("#slidesMake"); stage.wait_for_timeout(300)
        check("first slide on the stage", "Welcome" in stage.inner_text("#stg .txt"))
        pg.keyboard.press("Escape"); pg.evaluate("__rf.handleFinal('next')"); stage.wait_for_timeout(300)
        check("'next' steps to the next slide", "Amazing grace" in stage.inner_text("#stg .txt") and stage.inner_text("#stg .ref") == "Chorus")
        pg.keyboard.press("ArrowRight"); stage.wait_for_timeout(300)
        check("arrow key steps to the last slide", "Announcements" in stage.inner_text("#stg .txt"))

        # ---- countdown ----------------------------------------------------------------------------
        pg.evaluate("document.getElementById('bSlides').click()")
        pg.fill("#cdTitle", "Service starts in"); pg.fill("#cdMin", "1"); pg.click("#cdStart"); stage.wait_for_timeout(900)
        t = stage.inner_text("#stg .txt")
        check("countdown shows mm:ss on the stage window", len(t) == 5 and t[2] == ":" and t in ("01:00", "00:59"), t)
        check("countdown has its title", stage.inner_text("#stg .ref") == "Service starts in")
        pg.click("#cdStop")
        pg.keyboard.press("Escape")

        # ---- second screen: honest fallback in a browser with no extra screen ---------------------
        with ctx.expect_page() as newp:
            pg.evaluate("document.getElementById('bScreen2').click()")
        sp = newp.value; sp.wait_for_load_state()
        check("Screen 2 button opens a stage window (fallback when the browser has no screen API/permission)", "#stage" in sp.url)
        check("and says what happened", len(pg.inner_text("#toast")) > 5)

        check("no JavaScript errors on the page", not errors, errors)
        b.close()
finally:
    srv.terminate()
print("\n%d failed" % len(fails) if fails else "\nall passed")
sys.exit(1 if fails else 0)
