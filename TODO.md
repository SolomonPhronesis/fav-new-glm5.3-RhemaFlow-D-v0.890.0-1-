# TODO (v0.890.0)

## v0.890.0 checks (new)
1. Mac pass: build, `cargo test --workspace`, then NEEDS YOUR REVIEW > "Suggestion gates": set Cards min words 3/4/5 and min %, Auto-show min words 3/4/5 and min % 75-90, near-tie "show winner" vs "ask me", Stability hold off/0.5/0.65/1 s — close the app, reopen, confirm settings.json kept them.
2. Near-tie demo: read "the grace of our Lord Jesus Christ be with you" — winner (Rev 22:21) auto-stages by default; switch to "ask me" and confirm a card comes up instead.
3. Speech layer: `./run.sh model-turbo`, read John 3:16 and 2 Kings 19:15's opening — the stage text should read clean KJV with the "repaired against <verse>" note; chatter ("there are many devices...") must stay silent. Bench: docs/STT-V0890.md.

## Still open from before
1. v0.856.0 Mac pass: build, `cargo test --workspace`, then the HANDOFF mic checks (Luke 1:17 lock, music-under-reading, line repair, `./compare.sh` WER) PLUS the new one: say "there are many devices" mid-sermon and confirm Proverbs 19:21 only CARDS, never takes the stage; the full verse read still shows by itself.
1. Build on the Mac and run `cargo test -p rf-app`; fix compile errors in the v0.845-0.848 edits.
2. Mac check list: Cmd+A / C / V / X in Songs; gold menu strip for 0.5 s; reading line on a projector; rename an item and check the file on disk; remove then Options > Undo last remove; Find repeats on a pasted song; "Use drive" in Tools > Backup.
3. Hook Undo remove to Edit > Undo (Cmd+Z) when no text box is active and the MEDIA bar was the last thing used. Also Mac-check Cmd+Z / Shift+Cmd+Z in the Songs words box.
4. Library Check panel: buttons for unused files (`remove_orphans`) and missing files.
5. Click-to-add part chips under "Order sung".
6. Per-file progress and Cancel for very large folder drops.
7. Reading-line text colour / background option.

## Top 10 more ideas
1. Service orders store media by item id (needs a saved-service format change plus a one-time upgrade of old files).
2. Search INSIDE every PDF at once (index the words once, search them from the MEDIA box).
3. Song "Order sung" as drag-and-drop chips instead of typed text.
4. Stage preview of the CURRENT slide with the real font, background and size (not a lookalike panel).
5. "Sunday check" button: missing files, big videos, songs with unknown parts, unsaved edits, backup older than a week, all in one list.
6. Auto-fit long lyric lines (shrink or split) with a warning before they go on screen.
7. Import a whole service folder at once: pictures, videos, PDFs and a song text file become shelf items and an order.
8. Remember the last page / line of each PDF between services.
9. Per-song background and font override (a slow worship song vs a fast praise song).
10. Show a small "last shown" date on each item so unused files can be archived.

## Left from the top 10 (v0.850.0)
- Per-song background and font: add `#[serde(default)] look: Option<{bg, fg}>` to `Song`; set a `song_look` in `Present::stage_song_slide`, apply it where `StageLook` is built (app.rs ~810, ui.rs ~2204, extras.rs ~965) and clear it in `show_content` for anything that is not a song slide. Test on the real stage window.
- Service files by item id (only if resolve_any proves not enough).
- Mac check list additions: MEDIA search with 3+ letters shows PDF pages; Sunday check; right-click "Last shown"; Shift+Cmd+W; adding a folder with .txt song files.

## Speech accuracy plan (v0.851.0)
1. Record 10-15 real clips, write what was said, run `rhemaflow-d evalset` (docs/EVALSET.md). Keep the report as the baseline.
2. Feed `vocabulary-suggestions.txt` into the vocabulary and `calibrate`; re-run evalset; keep only changes that lower the error rate.
3. Compare Whisper model sizes and Parakeet on the same clips (delay versus accuracy on your Mac).
4. Tolerant scripture matching: spoken numbers, near-miss book names, a "did you mean?" strip (check what `evalset` says is missed first).
5. Noise: speech detection with music and congregation; test with `benchmark --mix-music`.
