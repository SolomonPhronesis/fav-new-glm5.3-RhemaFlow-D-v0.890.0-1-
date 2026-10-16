# PRD v1.0 vs PRD v2.0 — is the v2 md better, and why?

**Short answer: yes, clearly — v2.0 is the better document. Keep v2.0 as the
single source of truth and treat v1.0 as history.**

v1.0 is a good vision document: short sentences, one idea per paragraph, a
clear central promise ("the preacher speaks naturally; RhemaFlow understands
and presents"). What v1.0 never says is *how good is good enough*, *what
ships first*, or *what failure is unacceptable*. v2.0 answers all three
without losing v1.0's voice — it even keeps v1.0's structure section by
section.

## What v2.0 adds that changes real decisions

| Dimension | v1.0 | v2.0 | Why it matters |
|---|---|---|---|
| Priorities | twelve "essential" capabilities, all equal | MoSCoW (M/S/C) per feature | The MVP is now decidable; v1.0's list is a v1.0, not an MVP |
| Acceptance criteria | none | per-feature, testable ("given X, then Y") | Acceptance criteria become regression tests — v0.801 ships with 47 of them |
| Success metrics | open questions ("how often?") | numbered thresholds (auto-stage ≥ 90%, precision ≥ 97%, latency ≤ 2.5 s p95, operator touches ≤ 3) | You can measure a build and call it good or failed |
| Silent failure | not addressed | named a product defect, release blocker | This one sentence shaped the whole parser: a wrong verse on screen is worse than no verse |
| Technical budgets | none | platform, memory ≤ 350 MB, installer ≤ 100 MB, parse < 50 ms, offline live path | Constraints you can fail a release on, not aspirations |
| Roadmap | none | M0 shipped → M1 → M2 → M3 with reasons | "Automation should grow as trust grows" — phase gates instead of a feature dump |
| Risks | none | table with mitigations (ASR mishears, mic permissions, model size, licensing) | The macOS mic-permission risk in v2.0 is exactly the first-run trap; the app now handles it with a guided state indicator |
| Honest status | implicit "should" | per-feature "shipped / partial / planned" | You can't fool yourself about what exists |

## Where v2.0 is honest about the hard parts

v2.0's §6.1 quietly raises the bar on the parser in two places that cost real
engineering: parsing must be **structural, not a word-whitelist**, and a verse
that does not exist must produce an **honest out-of-range message with
suggestions** ("1 Corinthians 5 has only 13 verses (no verse 17) — did you
mean 2 Corinthians 5:17?"). Both are in v0.801 and both are locked by tests.
v1.0's phrasing ("should understand natural expressions") would never have
surfaced those requirements.

## One critique of v2.0, in fairness

The document says "M0 — shipped (this is v0.813.0 today)" while the deliverable
is named v0.801; version strings across branches have drifted. Minor, but fix
the numbering before tomorrow's demo so the story you tell is one story.
(Also, v2.0's status column is only as honest as its last edit — keep it
updated with each release or it decays into v1.0's optimism.)

## Verdict

v2.0 converts a vision into a contract: it is the version a senior engineer
can build against, a tester can write cases from, and an operator can be
promised things from. That is why this build (RhemaFlow-D v0.801) is scoped
exactly to v2.0's M0/M1 line and ships its acceptance criteria as executable
tests.
