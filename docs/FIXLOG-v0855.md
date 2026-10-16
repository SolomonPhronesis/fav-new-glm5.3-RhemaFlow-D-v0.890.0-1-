# FIXLOG v0.855.0 — the two operator screenshots

Every claim below was reproduced against the SHIPPED v0.854.0 zip first
(`cargo run -p rf-core --example repro855`), then fixed, then re-measured
(`cargo run --release -p rf-core --example textbench`). That is the part
v0.854.0 skipped — it shipped fixes without a before measurement.

---

## Screenshot 1 — "I said Luke 1:17, Malachi 4:6 appeared"

### What the screenshot shows
- The operator named **Luke 1:17**.
- The preacher read the verse. The transcript showed the half-line
  *"turn the heart of the father to the children"* — and the app put
  **Malachi 4:6** on stage instead (or carded it for approval).
- The same screenshot's transcript has *"to the gods"* where the verse reads
  *"to the wisdom of the just"* — a garbled tail with words missing.

### Why it happened (measured on v0.854.0 code)
1. Luke 1:17 QUOTES Malachi 4:6's opening, so the half-line fits both verses.
   The ranked matcher only found **Malachi 4:6 (66%)** — Luke 1:17 was not in
   the candidate list at all, because the preacher's singular *"father"*
   never matched the verse's plural *"fathers"* in any exact-text needle.
2. With one wrong-but-confident candidate and no alternatives, the
   auto-approve rule (single candidate ≥ 60%) could take the stage with the
   look-alike, displacing the verse the operator had named.
3. The parser had no idea which verse was "named": `ParseContext::last` was
   only used to suppress a repeat of the SAME verse, never to protect the
   named verse from a DIFFERENT verse over the same words.

### The fix (three layers, all parser-side)
- **Named-verse lock** (`rf-core/src/parser.rs`): when a fragment's quotation
  hit points at a DIFFERENT verse than the one on stage, the parser measures
  how well the fragment fits the STAGED verse's own text
  (`Bible::span_fit_pct`). If that fit clears the same accuracy floor the
  fragment needed anywhere else, the look-alike is dropped — no card, no
  auto-stage, the named verse stays. When the preacher genuinely moves on,
  the fragment no longer fits and detection resumes (measured: "lest I come
  and smite the earth with a curse" still cards Malachi 4:6 while Luke is on
  stage).
- **Anchored span alignment** (`rf-bible/src/lib.rs::span_align`): the
  fit-check needs to measure a fragment that sits MID-VERSE, so every
  (heard-word, verse-word) pair seeds a greedy in-order extension and the
  best span (matched words × F1) wins. Two earlier drafts failed their own
  tests: greedy-earliest alone anchored "and/the" at the verse's first
  chapter-openers (90% real fit scored 24%), and a DP backtracker matched
  "children" at its second occurrence and smeared the span across the verse.
- **Singular/plural folding**: the mid-verse probe trigram and every needle's
  last word are also tried with the trailing 's' added/removed, so
  *"turn the heart"* reaches *"turn the hearts"*. The card now shows BOTH
  look-alikes (Malachi 4:6 69%, Luke 1:17 56%) instead of one wrong answer,
  and the KJV wording itself still wins outright (Luke 1:17 87%).
- **OT/NT name aliases + -eth verbs** in `modernize()`: Elijah↔Elias,
  Isaiah↔Esaias, Jonah↔Jonas, Noah↔Noe, Jeremiah/Jeremy↔Jeremias;
  dwelleth→dwells, believeth→believes, cometh→comes, … (22 verbs). Applied to
  BOTH verse text and speech, matching only — display text is never rewritten.

### The "to the gods" half — verse-anchored repair
`rf-core/src/repair.rs`: while a verse is on stage and the utterance is plain
speech, the transcript line is repaired against THAT verse:
- the line must fit the verse first (`span_align` pct ≥ 60), else untouched;
- the aligned span is re-rendered from the verse's own words (KJV
  punctuation), gaps inside the sentence filled, mid-span noise dropped;
