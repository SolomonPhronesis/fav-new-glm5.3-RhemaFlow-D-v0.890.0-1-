//! The real Mac menu bar at the top of the screen (v0.839.0).
//!
//! It stays in the same place whatever the window does (moved, shrunk, hidden behind others), like every
//! Mac app. The Help menu is registered with macOS as THE Help menu, and macOS then adds its own
//! "Search" box at the top of it, which finds any menu item by name and shows where it is (the same as
//! Edge, Finder and Safari).
//!
//! Rules kept on purpose:
//! * Only shortcuts with Cmd (or Ctrl+Cmd) are attached to menu items. A bare key such as F or B attached
//!   to a menu would be taken by the menu even while you are typing in a text box. Bare keys are listed in
//!   the item's name instead, and still work as before inside the window.
//! * Cut / Copy / Paste / Select All ARE in the Edit menu (v0.840.0). A menu item that owns Cmd+C takes the
//!   key away from the window, so the item itself does the work: `clipboard_events` turns the click into the
//!   same events egui's text boxes already understand, and the app feeds them to whichever box has the cursor.
//!   Menu and keyboard therefore agree, and the text boxes keep working.
//! * Clicking an item puts its id in a queue; the app reads the queue every frame (`take_events`) and runs
//!   the same actions as the keys, the palette and the voice commands.

use std::sync::Mutex;

static QUEUE: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The ids of menu items clicked since the last call.
pub fn take_events() -> Vec<String> {
    match QUEUE.lock() {
        Ok(mut q) => std::mem::take(&mut *q),
        Err(_) => Vec::new(),
    }
}

