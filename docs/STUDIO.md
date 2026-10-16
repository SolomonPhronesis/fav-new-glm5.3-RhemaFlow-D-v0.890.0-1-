# Studio (v0.843.0): what is wired and what is not

All rules live in `crates/rf-studio` (no window, no clock, no hidden files). `rf-app/src/studio.rs` and `studio_ui.rs` add the buttons. Written from what a church needs, not copied from any other program.

| # | Feature | Module | In the app? |
|---|---------|--------|-------------|
| 1 | Song import/export (OpenLyrics, OpenSong, ChordPro, chords over words, plain) | `import` | Studio > Import songs |
| 2 | Chords: transpose, capo, Nashville, key | `chords` | Studio > Chords (+ band screen) |
| 3 | Bible versions side by side | `parallel` | Studio > Bible versions |
| 4 | Auto-fit, balanced lines, paging | `fit` | engine only |
| 5 | Transitions (cut, fade, dip, push, wipe, zoom) | `transition` | engine only (the app still has Cut/Fade) |
| 6 | Multi-box layouts + templates + editor rules | `layout` | used by the stage monitor; no editor yet |
| 7 | `{date}` `{time}` `{countdown:Name}` variables | `vars` | stage monitor, service templates |
| 8 | Macros (script, triggers) | `macros` | Studio > Macros |
| 9 | Auto-advance, loop, ping-pong | `cues` | engine only |
| 10 | Walk-in playlists | `playlist` | engine only |
| 11 | Media trim/speed/fades/filters | `mediaset` | engine only |
| 12 | Stage-monitor layouts | `stagemon` | Studio > Stage monitor (own window) |
| 13 | Alerts and nursery calls | `alerts` | Studio > Alerts; drawn on every screen |
| 14 | Teleprompter | `teleprompter` | operator-screen preview only |
| 15 | Library tools | `library_tools` | Studio > Song library |
| 16 | Service templates | `servicetpl` | Studio > Service templates (copy-forward is engine only) |
| 17 | Own shortcuts | `hotkeys` | Studio > Shortcuts / Macros |
| 18 | Go-to targets (`chorus`, `v2`, `+2`) | `goto` | used by macro `goto` |
| 19 | Preview then Take (safe mode) | `staging` | engine only |
| 20 | Bilingual lyrics | `bilingual` | engine only |

Not supported: reading other programs' private files (binary or database). Export from there as text/OpenLyrics.
Macro `sound` steps do not play files; use `run media_<shelf number>`.
User shortcuts only start macros (`macro:Name`) or named actions (`run:next_slide`); the built-in keys cannot be rebound.
