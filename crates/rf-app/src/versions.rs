//! Bible versions (v0.812.0).
//!
//! KJV and BBE are built in. Any other translation is a file in the Bibles folder:
//!
//!   macOS : ~/Library/Application Support/RhemaFlow/Bibles/
//!   other : ~/.local/share/RhemaFlow/Bibles/
//!
//! `*.json` (three common layouts) or `*.tsv`/`*.txt` (Book<TAB>Ch<TAB>V<TAB>text) -
//! see `rf_bible::Bible::from_any`. The file name (without extension) is the name
//! shown in the translation menu (`ASV.json` -> "ASV").
//!
//! Public-domain versions can be downloaded from inside the app (a GitHub-hosted
//! copy, fetched with the system `curl`; no extra dependency). Licensed versions
//! (NIV, ESV, NKJV, NLT...) cannot be bundled or scraped: get the text from your
//! licence (e.g. API.Bible) with `tools/fetch_bible.py` and drop the file in the folder.

use rf_bible::Bible;
use std::path::{Path, PathBuf};

pub const BASE: &str = "https://raw.githubusercontent.com/scrollmapper/bible_databases/master/formats/json";

/// Public-domain English versions offered for one-click download: (menu name, file, description).
pub const DOWNLOADS: &[(&str, &str, &str)] = &[
    ("ASV", "ASV", "American Standard Version (1901)"),
    ("YLT", "YLT", "Young's Literal Translation (1898)"),
    ("Darby", "Darby", "Darby Bible (1890)"),
    ("Webster", "Webster", "Webster's Bible (1833)"),
    ("Rotherham", "Rotherham", "Rotherham's Emphasized Bible (1902)"),
    ("AKJV", "AKJV", "American King James Version"),
];

pub fn bibles_dir() -> PathBuf {
    let home = crate::export::home();
    if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support").join("RhemaFlow").join("Bibles")
    } else {
        home.join(".local").join("share").join("RhemaFlow").join("Bibles")
    }
}

/// Menu name for a file: "ASV.json" -> "ASV". None for files we do not read.
pub fn name_for(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(ext.as_str(), "json" | "tsv" | "txt") {
        return None;
    }
    let stem = path.file_stem()?.to_str()?.trim().to_string();
    if stem.is_empty() || stem.starts_with('.') || stem.eq_ignore_ascii_case("KJV") || stem.eq_ignore_ascii_case("BBE") {
        return None; // built in
    }
    Some(stem)
}

/// Where Bible files are looked for: the user's folder first (it wins), then the `bibles` folder
/// that ships with the app (next to the program, or the folder it was started from).
pub fn search_dirs() -> Vec<PathBuf> {
    let mut v = vec![bibles_dir()];
    if let Ok(cwd) = std::env::current_dir() {
        v.push(cwd.join("bibles"));
    }
    if let Ok(exe) = std::env::current_exe() {
        for up in 1..=4 {
            if let Some(dir) = exe.ancestors().nth(up) {
                let c = dir.join("bibles");
                if !v.contains(&c) {
                    v.push(c);
                }
            }
        }
    }
    v
}

/// Files from several folders; when the same name is in two, the earlier folder wins.
pub fn scan_many(dirs: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for d in dirs {
        for (name, path) in scan(d) {
            if !out.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                out.push((name, path));
            }
        }
    }
    out.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));
    out
}

/// Every candidate file in the folder, sorted by name.
pub fn scan(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| Some((name_for(&e.path())?, e.path()))).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));
    v.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
    v
}

/// Read and validate one file. The Bible is leaked on purpose: a few MB held for the
/// life of the app, which lets the rest of the code keep its `&'static Bible` API.
pub fn load(path: &Path) -> Result<&'static Bible, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let b = Bible::from_any(&text)?;
    Ok(Box::leak(Box::new(b)))
}

