//! The media library (v0.820.0): pictures and videos are ADDED ONCE to a shelf, then shown later by
//! clicking their thumbnail. Pure logic, std only, no UI.
//!
//! Two ways a file can live on the shelf:
//! * **Copy**: RhemaFlow keeps its own copy inside the library folder. Unplug the USB stick, move
//!   the original, rename it: the shelf still works. Costs disk space.
//! * **Link**: the shelf only remembers where the original is. No extra space, but it breaks when
//!   the original moves. Broken links show as "missing" and can be re-linked.
//! `Policy::Auto` copies everything up to a size limit and links bigger videos.
//! The same content is stored once on disk, but it may be listed as many times as you like (v0.840.0): adding a
//! picture or video twice, or pressing Duplicate, makes a second item that shares the stored file. That is handy for
//! showing the same clip with two different "when it ends" steps, or the same picture in two folders.

use crate::media::{classify, Classified, MediaKind};
use std::fs;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Copy, Link }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Copy up to this many bytes, link anything bigger. (Default: 300 MB.)
    Auto { copy_up_to: u64 },
    AlwaysCopy,
    AlwaysLink,
}
/// v0.832.0: always copy, however big (a linked file broke when moved, and 754 MB videos were never imported the same way as small ones).
impl Default for Policy { fn default() -> Self { Policy::AlwaysCopy } }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: u64,
    pub name: String,
    /// The file that is shown. For Copy it is inside the library; for Link it is the original.
    pub path: String,
    /// Where it came from (for Copy this is only a memory, so "Show in folder" and re-adding work).
    pub original: String,
    pub mode: Mode,
    pub size: u64,
    /// Fingerprint of the bytes: "size-hash" in hex.
    pub fingerprint: String,
    /// Shelf name ("Songs", "Church"...). Empty = no shelf.
    pub folder: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Added { New(u64), AlreadyThere(u64) }

#[derive(Debug, PartialEq, Eq)]
pub enum LibError { NotMedia(String), Io(String), NotThisFile, NoSuchEntry }
impl std::fmt::Display for LibError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibError::NotMedia(m) => write!(f, "{m}"),
            LibError::Io(m) => write!(f, "Could not read or write the file ({m})."),
            LibError::NotThisFile => write!(f, "That is a different file from the one on the shelf."),
            LibError::NoSuchEntry => write!(f, "That item is not on the shelf."),
        }
    }
}

pub struct MediaLibrary { dir: PathBuf, pub entries: Vec<Entry>, next_id: u64, /// folders made by the operator, even while empty
    made: Vec<String>,
    /// stored files that changed name: (old path, new path); `resolve_moved` follows them, `take_moves` hands them to the app
    moves: Vec<(String, String)>, pending_moves: Vec<(String, String)>,
    /// items taken off the shelf this run, newest last, so "Undo remove" can bring them back
    removed: Vec<(Entry, Option<PathBuf>)>,
    /// when each item was last shown: id -> unix seconds
    usage: std::collections::HashMap<u64, u64> }

/// FNV-1a 64 over the whole file, read in chunks. Fast and good enough to tell files apart.
pub fn fingerprint(path: &Path) -> std::io::Result<String> {
    fingerprint_progress(path, None)
}

/// Same, adding the bytes read so far to `done` (so a progress bar can follow along).
pub fn fingerprint_progress(path: &Path, done: Option<&AtomicU64>) -> std::io::Result<String> {
    let mut f = fs::File::open(path)?;
    let (mut h, mut n, mut buf) = (0xcbf29ce484222325u64, 0u64, vec![0u8; 1 << 20]);
    loop {
        let k = f.read(&mut buf)?;
        if k == 0 { break; }
        for b in &buf[..k] { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
        n += k as u64;
        if let Some(d) = done { d.fetch_add(k as u64, Ordering::Relaxed); }
    }
    Ok(format!("{n:x}-{h:016x}"))
}


/// Everything slow about adding a file, done WITHOUT touching the library, so it can run on a
/// background thread while the window stays smooth: read it, fingerprint it, copy it if needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared { pub name: String, pub path: String, pub original: String, pub mode: Mode, pub size: u64, pub fingerprint: String }

fn mode_for(policy: Policy, size: u64) -> Mode {
    match policy {
        Policy::AlwaysCopy => Mode::Copy,
        Policy::AlwaysLink => Mode::Link,
        Policy::Auto { copy_up_to } => if size <= copy_up_to { Mode::Copy } else { Mode::Link },
    }
}

/// How many bytes of reading and writing adding this file will take (for a progress bar).
pub fn planned_work(src: &Path, policy: Policy) -> u64 {
    let size = fs::metadata(src).map(|m| m.len()).unwrap_or(0);
    size * if mode_for(policy, size) == Mode::Copy { 2 } else { 1 }
}

/// Fingerprint and (when the policy says so) copy `src` into `<dir>/files/`. A file whose copy is
/// already there is not copied again. `done` counts bytes handled.
pub fn prepare(src: &Path, policy: Policy, dir: &Path, done: Option<&AtomicU64>) -> Result<Prepared, LibError> {
    let s = src.to_string_lossy().to_string();
    match classify(&s) {
        Classified::Ok(_) => {}
        Classified::Unsupported(m) => return Err(LibError::NotMedia(m)),
        Classified::Unknown => return Err(LibError::NotMedia(format!("\"{}\" is not a picture, video or sound file.", file_name(src)))),
    }
    let size = fs::metadata(src).map_err(|e| LibError::Io(e.to_string()))?.len();
    let fp = fingerprint_progress(src, done).map_err(|e| LibError::Io(e.to_string()))?;
    let mode = mode_for(policy, size);
    let path = if mode == Mode::Copy {
        let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let files = dir.join("files");
        fs::create_dir_all(&files).map_err(|e| LibError::Io(e.to_string()))?;
        // v0.845.0: the stored copy keeps the file's OWN name ("Everlasting God.mp4"), not a hash, so the folder
        // on disk reads the same as the MEDIA bar. Same bytes are still stored once (looked up by fingerprint);
        // a different file with the same name becomes "Name (2).ext".
        let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let dest = match stored_for(dir, &fp) {
            Some(existing) => {
                if let Some(d) = done { d.fetch_add(size, Ordering::Relaxed); }
                existing
            }
            None => {
                let dest = free_stored_name(&files, &safe_stem(&stem), &ext);
                let tmp = files.join(format!("{fp}.part"));
                let copy = || -> std::io::Result<()> {
                    let mut r = fs::File::open(src)?;
                    let mut w = fs::File::create(&tmp)?;
                    let mut buf = vec![0u8; 1 << 20];
                    loop {
                        let k = r.read(&mut buf)?;
                        if k == 0 { break; }
                        w.write_all(&buf[..k])?;
                        if let Some(d) = done { d.fetch_add(k as u64, Ordering::Relaxed); }
                    }
                    w.sync_all()
                };
                if let Err(e) = copy() { let _ = fs::remove_file(&tmp); return Err(LibError::Io(e.to_string())); }
                fs::rename(&tmp, &dest).map_err(|e| { let _ = fs::remove_file(&tmp); LibError::Io(e.to_string()) })?;
                remember_stored(dir, &fp, &dest);
                dest
            }
        };
        dest.to_string_lossy().to_string()
    } else { s.clone() };
    let name = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into());
    Ok(Prepared { name, path, original: s, mode, size, fingerprint: fp })
}

