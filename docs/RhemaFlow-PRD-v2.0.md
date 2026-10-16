# **RhemaFlow**

## **Product Requirements Document — Version 2.0**

> **Supersedes v1.0.** Nothing from v1.0 was removed — it was sharpened. New in v2.0:
> numbered success metrics, MoSCoW priorities, a phased roadmap, technical budgets,
> risks with mitigations, per-feature acceptance criteria, and an honest map of what
> already ships today. The voice of v1.0 ("should", short sentences, one idea per
> paragraph) is retained deliberately.

---

### **1. Product Overview**

**RhemaFlow** is a smart church presentation and sermon intelligence platform designed to understand a preacher's spoken message in real time.

RhemaFlow listens during a church service, recognizes spoken Scripture references and recognizable Scripture quotations, retrieves the appropriate Bible passage, and automatically presents it to the congregation.

At the same time, RhemaFlow records the sermon, creates a timestamped transcription, identifies scriptures referenced during the service, and builds a searchable sermon archive.

The product is designed around one central principle:

> **The preacher speaks naturally. RhemaFlow understands and presents.**

RhemaFlow should automate as much of the live presentation experience as possible while ensuring the church's media team always retains meaningful control.

**Live-path rule:** everything needed for a live service — listening, parsing,
displaying, recording — must work with the network cable pulled. Cloud features are
convenience, never dependency.

---

### **2. Problem**

Church media teams often have to listen carefully to a preacher, identify Bible references, search for the correct passage, select the appropriate translation, prepare the presentation, and display the result at the right moment.

This creates several problems:

* Scripture can appear late because someone has to find it manually.
* The media operator must divide attention between the preacher and presentation system.
* A preacher can move to a new passage faster than the media operator can prepare it.
* Spontaneous Scripture references can be missed.
* Recordings do not automatically contain a structured index of the Scriptures used.
* Sermon recordings can become difficult to search or revisit later.
* Churches may already have presentation workflows they do not want to replace.

RhemaFlow addresses these problems by allowing the presentation system to understand the sermon rather than simply wait for manual instructions.

**Silent failure is a product defect.** When RhemaFlow cannot resolve what it heard,
it must say so in operator-readable language and offer the best alternative — never
quietly display the wrong verse. A wrong verse in front of a congregation is worse
than no verse.

---

### **3. Product Vision**

RhemaFlow should become the church's intelligent live presentation assistant.

It should eventually understand:

* What Scripture the preacher is referring to.
* What translation the preacher wants.
* When a Scripture should appear.
* When the preacher refers back to an earlier passage.
* What important sermon statements may be useful as presentation content.
* What happened during the service.
* Where important moments occurred in the recording.

The system should remain helpful without becoming intrusive.

---

### **4. Target Users**

### **Primary Users**

**Preachers and speakers**

They should be able to preach naturally without learning special commands or changing their speaking style.

**Church media and AV operators**

They should have visibility and control over what RhemaFlow detects and presents.

### **Secondary Users**

**Church administrators**

They manage church settings, users, services and archive access.

**Church members and viewers**

They may consume shared sermon recordings and searchable sermon content.

**Ministries and larger church organizations**

They may operate multiple services, teams or locations.

---

### **5. Core Product Promise**

RhemaFlow should provide:

> **Automatic presentation when confidence is high, human control when judgment is needed.**

The preacher should not have to operate the software.

The media operator should not have to manually perform tasks that RhemaFlow can safely perform itself.

---

# **6. Core Features**

*Priority labels: **M** = Must (MVP), **S** = Should (M1), **C** = Could (M2+).
Each feature carries acceptance criteria. Status names the v0.813.0 reality.*

## **6.1 Voice-Activated Scripture Trigger** — **M** · status: **shipped (structural parser)**

RhemaFlow continuously interprets the selected live service audio and identifies Scripture references.

It should understand natural expressions such as:

