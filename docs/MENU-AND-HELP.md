# Menu bar and Help search (v0.839.0)

**Where the menu goes.** The Mac menu bar at the top of the screen, so it stays in the same place when the window is moved, shrunk or hidden. Menus: RhemaFlow-D, File, Edit, View, Scripture, Songs, Media, Window, Help (last). Built with the `muda` crate (MIT or Apache-2.0) in `macmenu.rs`. Other systems still get the in-window bar.

## Top 3 ways to give Help a search like Edge

| # | Way | Verdict |
|---|---|---|
| 1 | **Register our Help menu with macOS as the Help menu.** macOS adds its own Search box that finds any menu item by name, shows the path to it and can run it | **Chosen.** It is the standard Mac way, costs one call (`set_as_help_menu_for_nsapp`), and searches every menu automatically. |
| 2 | Our own search box in the Help menu | A custom text box inside a native menu is not supported; it would be a pop-up window pretending to be a menu. Rejected as the main way. |
| 3 | A command palette (Cmd+K) listing every action and its key | Built as well (0.837.0) and kept as the second way; it also runs the action. The Help window has its own search for the guide text. |

**Rules kept.** Only Cmd shortcuts are attached to menu items (bare keys like F or B would be taken from text boxes). No Cut/Copy/Paste items (the text boxes read the keys themselves). Bare keys are shown in the item names.
