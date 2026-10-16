# HANDOFF: read this first (RhemaFlow-D v0.824.0 / Web 0.5)

**v0.830.0:** UI/UX overhaul round 1: big live-control row, two-column middle, preview badge, type scale, Cut/Fade (`stage_fade_ms`, `arrival_alpha`), `shot_probe.rs` (rasterize the UI to PPM: `cargo test -p rf-app --ignore-rust-version shot_probe -- --nocapture`, files in /tmp/rf-shots). Not seen on a real Mac. Stage monitor and themes still NOT started.

**v0.829.0:** run.sh disk-space check + old-build cleanup (user hit "No space left on device"), Stop button (`media_stop`). Overhaul / stage monitor / themes+transitions still NOT started; ask which first.

**v0.828.0:** right-click "When it ends" on video/sound (`AfterEnd`, `PresentSettings::after_end`, `tick_at`). Not seen on a real Mac. Overhaul and top-20 NOT started.

**v0.827.0:** run.sh now answers Homebrew's own y/n prompt (`brew_install`); play bar for video and sound (`transport_ui`, `Transport`, `Present::media_seek/media_skip`, `Sound::seek` via ffmpeg, `Player::seek`). Verify on the Mac: sound jumping, run.sh with real Homebrew. Still waiting for the mp3 failure text and `last-video-problem.txt`.

**v0.826.0:** run.sh auto-yes + ffmpeg install; layout fixes (probe `tools/sandbox-check/ux_probe.rs`); mp4fix handles unreadable handlers and writes `~/.rhemaflow/last-video-problem.txt`. The user's mp4 ("mp4a box contains a box with a larger size") and mp3 still failed on the Mac in v0.825.0: ASK FOR that report file and the Terminal text of any crash. Next: UI/UX overhaul step by step, then top 20.

**v0.825.0:** service order is on screen (`rf-app/src/order.rs`, right panel tab; wraps `rf_service::Runner`), sound files (`rf-service/src/media.rs` Audio kind, `Loaded::Audio` in `present.rs`, `sound.rs` reports failures), automatic ffmpeg converter (`rf-media/src/convert.rs`, hooked in `add_with_repair` in `present.rs`). LOOK AT IT ON THE MAC: panel look, drag from the bar, real mp3/mp4 sound. Next: fix the stage preview being squeezed on windows of about 1280x720 or smaller (measured with a headless probe, not seen), then UI/UX overhaul step by step, then the top 20 in `UX-DESIGN-REVIEW.md` order (service order done in v1; still no rename, no Logo).

**v0.824.0:** MEDIA bar upgrade (virtual grid, background adds with progress, search/size/list/sort, keyboard). See `docs/UX-DESIGN-REVIEW.md` for the ranking and top-20 feature list. Optional next: "safe mode" (click selects, Enter shows).

**v0.823.0:** media is a docked bar (`Present::dock_ui`, called from `ui_media_dock` in app.rs), the popup keeps Slides/Songs/Background/Timers. LOOK AT IT ON THE MAC.

**v0.822.0:** media window is a file manager (folders, right-click, drag, Finder folder drop), mp4 sound-track repair (`rf-media/src/mp4fix.rs`), scripture lanes panel replaces the simulator. LOOK AT IT ON THE MAC (never drawn in a real window here). Next: service order/playlist, songs, Windows.

**v0.821.0:** stage self-correcting size loop (`app.rs` + `displays.rs`), corrections in the transcript (double-click), shelf video thumbs/rename/move, phone `/media`. TEST ON THE MAC: screen 2 at 100%, whisper `native` build with learned names. Next: copy progress bar, FreeShow/ProPresenter gap list in the chat answer.

**v0.820.0:** media shelf (`rf-service/src/library.rs`, grid in `present.rs`). Next: video preview frames, rename/move buttons, copy progress bar, drag onto shelf, shelf in the phone remote.

**v0.819.0:** `rf-core/src/learn.rs` (learn from corrections; library only, UI and Whisper prompt NOT wired), web sample-sermon button, `site/` landing page, `web/make_deploy.py`. Media: pictures PNG/JPEG/BMP/first-frame GIF and H.264 MP4 already work (v0.815); NOT supported: .mov, .mp3/.wav audio files, WebP, HEVC. Good next jobs: wire learn.rs; audio-file items; .mov.


**What this is:** a church scripture-display program. It listens to a preacher, finds the Bible verse, and shows it on screen 2. Two versions share the same rules: a **desktop app** (Rust, egui, macOS/M1; offline Whisper) and a **single-file web app** (`web/rhemaflow-web.html`; browser speech).

