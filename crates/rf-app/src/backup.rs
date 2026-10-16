//! Backup of settings and the media library (v0.835.0).
//!
//! RhemaFlow does not log in to anyone's cloud. It writes one dated zip into a folder you choose, and if that
//! folder is one a cloud app keeps in step (iCloud Drive, Dropbox, Google Drive, OneDrive) the backup is in the
//! cloud too. The sign-in ticket is never included. Restoring first makes a safety copy of what is there now.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const PREFIX: &str = "RhemaFlow-backup ";

/// Files that must never leave this Mac in a backup.
fn excluded(rel: &str) -> bool {
    let low = rel.to_lowercase();
    low == "ticket.json" || low.contains("/trash/") || low.starts_with("trash/") || low.ends_with(".pid") || low.ends_with(".lock") || low.contains("tmp") || low == "last-video-problem.txt" || low == "recovery.json"
}

fn is_media(rel: &str) -> bool {
    rel.starts_with("files/") || rel.starts_with("files\\") || rel.contains("/files/")
}

#[derive(Debug, Default, PartialEq)]
pub struct BackupReport {
    pub file: Option<PathBuf>,
    pub files: usize,
    pub bytes: u64,
    pub skipped_media: usize,
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, base, out);
        } else if let Ok(rel) = p.strip_prefix(base) {
            out.push((rel.to_string_lossy().replace('\\', "/"), p));
        }
    }
}

/// Write `<dest>/RhemaFlow-backup <stamp>.zip` from the app's data folder. `include_media` adds the copied pictures,
/// videos and sounds (can be large). `stamp` is like "2026-10-14 0002".
pub fn make(data_dir: &Path, dest: &Path, stamp: &str, include_media: bool) -> Result<BackupReport, String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("Could not use the backup folder ({e})."))?;
    let mut all = Vec::new();
    walk(data_dir, data_dir, &mut all);
    let out_path = dest.join(format!("{PREFIX}{stamp}.zip"));
    let tmp = dest.join(format!("{PREFIX}{stamp}.zip.part"));
    let f = std::fs::File::create(&tmp).map_err(|e| format!("Could not write the backup ({e})."))?;
    let mut zw = zip::ZipWriter::new(f);
    let mut rep = BackupReport::default();
    for (rel, path) in all {
        if excluded(&rel) {
            continue;
        }
        let media = is_media(&rel);
        if media && !include_media {
            rep.skipped_media += 1;
            continue;
        }
        let method = if media { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated };
        let opts = zip::write::FileOptions::default().compression_method(method).large_file(true);
        let mut src = match std::fs::File::open(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        zw.start_file(rel, opts).map_err(|e| format!("Backup failed ({e})."))?;
        let n = std::io::copy(&mut src, &mut zw).map_err(|e| format!("Backup failed ({e})."))?;
        rep.files += 1;
        rep.bytes += n;
    }
    let mut f = zw.finish().map_err(|e| format!("Backup failed ({e})."))?;
    f.flush().map_err(|e| e.to_string())?;
    drop(f);
    std::fs::rename(&tmp, &out_path).map_err(|e| format!("Backup failed ({e})."))?;
    rep.file = Some(out_path);
    Ok(rep)
}

/// Keep only the newest `keep` backups in `dest`. Returns how many were removed.
pub fn prune(dest: &Path, keep: usize) -> usize {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dest)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_name().map_or(false, |n| n.to_string_lossy().starts_with(PREFIX) && n.to_string_lossy().ends_with(".zip"))).collect())
        .unwrap_or_default();
    v.sort(); // stamps sort by time
    let mut removed = 0;
    while v.len() > keep {
        if std::fs::remove_file(v.remove(0)).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub fn latest(dest: &Path) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dest).ok()?.flatten().map(|e| e.path()).filter(|p| p.file_name().map_or(false, |n| n.to_string_lossy().starts_with(PREFIX) && n.to_string_lossy().ends_with(".zip"))).collect();
    v.sort();
    v.pop()
}