#[cfg(target_os = "macos")]
pub fn install(ctx: egui::Context) {
    use muda::accelerator::{Accelerator, Code, Modifiers};
    use muda::{AboutMetadata, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

    let cmd = Some(Modifiers::SUPER);
    let item = |id: &str, label: &str, acc: Option<Accelerator>| -> Box<dyn IsMenuItem> { Box::new(MenuItem::with_id(id, label, true, acc)) };
    let sep = || -> Box<dyn IsMenuItem> { Box::new(PredefinedMenuItem::separator()) };
    let fill = |sub: &Submenu, items: Vec<Box<dyn IsMenuItem>>| {
        for it in &items {
            let _ = sub.append(it.as_ref());
        }
    };

    let menu = Menu::new();

    // ---- the app menu (named after the app by macOS) ----
    let version = env!("CARGO_PKG_VERSION").to_string();
    let about = AboutMetadata {
        name: Some("RhemaFlow".into()),
        version: Some(version.clone()),
        comments: Some("The preacher speaks. RhemaFlow understands.".into()),
        ..Default::default()
    };
    let app = Submenu::new("RhemaFlow", true);
    fill(
        &app,
        vec![
            Box::new(PredefinedMenuItem::about(None, Some(about))),
            item("about", "About and diagnostics\u{2026}", None),
            sep(),
            Box::new(PredefinedMenuItem::services(None)),
            sep(),
            Box::new(PredefinedMenuItem::hide(None)),
            Box::new(PredefinedMenuItem::hide_others(None)),
            Box::new(PredefinedMenuItem::show_all(None)),
            sep(),
            Box::new(PredefinedMenuItem::quit(None)),
        ],
    );

    // ---- File ----
    let file = Submenu::new("File", true);
    fill(
        &file,
        vec![
            item("toggle_stage", "Open / close stage window    (F)", None),
            item("toggle_fullscreen", "Full screen", Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::CONTROL), Code::KeyF))),
            sep(),
            item("songs", "Songs\u{2026}", None),
            item("slides_window", "Slides and timers\u{2026}", None),
            item("save_run_sheet", "Save run sheet as text", None),
            sep(),
            item("tools_backup", "Backup\u{2026}", None),
            item("open_data", "Show data folder in Finder", None),
        ],
    );

    // ---- Edit (Cut / Copy / Paste work through `clipboard_events`, see the top of this file) ----
    let edit = Submenu::new("Edit", true);
    fill(
        &edit,
        vec![
            item("cut", "Cut", Some(Accelerator::new(cmd, Code::KeyX))),
            item("copy", "Copy", Some(Accelerator::new(cmd, Code::KeyC))),
            item("paste", "Paste", Some(Accelerator::new(cmd, Code::KeyV))),
            item("select_all", "Select All", Some(Accelerator::new(cmd, Code::KeyA))),
            sep(),
            item("undo_edit", "Undo edit", Some(Accelerator::new(cmd, Code::KeyZ))),
            item("redo_edit", "Redo edit", Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyZ))),
            sep(),
            item("paste_search", "Paste and search scripture", None),
            item("search", "Search scripture", Some(Accelerator::new(cmd, Code::KeyF))),
            item("palette", "Command palette\u{2026}", Some(Accelerator::new(cmd, Code::KeyK))),
            item("save_song", "Save song", Some(Accelerator::new(cmd, Code::KeyS))),
            sep(),
            item("clear", "Clear the screen    (C)", None),
            item("blackout", "Blackout on / off    (B)", None),
        ],
    );

    // ---- View ----
    let view = Submenu::new("View", true);
    fill(
        &view,
        vec![
            item("layout_a", "Layout A: Operator console", Some(Accelerator::new(cmd, Code::Digit1))),
            item("layout_b", "Layout B: Dock first", Some(Accelerator::new(cmd, Code::Digit2))),
            item("layout_c", "Layout C: Focus", Some(Accelerator::new(cmd, Code::Digit3))),
            sep(),
            item("toggle_transcript", "Hide / show transcript", Some(Accelerator::new(cmd, Code::KeyT))),
            item("zoom_in", "Bigger text", Some(Accelerator::new(cmd, Code::Equal))),
            item("zoom_out", "Smaller text", Some(Accelerator::new(cmd, Code::Minus))),
            sep(),
            item("strongs", "Strong's (Greek and Hebrew)", None),
            item("voice_list", "Voice commands list", None),
            item("versions", "Bible versions", None),
        ],
    );

    // ---- Scripture ----
    let scripture = Submenu::new("Scripture", true);
    fill(
        &scripture,
        vec![
            item("approve", "Approve first card    (Space)", None),
            item("dismiss", "Dismiss first card    (X)", None),
            sep(),
            item("next_verse", "Next verse    (\u{2192})", None),
            item("prev_verse", "Previous verse    (\u{2190})", None),
            item("next_chapter", "Next chapter    (\u{2193})", None),
            item("prev_chapter", "Previous chapter    (\u{2191})", None),
            sep(),
            item("paste_go", "Go to the reference on the clipboard", Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV))),
            item("copy_verse", "Copy verse", None),
            item("copy_verse_ref", "Copy verse with reference", None),
            item("copy_ref", "Copy reference", None),
            sep(),
            item("history_back", "Previous scripture shown    ([)", None),
            item("history_forward", "Next scripture shown    (])", None),
            sep(),
            item("read", "Read aloud    (R)", None),
            item("stop_reading", "Stop reading    (S)", None),
        ],
    );

    // ---- Songs ----
    let songs = Submenu::new("Songs", true);
    fill(
        &songs,
        vec![
            item("songs", "Open Songs\u{2026}", None),
            item("paste_lyrics", "Paste lyrics\u{2026}", None),
            sep(),
            item("next_slide", "Next slide or song part    (Page Down)", None),
            item("prev_slide", "Previous slide or song part    (Page Up)", None),
        ],
    );

    // ---- Media ----
    let media = Submenu::new("Media", true);
    fill(
        &media,
        vec![
            item("media_toggle", "Play / pause    (K)", None),
            item("media_back", "Back 10 seconds    (J)", None),
            item("media_fwd", "Forward 10 seconds    (L)", None),
            item("media_restart", "Restart", None),
            sep(),
            item("media_paste_path", "Add the file whose path is on the clipboard", None),
            sep(),
            item("media_select_all", "Select all files    (Cmd+A)", None),
            item("media_select_none", "Select nothing    (Esc)", None),
            item("media_copy", "Copy selected files    (Cmd+C)", None),
            item("media_paste", "Paste files or a song    (Cmd+V)", None),
            item("media_remove_selected", "Remove selected files    (Delete)", None),
        ],
    );

    // ---- Window ----
    let window = Submenu::new("Window", true);
    fill(
        &window,
        vec![
            Box::new(PredefinedMenuItem::minimize(None)),
            Box::new(PredefinedMenuItem::bring_all_to_front(None)),
            sep(),
            item("tools_lower", "Tools (lower thirds, run-sheet, lock\u{2026})", None),
            item("palette", "Command palette\u{2026}", None),
            item("songs", "Songs", None),
            item("slides_window", "Slides and timers", None),
        ],
    );

    // ---- Help (last; macOS adds its Search box to it) ----
    let help = Submenu::new("Help", true);
    fill(
        &help,
        vec![
            item("help", "RhemaFlow guide: first service, step by step", Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Slash))),
            item("help_keys", "Keyboard shortcuts", None),
            item("help_trouble", "Troubleshooting", None),
            item("open_data", "Show data folder in Finder", None),
            sep(),
            item("about", "About and diagnostics\u{2026}", None),
        ],
    );

    for s in [&app, &file, &edit, &view, &scripture, &songs, &media, &window, &help] {
        let _ = menu.append(s);
    }

    // a click wakes the app (it may be idle) and leaves its id for the next frame
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if let Ok(mut q) = QUEUE.lock() {
            q.push(e.id.0.clone());
        }
        ctx.request_repaint();
    }));

    menu.init_for_nsapp();
    window.set_as_windows_menu_for_nsapp();
    help.set_as_help_menu_for_nsapp(); // this is what makes macOS put the Search box in Help

    // the menu must live as long as the program
    std::mem::forget((menu, app, file, edit, view, scripture, songs, media, window, help));
}