/// A name that is safe on disk: no slashes or colons, no control characters, no leading dots, not too long.
fn safe_stem(stem: &str) -> String {
    let cleaned: String = stem.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { ' ' } else { c }).collect();
    let t = cleaned.trim().trim_start_matches('.').trim();
    let t: String = t.chars().take(120).collect();
    let t = t.trim().to_string();
    if t.is_empty() { "Untitled".to_string() } else { t }
}

/// "Name.ext", else "Name (2).ext", "Name (3).ext" ... the first one not already in `files`.
fn free_stored_name(files: &Path, stem: &str, ext: &str) -> PathBuf {
    let make = |n: u32| {
        let base = if n <= 1 { stem.to_string() } else { format!("{stem} ({n})") };
        files.join(if ext.is_empty() { base } else { format!("{base}.{ext}") })
    };
    let mut n = 1;
    loop {
        let c = make(n);
        if !c.exists() { return c; }
        n += 1;
    }
}

/// "pdf" or ".mp4" typed in the search box finds every file of that type.
fn ext_matches(path: &str, f: &str) -> bool {
    let f = f.trim_start_matches('.');
    !f.is_empty() && Path::new(path).extension().map_or(false, |x| x.to_string_lossy().to_lowercase() == f)
}

fn names_index(dir: &Path) -> PathBuf { dir.join("stored-names.tsv") }

/// The stored copy already made from these exact bytes, if it is still there.
fn stored_for(dir: &Path, fp: &str) -> Option<PathBuf> {
    let text = fs::read_to_string(names_index(dir)).ok()?;
    for line in text.lines().rev() {
        if let Some((f, name)) = line.split_once('\t') {
            if f == fp {
                let p = dir.join("files").join(name);
                if p.exists() { return Some(p); }
            }
        }
    }
    // copies made before v0.845.0 are named by their fingerprint
    let files = dir.join("files");
    if let Ok(rd) = fs::read_dir(&files) {
        for e in rd.flatten() {
            let p = e.path();
            if p.file_stem().map(|s| s.to_string_lossy() == fp).unwrap_or(false) && p.extension().map_or(true, |x| x != "part") { return Some(p); }
        }
    }
    None
}

fn remember_stored(dir: &Path, fp: &str, dest: &Path) {
    if let Some(name) = dest.file_name() {
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(names_index(dir)) {
            let _ = writeln!(f, "{fp}\t{}", name.to_string_lossy());
        }
    }
}