/// Result of a background load/download, sent to the UI.
pub enum VersionEvent {
    Loaded(String, &'static Bible),
    Failed(String, String),
    Downloading(String),
    Done(String),
}

/// Load every file in the folder on a worker thread (building the search index takes a
/// second or two per version, which must not freeze the window).
pub fn load_all_async(dirs: Vec<PathBuf>, tx: crossbeam_channel::Sender<VersionEvent>) {
    std::thread::spawn(move || {
        for (name, path) in scan_many(&dirs) {
            match load(&path) {
                Ok(b) => {
                    let _ = tx.send(VersionEvent::Loaded(name, b));
                }
                Err(e) => {
                    let _ = tx.send(VersionEvent::Failed(name, e));
                }
            }
        }
    });
}

/// Download a public-domain version with the system curl, then load it.
pub fn download_async(menu_name: String, file: String, dir: PathBuf, tx: crossbeam_channel::Sender<VersionEvent>) {
    std::thread::spawn(move || {
        let _ = tx.send(VersionEvent::Downloading(menu_name.clone()));
        if std::fs::create_dir_all(&dir).is_err() {
            let _ = tx.send(VersionEvent::Failed(menu_name, "cannot create the Bibles folder".into()));
            return;
        }
        let dest = dir.join(format!("{menu_name}.json"));
        let part = dir.join(format!(".{menu_name}.part"));
        let url = format!("{BASE}/{file}.json");
        let ok = std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", "180", "-o"])
            .arg(&part)
            .arg(&url)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            let _ = std::fs::remove_file(&part);
            let _ = tx.send(VersionEvent::Failed(menu_name, "download failed (no internet?)".into()));
            return;
        }
        // validate BEFORE it joins the list, so a bad download never sits in the folder
        match load(&part) {
            Ok(b) => {
                let _ = std::fs::rename(&part, &dest);
                let _ = tx.send(VersionEvent::Loaded(menu_name.clone(), b));
                let _ = tx.send(VersionEvent::Done(menu_name));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                let _ = tx.send(VersionEvent::Failed(menu_name, e));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_become_menu_names_and_builtins_are_skipped() {
        assert_eq!(name_for(Path::new("/x/ASV.json")), Some("ASV".into()));
        assert_eq!(name_for(Path::new("/x/My Church Bible.tsv")), Some("My Church Bible".into()));
        assert_eq!(name_for(Path::new("/x/kjv.json")), None);
        assert_eq!(name_for(Path::new("/x/BBE.json")), None);
        assert_eq!(name_for(Path::new("/x/.ASV.part")), None);
        assert_eq!(name_for(Path::new("/x/readme.md")), None);
    }

    /// Real files: `RF_TEST_BIBLES="/tmp/asv.json:/tmp/en_kjv.json" cargo test real_downloaded -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_downloaded_files_load() {
        let list = std::env::var("RF_TEST_BIBLES").unwrap_or_default();
        for path in list.split(':').filter(|p| !p.is_empty()) {
            let b = load(Path::new(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
            eprintln!("{path}: {} verses, John 3:16 = {}", b.total_verses(), b.verse(42, 3, 16));
            assert!(b.total_verses() > 30_000);
            assert!(!b.verse(42, 3, 16).is_empty());
            let hits = b.search_text("god so loved", 3);
            assert_eq!((hits[0].book, hits[0].chapter, hits[0].verse), (42, 3, 16), "{path}");
        }
    }

    #[test]
    fn the_users_folder_wins_over_the_bundled_one() {
        let base = std::env::temp_dir().join(format!("rf-bibles-multi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (user, bundled) = (base.join("user"), base.join("bundled"));
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&bundled).unwrap();
        std::fs::write(user.join("ASV.json"), "{}").unwrap();
        std::fs::write(bundled.join("asv.json"), "{}").unwrap();
        std::fs::write(bundled.join("YLT.json"), "{}").unwrap();
        let found = scan_many(&[user.clone(), bundled.clone(), base.join("missing")]);
        assert_eq!(found.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), vec!["ASV", "YLT"]);
        assert!(found[0].1.starts_with(&user), "the user's own file is used");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scanning_and_loading_a_folder_end_to_end() {
        let k = Bible::embedded();
        let dir = std::env::temp_dir().join(format!("rf-bibles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // a complete Bible in the tab layout, plus a broken one
        let mut lines = String::new();
        for b in 0..66 {
            for c in 1..=k.chapter_count(b) {
                for v in 1..=k.verse_count(b, c) {
                    lines.push_str(&format!("{}\t{c}\t{v}\t{}\n", k.book_name(b), k.verse(b, c, v).replace("God", "GOD")));
                }
            }
        }
        std::fs::write(dir.join("Test.tsv"), lines).unwrap();
        std::fs::write(dir.join("Broken.json"), "{\"books\":[]}").unwrap();
        std::fs::write(dir.join("notes.md"), "x").unwrap();
        let found = scan(&dir);
        assert_eq!(found.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), vec!["Broken", "Test"]);
        let good = load(&found[1].1).unwrap();
        assert!(good.verse(0, 1, 1).contains("GOD"));
        assert!(load(&found[0].1).is_err());
        // async path delivers both outcomes
        let (tx, rx) = crossbeam_channel::unbounded();
        load_all_async(vec![dir.clone()], tx);
        let mut loaded = 0;
        let mut failed = 0;
        while let Ok(ev) = rx.recv_timeout(std::time::Duration::from_secs(60)) {
            match ev {
                VersionEvent::Loaded(..) => loaded += 1,
                VersionEvent::Failed(..) => failed += 1,
                _ => {}
            }
            if loaded + failed == 2 {
                break;
            }
        }
        assert_eq!((loaded, failed), (1, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
