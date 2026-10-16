# Proverbs 19:21 — root cause, whole-Bible grid, and the fix (v0.856.0)

The operator's question, quoted: *"find reason for proverbs 19 v 21 falsely
showing — is it 3 words min or accuracy — and fix: if it is 3 words then make
min 4/5 and if 80% accuracy then make 85%. Do a rank, graph, grid and test the
best and most accurate through the bible book, ch, verses; show which is the
most accurate and go with that; show the b4 and after and % in accuracy and if
any trade off. But make sure accuracy is the goal and is hit."*

Every number below was produced on the exact code in this zip. Re-run anything
with:

```
cargo run -p rf-core --example prov1921          # the bug, before AND after
cargo run --release -p rf-core --example biblegrid -- grid.json   # the grid (about 3 min)
cargo run --release -p rf-core --example textbench -- after.json  # the nine groups
```

---

## 1. The reason (measured, not guessed)

Proverbs 19:21 (KJV): *"There are many devices in a man's heart; nevertheless
the counsel of the LORD, that shall stand."*

Run `prov1921` against the pristine v0.855.0 code:

| the preacher says | v0.855.0 does |
|---|---|
| **"there are many devices"** (4 ordinary words — e.g. about phones) | **AUTO-STAGES Proverbs 19:21 at 89%** |
| "there are many devices here today" (6w) | cards it at 62% (one click shows it) |
| "there are many devices in this room" (7w) | cards it at 67% |
| "the counsel of the lord that shall stand" (8w) | auto-stages Prov 19:21 at 92% |
| "many plans in a man's heart nevertheless the counsel of the lord" (12w, a mixed paraphrase) | auto-stages Prov 19:21 at 88% |

**The answer to the question: it was the ACCURACY bar, not the 3-word
minimum.**