* "John 3:16."
* "John chapter 3 verse 16."
* "John chapter 3 verses 16 through 18."
* "First Corinthians chapter 13."
* "Let's go back to verse 7."
* "Turn with me to Psalm 23."

Parsing must be **structural, not a word-whitelist**: any number of filler words is
tolerated between the numbers of a reference, because no word list can cover natural
speech. When a spoken verse does not exist in the target book, the product says so
honestly and suggests the closest real candidates (for example: "1 Corinthians 5 has
only 13 verses (no verse 17) — did you mean 2 Corinthians 5:17?").

**Acceptance criteria**

* Given the transcript "let's go to john three sixteen", when it is processed, then
  John 3:16 is staged with its full verse text.
* Given "1st Corinthians five, for seventeen", then the product reports the honest
  out-of-range message and offers suggestions — it must not stage verse 1 silently.
* Given the bare phrase "verse 17" while a chapter is on stage, then the on-stage
  reference jumps within that chapter.
* Given a chapter-only mention ("First Corinthians chapter 13"), then verse 1 of that
  chapter is staged and the operator sees that it is chapter-only.

## **6.2 Scripture Quotation Recognition** — **S** · status: **planned**

RhemaFlow should also recognize recognizable quotations of Scripture even when the preacher does not state the reference.

> "For God so loved the world…"

RhemaFlow should be able to identify the likely Scripture and make it available for presentation.

The operator must retain the ability to approve, modify or reject uncertain detections.

**Acceptance criterion:** given a quotation whose first eight words match a unique
KJV verse, then the candidate appears in the confirm queue labeled as a quotation
match, never auto-staged above the operator's confidence setting.

## **6.3 Natural Context Understanding** — **S** · status: **partially shipped (bare "verse N" jumps; "that verse" planned)**

RhemaFlow should understand references in context rather than treating every statement independently.

> "Let's go back to that verse." · "Now look at verse 7." · "Compare that with Romans 8."

Context should help RhemaFlow understand what the preacher is referring to during the current service. Bare verse numbers already resolve relative to the chapter on stage; deictic phrases ("that verse", "the same passage") should resolve against stage history.

## **6.4 Translation Management** — **S** · status: **KJV shipped; dropdown ported from the v0.812 branch in M1**

RhemaFlow should support a church-selected default translation.

It should also recognize when the preacher explicitly asks for another translation. The product should support public-domain Bible translations directly while providing a pathway for churches to use appropriately licensed translations.

**Constraint:** initially only translations whose license permits redistribution
(KJV public domain; WEB; others reviewed per text). No bundled text without a license
check recorded next to it.

## **6.5 Confidence and Operator Review** — **M** · status: **shipped (confirm queue + confidence floor)**

When RhemaFlow is uncertain, it should not silently make an unreliable decision.

Instead, the operator should receive a useful suggestion.

**Possible Scripture detected** — John 3:15 · John 3:16 — **Approve · Correct · Dismiss**

The system should make uncertainty visible without making the operator manually verify every straightforward Scripture reference.

**Acceptance criteria**

* Given a reference scoring below the auto-stage threshold, then it appears in the
  confirm queue with alternatives, and the stage is unchanged until an operator acts.
* Given a non-speech transcript ("[BLANK_AUDIO]", "[MUSIC]"), then no candidate is
  produced and no error status is raised.

## **6.6 Smart Scripture Presentation** — **M** · status: **shipped (congregation stage, adjustable size/colors; layout adaptation M2)**

When Scripture is displayed, RhemaFlow should automatically create a polished presentation containing the Scripture text, reference, translation and appropriate formatting.

The layout should adapt to the amount of content. Short passages should remain prominent and readable. Longer passages should remain easy to follow.

## **6.7 Church Branding and Presentation Styles** — **C** · status: **planned (M2)**

RhemaFlow should provide a polished default presentation style. Churches should be able to customize branding, logo, visual style, fonts, colours, backgrounds and layouts, and save multiple styles for different services or events.

**Acceptance criterion:** a style is a single exported file that a different
installation can import and render identically.

# **7. Sermon Intelligence**

## **7.1 Live Transcription** — **M** · status: **shipped (offline ASR)**

RhemaFlow should create a live transcription of the sermon. The transcript should provide context for the live service and become part of the sermon archive. Transcription is local; no audio leaves the machine.

## **7.2 Scripture Index** — **M** · status: **shipped (timestamped scripture events written with the recording)**

Every detected Scripture reference should become part of the sermon record with its time within the service.

**Sunday Service** — 10:42 John 3:16 · 17:15 Romans 8:28 · 31:07 Psalm 23:1–6

Selecting an entry should take the user to that point in the sermon.

**Acceptance criterion:** every staged reference produces exactly one index entry
whose timestamp is within one second of the stage event, whether the reference came
from voice, the confirm queue, or manual entry.

## **7.3 Sermon Timeline** — **S** · status: **partial (events recorded; timeline view M2)**

The service should have a chronological timeline containing scripture references, translation changes, suggested content and important sermon moments. This timeline becomes part of the completed sermon archive.

## **7.4 Suggested Sermon Content ("three things" detection)** — **C** · status: **planned**

> "There are three things I want you to remember: Faith, Obedience and Perseverance."

RhemaFlow could suggest **Faith / Obedience / Perseverance** as presentation content. The operator can approve, edit or reject. This is enumerated-list detection first, semantics later — a strict pattern ("there are N things…") ships before any open-ended suggestion.

**Acceptance criterion:** given the enumerated pattern with N between 2 and 5 and a
list of short items, then one suggestion card is raised listing all N items; nothing
is staged without operator approval.

# **8. Live Operator Command Centre** — **M** · status: **partial (status, transcript feed, confirm queue, stage controls shipped; consolidated dashboard M2)**

The media operator should have one central workspace showing the state of the service: live transcription, current detected Scripture, current presentation, translation, uncertain detections, suggested content, Scripture history, service timeline, recording state and presentation controls.

The goal is not to give the operator more work. The goal is to give the operator **one place to understand what RhemaFlow is doing**.

**Acceptance criterion:** from a cold start, an operator who has never seen RhemaFlow can answer "what is on stage, what is it hearing, and is it recording?" within ten seconds.

# **9. Service Modes** — **C** · status: **planned (M2)**

**Full Service Mode** · **Sermon Mode** · **Bible Reading Mode** · **Custom Service Mode**

This prevents unwanted triggers during unrelated parts of the service.

# **10. Audio Source Management** — **S** · status: **partial (default input shipped; picker M1)**

RhemaFlow should work with an appropriate preacher/audio source while allowing the operator to change or select the source when necessary. The operator should never be locked into a single workflow.

**Acceptance criterion:** changing the input device takes effect within two seconds
without restarting the app, and the active device is always visible.

# **11. Recording** — **M** · status: **shipped (offline recording, scripture events embedded)**

RhemaFlow should create a high-quality sermon recording and associate it with the generated sermon record. The recording should remain connected to: transcript, scripture index, timeline, presentation events and sermon metadata.

**Constraints:** local storage first; a 60-minute service should not exceed
approximately 300 MB on disk (WAV/lossless acceptable in MVP; compressed container
M1); recording state must be visible at all times.

# **12. Sermon Archive** — **S** · status: **planned (local-first M2; cloud M3)**

Every completed service should become a searchable sermon record: title, date, speaker, service, transcript, recording, scripture index, translation information, timeline, presentation events, tags. Searching should work by scripture, speaker, date, series, topic and tag.

# **13. Post-Service Editing** — **S** · status: **planned**

Church users should be able to correct transcription errors, incorrectly identified references, sermon names, metadata, translation information and categories. Corrections should not destroy the underlying original record.

# **14. Sharing** — **C** · status: **later (M3, requires accounts)**

Churches should be able to share completed sermons with control over what is publicly shared. Downloading should remain available where permitted by the church.

# **15. Offline and Online Operation** — **M** · status: **shipped (fully offline live path)**

RhemaFlow should continue providing essential live functionality even when internet access is interrupted. When connectivity is restored, appropriate cloud information should synchronize. This is especially important for live worship environments where service continuity matters.

# **16. Church Accounts and Roles** — **C** · status: **later (M3)**

Roles: Church Administrator · Media Operator · Speaker/Preacher · Archive Viewer.

# **17. Multi-Service and Multi-Location Support** — **C** · status: **later (M3)**

Sunday Morning · Sunday Evening · Wednesday Bible Study · Youth Service · Main Campus · North Campus · Online Service. Each service retains its own sermon record.

# **18. Translation Expansion** — **S** (architecture M1, languages later)

The initial product prioritizes excellent English-language support. RhemaFlow should then expand to additional spoken languages and Bible-language combinations without weakening the initial experience.

# **19. Existing Presentation Software** — **S** · status: **companion mode shipped (standalone stage window)**

RhemaFlow should operate as a complete presentation system **or** as an intelligent companion to an existing presentation system. Churches that invested in current workflows should never be forced to abandon them.

# **20. Service Preparation and Reusable Templates** — **C** · status: **planned (M2)**

Before a service, a church may optionally prepare service information, speaker, sermon title, expected scriptures, preferred translation, presentation style. During the service, RhemaFlow continues responding to new or unexpected material. Preparation should make RhemaFlow better informed, not mandatory. Common arrangements should be savable as templates.

# **21. Privacy and Data Ownership** — **M** · status: **shipped (local-first, no telemetry)**

Churches should have clear control over their sermon information: what is recorded, how long content is retained, who can access it, what is shared, what is deleted. Privacy and trust should be treated as core product requirements rather than secondary settings. The live path sends nothing anywhere.

# **22. Core User Journey**

**Before the service** — the operator opens RhemaFlow, selects or starts a service, optionally applies a template. The default translation and presentation style load. *(Template selection: M2.)*

**During the service** — the preacher says "Let's turn to John chapter 3, verse 16." RhemaFlow identifies the reference. The Scripture appears on screen. The operator can override it. Later the preacher quotes "For God so loved the world…" and the candidate appears for approval. The service continues without the preacher touching software. *(Quotation candidates: M1.)*

**During an uncertain moment** — RhemaFlow raises a suggestion; the operator approves, corrects or dismisses; the service continues. *(Shipped.)*

**After the service** — recording, transcript, scripture index and timeline are saved; corrections can be made; the sermon joins the archive and may be shared. *(Archive and editing: M2.)*

# **23. Technical Constraints and Budgets**

These are commitments, not aspirations. A release that breaks a budget is a failed release even if its features work.

| Constraint | Budget |
|---|---|
| Platform | macOS on Apple Silicon (aarch64-apple-darwin), M1 baseline |
| Live connectivity | Fully offline: ASR, parsing, display, recording |
| ASR | Local whisper (English) bundled once; no download at service time |
| Reference → on screen | ≤ 2.5 s end-to-end on M1 for a clear utterance |
| Parse + stage work | < 50 ms CPU per transcript (excluding ASR decode) |
| Memory | ≤ 350 MB RSS during a live service |
| Installer | ≤ 100 MB zip; the ASR model is the dominant term and ships once |
| Bible text | Full KJV (31,102 verses) embedded and validated against verse counts at build |
| UI stack | egui/eframe 0.28.1; audio capture via cpal 0.15.3 |
| Tests | Regression suite green before any release; every user-reported failure becomes a locked test |

# **24. Success Metrics**

The product succeeds when it meaningfully reduces the amount of manual work required from the church media team. §27 of v1.0 asked questions; v2.0 answers them with thresholds.

| Metric | Definition | Threshold | Measured by |
|---|---|---|---|
| Auto-stage rate | Chapter-qualified references staged with zero operator action | ≥ 90% per 20-min sermon | Session log |
| Recognition precision | Staged references that the operator did not correct | ≥ 97% | Operator corrections |
| Silent-failure rate | Wrong verse displayed with no warning | 0 — any occurrence is a release blocker | Session log audit |
| Honest-miss quality | Unresolvable requests answered with a useful suggestion | ≥ 80% of misses carry a correct suggestion | Sampled review |
| Reference latency | Speech end → verse on screen | ≤ 2.5 s (p95) | Timestamped events |
| Operator touches | Manual interactions per 20-min sermon | ≤ 3 for a typical sermon | Session log |
| Preacher friction | Behaviour changes the preacher must make | 0 by design | Review |
| Archive usefulness | Time to find a referenced verse in a past sermon | ≤ 30 s | Timed test |
| Adoption | Churches active after 4 weeks of trial | ≥ 60% | (M3, once accounts exist) |

# **25. Phased Roadmap**

**M0 — shipped (this is v0.813.0 today)**

Offline whisper ASR · structural reference parser (filler-tolerant, digit-split,
fuzzy book match) · honest out-of-range messages with suggestions · bare "verse N"
jumps relative to the stage · next/previous navigation with chapter rollover · voice
fullscreen enter/exit · confirm queue with confidence floor · offline recording with
timestamped scripture events · full KJV embedded · 47/47 regression tests green.

**M1 — next release train**

Port the v0.812-branch features (translation dropdown, auto-gain, italics) ·
audio input picker · ASR accuracy pass (initial-prompt biasing with the 66 book
names, post-ASR fuzzy correction against the book list) · quotation recognition
(6.2) behind the confirm queue · context commands "back to that verse" · locked
regression utterances from every real mishear.

**M2 — archive and polish**

Local sermon archive with search (12) · post-service editing (13) · sermon timeline
view (7.3) · service modes (9) · templates (20) · branding/styles (6.7) ·
consolidated operator dashboard (8) · suggested "three things" content (7.4).

**M3 — cloud, accounts, teams**

Accounts and roles (16) · multi-service/multi-location (17) · cloud sync and sharing
(14) · adoption metrics (24) · additional languages (18).

> **Automation should grow as trust grows.** Each phase earns the next.

# **26. Risks and Mitigations**

| Risk | Impact | Mitigation |
|---|---|---|
| ASR mishears book names and numbers ("Perverse seventeen" for "verse seventeen") | Wrong or missing verse | Structural parser + suffix correction + fuzzy book match with preaching-frequency tie-break; every real mishear becomes a locked regression |
| Verse does not exist (1 Corinthians 5:17) | Wrong verse silently displayed | Verse-count-validated parsing; honest out-of-range message with "did you mean" suggestions; silent-failure metric is 0 by policy |
| Operator trust erodes after one visible mistake | Product abandoned mid-service | Confirm queue below confidence threshold; override always one key away; mistakes corrected in the next build, never explained away |
| Model size dominates installer | 72+ MB deliverables | Bundle the model once, exclude build artifacts, document expected size; download-on-demand alternative for constrained sites |
| Mic permissions on macOS block first run | App appears broken | One-time guided permission step with a visible state indicator |
| Translation licensing | Legal exposure | Public-domain texts only until each license is reviewed and recorded next to the text |
| Recording disk usage surprises a church mid-service | Lost recording | Visible recording state, free-space check at start, documented ~300 MB/hour |

# **27. MVP Definition (revised)**

v1.0 listed twelve "essential" capabilities. That is a v1.0, not an MVP. The MVP is
the smallest thing a church would run a real service on:

**Essential first release (all shipped in M0)**

1. **Live Scripture understanding** — structural parsing of spoken references.
2. **Automatic presentation + operator override** — stage and confirm queue.
3. **Navigation** — next/previous, chapter rollover, bare "verse N" jumps.
4. **Honest errors** — out-of-range messages with suggestions; no silent failures.
5. **Live transcription** — offline.
6. **Recording with scripture index** — timestamped events tied to the recording.

Everything else is M1+ and listed where it belongs in §25. The MVP is not the vision;
it is the trust-building core.

# **28. Later Expansion**

More automatic presentation content generation · greater sermon understanding · additional languages · more sophisticated multi-location workflows · deeper presentation integration · more advanced sermon discovery · richer sharing · more automated service preparation.

# **29. Non-Goals for the Initial Product**

RhemaFlow should not initially attempt to become a complete replacement for every church media tool, a church management system, a worship/music platform, a general-purpose AI assistant, or a video editor. Its differentiation remains strongly centered on:

> **Understanding live preaching and turning it into intelligent presentation and sermon records.**

# **30. Product Principles**

1. **Preacher first** — the preacher speaks naturally.
2. **Automation first** — repetitive presentation work is automated.
3. **Human control always** — the operator can intervene.
4. **Understand context** — meaning, not keywords.
5. **Beautiful by default** — auto-generated content looks professional.
6. **Useful after the service** — the sermon becomes a searchable record.
7. **Church-controlled** — presentation, access, sharing, archive.
8. **Start focused, expand intelligently** — win through scripture and sermon intelligence first.
9. **Never fail silently** *(new in v2.0)* — an honest "I could not resolve that, did you mean…" beats a confident wrong verse every time.

# **31. One-Sentence Product Definition**

> **RhemaFlow is an intelligent church presentation and sermon platform that listens to preachers in real time, understands Scripture and sermon content, automatically presents relevant information, and turns every service into a searchable, timestamped sermon record.**

# **32. Product Tagline**

> **RhemaFlow — The preacher speaks. RhemaFlow understands.**

---

# **Appendix A — Current-State Map (v0.813.0, honest inventory)**

| PRD capability | State | Code evidence |
|---|---|---|
| Structural reference parsing (filler-tolerant) | shipped | `src/engine.rs` — parser + `interpret_single` digit-split |
| Fuzzy book matching with frequency tie-break | shipped | `src/engine.rs` — alias table + `FUZZY_PRIORITY` |
| Honest out-of-range + "did you mean" | shipped | `src/session.rs` — `out_of_range_suggestions`, "has only N verses (no verse R)" |
| Bare "verse N" / number-only jumps | shipped | `src/session.rs` — relative-to-stage resolution |
| Voice fullscreen enter/exit | shipped | `src/session.rs` — `Action::Fullscreen(bool)` incl. ASR-mishearing tolerance |
| Next/previous with chapter rollover | shipped | `src/session.rs` — `nav()` |
| Confirm queue + confidence floor | shipped | `src/session.rs` — `Candidates`, `auto_accept_pct`, `voice_floor_pct` |
| Offline ASR (whisper, English) | shipped | `src/asr.rs` + bundled model in `models/` |
| Offline recording + timestamped scripture events | shipped | `src/rec.rs` — `ScriptureEvent` |
| Full KJV embedded (31,102 verses) | shipped | `data/kjv.txt` + build-time validation |
| Translation dropdown, auto-gain, italics | **on the user's v0.812 branch — port in M1** | local branch only |
| Quotation recognition (6.2) | planned M1 | — |
| Sermon archive / timeline view / templates / styles | planned M2 | — |
| Cloud, accounts, sharing, multi-site | planned M3 | — |

# **Appendix B — Locked Regression Utterances (must stay green forever)**

Every real-world failure a user actually hit becomes a permanent test. Current set
(the screenshot set that produced v0.813.0):

1. "1st Corinthians five, for seventeen" → honest out-of-range + suggests 2 Corinthians 5:17 (1 Cor 5 has 13 verses)
2. "Perverse seventeen" → corrected to "verse seventeen" → relative jump
3. Bare "verse 17" → relative jump within the on-stage chapter
4. "exit full screen" / ASR mishears of it → fullscreen exits
5. "let's go to john three sixteen" → John 3:16 staged
6. Chapter-only "First Corinthians chapter 13" → chapter 13 verse 1, marked chapter-only

A release that regresses any of these is GATE: RED regardless of other results.
