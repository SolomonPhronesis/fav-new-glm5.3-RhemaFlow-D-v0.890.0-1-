# RhemaFlow → Replace FreeShow and ProPresenter

**Baseline reviewed:** RhemaFlow-D v0.804.3 (Rust, egui/eframe 0.28.1, 4 crates, ~8,100 lines, 120 tests per your report)
**Written:** 30 Sep 2026
**Target:** a church can run a *full* Sunday service (worship + sermon + announcements) without opening FreeShow or ProPresenter.

> **Honest read of where you are.** RhemaFlow today is a very good *scripture-and-sermon-intelligence* tool: voice-triggered verses, quotation matching, a confirm queue, a recorder with a scripture index. It is **not yet a presentation suite**. The code has exactly three stage content kinds (`Blank`, `Scripture`, `Points`), one stage window, no songs, no slides, no media, no service order, no timers. That is not a criticism; it is the size of the job. About 85% of what ProPresenter and FreeShow do is still ahead of you, and that 85% is what a church checks before it switches.

> **Things I did not do:** I read the source and docs; I did not compile or run it (no Mac here). The README, TESTING.md and IDEAS-100.md inside the zip still say **v0.801.2**, and the "Follow-up 7" report you mentioned was not in the zip. Dates below are estimates, not promises (see assumptions).

---

## 1. Strategy: don't clone, out-flank

FreeShow is free and open source; ProPresenter is the incumbent with a huge feature list. You will lose a feature-count race against either. You win by being:

1. **Voice-first:** the operator's hands stay free. Neither competitor can do this. This is your moat, protect it.
2. **Sermon-aware:** recording + scripture index + live captions + sermon-notes priors, all offline.
3. **Boringly reliable:** fewer features, each one bulletproof on a base M1 MacBook.
4. **Easy to leave:** open file formats and export. Counter-intuitively this is what makes churches willing to come.

**Rule:** a feature enters the plan only if it (a) is table stakes for a Sunday service, or (b) strengthens the voice/sermon moat. Everything else goes on the kill list (section 6).

---

## 2. Gap matrix

Tier: **M** = must have to replace (table stakes), **S** = should have for v1.0, **C** = could, after v1.0.
"RhemaFlow now" is from reading v0.804.3.

| # | Capability | ProPresenter / FreeShow | RhemaFlow now | Tier | Phase |
|---|---|---|---|---|---|
| 1 | Service order / playlist of items | Core concept in both | None (single live stage) | M | 1 |
| 2 | Text slides (announcements, prayers, welcome) | Yes | No (only scripture + "points") | M | 1 |
| 3 | Themes / templates (fonts, colours, positions) | Yes | One fixed "Cathedral Dark" look | M | 1 |
| 4 | Panic keys: Clear / Logo / Black | Yes | Partial ("clear the screen") | M | 1 |
| 5 | Autosave, crash recovery, undo | Yes | No persistent service file | M | 1 |
| 6 | Song library + search | Yes | None | M | 2 |
| 7 | Song editor (verse/chorus/bridge auto-split, arrangements) | Yes | None | M | 2 |
| 8 | Import songs (OpenLyrics, ChordPro, text, PPTX, SongSelect) | Yes | None | M | 2 |
| 9 | CCLI / OneLicense usage log + report | Yes | None | M | 2 |
| 10 | Image / video backgrounds and loops | Yes | None | M | 3 |
| 11 | Countdown / pre-service timers, clock | Yes | None | M | 3 |
| 12 | Announcements loop | Yes | None | M | 3 |
| 13 | Stage / confidence monitor (current + next + clock + message) | Yes | None | M | 3 |
| 14 | Audio playback (walk-in music, stingers) | Yes | None (TTS only) | S | 3 |
| 15 | Multi-screen routing + hot-plug | Yes | One stage viewport | M | 4 |
| 16 | Transitions (fade/slide/none) | Yes | Fixed 350 ms fade | S | 4 |
| 17 | Lower-thirds / alpha output for streaming | Yes | None | S | 4 |
| 18 | NDI / Syphon output to OBS/vMix | Yes (NDI); Syphon on Mac | None | S | 4 |
| 19 | Live captions on screen | Not standard | ASR exists; no caption output | S (differentiator) | 4 |
| 20 | Remote control (phone/tablet web remote) | Yes | None | M | 5 |
| 21 | MIDI / OSC / HTTP API | Yes | None | S | 5 |
| 22 | Planning Center Services import | Yes | None | S | 5 |
| 23 | Signed `.app`, updater | Yes | `./run.sh` via Terminal | M | 6 |
| 24 | **Voice-triggered scripture + quotation matching** | No | **Yes, shipped** | M | done |
| 25 | **Sermon recording + scripture index + transcript** | No | **Yes, shipped** | M | done |
| 26 | Bible module: multiple translations, search | Yes | KJV + BBE, voice lookup; no text search | M | 2 and 4 |
| 27 | Stage motion backgrounds, 3D, plugins/scripting | Partial | No | C | after v1.0 |

