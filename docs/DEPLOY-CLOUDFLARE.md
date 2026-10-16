# Putting the web app on Cloudflare (and what "offline" really means)

*v0.817.0 / Web 0.4. Written for the layout in THIS project. The Cloudflare menu names and limits below are from Cloudflare's public docs as quoted by others and may have changed: check them on screen.*

## What is ready

`python3 web/build.py` makes **`web/dist/`**: the whole app as `index.html` (the KJV is inside it), a manifest, icons, an offline service worker, a `_headers` file and (if `web/bibles/` has them) the public-domain Bibles. **Commit `web/dist/`** to GitHub; publish that folder and nothing else (the `src`, `test` and `data` folders stay private).

## Steps (Cloudflare Pages, the no-build way)

1. Cloudflare dashboard (your existing account) > **Workers & Pages** > **Create** > **Pages** > **Import an existing Git repository**.
2. Choose the repository. Production branch: whatever your repo really uses (the plan I was shown said `master`; check).
3. **Framework preset: None. Build command: `exit 0`. Build output directory: `web/dist`.**
4. Deploy. You get an address like `https://something.pages.dev`.
5. **Do not add your own domain yet.** Run the checks below on the `pages.dev` address first.

*Alternative that builds on Cloudflare:* build command `python3 web/build.py`, same output folder. I have not checked that Cloudflare's build machines have Python 3 (probably), so try the `exit 0` way first.

*Pages or Workers?* A third-party guide (Oct 2026) says Cloudflare now steers NEW projects towards "Workers with static assets", and that Pages is still fully supported. For a pure static app like this either is fine. When the account and payment server arrive (Stage 3) a Worker is the natural home. A starting file for the Workers way is below; **I have not run it**.

```jsonc
// wrangler.jsonc (untested)
{ "name": "rhemaflow", "compatibility_date": "2026-10-01", "assets": { "directory": "./web/dist" } }
```

## The checks (5 minutes, on a computer with Chrome)

1. Open the `pages.dev` address. The address bar should offer **Install** (or use the app's own **Install app** button). Install it; it opens in its own window.
2. Chrome DevTools > **Application** > **Manifest**: name RhemaFlow, three icons, no red warnings. **Service workers**: status "activated".
3. Turn the Wi-Fi off and reload. The app must still open, and Bible versions you had downloaded before are still there.
4. Turn Wi-Fi on, deploy a new build later: the OLD copy keeps running until every RhemaFlow window is closed, then the new one takes over. (This is on purpose, so nothing changes under an operator mid-service.)

## What "offline" means here (be honest with customers)

| Works with no internet | Needs internet |
|---|---|
| Opening the app; the KJV; search; typing a reference; the verse on the stage; Bibles you already fetched once; slides, timers (web version) | **Live voice listening**: the web version uses the browser's own speech service (`SpeechRecognition`). In Chrome that service runs on Google's servers. Safari/Edge differ. |

The Mac desktop app listens offline on its own (Whisper). That is the real offline product; the web version is the free taster. Say so.

## What I tested, and what I could not

* **Tested (real Chromium):** the page installs a worker, the worker takes control, Chromium reports **no installability errors**, the manifest reads cleanly, and with the web server **shut down completely** the app still opens, an already-fetched Bible is still there and one never fetched is honestly missing. A control browser with no saved copy cannot open it. I also broke the worker on purpose and the test failed, as it should.
* **Not tested:** Cloudflare itself (I have no access to your account); the install prompt on a real phone; iOS Safari (it ignores some manifest parts and shows "Add to Home Screen" instead); microphone permission on the real `pages.dev` address; your GitHub repository (I could not see it: the plan I was given describes `index.html`, `styles.css`, `script.js`, which is not this project's layout; compare it with your zip).
* The icon is a simple gold serif **R** on the app's dark colour, drawn by `web/make_icons.py`. Replace it with your real logo when you have one (keep the 192, 512, maskable-512 and 180 sizes).

## Files

`web/src/sw.js` (the worker, its rules are unit-tested), `web/src/manifest.webmanifest`, `web/src/_headers`, `web/src/icons/`, `web/build.py` (makes `web/dist/`), tests: `web/test/pwa.test.js` and `web/test/pwa_test.py`.
