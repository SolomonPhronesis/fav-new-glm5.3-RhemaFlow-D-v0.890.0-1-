# Changelog

## 0.890.0 — the operator's gates: 856's suggestive scriptures back, adjustable, + the 91/92 near-tie rule, + a measured speech layer

The operator compared v0.856 against v0.889: **"I like 856 because it gives
suggestive scriptures, 889 doesn't."** Both versions were right about their
own numbers — 856 carded from 3 words / 50% (suggestive), 889 raised its
whole matcher onto a hard 5-word / 90% floor (quiet). v0.890 stops choosing
FOR the operator and ships the whole spectrum as settings, keeping every
measured safety rail the grids found.

**The suggestion gates are now settings** (NEEDS YOUR REVIEW panel →
"Suggestion gates", saved in settings.json like every other setting):

| Setting | Range | Default | Meaning |
|---|---|---|---|
| Cards (suggest): min words | 3 / 4 / 5 | **3** (v0.856) | shortest fragment that may raise a suggestion card |
| Cards (suggest): min % | 50-95, step 5 | **50%** (v0.856) | lowest score a suggestion may carry |
| Auto-show: min words | 3 / 4 / 5 | **5** (the v0.856 grid winner) | shortest fragment that may take the stage alone |
| Auto-show: min % | 75-90, step 5 | **90%** | the accuracy bar for auto-staging |
| Two verses over the bar (91 & 92) | show winner / ask me | **show winner** | the near-tie rule (below) |
| Stability hold | off / 0.5 / 0.65 / 1 s | **off** (v0.856 snappy) | how long a partial's candidate must survive every live decode before a card |

