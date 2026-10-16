//! Clipboard helpers (v0.841.0): paste a verse reference and jump to it, copy the verse on the stage
//! in three shapes, and paste a file path into the MEDIA bar.
//!
//! The text rules are plain functions with tests. The three `impl RhemaFlowApp` methods at the bottom
//! only read or write the clipboard and call what the app already does.
//!
//! NOT run on a real Mac: the clipboard read goes through `pbpaste`, the write through egui.
//! v0.844.0: Cmd+C / Cmd+V work on FILES and SONGS everywhere (not only in text boxes): copy files from the
//! MEDIA bar, paste files copied in Finder, paste a file path, or paste a song's words.

use crate::app::RhemaFlowApp;
use crate::state::StageKind;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CopyKind {
    /// just the words of the verse
    Text,
    /// "John 3:16"
    Reference,
    /// the words, then the reference and Bible: `For God so loved ... (John 3:16, ASV)`
    WithReference,
}

/// What goes on the clipboard for the verse on the stage. `None` when there is nothing to copy.
pub fn verse_copy(kind: CopyKind, label: &str, text: &str, translation: &str) -> Option<String> {
    let label = label.trim();
    let body = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match kind {
        CopyKind::Reference => (!label.is_empty()).then(|| label.to_string()),
        CopyKind::Text => (!body.is_empty()).then_some(body),
        CopyKind::WithReference => {
            if body.is_empty() {
                return None;
            }
            let tr = translation.trim();
            Some(match (label.is_empty(), tr.is_empty()) {
                (true, _) => body,
                (false, true) => format!("{body} ({label})"),
                (false, false) => format!("{body} ({label}, {tr})"),
            })
        }
    }
}

/// Make a clipboard string into a file path: the first line only; quotes, `file://` links (with `%20`
/// and friends), `\ ` from Terminal, and `~/` are all understood. It does not check the file exists.
pub fn path_from_clipboard(text: &str, home: Option<&str>) -> Option<PathBuf> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let mut s = line.trim_matches(|c| c == '"' || c == '\'' || c == '\u{201c}' || c == '\u{201d}').to_string();
    if let Some(rest) = s.strip_prefix("file://") {
        s = percent_decode(rest.strip_prefix("localhost").unwrap_or(rest));
    }
    s = s.replace("\\ ", " ");
    if let Some(rest) = s.strip_prefix("~/") {
        s = format!("{}/{}", home?.trim_end_matches('/'), rest);
    }
    if !s.starts_with('/') || s.len() < 2 {
        return None;
    }
    Some(PathBuf::from(s))
}

/// Put FILES (not just their names) on the Mac's clipboard, so Cmd+V works in Finder, Mail and here.
/// Returns false when that could not be done (not a Mac, or osascript failed); the caller then copies the paths as text.
pub fn write_clipboard_files(paths: &[String]) -> bool {
    #[cfg(target_os = "macos")]
    {
        const JS: &str = "function run(argv){ObjC.import('AppKit');var pb=$.NSPasteboard.generalPasteboard;pb.clearContents();var a=$.NSMutableArray.alloc.init;for(var i=0;i<argv.length;i++){a.addObject($.NSURL.fileURLWithPath(argv[i]));}return pb.writeObjects(a)?'ok':'no';}";
        if paths.is_empty() {
            return false;
        }
        return match std::process::Command::new("osascript").args(["-l", "JavaScript", "-e", JS]).args(paths).output() {
            Ok(o) => o.status.success() && String::from_utf8_lossy(&o.stdout).contains("ok"),
            Err(_) => false,
        };
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = paths;
        false
    }
}

/// The files on the Mac's clipboard (copied in Finder, or with Cmd+C in the MEDIA bar). Empty if there are none.
pub fn read_clipboard_files() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        const JS: &str = "function run(){ObjC.import('AppKit');var pb=$.NSPasteboard.generalPasteboard;var arr=pb.readObjectsForClassesOptions($.NSArray.arrayWithObject($.NSURL),$.NSDictionary.dictionary);var out=[];if(arr){for(var i=0;i<arr.count;i++){var u=arr.objectAtIndex(i);if(u.isFileURL){out.push(ObjC.unwrap(u.path));}}}return out.join('\\n');}";
        if let Ok(o) = std::process::Command::new("osascript").args(["-l", "JavaScript", "-e", JS]).output() {
            if o.status.success() {
                return files_from_lines(&String::from_utf8_lossy(&o.stdout));
            }
        }
    }
    Vec::new()
}