#[cfg(not(target_os = "macos"))]
pub fn install(_ctx: egui::Context) {}

/// What the Edit menu's Cut / Copy / Paste / Select All do, as events for the text box that has the cursor.
/// `paste_text` is what is on the clipboard (only read for "paste"). None if `id` is not a clipboard item.
pub fn clipboard_events(id: &str, paste_text: impl FnOnce() -> String) -> Option<Vec<egui::Event>> {
    Some(match id {
        "cut" => vec![egui::Event::Cut],
        "copy" => vec![egui::Event::Copy],
        "paste" => {
            let t = paste_text();
            if t.is_empty() {
                vec![]
            } else {
                vec![egui::Event::Paste(t)]
            }
        }
        "select_all" => vec![egui::Event::Key {
            key: egui::Key::A,
            physical_key: Some(egui::Key::A),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
        _ => return None,
    })
}

/// The menu titles in the order they appear on the Mac menu bar (after the app menu).
pub const MENU_TITLES: [&str; 8] = ["File", "Edit", "View", "Scripture", "Songs", "Media", "Window", "Help"];

/// Where a menu item lives and how it reads: (menu title, item name, keys), for the 0.5 s flash (v0.845.0).
/// The real Mac menu cannot light one item up by itself, so the window shows a copy of the menu bar for half a
/// second with the used menu and command lit. None for ids that are not worth flashing.
pub fn describe(id: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match id {
        "cut" => ("Edit", "Cut", "\u{2318}X"),
        "copy" => ("Edit", "Copy", "\u{2318}C"),
        "paste" => ("Edit", "Paste", "\u{2318}V"),
        "select_all" => ("Edit", "Select All", "\u{2318}A"),
        "undo_edit" => ("Edit", "Undo edit", "\u{2318}Z"),
        "redo_edit" => ("Edit", "Redo edit", "\u{21e7}\u{2318}Z"),
        "song_tidy" => ("Songs", "Tidy the words", "\u{21e7}\u{2318}T"),
        "song_strip" => ("Songs", "Strip chords and web junk", "\u{21e7}\u{2318}L"),
        "song_repeats" => ("Songs", "Find repeats", "\u{21e7}\u{2318}R"),
        "song_wrap" => ("Songs", "Wrap long lines", "\u{21e7}\u{2318}W"),
        "song_add_verse" => ("Songs", "Add the next Verse", "\u{21e7}\u{2318}P"),
        "paste_search" => ("Edit", "Paste and search scripture", ""),
        "search" => ("Edit", "Search scripture", "\u{2318}F"),
        "palette" => ("Edit", "Command palette", "\u{2318}K"),
        "save_song" => ("Edit", "Save song", "\u{2318}S"),
        "clear" => ("Edit", "Clear the screen", "C"),
        "blackout" => ("Edit", "Blackout", "B"),
        "toggle_stage" => ("File", "Open / close stage window", "F"),
        "toggle_fullscreen" => ("File", "Full screen", "\u{2303}\u{2318}F"),
        "songs" => ("Songs", "Open Songs", ""),
        "slides_window" => ("File", "Slides and timers", ""),
        "layout_a" => ("View", "Layout A: Operator console", "\u{2318}1"),
        "layout_b" => ("View", "Layout B: Dock first", "\u{2318}2"),
        "layout_c" => ("View", "Layout C: Focus", "\u{2318}3"),
        "toggle_transcript" => ("View", "Hide / show transcript", "\u{2318}T"),
        "zoom_in" => ("View", "Bigger text", "\u{2318}+"),
        "zoom_out" => ("View", "Smaller text", "\u{2318}\u{2212}"),
        "paste_go" => ("Scripture", "Go to the reference on the clipboard", "\u{21e7}\u{2318}V"),
        "next_verse" => ("Scripture", "Next verse", "\u{2192}"),
        "prev_verse" => ("Scripture", "Previous verse", "\u{2190}"),
        "paste_lyrics" => ("Songs", "Paste lyrics", ""),
        "media_select_all" => ("Media", "Select all files", "\u{2318}A"),
        "media_copy" => ("Media", "Copy selected files", "\u{2318}C"),
        "media_paste" => ("Media", "Paste files or a song", "\u{2318}V"),
        "media_remove_selected" => ("Media", "Remove selected files", "\u{232b}"),
        "help" => ("Help", "RhemaFlow guide", "\u{21e7}\u{2318}/"),
        _ => return None,
    })
}

/// The text on the Mac's clipboard ("" if there is none or it is not text).
pub fn read_clipboard() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(o) = std::process::Command::new("pbpaste").output() {
            return String::from_utf8_lossy(&o.stdout).to_string();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_items_become_the_events_text_boxes_understand() {
        assert!(matches!(clipboard_events("copy", String::new).unwrap()[0], egui::Event::Copy));
        assert!(matches!(clipboard_events("cut", String::new).unwrap()[0], egui::Event::Cut));
        match &clipboard_events("paste", || "John 3:16".into()).unwrap()[0] {
            egui::Event::Paste(t) => assert_eq!(t, "John 3:16"),
            e => panic!("{e:?}"),
        }
        assert!(clipboard_events("paste", String::new).unwrap().is_empty(), "an empty clipboard pastes nothing");
        assert!(matches!(clipboard_events("select_all", String::new).unwrap()[0], egui::Event::Key { key: egui::Key::A, .. }));
        assert!(clipboard_events("clear", String::new).is_none());
    }

    #[test]
    fn every_flashed_command_names_a_real_menu() {
        for id in ["cut", "copy", "paste", "select_all", "undo_edit", "redo_edit", "song_tidy", "song_strip", "song_repeats", "song_wrap", "song_add_verse", "save_song", "layout_a", "media_copy", "help", "toggle_stage"] {
            let (menu, name, _) = describe(id).unwrap();
            assert!(MENU_TITLES.contains(&menu) && !name.is_empty(), "{id}");
        }
        assert!(describe("nope").is_none());
    }
}

/// True when the real system menu bar is in use (so the in-window one is not drawn).
pub const NATIVE: bool = cfg!(target_os = "macos");