---

## 3. Phased plan (steps + "done when" + target date)

**Assumptions:** solo builder, about 15–20 h/week, GLM 5.3 writing most code with you reviewing, Claude/you pushing back. If you can give 30+ h/week, pull each date in by about a third. If a real-Mac test finds a nasty bug, that phase slips; do not compress the test.

### Phase 0: Prove what you have (1–14 Oct 2026)

Steps:
- [ ] Run v0.804.3 through a **full 90-minute service** on the M1 with a real mic: unplug/replug the mic once, switch device once.
- [ ] Verify `audio.wav` duration equals wall-clock within 1 s, and every Scripture Index timestamp is within 1 s of when the verse hit the stage.
- [ ] Put the project under **git**, tag `v0.804.3`, add `CHANGELOG.md`.
- [ ] Fix stale docs (README/TESTING/IDEAS say 0.801.2): one version source, one script that fails the build if any doc disagrees.
- [ ] CI: `cargo fmt --check`, `clippy -D warnings`, `cargo test` on GitHub Actions `macos-14` (Apple Silicon) for every commit.
- [ ] Build the **record/replay harness**: feed a saved `audio.wav` + `events.jsonl` back through the pipeline and assert the same stagings (item 13 below).
- [ ] Write ADR-001: **render stack decision** (item 4 below) before any slide work starts.

**Done when:** two consecutive full services with no dropped-mic incident, replay test green in CI, render-stack ADR written.

### Phase 1: Service foundation (15 Oct – 18 Nov 2026)

Steps:
- [ ] Data model: `Library → Presentation → Slide`, `Service → Item` (scripture, slide, song, media, timer), with `schema_version` in every file.
- [ ] Persistence: atomic writes (temp file + fsync + rename), autosave every change, rolling backups, relaunch restores to the same slide.
- [ ] Operator UI: service-order panel, slide grid, click/keyboard to show, Next/Prev walks the order.
- [ ] Refactor output into an `Output` abstraction (audience, stage monitor, recorder, later NDI) so later phases don't touch the core.
- [ ] Text-slide editor + **Theme v1** (font, size, colour, alignment, background colour, safe-area margins).
- [ ] Scripture becomes a first-class service item (voice-staged verses can be *added to the order* as well as shown).
- [ ] Panic keys: **Clear, Logo, Black**, plus Undo for the last 10 live actions.
- [ ] Edit mode vs Show mode (locked) so a volunteer can't edit slides live.

**Done when:** you build a 10-item service (welcome, placeholder song slides, scripture, announcements) and run it from the keyboard only; `kill -9` mid-service and relaunch lands you back on the same slide in under 10 s.

### Phase 2: Songs and lyrics (19 Nov 2026 – 6 Jan 2027, includes Christmas buffer)

Steps:
- [ ] Song model: title, author, CCLI number, sections (verse/chorus/bridge/tag), arrangement order, key, optional chords.
- [ ] Editor with paste-lyrics → auto-split into sections and slides (configurable lines per slide).
- [ ] Fast search (title, first line, any lyric fragment) in under 100 ms on 5,000 songs.
- [ ] Importers: OpenLyrics, ChordPro, plain text, SongSelect text/USR, PPTX text-only. Bulk-import with a preview and duplicate detection.
- [ ] CCLI/OneLicense usage log per service + CSV report export.
- [ ] Bible text search (not only voice): type "love your enemies" and find the verse.
- [ ] Add a third public-domain translation (ASV or WEB) using the existing `from_translation()` path.

