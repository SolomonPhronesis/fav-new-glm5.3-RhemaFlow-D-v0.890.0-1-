//! Where a service is saved, and in which formats (v0.811.0).
//!
//! Pure, testable pieces: file-name safety, the three transcript formats
//! (plain text, SubRip captions, Markdown sermon notes), the saved-folder
//! summary, and the few OS helpers (free disk space, open a folder, compress
//! the recording to a small .m4a on macOS).
//!
//! Why these three formats (the "top 3" decision):
//! 1. `.txt`  - opens anywhere, searchable, pasteable into bulletins and e-mails.
//! 2. `.srt`  - SubRip captions: drop it next to the recording / on YouTube and
//!    the words appear in time with the voice. (The user publishes to YouTube.)
//! 3. `.md`   - sermon notes: title, date, the scripture index with timestamps,
//!    then the full transcript. Reads well as-is and converts to Word/PDF.
//! `events.jsonl` (machine-readable) is still written live as before.

use std::path::{Path, PathBuf};

/// Which transcript files to write at the end of a service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Formats {
    pub txt: bool,
    pub srt: bool,
    pub md: bool,
}

impl Default for Formats {
    fn default() -> Self {
        Formats { txt: true, srt: true, md: true }
    }
}

/// One finished transcript line.
#[derive(Clone, Debug)]
pub struct Line {
    pub ms: u64,
    pub text: String,
}

/// One scripture that went on stage.
#[derive(Clone, Debug)]
pub struct Cite {
    pub ms: u64,
    pub label: String,
    pub source: String,
    /// the verse text that was shown
    pub text: String,
}

pub fn hms(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

fn srt_time(ms: u64) -> String {
    format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, (ms / 60_000) % 60, (ms / 1000) % 60, ms % 1000)
}

/// SubRip captions. A line is shown from when it was heard until just before the
/// next one, but never longer than 8 s and never shorter than 1.5 s.
pub fn to_srt(lines: &[Line], total_ms: u64) -> String {
    let mut s = String::new();
    let mut n = 0;
    for (i, l) in lines.iter().enumerate() {
        let text = l.text.trim();
        if text.is_empty() {
            continue;
        }
        n += 1;
        let words = text.split_whitespace().count() as u64;
        let want = (words * 400).clamp(1500, 8000);
        let next = lines.get(i + 1).map(|x| x.ms).unwrap_or(total_ms.max(l.ms + want));
        let end = (l.ms + want).min(next.saturating_sub(80)).max(l.ms + 1000);
        s.push_str(&format!("{n}\n{} --> {}\n{text}\n\n", srt_time(l.ms), srt_time(end)));
    }
    s
}

/// A folder name that is safe on macOS/Windows/Linux and never empty.
pub fn safe_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '\'' { c } else { '-' })
        .collect();
    let t = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = t.trim_matches(|c| c == '-' || c == ' ').to_string();
    t.chars().take(60).collect()
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// The three built-in places, plus a custom folder.
pub fn downloads_root() -> PathBuf {
    home().join("Downloads").join("RhemaFlow")
}
pub fn documents_root() -> PathBuf {
    home().join("Documents").join("RhemaFlow")
}
pub fn legacy_root() -> PathBuf {
    home().join("RhemaFlow-Services")
}

/// True when we can create `dir` and write a file in it.
pub fn is_writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".rf-write-test");
    let ok = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// The folder services are saved under. `chosen` empty = Downloads/RhemaFlow.
/// A chosen folder that cannot be written falls back to Downloads, then to the
/// old ~/RhemaFlow-Services, so a service is never lost to a bad setting.
/// Returns (folder, note-if-a-fallback-was-used).
pub fn resolve_root(chosen: &str) -> (PathBuf, Option<String>) {
    let want = if chosen.trim().is_empty() { downloads_root() } else { PathBuf::from(chosen.trim()) };
    if is_writable(&want) {
        return (want, None);
    }
    for fb in [downloads_root(), legacy_root()] {
        if fb != want && is_writable(&fb) {
            return (fb.clone(), Some(format!("cannot write to {} \u{2014} saving to {} instead", want.display(), fb.display())));
        }
    }
    (want.clone(), Some(format!("cannot write to {}", want.display())))
}

/// Free space on the disk holding `dir`, in bytes (None if it cannot be read).
pub fn free_bytes(dir: &Path) -> Option<u64> {
    let probe = if dir.exists() { dir.to_path_buf() } else { dir.parent()?.to_path_buf() };
    let out = std::process::Command::new("df").arg("-k").arg(&probe).output().ok()?;
    parse_df_available_kb(&String::from_utf8_lossy(&out.stdout)).map(|kb| kb * 1024)
}

/// Pull the "Available" column (KiB) out of `df -k <path>` output.
pub fn parse_df_available_kb(df: &str) -> Option<u64> {
    let line = df.lines().nth(1)?;
    let cols: Vec<&str> = line.split_whitespace().collect();
    cols.get(3)?.parse().ok()
}

/// Rough size of an hour of the 16 kHz mono WAV (115 MB), used for the low-disk warning.
pub const WAV_BYTES_PER_HOUR: u64 = 115_000_000;

pub fn human_size(b: u64) -> String {
    if b >= 1_000_000_000 {
        format!("{:.1} GB", b as f64 / 1e9)
    } else if b >= 1_000_000 {
        format!("{:.1} MB", b as f64 / 1e6)
    } else if b >= 1000 {
        format!("{:.0} KB", b as f64 / 1e3)
    } else {
        format!("{b} B")
    }
}