- **The near-tie rule (the operator's ask):** when TWO candidates both clear
  the bar — "91 and 92" — v0.856 raised a card (12-point margin rule);
  v0.890's default **auto-shows the winner, so the 92 is still shown**. One
  radio button switches back to "ask me". Measured on the real KJV: the
  five benediction twins ("the grace of our Lord Jesus Christ be with you" →
  Rev 22:21 99% vs 2 Cor 13:14 / Gal 6:18 / Phil 4:23 at 95%) now stage the
  winner by default and card again in ask-me mode.
- **The gates ride on ParseContext, never a global** — every parse carries
  its own copy, so the settings window can change them between utterances
  with no shared state and no test-order coupling (the v0.890 gate tests
  run parallel-safe).
- **Ported from v0.889 (the two fixes that made it stricter were kept out):**
  `decide_raise` — a card raised from a weaker live partial no longer blocks
  the stronger finished line (the 2 Timothy 3:15 bug: an 87% partial carded
  and the 93% finished line never staged; now the finished line upgrades and
  stages); `strip_filler_tail` — a trailing "So." / "Amen." / "okay" is
  scored both ways and the better reading kept (matching only, display text
  never rewritten); `QuoteHold` — the R2T2-style stability wait survives as
  the hold setting, default OFF (v0.856's instant cards).
- **Pieces, ranked, exact-window and trigram paths all answer to the same
  card gates**, so no tier can sneak a card past the operator's word floor.

**Speech layer measured and upgraded (docs/STT-V0890.md, charts
docs/stt-accuracy-rank-v0890.png + docs/stt-before-after-v0890.png, raw
docs/stt-bench-v0890.json):** 60 clips (12 real verse reads + 8 chatter
lines — including the Proverbs 19:21 and Genesis 1:1 traps — × clean /
music-noise / quiet-mic) through 12 candidate fixes, ranked by verse-read
accuracy on Linux. The shipped draft path (tiny.en, greedy) measures
~72%; the measured ways to 90%+ and what shipped:

- **KJV scripture post-correction (shipped):** a finished plain-speech line
  that strongly matches exactly ONE verse (ranked matcher ≥ 70 AND span fit
  ≥ 60 — the same double gate that keeps chatter silent) is re-rendered from
  the verse for the display, v0.855-style, with the honest note ("repaired
  against 2 Kings 19:15: 1 words filled…") and the raw ASR text kept in the
  export/transcript logs. Measured: +2.7 points on the turbo model
  (95.7 → 98.4) and +2-2.5 at every size, with **0 chatter rewrites in all
  12 bench rows** (the traps stay silent). Code:
  `rf-core/src/repair.rs::repair_from_match`, wired in the finished-line
  path of `rf-app/src/actions.rs`.
- **Final pass on the big model (already shipped in v0.852-854, now the
  measured default):** draft = streaming whisper (fast, low accuracy),
  final = large-v3-turbo decoded GREEDY (`./run.sh model-turbo`). Measured
  end-to-end on the bench: the two-pass line the operator reads hits
  **98.4% verse accuracy** (engine 95.7% + the KJV repair), above R2T2's
  published 93.9-95% band on the same metrics class; per condition the
  operator's 58-85% complaint is gone (clean 81→99, music-noise 57→97,
  quiet mic 79→99).
- **Beam + hotwords are worth +4-5 points at every SMALL model size** (tiny
  72.3 → 76.9; base 82.8 → 86.5; small + beam + hotwords = 91.1%, the goal
  on its own) — but on the TURBO model beam + prompt REGRESS to 59.7%:
  faster-whisper's beam search with an initial_prompt enters repetition
  loops on noisy input (one clip decoded "O Lord God of Israel" nine
  times). Shipped rule from the bench: big model decodes greedy; small
  models keep beam 5. The front-end (75 Hz HPF + speech-gated AGC + VAD
  trim, mirrored faithfully in the bench) and a spectral noise gate both
  measured neutral-to-negative for accuracy (69-72%) — the capture chain
  stays as-is.

Full 12-way ranked table, the two charts and the reproduction steps:
docs/STT-V0890.md.

Tests: rf-core 174 (7 new: gate word floor, gate % bar, true-read always
suggested, filler tail, repair-from-match ×3), rf-bible 46 (2 new), rf-app
338 (3 new: near-tie rule, adjustable auto bars, ask-me mode) — all green in
the Linux sandbox. rf-service / rf-media / rf-strongs / rf-studio green.

Trade-offs (honest): the near-tie default trades v0.856's card for an
automatic stage on near-identical benediction twins (one setting undoes
it); the KJV post-correction rewrites the DISPLAY only when the matcher is
confident, so ASR-damaged reads of verses NOT in the KJV wording (a
different translation read aloud) never trigger it; the bench used TTS
audio (one speaker per family), not a pulpit mic — run `./compare.sh` on
your Mac for the pulpit-mic number.

## 0.856.0 — the Proverbs 19:21 false show: root cause found, whole-Bible grid, accuracy-first fix

The operator asked: is the false Proverbs 19:21 a 3-words-minimum problem or an
accuracy problem — and whichever it is, raise the bar and PROVE the choice over
the whole Bible. Measured answer: **accuracy bars, not the 3-word tier.** The
four ordinary words "there are many devices" (said about phones, the verse's
own opening) scored 89% and the v0.855 auto-approve rule (60% single / 75%
clear-winner) put Proverbs 19:21 straight on the stage.

The proof (`docs/GRID-PROV19-21-v0856.md`, chart `docs/accuracy-v0856.png`,
raw `docs/grid-v0856.json`): every 4/5/6-word window of all 31,102 KJV verses,
deduplicated — 1,698,315 utterances a preacher can say IN PASSING — plus
33,189 real verse reads, through 13 configs (word-min 3/4/5 × bar
75/80/85/90). Shipped v0.855 auto-stages **653 per 1,000** of the passing
windows. The grid's winner, **W=5 / A=90** (rank 1 of 13, composite 93.3):
**57.25 per 1,000 (−91.2%)**, identification unchanged at 98.2%, 95.7% of
reads still auto-stage, the Proverbs 19:21 false fragments all silent, the
full verse read still stages at 95%. A=85 alone (the guess) only reaches
327.9 per 1,000 — the grid is why the bar went to 90.

Code: `rf-core/parser.rs` gains `AUTO_MIN_WORDS`/`AUTO_STAGE_PCT` and
`Detection.words` (the parser now records the utterance's word count);
`rf-app/state.rs::should_auto_approve` applies the two bars (below 5 words or
below 90% = card, the operator decides — a wrong verse can no longer take the
stage by itself). New examples: `prov1921` (the repro, now reads the live
constants) and `biblegrid` (the whole-Bible harness). Tests: the Prov 19:21
regressions at gate level and end-to-end through the real parser (rf-core 167,
rf-app 332, rf-bible 44 green in the Linux sandbox; rf-service/media/strongs/
studio green). Nine-group textbench: 100.0 before, 100.0 after.

Trade-offs (measured, in the doc): 52 of 33,189 true reads now wait as cards
instead of self-staging; "for God so" (3-word unique opening) cards instead of
self-staging (identification kept); the residual 57 per 1,000 are 5-6-word
verbatim openings textually indistinguishable from real quotes — the honest
text-only floor, 11.4× better than v0.855.

## 0.855.0 — the two operator screenshots fixed, with BEFORE/AFTER measured this time

v0.853/854 claimed "fixed and tested" but never measured the two NEW screenshot
failures against the tree they shipped. v0.855.0 reproduces both on the shipped
v0.854.0 code first (`cargo run -p rf-core --example repro855`), fixes them, and
re-measures everything with a nine-group text-layer benchmark
(`cargo run --release -p rf-core --example textbench`, BEFORE/AFTER JSONs in
`docs/`, chart `docs/accuracy-v0855.png`). Composite "listening quality":
**87.7 → 100.0** (method and honesty notes in `docs/TEXT-EVAL-NINE.md`).

**Screenshot 1 — the named verse was replaced by its look-alike.** The operator
put "Luke 1:17" on stage; the preacher read it; the half-line "turn the heart of
the father to the children" ALSO fits Malachi 4:6 (Luke quotes Malachi), and the
app carded Malachi 4:6 (66%, and on the shipped code Luke 1:17 was not even in
the candidate list — the singular "father" never reached the plural "fathers"
needles). Three fixes, all parser-side (`rf-bible/src/lib.rs`,
`rf-core/src/parser.rs`):
- **Named-verse lock**: while a verse is on stage and the incoming fragment is
  still explained by THAT verse's own text (`Bible::span_fit_pct` ≥ the same
  floor the fragment would have needed anywhere else), no look-alike verse may
  card or auto-stage. When the preacher genuinely moves on, the fragment stops
  fitting and detection resumes — the negative control stays working ("lest I
  come and smite the earth with a curse" still cards Malachi 4:6 while Luke is
  on stage). Both the finished-utterance path and the live-partial path get
  this automatically, because both parse through `parse_full(ctx)`.
- **Anchored span alignment** (`Bible::span_align` / `span_fit_pct`): every
  (heard, verse) word pair seeds one greedy in-order extension and the best
  matched-words × F1 span wins. A naive earliest-match alignment let filler
  words anchor at the verse's first "and/the" and hid a real 90% fit behind a
  24% score; a plain DP backtrack scattered the span across the whole verse.
- **Singular/plural folding** on the mid-verse probe trigram and on every
  needle's last word: "turn the heart" now reaches "turn the hearts" (and
  vice versa), so the ranked card shows BOTH look-alikes (Luke 1:17 56% next
  to Malachi 4:6 69%) instead of one wrong confident answer.
- Also new: **OT/NT name aliases** and **KJV -eth verb folding** in
  `modernize()` — Elijah/Elias, Isaiah/Esaias, Jonah/Jonas, Noah/Noe,
  dwelleth/dwells, believeth/believes, … applied symmetrically to verse text
  and speech, matching only, display text never rewritten.

**Screenshot 2 — short mid-verse fragments carded (even auto-staged) wrong
verses.** "also before him" — the last three words of Acts 23:33 — scored 79%
with no alternatives and, with auto-approve on, TOOK THE STAGE. New rule in the
ranked scorer: a 3–4 word fragment claiming a spot past the verse's mouth
(offset > 2) is offered only when it carries real evidence — the claim sits
within two words of the verse's opening (continuation can confirm it), or it
covers ≥ 35% of the verse's words. Tail claims with no continuation and no bulk
are capped below the offer floor and stay silent — even after FAME_BONUS
(which can lift a capped-but-real claim, never a silenced one, and as of this
release never lifts a sub-60 base match across the auto-approve bar: "and he
gave his only horse away" can no longer auto-stage John 3:16).

**Sound-tag filter (promised in the lost v0.854 follow-up, now actually in).**
`rf-core/src/soundtag.rs` strips bracketed tags — "[MUSIC]",
"(upbeat guitar music)", "[APPLAUSE]" — before parsing, when EVERY word inside
the brackets is an audio/silence/audience word. Real asides ("Romans (chapter
8)", "(verse 16)") contain non-audio words and are never touched. Display text
stays exactly what the engine heard; the PARSER just stops seeing tag words
that used to dilute quotation matches and steer them wrong. Wired at the top of
`parse_full` (finished + live-partial + recent-pieces paths).

**Verse-anchored line repair (the "to the gods" half of screenshot 1).**
`rf-core/src/repair.rs`: while a verse is on stage and the utterance is plain
speech (no command, no detection), the transcript line is repaired against THAT
verse's own text — the aligned span is re-rendered from the verse (KJV
punctuation included), dropped verse words inside the sentence are filled in,
mangled sentence endings ("...to the gods" for "...to the wisdom of the
just;") are replaced when the verse's own sentence completes within a few
words, and ordinary pulpit asides ("...to the children today") are kept
verbatim. The app shows the repaired line, marks it as Scripture-coloured, the
export/transcript logs keep the RAW ASR text, and the interpretation strip says
what happened ("repaired against Luke 1:17: 4 words filled, 1 noise words
dropped"). Measured: on the five repair cases the on-screen WER drops
0.136 → 0.000 and no repair ever made a line worse.

**RF_PREFILTER=0** now bypasses the 75 Hz high-pass (`rf-asr/src/capture.rs`)
so the pre-filter can be A/B'd against a known-good desk feed.

**Tests** (all in the Linux sandbox, none on a Mac): rf-core 167 (13 new:
named-verse lock, look-alike alternatives, tail fragment silence incl. pieces
assembler, sound tags, navigation unchanged), rf-bible 44 (5 new: screenshot
regressions + span fit + aliases + plural fold), rf-app 330 (incl. the new
repair wiring), all other crates green (rf-service 179+21+…, rf-media 49+…,
rf-asr, rf-strongs, rf-studio) — 0 failures. The full v0.853 reference battery
(`refbench`) still scores 100% everywhere (43,358 reference parses, quotations,
chatter). `repro855` now prints: half-verse → Malachi 4:6 card WITH Luke 1:17
offered as an alternative (was: Malachi alone), "also before him" → nothing
(was: auto-stage Acts 23:33), Luke-on-stage → nothing, verse stays (was:
Malachi card).

**Honest limits:** the nine PUBLIC speech test sets behind the 93.9%/83.7%
chart still cannot run in this sandbox (no speech model, no audio, no dataset
access) — `docs/TEXT-EVAL-NINE.md` documents the text-layer substitute, its
nine groups, and exactly what it cannot tell you. Nothing here has run on a
Mac, with a real mic, or against the R2T2 sidecar live; run
`cargo test --workspace` after building, then read `HANDOFF.md`'s try-list.

## 0.854.0 - the LISTENING side of the accuracy plan (hotwords in, rumble out, rescue decode)

v0.853.0 fixed the UNDERSTANDING side (parsing, ranking, auto-show). v0.854.0
upgrades the three listening-layer pieces the R2T2 comparison chart says are
cheapest per point of accuracy — all offline, all unit-tested:

- **Streaming hotwords (R2T2 `context` slot) — new `RF_CONTEXT` protocol
  frame.** The app now opens every streaming session by pushing the 66 book
  names + the words learned from the operator's corrections to the sidecar,
  and `StreamClient::set_context()` can push updates any time. The sidecar
  applies new hotwords from the next utterance (R2T2 reads `context` natively
  when a streaming state is created; resetting mid-utterance would drop
  audio, so it waits for the utterance boundary). "Habakkuk"-class book names
  become far more likely to survive streaming, at zero latency cost. This
  retires item 2 of docs/R2T2-ON-M1.md §7 ("pass the book names as
  --context") — it now happens automatically. Code:
  `rf-asr/src/stream_client.rs`, `rf-asr/src/engines.rs::stream_context`,
  `sidecar/r2t2_server.py`.
- **Rescue decode for hard audio (Whisper accuracy path).** The whisper.cpp
  shim decoded fully deterministically (`temperature_inc = 0`), so a music-
  or noise-damaged window could only come out wrong or hallucinate. The beam
  accuracy path now uses whisper.cpp's standard temperature fallback
  (0.2 → 0.4 → 0.6) behind the quality gates (entropy 2.4, logprob −1.0 —
  this whisper.cpp pin's field names), while the fast interim path stays
  deterministic. The published recipe for exactly the "background music"
  case. Code: `rf-asr/native/rf_whisper_shim.cpp`.
- **75 Hz rumble high-pass before the AGC.** HVAC rumble, traffic and
  mic-stand thumps ate the AGC's gain budget and dragged the VAD's silence
  floor. The mic chain is now HPF → AGC → VAD → engines (the recorder tap
  still keeps raw audio). New `rf-asr/src/hpf.rs` (2nd-order Butterworth,
  coefficients in f64): DC killed, 30 Hz strongly attenuated, 400 Hz
  untouched, zero latency, ~7 multiply-adds per sample. 3 tests.

Measured: full workspace `cargo test --workspace` — **716 passed, 0 failed**
(2 ignored = the two run-on-your-Mac integration tests); sidecar suite
**13 passed** (2 new: RF_CONTEXT unit + protocol tests); refbench battery
re-run on this tree — **every group still 100%** (docs/accuracy-v0854.png,
`results-after-v0854.json` inside the zip). NOT measurable in this sandbox:
the WER effect of hotwords + rescue decode — run `./compare.sh` on your Mac
(2 min of your pulpit mic with the exact words written down) and put the
numbers in `docs/comparison_data.json`.

## 0.853.0 - the five operator-reported accuracy failures fixed (measured before/after)

All five screenshot failures are fixed and PROVEN by a new before/after harness
(`cargo run --release -p rf-core --example refbench -- out.json`; picture:
**docs/accuracy-v0853.png**). Every group of the battery now measures 100%,
including the full sweep of all 31,102 verses in four spoken forms (43,358
reference parses), every chapter of every book, the five real failures, a
quotation battery (3/4/6/8-word fragments, ASR-damaged, long lead-ins) and
pulpit-chatter false positives. Zero remaining failures.

- **#1 "Second Chronicles — no, Second Corinthians…" showed Joshua 1:17:** a
  self-correction inside one utterance kept the FIRST book instead of the
  corrected one. The parser now splits on the correction ("No/— sorry/actually")
  and stages **2 Corinthians 5:17**.
- **#2 "We go to Psalms." + "23 verse 1." showed 2 Corinthians 5:1:** a
  book-only detection followed by bare numbers kept the OLD context. The
  pending book now wins: **Psalms 23:1**.
- **#3 "…the evidence of things not seen. Next verse." did nothing:** the nav
  phrase is no longer swallowed by the long quotation tail — the command runs,
  and other voice commands keep working ("previous verse", "verse 5", "read
  till 20", "stop/clear", translations, timers).
- **#4 "let this mind be in you" did not auto-show:** auto-show now fires at
  4-5 words, and at **3 words when the fragment is distinctive** — "for God so"
  resolves to John 3:16 because no other verse in the whole KJV opens that way.
  The lead-in window widened 6 → 18 words ("…when the Bible says let this mind
  be in you" carries 15 lead words). With auto-show OFF the same hit appears
  as a suggestion card, best candidate first.
- **#5 misheard 3× in a row, nothing on screen:** the ranking matcher now
  scores every candidate verse against what was actually heard (word-order F1,
  own start position), so "let this mind being you" still lands Philippians
  2:5, a mishearing between repeats no longer clears the verse, and the
  repeat-3× case lands • waits • lands instead of nothing, nothing, nothing.
- **"behold I come quickly" now leads with Revelation 22:12** (the verse people
  mean), not the verse that textually opens with it (22:7) — 27 best-loved
  verses (Ps 23:1, John 3:16, Phil 2:5…) break ties toward the intended verse
  (`WELL_KNOWN`, FAME_BONUS 12). "The Lord is" cards Psalms 23:1 first, not
  Exodus 15:2.
- **Chatter can no longer card or stage itself:** "and he told them the story
  about the fisherman and his net again" used to open like Genesis 37:10 and —
  with auto-show on — stage it. A whole-utterance F1 gate now silences it.
  Chatter false positives: 90% → 100%.
- **Speed:** quotation matching 7.4 ms → 3.8 ms per call (trigram prefilter
  shared with the pieces matcher); references still ~1.4 µs/call.
- New tests: parser/books/cmdtable suites grew for every fix above; full
  workspace suite green in a clean sandbox (rf-core+rf-bible 193, rf-app 330).
- NOT verified on a Mac (sandbox-built only): the live mic path — run
  `cargo test -p rf-app` after build, then the real service: say "Second
  Chronicles no Second Corinthians chapter 5 verse 17", "Next verse", "for God
  so" with auto-show on/off.

## 0.852.0 - ClearSpeech: the transcript now corrects itself (noise/music/other voices)

- **Two engines on your voice (the broadcast-captioning pattern):** the fast streaming draft appears instantly as before; the moment a sentence ends, the best LOCAL high-accuracy model (Parakeet TDT 0.6B v3 → Whisper large-v3-turbo → base/tiny, auto-picked by what is downloaded) re-decodes the same audio and **corrects the transcript line** ~0.3-2 s later. If the corrected text is a scripture reference, the stage updates too — a verse misheard over the music fixes itself. Voice commands are NOT re-run from a correction (a corrected line can never double-advance). Everything stays offline and private.
- **New `find_final_model()`** (main.rs): picks the final-pass model; `RF_FINAL_MODEL=/path.bin` overrides. `RF_FINAL_PASS=off ./run.sh` returns to the draft-only behaviour. The operator banner names the active engine.
- **One engine, two jobs:** the failover standby (stream down) and the final pass (stream up) share a single model behind a mutex — no double RAM.
- **`./run.sh model-turbo`** downloads Whisper large-v3-turbo (574 MB). `./run.sh model-parakeet` (1.2 GB) is the recommended best (HF Open ASR Leaderboard #1, ~94% avg accuracy incl. noisy sets). The story, model-name answer, top-20 triage and next steps: **docs/CLEARSPEECH.md**.
- **`sidecar/bench_wer.py`** — quick before/after accuracy on YOUR mic (pure Python, 3 self-tests): `python3 sidecar/bench_wer.py compare --ref reference.txt --hyp-before draft.txt --hyp-after final.txt`. Works alongside the fuller `evalset` from v0.851.
- Estimated in-noise accuracy: ~60-80% (draft only) → **~92-95%** with the final pass (CLEARSPEECH §3 splits published facts from estimates honestly).


## 0.851.0 - measure speech accuracy on real clips (evalset)

- **`rhemaflow-d evalset FOLDER`:** score Whisper and Parakeet on a folder of your own clips (`clip.wav` + `clip.txt` with what was said, optional `clip.expect` such as `John 3:16`). Prints and saves (`evalset-report.md`): overall word error rate, how often the right scripture was found (and how often it only asked "did you mean?"), the worst clips, the mix-ups that came back ("habakkuk" -> "have a cook"), and `vocabulary-suggestions.txt` (words worth teaching). How-to: `docs/EVALSET.md`.
- Scoring lives in `rf-core/src/evalset.rs` (word alignment, mix-up runs, summary, report; 4 tests pass in a Linux sandbox). Clip pairing in `rf-app/src/evalset.rs` (1 test passes). The engine run itself needs your models and has NOT been run here; `rf-app` is NOT compiled here either: run `cargo test -p rf-app` and `cargo test -p rf-core` on the Mac first.
- Already in the project and not changed: the Whisper vocabulary prompt, learning from corrections (`learn.rs`), the personal lexicon and `calibrate` (`lexicon.rs`), `benchmark` for one file.

## 0.850.0 - the top 10 ideas

Built:
- **Search inside every PDF at once:** typing 3+ letters in the MEDIA search box reads every PDF on the shelf once (background, needs poppler like the PDF Find box) and shows "N PDF pages match"; click a page to open that PDF there (`Present::pdf_search`). Opens at that page when the PDF is not already on screen.
- **Order sung as chips:** each part in the order is a chip (click to remove) plus a "+ part" menu.
- **Slides preview** is a 16:9 panel (closer to the real screen).
- **Sunday check** (Options): missing files, big videos, songs whose order names a missing part, duplicates, unused files, files not shown for 3 months, backup older than a week or never (`Present::sunday_check`).
- **Wrap long lines** button and Shift+Cmd+W (`wrap_long_lines`, tested): splits any line over 42 letters at a space.
- **Whole service folder:** dropping/adding a folder also imports its .txt / .pptx files as songs, next to the pictures, videos and PDFs.
- **Last shown:** the right-click menu of a MEDIA item says "Last shown 3 days ago" (`MediaLibrary::shown_days_ago_at`, tested; fed by `touch`).

Not built, on purpose:
- **Media by item id in service files:** `resolve_any` (0.848.0) already finds renamed or moved files; changing the saved-service format risks old services. 
- **Per-song background and font:** needs the song's colours to reach three stage drawing paths and to be cleared again for scripture; it needs testing on the real stage window. Plan in TODO.md.
- **Remember the last PDF page:** already existed (`settings.resume`).

Library tests: 24 pass in a Linux sandbox. `rf-app` changes are NOT compiled here: run `cargo test -p rf-app` on the Mac first.

## 0.849.0 - Undo / Redo in the Songs editor, Songs shortcuts

- **Fix:** Edit > Undo (Cmd+Z) and Redo (Shift+Cmd+Z) did nothing in the Songs window, the same cause as Copy / Paste in 0.846.0: the key event was added after that window had been drawn for the frame. Now it is held in `pending_edit_events` for the next frame (`ui.rs::menu_action`). The Songs buttons (Tidy, Strip chords, Find repeats, + Verse) are undoable too, because the text box records each change.
- **Songs editor shortcuts** (only while a song is open for editing; shown on the buttons' hover text and in the gold menu strip): Shift+Cmd+T Tidy, Shift+Cmd+L Strip chords, Shift+Cmd+R Find repeats, Shift+Cmd+P add the next Verse. `Present::song_edit_shortcut`. Cmd+S Save song already existed.
- NOT compiled here: run `cargo test -p rf-app` on the Mac first.

## 0.848.0 - five more ideas

- **Saved service orders find renamed or moved files:** `MediaLibrary::resolve_any` (used by `stage_path`) follows rename logs, then the item made from that path, then a file with the same name. (A full "orders store item ids" change would touch the saved-service format; this fixes the symptom without it.)
- **Find repeats** (song editor): a song pasted with every verse and chorus written out gets named parts, one copy of a repeated chorus, and "Order sung" filled in. Tested (`detect_repeats`).
- **Slides preview** is drawn like the stage: light words centred on a dark panel.
- **Backup to a USB stick:** Tools > Backup shows "Use drive: NAME" for each plugged-in drive (media included). Backups now skip `media-library/trash`. Tested (`usb_volumes`).
- **MEDIA search** also counts matching songs ("3 songs match") and opens the Songs list with the same search. Searching INSIDE PDF words stays in the PDF's own Find box.
- Library tests: 23 pass in a Linux sandbox. `rf-app` changes are NOT compiled here: run `cargo test -p rf-app` on the Mac first.

## 0.847.0 - the 12 file-system ideas, song editor 5-7, TODO.md

Media library (`rf-service/src/library.rs`, 20 tests pass in a Linux sandbox):
- **Undo remove** (Options > Undo last remove): a removed file's stored copy goes to `media-library/trash` (deleted after 30 days) and comes back with its folder.
- **Re-link all missing** (Options): type the folder in the path box; matches by file name AND identical bytes, 4 folders deep.
- **Find duplicates** (Options): same bytes listed more than once.
- **Rename renames the file too:** renaming an item renames its stored copy (never overwrites; `Name (2)` if taken).
- **Old paths keep working:** every rename is logged in `path-moves.tsv`; `resolve_moved` is used by `stage_path`, so a saved service order still finds a file renamed after it was saved. `apply_shelf_moves` fixes the stage list and the "when it ends" steps.
- **Search by type:** type `pdf` or `.mp4` in the MEDIA search box.
- **Recently used** sort (Options > sort), remembered in `usage.tsv`.
- **Library backup:** `library.tsv.bak` is refreshed on every save (one step back).
- **Big-file warning** when a file is 500 MB or more (stronger at 2 GB).
- Already there from before: Show in Finder, folder drop with a progress bar. Thumbnails are made in memory (no disk cache), so "thumbnail cache cleanup" became the 30-day trash cleanup.

Songs editor: **warning** when "Order sung" names a part that does not exist (the preview now also follows the order), **Strip chords** (chord lines, web tags and `[Verse 1]` headings cleaned; tested), **Repeat a part** menu (adds a part to the order).

NOT compiled here (rf-app needs a newer Rust than the sandbox): run `cargo test -p rf-app` on the Mac first. See `TODO.md`.

## 0.846.0 - Select All / Copy / Paste work in the Songs window; song editor helpers

- **Fix:** Cmd+A / C / X / V did nothing inside the Songs window. The Mac menu owns those keys and handed them over AFTER the Songs window had already been drawn that frame, so its text boxes never saw them. They are now held in `App::pending_edit_events` and given to the next frame before any window is drawn.
- **Songs editor:** "+ Verse / Chorus / Pre-chorus / Bridge / Tag" buttons (Verse numbers itself), **Tidy** (trailing spaces, extra blank lines), and a live **Slides preview**. Helpers `next_part_header`, `append_part`, `tidy_lyrics` in `present.rs` (tested).
- NOT compiled here (rf-app needs a newer Rust than the sandbox has). Run `cargo test -p rf-app` on the Mac first.

## 0.845.0 - menu flash, real file names, marker-pen reading line, library check

- **Menu flash:** every command used from the menu or by its shortcut (Cmd+C / V / X / A, Cmd+S, Cmd+K, layouts, zoom...) lights up for 0.5 s on a copy of the menu bar across the top of the window: the menu title and "Edit > Copy Cmd+C" in gold. The real Mac menu cannot light one item itself. `macmenu::describe` is the table; add an id there to make a command flash.
- **Real file names on disk:** a copied media file is stored as `Everlasting God.mp4` (not `1d1bf20-...mp4`). Same bytes are still stored once (index: `media-library/stored-names.tsv`); a different file with the same name becomes `Name (2).ext`. Older hash-named copies are renamed on start (`MediaLibrary::tidy_stored_names`).
- **Reading line = marker pen:** the page is repainted inside the band with a colour tint (multiply), so paper turns solid highlighter colour and the words stay black. Brighter palette.
- **Library check on start:** counts unused files in `media-library/files` and says so; deletes nothing (`orphans`, `remove_orphans`).
- Tested: `rf-service` library tests (15) pass in a Linux sandbox (Rust 1.75). The `rf-app` changes (flash strip, marker pen, start-up note) could NOT be compiled here (egui build and lock file v4 need a newer Rust); build on the Mac first: `cargo test -p rf-app`.

## 0.843.0 - Studio: 20 features toward replacing a church presentation program

New crate **`rf-studio`** (pure rules, 179 tests) and a new **Tools > Studio** tab. Every rule is plain code with its own tests; the app only adds buttons. See `docs/STUDIO.md` for what is wired and what is engine-only.

- **Wired into the app:** Import songs (OpenLyrics, OpenSong, ChordPro, chords-over-words, plain; export to OpenLyrics) - Chords (transpose, capo, easiest capo, Nashville numbers, key detection, "show on the band's screen") - Bible versions side by side (main + up to 3 more, paged) - Alerts and nursery calls drawn over every screen, hidden during blackout - Macros (script, run, bind to a shortcut or a spoken phrase) - Song library tools (duplicates + merge, find and replace with preview, tags, most used) - Service templates (save the order, add its items later; `{date}` filled in) - Your own shortcuts (never the built-in keys) - Teleprompter preview - Stage monitor for preacher / band / tech, in its own window.
- **Engine only for now (tested, no screen yet):** auto-fit and balanced line breaks (`fit`), slide transitions (`transition`), multi-box slide layouts with an editor model (`layout`; used by the stage monitor), auto-advance cues (`cues`), walk-in playlists (`playlist`), media trim/speed/fade/filters (`mediaset`), preview-then-Take (`staging`), bilingual lyrics (`bilingual`).
- **From the pro list:** phone remote now shows a **QR code**; PDFs have **Find** (needs `pdftotext` from poppler) and a **page picture strip**; **stage preview in a second window**; **multi-language verses** = Bible versions side by side.
- Fixed while testing: none of the old tests changed; `Present::save_library` is now public.
- Verified in the Linux sandbox (Rust 1.75, eframe stub): whole tree compiles, **329 app tests + 179 rf-studio tests + all other crates pass**. This also settles v0.842.0, which had never been compiled. NOT seen on a Mac window.

## 0.842.0 - the reading line follows line by line, and songs paste properly

- **Reading line fix (documents):** a tightly-set paragraph no longer lights up as one block. The line finder now cuts a merged paragraph at the thin gaps between its wrapped lines (and a very tall solid band at an even pitch), so the reading line steps **line by line**. Real lines on such a page are no longer dropped (the old "thinner than the median" test was poisoned by the merged paragraph).
- **Reading line options:** next to the Reading line switch there are now **"Lines at a time: 1 / 2 / 3"** and six highlight colours (**yellow, green, blue, pink, orange, white**). The choice is remembered (`PresentSettings::doc_guide_lines`, `doc_guide_color`).
- **Songs:** the Paste lyrics form and the song editor both gained clipboard buttons. **Paste from clipboard** fills the paste box; **Paste into words** (next to Copy words) fills an empty Words box or adds the clipboard text at the end as a new part. The Words box hint now says Cmd+V works in the box too. Uses `arboard`, which egui already brings in, so nothing new is compiled.

## 0.841.0 - undo/redo for the service order, notes and timing, clipboard jumps, tidier dock

- **Undo edit / Redo edit** for the service order (add, remove, move, rename, duplicate), 50 steps; Cmd+Z / Shift+Cmd+Z when no text box is active (in a text box they stay the text box's own undo). A burst of typing or dragging is one step. Refused while the list is locked. The old button is now called **Undo screen**.
- **Item details** (button on each row): rename, planned minutes, a private operator note (never on the stage; allowed even when locked), Duplicate.
- **Time strip** at the top of the order: countdown for the item on the screen (counts over when late), what comes next, time left of the total. **Copy run sheet** and **File > Save run sheet as text** (saved in the data folder, `run-sheets`).
- **Clipboard:** Scripture > *Go to the reference on the clipboard* (Shift+Cmd+V), *Copy verse*, *Copy verse with reference*, *Copy reference*; Edit > *Paste and search scripture*; Media > *Add the file whose path is on the clipboard* (understands quotes, `file://`, `~/`).
- **Layout:** the dock tabs always stay on one line (the words shorten as the dock narrows); View > *Hide transcript* (Cmd+T); the last dock tab and the transcript choice are remembered.
- **PDF / HTML:** each document remembers its zoom.
- Old service-order and settings files still load (new fields have defaults).
- Not done from the lists: second-window stage preview, phone remote with QR, multi-language verses, PDF thumbnails and search. Auto-backup already existed (File > Backup; off until a folder is chosen).
- Compiled and tested in a Linux sandbox with a stub window (see the count in HANDOFF.md); NOT run on a Mac.

## 0.840.0 - PDF and HTML on the stage, resume, duplicates, working Cut/Copy/Paste, cleaner layout

- **PDF and HTML pages** can be dropped on the MEDIA bar and shown page by page on screen 2: next / previous page, go to page, zoom in / out, Fit page / Fit width, and a **Reading line** that highlights one line at a time and dims the rest so people can follow (arrow keys move it; it crosses to the next page by itself). Pages are made in the background (poppler `pdftoppm` if installed, else `mutool`, the Mac's own CoreGraphics, Ghostscript, or Quick Look); HTML is typeset by RhemaFlow itself.
- **Resume.** Going to something else and coming back carries a video or sound on from where it was (and a document from its page). Stop forgets the place.
- **Duplicates allowed.** The same picture or video can be listed twice (right-click > Duplicate). Both share one stored file. No more "already on the shelf".
- **Cut / Copy / Paste / Select All** are in the Edit menu and work in the text boxes (the menu item injects the same events the keys do).
- **Branding.** The header and the window title say just "RhemaFlow".
- **Sound sync (-50 / +50 / reset) removed** from the play bar (the picture follows the sound by itself).
- **Missing-glyph boxes fixed** (folder, arrows, tools icons) by adding a font that has them and swapping glyphs that no font has.
- **Layout.** The MEDIA bar's two header rows are one; it starts folded on short windows so the stage preview keeps its room. **End service** asks once before ending.
- run.sh installs poppler (optional) for sharper PDF pages. App bundle is named RhemaFlow.
- Compiled and unit-tested in a Linux sandbox (301 tests) with a stub window; not run on a Mac.

## 0.839.0 - real Mac menu bar with Help search, sound that stays matched, no frozen picture

- **Real Mac menu bar** (top of the screen) with Help last; macOS adds its Search box to Help (see docs/MENU-AND-HELP.md for the 3 ways considered). Uses the `muda` crate; the first build downloads it.
- **Sound and picture match by themselves.** Sound now plays inside the app with macOS's `AVAudioPlayer` (instant start, instant jump, true position) and the picture follows it, so there is nothing to adjust. `afplay` stays as the fallback. See docs/SOUND-SYNC.md for the 5 ways considered.
- **Picture froze while the sound played on, and was slow to catch up when you clicked back.** App Nap is switched off three ways, and a helper thread keeps redrawing while a video plays, so the stage on another screen keeps moving.
- A jump in a video no longer holds the picture still; Sound sync (ms) is now only for the fallback player.
- Not compiled or run on a Mac in this version.

## 0.838.0 - menu bar (File, Edit, View, Scripture, Songs, Media, Window, Help), header version fixed for real

- **Menu bar** across the top of the window (File, Edit, View, Scripture, Songs, Media, Window, Help last), every item with its shortcut. It is drawn inside the window, not in the Mac system menu bar.
- **Header said v0.831.0.** The fix was in the zip, but it was added after the first 0.837.0 zip and the second zip kept the same number, so an old build could be run. Each zip now has its own number, the header uses the same text as the title bar, and a test checks it matches the build.

## 0.837.0 - version everywhere, command palette, media keys, song editing, sound-sync setting

**Fixes**
- **The old version number (v0.831.0)** was still hard-coded in the top-left header. The name and version now come from the build, so every window, the About window and the title bar always show the real number.
- **Sound about half a second late after a jump.** The picture now waits for the sound to really start; the wait is the new "Sound sync (ms)" box on the play bar (default 500, with -50 / +50 / reset).
- **Show on stage** now also prints the CCLI line and counts in the usage report (it used the plain slide path before).

**17 things**
1. Version from the build everywhere. 2. Command palette (Cmd/Ctrl+K). 3. J / K / L = back 10 s, play-pause, forward 10 s. 4. PageDown / PageUp = next / previous slide. 5. Esc closes Help, About, palette. 6. Cmd/Ctrl+S saves the song being edited. 7. "unsaved changes" marker. 8. Delete needs a second click. 9. Copy song. 10. Copy words to the clipboard. 11. Songs listed A to Z with a count ("3 of 40 match"). 12. "N parts, N slides" preview while editing. 13. Warning for very long lines. 14. Window title shows LIVE while a service runs. 15. Sound sync -50 / +50 / reset. 16. Open the songs folder from the Songs tab. 17. Help rewritten with a search box, files, troubleshooting, shortcuts.

**7 more**
18. Recently shown songs: one click to show again. 19. Slide list scrolls to the slide on the screen. 20. Trailing spaces are removed from lyrics when saving. 21. About window with Copy diagnostics. 22. Help "Open data folder". 23. Mac app bundle in run.sh so the menu says RhemaFlow-D and the version. 24. F1 opens Help.

NOT run on a Mac or compiled in this version; check with `cargo test -p rf-app`.

## 0.835.0 - video thumbnails and sound after a jump fixed, pasted song words, lower thirds, and 17 new features

**Fixes**
- **Video thumbnail missing on big videos.** The old code refused videos over 150 MB (yours was 754 MB). Thumbnails are now made on a helper thread from the first good frame (skipping black ones), at any size; the app never waits for them.
- **No sound after a jump.** Jumping used to need ffmpeg to rebuild the whole rest of the sound before anything played, and nothing played if that was slow or missing. Now the AAC sound is cut straight from the video file (no other program) in milliseconds; ffmpeg is only the fallback for unusual sound formats. 3 tests with a real clip. NOT heard on a Mac (the sound still plays through macOS `afplay`).
- **Song words by pasting.** "Paste lyrics" in the Songs tab and Tools > Song words: paste, give a title, done. `[Verse 1]`, `Chorus:` lines or blank lines make the parts.

**Lower thirds** (Tools > Lower thirds): speaker name, scripture reference (can be automatic) or song title; three styles; stays N seconds with a fade.

**The 7 listed features:** announcement loop (stops itself if you put something else up) / giving card with QR code (drawn inside the app) / after-service handout (web page + text) / sermon-series looks (save and switch) / live caption translation (LibreTranslate, DeepL or Google; your key) / sermon clips (mark, caption file, audio cut with ffmpeg if present) / read-along (whole chapter, highlights the verse being read, follows the voice).

**The top 10:** crash recovery (offer to restore where you were) / volunteer lock (PIN) / run-sheet with auto-advance / scrub-bar hover thumbnail / pedal, MIDI and Stream Deck (see docs/CONTROLLERS.md) / different looks per screen (lobby, overflow, livestream) / accessibility (large print, high contrast, OpenDyslexic, Atkinson Hyperlegible, bigger operator window) / backup to a folder (use a cloud-synced folder) / remote operator page for a second campus (`/operator` on the web server) / plan from sermon notes with timings.

**Honest limits.** Tested on Linux with Rust 1.91 (all tests pass); the window layout was compiled but not looked at, and none of this was run on a Mac. Not tried with real hardware or services: MIDI devices, second monitors for the extra screens, the translation services, cloud sync (RhemaFlow copies into a folder; your sync app does the upload), Stream Deck. The lock stops accidents, it is not security against a determined person.

## 0.834.0 - big-video jumps fixed, pre-flight check, live captions, preacher monitor, CCLI

- **Video jump freeze fixed.** Every jump (Back/Forward 10s, dragging the bar, any time) used to rewind to the very first frame and decode every picture up to the target: jumping to 13:52 of a 22 minute video meant decoding 13 minutes first, and the screen froze. It now jumps to the nearest keyframe before the target and decodes only that short stretch (`VideoReader::seek_to`). New tests on a 20 s clip with keyframes and B-frames: every jump (forward, back, to the start, near the end) shows exactly the same picture as playing from the start, and a jump decodes at most 30 pictures instead of about 475.
- **Pre-flight check** ("Check" button next to Start service): microphone, speech model, disk space, sound/ffmpeg, screens, internet, subscription. Start service only stops to ask when something is RED (no mic, no model, under 1 hour of disk); a subscription problem is never red.
- **Live captions**: "Captions" button; what the preacher just said appears as subtitles on the congregation screen for 8 seconds (voice commands are never shown).
- **Preacher monitor**: "Monitor" button opens a separate window (put it on the screen facing the pulpit): clock, service time, what the congregation sees, and the next planned scripture.
- **Song lyrics with CCLI**: each song has a CCLI song number and a copyright line; the church's CCLI licence number is set once. The footer (author, CCLI Song #, copyright, licence) shows under the words, and each song used is added to `~/.rhemaflow/ccli-report.csv` once per run, ready to send to CCLI.
- Captions on/off and the monitor window are not remembered after a restart yet.
- Tests: rf-core 136, rf-app 191, rf-media 14+ (video), rf-service all pass (Linux, Rust 1.91). Not run on a Mac. Sound that was started with a video jump still uses the separate ffmpeg step; I did not change that part.

## 0.833.0 - one folder per service, "exit for screen", frozen-app escape, trial + billing + code sign-in

- **Voice: leave full screen.** "Exit for screen", "exit four screen", "that's it for screen" (what the microphone really wrote) now close full screen, as well as "exit full screen" and "exit".
- **One folder per service.** `<save folder>/<full date>/<start time> - <title>/` holds only that service's recording, transcript, notes and events, so three services on one day are three folders.
- **Frozen app.** New `quit_guard.rs`: a normal Quit (Activity Monitor, Dock, Terminal) now stops the sound players first even if the window is frozen. New `Force-Quit-RhemaFlow.command` (double-click): closes the app, force-closes if needed, stops leftover sound. A hard Force Quit (SIGKILL) still cannot be caught: the next start cleans up as before.
- **Accounts:** 14-day free trial with no card (database trigger), "Manage billing" button (Stripe portal, new edge function `billing-portal`), "email me a 6-digit code" sign-in (no password), 2 computers per account (church plan 5) with "sign out my other computers". Run the new section at the end of `supabase/schema.sql`. NOT tested against a live Supabase/Stripe project.
- Tests: rf-core 136, rf-service all, rf-app 182 pass (Linux, Rust 1.91). Not run on a Mac.

## 0.832.0 - big files are always copied, no player left behind, "kay jay vee"

- **Media: always copy.** The shelf used to copy files up to 300 MB and only link bigger ones, so a 34 MB video was copied but a 754 MB one was not. New default is always copy, however big (Settings still offers "only linked" to save disk space). Old settings that said "auto" now mean copy. Needs free disk space of the file's size.
- **Force-quit no longer leaves sound playing.** A force-quit cannot run cleanup, so the sound player (afplay/ffmpeg) kept playing. Every player is now noted in `rhemaflow-players.pid` (temp folder); the next start stops any still running (only processes named afplay or ffmpeg). The player still plays until you start RhemaFlow again; it is not stopped at the moment of force-quit.
- **Voice: translation names spoken as letter sounds** ("kay jay vee", "en kay jay vee") now switch the translation like "KJV" does. New test in cmdtable.rs.
- Tests: rf-core 136, rf-service all, rf-app 182 pass (Linux, Rust 1.91). Not run on a Mac.

## 0.831.0 - dock tabs, accounts (£10/month), the three layouts, blue/green transcript

Two rounds in one version: (1) Strong's and Scripture history moved out of floating windows, plus Supabase sign-in and the £10/month subscription rules; (2) the approved mock-up ("rhemaflow-overhaul.html") ported into the real app.

**Dock tabs and accounts (code by the previous session, unchanged here):**
- **Right dock is now five tabs**: Order · Review · Strong's · History · Index (`RightTab` in `ui_queue`). The old triggers still work — the Strong's button, the `[` `]` keys, and voice commands now just bring their tab forward. No floating window can cover the verse list any more.
- **Accounts**: `rf-app/src/account.rs` — sign in / sign up with a Supabase project through the system `curl` (no new TLS dependency), read the subscription, and feed it to the existing `license.rs` rules (7-day offline grace, 14 days when signed out, 5-day overdue grace, never cut a running service; no subscription = free Bible display, never a locked screen). The ticket is saved in `~/.rhemaflow/ticket.json` so the grace survives a restart. Header chip + "Your account" window. Server side in `supabase/` (schema.sql with row-level security, `create-checkout`, `stripe-webhook` — set `RF_SUPABASE_URL` and `RF_SUPABASE_ANON_KEY` when building; Stripe keys live in the two Edge Functions). NOT tested against a live Supabase/Stripe project.

**The mock-up ported into the app (this session):**
- **A/B/C layout switch** — top-right of the header, remembered across restarts (`LayoutMode` in state.rs, saved in settings.json). **A · Operator console** = transcript left, preview + working lists centre, tabbed dock right (default; unchanged for existing users). **B · Dock-first** = transcript hidden, wider dock for media-heavy services. **C · Focus** ("simple mode") = big preview, transcript on the RIGHT, no dock and no MEDIA bar — fewest distractions for a nervous first-timer or a small window.
- **The transcript is the blue/green panel**: deep blue-to-green wash, teal heading, a pulsing red dot while listening. Each line gets a 3 px edge: **green = a scripture was detected**, **blue = a voice command**, plain = ordinary speech (`LineKind` in state.rs, set in `handle_parse`; a verse beats a command when an utterance has both).
- **Nine display-colour swatches under the preview** (same presets as the 🎨 Colours menu): one tap puts Black/white, Deep navy, Dark green, Maroon, Charcoal, Royal purple, Warm paper, White/black or Black/yellow on the screen; the preset carries its own readable text colour; the chosen pair is remembered. The custom colour picker stays in the Colours menu.
- **Index de-duplication**: the timestamped index + honesty stats now live ONLY in the Index tab; the middle panel keeps the working lists (Planned + As they speak). Before, the same index grid was painted in both places.
- **Preview size fix found by the probe**: at short window sizes (1100x700) the stage preview could run a few pixels into the Scripture panel, because it was sized from the scroll content instead of the space actually visible. It is now capped by the visible area (`center_preview`); the bigger windows are pixel-identical.
- Tests: 6 new (`layout_keys_round_trip`, `transcript_line_kind_defaults_to_plain`, and 4 headless probes in `tools/sandbox-check/v0831_probe.rs` that run real frames: transcript kinds colour-coded; A/B/C paint the right panels; the layout is saved and unknown keys fall back to A; the index appears exactly once). The layout probe (`ux_probe`) now passes at 1512x900, 1440x820, 1280x720 and 1100x700 — the 1100x700 case needed the preview fix above. **196 app tests pass** in the Linux sandbox (Rust 1.99, eframe stub), plus all other crates (rf-core 26, rf-bible 12+16, rf-service 135+..., rf-media, rf-strongs).
- NOT seen on a real Mac window (the sandbox renders headlessly). Next from the top-20: Simple/Pro polish of C, Safe mode, next-verse prediction, confidence meter, lower-thirds.

## 0.830.0 - UI/UX overhaul, round 1 (live controls, layout, readability, Cut/Fade)

Found by LOOKING at the real UI: a new `tools/sandbox-check/shot_probe.rs` rasterizes egui's own output to a picture (rough, software, no Mac needed; NOT pixel-exact). Problems seen and fixed:

- **Live controls had no hierarchy** (about 25 same-size small buttons). Now a big first row: Start/End service, Record, Verse back / Verse forward, Blackout (turns solid red with "BLACKOUT ON"), Clear, Open stage, Full screen. Settings (chapter, screen, colours, style, transition, translation, zoom, read aloud) are smaller below.
- **Middle area text was cut off and overprinted** (a scroll area inside a scroll area). One scroll now. On wide windows the stage preview sits LEFT and the scripture lists RIGHT; narrow windows stack them. "Planned" and "As they speak" stack when the column is narrow (they overprinted each other before).
- **Preview is larger** and has a badge: ON SCREEN (green), BLACKOUT (red), or "stage window is closed". It is sized to the visible middle, so it cannot run into the panels below (checked at 1512x900, 1440x820, 1280x720, 1100x700).
- **Readability**: a real type scale (small text 12 px, was 9), lighter muted grey, section labels brighter. Side panels are narrower on small windows; Scripture and Media panels start shorter.
- **Cut / Fade / Slow fade**: new "Fade" menu on the settings row (Cut = instant, Fade = 350 ms, Slow fade = 900 ms). Remembered. Used by the stage window too.
- Tests: 1 new (fade maths); the layout probe now accepts a 150 px minimum preview. 184 app tests pass in the sandbox.
- NOT seen on a real Mac window. Round 2 candidates: stage monitor for the preacher, themes, an emptier-looking Media panel when there are no files, a LIVE badge in the header.

## 0.829.0 - disk-space check, Stop button

- **"No space left on device" during the first build** (reported from a real Mac): the disk was full. `run.sh` now checks for about 6 GB free before building. If short, it deletes the build files (`target/`) of OLD `RhemaFlow-D-v*` folders next to this one, then Cargo's download cache, and if that is still not enough it stops with a plain message and quick ways to free space. Nothing of yours is touched (only regenerable build files). Logic of the cleanup loop tested; the whole script NOT run on a real Mac.
- **Stop button** on the play bar: stops the video or sound and clears the screen (`Present::media_stop`). Pause still keeps the picture. 1 new test; 182 app tests pass.
- NOT done: UI/UX overhaul, stage monitor for the preacher, themes and transitions. (A 350 ms text fade-in already exists in `stage.rs`; there is no Cut/Fade choice yet.)

## 0.828.0 - "When it ends" for video and sound

- **Right-click a video or sound on the MEDIA bar -> "When it ends"**: Nothing (stay as it is), Clear the screen, or Show next: any picture, video or sound on the shelf. Chains work (video -> video -> picture), and a sound can hand over to the next sound, which makes a simple walk-in playlist. It does not happen while Loop is on. The play bar shows what is chosen. Saved per file in the settings (`after_end`), so it survives a restart. Code: `AfterEnd` and the hand-over in `tick_at` in `present.rs`.
- Tests: 2 new (rule saved and read back; real clip ends -> clear / next picture / nothing / looping). 181 app tests pass in the sandbox. NOT tried in a real Mac window.
- NOT done: the UI/UX overhaul and the rest of the top-20 list (themes, transitions, Windows app, stage monitor, MIDI, roles ...). Those are big separate jobs; pick the next one in `docs/UX-DESIGN-REVIEW.md` order.

## 0.827.0 - run.sh really asks nothing; play bar for video and sound

- **`run.sh`**: the "Do you want to proceed with the installation? [y/n]" question came from Homebrew itself (newer Homebrew asks it), so the script's own auto-yes never reached it. `run.sh` now answers Homebrew's questions too (ffmpeg and cmake installs). Tested against a stand-in `brew` that asks the same question. `./run.sh --ask` brings the questions back. NOT run against the real Homebrew here.
- **Play bar** (under the MEDIA bar, for any video or sound on the stage): Restart, Back 10s, Play/Pause, Forward 10s, a bar you can drag, time played / length, time left, the clock time it ends (`ends 10:42:05`), and Loop. Code: `transport_ui` and `Transport` in `present.rs`.
- **Jumping in video** is built in (`Player::seek` / `skip`, `PlaybackClock::seek`), works while playing or paused. **Jumping in sound** needs ffmpeg (afplay cannot start part-way): ffmpeg cuts a copy from the new time (mp3 is copied, instant; other formats become AAC) and afplay plays that. Without ffmpeg the jump buttons are greyed with a note; Play, Pause and Restart still work. Sound length comes from the Mac's `afinfo`.
- Tests: 6 new in the app, 2 new real-clip tests in `rf-media/tests/player.rs`. NOT seen or heard on a real Mac. The mp3 failure and the "mp4a box" mp4 failure are still not reproduced (need the Terminal text / `~/.rhemaflow/last-video-problem.txt`).

## 0.826.0 - run.sh asks nothing, installs ffmpeg itself; layout and mp4 fixes

- `run.sh` now answers "yes" to its own questions (model download etc.) and installs ffmpeg through Homebrew when missing. Use `./run.sh --ask` to be asked again.
- Media bar: long messages get their own line (they printed over the buttons before).
- Layout: the middle area scrolls instead of drawing over the panels below; the Scripture and MEDIA panels start smaller on short windows; minimum window is now 1100x700. Checked headlessly at 1512x900, 1440x820, 1280x720, 1100x700 (`tools/sandbox-check/ux_probe.rs`); NOT seen on a real Mac.
- MP4: sound tracks the parser cannot read are ignored even when the track's handler is unreadable. If a file still will not open, `~/.rhemaflow/last-video-problem.txt` holds an outline of the file to send for a fix. Cause of the reported "mp4a box ... larger size" file is NOT confirmed (no copy of the file here).

## 0.825.0 - service order, sound files, automatic converter

- **Service order** (right panel, "Service order" tab): drag from the MEDIA bar or right-click "Add to service order"; Songs tab "+ Service order"; "+ What is on the stage"; Next / Back / Clear / Black / Undo; lock; reorder/remove; crash-safe save (`service-order.json`). Code: `crates/rf-app/src/order.rs`. NOT seen on a real Mac window.
- **Sound files**: mp3, wav, m4a, aac, aiff, caf, flac play through the Mac's `afplay` (play/pause/restart/loop; mic ignored while playing). A failing player is now reported in plain words (`sound.rs`). NOT run on a real Mac.
- **Automatic converter** (`rf-media/src/convert.rs`): when ffmpeg is installed, HEVC/iPhone mp4, .mov/.mkv/etc. become H.264 mp4 and ogg/opus/wma become m4a, in the background, at add time. Tested with real ffmpeg. ffmpeg is started as a separate program, never bundled. `RHEMAFLOW_NO_FFMPEG=1` turns it off.
- Tests: `tools/sandbox-check/order_probe.rs`, `rf-media/tests/convert.rs`, new tests in `present.rs`, `order.rs`, `sound.rs`.
- Known gaps: no rename and no Logo button in the order; sound-only item leaves the audience screen blank; stage preview is squeezed on windows of about 1280x720 or smaller (next job).

## 0.824.0 - a faster, simpler MEDIA bar (design review in docs/UX-DESIGN-REVIEW.md)

1. **Only visible rows are drawn** (virtual grid) so hundreds of files stay smooth; thumbnails are made only for what you can see (test: 300 files, under 90 thumbnails).
2. **Adding files runs on a helper thread** (`library::prepare` / `commit`, `Present::queue_job`) with a progress bar; the window never freezes on a big video. Duplicate files are still recognised. The app turns this on (`bg_adds`); tests keep adding instant.
3. **Search box, thumbnail size slider, grid/list toggle, sort** (order added / newest / name) - all remembered.
4. **ON SCREEN badge** on the item showing now; **arrow keys + Enter/Space** move and show (mouse over the bar, nothing being typed).
5. Right-click on empty space for New folder. Drag items onto folder chips; drag files or whole folders from Finder.
- Tests: rf-app 160, rf-service 12 library tests (prepare/commit/progress/sort), all other suites pass. Not drawn in a real window here.

## 0.823.0 - the MEDIA bar is docked in the main window (no pop-up)

1. **Media is no longer a pop-up.** A dedicated MEDIA bar sits at the bottom of the main window (above the buttons, under the stage preview and scripture lanes). Drag pictures, videos or whole folders onto it; each appears as a thumbnail (videos show their length); click a thumbnail to show it on screen 2. Nothing else is needed. Folder chips along the top (All, your folders), right-click for New / Rename / Delete (files are kept), drag a thumbnail onto a chip to file it. "Add files..." opens the file chooser; "Options" holds picture size (Fit/Fill/Stretch), loop, mute, copy-or-link and the path box. Play/Pause/Restart show in the bar while a video is on. Fold it to one line with the small arrow. A gold frame appears when files are dragged over the window. Dropping files no longer opens any window.
2. "Media & slides" now holds only Slides, Songs, Background and Timers. The old "Pictures & video" tab was removed (a note points to the bar). The voice commands "open media"/"close media" open and fold the bar.
- Tests: all suites pass (rf-app 157, rf-core 135, rf-bible 26+12, rf-service, rf-media, rf-strongs); new test for the bar (open text, folded to one line). Never drawn in a real window here: check the look, the bar's height (drag its top edge) and drag and drop on your Mac.

## 0.822.0 - media like a file manager, video error fixed, scripture lanes, simulator removed

1. **"mp4a box contains a box with a larger size than it" fixed** (`rf-media/src/mp4fix.rs`, 4 tests). Cause: the file's SOUND track header was unreadable to the MP4 parser, which then refused the whole file although the picture was fine. Now, if the normal open fails, non-video tracks are relabelled "free" while reading (the file on disk is never changed) and the picture opens. The video's sound is unaffected because it plays through the Mac's own player. Checked on a copy of the test clip with a deliberately broken sound header (plain parser fails, repaired open plays all 60 frames). NOT checked on your Tanksage file itself: if it still fails, send me the new message.
2. **Media window = file manager** (`present.rs`, `rf-service/src/library.rs`, 9+3 tests). Folder list on the left (right-click: New folder / Rename / Delete folder - files are kept), thumbnail grid on the right with number, name and video length, blue border on what is on screen. Right-click an item: Show, Rename, Move to folder, Re-link, Remove. Drag items onto a folder. Drag files OR WHOLE FOLDERS from Finder (a dropped folder becomes a folder here, sub-folders as "Parent / Child", max 500 files, junk and non-media ignored). Empty folders are remembered. Window opens bigger (860x640).
3. **Scripture lanes** replace the Simulator (`ui.rs`): PLANNED (type or paste "John 3:16, Romans 8:28-30", click to show, up/down, remove, ticks, Next) and AS THEY SPEAK (everything shown this service, click to show again, "+ plan" to keep). Anything can still be typed in the search box: nothing has to be planned.
4. **Simulator removed from the screen.** The code behind it (`handle_parse`) stays because the tests and the phone use it.
- Not drawn in a real window here (no screen in the sandbox): please look at the media window, the folder right-click menus, dragging and the scripture lanes on your Mac.
- Tests: rf-app 156, rf-core 135, rf-bible 26+12, rf-service, rf-media (incl. 4 new), rf-strongs: all pass.

## 0.821.0 - screen 2 at 100%, corrections learned in the app, shelf on the phone

1. **Screen 2 not filling the screen.** Cause (likely; not reproducible without your Mac and projector): the app asked the window system for the screen's size ONCE and never looked at what it got. Mixed screens (a Retina laptop plus an external screen) use different pixel densities, and a window opened on one and moved to the other can come out at the wrong size, such as 80%. Fix: `displays::corrected_size/corrected_pos` (3 tests) + a loop in `app.rs`: after asking, read the window's real size and position, ask again with the difference built in (up to 8 times), then show "Stage window: W x H at X,Y. Fills the screen." next to the Screen picker (or "tried 8 times" with the numbers). The full-screen stage is also set always-on-top so the Dock and menu bar do not cover it. If it still happens, send that line.
2. **Learning from corrections is now in the app.** Double-click any transcript line, edit, Enter. The changed words are remembered (`corrections.tsv` next to settings.json); after the same fix twice it is applied to new lines automatically before detection; confirmed names are added to Whisper's prompt (`rf_asr::engines::set_extra_prompt`, 1 test). The Whisper-side edit is inside `#[cfg(feature = "native")]` and was NOT compiled here (no whisper.cpp/cmake in the sandbox): build on the Mac.
3. **Shelf**: video preview thumbnails (first non-black frame, files up to 150 MB, one per frame), Rename and Move-to-shelf under the "..." button on each item.
4. **Phone remote shows the media shelf** (`/media`, PIN needed; the phone may only send `media_<number>`): tap a name to show it. Missing items are greyed out.
- Not done: a progress bar while a very big file copies (the window waits), and the shelf is not in the web stage page (pictures are never sent to OBS by design).
- Tests: rf-app 152, rf-core 135, rf-bible 26+12, rf-service, rf-media, rf-strongs, rf-asr prompt 1: all pass in the sandbox.

## 0.820.0 - the media shelf (add once, click the thumbnail to show)

Pictures and videos are added ONCE to a shelf, then shown by clicking their thumbnail (Media & slides window, Media tab).
1. **Copy or link, chosen for you.** `crates/rf-service/src/library.rs` (8 tests). Default "auto": files up to 300 MB are COPIED into `media-library/files/` next to the app's settings (unplug the USB stick, rename or move the original, it still shows); bigger videos are LINKED (no extra disk). Options: always copy / only link. The same bytes are never stored twice (fingerprint), even under another name.
2. **Missing files are caught before Sunday**: a linked file that moved shows "missing", a warning line counts them, and **Re-link** accepts only the same file (fingerprint check). Remove deletes only the shelf's own copy, never an original.
3. **Shelves** (folders), filter box, rename/move in the logic (rename and move buttons are not drawn yet).
4. **Grid UI** in `present.rs`: thumbnail cards for pictures; videos show a play badge (no video preview frame yet). Old media lists are adopted as links on first start. Showing still goes through the same `stage_media`, so video, sound, loop and fit behave as before.
5. **Bug found by the tests and fixed**: copied files were labelled on the stage with their internal fingerprint file name; the stage now shows the real name.
- Tests: whole workspace in the sandbox (rf-app 146, rf-service 135 + 8 new, rf-core, rf-bible, rf-media, rf-strongs): all pass. Not tried on a real Mac window: the look of the grid, drag and drop onto the shelf, the file chooser, a 300 MB copy taking a few seconds (the window waits while it copies; no progress bar yet).

## 0.819.0 (Web 0.5) - learns from corrections, sample sermon, new landing page

1. **Learning from corrections** (`crates/rf-core/src/learn.rs`, 9 tests, compiled and run with rustc 1.75). When the operator fixes a transcript line, only the CHANGED words are remembered ("adaoyin" -> "Adeoye"). After the same fix is made **twice** it is applied to later lines automatically (whole words only, never digits or verse numbers, never words under 3 letters, capitals-only changes ignored). Confirmed names also come out of `prompt_words(max_chars)` for Whisper's prompt. `to_text`/`from_text` save and load a plain file; `forget` undoes one fix.
   - **NOT wired yet**: the egui "edit this line" box, the file location, and passing `prompt_words` into `DOMAIN_PROMPT` in `rf-asr/src/engines.rs` (it is a constant now; needs to become per-session). Not run inside the full app (no Mac / no Whisper here).
2. **Web demo 0.5**: "Try a sample sermon" button (no microphone; pretend sermon through the real detector; checked in real Chromium: John 3:16 reaches the stage, Romans 8:28 waits for review), better page title and description.
3. **Landing page** `site/index.html` (+ draft `privacy.html`, `terms.html`) and `web/make_deploy.py`, which builds `deploy/`: landing at `/`, app at `/app/`. Cloudflare: build command `exit 0`, output folder `deploy`. Replace `YOUR-DOMAIN.com` and add `og.png`.
- Tests: web core 23 + pwa 6 pass; Rust learn 9 pass. The full Rust workspace was NOT rebuilt here (Cargo.lock not regenerated: run `cargo build` on the Mac).

## 0.818.0 - microphone check

1. **`mic_report`**: `cargo run --release -p rf-asr --example mic_report -- recording.wav` listens to a recording and says, in plain English, what is wrong with the SOUND: no voice (muted/unplugged), too quiet, too loud, clipped, voice too close to the background noise, mains hum (50 or 60 Hz) and a constant offset. Each finding has a fix. Hum is judged by how loud it is **next to the voice** (a hum 35 dB under the voice is harmless and is not reported). Badly clipped audio says "distorted", not "no voice". `docs/MIC-CHECK.md` also has the ranked top 5 ways to improve accuracy and which are already done.
2. Tests (12, real speech plus broken copies): quiet, clipped, noisy, hum at 50 and 60 Hz at three loudness levels, offset, silence, hiss, too short, NaN/inf input, odd sample rates. An independent FFT confirmed the old test recording genuinely has 60 Hz hum (so the check had to leave it alone). I broke the rules on purpose 3 ways and every break was caught. Two of my own first assumptions were wrong and are fixed: I thought the old recording was quiet (it is loud), and a loud hum made the code decide there was no voice.
- Not verified: your real room and microphones; the rule-of-thumb limits (-20 dB voice, 25 dB above the background, 32 dB hum margin) are not calibrated on your church. No in-app button yet (command line only).

## 0.817.0 (Web 0.4) - the web app is now an installable, offline-capable PWA

Desktop app unchanged apart from the version number. Web only:

1. **Install it like an app** (Chrome/Edge: address-bar install icon or the new **Install app** button): manifest, 192/512/maskable/180 icons (a gold serif R, our own mark, `web/make_icons.py`), theme colour.
2. **Works offline after the first visit.** A service worker keeps the app (the KJV is inside the page) and any Bible version fetched from `bibles/`. It **never swaps itself mid-service**: a new build waits until every RhemaFlow window is closed. The cache name comes from a hash of the app, so every change gives a fresh copy.
3. **`web/build.py` now also makes `web/dist/`**, the folder to publish (`index.html`, manifest, icons, worker, `_headers`, public-domain Bibles). Commit it; Cloudflare then needs only build command `exit 0` and output folder `web/dist`. Steps and honest limits: `docs/DEPLOY-CLOUDFLARE.md`.
4. Tests: 6 Node tests (routing rules, manifest really matches its icons, every cached file exists, cache name follows the app) and 14 real-Chromium checks: worker takes control, **Chromium reports no installability errors**, and with the server **shut down completely** the app still opens and a previously fetched Bible is still there (a control browser without the saved copy cannot open it). My first version of the offline test could have passed for the wrong reason (Playwright's "offline" switch does not reach a service worker's own requests), so it now stops the server; I broke the worker on purpose and the test fails as it should. Existing web suites unchanged: 23 + 14 + 33.
- **Not verified:** Cloudflare itself, a real phone, iOS Safari, the real `pages.dev` microphone permission, and your GitHub repository (I could not see it; the plan I was shown describes different file names).
- **Offline honesty:** the web version's live voice uses the browser's speech service, which in Chrome needs internet. Only the Mac app listens fully offline.

## 0.816.0 (desktop) - garbled quotations, 20+ new voice commands, Greek and Hebrew (Strong's)

Found from a real session on the operator's Mac (screenshot, 8 Oct 2026). Tests first, with the operator's exact words.

1. **A verse said in short garbled pieces is now found.** The speech engine cut 2 Corinthians 5:17 into nine pieces ("If any man being Christ." "He is." "A new." "Creation" "Old things." "I'm passed away." "Behold." "All things." "I'll become new."). Two causes: (a) the matcher only recognised a verse by its OPENING words, so wrong words ("being" for "be in") broke it; (b) the "join recent pieces" rescue kept only the last 2 pieces from the last 2.5 seconds, but the pieces were 2 to 7 seconds apart and a flagged piece ended the search. Now: up to 14 pieces from the last 45 seconds are tried (every "last k pieces" window, so junk before does not spoil it), candidates come from word triples ANYWHERE in a verse, and scoring forgives near-spellings ("creation" for "creature"). It must still match 6+ words in order with 3 together and score 60%+. In the real app, your nine pieces put 2 Corinthians 5:17 on the screen by the fourth piece. Ordinary talk, a quote buried in chatter, 5 words, and a very common opening all stay quiet (12 matcher tests; I broke the code on purpose 3 ways and every break was caught).
   * One piece on its own must score 80%+ (it is the "a few words wrong" case: "If anyone be in Christ..." now works alone). **A quote in another translation's wording is still offered as that translation, never as a loose KJV guess**: the matcher also tries the active translation and the better one wins and is labelled.
   * Pieces that already produced a verse are spent, so they cannot bring it back a second time.
2. **Translation by voice.** "in the KJV", "KJV", "KJV translation", "K J V", "k.j.v.", "king james version", "authorized version", "basic english", "B B E", "A S V", "american standard", "young's literal", "darby", "webster", "rotherham", "A K J V", plus NIV / ESV / NLT / NASB / NKJV / AMP (said clearly as "not installed"). Any installed translation now switches (before, only KJV and BBE). Last mention wins. Long names only count in short sentences, so a sermon about the King James Version changes nothing.
3. **"Read till verse 20"** (also until, to, through, up to, down to; "verse" optional, numbers as words or digits) reads aloud from the verse on screen to that verse and shows the range. "Read the chapter / to the end / the rest" reads to the end of the chapter. It no longer also stages "verse 20" as a new reference.
4. **20+ more voice commands** (all short-sentence only): black out / screen back on (new words, so "go black" still means Clear), next / previous translation, final verse of the chapter, show/hide history, show/hide Bible browser, open/close Media & slides, dismiss all review cards, stage look (lower third, green screen, large print, classic), put the stage on screen N, start an N-minute countdown, pause / resume / stop timer, show timer, show clock, show/hide Strong's, "what is the Greek for love". List: `docs/VOICE-COMMANDS.md`.
5. **Greek and Hebrew (Strong's).** New **Strong's** button and window. Way 2 (English word to original word, "what is the Greek for word" gives logos then rhema) works now, offline, from a built-in **CC0** Greek dictionary (5,624 entries) with about 50 first-pick words (love, word, spirit, faith, grace, peace...). Way 1 (press a word of the verse on screen, get its exact original word) is built and tested on sample data and switches on when a Strong's-tagged KJV file is installed; **I could not get a tagged KJV or confirm a Hebrew licence here, so neither is in the zip** (`tools/fetch_strongs.py`, `docs/STRONGS.md`). "Show on stage" puts the chosen word on the screen.
6. Removed the names of other programs from code comments (clean-room rule).
- Tests: **400 Rust** (was 341): rf-core 126, rf-bible 38, rf-app 145 (includes 4 sandbox-only end-to-end probes that speak to the real app), rf-service 54, rf-media 21, rf-strongs 16; 1 ignored as before. Web 23 + 14 + 33 (unchanged).
- **Not verified (needs your Mac):** how the new Strong's window looks; the real garbled-speech behaviour with your voice (I used your nine pieces as text, not audio); Way 1 on the real CrossWire KJV; the Hebrew licence; the SWORD export steps in STRONGS.md (written from memory of those tools).

## 0.815.0 (desktop) - Stage 2: pictures, video, slides, songs, backgrounds, timers

Spec first (`docs/STAGE2-SPEC.md`, own words), then tests, then code. Everything below is the DESKTOP app; the web version already had pictures, slides and a countdown.

1. **Pictures and video on the stage.** PNG, JPEG, GIF, BMP and MP4/M4V with H.264. Fit / Fill / Stretch, Loop, Mute, Play / Pause / Restart (buttons and voice: "play video", "pause video", "restart video"). Video plays on its own clock, so a slow computer drops frames instead of falling behind; with Loop off the last frame stays. Files it cannot play (MOV, MKV, WebM, HEVC inside an MP4, WebP, HEIC...) are refused with a sentence that says what to do (convert with HandBrake). A damaged or cut-off file gives a message and never touches the stage.
2. **Video library chosen (permissive only):** `openh264` (BSD-2-Clause, Cisco's source) + `mp4` (MIT) + `image` (MIT/Apache). No GPL/AGPL/LGPL/MPL anywhere in the new tree (`THIRD-PARTY.md`). FFmpeg was rejected (LGPL/GPL depending on one build flag). **Decision needed from you: H.264 is patented**; see `THIRD-PARTY.md` before you sell.
3. **Video sound** is played by the Mac's own `afplay` (so no MPL sound library is needed). While a video with sound plays the microphone is ignored, like while reading aloud, so the film's voices cannot be mistaken for the preacher. **Untested on a real Mac.**
4. **Slides and lyrics.** Type text (blank line = new slide, "Chorus:" names a slide for you; the congregation never sees the name) or drop a `.txt` / `.pptx` on the window (PowerPoint words only, slides in number order, one line per paragraph). Next / Previous by button, by the arrow keys and "next" / "next verse" (while a slide is up), and "next slide" / "previous slide". Text never leaves the screen at any zoom (tested at 60 % to 400 %).
5. **Song library.** Title, author, parts with names, and an order sung ("Verse 1, Chorus, Verse 2, Chorus"), search by title or any words from the lyrics, long parts split into pieces (lines per slide), import from `.txt`/`.pptx`, saved safely to `~/.rhemaflow/songs.json` (a damaged file is kept aside as `songs.json.damaged`, never thrown away). **No song words are included**; the Songs tab reminds the church about its licence (CCLI in the UK).
6. **Background loops.** A picture or a silent looping video behind slides and timers (never behind scripture), a Darken slider (0 to 85 %) and a soft shadow on the words.
7. **Timers and clocks.** Countdown, count down to a clock time (never wraps to tomorrow), stopwatch, clock (24 h / 12 h). Rounds up like the web version so 00:00 means really over; warning in the last minute for the operator only (nothing flashes for the congregation). A timer keeps running when you show a scripture.
8. **OBS page** shows slides and countdowns as text (lines kept). Pictures and video are not sent to the OBS page in this version.
9. **New window "Media & slides"** (button next to Clear): five tabs. Drag files onto the window to add them.
10. **Bugs found while building (all fixed, with tests):** (a) video frames came out of the decoder in file order, not display order, so a video with B-frames (almost every real video) would have played jumbled; found with a real x264 clip, fixed with a reorder buffer; (b) the `.pptx` reader never saw closing tags; (c) my first "Fill is clipped" test could not fail, so I rewrote it. I then broke the code on purpose in five places (no text shadow, scripture getting a background, a video not stopping when it leaves the stage, a countdown not telling the web page, a picture not clipped) and every one is now caught by a test.
11. Version strings fixed (`--help` text said v0.801.2). `THIRD-PARTY.md` added (asked for in `LEGAL-AND-CLEAN-ROOM.md`). `crates/rf-media/examples/decode_speed.rs` measures decoding on your Mac.
12. **Measured here (1 slow Linux core, not a Mac):** a 1080p 30 fps clip decodes at 112 fps and at 58 fps including turning frames into pictures (1.9 times real time). An M1 should do better; run `decode_speed` to see.
- Tests: rf-core 110, rf-bible 26, rf-app 130 (1 ignored as before), rf-service 54, rf-media 21 = **341 Rust** (was 240); web 23 logic + 14 page (jsdom) + 33 real-Chromium checks (unchanged).
- Real files used in tests: an x264 clip with B-frames and an AAC sound track, an x265 clip, a 12-slide `.pptx` made by python-pptx.
- **Not verified (needs your Mac):** how any of it looks in a real window or on screen 2; real video smoothness; the OpenH264 build on Apple Silicon (the first build downloads and compiles new crates, it needs internet once); `afplay` sound and the mic muting; the Mac file chooser and drag-and-drop; `.pptx` files from real PowerPoint / Keynote / Google Slides; interlaced, 10-bit or odd phone videos.
- Not done (said plainly): the saved **service order** (the model and live runner exist in `rf-service` but are not connected to the screen), choosing a different sound device for video, transitions, video on the OBS page.

## 0.814.0 (desktop) / Web 0.3 - screen 2, colours, scripture history, show/hide, bundled Bibles, media

1. **Output to screen 2 (desktop).** The stage is now a BORDERLESS window sized exactly to the chosen screen (screen 2 by default when one is plugged in; remembered by name), instead of macOS native full screen. That removes the separate-desktop (Space) behaviour that made "exit full screen" flaky: leaving is just putting the window back. Screens are found with a one-line `osascript` call; Cocoa's bottom-left coordinates are converted to top-left (tested for screens to the right, above and left). **Not verified on a real second monitor here** (no Mac): please try it and tell me what you see.
2. **Background and text colours** (desktop, web, OBS page): 9 presets (all pass a 7:1 contrast test) + pickers; changing the background picks readable text; gold reference line turns dark on light backgrounds.
3. **Show AND Hide for the Bible panel.** Hiding leaves a slim "Show scripture" bar (with History and the search hint); nothing is lost.
4. **Scripture history.** Every scripture shown is kept in order. "Previous scripture" / "the one before" steps back, again and again; "next scripture" goes forward (or next verse when already at the newest). Reading along with next verse inside a chapter moves ONE entry instead of flooding the list. History window (click to jump), keys [ and ], phone-remote buttons. Desktop (`history.rs`, 7 tests) + web.
5. **Bible versions bundled:** ASV, YLT, Darby, Webster, Rotherham, AKJV ship in `bibles/` (plus BBE/KJV built in) and load automatically; your own folder wins. Wycliffe and Douay-Rheims were removed from the download list: their books do not fit the 66-book layout. Noyes is a partial translation and is not bundled.
6. **Web 0.3:** pictures and video on the stage window (Blob sent between windows; play/pause/restart/loop/mute, voice "play video"), slides (blank line = new slide, "Chorus:" labels), countdown, "Screen 2" button (Window Management API; honest fallback), colours, history, show/hide bar, Bibles from the `bibles/` folder.
7. **Subscription rules** (`license.rs`, 7 tests): weekly check-in, 14 days offline grace, 5 days payment grace, clock-rollback detection, never cut a live service, free-mode fallback. Rules only, not yet connected to a server.
8. **Docs:** `LEGAL-AND-CLEAN-ROOM.md`, `LICENSING-AND-PRICING.md`, `ROADMAP-50.md`.
- Tests: rf-core 108, rf-bible 26, rf-app 91, rf-service 15; web 23 logic + 14 page (jsdom) + 33 real-Chromium checks.

## 0.813.1 (desktop) / Web 0.2 - "Next." fix, command upgrade, web layout

Found by reproducing your 5 Oct web screenshot in a real Chromium:
1. **"Next." and "Previous." did nothing.** Speech engines return exactly that for "next verse". Now a bare *next / forward / previous / back / prior / again / read / clear* is a command (it only does anything while a verse is on stage). Run-together and slipped words ("Netverse", "nextverse", "nex verse", "Previews") are understood by comparing the whole short utterance to every phrase. Fillers are ignored ("ok next please"). Applied to BOTH the web page and the desktop app.
2. **Safety found while testing:** "the next verse says something wonderful about grace" would have navigated. Navigation commands now only fire in utterances of 6 words or fewer (web + desktop). Also "go black" was being read as "go blank" (one-letter fuzz): short words must match exactly.
3. **Web: faster commands.** An unfinished (interim) result made only of command words is acted on once it has been stable for 450 ms (so "next" can still become "next chapter"); the final result does not repeat it. Scripture is never staged from interim text. A green flash shows what was understood; "Nothing on the stage yet" if you say next with nothing up.
4. **Web: references without a book** use the current one: "chapter four", "chapter 2 verse 4", "verse six to nine".
5. **Web: layout.** The stage preview was being squashed by the flex column (tiny text); it is now a true 16:9 box sized to the window. Header / main / scripture browser are a fixed grid so the browser is always visible; controls shrunk (zoom presets in a menu); nothing needs scrolling at 1440x900 and up, and the center column scrolls (never clips) below that. Checked in a real Chromium at 1181x706, 1000x640, 1440x900, 1920x1080.
6. **Web: the clock** said 01:04 next to IDLE; it now says "ended 01:04". The old "Microphone blocked" note no longer lingers. Opened as a `file://` page, Chrome refuses or re-asks for the microphone: the page now says so and how to fix it (`python3 -m http.server 8000`).
7. **Web: Commands button** lists every phrase (searchable), grouped by what it does.
- Tests: rf-core 107, rf-bible 26, rf-app 69, rf-service 15; web 18 logic + 14 page.

## 0.813.0 - layout, search, Bible versions, auto-approve fix, phone/OBS, web version

1. **Layout (screenshot 1).** The books list was drawn as a horizontal strip (the column inherited a horizontal layout), which pushed chapters and verses off-screen. Books, chapters and verses are now three real columns. Controls are compact: three text rows at 1190x770, rare items under *More*, zoom presets in a menu, 16:9 stage preview capped at 55% of the space, simulator collapsed. Checked by rendering the real UI headlessly at 1190x770, 1000x640 and 1440x900: nothing painted outside the window, books a vertical list, verse text in the third column.
2. **Search (screenshot 2).** One box: `1 sam` opens 1 Samuel (no error), `john 3` the chapter, `rev 1:6` the verse, `god so loved` finds John 3:16 first plus other relevant verses (phrase > all words in order > all words > most words; the last word may be a prefix; widely quoted verses nudged up). `/` focuses it, Enter takes the first result, Esc clears. Misspelt books (`revelations`, `efeziens`) still find the book.
3. **Bible versions.** Folder of files (json x3 layouts, tsv) read by `Bible::from_any`; one-click download of public-domain ASV, YLT, Darby, Webster, Wycliffe, DRC, Rotherham, Noyes, AKJV (tested on real ASV/YLT/Darby/BBE/KJV files); `tools/fetch_bible.py` for API.Bible (api path NOT run live). Matching stays KJV; display uses the chosen version and redraws live. See `docs/BIBLE-VERSIONS.md`.
4. **Screenshot 3 fixed.** Auto-approve was ON and "and have made us kings and priests" still waited: Revelation 1:6 and 5:10 tied at 73% because the score ignored word order gaps and penalised verses for words not yet spoken. Now scored on the words the speaker could have reached (+1) with a contiguity bonus (5:10 "...us unto our God kings and priests" no longer ties), and auto-approve needs 75% and 12 points ahead (was 90/15). Tested on the real parser. Also: an uncertain book guess inside a sentence ("those kings entreeced") no longer makes a "1 Kings" card.
5. **Top-20 features built**: keyboard control (Space/X/1-5/arrows/B/C/R/S/F), blackout, recent strip, sermon plan, stage looks (Classic, Lower third, Green screen, Large print), OBS browser-source page and PIN-protected phone remote (`web.rs`, std only, off by default, this Mac only unless Wi-Fi is ticked). Full list and status in `docs/TOP-20.md`.
6. **Web version**: `web/rhemaflow-web.html` (single file, KJV embedded). Same rules as the desktop app; 14 logic tests + 9 page tests (jsdom). Browser speech limits are stated in `web/README.md`.
- Tests: rf-core 105, rf-bible 26, rf-app 68, rf-service 15; web 14 + 9.

## 0.812.0 - kings-and-priests fix, date-named files, scripture browser, exit-full-screen watchdog

1. **"And have made us kings and priests..." now finds Revelation 1:6.** Two causes, both fixed. (a) The word "kings" looked like the book 1 Kings (54%), and that weak book-only card BLOCKED the quotation matcher - hence four "1 Kings" cards and no verse. A weak book guess (< 60%) no longer blocks a quotation, and is dropped when a quotation is found; a clearly spoken book name ("second Samuel") still gets its book card. (b) The KJV says "hath", the speech engine writes "have". Quotation matching now treats hath/hast=have, doth/dost=does, saith=says, thee/thou/ye=you, thy/thine=your, shalt/wilt/art, wherefore=therefore on BOTH sides. Tested on the real garbled lines from your screenshot ("Relations and have made us kings and priests", "...on to gods", "...those kings and priests unto God and his father"). With *auto-approve quotes* on, a lone clear match (70%+) now goes straight to the stage.
2. **Scripture browser** (`browser.rs`): books | chapters | verses, like ProPresenter's Scripture tab. It FOLLOWS the stage (any verse that reaches the stage by voice, quotation, card or next/previous selects its book, chapter and verse and scrolls it into view, highlighted) and works BY HAND (click a book, a chapter, then a verse to show it; Shift+click for a range). A typed-reference box accepts "rev 1:6", "john 3:16-18", "2 cor 5:17" and says honestly when a verse does not exist ("1 Corinthians 5 has only 13 verses"). Show/Hide button; remembered.
3. **Files named with the date and times.** Folder = the full date, e.g. `Sunday 4th October 2026/`. Files: `Sunday 4th October 2026 - 01h27m43s to 02h15m10s - recording.wav` (also `- transcript.txt`, `- transcript.srt`, `- sermon notes.md`, `- events.jsonl`, `- recording.m4a`). While live they say `- start 01h27m43s -` and are renamed when the service ends. Two services the same day share the folder and never overwrite each other. Uses the Mac's LOCAL clock (the old stamp was UTC, an hour out in UK summer). Stop then Record again no longer overwrites the first recording (`recording (part 2).wav`).
4. **Detailed transcript.** Opens with `Sunday 4th October 2026`, then start time to the second in 24 h and 12 h (`01:27:43 (1:27:43 AM) UTC+01:00 BST`), end time, length, title, church, software, speech engine, microphone, Bible shown, folder, recording files; totals (lines, words, scriptures by how they were shown, auto-staged, approved, dismissed, longest quiet stretch); the scripture index with the verse text; then every line as `[time into service | clock time]` with markers for quiet stretches of 30 s or more. Sermon notes (.md) has the same in tables. events.jsonl records carry `clock` and `date`.
5. **Exit full screen.** Cause not provable without your Mac; the likely one: the control window is on another desktop while the stage is full screen, and macOS ignores a leave request made mid-animation. Now: the app checks the stage window's REAL state, asks again if it did not leave, and as a last resort rebuilds the window (always works); Esc, F11, double-click, and a small "Exit full screen" button (appears when the mouse moves) all work inside the stage window; the control window keeps repainting while the stage is full screen.
- Tests: rf-core 104, rf-bible 22, rf-app 55, rf-service 15.

## 0.811.0 - layout width, stage voice control, saving, zoom 400%, stop-reading fix, ranked quotations

1. **Width**: the transcript panel is bounded (260-460 px, resizable) and every line wraps inside it, so one long quoted verse no longer widens it and squeezes the stage. The right panel is bounded too, and the bottom control rows wrap instead of clipping ("Stop recording" was cut off).
2. **Voice commands**: new data table `rf-core/src/cmdtable.rs` (single source of truth, 120+ phrases; list in `docs/VOICE-COMMANDS.md` and in the app under *Voice commands*). New: open/close stage, full screen / exit full screen, first verse, approve / dismiss / "option two" for the review queue, auto-approve on/off, show all/one match, auto-read on/off, read faster/slower, show/hide commands, many synonyms for clear, zoom, navigation, stop reading. Longest phrase wins and consumed words cannot trigger a second command ("close full screen" is not "clear stage"). "Short only" phrases cannot fire mid-sermon. BUG FIXED: "exit full screen" changed a flag but never left full screen; the stage window now gets full screen ON and OFF (also F11 / Esc inside the stage window).
3. **Saving**: a service folder is `<root>/<date-time - title>/`, root chosen under *Save settings* (default `~/Downloads/RhemaFlow`, or Documents, or any folder). *Open folder* button while live; a "Service saved" window after End service lists every file and size. Written: audio.wav (live), transcript.txt (live, crash-safe), transcript.srt, sermon-notes.md (scripture index with times + transcript), index.md, events.jsonl, optional audio.m4a (about 1/10 size, macOS afconvert). Also: unwritable folder falls back to Downloads instead of losing the service, low-disk warning (< 3 h free), settings remembered.
4. **Zoom** 60-400% (was 60-220). Voice: `zoom 300`, `max zoom` (400), `half max zoom` (200), `zoom in/out` (20% steps). Slider and 200/300/400 buttons. The stage shrinks text only as much as needed so a big zoom never pushes a verse off screen.
5. **Stop reading**: ROOT CAUSE - the mic heard the speakers, parsed the verse as new speech, and auto-read started it again; also the "speaking" flag never cleared when a verse finished on its own. Now: the mic is ignored while reading (only "stop" is listened for; switch off with *ignore mic while reading* for headphones), the flag clears itself, Stop is always visible, stops at once even with auto-read on and holds auto-read off for 4 s. Key S, voice "stop", "stop reading", "be quiet", etc.
6. **Quotation approval setting**: *auto-approve quotes* checkbox (voice: "auto approve on/off"). Auto only when unambiguous (one verse fits, or best >= 90% and 15 points ahead); otherwise it waits for you. Default OFF.
7. **Many matching verses**: "the grace of our Lord Jesus Christ..." now lists every verse that fits, best % first (each tappable; voice "option two"), including phrases said from the middle of a verse. *show all matches* checkbox switches to best-only. `Bible::match_quotations_ranked` (word-order-aware score).
- Also: one pending "book only" card per book (four identical "1 Kings" cards before).
- Tests: rf-core 101, rf-bible 20, rf-app 40, rf-service 15.

## 0.810.0 - live text that never rewrites + R2T2 primary with Whisper backup

- **`rf_core::LiveText`** (LocalAgreement-2): the live strip shows permanent (solid) words only after two consecutive decodes agree; undecided words are dim; permanent words are never revised during an utterance. Append-only mode for streaming engines. Tested on 14 REAL recorded Whisper interim decodes (`jfk-interims.jsonl`). Known cost: a word the model gets wrong twice stays wrong; `live_disagree` events record when the final text differed.
- **Failover** (`failover.rs`): with `--stream-url`, the built-in Whisper is a standby that only works while the stream is down or silent through real speech (2 s grace); notices tell the operator which engine is active.
- **`--stream-url auto`**: starts `~/RhemaFlow-R2T2/run_sidecar.sh` if installed and not already running; the sidecar exits when the app ends (`--parent-pid`).
- Scorecard updated only where justified: live words 2 -> 4 (ours), failover 4 -> 5 (combined). Listening quality: ours 52 -> 58; whole job: ours 74 -> 77, combined 82 -> 84.
- Tests: rf-core 90, rf-app 26, rf-asr 44 lib + 4 real-speech (+2 slow Whisper tests run on demand), rf-service 15, sidecar 11.

## 0.809.0 - same-audio comparison tools (R2T2 vs ours)

- `benchmark --stream-url URL`: drives a streaming service (R2T2 protocol) with the same audio, reports words correct, first-word time and last-word-after-end.
- `benchmark --mix-noise DB / --mix-music DB`: identical noisy/music-bed audio to every engine.
- `compare.sh`: clean / noise / music runs on your own recording.
- `rf_asr::testsig` (noise, chord-bed music, mix-at-SNR; SNR verified to 0.05 dB).
- `docs/COMPARISON-R2T2-vs-RhemaFlow.md`, `docs/comparison.png`, `docs/comparison_data.json`, `tools/make_comparison.py`.
- VAD keeps 200 ms of trailing audio per utterance (word-final sounds like the 's' in "Americans" were trimmed): quiet-mic accuracy on the test clip 95.5 -> 100%.
- New measured results (Whisper tiny.en, one clip): noise 20/10/5 dB 95.5%, music 20/10/5 dB 100%.

## 0.808.0 - "never stops listening": level-proof, noise-aware, continuous

Cause found by running real speech through the voice detector: a fixed loudness rule dropped soft words as "silence" and chopped sentences (43-47% of speech lost at a very quiet mic, depending on the version measured). Fixes:

- **AGC** (`rf-asr/agc.rs`): lifts quiet voices up to +24 dB, never attenuates, ignores steady noise/music (learns only from speech-like wobble). `audio.wav` stays raw. `RF_AGC=0` turns it off.
- **VAD hysteresis**: staying in speech needs less than starting it.
- **Steady-background learning**: a fan/music bed is learned as the floor, even mid-utterance.
- **Soft cut**: continuous speech is cut at its quietest 10 ms frame every ~12 s, so a 60 s monologue becomes ~6 full transcripts with nothing lost (was: one 30 s hard chop).
- Level meter shows the real mic level (not the levelled audio).
- macOS: `caffeinate -d -i -m -w PID` keeps Mac and display awake; `ui_stall` events logged when the window is not drawn for >1.5 s.
- Sidecar: resets at a breath after 8 s (hard limit 16 s) instead of mid-word; sends book names as hints (`sidecar/context_books.txt`).
- Measured (JFK clip, Whisper tiny.en, CPU): word accuracy 86.4 -> 95.5% (quiet mic), 50.0 -> 95.5% (very quiet mic), 100% unchanged at normal level.
- Honest limit: speech 10 dB over a loud music bed keeps ~87-92% (87% at 0.808.0, 92% after the 0.809 tail fix); steady-noise handling is energy based.
- docs/ALWAYS-LISTENING.md (diagnosis, results, steps for a 9-year-old).

## 0.807.0 - streaming engine path (Confucius4-R2T2 protocol) + Mac sidecar

- **`--stream-url ws://127.0.0.1:8272/asr_stream_api_v1`**: use a local streaming-ASR service that speaks the Confucius4-R2T2 WebSocket protocol instead of Whisper/Parakeet. Whisper stays the default and the fallback.
- `rf-asr::stream_client`: WebSocket client (int16 PCM frames, EOS/FLUSH, reconnect with backoff, drains the server's last words on close).
- `rf-core::CommitTracker`: append-only consumer. A finished reference/command is released ~350 ms after the last word (unfinished ones never early); decoder-loop guard.
- `rf-app::stream_bridge`: wires the above into the existing parser/transcript UI.
- `sidecar/r2t2_server.py` (+ `setup_mac.sh`, tests): protocol-compatible server for Apple Silicon around R2T2's official llama.cpp backend; adaptive chunk size (160-960 ms) from measured cost; stand-in for the `vllm` import that breaks R2T2 on macOS.
- docs: `R2T2-ON-M1.md` (how it works, what is needed, M1 estimate, licence), `PROMPT-SPEECH-ENGINEER.md`.
- Tests: rf-core 83, rf-app 20, rf-asr 36 (35 + 1 cross-language, run manually), rf-service 15, sidecar 6.
- NOT verified: the real R2T2 model, Metal build on macOS, live mic, real latency/accuracy.

## 0.806.0 — faster mic path + calibrate (speak -> text, measured on your voice)

- **Semantic endpointing**: the ASR worker asks the Scripture parser how long to wait in silence (finished reference ~350 ms, unfinished up to ~1.1 s, commands 300 ms, ordinary speech unchanged). `rf_core::end_hint`, `rf_asr::vad::EndHintCell`.
- **Decode during the pause + reuse**: a >=100 ms pause triggers one full-quality decode; if no more speech arrives it is the final transcript (no second decode). `Utterance` gained `pause`, `seq`, `speech_frames`.
- **Adaptive interim cadence**: first interim after ~1.0 s of audio; step 0.4-1.0 s based on measured decode time.
- **Personal lexicon** (`rf_core::Lexicon`), loaded from `~/RhemaFlow-Services/lexicon.tsv` and applied before parsing (display text untouched).
- **`rhemaflow-d calibrate`**: read 30 pulpit sentences (or `--wav-dir`), reports word accuracy, held-out before/after, decode time p95; saves the lexicon.
- **LocalAgreement** helper (`rf_core::Agreement`) - library only, not yet driving actions.
- Timeline simulation test (`latency_sim`) and docs/MIC-PIPELINE.md.
- Tests: 161 pass across the workspace (was 135 in 0.805.0): +14 rf-core, +9 rf-asr VAD, +3 rf-app simulation, incl. the real whisper tiny.en smoke test on Linux.

## 0.805.0 — Phase 1 foundation begins (new crate `rf-service`)

Added `crates/rf-service`, pure logic with no UI dependency (see docs/ROADMAP.md, Phase 1):

- **Service model** — ordered items: scripture (reference only, never translation text), text slides, song (slot for Phase 2), media, timer. Theme v1 with validation. Stable item ids.
- **Live runner** — Next/Prev flow across slides and items (empty items skipped, no wraparound), **Clear / Logo / Black** panic keys, `restore_content`, 10-deep **undo**, **Edit/Show mode** lock (structural edits refused while live; driving the show always works). Live position is keyed by item id, so reordering or deleting other items cannot change what is on screen; deleting the live item clears the screen.
- **Crash-safe store** — atomic write (temp + fsync + rename + dir fsync), 5 rolling backups (unchanged saves don't churn them), corrupt-file recovery from the newest good backup, schema versioning with a migration chain, files from a newer app refused rather than downgraded, debounced autosave policy with a max-wait.
- Relaunch offers **resume** of the last live position but never auto-shows it to the congregation.

Tests: 15 new (rf-service). Not yet wired into the egui operator UI or the stage window.

Also: roadmap added as `docs/ROADMAP.md`.

## 0.804.3
Microphone restart fix: recording re-enabled on the new stream, service clock preserved, downtime filled with silence (see previous report).