/// One path per line (as osascript prints them) -> the ones that exist, in order, each once.
pub fn files_from_lines(text: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for l in text.lines().map(str::trim).filter(|l| l.starts_with('/')) {
        let p = PathBuf::from(l);
        if p.exists() && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// `Service order 2026-10-18.txt`: safe on any disk (no slashes or odd characters in the title).
pub fn run_sheet_file_name(title: &str, stamp: String) -> String {
    let clean: String = title.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    let clean = clean.trim();
    format!("{} {}.txt", if clean.is_empty() { "Service order" } else { clean }, stamp)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

impl RhemaFlowApp {
    /// Menu: Scripture > Go to the reference on the clipboard. Says plainly if it is not a reference.
    pub fn clip_go_to_reference(&mut self) {
        let text = crate::macmenu::read_clipboard();
        let note = self.clip_go_to_text(&text);
        self.short_utterance_note = Some((note, std::time::Instant::now()));
    }

    /// The reference is read the same way the Scripture search box reads it ("jn 3 16", "John 3:16-18").
    /// A chapter ("John 3") opens that chapter in the browser instead of putting anything on the stage.
    pub fn clip_go_to_text(&mut self, text: &str) -> String {
        let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
        if first.is_empty() {
            return "The clipboard is empty.".into();
        }
        let short: String = first.chars().take(60).collect();
        match crate::browser::reference_from_text(self.text_bible(), first) {
            Ok(p) if p.from > 0 => {
                self.stage_by_hand(p.book as usize, p.chapter as usize, p.from, p.to);
                format!("Jumped to {short}.")
            }
            Ok(p) => {
                self.browser.visible = true;
                self.browser.pick_book(p.book as usize);
                self.browser.chapter = p.chapter as usize;
                self.browser.scroll_pending = true;
                format!("Opened {short} in the Scripture panel (a whole chapter is not put on the stage).")
            }
            Err(e) => format!("Not a reference: \"{short}\" ({e})."),
        }
    }

    /// Menu: Scripture > Copy verse / Copy verse with reference / Copy reference.
    pub fn clip_copy_verse(&mut self, ctx: &egui::Context, kind: CopyKind) {
        let note = if self.stage.kind != StageKind::Scripture {
            "There is no scripture on the stage to copy.".to_string()
        } else {
            match verse_copy(kind, &self.stage.label, &self.stage.text, &self.stage.translation) {
                Some(s) => {
                    ctx.copy_text(s);
                    match kind {
                        CopyKind::Text => "Copied the verse.".to_string(),
                        CopyKind::Reference => format!("Copied \"{}\".", self.stage.label),
                        CopyKind::WithReference => "Copied the verse with its reference.".to_string(),
                    }
                }
                None => "There is nothing to copy.".to_string(),
            }
        };
        self.short_utterance_note = Some((note, std::time::Instant::now()));
    }

    /// Cmd+V when no text box has the cursor (v0.844.0). Decides by what is on the clipboard:
    ///   1. files copied in Finder (or in the MEDIA bar)  -> added to the MEDIA bar
    ///   2. a file path                                   -> that file is added
    ///   3. several lines of words                        -> a new song
    /// `text_hint` is the text egui already read (other systems); on a Mac it is read with pbpaste.
    pub fn paste_anything(&mut self, ctx: &egui::Context, text_hint: Option<String>) {
        use crate::selection::{classify_pasted_text, PastedText};
        let files = read_clipboard_files();
        let note = if !files.is_empty() {
            self.paste_files(ctx, &files)
        } else {
            let text = match text_hint {
                Some(t) if !t.is_empty() => t,
                _ => crate::macmenu::read_clipboard(),
            };
            let home = std::env::var("HOME").ok();
            match path_from_clipboard(&text, home.as_deref()) {
                Some(p) if p.exists() => self.paste_files(ctx, &[p]),
                _ => match classify_pasted_text(&text) {
                    PastedText::Song => match self.present.import_pasted_song(&text, "") {
                        Some(id) => {
                            let title = self.present.library.get(id).map(|s| s.title.clone()).unwrap_or_default();
                            self.present.open_at(crate::present::Tab::Songs);
                            format!("Pasted as a new song: \"{title}\". Check the section names in Songs, then press Cmd+S to save.")
                        }
                        None => "Those words could not be made into a song.".to_string(),
                    },
                    PastedText::OneLine => "Nothing to paste here: one line is not a song. Copy files in Finder, a file path, or the whole song's words. (A Bible reference: Shift+Cmd+V.)".to_string(),
                    PastedText::Empty => "The clipboard is empty.".to_string(),
                },
            }
        };
        self.present.dock_open = true;
        self.short_utterance_note = Some((note, std::time::Instant::now()));
    }

    /// Add copied files (and whole folders) to the MEDIA bar, into the folder being looked at.
    fn paste_files(&mut self, ctx: &egui::Context, files: &[PathBuf]) -> String {
        for p in files {
            if p.is_dir() {
                self.present.add_directory(ctx, p);
            } else {
                self.present.route_path(ctx, p, crate::present::PickFor::Media);
            }
        }
        format!("Pasting {} file{} into the MEDIA bar (you can keep working while they are added).", files.len(), if files.len() == 1 { "" } else { "s" })
    }

    /// Cmd+C when no text box has the cursor: the selected files, else the verse on the stage.
    pub fn copy_anything(&mut self, ctx: &egui::Context) {
        if !self.present.dock_sel.is_empty() {
            let note = self.present.dock_copy_selected(ctx);
            self.short_utterance_note = Some((note, std::time::Instant::now()));
        } else if self.stage.kind == StageKind::Scripture {
            self.clip_copy_verse(ctx, CopyKind::WithReference);
        } else {
            self.short_utterance_note = Some(("Nothing is selected to copy. Click a file in the MEDIA bar first (Cmd+click or Shift+click for several, or drag a box round them).".into(), std::time::Instant::now()));
        }
    }

    /// Menu: Media > Add the file whose path is on the clipboard.
    pub fn clip_add_path(&mut self, ctx: &egui::Context) {
        let text = crate::macmenu::read_clipboard();
        let home = std::env::var("HOME").ok();
        match path_from_clipboard(&text, home.as_deref()) {
            Some(p) if p.exists() => self.present.route_path(ctx, &p, crate::present::PickFor::Media),
            Some(p) => self.present.say(format!("Cannot find \"{}\".", p.display())),
            None => self.present.say("The clipboard does not hold a file path. In Finder, select the file and press Option+Cmd+C to copy its path."),
        }
        self.present.dock_open = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_verse_is_copied_in_three_shapes_and_never_empty() {
        let t = "  For God so loved\n the world  ";
        assert_eq!(verse_copy(CopyKind::Text, "John 3:16", t, "ASV").unwrap(), "For God so loved the world");
        assert_eq!(verse_copy(CopyKind::Reference, "John 3:16", t, "ASV").unwrap(), "John 3:16");
        assert_eq!(verse_copy(CopyKind::WithReference, "John 3:16", t, "ASV").unwrap(), "For God so loved the world (John 3:16, ASV)");
        assert_eq!(verse_copy(CopyKind::WithReference, "John 3:16", t, "").unwrap(), "For God so loved the world (John 3:16)");
        assert_eq!(verse_copy(CopyKind::WithReference, "", t, "ASV").unwrap(), "For God so loved the world");
        assert!(verse_copy(CopyKind::Text, "John 3:16", "  ", "ASV").is_none());
        assert!(verse_copy(CopyKind::Reference, " ", "x", "ASV").is_none());
    }

    #[test]
    fn clipboard_file_lines_keep_real_files_once_and_in_order() {
        let dir = std::env::temp_dir().join(format!("rf-clip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.png"), dir.join("b.mp4"));
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"x").unwrap();
        let text = format!("{}\n{}\n{}\n/nope/missing.png\nnot a path\n", a.display(), b.display(), a.display());
        assert_eq!(files_from_lines(&text), vec![a, b]);
        assert!(files_from_lines("").is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_sheet_files_have_safe_names() {
        assert_eq!(run_sheet_file_name("Sunday/AM: 10", "2026-10-18".into()), "Sunday_AM_ 10 2026-10-18.txt");
        assert_eq!(run_sheet_file_name("  ", "2026-10-18".into()), "Service order 2026-10-18.txt");
    }

    #[test]
    fn a_pasted_path_is_cleaned_in_every_way_a_mac_gives_it() {
        let h = Some("/Users/me");
        let p = |s: &str| path_from_clipboard(s, h).map(|p| p.to_string_lossy().to_string());
        assert_eq!(p("/Users/me/Movies/a b.mp4\n").unwrap(), "/Users/me/Movies/a b.mp4");
        assert_eq!(p("\"/Users/me/Movies/a b.mp4\"").unwrap(), "/Users/me/Movies/a b.mp4");
        assert_eq!(p("/Users/me/Movies/a\\ b.mp4").unwrap(), "/Users/me/Movies/a b.mp4");
        assert_eq!(p("file:///Users/me/Movies/a%20b.mp4").unwrap(), "/Users/me/Movies/a b.mp4");
        assert_eq!(p("file://localhost/tmp/x.png").unwrap(), "/tmp/x.png");
        assert_eq!(p("~/Pictures/x.png").unwrap(), "/Users/me/Pictures/x.png");
        assert_eq!(p("\n\n  /tmp/x.png  \nsecond line").unwrap(), "/tmp/x.png");
        assert!(p("").is_none() && p("John 3:16").is_none() && p("/").is_none());
        assert!(path_from_clipboard("~/x.png", None).is_none());
        // a stray % that is not a code is left alone
        assert_eq!(p("file:///tmp/100%.png").unwrap(), "/tmp/100%.png");
    }
}