/// Files in a service folder whose name starts with `prefix` (this service's files; another
/// service the same day sits in the same folder), with sizes. Empty prefix = everything.
pub fn list_files(dir: &Path, prefix: &str) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().chars().next() != Some('.'))
                .filter(|e| prefix.is_empty() || e.file_name().to_string_lossy().starts_with(prefix))
                .filter_map(|e| Some((e.file_name().to_string_lossy().to_string(), e.metadata().ok()?.len())))
                .collect()
        })
        .unwrap_or_default();
    let rank = |n: &str| match n.rsplit('.').next().unwrap_or("") {
        "wav" => 0,
        "m4a" => 1,
        "txt" => 2,
        "srt" => 3,
        "md" => 4,
        _ => 5,
    };
    v.sort_by(|a, b| rank(&a.0).cmp(&rank(&b.0)).then(a.0.cmp(&b.0)));
    v
}

/// Open a folder in Finder (macOS) / the file manager (Linux).
pub fn open_folder(dir: &Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(dir).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
}

/// Ask macOS for a folder with its own chooser dialog (no extra dependency).
/// Blocks until the dialog closes, so call it from a worker thread.
pub fn choose_folder_dialog() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("osascript")
            .args(["-e", "POSIX path of (choose folder with prompt \"Where should RhemaFlow save services?\")"])
            .output()
            .ok()?;
        let p = String::from_utf8_lossy(&out.stdout).trim().trim_end_matches('/').to_string();
        return if out.status.success() && !p.is_empty() { Some(p) } else { None };
    }
    #[allow(unreachable_code)]
    None
}

/// Make a small AAC `.m4a` copy of `audio.wav` (about a tenth of the size) with
/// macOS's built-in `afconvert`. Returns the new file on success. Blocking.
pub fn compress_to_m4a(wav: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let out = wav.with_extension("m4a");
        let ok = std::process::Command::new("afconvert")
            .args(["-f", "m4af", "-d", "aac", "-b", "64000"])
            .arg(wav)
            .arg(&out)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        return if ok && out.exists() { Some(out) } else { None };
    }
    #[allow(unreachable_code)]
    {
        let _ = wav;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines() -> Vec<Line> {
        vec![
            Line { ms: 63_000, text: "Revelations chapter 1 verse 6.".into() },
            Line { ms: 66_000, text: "Read.".into() },
            Line { ms: 75_000, text: "And hath made us kings and priests unto God and his Father.".into() },
        ]
    }

    #[test]
    fn srt_is_valid_numbered_and_never_overlaps() {
        let s = to_srt(&lines(), 90_000);
        let blocks: Vec<&str> = s.trim().split("\n\n").collect();
        assert_eq!(blocks.len(), 3);
        assert!(blocks[0].starts_with("1\n00:01:03,000 --> 00:01:0"), "{}", blocks[0]);
        let mut prev_end = 0u64;
        for b in &blocks {
            let t = b.lines().nth(1).unwrap();
            let (a, e) = t.split_once(" --> ").unwrap();
            let p = |x: &str| {
                let (hms, ms) = x.split_once(',').unwrap();
                let v: Vec<u64> = hms.split(':').map(|n| n.parse().unwrap()).collect();
                (v[0] * 3600 + v[1] * 60 + v[2]) * 1000 + ms.parse::<u64>().unwrap()
            };
            assert!(p(e) > p(a), "{t}");
            assert!(p(a) >= prev_end, "{t} overlaps previous");
            prev_end = p(e);
        }
    }

    #[test]
    fn srt_skips_empty_lines_and_keeps_numbering_dense() {
        let l = vec![Line { ms: 1000, text: "  ".into() }, Line { ms: 5000, text: "Hello there".into() }];
        let s = to_srt(&l, 9000);
        assert!(s.starts_with("1\n"), "{s}");
        assert!(!s.contains("\n2\n"));
    }

    #[test]
    fn folder_names_are_safe() {
        assert_eq!(safe_name("Faith / that: moves?"), "Faith - that- moves");
        assert_eq!(safe_name("///"), "");
        assert!(safe_name(&"a".repeat(200)).len() <= 60);
    }

    #[test]
    fn df_output_is_parsed() {
        let df = "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/disk3s1 488245288 230000000 123456789 66% /\n";
        assert_eq!(parse_df_available_kb(df), Some(123_456_789));
        assert_eq!(parse_df_available_kb("garbage"), None);
    }

    #[test]
    fn human_sizes() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2_600), "3 KB");
        assert_eq!(human_size(115_000_000), "115.0 MB");
        assert_eq!(human_size(3_200_000_000), "3.2 GB");
    }

    #[test]
    fn a_bad_chosen_folder_falls_back_instead_of_losing_the_service() {
        let (root, note) = resolve_root("/proc/definitely/not/writable");
        assert!(note.is_some());
        assert!(root.to_string_lossy().contains("RhemaFlow"), "{root:?}");
    }

    #[test]
    fn listing_shows_only_this_services_files() {
        let dir = std::env::temp_dir().join(format!("rf-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("A - recording.wav"), b"1").unwrap();
        std::fs::write(dir.join("A - transcript.txt"), b"22").unwrap();
        std::fs::write(dir.join("B - recording.wav"), b"333").unwrap();
        let a = list_files(&dir, "A -");
        assert_eq!(a.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), vec!["A - recording.wav", "A - transcript.txt"]);
        assert_eq!(list_files(&dir, "").len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
