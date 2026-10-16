# Media and file-system design review (v0.824.0)

**Honest note:** the scores below are my design judgement against a stated rubric. They are NOT from user testing. Test the winner with 5 real volunteers on a real Sunday set-up before trusting it.

## Rubric (weights add up to 100)
Speed to show a file 25 · Safety when live 20 · Simplicity 20 · Handles hundreds of files 15 · Screen space kept 10 · Clarity of state 10

## The six designs, ranked
| Rank | Design | Score /100 | Why |
|---|---|---|---|
| 1 | **Docked bar, upgraded (built)** | 81.0 | One click shows a file; stage and scriptures stay visible; only visible rows are drawn, so hundreds of files stay smooth; search, size, list view, ON SCREEN badge and arrow keys. |
| 2 | Filmstrip (one row) | 71.0 | Simplest and smallest, but one row cannot hold many files and has no room for folders. |
| 3 | Search-first launcher | 68.5 | Great for 1000+ files and keyboard users; poor for volunteers who browse by picture and cannot remember names. |
| 4 | Full library workspace (sidebar + big grid) | 68.0 | The classic. Best for organising, but it takes over the screen, so the stage and scripture lanes would be hidden during a service. |
| 5 | Ours before today (v0.823 bar) | 67.0 | Right idea, but every thumbnail was loaded even off screen, no search, fixed size, copying froze the window. |
| 6 | Preview, then Take (two-step) | 64.5 | Safest against a wrong click, but doubles the clicks and the screen cost. Best offered as an optional "safe mode", not the default. |

## What changed in the built design (and why)
1. **Only visible rows are drawn** (virtual grid). Thumbnails are made only for what is on screen: 300 files made fewer than 90 thumbnails in the test.
2. **Adding files runs on a helper thread** with a progress bar: the window never freezes on a big video.
3. **Search, thumbnail size slider, grid/list toggle, sort** (order added, newest, name).
4. **ON SCREEN badge** (not colour alone) on whatever is showing now.
5. **Arrow keys + Enter/Space** move and show while the mouse is over the bar and nothing is being typed.
6. Right-click everywhere (folders, items, empty space); drag items onto folder chips; drag files and whole folders from Finder.

## Top 20 features to replace ProPresenter (my order)
**Core service flow:** 1 Service order (playlist, drag items in from the bar) · 2 Songs and lyric slides with a CCLI report · 3 Presentation import (PowerPoint, ProPresenter, FreeShow) · 4 Themes and templates · 5 Transitions (cut, fade) and motion backgrounds · 6 Slide editor with a live preview
**Outputs:** 7 Windows app · 8 Stage (confidence) monitor for the preacher · 9 Several outputs (lower-thirds, OBS/NDI, a second room) · 10 Alerts and nursery messages overlay
**Media and file system:** 11 Audio files and walk-in music playlists · 12 More video formats (.mov, HEVC, WebM) with an automatic converter · 13 Video trim and loop points · 14 Tags and smart folders ("videos", "this month", "missing") · 15 Watch-a-folder (new files appear by themselves) · 16 Storage view and duplicate clean-up · 17 Backup and restore of the whole library and service
**Team and control:** 18 Full phone and tablet control with permissions · 19 Hotkeys, MIDI and Stream Deck buttons · 20 Roles (operator, preacher, reviewer) and a shared review queue
**Your edge (keep investing):** live captions and translation, sermon notes and a scripture timeline export.