/// Put a backup back into the data folder. A safety zip of the current data is written to `safety_dir` first.
pub fn restore(zip_path: &Path, data_dir: &Path, safety_dir: &Path, stamp: &str) -> Result<usize, String> {
    let f = std::fs::File::open(zip_path).map_err(|e| format!("Could not open the backup ({e})."))?;
    let mut za = zip::ZipArchive::new(f).map_err(|_| "That file is not a RhemaFlow backup.".to_string())?;
    // check before touching anything
    for i in 0..za.len() {
        let e = za.by_index(i).map_err(|_| "The backup is damaged.".to_string())?;
        if e.enclosed_name().is_none() {
            return Err("The backup contains an unsafe file name, so it was not restored.".into());
        }
    }
    if data_dir.exists() {
        make(data_dir, safety_dir, &format!("before-restore {stamp}"), false)?;
    }
    let mut n = 0;
    for i in 0..za.len() {
        let mut e = za.by_index(i).map_err(|_| "The backup is damaged.".to_string())?;
        let Some(rel) = e.enclosed_name().map(|p| p.to_path_buf()) else { continue };
        if e.is_dir() || excluded(&rel.to_string_lossy()) {
            continue;
        }
        let target = data_dir.join(rel);
        if let Some(d) = target.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::new();
        e.read_to_end(&mut buf).map_err(|_| "The backup is damaged.".to_string())?;
        crate::fsutil::write_atomic(&target, &buf).map_err(|e| format!("Could not restore a file ({e})."))?;
        n += 1;
    }
    Ok(n)
}

/// Folders a cloud app is likely to keep in step. Only ones that exist are returned.
pub fn cloud_folders(home: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = Vec::new();
    let mut add = |name: &str, p: PathBuf| {
        if p.is_dir() {
            v.push((name.to_string(), p));
        }
    };
    add("iCloud Drive", home.join("Library/Mobile Documents/com~apple~CloudDocs"));
    add("Dropbox", home.join("Dropbox"));
    add("OneDrive", home.join("OneDrive"));
    add("Google Drive", home.join("Google Drive"));
    if let Ok(rd) = std::fs::read_dir(home.join("Library/CloudStorage")) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            let label = if n.starts_with("GoogleDrive") { "Google Drive" } else if n.starts_with("OneDrive") { "OneDrive" } else if n.starts_with("Dropbox") { "Dropbox" } else { continue };
            add(label, e.path());
        }
    }
    v
}

/// USB sticks and other drives plugged in (the entries of /Volumes that are real folders; the start-up disk is a link and is left out).
pub fn usb_volumes(root: &Path) -> Vec<(String, PathBuf)> {
    let mut v = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            let is_link = e.file_type().map_or(true, |t| t.is_symlink());
            if !n.starts_with('.') && !is_link && e.path().is_dir() {
                v.push((n, e.path()));
            }
        }
    }
    v.sort();
    v
}