- a mangled sentence ENDING ("...to the gods") is replaced by the verse's
  sentence ending ("...to the wisdom of the just;") when that ending is within
  6 verse words and the tail word is not an ordinary aside;
- ordinary asides ("...to the children today") are kept verbatim;
- the app shows the repaired line (Scripture-coloured), the export and
  transcript logs keep the RAW ASR text, and the interpretation strip reports
  "repaired against Luke 1:17: 4 words filled, 1 noise words dropped".

Measured on five repair cases: on-screen WER **0.136 → 0.000**, and no repair
ever made a line worse (asserted in the harness and in unit tests).

---

## Screenshot 2 — "short fragments made false cards (Acts 23:33, Daniel 6:22)"

### What the screenshot shows
Fragments like *"also before him"* raised cards — Acts 23:33 (whose text ends
"...presented Paul **also before him**") and Daniel 6:22.

### Why it happened (measured on v0.854.0 code)
"also before him" is the verse's LAST three words. The v0.853 tail-cap put
such claims at 79% — above the 70% short-quote floor — with **no
alternatives**, so the single-candidate auto-approve rule (pct ≥ 60) not only
carded it, it STAGED it. A 3-word tail claim is the weakest evidence there
is precisely because nothing follows it that could confirm the verse.

### The fix (`rf-bible/src/lib.rs`, ranked scorer)
A 3–4 word fragment claiming a spot past the verse's mouth (offset > 2) is
now offered only when it carries real evidence:
- the claim sits within two words of the verse's opening (the verse still has
  continuation words that can confirm it — "behold I come" one word into
  Revelation 22:12 keeps working), or
- the fragment covers ≥ 35% of the verse's words.
Everything else is capped below the offer floor and stays silent. FAME_BONUS
can lift a capped-but-real tail claim, NEVER a silenced one — and (new) fame
never lifts a sub-60 base match over the auto-approve bar at all, so "and he
gave his only horse away" can no longer auto-stage John 3:16.

"and it was so"-style fragments that genuinely OPEN several verses still card
— always with their alternatives, never as an unopposed auto-stage.

### Assembler and search still work
The pieces assembler (`match_pieces`), full-line utterances and text search
are untouched: verses silenced in the ranked list remain reachable through
their real context ("but have eternal life" is silent alone; said after
"that whosoever believeth in him should not perish," it assembles normally).

---

## Proof

`cargo run -p rf-core --example repro855` on this tree:

```
"turn the heart of the father to the children"
  ranked=[Malachi 4:6 69%, Exodus 15:8 57%, Matthew 12:40 57%, Luke 1:17 56%, ...]
  -> would card : Malachi 4:6 (6)          # WITH Luke 1:17 now offered too
"and the disobedient to the wisdom of the just"
  confirms=["Luke 1:17 pct=87 alts=7"]
"also before him"
  confirms=[]                               # was: would AUTO-STAGE Acts 23:33
with Luke 1:17 on stage, 'turn the heart of the father to the children' ->
  nothing (verse stays)                     # was: card Malachi 4:6
```

Nine-group text-layer benchmark (method: `docs/TEXT-EVAL-NINE.md`):

| group                      | v0.854.0 | v0.855.0 |
|----------------------------|---------:|---------:|
| G1 clean reads             | 100.0    | 100.0    |
| G2 noise garbles           | 100.0    | 100.0    |
| G3 music tags              | 83.3     | 100.0    |
| G4 short distinctive       | 83.3     | 100.0    |
| G5 tail fragments silent   | 66.7     | 100.0    |
| G6 quote overlap (lock)    | 80.0     | 100.0    |
| G7 name variants           | 75.0     | 100.0    |
| G8 gap fill (WER 0.136→0)  | 86.4     | 100.0    |
| G9 chatter silent          | 100.0    | 100.0    |
| **composite**              | **87.7** | **100.0**|

Regression locks: 13 new rf-core tests + 5 new rf-bible tests + the repair
wiring in rf-app fail loudly if either screenshot ever comes back. The full
v0.853 reference battery (`refbench`) still scores 100% everywhere.
