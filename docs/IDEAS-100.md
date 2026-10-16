# RhemaFlow — 100+ Ideas Backlog (v0.801.2)

Written as a senior engineer + senior UX designer would: every idea has a
**why** (the value it creates) and a **status** (what actually shipped in
v0.801.2 vs what comes next). The roadmap picks from here, not from hype.

Status legend: **[SHIPPED]** in v0.801.2 · **[NEXT]** v0.9 candidates · **[LATER]** v1.0+/needs research or licensing.

---

## A. Voice commands (the operator's hands stay free)

1. **Bare-word commands** — "next", "previous", "again", "read", "clear" work as single spoken words. *Why: the #1 demo failure was "next" not advancing; preachers speak in bursts, not sentences.* **[SHIPPED]**
2. **Fuzzy command matching** — tolerates classic whisper mishears ("text"→"next", "previus"→"previous"). *Why: ASR noise must never strand the operator mid-service.* **[SHIPPED]**
3. **"Read it" / "stop reading"** — TTS read-aloud control by voice. *Why: the congregation hears the verse without the preacher re-reading it.* **[SHIPPED]**
4. **"Last scripture" / "again"** — re-stage the most recent passage. *Why: ("turn back to that verse again") happens every sermon.* **[SHIPPED]**
5. **"Zoom in / out / reset"** — voice-controlled stage text size. *Why: back rows squinting is a pastor-visible problem; fixing it instantly is visible value.* **[SHIPPED]**
6. **"Full screen" / "normal screen"** — voice stage control. *Why: one less reason to touch the laptop during worship.* **[SHIPPED]**
7. **Scripture-vs-command guard** — "next is John 3" stages John 3 instead of advancing. *Why: commands must never eat references (safety-by-design).* **[SHIPPED]**
8. **Voice can't stop listening/recording** — lifecycle controls are operator-only. *Why: a misheard "stop recording" destroying a service recording is unacceptable; this is a hard safety rule.* **[SHIPPED]**
9. **Command cheat-sheet in-app** (H) — every command discoverable. *Why: undiscoverable commands are unused commands.* **[SHIPPED]**
10. **Multi-command utterances** — "read it" + reference in one breath works ("…read John 3:16"). *Why: natural speech combines intents.* **[SHIPPED — read+ref combination]**
11. Custom wake-word ("Rhema…") before commands. *Why: cuts false triggers in loud rooms.* **[NEXT]**
12. Verse "bookmark" commands ("mark this verse", "go to mark 2"). *Why: preachers jump back to earlier points.* **[NEXT]**
13. Multi-step macro ("open the sermon in John") — sequence of stagings. *Why: service prep automation.* **[NEXT]**
14. Confidence-gated commands — only auto-run commands above threshold, else queue. *Why: acting on a misheard command is worse than asking.* **[NEXT]**
15. "Show it in ASV/NIV" — translation switch bound to a reference. *Why: bilingual congregations.* **[NEXT — needs licensed/API text]**
16. Voice notes ("remember this for later") → sermon notes file. *Why: captures mid-sermon inspiration without paper.* **[NEXT]**
17. Dual-language commands (Spanish/Portuguese/French command sets). *Why: global churches.* **[LATER]**
18. Whisper grammar constraints (token-level grammar biasing to reference grammar). *Why: could push command WER below 1%.* **[NEXT]**
19. Push-to-talk foot-pedal support (USB HID). *Why: hands-free without false triggers; pro AV standard.* **[NEXT]**
20. Voice activity "confidence ring" on the operator screen. *Why: operator knows when the system is listening vs decaying.* **[LATER]**

## B. Recognition accuracy (in code, measurable)