## Map
| Where | What |
|---|---|
| `crates/rf-bible` | Bible text, quotation matching (hath/have), ranked candidates, full-text search, importer for other Bible files |
| `crates/rf-core` | Reference parser, voice-command table (`cmdtable.rs`, 350+ phrases, fuzzy), stream logic |
| `crates/rf-service` | Pure logic, no UI: service order + live runner (not yet on screen), **slides** (text/.pptx), **songs** (library, order, search), **timer**, **media** rules (classify, fit maths, video clock, darken) |
| `crates/rf-strongs` | Strong's: built-in CC0 Greek dictionary, optional Hebrew, English-word to original-word search, reader for Strong's-tagged OSIS Bibles. `tools/fetch_strongs.py`, `tools/make_strongs_greek.py`, `docs/STRONGS.md` |
| `crates/rf-media` | Pictures (PNG/JPEG/GIF/BMP) and H.264-in-MP4 video -> RGBA frames in display order; `Player` thread plays on its own clock. `examples/decode_speed.rs` |
| `crates/rf-app` | The desktop UI. **New in 0.815.0:** `present.rs` (the Media & slides window and all its logic, fully testable without a window), `sound.rs` (video sound via macOS `afplay`), `stage.rs` now draws Slide / Media / Countdown. `actions.rs` (what happens), `ui.rs` (screens), `browser.rs` (books/chapters/verses + search), `stage.rs` (drawing), `displays.rs` (screen 2), `history.rs` (previous/next scripture), `naming.rs` + `export.rs` (date-named files, transcripts), `versions.rs` (Bible files), `web.rs` (OBS page + phone remote), `license.rs` (subscription RULES only) |
| `web/` | `src/core.js` (all web logic, tested in Node), `src/app.js` (page), `build.py` (makes the single file), `test/` |
| `bibles/`, `web/bibles/` | Bundled public-domain Bibles (ASV, YLT, Darby, Webster, Rotherham, AKJV; BBE in web). Excluded from the slim zip. |
| `tools/fetch_bible.py` | Bible fetch (`pd` tested; `api` for API.Bible NOT run live) |
| `tools/sandbox-check/` | Lets a chat compile and test the desktop app without a Mac (`setup.sh`) |
| `docs/` | `ROADMAP-50.md` (57 features, 7 stages), `STAGE2-SPEC.md` (behaviour spec + video-library choice), `THIRD-PARTY.md` (root), `LICENSING-AND-PRICING.md`, `LEGAL-AND-CLEAN-ROOM.md`, `BIBLE-VERSIONS.md`, `TOP-20.md`, `VOICE-COMMANDS.md`, `CHANGELOG.md` |

## How to test
```
bash tools/sandbox-check/setup.sh "$PWD" /tmp/rf-check
cd /tmp/rf-check && cargo test -p rf-app -p rf-core -p rf-bible -p rf-service -p rf-media -p rf-strongs --ignore-rust-version   # see CHANGELOG for the count. The sandbox also runs 4 end-to-end probes (tools/sandbox-check/command_probe.rs) that speak to the real app
cd web && python3 build.py && node --test test/core.test.js                                           # 23
NODE_PATH=<where jsdom is installed> node --test test/dom.test.js                                     # 14
PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers python3 test/browser_test.py                                # 33 real-Chromium checks
```
On a real Mac with current Rust, just `cargo test` and `./run.sh`.
Sandbox notes (Rust 1.75, Ubuntu): `apt-get install rustc cargo libasound2-dev pkg-config` (run `apt-get update` first); set `CARGO_TARGET_DIR=/tmp/rf-target` so re-running `setup.sh` does not throw away the build cache; `openh264` must stay at 0.5 there (0.6 needs Rust 1.82). `pip install av python-pptx --break-system-packages` only to MAKE test fixtures (not needed to run tests).
The first desktop build on the Mac downloads and compiles new crates (openh264 needs a C++ compiler: Xcode command line tools are enough). `Cargo.lock` was NOT regenerated for the new crates (the sandbox lock has stubs); cargo adds them on the Mac.

## NOT verified (say so; ask the user to test on their Mac)
1. **Output to screen 2** on a real second monitor (only the coordinate maths is tested). **Exit from it** (button, Esc, F11, double-click).
2. Real microphone + Whisper accuracy; Chrome speech on a real mic; reading aloud (`say`) and the echo guard.
3. OBS browser-source page inside OBS; phone remote over real Wi-Fi.
4. `tools/fetch_bible.py api` against API.Bible. Publisher licences, prices and who to contact (see LICENSING-AND-PRICING.md: figures are from web searches on 5 Oct 2026, confirm them).
5. **Stage 2 on a real Mac** (see `TESTING.md`, last section): the Media & slides window, pictures and video on the stage and on screen 2, video smoothness, OpenH264 build on Apple Silicon, `afplay` sound + mic muting, the file chooser and drag-and-drop, real `.pptx` files from PowerPoint/Keynote/Google Slides, odd phone videos.
6. Anything visual: layout was checked headlessly (nothing painted outside the window, books a vertical list) and in Chromium for the web page, never on a real Mac window.