- The 3-word tier is innocent here: 3-word probes on this verse ("that shall
  stand", "many devices") were already silent in v0.855.0.
- The guilty gate was the auto-approve rule in `rf-app/src/state.rs`: a SINGLE
  candidate auto-staged at ≥ 60% and a clear winner (12+ points ahead)
  auto-staged at ≥ **75%**. The four words "there are many devices" are all
  the verse's own opening words, so the F1 scorer gave them 89% — over both
  bars — and the verse went straight to the big screen. No screenshot needed:
  this is one instance of a MILLION-strong class (see §2).

So the fix axis that matters is the accuracy bar — the operator's instinct
("make it 85%") was the right direction; the grid below picked the exact
number.

## 2. The grid — every book, chapter and verse

Two surfaces, both built from the embedded KJV only:

- **Negative (a false show is ANY auto-stage here):** every contiguous
  4-, 5- and 6-word window of all 31,102 verses, deduplicated —
  **1,698,315 unique utterances** (93,115 raw windows). These are the words a
  preacher can say IN PASSING without quoting. "there are many devices" is
  literally one of these windows.
- **Positive (must keep working):** every verse read from its opening
  (≤ 12 words) plus every short verse read whole — **33,189 verse reads**.

One threaded matcher pass caches each utterance's ranked result; the gates are
then applied per config, so all 13 rows share identical evidence.

Configs: **W** = minimum words an utterance needs before it may auto-stage
(the operator's "min 4/5"); **A** = the accuracy bar for auto-staging (the
operator's "80 → 85"). Shipped v0.855 = W3 with bars 60 (single) / 75 (multi).

| rank | W | A | false shows (of 1,698,315) | per 1,000 | reads auto-staged | reads carded | reads missed | identification | composite |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| 13 | 3 | 75 **(shipped v0.855)** | 1,109,015 | 653.01 | 31,816 | 778 | 595 | 98.2% | 29.5 |
| 11 | 3 | 75 uniform | 1,087,432 | 640.30 | 31,816 | 778 | 595 | 98.2% | 30.8 |
| 9  | 3 | 80 | 1,039,672 | 612.18 | 31,816 | 778 | 595 | 98.2% | 33.8 |
| 5  | 3 | 85 | 586,509 | 345.35 | 31,816 | 778 | 595 | 98.2% | 62.4 |
| 2  | 3 | 90 | 105,701 | 62.24 | 31,816 | 778 | 595 | 98.2% | 92.8 |
| 12 | 4 | 75 | 1,087,432 | 640.30 | 31,778 | 816 | 595 | 98.2% | 30.8 |
| 10 | 4 | 80 | 1,039,672 | 612.18 | 31,778 | 816 | 595 | 98.2% | 33.8 |
| 6  | 4 | 85 | 586,509 | 345.35 | 31,778 | 816 | 595 | 98.2% | 62.4 |
| 3  | 4 | 90 | 105,701 | 62.24 | 31,778 | 816 | 595 | 98.2% | 92.8 |
| 8  | 5 | 75 | 1,009,884 | 594.64 | 31,764 | 830 | 595 | 98.2% | 35.7 |
| 7  | 5 | 80 | 1,009,829 | 594.61 | 31,764 | 830 | 595 | 98.2% | 35.7 |
| 4  | 5 | 85 | 556,892 | 327.91 | 31,764 | 830 | 595 | 98.2% | 64.3 |
| **1** | **5** | **90** | **97,225** | **57.25** | **31,764** | **830** | **595** | **98.2%** | **93.3** |

- composite = 70% × accuracy_score + 30% × identification (accuracy is the
  goal, so accuracy carries the weight; accuracy_score = 100 × (1 − FP/1k ÷
  worst FP/1k)).
- `reads missed` never moves: identification is untouched in every config —
  what changes is only who presses the button, the app or the operator.
- `misfire` (a true read staging the WRONG verse) is 0 in every row.

**The chart:** `docs/accuracy-v0856.png` (four panels: false-show surface,
what real reads still do, the rank, and the ten worst books — Psalms, Genesis,
Jeremiah, Isaiah, Ezekiel… — each dropping ~85-90%).
**The raw data:** `docs/grid-v0856.json` (this table, plus per-book counts and
the Proverbs 19:21 acceptance checks).

## 3. The winner: W=5, A=90 — and why

- **Most accurate of all 13 configs** (rank 1): false shows 1,109,015 →
  97,225 (**−91.2%**, 653.01 → 57.25 per 1,000 passing-speech windows).
- The 4-word "there are many devices" class dies at W=5; the 75%-bar class
  dies at A=90. Neither knob alone gets close (W5/A75 still has 594.6 per 1k;
  the bar does the heavy lifting — exactly why the user's "80% → 85%" instinct
  pointed at the right lever, and the grid pushed it to 90).
- W=5 beats the near-tie W=3/A=90 (rank 2, 92.8) by killing 8,476 more false
  windows at the cost of 52 more reads waiting as cards — the accuracy-first
  trade the operator asked for ("make sure accuracy is the goal").
- The Proverbs 19:21 acceptance list passes: all four false fragments silent
  (pF = 0), all three true reads still stage-or-card (pT = 0).

## 4. Before / after on the shipped code

| the preacher says | v0.855.0 (before) | v0.856.0 (after) |
|---|---|---|
| "there are many devices" | **AUTO-STAGE Prov 19:21 (89%)** | card — operator decides |
| "there are many devices here today" | card 62% | card (unchanged, safe) |
| "there are many devices in this room" | card 67% | card (unchanged, safe) |
| "the counsel of the lord that shall stand" | AUTO-STAGE (92%) | AUTO-STAGE (92% — verbatim 8 verse-words, still clear-winner grade) |
| "many plans … nevertheless the counsel of the lord" (mixed paraphrase) | AUTO-STAGE (88%) | card |
| **full verse read** (16 words) | AUTO-STAGE 95% | **AUTO-STAGE 95%** |
| nine-group benchmark | 100.0 | **100.0** (nothing lost) |
| false-show windows per 1,000 | 653.01 | **57.25 (−91.2%)** |
| true verse reads still auto-staging | 31,816 / 33,189 (95.9%) | 31,764 / 33,189 (95.7%) |
| true verse reads identified (stage + card) | 98.2% | **98.2%** |

The acceptance tests now lock this in: `rf-app` `prov1921_four_word_fragment_
never_auto_stages`, `prov1921_fragment_cards_but_never_stages_full_verse_
still_stages`, `auto_approve_needs_the_grid_bars` (all green in the Linux
sandbox, together with the rest of the suite: rf-core 167, rf-app 332, rf-bible
44, rf-service/rf-media/rf-strongs/rf-studio green).

## 5. Trade-offs (the operator asked for them, honestly)

1. **Below-bar true quotes now wait for a click.** A true read scoring under
   90 with a runner-up within 12 points — e.g. the Revelation 1:6 / 5:10 tie
   at 73% — always waited; now damaged one-word-off readings in the 60s-80s
   wait too. The card still shows the RIGHT verse; the operator presses
   approve. Measured cost: 52 reads of 33,189 moved from auto to card
   (0.16%). Nothing became harder to FIND — identification is unchanged.
2. **The famous 3-word opening "for God so" no longer self-stages** (it
   cards). This is the "3 word min" trade-off the operator suspected: the
   grid shows 3-word AUTO-STAGE ability was worth nothing for accuracy (W3
   rows are the worst) — but 3-word IDENTIFICATION is fully kept (the
   nine-group G4 short_distinctive stays 100: right verse in top-3, card shows
   it). If a service never uses auto-approve, NOTHING in this section applies
   — every below-bar case was already a card.
3. **The residual 57.25 per 1,000 cannot be removed by any text-only bar.**
   Those are 5-6-word verbatim openings of verses ("there are many devices
   in"…) that score ≥ 90 with a clear lead — textually indistinguishable from
   a genuine quote start. Only CONTEXT can judge them (a named verse on stage
   — the v0.855 lock — already covers part of this). This is the honest floor:
   11.4× fewer false shows than v0.855, not zero.
4. **A=85 was measurably worse** (327.9 per 1k — the exact number the operator
   guessed would be enough; it cuts less than half the surface). The grid is
   why the bar went to 90 and not 85.

## 6. What changed in the code

- `crates/rf-core/src/parser.rs` — new public constants `AUTO_MIN_WORDS = 5`
  and `AUTO_STAGE_PCT = 90` (the grid's winner), and `Detection.words`: the
  parser now records how many words each quotation detection heard (0 =
  unknown for non-quotation kinds).
- `crates/rf-app/src/state.rs::should_auto_approve` — the two grid bars: a
  quotation shorter than 5 words never auto-stages (still cards); single
  candidates need ≥ 90 (was 60), clear winners need ≥ 90 AND the 12-point
  lead (was 75). pct == 0 (the exact-text unique tier) behaves as before,
  subject to the word gate.
- `crates/rf-core/examples/prov1921.rs` — the repro, now reading the live
  constants so it can never drift from the app again.
- `crates/rf-core/examples/biblegrid.rs` — the whole-Bible grid harness.
- `crates/rf-app/src/state.rs` tests + `crates/rf-core/src/tests.rs` — the
  Proverbs 19:21 regressions and the new bar expectations.