21. **Whisper beam search (5 beams)** instead of greedy best-of-1. *Why: fewer word errors on short commands; ~same latency on M1.* **[SHIPPED]**
22. **Domain initial-prompt biasing** — pulpit vocabulary + all book names + command grammar injected as the decoder prompt. *Why: the cheapest big accuracy win that exists offline; biases toward "Habakkuk", "verse", "next".* **[SHIPPED]**
23. **Deterministic decode** — temperature 0.0, temp-inc 0.0. *Why: the same sentence must always produce the same result; tests and services both depend on determinism.* **[SHIPPED]**
24. **Non-speech token suppression** — fewer "[BLANK_AUDIO]" artifacts. *Why: artifacts were parsed as noise utterances.* **[SHIPPED]**
25. **English forced** (`language=en`) — already present, kept. *Why: removes language-detection errors.* **[SHIPPED earlier]**
26. **Digit-split parsing** ("three sixteen"→3:16, "316"→3:16). *Why: whisper merges numbers constantly.* **[SHIPPED earlier]**
27. **Honest out-of-range + did-you-mean suggestions** (1 Cor 5:17→2 Cor 5:17). *Why: silent wrong staging is a product-killer; honesty builds operator trust.* **[SHIPPED earlier]**
28. **Book-name fuzzy match + preaching-frequency tie-break**. *Why: "jon"→John beats Jonah statistically.* **[SHIPPED earlier]**
29. **Filler-tolerant structural parsing** — any filler between book/chapter/verse. *Why: no word list covers natural speech.* **[SHIPPED earlier]**
30. **Relative references** ("verse 17" while John 3 is on stage). *Why: preachers iterate inside one chapter.* **[SHIPPED earlier]**
31. **Quotation recognition** ("For God so loved…"→John 3:16, confirm-queued). *Why: most preaching cites text, not numbers.* **[SHIPPED earlier]**
32. **Upgrade model to base.en** (auto-detect, tiny.en fallback). *Why: base.en cuts WER roughly in half vs tiny.en on clean speech; M1 runs it comfortably.* **[NEXT]**
33. **Per-segment confidence surfaced** (avg log-prob → operator "how sure am I" color). *Why: lets the operator trust-but-verify; binding exists in whisper.cpp, needs an upgraded whisper-rs API.* **[NEXT]**
34. **ASR-confidence-weighted auto-stage threshold** (parser confidence × ASR confidence). *Why: combines both uncertainty sources into one honest gate.* **[NEXT — needs #33]**
35. **Self-learning correction table** — one-click "operator corrected X→Y" persists and pre-corrects future transcripts. *Why: every church's vocabulary (names, phrases) is local; local learning is the moat.* **[NEXT]**
36. **Room noise-profile calibration** (30 s calibration routine → VAD thresholds tuned per sanctuary). *Why: reverberant rooms break fixed thresholds.* **[NEXT]**
37. **Echo/PA-loop rejection** (simple correlation check against the PA feed if available). *Why: the mic often hears the room's own echo of the verse reader.* **[LATER]**
38. **Whisper "condition on previous text" within one utterance** — improves long-range consistency. *Why: range verses "16 through 18" benefit.* **[NEXT]**
39. **Command WER eval harness** — 200-utterance recorded eval set, CI-measured WER per release. *Why: you cannot improve what you do not measure (see ACCURACY.md).* **[NEXT]**
40. **Streaming partial hypotheses** — show "live guess" while the preacher speaks. *Why: perceived latency halves even when real latency doesn't.* **[LATER]**
41. **VAD pre-roll buffer** (500 ms captured before speech starts). *Why: first-word clipping is the top cause of dropped book names.* **[SHIPPED earlier]**
42. **Adaptive noise floor** with slow-silence adaptation. *Why: HVAC noise drifts; a fixed threshold breaks.* **[SHIPPED earlier]**
43. **Minimum-speech gate** (250 ms). *Why: coughs/coughs-adjacent noise must not hit whisper.* **[SHIPPED earlier]**
44. **Mic device picker + persistence.** *Why: churches plug/unplug interfaces; wrong default device = zero recognition.* **[SHIPPED earlier]**
45. **Input level meter with speaking indicator.** *Why: 90% of "it doesn't hear me" is gain/device, visible in one glance.* **[SHIPPED earlier]**
46. **AGC/normalization study** (soft-gain before whisper). *Why: quiet preachers + distant mics under-perform; whisper is robust but not magic.* **[NEXT]**
47. **Metal acceleration for whisper** (Apple GPU). *Why: base.en+beam needs the GPU for <1 s latency; whisper.cpp supports Metal natively.* **[NEXT]**
48. **Fine-tuned whisper** on pulpit speech (transfer learning, 10 h sermon audio). *Why: domain fine-tuning is the single biggest possible WER cut.* **[LATER — needs data + training pipeline]**
49. **Alternative engine slot** (pluggable ASR trait already exists) — e.g. a future distilled local model. *Why: no vendor lock-in; the architecture accepts better engines.* **[SHIPPED — trait exists; new engines plug in]**
50. **Per-utterance audio snapshot on error** — save the WAV clip when staging fails so the operator can replay/label it. *Why: turns every failure into future training data.* **[NEXT]**

## C. Stage & projection (the congregation's experience)

51. **Stage zoom controls** (A− / A+ / 1:1 + voice + persisted). *Why: rooms differ; one fixed font never fits all.* **[SHIPPED]**
52. **Gentle 350 ms fade-in** on every stage change. *Why: hard cuts feel harsh and draw attention to the tech.* **[SHIPPED earlier]**
53. **Pure-black stage, warm-white serif, gold reference badge** (Cathedral Dark). *Why: maximum projector contrast, reverent tone; chosen over 4 alternatives in DESIGN.html.* **[SHIPPED earlier]**
54. **Church name watermark** on stage. *Why: professional look for the congregation screen.* **[SHIPPED earlier]**
55. **Chapter-only badge** ("chapter · verse 1"). *Why: honesty about what was asked for vs shown.* **[SHIPPED earlier]**
56. **Stage preview mirrors the live window** in the operator UI. *Why: the operator must see exactly what the congregation sees.* **[SHIPPED earlier]**
57. **Points/sermon-outline staging** (from "there are three things…"). *Why: structures the sermon visually, not just verses.* **[SHIPPED earlier]**
58. Fullscreen F11 + Esc handling on the stage window. *Why: projector operators expect standard keys.* **[SHIPPED earlier]**
59. **Auto-fit font scaling** by text length + viewport. *Why: long ranges must stay readable without manual tweaks.* **[SHIPPED earlier]**
60. Stage themes (light "projector-max", high-contrast, custom colors). *Why: sanctuaries differ in lighting and taste.* **[NEXT]**
61. Background image/video support on stage. *Why: worship-flow aesthetics; matches pro presentation suites.* **[LATER]**
62. Lower-third mode (verse as overlay over video feed). *Why: streaming churches.* **[LATER]**
63. NDI/stream output of the stage surface. *Why: OBS integration without screen capture.* **[LATER]**
64. Stage motion (ken-burns-style subtle drift). *Why: polish; must stay reverent.* **[LATER]**
65. Verse-transition choreography presets (fade/slide/none). *Why: taste control for media teams.* **[NEXT]**

## D. Recording, index & sync (PRD feature 3)

66. **Human-readable `index.md`** written at service end — timestamped scripture index + artefact links. *Why: the pastor's week-after artifact; zero-effort sermon prep memory.* **[SHIPPED]**
67. **`events.jsonl` machine-readable index** (already per-event). *Why: machine-readable = integrations (church CMS, podcast chapters) without re-work.* **[SHIPPED earlier]**
68. **`transcript.txt`** timestamped. *Why: the raw history; pairs with the index.* **[SHIPPED earlier]**
69. **Recording defaults ON for live service.** *Why: the most valuable take-home artifact must never be forgotten.* **[SHIPPED earlier]**
70. **Local-first storage** (~/RhemaFlow-Services). *Why: pulpit audio is sensitive; local-first is a privacy promise, and it works with the network cable pulled.* **[SHIPPED earlier]**
71. **Cloud sync to user-owned storage** (iCloud Drive folder copy or S3 with user keys). *Why: backup without a server trust relationship.* **[NEXT]**
72. **Auto chapter markers for podcast export** (from events.jsonl). *Why: sermon podcast post-production drops from hours to minutes.* **[NEXT]**
73. Sermon archive browser (past services, searchable by verse). *Why: "what did pastor say about Romans 8 last year?"* **[NEXT]**
74. Verse-clip export (audio snippet around a cited verse). *Why: social clips + study groups.* **[LATER]**
75. Auto-transcript polishing pass (whisper re-decode with context). *Why: transcripts currently serve indexing, not reading.* **[NEXT]**

## E. Multi-translation engine (PRD feature 2)

76. **KJV embedded, build-validated (31,102 verses).** *Why: public domain = zero licensing risk; build-time validation means a corrupt asset cannot ship.* **[SHIPPED earlier]**
77. **BBE embedded as second translation** + translation switch in UI. *Why: proves the multi-translation architecture end-to-end; simpler English helps newcomers.* **[SHIPPED]**
78. **`from_translation()` framework API** — any public-domain translation in the same schema plugs in with structural validation. *Why: the framework exists; adding ASV/WEB is a data task, not an engineering task.* **[SHIPPED]**
79. **Licensed-translation scaffold** (NIV/AMPC-ready) with honest "requested — shows KJV" note. *Why: the PRD requires licensing-awareness; we never fake a licensed text.* **[SHIPPED earlier — note; SHIPPED v0.801.2 — BBE path]**
80. Translation applied to stage text only; parsing stays KJV-canonical. *Why: versification differences (BBE has +2 verses) must never break detection.* **[SHIPPED]**
81. Add ASV/WEB (public domain) as built-ins. *Why: more choice, zero licensing risk.* **[NEXT]**
82. Parallel view (two translations side-by-side). *Why: teaching settings.* **[NEXT]**
83. Strong's numbers / original-language peek. *Why: teaching pastors.* **[LATER]**
84. Translation download manager (user picks, validated, cached). *Why: keeps the binary small, choice large.* **[NEXT]**

## F. UI/UX (operator experience)

85. **HEARD → RESULT interpretation strip.** *Why: the operator's #1 anxiety is "did it understand me?"; instant feedback closes the loop.* **[SHIPPED]**
86. **Three-zone operator layout** (transcript | stage+index | review queue) — zero scrolling for the core loop. *Why: media-booth ergonomics; chosen in the design phase.* **[SHIPPED earlier]**
87. **Review queue with Approve/Correct/Dismiss + alternatives.** *Why: human-in-the-loop beats blind automation in live settings.* **[SHIPPED earlier]**
88. **Auto-read toggle** (speak every new verse). *Why: some preachers want it, some hate it — it's a preference, persisted.* **[SHIPPED]**
89. **Voice picker with gender + natural-quality badges (★).** *Why: "male or female, natural not robot" is an explicit user requirement.* **[SHIPPED]**
90. **"Manage voices…" button** opens macOS voice downloads directly. *Why: discoverability of the free natural voices.* **[SHIPPED]**
91. **Rate slider** for read-aloud. *Why: pacing is personal.* **[SHIPPED]**
92. **Simulator + "play demo phrase"** — test the full pipeline without speaking. *Why: demo-day insurance; training new operators.* **[SHIPPED earlier]**
93. Dark, calm operator theme (Cathedral Dark) with a single gold "live" accent. *Why: dim media booth; eye lands on what matters.* **[SHIPPED earlier]**
94. Keyboard-complete operation (N/P/M/B/R/F/H/Esc). *Why: fastest path for operators; voice is for the preacher, keys for the operator.* **[SHIPPED earlier + R]**
95. Command palette (⌘K) for every action. *Why: power operators.* **[NEXT]**
96. Operator "second screen" layout presets (single-window vs triple). *Why: laptop screens differ.* **[NEXT]**
97. Onboarding tour (first-run 60 s walkthrough). *Why: volunteers rotate; training must be in-product.* **[NEXT]**
98. Sound design (subtle confirm chime on stage). *Why: ear feedback for the operator facing away from the screen.* **[NEXT — off by default]**
99. Accessibility pass ( VoiceOver labels, contrast audit, reduced-motion). *Why: church AV teams include disabled operators; also just right.* **[NEXT]**
100. Live latency + confidence HUD (avg ms, % auto-staged). *Why: the health metrics in the corner build trust and debug rooms.* **[SHIPPED earlier]**

## G. Platform & distribution

101. **One-command `./run.sh`** (installs toolchain if missing, downloads model with checksum, builds, launches, guides mic permission). *Why: "make it easy" is a feature; every step removed doubles adoption.* **[SHIPPED]**
102. **Model bundled in the zip** (turnkey offline). *Why: demo must never depend on venue Wi-Fi.* **[SHIPPED]**
103. **`--demo` / `--no-asr` / `--voices` / `--church` CLI modes.** *Why: scriptable setup + support diagnostics.* **[SHIPPED + --voices]**
104. **Crash-log discipline** (latest-crash.log + run.sh auto-tails on failure). *Why: support starts from evidence.* **[SHIPPED]**
105. **Regression-locked tests** (64 green; every user-reported failure becomes a test). *Why: the PRD's own rule; it converts pain into permanence.* **[SHIPPED — 47→64]**
106. Version-sync discipline (one version everywhere, audited at release). *Why: drift destroyed trust in many projects; the audit grep is cheap.* **[SHIPPED]**
107. Signed/notarized .app bundle (drag-install, no terminal). *Why: the last mile of "easy" for non-technical volunteers.* **[NEXT]**
108. Auto-update channel. *Why: churches won't run installers weekly.* **[LATER]**
109. Windows/Linux builds. *Why: market expansion.* **[LATER]**
110. iPad remote-control companion (operator walks the stage). *Why: real churches move the operator off the desk.* **[LATER]**

## H. Community & growth

111. Multilingual UI. *Why: missions networks.* **[LATER]**
112. Plugin/scripting surface (Lua/WASM for custom flows). *Why: churches hack everything; give them a safe place to.* **[LATER]**
113. Worship-software integrations (ProPresenter/Planning Center import). *Why: meet teams where they are.* **[NEXT]**
114. Open eval dataset of pulpit speech (opt-in contributions). *Why: the community's data becomes everyone's accuracy.* **[LATER]**
115. "Ask the passage" AI assistant (fully local LLM answering questions about the staged text). *Why: natural next product layer after text is reliably on screen; privacy-compatible because local.* **[LATER]**

---
*Count: 115 ideas. Shipped in v0.801.2 or earlier: 57. The rest are sequenced, not forgotten.*
