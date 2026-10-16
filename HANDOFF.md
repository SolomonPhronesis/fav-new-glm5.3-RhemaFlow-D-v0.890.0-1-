# HANDOFF: read this first (RhemaFlow-D v0.890.0)

**v0.890.0:** 856's suggestive scriptures back as SETTINGS, the 91/92
near-tie rule, and a measured speech layer. The operator liked 856's
suggestion cards; 889 had quieted them. v0.890 (built on the 856 base)
ships the whole spectrum as settings in NEEDS YOUR REVIEW > "Suggestion
gates": cards min words (3/4/5, default 3) + min % (50-95, default 50 =
856), auto-show min words (default 5) + min % (75-90, default 90), the
near-tie rule ("two verses over the bar" -> show winner, default, or ask
me), and the QuoteHold stability hold (default off). Ports from 889 kept:
decide_raise (a finished stronger line upgrades past a weaker card) and
strip_filler_tail ("So./Amen." tails scored both ways). Speech layer:
draft tiny.en stays live; final pass = large-v3-turbo greedy +
`rf-core::repair::repair_from_match` KJV post-correction — measured
98.4% verse accuracy end-to-end vs the operator's 58-85% complaint
(12-way ranked bench, charts and trade-offs: `docs/STT-V0890.md`,
`docs/stt-accuracy-rank-v0890.png`, `docs/stt-before-after-v0890.png`,
raw `docs/stt-bench-v0890.json`). All workspace tests green on Linux.

---

