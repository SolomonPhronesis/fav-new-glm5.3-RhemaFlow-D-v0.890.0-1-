# Top 20 improvements, from the operator's chair and the engineer's (v0.813.0)

Written as someone who runs the screen on a Sunday and as a senior developer. **Status: BUILT** = in this zip and tested; **NEXT** = designed, not built.

| # | Feature | Why it matters on Sunday | Status |
|---|---|---|---|
| 1 | One search box: book, `John 3:16`, or words ("god so loved") | Fastest fallback when the voice misses | BUILT |
| 2 | Scripture browser that follows the stage and works by hand | You always see where you are, and can fix it with 2 clicks | BUILT |
| 3 | Keyboard control: Space approve / next-in-plan, X dismiss, 1-5 pick, arrows, B, C, R, S, F, `/` | Hands never leave the keyboard | BUILT |
| 4 | Blackout (B) that hides the verse without losing it | Prayer, worship, a joke: instantly and reversibly | BUILT |
| 5 | Recent scriptures strip (one click to re-show) | "Go back to the verse from earlier" | BUILT |
| 6 | Sermon plan: paste notes, Space steps through every reference | Prepared sermons run themselves | BUILT |
| 7 | Stage looks: Classic, Lower third, Green screen, Large print | Streams, overflow rooms, back rows | BUILT |
| 8 | OBS Browser-source page (`/stage`), transparent, restyle by URL | Streaming without screen capture | BUILT |
| 9 | Phone remote with PIN (`/remote`): next, approve, blackout, read | Media operator can walk the room | BUILT |
| 10 | Bible versions: folder + one-click public-domain downloads + importer | NIV/ESV etc. from your own licence | BUILT |
| 11 | Change version live, verse redraws | Pastor says "in the Amplified..." | BUILT |
| 12 | Auto-approve only when unambiguous (75% and 12 points ahead) | Never guesses between look-alike verses | BUILT |
| 13 | No card spam (one book card, uncertain words in sentences ignored) | The review panel stays readable | BUILT |
| 14 | Hath/have and other KJV forms understood | Speech engines write modern English | BUILT |
| 15 | Layout that fits (3 control rows at 1190x770, nothing off-screen) | Everything visible | BUILT |
| 16 | Web (JS) version | Any laptop, no install | BUILT (Chrome/Edge/Safari) |
| 17 | Live captions on the stage (lower-third transcript) for the hard of hearing | Accessibility, and a real selling point | NEXT |
| 18 | Verse paging: a very long verse or range split over slides | Big text without shrinking | NEXT |
| 19 | Operator preview with "next up" and a confidence monitor on a second screen | See what is coming | NEXT |
| 20 | Pronunciation/corrections lexicon editor (accents, names), learning from "Correct" | Accuracy on your own voice | NEXT (engine exists: `calibrate`) |

## Engineer's notes (what I would not skip)
* **Everything decided in pure functions** (matching, auto-approve, plan parsing, naming, search) with tests; the screen only draws. That is why the web version could reuse the rules and be tested in Node.
* **Honest limits are in the UI**, not hidden: ambiguous quotes wait for a person; licensed Bibles are not faked; the web version says what browsers refuse.
* **Safe by default:** the web server is off, listens on this Mac only unless you tick Wi-Fi, controls need a PIN, `/state` is read-only.
* **Do next for reliability:** run a 2-hour soak test on your Mac with the real mic and a real sermon recording; add a crash-recovery dialog on start (the live files already survive a crash); sign and notarise the macOS app.
