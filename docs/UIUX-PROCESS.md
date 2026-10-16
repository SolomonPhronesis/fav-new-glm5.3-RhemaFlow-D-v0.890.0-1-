# UI/UX Process & Design Rationale — RhemaFlow-D v0.801.2

Written from the 30-years-in-the-booth seat. The product has exactly two
users with exactly two different jobs — every design decision flows from
that split.

---

## 1. The two users (design cannot start without this)

| | **The Preacher** (in the pulpit) | **The Operator** (in the booth) |
|---|---|---|
| Hands | open Bible, gestures | keyboard & mouse |
| Eyes | congregation | screens |
| Needs | voice that just works | total situational awareness |
| Failure cost | distraction, lost flow | wrong verse in front of everyone |

Consequence: the Preacher's interface is the **voice grammar** (broad,
forgiving, fuzzy); the Operator's interface is the **Command Centre**
(informative, keyboard-complete, honest). We never blur the two.

## 2. The 8-step design process (what we actually follow)

1. **Observe the booth** — sit through a live service; map every glance,
   keystroke, and "did it take?" moment. (Done; produced the three-zone
   layout.)
2. **Define the 3-second loop** — glance → know → act. Everything the
   operator touches in a live service must be reachable inside that loop,
   zero scrolling. (Center stage preview + right review queue + bottom
   transport exist precisely for this.)
3. **Information hierarchy** — one gold accent = "live/staged". Everything
   else is calm slate. The eye should land on the stage state before
   anything else. (Cathedral Dark token system in `theme.rs`.)
4. **Prototype in the real lighting** — dark booth, projector at 60%,
   laptop at 30% brightness. (Contrast decisions were validated against
   dimmed conditions, not a studio monitor.)
5. **Motion with reverence** — 350 ms fade-ins, no bounces or slides.
   Congregation-facing motion must never draw attention to the tool.
6. **Forgiveness hierarchy** — for any utterance: auto-stage (sure) →
   confirm card (likely) → honest error (impossible). The system must
   NEVER silently do the wrong thing; trust is the product.
7. **Discoverability in context** — every voice command listed in Help (H),
   every button hover-labeled, demo phrases built into the UI for training.
8. **Instrument & iterate** — auto-staged count, touches, avg latency are
   on-screen; they tell us whether we are actually reducing operator load.

## 3. v0.801.2 UX additions and why

* **HEARD → RESULT strip** — closes the operator's biggest anxiety loop
  ("did it understand me?") with an instant, one-line answer. Placed
  directly under the stage preview where the eye already goes.
* **Second control row** — read-aloud (button + R), voice picker with
  ★ natural + gender labels, rate, auto-read, zoom (A−/A+/1:1). Grouped as
  "voice & projection" so row 1 stays pure transport.
* **Voice picker design** — natural voices star-marked and sorted first;
  gender explicit ("male/female option" requirement); "Manage voices…"
  deep-links the free macOS voice downloads instead of a fake in-app store.
* **Zoom via three redundant paths** (voice, buttons, keys) because a
  feature needed mid-service must have a fallback when the mic is busy.
* **Demo phrases now include commands** ("Next.", "Read it.", "Zoom in.")
  so a new operator can rehearse the full grammar in 20 seconds.

## 4. Next design work (v0.9, in priority order)

1. **Onboarding tour** (first run: mic permission → voice test → 10-second
   demo) — volunteers rotate; the product must train its own operators.
2. **Command palette (⌘K)** — power-operator layer as the feature surface
   grows.
3. **Layout presets** — single-window (small booth) vs triple (full booth).
4. **Reduced-motion + contrast audit** — accessibility is part of polish,
   not an afterthought.
5. **Stage theme presets** (light projector-max for bright sanctuaries).

## 5. Design rules we refuse to break

1. Never auto-stage what we are not sure of — honesty beats magic.
2. Never make the Preacher wait on the Operator, or the Operator guess.
3. Never put a camera-red "REC" anywhere the congregation can see panic.
4. Never add a control that can be misfired by a misheard word into an
   irreversible action (voice cannot stop recording — ever).
5. Every new feature must shrink or keep the 3-second loop, not grow it.
