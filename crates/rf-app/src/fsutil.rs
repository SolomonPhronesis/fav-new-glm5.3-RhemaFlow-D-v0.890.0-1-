//! Small file helpers shared by the v0.835.0 features.

use std::io::Write;
use std::path::Path;

/// Write to a temporary file next to `path`, then rename: a crash or power cut leaves the old file or the new one, never half of each.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// `~/.rhemaflow` (made if needed).
pub fn data_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    let d = std::path::PathBuf::from(home).join(".rhemaflow");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

/// Today's date as `2026-10-18` (UTC; good enough to tell the days apart in a file name).
pub fn today_stamp() -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| (d.as_secs() / 86400) as i64).unwrap_or(0);
    // civil-from-days (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_date_stamp_is_a_real_date() {
        let s = super::today_stamp();
        assert_eq!(s.len(), 10);
        assert!(s.as_str() >= "2026-10-14", "{s}");
    }

    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp_file() {
        let d = std::env::temp_dir().join(format!("rf-fsutil-{}", std::process::id()));
        let p = d.join("a").join("x.json");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "two");
        let leftovers = std::fs::read_dir(p.parent().unwrap()).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains("tmp")).count();
        assert_eq!(leftovers, 0);
    }
}