**Done when:** you import a real church's library (target 500+ songs), find any song in under a second, and run a complete worship set including a repeated chorus.

### Phase 3: Media and timing (7 Jan – 10 Feb 2027)

Steps:
- [ ] Image and video backgrounds (hardware-decoded via AVFoundation; loops; thumbnails cached).
- [ ] **Missing-media detection and relinking** (files moved/deleted is the classic Sunday-morning failure).
- [ ] Countdown timer, pre-service countdown, wall clock, "service start" timer.
- [ ] Announcements loop (auto-advance slides before service).
- [ ] Stage/confidence monitor v1: current slide, next slide, clock, timer, operator message.
- [ ] Audio player for walk-in music and stingers (separate from TTS and from ASR input).
- [ ] **Preflight "Sunday readiness" screen** (item 9 below).

**Done when:** a 90-minute service with video backgrounds, a countdown, loops, and a stage monitor runs with no dropped frames on a base M1 and ASR still on.

> **Milestone: "Sunday-capable" ≈ 10 Feb 2027.** This is the earliest point a church could realistically run a service without ProPresenter/FreeShow. Everything after this is polish and reach.

### Phase 4: Multi-output and polish (11 Feb – 17 Mar 2027)

Steps:
- [ ] Output routing per display, hot-plug handling, remembered layout per display set.
- [ ] Transitions (fade/slide/none), per-theme.
- [ ] Theme/template editor; "Looks" (saved combinations of theme + layers).
- [ ] Lower-thirds with alpha; NDI and Syphon output for OBS/vMix.
- [ ] **Live captions output** from your own ASR (see item 11 below).
- [ ] Dual-language / two-translation slide.

**Done when:** two projectors plus an NDI feed run simultaneously, unplugging one display mid-service does not blank or crash the others.

### Phase 5: Control and integration (18 Mar – 21 Apr 2027)

Steps:
- [ ] One stable local control API (HTTP + WebSocket + OSC), authenticated.
- [ ] Phone/tablet web remote served from the app (pairing PIN).
- [ ] MIDI input mapping; Bitfocus Companion module (this gives you Stream Deck for free).
- [ ] Planning Center Services import (plan → service order).
- [ ] Sermon-notes import to prime the recogniser (item 12 below).

**Done when:** an operator can run a service from an iPad on the church Wi-Fi and a Stream Deck, and a stranger on the same Wi-Fi cannot.

### Phase 6: Ship-grade (22 Apr – 26 May 2027)

Steps:
- [ ] Developer ID signing + notarization, drag-install `.app`/DMG with its own microphone permission (item 19).
- [ ] Updater with an update **freeze window** (no updates Fri–Sun) and one-click rollback.
- [ ] First-run onboarding tour; accessibility pass (VoiceOver, contrast, reduced motion).
- [ ] Support-bundle export; docs site and 10 short training videos for volunteers.
- [ ] Security review (remote API, file handling) and dependency audit (`cargo audit`, `cargo deny`).

**Done when:** a non-technical volunteer installs from a download, never opens Terminal, and completes a service after a 20-minute training.

### Phase 7: Pilot and v1.0 (27 May – 30 Jun 2027)

Steps:
- [ ] 3 pilot churches: 4 Sundays shadowing their current software, then 4 Sundays as primary with the old tool on standby.
- [ ] Bug burn-down from pilot logs; fix every "I was afraid to click X" comment.
- [ ] Freeze, tag **v1.0**, publish migration guides "from ProPresenter" and "from FreeShow".

**Done when:** all 3 pilots complete 4 primary Sundays with zero "fell back to the old software" events.

### Timeline at a glance