/// A warning for a very big file (v0.846.0): they copy slowly, fill the disk and can stutter on the big screen.
pub fn size_warning(size: u64) -> Option<String> {
    const GB: u64 = 1 << 30;
    if size >= 2 * GB { Some(format!("This file is {:.1} GB. That is very big: copying takes minutes and it may stutter on the big screen. Consider a smaller export (1080p).", size as f64 / GB as f64)) }
    else if size >= GB / 2 { Some(format!("This file is {:.0} MB: a big one. Check it plays smoothly on the big screen before Sunday.", size as f64 / 1_048_576.0)) }
    else { None }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy { Added, Newest, Name, Recent }

fn clean(s: &str) -> String { s.replace(['\t', '\n', '\r'], " ") }
fn file_name(p: &Path) -> String { p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default() }

impl MediaLibrary {
    /// Open (or start) the library in `dir`. A damaged line is skipped, never fatal.
    pub fn open(dir: &Path) -> MediaLibrary {
        let mut lib = MediaLibrary { dir: dir.to_path_buf(), entries: Vec::new(), next_id: 1, made: Vec::new(), moves: Vec::new(), pending_moves: Vec::new(), removed: Vec::new(), usage: Default::default() };
        if let Ok(text) = fs::read_to_string(dir.join("library.tsv")) {
            for line in text.lines() {
                if let Some(name) = line.strip_prefix("F\t") {
                    let n = name.trim();
                    if !n.is_empty() && !lib.made.iter().any(|m| m == n) { lib.made.push(n.to_string()); }
                    continue;
                }
                let p: Vec<&str> = line.split('\t').collect();
                if p.len() != 9 { continue; }
                let (Ok(id), Ok(size)) = (p[0].parse::<u64>(), p[5].parse::<u64>()) else { continue };
                let mode = if p[4] == "copy" { Mode::Copy } else { Mode::Link };
                lib.next_id = lib.next_id.max(id + 1);
                lib.entries.push(Entry { id, name: p[1].into(), path: p[2].into(), original: p[3].into(), mode, size, fingerprint: p[6].into(), folder: p[7].into() });
            }
        }
        if let Ok(text) = fs::read_to_string(dir.join("path-moves.tsv")) {
            for line in text.lines() { if let Some((a, b)) = line.split_once('\t') { lib.moves.push((a.to_string(), b.to_string())); } }
        }
        if let Ok(text) = fs::read_to_string(dir.join("usage.tsv")) {
            for line in text.lines() { if let Some((a, b)) = line.split_once('\t') { if let (Ok(i), Ok(t)) = (a.parse::<u64>(), b.parse::<u64>()) { lib.usage.insert(i, t); } } }
        }
        lib
    }

    fn note_move(&mut self, old: &str, new: &str) {
        if old == new { return; }
        self.moves.push((old.to_string(), new.to_string()));
        self.pending_moves.push((old.to_string(), new.to_string()));
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(self.dir.join("path-moves.tsv")) { let _ = writeln!(f, "{old}\t{new}"); }
    }

    /// Paths of stored files renamed since the last call, so the app can fix the lists that remember them.
    pub fn take_moves(&mut self) -> Vec<(String, String)> { std::mem::take(&mut self.pending_moves) }

    /// A path remembered before a stored file was renamed -> where that file is now (unchanged if it still exists or is unknown).
    pub fn resolve_moved(&self, path: &str) -> String {
        let mut cur = path.to_string();
        for _ in 0..16 {
            if Path::new(&cur).exists() { break; }
            match self.moves.iter().rev().find(|(o, _)| *o == cur) { Some((_, n)) => cur = n.clone(), None => break }
        }
        cur
    }

    /// `resolve_moved`, and when the file is still not there: the shelf item that was made from that path, or that has
    /// the same file name (a saved service order points at files by path; the shelf knows them by item). v0.848.0.
    pub fn resolve_any(&self, path: &str) -> String {
        let r = self.resolve_moved(path);
        if Path::new(&r).exists() { return r; }
        let want = Path::new(path).file_name().map(|n| n.to_string_lossy().to_lowercase());
        let same_name = |p: &str| want.is_some() && Path::new(p).file_name().map(|n| n.to_string_lossy().to_lowercase()) == want;
        if let Some(e) = self.entries.iter().find(|e| e.original == path && Path::new(&e.path).exists()) { return e.path.clone(); }
        if let Some(e) = self.entries.iter().find(|e| (same_name(&e.path) || same_name(&e.original)) && Path::new(&e.path).exists()) { return e.path.clone(); }
        r
    }

    /// How many whole days ago item `id` was last shown, as of `now` (unix seconds). None = never shown since this
    /// was tracked (v0.850.0).
    pub fn shown_days_ago_at(&self, id: u64, now: u64) -> Option<u64> {
        self.usage.get(&id).map(|t| now.saturating_sub(*t) / 86_400)
    }

    pub fn shown_days_ago(&self, id: u64) -> Option<u64> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        self.shown_days_ago_at(id, now)
    }

    /// Items last shown more than `days` ago (never-shown items are not listed: they may be new), as of `now`.
    pub fn unused_ids_at(&self, days: u64, now: u64) -> Vec<u64> {
        self.entries.iter().map(|e| e.id).filter(|id| self.shown_days_ago_at(*id, now).map_or(false, |d| d > days)).collect()
    }

    /// Note that item `id` was just shown (feeds "Recently used").
    pub fn touch(&mut self, id: u64) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        self.usage.insert(id, now);
        let mut out = String::new();
        for (i, t) in &self.usage { out.push_str(&format!("{i}\t{t}\n")); }
        let _ = fs::create_dir_all(&self.dir);
        let _ = fs::write(self.dir.join("usage.tsv"), out);
    }

    pub fn save(&self) -> Result<(), LibError> {
        fs::create_dir_all(&self.dir).map_err(|e| LibError::Io(e.to_string()))?;
        let mut out = String::new();
        for f in &self.made { out.push_str(&format!("F\t{}\n", clean(f))); }
        for e in &self.entries {
            out.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t-\n", e.id, clean(&e.name), clean(&e.path), clean(&e.original),
                if e.mode == Mode::Copy { "copy" } else { "link" }, e.size, e.fingerprint, clean(&e.folder)));
        }
        let (tmp, fin) = (self.dir.join("library.tsv.tmp"), self.dir.join("library.tsv"));
        if fin.exists() { let _ = fs::copy(&fin, self.dir.join("library.tsv.bak")); } // one step back, in case an edit goes wrong
        let mut f = fs::File::create(&tmp).map_err(|e| LibError::Io(e.to_string()))?;
        f.write_all(out.as_bytes()).and_then(|_| f.sync_all()).map_err(|e| LibError::Io(e.to_string()))?;
        fs::rename(&tmp, &fin).map_err(|e| LibError::Io(e.to_string()))
    }

    /// Put a file on the shelf (slow part and bookkeeping in one go). Pictures and MP4 only.
    pub fn add(&mut self, src: &Path, policy: Policy, folder: &str) -> Result<Added, LibError> {
        let p = prepare(src, policy, &self.dir, None)?;
        Ok(self.commit(p, folder))
    }

    /// The quick bookkeeping half: register a prepared file. The same bytes may be listed again (v0.840.0): the
    /// stored file is shared, and the second item is named "Name (2)", "Name (3)" ... so they can be told apart.
    pub fn commit(&mut self, p: Prepared, folder: &str) -> Added {
        let same = self.entries.iter().filter(|e| e.fingerprint == p.fingerprint).count();
        let name = if same == 0 { p.name } else { self.unique_name(&p.name) };
        let id = self.next_id; self.next_id += 1;
        self.entries.push(Entry { id, name, path: p.path, original: p.original, mode: p.mode, size: p.size, fingerprint: p.fingerprint, folder: folder.to_string() });
        Added::New(id)
    }

    /// "Holy Lamb" -> "Holy Lamb (2)" -> "Holy Lamb (3)": the first number not already used by an item's name.
    fn unique_name(&self, base: &str) -> String {
        let stem = match base.rfind(" (") {
            Some(i) if base.ends_with(')') && base[i + 2..base.len() - 1].chars().all(|c| c.is_ascii_digit()) && base.len() > i + 3 => &base[..i],
            _ => base,
        };
        let mut n = 2;
        loop {
            let cand = format!("{stem} ({n})");
            if !self.entries.iter().any(|e| e.name == cand) { return cand; }
            n += 1;
        }
    }

    /// A second item for the same file (same folder, same stored copy). Returns its id.
    pub fn duplicate(&mut self, id: u64) -> Result<u64, LibError> {
        let e = self.get(id).ok_or(LibError::NoSuchEntry)?.clone();
        let name = self.unique_name(&e.name);
        let nid = self.next_id; self.next_id += 1;
        self.entries.push(Entry { id: nid, name, ..e });
        Ok(nid)
    }

    /// v0.845.0: copies made by older versions are named by a fingerprint ("1d1bf20-daeae41...mp4"). Give each the
    /// name its item has in the MEDIA bar. Returns how many files were renamed. Safe to run on every start.
    pub fn tidy_stored_names(&mut self) -> usize {
        let files = self.dir.join("files");
        let mut done = 0;
        let mut i = 0;
        while i < self.entries.len() {
            let (mode, path, fp, name) = { let e = &self.entries[i]; (e.mode, e.path.clone(), e.fingerprint.clone(), e.name.clone()) };
            i += 1;
            let old = PathBuf::from(&path);
            let hashed = old.parent() == Some(files.as_path()) && old.file_stem().map(|s| s.to_string_lossy() == fp).unwrap_or(false);
            if mode != Mode::Copy || !hashed || !old.exists() { continue; }
            let ext = old.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
            let stem = match name.rfind(" (") {
                Some(k) if name.ends_with(')') && name[k + 2..name.len() - 1].chars().all(|c| c.is_ascii_digit()) && name.len() > k + 3 => name[..k].to_string(),
                _ => name.clone(),
            };
            let dest = free_stored_name(&files, &safe_stem(&stem), &ext);
            if fs::rename(&old, &dest).is_err() { continue; }
            let new_path = dest.to_string_lossy().to_string();
            for e in self.entries.iter_mut().filter(|e| e.path == path) { e.path = new_path.clone(); }
            remember_stored(&self.dir, &fp, &dest);
            self.note_move(&path, &new_path);
            done += 1;
        }
        if done > 0 { let _ = self.save(); }
        done
    }

    /// v0.845.0 health check: stored copies in `files/` that no item uses any more (left behind by a crash, or by an
    /// old version). Returns (path, bytes). Nothing is deleted here.
    pub fn orphans(&self) -> Vec<(PathBuf, u64)> {
        let files = self.dir.join("files");
        let used: std::collections::HashSet<&str> = self.entries.iter().map(|e| e.path.as_str()).collect();
        let mut out = Vec::new();
        if let Ok(rd) = fs::read_dir(&files) {
            for e in rd.flatten() {
                let p = e.path();
                let md = match e.metadata() { Ok(m) if m.is_file() => m, _ => continue };
                if p.extension().map_or(false, |x| x == "part") { continue; } // a copy in progress
                if !used.contains(p.to_string_lossy().as_ref()) { out.push((p, md.len())); }
            }
        }
        out.sort();
        out
    }

    /// Delete the files `orphans` lists. Returns (how many, bytes freed). Never touches a file an item uses.
    pub fn remove_orphans(&mut self) -> (usize, u64) {
        let (mut n, mut bytes) = (0, 0);
        for (p, len) in self.orphans() {
            if fs::remove_file(&p).is_ok() { n += 1; bytes += len; }
        }
        (n, bytes)
    }

    /// Where the library keeps its own copies and list.
    pub fn dir(&self) -> &Path { &self.dir }

    pub fn get(&self, id: u64) -> Option<&Entry> { self.entries.iter().find(|e| e.id == id) }
    pub fn is_missing(&self, id: u64) -> bool { self.get(id).map_or(true, |e| !Path::new(&e.path).exists()) }
    pub fn kind(&self, id: u64) -> Option<MediaKind> {
        match classify(&self.get(id)?.path) { Classified::Ok(k) => Some(k), _ => None }
    }
    /// The file to put on the stage, or None when it is missing (the UI then offers Re-link).
    pub fn path_to_show(&self, id: u64) -> Option<PathBuf> {
        let e = self.get(id)?; let p = PathBuf::from(&e.path); if p.exists() { Some(p) } else { None }
    }

    /// Point a broken link at the file's new place. Refuses a different file unless `force`.
    pub fn relink(&mut self, id: u64, new: &Path, force: bool) -> Result<(), LibError> {
        let fp = fingerprint(new).map_err(|e| LibError::Io(e.to_string()))?;
        let e = self.entries.iter_mut().find(|e| e.id == id).ok_or(LibError::NoSuchEntry)?;
        if e.fingerprint != fp && !force { return Err(LibError::NotThisFile); }
        e.path = new.to_string_lossy().to_string(); e.original = e.path.clone(); e.fingerprint = fp; e.mode = Mode::Link;
        e.size = fs::metadata(new).map(|m| m.len()).unwrap_or(e.size);
        Ok(())
    }

    /// Take it off the shelf. Deletes the library's own copy; NEVER deletes a linked original.
    pub fn remove(&mut self, id: u64) -> Result<(), LibError> {
        let i = self.entries.iter().position(|e| e.id == id).ok_or(LibError::NoSuchEntry)?;
        let e = self.entries.remove(i);
        let mut trashed = None;
        if e.mode == Mode::Copy && !self.entries.iter().any(|o| o.path == e.path) {
            // v0.846.0: the stored copy goes to <library>/trash (emptied after 30 days), so "Undo remove" can bring it back
            let src = PathBuf::from(&e.path);
            if src.exists() {
                let trash = self.dir.join("trash");
                let ext = src.extension().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                let stem = src.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                let dest = if fs::create_dir_all(&trash).is_ok() { Some(free_stored_name(&trash, &safe_stem(&stem), &ext)) } else { None };
                match dest {
                    Some(d) if fs::rename(&src, &d).is_ok() => trashed = Some(d),
                    _ => { let _ = fs::remove_file(&src); }
                }
            }
        }
        self.removed.push((e, trashed));
        if self.removed.len() > 50 { let (_, old) = self.removed.remove(0); if let Some(p) = old { let _ = fs::remove_file(p); } }
        Ok(())
    }

    /// Bring back the item removed last. Returns its id, or None when there is nothing to undo.
    pub fn undo_remove(&mut self) -> Option<u64> {
        let (mut e, trashed) = self.removed.pop()?;
        if let Some(t) = trashed {
            let files = self.dir.join("files");
            let _ = fs::create_dir_all(&files);
            let ext = t.extension().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            let stem = t.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            let dest = free_stored_name(&files, &safe_stem(&stem), &ext);
            if fs::rename(&t, &dest).is_err() { return None; }
            let np = dest.to_string_lossy().to_string();
            if np != e.path { let old = e.path.clone(); e.path = np.clone(); self.note_move(&old, &np); }
            remember_stored(&self.dir, &e.fingerprint, &dest);
        }
        if self.entries.iter().any(|o| o.id == e.id) { e.id = self.next_id; self.next_id += 1; }
        let id = e.id;
        self.entries.push(e);
        Some(id)
    }

    /// How many removed items could still be brought back this run.
    pub fn can_undo_remove(&self) -> usize { self.removed.len() }

    /// Delete trash files older than `days`. Returns how many went. Safe to run on every start.
    pub fn purge_trash(&self, days: u64) -> usize {
        let mut n = 0;
        if let Ok(rd) = fs::read_dir(self.dir.join("trash")) {
            for e in rd.flatten() {
                let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map_or(false, |a| a.as_secs() > days * 86_400);
                if old && fs::remove_file(e.path()).is_ok() { n += 1; }
            }
        }
        n
    }

    /// Items whose bytes are identical, listed more than once: groups of item ids (v0.846.0).
    pub fn duplicate_groups(&self) -> Vec<Vec<u64>> {
        let mut by: std::collections::BTreeMap<&str, Vec<u64>> = Default::default();
        for e in &self.entries { by.entry(e.fingerprint.as_str()).or_default().push(e.id); }
        by.into_values().filter(|v| v.len() > 1).collect()
    }

    /// Re-link every missing linked item at once: look inside `dir` (and its folders, 4 levels deep) for a file with
    /// the same name AND the same bytes. Returns (re-linked, still missing).
    pub fn relink_all(&mut self, dir: &Path) -> (usize, usize) {
        let mut found: Vec<PathBuf> = Vec::new();
        fn walk(d: &Path, depth: u32, out: &mut Vec<PathBuf>) {
            if depth > 4 { return; }
            if let Ok(rd) = fs::read_dir(d) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() { walk(&p, depth + 1, out); } else if p.is_file() { out.push(p); }
                }
            }
        }
        walk(dir, 0, &mut found);
        let missing: Vec<u64> = self.missing_ids();
        let (mut ok, mut left) = (0, 0);
        for id in missing {
            let Some(e) = self.get(id).cloned() else { continue };
            let want = Path::new(&e.original).file_name().or_else(|| Path::new(&e.path).file_name()).map(|n| n.to_string_lossy().to_lowercase());
            let mut done = false;
            for c in found.iter().filter(|c| c.file_name().map(|n| n.to_string_lossy().to_lowercase()) == want) {
                if self.relink(id, c, false).is_ok() { done = true; break; }
            }
            if done { ok += 1 } else { left += 1 }
        }
        (ok, left)
    }

    /// Renames the item AND, when its copy is the library's own and no other item shares it, the stored file too
    /// (v0.846.0), so the folder on disk keeps matching the MEDIA bar.
    pub fn rename(&mut self, id: u64, name: &str) -> Result<(), LibError> {
        let n = clean(name); let n = n.trim().to_string();
        let i = self.entries.iter().position(|e| e.id == id).ok_or(LibError::NoSuchEntry)?;
        if n.is_empty() { return Ok(()); }
        self.entries[i].name = n.clone();
        let (mode, path, fp) = { let e = &self.entries[i]; (e.mode, e.path.clone(), e.fingerprint.clone()) };
        let old = PathBuf::from(&path);
        let files = self.dir.join("files");
        let shared = self.entries.iter().any(|o| o.id != id && o.path == path);
        if mode == Mode::Copy && !shared && old.parent() == Some(files.as_path()) && old.exists() {
            let ext = old.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
            let want = safe_stem(&n);
            if old.file_stem().map(|s| s.to_string_lossy().to_string()) != Some(want.clone()) {
                let dest = free_stored_name(&files, &want, &ext);
                if fs::rename(&old, &dest).is_ok() {
                    let np = dest.to_string_lossy().to_string();
                    self.entries[i].path = np.clone();
                    remember_stored(&self.dir, &fp, &dest);
                    self.note_move(&path, &np);
                }
            }
        }
        Ok(())
    }
    pub fn set_folder(&mut self, id: u64, folder: &str) -> Result<(), LibError> {
        self.entries.iter_mut().find(|e| e.id == id).ok_or(LibError::NoSuchEntry)?.folder = clean(folder).trim().to_string(); Ok(())
    }
    /// Folder names: the ones the operator made (in the order made), then any used only by items.
    pub fn folders(&self) -> Vec<String> {
        let mut v: Vec<String> = self.made.clone();
        for e in &self.entries { if !e.folder.is_empty() && !v.contains(&e.folder) { v.push(e.folder.clone()); } }
        v
    }
    /// Make a folder (right-click > New folder). Asking again for an existing name returns that folder.
    pub fn create_folder(&mut self, name: &str) -> Result<String, String> {
        let n = clean(name).trim().to_string();
        if n.is_empty() { return Err("A folder needs a name.".into()); }
        if let Some(old) = self.folders().into_iter().find(|f| f.to_lowercase() == n.to_lowercase()) { return Ok(old); }
        self.made.push(n.clone());
        Ok(n)
    }
    /// Rename a folder; everything inside follows. Refuses a name another folder already has.
    pub fn rename_folder(&mut self, old: &str, new: &str) -> Result<(), String> {
        let n = clean(new).trim().to_string();
        if n.is_empty() { return Err("A folder needs a name.".into()); }
        if n == old { return Ok(()); }
        if self.folders().iter().any(|f| f != old && f.to_lowercase() == n.to_lowercase()) { return Err(format!("There is already a folder called \"{n}\".")); }
        for m in self.made.iter_mut().filter(|m| *m == old) { *m = n.clone(); }
        if !self.made.contains(&n) { self.made.push(n.clone()); }
        for e in self.entries.iter_mut().filter(|e| e.folder == old) { e.folder = n.clone(); }
        Ok(())
    }
    /// Delete a folder. Its files are NOT deleted: they go back to "all media".
    pub fn delete_folder(&mut self, name: &str) {
        self.made.retain(|m| m != name);
        for e in self.entries.iter_mut().filter(|e| e.folder == name) { e.folder.clear(); }
    }
    /// How many items a folder holds (`None` = everything).
    pub fn count_in(&self, folder: Option<&str>) -> usize {
        self.entries.iter().filter(|e| folder.map_or(true, |f| e.folder == f)).count()
    }
    /// Ids to show in the grid: `folder` None = everything; `filter` matches the name, any case.
    pub fn list(&self, folder: Option<&str>, filter: &str) -> Vec<u64> {
        let f = filter.trim().to_lowercase();
        self.entries.iter().filter(|e| folder.map_or(true, |x| e.folder == x) && (f.is_empty() || e.name.to_lowercase().contains(&f) || ext_matches(&e.path, &f))).map(|e| e.id).collect()
    }
    /// Like `list`, in the order asked for (Added = oldest first, Newest = newest first, Name = A to Z).
    pub fn list_sorted(&self, folder: Option<&str>, filter: &str, by: SortBy) -> Vec<u64> {
        let mut v = self.list(folder, filter);
        match by {
            SortBy::Added => {}
            SortBy::Newest => v.reverse(),
            SortBy::Name => v.sort_by_key(|id| self.get(*id).map(|e| e.name.to_lowercase()).unwrap_or_default()),
            SortBy::Recent => v.sort_by_key(|id| std::cmp::Reverse(self.usage.get(id).copied().unwrap_or(0))),
        }
        v
    }
    /// How many items are broken (for a warning badge before Sunday).
    pub fn missing_ids(&self) -> Vec<u64> { self.entries.iter().filter(|e| !Path::new(&e.path).exists()).map(|e| e.id).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("rf-lib-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&d); fs::create_dir_all(&d).unwrap(); d
    }
    fn file(d: &Path, name: &str, bytes: &[u8]) -> PathBuf { let p = d.join(name); fs::write(&p, bytes).unwrap(); p }

    #[test] fn copy_survives_the_original_going_away() {
        let d = tmp(); let src = file(&d, "bg.png", b"pixels-1");
        let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(id) = lib.add(&src, Policy::AlwaysCopy, "Songs").unwrap() else { panic!() };
        fs::remove_file(&src).unwrap();
        assert!(!lib.is_missing(id)); assert_eq!(fs::read(lib.path_to_show(id).unwrap()).unwrap(), b"pixels-1");
        assert_eq!(lib.get(id).unwrap().mode, Mode::Copy);
    }
    #[test] fn link_breaks_then_relinks_only_to_the_same_file() {
        let d = tmp(); let src = file(&d, "a.mp4", b"video-bytes");
        let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(id) = lib.add(&src, Policy::AlwaysLink, "").unwrap() else { panic!() };
        let moved = d.join("moved.mp4"); fs::rename(&src, &moved).unwrap();
        assert!(lib.is_missing(id)); assert_eq!(lib.missing_ids(), vec![id]); assert!(lib.path_to_show(id).is_none());
        let other = file(&d, "other.mp4", b"different");
        assert_eq!(lib.relink(id, &other, false), Err(LibError::NotThisFile));
        lib.relink(id, &moved, false).unwrap(); assert!(!lib.is_missing(id));
    }
    #[test] fn same_bytes_are_stored_once_on_disk_but_may_be_listed_twice() {
        let d = tmp(); let a = file(&d, "a.jpg", b"same"); let b = file(&d, "copy of a.jpg", b"same");
        let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(id) = lib.add(&a, Policy::AlwaysCopy, "").unwrap() else { panic!() };
        let Added::New(id2) = lib.add(&b, Policy::AlwaysCopy, "").unwrap() else { panic!("a repeat is a new item now") };
        assert_ne!(id, id2);
        assert_eq!(lib.entries.len(), 2);
        assert_eq!(lib.get(id).unwrap().path, lib.get(id2).unwrap().path, "one stored file");
        assert_eq!(lib.get(id2).unwrap().name, "copy of a (2)");
        assert_eq!(fs::read_dir(lib.dir().join("files")).unwrap().count(), 1);
        // removing one item keeps the stored file for the other; removing the last one deletes it
        let stored = PathBuf::from(&lib.get(id).unwrap().path);
        lib.remove(id).unwrap();
        assert!(stored.exists());
        lib.remove(id2).unwrap();
        assert!(!stored.exists());
    }
    #[test] fn duplicate_makes_a_numbered_twin_in_the_same_folder() {
        let d = tmp(); let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(id) = lib.add(&file(&d, "Holy Lamb.png", b"1"), Policy::AlwaysCopy, "Songs").unwrap() else { panic!() };
        let a = lib.duplicate(id).unwrap(); let b = lib.duplicate(id).unwrap(); let c = lib.duplicate(a).unwrap();
        let names: Vec<String> = [id, a, b, c].iter().map(|i| lib.get(*i).unwrap().name.clone()).collect();
        assert_eq!(names, vec!["Holy Lamb", "Holy Lamb (2)", "Holy Lamb (3)", "Holy Lamb (4)"]);
        assert_eq!(lib.get(c).unwrap().folder, "Songs");
        assert_eq!(lib.duplicate(999), Err(LibError::NoSuchEntry));
    }
    #[test] fn auto_copies_small_and_links_big() {
        let d = tmp(); let small = file(&d, "s.png", &[1u8; 10]); let big = file(&d, "b.mp4", &[2u8; 100]);
        let mut lib = MediaLibrary::open(&d.join("lib")); let p = Policy::Auto { copy_up_to: 50 };
        let Added::New(s) = lib.add(&small, p, "").unwrap() else { panic!() };
        let Added::New(b) = lib.add(&big, p, "").unwrap() else { panic!() };
        assert_eq!((lib.get(s).unwrap().mode, lib.get(b).unwrap().mode), (Mode::Copy, Mode::Link));
    }
    #[test] fn remove_deletes_own_copy_but_never_the_original() {
        let d = tmp(); let a = file(&d, "a.png", b"x1"); let b = file(&d, "b.png", b"x2");
        let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(ia) = lib.add(&a, Policy::AlwaysCopy, "").unwrap() else { panic!() };
        let Added::New(ib) = lib.add(&b, Policy::AlwaysLink, "").unwrap() else { panic!() };
        let stored = PathBuf::from(&lib.get(ia).unwrap().path);
        lib.remove(ia).unwrap(); lib.remove(ib).unwrap();
        assert!(!stored.exists()); assert!(a.exists()); assert!(b.exists());
        assert_eq!(lib.remove(ia), Err(LibError::NoSuchEntry));
    }
    #[test] fn refuses_files_it_cannot_show_with_advice() {
        let d = tmp(); let mut lib = MediaLibrary::open(&d.join("lib"));
        assert!(matches!(lib.add(&file(&d, "x.mov", b"m"), Policy::default(), ""), Err(LibError::NotMedia(m)) if m.contains("HandBrake")));
        assert!(matches!(lib.add(&file(&d, "x.doc", b"m"), Policy::default(), ""), Err(LibError::NotMedia(_))));
        assert!(matches!(lib.add(&d.join("nope.png"), Policy::default(), ""), Err(LibError::Io(_))));
        assert!(lib.entries.is_empty());
    }
    #[test] fn saves_loads_and_survives_garbage_lines() {
        let d = tmp(); let a = file(&d, "a.png", b"x1"); let lib_dir = d.join("lib");
        let mut lib = MediaLibrary::open(&lib_dir); lib.add(&a, Policy::AlwaysCopy, "Songs").unwrap();
        lib.rename(1, "Holy\tLamb\nbg").unwrap(); lib.save().unwrap();
        let mut text = fs::read_to_string(lib_dir.join("library.tsv")).unwrap(); text.push_str("garbage\n1\t2\n");
        fs::write(lib_dir.join("library.tsv"), text).unwrap();
        let back = MediaLibrary::open(&lib_dir);
        assert_eq!(back.entries.len(), 1); assert_eq!(back.entries[0].name, "Holy Lamb bg"); assert_eq!(back.entries[0].folder, "Songs");
        let mut back = back; let Added::New(id2) = back.add(&file(&d, "z.png", b"zz"), Policy::AlwaysCopy, "").unwrap() else { panic!() };
        assert_eq!(id2, 2);
    }
    #[test] fn folders_and_filter() {
        let d = tmp(); let mut lib = MediaLibrary::open(&d.join("lib"));
        lib.add(&file(&d, "Holy Lamb.png", b"1"), Policy::AlwaysCopy, "Songs").unwrap();
        lib.add(&file(&d, "Welcome.mp4", b"2"), Policy::AlwaysCopy, "Church").unwrap();
        lib.add(&file(&d, "Peace.png", b"3"), Policy::AlwaysCopy, "Songs").unwrap();
        assert_eq!(lib.folders(), vec!["Songs", "Church"]);
        assert_eq!(lib.list(Some("Songs"), ""), vec![1, 3]); assert_eq!(lib.list(None, "WEL"), vec![2]);
        assert_eq!(lib.kind(2), Some(MediaKind::Video));
        lib.set_folder(2, "Songs").unwrap(); assert_eq!(lib.list(Some("Songs"), "").len(), 3);
    }

    #[test] fn folders_can_be_made_empty_renamed_deleted_and_remembered() {
        let d = tmp(); let dir = d.join("lib"); let mut lib = MediaLibrary::open(&dir);
        assert_eq!(lib.create_folder("  Songs ").unwrap(), "Songs");
        assert_eq!(lib.create_folder("songs").unwrap(), "Songs", "same name, any case = the same folder");
        assert!(lib.create_folder("   ").is_err());
        lib.add(&file(&d, "a.png", b"a1"), Policy::AlwaysCopy, "Songs").unwrap();
        lib.create_folder("Church").unwrap();
        assert_eq!(lib.folders(), vec!["Songs", "Church"]);
        assert_eq!((lib.count_in(Some("Songs")), lib.count_in(Some("Church")), lib.count_in(None)), (1, 0, 1));
        lib.rename_folder("Songs", "Worship").unwrap();
        assert_eq!(lib.get(1).unwrap().folder, "Worship");
        assert!(lib.rename_folder("Worship", "church").is_err());
        lib.save().unwrap();
        let mut back = MediaLibrary::open(&dir);
        assert_eq!(back.folders(), vec!["Worship", "Church"], "an empty folder survives a restart");
        back.delete_folder("Worship");
        assert_eq!(back.get(1).unwrap().folder, "", "its file is kept, just unfiled");
        assert_eq!(back.folders(), vec!["Church"]);
        assert!(back.path_to_show(1).is_some());
    }

    #[test] fn prepare_runs_without_the_library_and_commit_registers_it_once() {
        let d = tmp(); let src = file(&d, "big.png", &[7u8; 3_000_000]); let dir = d.join("lib");
        let done = AtomicU64::new(0);
        let p = prepare(&src, Policy::AlwaysCopy, &dir, Some(&done)).unwrap();
        assert_eq!(done.load(Ordering::Relaxed), planned_work(&src, Policy::AlwaysCopy), "the progress bar reaches exactly 100%");
        assert_eq!(fs::read(&p.path).unwrap().len(), 3_000_000);
        let mut lib = MediaLibrary::open(&dir);
        let Added::New(id) = lib.commit(p.clone(), "Songs") else { panic!() };
        assert!(matches!(lib.commit(p, ""), Added::New(n) if n != id));
        // a second prepare of the same bytes does not copy again but still counts as done
        let done2 = AtomicU64::new(0);
        prepare(&src, Policy::AlwaysCopy, &dir, Some(&done2)).unwrap();
        assert_eq!(done2.load(Ordering::Relaxed), planned_work(&src, Policy::AlwaysCopy));
        assert_eq!(fs::read_dir(dir.join("files")).unwrap().count(), 1, "no half-copied leftovers");
    }
    #[test] fn stored_copy_keeps_the_original_name_and_old_hash_names_are_tidied() {
        let d = tmp(); let a = file(&d, "Everlasting God.mp4", b"video-1"); let b = file(&d, "Everlasting God (copy).mp4", b"video-2");
        let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(id) = lib.add(&a, Policy::AlwaysCopy, "").unwrap() else { panic!() };
        assert_eq!(Path::new(&lib.get(id).unwrap().path).file_name().unwrap().to_string_lossy(), "Everlasting God.mp4");
        // a different file with the same name does not overwrite the first
        let other = d.join("other"); fs::create_dir_all(&other).unwrap(); let c = file(&other, "Everlasting God.mp4", b"video-3");
        let Added::New(id3) = lib.add(&c, Policy::AlwaysCopy, "").unwrap() else { panic!() };
        assert_eq!(Path::new(&lib.get(id3).unwrap().path).file_name().unwrap().to_string_lossy(), "Everlasting God (2).mp4");
        let Added::New(id2) = lib.add(&b, Policy::AlwaysCopy, "").unwrap() else { panic!() };
        assert!(Path::new(&lib.get(id2).unwrap().path).exists());
        // an old library with a hash-named copy
        let dir = d.join("old"); let files = dir.join("files"); fs::create_dir_all(&files).unwrap();
        let fp = fingerprint(&a).unwrap(); let hashed = files.join(format!("{fp}.mp4")); fs::copy(&a, &hashed).unwrap();
        let mut old = MediaLibrary::open(&dir);
        old.entries.push(Entry { id: 1, name: "Everlasting God".into(), path: hashed.to_string_lossy().to_string(), original: a.to_string_lossy().to_string(), mode: Mode::Copy, size: 7, fingerprint: fp, folder: String::new() });
        assert_eq!(old.tidy_stored_names(), 1);
        assert_eq!(old.take_moves().len(), 1);
        assert!(files.join("Everlasting God.mp4").exists() && !hashed.exists());
        assert!(!old.is_missing(1));
        assert_eq!(old.tidy_stored_names(), 0);
    }
    #[test] fn health_check_finds_unused_stored_files_and_never_touches_used_ones() {
        let d = tmp(); let a = file(&d, "keep.png", b"keep-bytes");
        let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(id) = lib.add(&a, Policy::AlwaysCopy, "").unwrap() else { panic!() };
        assert!(lib.orphans().is_empty());
        let stray = lib.dir().join("files").join("stray.png"); fs::write(&stray, b"12345").unwrap();
        fs::write(lib.dir().join("files").join("busy.part"), b"x").unwrap();
        assert_eq!(lib.orphans(), vec![(stray.clone(), 5)]);
        assert_eq!(lib.remove_orphans(), (1, 5));
        assert!(!stray.exists() && !lib.is_missing(id));
    }
    #[test] fn undo_remove_brings_back_the_item_and_its_file() {
        let d = tmp(); let a = file(&d, "Song bg.png", b"bg-bytes");
        let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(id) = lib.add(&a, Policy::AlwaysCopy, "Songs").unwrap() else { panic!() };
        let stored = PathBuf::from(&lib.get(id).unwrap().path);
        lib.remove(id).unwrap();
        assert!(!stored.exists() && lib.get(id).is_none() && lib.can_undo_remove() == 1);
        let back = lib.undo_remove().unwrap();
        assert!(lib.path_to_show(back).is_some());
        assert_eq!(fs::read(lib.path_to_show(back).unwrap()).unwrap(), b"bg-bytes");
        assert_eq!(lib.get(back).unwrap().folder, "Songs");
        assert!(lib.undo_remove().is_none());
    }
    #[test] fn rename_renames_the_stored_file_and_old_paths_still_resolve() {
        let d = tmp(); let a = file(&d, "a.png", b"one"); let b = file(&d, "b.png", b"two");
        let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(id) = lib.add(&a, Policy::AlwaysCopy, "").unwrap() else { panic!() };
        let Added::New(id2) = lib.add(&b, Policy::AlwaysCopy, "").unwrap() else { panic!() };
        let old = lib.get(id).unwrap().path.clone();
        lib.rename(id, "Welcome slide").unwrap();
        let now = lib.get(id).unwrap().path.clone();
        assert!(now.ends_with("Welcome slide.png") && Path::new(&now).exists() && !Path::new(&old).exists());
        assert_eq!(lib.take_moves(), vec![(old.clone(), now.clone())]);
        assert_eq!(lib.resolve_moved(&old), now);
        // another item that takes the same name gets a number, never overwrites
        lib.rename(id2, "Welcome slide").unwrap();
        assert!(lib.get(id2).unwrap().path.ends_with("Welcome slide (2).png"));
        // the moves are remembered after a restart
        let again = MediaLibrary::open(&d.join("lib"));
        assert_eq!(again.resolve_moved(&old), now);
    }
    #[test] fn relink_all_finds_moved_files_by_name_and_bytes_and_duplicates_are_grouped() {
        let d = tmp(); let a = file(&d, "walkin.mp4", b"video-a"); let b = file(&d, "other.mp4", b"video-b");
        let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(ia) = lib.add(&a, Policy::AlwaysLink, "").unwrap() else { panic!() };
        let Added::New(ib) = lib.add(&b, Policy::AlwaysLink, "").unwrap() else { panic!() };
        let usb = d.join("usb").join("sub"); fs::create_dir_all(&usb).unwrap();
        fs::rename(&a, usb.join("walkin.mp4")).unwrap(); fs::remove_file(&b).unwrap();
        assert_eq!(lib.relink_all(&d.join("usb")), (1, 1));
        assert!(!lib.is_missing(ia) && lib.is_missing(ib));
        let dup = lib.duplicate(ia).unwrap();
        assert_eq!(lib.duplicate_groups(), vec![vec![ia, dup]]);
    }
    #[test] fn search_by_type_recent_sort_backup_and_size_warning() {
        let d = tmp(); let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(p) = lib.add(&file(&d, "Cover.png", b"1"), Policy::AlwaysCopy, "").unwrap() else { panic!() };
        let Added::New(v) = lib.add(&file(&d, "Clip.mp4", b"2"), Policy::AlwaysCopy, "").unwrap() else { panic!() };
        assert_eq!(lib.list(None, "mp4"), vec![v]); assert_eq!(lib.list(None, ".png"), vec![p]);
        lib.touch(v); lib.save().unwrap();
        assert_eq!(lib.list_sorted(None, "", SortBy::Recent)[0], v);
        assert_eq!(MediaLibrary::open(&d.join("lib")).list_sorted(None, "", SortBy::Recent)[0], v, "remembered");
        lib.save().unwrap(); lib.save().unwrap();
        assert!(d.join("lib").join("library.tsv.bak").exists());
        assert!(size_warning(10 << 20).is_none() && size_warning(600 << 20).is_some() && size_warning(3 << 30).unwrap().contains("GB"));
    }
    #[test] fn resolve_any_finds_a_file_by_origin_or_name() {
        let d = tmp(); let a = file(&d, "Walk in.mp4", b"w");
        let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(id) = lib.add(&a, Policy::AlwaysCopy, "").unwrap() else { panic!() };
        let stored = lib.get(id).unwrap().path.clone();
        assert_eq!(lib.resolve_any(&a.to_string_lossy()), a.to_string_lossy(), "an existing path is left alone");
        fs::remove_file(&a).unwrap();
        assert_eq!(lib.resolve_any(&a.to_string_lossy()), stored, "found through where it came from");
        assert_eq!(lib.resolve_any("/old/usb/walk IN.mp4"), stored, "found through its name");
        assert_eq!(lib.resolve_any("/old/usb/none.mp4"), "/old/usb/none.mp4");
    }
    #[test] fn last_shown_days_and_unused_list() {
        let d = tmp(); let mut lib = MediaLibrary::open(&d.join("lib"));
        let Added::New(a) = lib.add(&file(&d, "a.png", b"1"), Policy::AlwaysCopy, "").unwrap() else { panic!() };
        let Added::New(b) = lib.add(&file(&d, "b.png", b"2"), Policy::AlwaysCopy, "").unwrap() else { panic!() };
        let Added::New(c) = lib.add(&file(&d, "c.png", b"3"), Policy::AlwaysCopy, "").unwrap() else { panic!() };
        lib.usage.insert(a, 1_000_000); lib.usage.insert(b, 1_000_000 + 100 * 86_400);
        let now = 1_000_000 + 101 * 86_400 + 5;
        assert_eq!(lib.shown_days_ago_at(a, now), Some(101)); assert_eq!(lib.shown_days_ago_at(b, now), Some(1));
        assert_eq!(lib.shown_days_ago_at(c, now), None);
        assert_eq!(lib.unused_ids_at(90, now), vec![a]);
    }
    #[test] fn a_failed_prepare_leaves_nothing_behind() {
        let d = tmp(); let dir = d.join("lib");
        assert!(matches!(prepare(&d.join("missing.png"), Policy::AlwaysCopy, &dir, None), Err(LibError::Io(_))));
        assert!(matches!(prepare(&file(&d, "a.mov", b"x"), Policy::AlwaysCopy, &dir, None), Err(LibError::NotMedia(_))));
        assert!(!dir.join("files").exists() || fs::read_dir(dir.join("files")).unwrap().count() == 0);
    }
    #[test] fn sorting_by_name_and_newest() {
        let d = tmp(); let mut lib = MediaLibrary::open(&d.join("lib"));
        for (n, b) in [("Zebra.png", b"1"), ("apple.png", b"2"), ("Mango.png", b"3")] { lib.add(&file(&d, n, b), Policy::AlwaysCopy, "").unwrap(); }
        assert_eq!(lib.list_sorted(None, "", SortBy::Added), vec![1, 2, 3]);
        assert_eq!(lib.list_sorted(None, "", SortBy::Newest), vec![3, 2, 1]);
        assert_eq!(lib.list_sorted(None, "", SortBy::Name), vec![2, 3, 1], "apple, Mango, Zebra: any case");
    }
}