## Rules for working on this (from the owner)
* **Our own code only.** Do not copy, paraphrase, or decompile code, files, icons, or names from ProPresenter, FreeShow or any other program. Build from behaviour described in our own words (`docs/LEGAL-AND-CLEAN-ROOM.md`). FreeShow is GPL-3.0: do not read or copy its source.
* Tests first for logic; keep logic in pure functions; draw in the UI.
* Be honest: say what you could not run. Never claim a licensed Bible (NIV, NKJV, AMP, ESV, NLT) is included.
* Safe defaults: web server off; never cut a live service; ambiguous quotes wait for a person.
* The owner is in the UK (use £), wants answers explained simply (like to a 9-year-old), with the reason ("why") for each thing, and a ranked list when asked for options.
* Output files go in `/mnt/user-data/outputs`; give a new zip named with the next version.

## Owner's naming and transcript wishes (already built; keep)
Folder = full date (`Sunday 4th October 2026`); files = date + `01h27m43s to 02h15m10s` + what it is; transcript starts with the day and start time to the second, every line has both time-into-service and clock time.

## v0.818.0 in one paragraph
Mic check: `crates/rf-asr/src/micreport.rs`, `examples/mic_report.rs`, `docs/MIC-CHECK.md` (also the ranked accuracy plan). Test with `cargo test -p rf-asr --no-default-features --test micreport` in the sandbox (no native Whisper there). Good next jobs: church/pastor names in the Whisper prompt; learn from operator corrections; in-app mic-check button; wire the service order.

## v0.817.0 (Web 0.4) in one paragraph
PWA: `web/src/sw.js`, `manifest.webmanifest`, `_headers`, `icons/` (from `web/make_icons.py`), `web/build.py` writes `web/dist/` (commit it). Tests `web/test/pwa.test.js` (Node) and `web/test/pwa_test.py` (real Chromium, needs `PLAYWRIGHT_BROWSERS_PATH`). Guide: `docs/DEPLOY-CLOUDFLARE.md`. Never verified on Cloudflare / a phone / iOS. Next suggested: wire the `rf-service` service order to the screen.

## v0.816.0 in one paragraph
Garbled quotations: `rf-bible/src/pieces.rs` (matcher), `rf-core` `pick_recent_pieces` + `pieces_detection` (parser), `quote_consumed_ms` in the app. 20+ commands: `rf-core/src/cmdtable.rs` (`payload_commands`, `translation_in`), arms in `rf-app/src/actions.rs`, `read_until`. Strong's: `rf-strongs` + `rf-app/src/strongs_ui.rs`. Not verified on a Mac: the Strong's window, real voice, Way 1 on the real tagged KJV.

## Open decisions for the owner
1. **H.264 patents** before selling widely: Via LA sign-up (first 100,000 units listed at US$0) or switch to Cisco's downloadable OpenH264 file (one line, `crates/rf-media/src/video.rs`). Details in `THIRD-PARTY.md`. Not legal advice.
2. **Licence audit of the older dependencies** (egui, cpal, whisper-rs...): not done; `cargo deny check licenses` is the quick way.
3. Song words: the app ships none. Churches need CCLI (UK) for copyrighted songs.

## Known gaps in Stage 2 (honest)
* The saved **service order** (`rf-service` model + runner) is still not connected to the screen: items are put on the stage by clicking in each tab.
* Video sound goes to the default output only; `afplay` cannot start part-way, so "restart" restarts sound from the start, and un-muting mid-video does not resume sound.
* Pictures and video are not sent to the OBS page; slides and countdowns are (as text).
* GIFs show their first frame only. WebP is refused (not in the build).
* Fragmented ("streaming style") MP4 files are refused with advice.
* 4K video will be slower: frames are turned into RGBA on the CPU (measured 58 fps at 1080p on one slow Linux core).

## Next: Stage 3 (accounts and money), then 4 Bible licences, 5 live extras, 6 installer/signing, 7 teams
See `docs/ROADMAP-50.md`. A good small job first: wire the `rf-service` Runner (service order, Clear / Logo / Black, undo) to the egui UI, since songs, slides, media and timers now exist as items it can hold (`ItemKind::Song / Media / Timer`).