**v0.856.0 (previous):** the Proverbs 19:21 false show, root-caused and fixed by a
whole-Bible grid. The four ordinary words "there are many devices" (the
verse's own opening, said about phones) scored 89% and the v0.855 auto-approve
bars (60% single / 75% clear-winner) put the verse straight on the stage — so
the answer to "is it 3 words min or accuracy" is: **the accuracy bar, not the
3-word tier** (3-word probes on 19:21 were already silent). The fix was CHOSEN
by measurement: every 4/5/6-word window of the whole KJV (1,698,315 unique
utterances a preacher can say in passing) plus 33,189 real verse reads through
13 configs (`cargo run --release -p rf-core --example biblegrid`; grid, rank,
chart and trade-offs in `docs/GRID-PROV19-21-v0856.md`, chart
`docs/accuracy-v0856.png`, raw JSON `docs/grid-v0856.json`). Shipped v0.855
auto-stages 653 per 1,000 of those windows; the winner **W=5 / A=90** (rank 1)
auto-stages 57.25 per 1,000 (−91.2%) with identification unchanged at 98.2%
and 95.7% of true reads still auto-staging. Code: `rf-core/parser.rs` gains
`AUTO_MIN_WORDS`/`AUTO_STAGE_PCT` + `Detection.words`; `rf-app/state.rs::
should_auto_approve` enforces both bars (short/below-bar quotations still card
— the operator decides; a wrong verse can no longer take the stage alone).
Repro/regression: `cargo run -p rf-core --example prov1921` (now reads the
live constants), plus three new rf-app tests. Measured in the Linux sandbox:
rf-core 167, rf-app 332, rf-bible 44, rf-service/rf-media/rf-strongs/rf-studio
green; nine-group textbench 100.0 → 100.0 (docs/textbench-after-v0856.json).
**NOT seen on a Mac:** everything — build, run `cargo test --workspace`, then
with a mic: say "there are many devices" mid-sermon and confirm the verse only
CARDS (never takes the stage), then read Proverbs 19:21 fully and confirm it
still shows by itself.

**v0.855.0:** BOTH operator screenshots fixed, and this time the fixes were measured before AND after on the exact shipped code. BEFORE (the pristine v0.854.0 zip): the half-line "turn the heart of the father to the children" carded Malachi 4:6 while Luke 1:17 was on stage (Luke wasn't even a candidate — singular/plural), and "also before him" auto-staged Acts 23:33 at 79%. The fixes: **named-verse lock** in `parse_full` (a fragment still explained by the verse on stage — `Bible::span_fit_pct` — can never let a look-alike card or auto-stage; preacher moving on releases it, verified by the Malachi negative control), **anchored span alignment** (`Bible::span_align`), **singular/plural folded needles** (`turn the heart` reaches `turn the hearts`), **OT/NT name aliases + -eth verb folding** (Elijah/Elias, dwelleth/dwells), **tail-fragment gate** (3–4 word claims past the verse's mouth need opening proximity or ≥35% verse coverage; fame can no longer lift a sub-60 match over the auto-approve bar), **sound-tag filter** (`rf-core/src/soundtag.rs` — "[MUSIC]" removed from parsing, "Romans (chapter 8)" untouched), **verse-anchored repair** (`rf-core/src/repair.rs` — on-screen line rebuilt from the verse while it's on stage; "to the gods" → "to the wisdom of the just"; logs keep raw ASR; interpretation strip reports the repair), **RF_PREFILTER=0** A/B switch for the high-pass. Measured: nine-group text-layer benchmark **87.7 → 100.0 composite** (BEFORE/AFTER JSONs in `docs/`, `docs/TEXT-EVAL-NINE.md` has the method and its limits); five repair cases WER 0.136 → 0.000, never worse; the full v0.853 refbench battery still 100%; ~940 tests green in the Linux sandbox (rf-core 167, rf-bible 44, rf-app 330, all other crates green). Repro tool: `cargo run -p rf-core --example repro855`. **NOT seen on a Mac:** everything — build, run `cargo test --workspace`, then with a mic: (1) stage Luke 1:17, read the verse aloud, confirm Malachi never appears; (2) let a worship band play under the reading and confirm no "[MUSIC]"-driven verse; (3) read a verse with a cough/cough drop garble and watch the line repair; (4) `./compare.sh` before/after for WER on a real recording.

**v0.854.0:** the LISTENING side of the accuracy plan (v0.853.0 fixed the understanding side — parsing/ranking/auto-show; see its entry below). Three changes, all unit-tested: (1) **streaming hotwords** — the app now pushes the 66 book names + learned words to the R2T2 sidecar with a new `RF_CONTEXT <words>` text frame at session open and via `StreamClient::set_context()` any time (`rf-asr/src/stream_client.rs`, `rf-asr/src/engines.rs::stream_context`, sidecar `r2t2_server.py`); R2T2 reads the `context` slot natively so "Habakkuk"-class book names survive streaming. (2) **rescue decode** — the whisper.cpp shim's accuracy path (beam ≥ 2) now uses the standard temperature fallback 0.2→0.6 behind the compression-ratio/logprob gates for music/noise-damaged windows; the fast interim path stays deterministic (`rf-asr/native/rf_whisper_shim.cpp`). (3) **75 Hz rumble high-pass** before the AGC so HVAC/traffic/thumps stop eating the gain budget (`rf-asr/src/hpf.rs`, mic chain now HPF → AGC → VAD → engines; recorder tap still keeps raw). **Compiled and unit-tested in the Linux sandbox (716 Rust + 13 Python tests green at the time). NOT seen on a Mac:** WER effect of (1)+(2), and the audible behaviour of (3) — after build run `cargo test --workspace`, then with the sidecar up say a Habakkuk / Second Chronicles reference and check the live words; re-measure with `./compare.sh`.

**v0.853.0:** the five operator-reported accuracy failures fixed and measured (harness: `cargo run --release -p rf-core --example refbench -- out.json`, picture `docs/accuracy-v0853.png`, full story in CHANGELOG). Code: `rf-core/src/parser.rs` (self-correction split, book-then-numbers, nav-vs-quote precedence, whole-utterance F1 gate on the unique-tier card), `rf-bible/src/lib.rs` (3-word ranked tier, lead-in 6→18, per-candidate own-start F1 scoring, `WELL_KNOWN` fame tie-break, `quote_pct`), `rf-core/src/cmdtable.rs` + `rf-app/src/{actions,app,state,ui}.rs` (nav commands, auto-show rule, suggestion card display). **Compiled and unit-tested in a Linux sandbox (rf-core+rf-bible 193, rf-app 330 tests green). NOT seen on a Mac:** the live mic path — after build run `cargo test -p rf-app`, then in the real service say "Second Chronicles no Second Corinthians chapter 5 verse 17", "Next verse", "for God so" with auto-show on and off, and Psalms 23 after "We go to Psalms."

**v0.845.0:** menu flash (`app.rs::flash_menu` / `draw_menu_flash`, `macmenu::describe`), real stored file names + rename of old hash names + library health check (`rf-service/src/library.rs`), marker-pen reading line (`stage.rs::draw_doc`). `rf-app` edits are uncompiled: build and run `cargo test -p rf-app` first.

**v0.844.0:** (1) `run.sh` disk check now needs 2 GB (was 6 GB; override with `RF_NEED_MB=...`). (2) MEDIA bar SELECTION: new `rf-app/src/selection.rs` (pure rules + tests); `present.rs` draws a teal tint, border and tick on selected files, supports click / Cmd+click / Shift+click / drag-a-box / Cmd+A / Esc / arrow keys (Shift extends) and drags the whole selection onto a folder. Plain click still shows the file on screen, as before; Cmd+click, Shift+click and the box never put anything on screen. (3) Cmd+C / V / X / A now work when NO text box is active: the native Mac menu owns those keys, so `app.rs::edit_outside_text` gets the menu event and calls `clip.rs::copy_anything` / `paste_anything` (files on the clipboard through `osascript -l JavaScript`, then a file path, then a song's words). Delete / Backspace remove the selection, but only on a second press within 5 s, and never while locked. Also fixed: plain-letter shortcuts (C = clear, X = dismiss) no longer fire when Cmd is held. **Compiled and unit-tested in a Linux sandbox (Rust 1.91): 322 app tests green. NOT seen on a Mac:** the `osascript` file copy / paste, the drag box with a real mouse, and the look of the teal selection. Check those first.


**v0.843.0:** new crate `crates/rf-studio` (20 pure modules, see its `lib.rs` table and `docs/STUDIO.md`), glue in `rf-app/src/studio.rs` (state, macro/alert/shortcut runtime, import, templates, Bible versions) and `studio_ui.rs` (Tools > Studio tabs, stage-monitor painter, alert bars, QR). Hooks: `Extras.studio`, `Overlays.alerts`, `x_tick` calls `studio_tick` + `studio_monitor`, `handle_parse` calls `studio_voice`, `Present.newly_shown` feeds usage counts. PDF Find + page strip in `docview.rs`/`present.rs::doc_ui`, search rules in `rf-media/src/docsearch.rs`. **Compiled and tested** in the sandbox (`tools/sandbox-check/setup.sh`, Rust 1.75, `--ignore-rust-version`): 329 app + 179 rf-studio tests green. **Not seen on a Mac**: check the Studio tab, alert bars on screen 2, the stage-monitor window, QR scan from a phone, PDF Find/thumbnails (needs `pdftotext`). Engine-only modules (no UI yet): fit, transition, cues, playlist, mediaset, staging, bilingual, layout editor.

**v0.842.0:** reading line fix + options in `rf-media/src/document.rs::detect_lines` (merged-paragraph splitting at light interior valleys, edge trimming, quarter-line speck reference, absolute floor; 3 new tests) + `docview.rs` (`DocState::{guide_lines,guide_color}`, `art()` spans N lines) + `stage.rs` (`DocArt::guide_color`, `DOC_GUIDE_COLORS`) + `present.rs` (settings `doc_guide_lines`/`doc_guide_color`, controls in `doc_ui`, `clipboard_text()` using `arboard`, "Paste from clipboard" in the paste form, "Paste into words" in the song editor). **Made without compiling anywhere** (operator asked for speed; the M1 build was left to them): the new `detect_lines` tests were traced by hand against the algorithm, but `cargo test` / `cargo build` have NOT run on this tree. First things to try on the Mac: open a document, toggle the Reading line, step with the arrows and try 1/2/3 lines and the six colours; in Songs try Paste from clipboard and Paste into words.

**v0.841.0:** `order.rs` (undo/redo history, notes, minutes, `summarize`, `next_up`, `item_clock`, `dock_tab_labels`), `clip.rs` (clipboard actions), `Item::{notes,minutes}` + `Runner::{rename_item,set_item_notes,set_item_minutes,restore_items}`, per-document zoom in `present.rs`. New probe `tools/sandbox-check/v0841_probe.rs`. Not seen on a Mac. Remaining ideas: second-window stage preview, phone remote QR, multi-language verses, PDF thumbnails/search, status bar.

**v0.831.0 — COMPLETE:** (1) Strong's and Scripture history are no longer floating windows: they are tabs in the right dock (`RightTab::{Order,Review,Strongs,History,Index}` in `ui_queue`; `StrongsState::body`, `ui_history_body`). Old triggers (button, key, voice) still work: they switch the dock tab. (2) Accounts: `rf-app/src/account.rs` (Supabase sign-in through system `curl`, subscription -> `license::Ticket`, ticket saved to `~/.rhemaflow/ticket.json`), header chip + "Your account" window (`ui.rs`: `account_badge`, `tick_account`, `ui_account_window`), server files in `supabase/` (schema.sql, create-checkout, stripe-webhook). Set `RF_SUPABASE_URL` and `RF_SUPABASE_ANON_KEY` when building or running; NOT tested against a live Supabase/Stripe project. (3) The approved mock-up is now IN the app: **A/B/C layout switch** (header, top-right; `LayoutMode` in state.rs, remembered in settings.json — A Operator console = default, B Dock-first = transcript hidden + wider dock, C Focus/simple = big preview, transcript right, no dock/media), **blue/green transcript panel** (`ui_transcript`; green edge = scripture, blue = command, `LineKind` set in `handle_parse`), **nine colour swatches under the preview** (`center_preview`, same presets as the Colours menu, readable text included, remembered), **Index de-duplication** (index + stats live only in the Index tab; middle keeps Planned + As-they-speak), and a **preview clamp fix** (at 1100x700 the preview ran into the Scripture panel; now capped by the visible area — `ux_probe` passes at all four sizes). Verified in the Linux sandbox with Rust 1.99 + the eframe stub: **196 app tests pass** (incl. 6 new: 2 unit + 4 headless probes in `tools/sandbox-check/v0831_probe.rs`), every other crate green. NOT seen on a real Mac window; NOT tested with a real mic or a live Supabase/Stripe. Policy: nothing is gated mid-service; no subscription = free Bible display.

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


## v0.835.0 handoff
New files in rf-app: lyrics, overlay, announce, themes, fsutil, runsheet, autoplan, clips, readalong, translate, handout, recovery, lock, backup, access, controllers, screens, scrub (each pure and tested), plus `extras.rs` (state + per-frame work + actions) and `extras_ui.rs` (Tools window). rf-media: `audiocut.rs`, `thumb.rs`. New actions go through `run_named` -> `extras::is_x_action` -> `x.queue` -> `x_action` next frame. Web: `/operator`, `/actions`. Unverified on a Mac: afplay sound after jump, MIDI hardware, extra screens, translation services, Stream Deck. Next: look at the Tools window on a real screen and tidy the layout.