| Phase | Window | Outcome |
|---|---|---|
| 0 Prove | 1–14 Oct 2026 | Verified, in CI, render stack decided |
| 1 Foundation | 15 Oct – 18 Nov | Service order, slides, themes, panic keys |
| 2 Songs | 19 Nov – 6 Jan | Library, import, CCLI |
| 3 Media | 7 Jan – 10 Feb | Video, timers, stage monitor, **Sunday-capable** |
| 4 Outputs | 11 Feb – 17 Mar | Multi-screen, NDI, captions |
| 5 Control | 18 Mar – 21 Apr | API, phone remote, Companion, Planning Center |
| 6 Ship | 22 Apr – 26 May | Signed app, updater, docs |
| 7 Pilot / v1.0 | 27 May – 30 Jun 2027 | Three churches live, v1.0 |

---

## 4. Top 20 things you probably haven't thought about

Written the way someone who has watched many products die in year two would put it. I deliberately skipped anything already in `docs/IDEAS-100.md`. Effort: **S** ≤ 2 days, **M** ≈ 1 week, **L** multi-week.

### 1. The stage must survive the operator app crashing
**Why:** today the congregation window is an in-process egui viewport (`show_viewport_immediate` in `app.rs`). A panic anywhere (ASR, UI, recorder) blanks the screen mid-sermon. ProPresenter's worst reputation moments are exactly this.
**Do:** split into a small **output process** (renders only what it's told) and an operator process; a supervisor restarts whichever dies, and the output keeps showing its last frame. **Phase 1, L.**

### 2. macOS will actively fight you during a service
**Why:** the Mac can sleep, show a Messages/Calendar banner over the verse, start the screensaver, App-Nap your audio thread, or shuffle windows between Spaces when a projector is plugged in.
**Do:** hold a power assertion while live, respect/guide Focus mode, pin the output window above the menu bar/Dock, disable App Nap for the process, handle display hot-plug explicitly. Write a "Mac setup for church" checklist into preflight. **Phase 3, M.**

### 3. Your read-aloud voice can trigger your own recogniser
**Why:** TTS plays through the PA, the preacher's mic hears it, ASR transcribes it, and it stages another verse, a self-inflicted loop. Related: capture-device clock drift makes a 90-minute recording slowly slide against wall-clock time.
**Do:** gate ASR (or mark it "synthetic") while TTS is speaking; measure drift by comparing sample count to wall-clock over a long run and resample-correct it; log device sample-rate changes. **Phase 0–1, M.**

### 4. egui 0.28 may not be the right foundation for a presentation suite
**Why:** slides need rich text, reliable wrapping, complex scripts (Arabic/Hindi shaping, RTL, CJK), emoji, colour management, and smooth 1080p/4K video. egui is great for operator tools and weak at several of these. You're also pinned to an old version (`=0.28.1`).
**Do:** decide *now*, in an ADR, whether the **audience renderer** is egui, a custom `wgpu` renderer with `cosmic-text`, or a web-view. A common good answer: egui for the operator UI, a dedicated wgpu output renderer. Changing this after slides exist costs 5x. **Phase 0, M.**

### 5. Bible versification and canon are a real swamp
**Why:** you already hit it with BBE's +2 verses. Psalms number differently in Hebrew vs Septuagint-based Bibles, some verses exist in one translation and not another, Catholic/Orthodox canons include books your parser doesn't know, and 3 John 14/15 and Malachi chapter breaks differ.
**Do:** a per-translation **versification map** layer; keep KJV as the structural key but map every translation through it; add deuterocanonical books behind a canon setting; test with a small set of known-tricky references per translation. **Phase 2, M–L.**

### 6. The licensing matrix is a product feature, not paperwork
**Why:** songs need CCLI/OneLicense reporting; NIV/ESV/etc. have license terms; your bundled **ASR models, fonts, TTS voices and any imported content** each have their own terms (macOS voices can't be redistributed; Parakeet has attribution requirements). AI-written code can also pull in dependencies with incompatible licences. FreeShow is open source under a copyleft licence (verify current terms): never copy its code into an MIT project; clean-room only.
**Do:** a `LICENSES.md` matrix, `cargo deny` in CI, in-app "About → Licences", and a rule that nothing is bundled without its licence recorded. **Phase 0 start, ongoing, S–M.**

### 7. Data durability and open formats decide whether churches trust you
**Why:** a church's song library is years of work. One corrupted save is a one-star review forever, and lock-in kills adoption.
**Do:** `schema_version` + migrations on every file, atomic writes, automatic rolling backups, a documented open format (JSON/zip) plus exports (OpenLyrics, PPTX, PDF), and a "restore from backup" button. **Phase 1, M.**

### 8. Migration is your real adoption funnel
**Why:** nobody switches if it means retyping 2,000 songs. Importers are not a nice-to-have; they *are* the go-to-market.
**Do:** prioritise OpenLyrics, ChordPro, text, SongSelect, PPTX-as-images, and media-folder import with relinking. Publish an honest "what transfers / what doesn't" page per competitor. Build an **export** too; people migrate to tools they can leave. **Phase 2–5, L.**

### 9. A "Sunday readiness" preflight, not just a launch screen
**Why:** most Sunday failures are avoidable: missing media, wrong display, mic muted, disk nearly full (recordings are ~115 MB/hour), laptop on battery, model not loaded, wrong service file open.
**Do:** a green/amber/red checklist run on demand and automatically 30 minutes before the scheduled service; it blocks "Go Live" on red items. **Phase 3, M.**

### 10. A proper operator-safety model
**Why:** volunteers rotate. A mis-click live is the most common incident in this category.
**Do:** Edit/Show mode, confirmation only for destructive actions, visible "what's live" state, Undo for live actions, per-role permissions (operator vs editor vs admin), and a documented "panic" gesture everyone can find. **Phase 1, M.**

### 11. Live captions: a differentiator hiding in your existing ASR
**Why:** you already transcribe the sermon. Putting rolling captions on a secondary output or the stream serves deaf/hard-of-hearing members, is increasingly expected, and costs you very little. Add offline translation later for multilingual congregations.
**Do:** caption output layer with line-break rules, latency target, profanity/garbage suppression, operator kill-switch, and a clear "captions are auto-generated" notice. **Phase 4, M.**

### 12. Sermon-aware priors: use the pastor's notes to beat the recogniser
**Why:** pastors usually know their passages in advance. If you load the notes/manuscript, you can **bias both reference parsing and quotation matching toward those verses**. That should lift accuracy more than any model upgrade, and no competitor can do it.
**Do:** import sermon notes (txt/docx/pdf), extract references and quoted phrases, feed them as priors to the matcher and the ASR prompt, show "expected" verses as one-tap cards. **Phase 5, M.**

### 13. Deterministic record/replay + soak + chaos testing
**Why:** unit tests (120 is good) don't catch "dies at minute 74." You already record `audio.wav` + `events.jsonl`, which is a golden regression test waiting to be used.
**Do:** replay saved services through the pipeline in CI and diff stagings; a 4-hour soak test watching memory/CPU/handle growth; chaos tests: unplug mic, sleep/wake, unplug display, fill disk, kill process; property-based and fuzz tests on the parser. **Phase 0 start, L (ongoing).**

### 14. Performance and thermal budgets on the *worst* Mac a church owns
**Why:** churches buy old hardware. A fanless 8 GB MacBook Air running ASR + video + recording for two hours will throttle.
**Do:** write budgets (e.g., idle CPU, p95 speech-to-stage latency, memory ceiling, no dropped frames) and test on a base M1 Air 8 GB with a thermal log. Publish the numbers in the README. If you can't meet them, cut features, not the budget. **Phase 0 + every phase, M.**

### 15. Observability and a one-click support bundle
**Why:** at 8:40 on Sunday you can't debug from "it froze." Without logs you can't reproduce, and you can't help a church remotely.
**Do:** structured rotating local logs, a crash handler that writes a report, and a **Support Bundle** button that zips logs, config, system info and (optionally, never by default) a redacted event timeline. No audio, no telemetry. **Phase 1, S–M.**

### 16. Security and privacy are bigger than "offline"
**Why:** the moment you add a web remote, anyone on church Wi-Fi could drive the screen. Sermon recordings can contain pastoral details and names, which is personal data (you're UK-based, so UK GDPR applies to UK churches). Children's photos on screens raise safeguarding rules.
**Do:** pairing PIN + token auth + optional TLS for the remote; bind to LAN only with an explicit toggle; optional encryption-at-rest for recordings; retention settings and "delete service" that truly deletes; a short privacy notice template churches can publish. **Phase 5–6, M–L.**

### 17. One stable control API instead of many bespoke integrations
**Why:** every integration you hand-build (Stream Deck, OBS, lighting, ATEM, MIDI) is forever maintenance.
**Do:** expose a versioned local HTTP/WebSocket/OSC API, then let Bitfocus Companion and community modules do the rest. Document it and keep it backward-compatible. **Phase 5, M.**

### 18. Projector and display physics
**Why:** "pure black" on a projector is actually grey light; LED walls and 4:3 or ultrawide screens behave differently; overscan crops edges; colour profiles shift gold badges; multi-projector edge blending exists.
**Do:** resolution-independent layout, configurable safe-area, per-output aspect ratio, contrast/black-level themes (a "projector" vs "LED" preset), test with real projector and TV. The stage monitor has different rules again (big, high-contrast, glanceable). **Phase 3–4, M.**

### 19. Distribution identity: who owns the mic permission?
**Why:** `./run.sh` launches via Terminal, so macOS grants microphone permission to *Terminal*, not RhemaFlow. A signed, notarized `.app` gets its own entry and its own failure modes (TCC resets after updates, quarantine flags, Gatekeeper). Auto-updates that land on Saturday night are how churches lose Sunday.
**Do:** sign + notarize early (an Apple Developer account, about £79/year, verify current price); test permission behaviour on a clean Mac; build an updater with a **freeze window** (no updates Fri–Sun), staged rollout, and one-click rollback. **Phase 6 (start a spike in Phase 2), M.**

### 20. Sustainability: who answers the phone on Sunday morning?
**Why:** you are a solo producer with two animated series plus this. A church depending on RhemaFlow needs a support path, a bus-factor plan, clear liability wording (a wrong verse on screen is embarrassing, not dangerous, but say so), and a business model (paid, donation, open-core, or church-tier) that funds the support.
**Do:** decide the model before pilots; write a support SLA you can actually keep (e.g., weekday response, "Sunday hotline" for pilot churches only); publish docs and a troubleshooting flowchart; recruit 1–2 technical volunteers from pilot churches as backup. **Decide by Phase 4, S but vital.**

---

## 5. Immediate next actions (this week)

1. Run the **90-minute real-Mac service test** of v0.804.3 and record the results.
2. Put the project under git and add CI (macos-14 runner).
3. Sync all docs to 0.804.3 and add the version-audit script.
4. Write **ADR-001: render stack**, the most expensive decision still open.
5. Pick one real church's song library for Phase 2 testing now, and start collecting its formats.

## 6. Kill list (explicitly *not* building before v1.0)

- Windows/Linux builds (plan for them in the architecture, ship Mac first).
- A full PowerPoint renderer (import text/images; don't emulate PowerPoint).
- 3D transitions, particle backgrounds, a built-in audio mixer/DAW, lighting desk features.
- Cloud accounts/sync as a requirement (offline-first stays; user-owned folder sync is enough).
- A plugin/scripting system (the control API covers 90% of the need).

## 7. Risks to watch

| Risk | Likelihood | Mitigation |
|---|---|---|
| Real-Mac issues not yet seen (mic, Metal, Parakeet) | High | Phase 0 full-service tests before any new feature |
| Render-stack change late in the project | Medium | ADR-001 in Phase 0 |
| Scope creep to "match ProPresenter" | High | Section 1 rule + kill list |
| Solo bandwidth (three projects) | High | Fixed weekly hours; cut Phase 4/5 features before slipping the pilot |
| AI-generated code quality drift | Medium | CI gates, replay tests, keep the self-check prompt for GLM |
| Licensing surprise (songs, translations, bundled assets) | Medium | Licence matrix + `cargo deny` from week 1 |