/// Is an automatic daily backup due? `last_unix` is when the last one finished (0 = never).
pub fn due(last_unix: i64, now_unix: i64) -> bool {
    last_unix == 0 || now_unix - last_unix >= 20 * 3600
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rf-backup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn data() -> PathBuf {
        let d = tmp("data");
        std::fs::write(d.join("settings.json"), "{\"church\":\"X\"}").unwrap();
        std::fs::write(d.join("songs.json"), "[1,2,3]").unwrap();
        std::fs::write(d.join("ticket.json"), "SECRET-TOKEN").unwrap();
        std::fs::create_dir_all(d.join("files")).unwrap();
        std::fs::write(d.join("files").join("a.png"), vec![7u8; 5000]).unwrap();
        d
    }

    #[test]
    fn the_sign_in_ticket_is_never_in_a_backup_and_media_is_optional() {
        let (d, dest) = (data(), tmp("dest"));
        let r = make(&d, &dest, "2026-10-14 0002", false).unwrap();
        assert_eq!((r.files, r.skipped_media), (2, 1));
        let z = std::fs::read(r.file.as_ref().unwrap()).unwrap();
        assert!(!z.windows(12).any(|w| w == b"SECRET-TOKEN") && !String::from_utf8_lossy(&z).contains("ticket.json"));
        let r2 = make(&d, &dest, "2026-10-14 0003", true).unwrap();
        assert_eq!(r2.files, 3);
    }

    #[test]
    fn restoring_puts_files_back_and_keeps_a_safety_copy_of_what_was_there() {
        let (d, dest) = (data(), tmp("dest2"));
        let r = make(&d, &dest, "2026-10-14 0002", true).unwrap();
        std::fs::write(d.join("settings.json"), "CHANGED").unwrap();
        std::fs::remove_file(d.join("songs.json")).unwrap();
        let n = restore(r.file.as_ref().unwrap(), &d, &dest, "2026-10-14 0100").unwrap();
        assert_eq!(n, 3);
        assert_eq!(std::fs::read_to_string(d.join("settings.json")).unwrap(), "{\"church\":\"X\"}");
        assert!(d.join("songs.json").exists() && d.join("files/a.png").exists());
        assert_eq!(std::fs::read_to_string(d.join("ticket.json")).unwrap(), "SECRET-TOKEN", "the ticket is left alone");
        assert!(dest.join(format!("{PREFIX}before-restore 2026-10-14 0100.zip")).exists());
    }

    #[test]
    fn a_zip_with_an_escaping_path_is_refused_untouched() {
        let (d, dest) = (data(), tmp("dest3"));
        let evil = dest.join("evil.zip");
        {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&evil).unwrap());
            zw.start_file("../../escaped.txt", zip::write::FileOptions::default()).unwrap();
            zw.write_all(b"x").unwrap();
            zw.finish().unwrap();
        }
        assert!(restore(&evil, &d, &dest, "s").unwrap_err().contains("unsafe"));
        assert!(!d.parent().unwrap().parent().unwrap().join("escaped.txt").exists());
        assert!(restore(&dest.join("nope.zip"), &d, &dest, "s").is_err());
        std::fs::write(dest.join("junk.zip"), "not a zip").unwrap();
        assert!(restore(&dest.join("junk.zip"), &d, &dest, "s").unwrap_err().contains("not a RhemaFlow backup"));
    }

    #[test]
    fn old_backups_are_pruned_oldest_first_and_the_newest_is_found() {
        let (d, dest) = (data(), tmp("dest4"));
        for i in 1..=5 {
            make(&d, &dest, &format!("2026-10-1{i} 0900"), false).unwrap();
        }
        assert_eq!(prune(&dest, 3), 2);
        let left = latest(&dest).unwrap();
        assert!(left.to_string_lossy().contains("2026-10-15"));
        assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 3);
    }

    #[test]
    fn cloud_folders_are_found_only_if_they_exist_and_daily_timing_works() {
        let h = tmp("home");
        assert!(cloud_folders(&h).is_empty());
        std::fs::create_dir_all(h.join("Dropbox")).unwrap();
        std::fs::create_dir_all(h.join("Library/CloudStorage/GoogleDrive-me@x.org")).unwrap();
        let names: Vec<String> = cloud_folders(&h).into_iter().map(|c| c.0).collect();
        assert_eq!(names, ["Dropbox", "Google Drive"]);
        assert!(due(0, 5) && !due(1000, 1000 + 3600) && due(1000, 1000 + 21 * 3600));
    }

    #[test]
    fn drives_are_listed_but_the_startup_disk_link_and_hidden_items_are_not() {
        let d = std::env::temp_dir().join(format!("rf-vol-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("CHURCH USB")).unwrap();
        std::fs::create_dir_all(d.join(".hidden")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/", d.join("Macintosh HD")).unwrap();
        let v = usb_volumes(&d);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, "CHURCH USB");
        assert!(excluded("media-library/trash/old.png") && !excluded("media-library/files/a.png"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
