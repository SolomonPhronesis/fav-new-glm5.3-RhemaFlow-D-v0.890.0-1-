//! Crash-safe persistence.
//!
//! * **Atomic write** — temp file + fsync + rename (+ directory fsync), so a
//!   power cut or `kill -9` leaves either the old file or the new one, never half.
//! * **Rolling backups** — `name.json.bak.1 … .bak.N`, newest first.
//! * **Recovery** — a corrupt main file falls back to the newest good backup.
//! * **Versioning** — files from a *newer* app are refused, never silently
//!   downgraded; older files pass through an explicit migration chain.

use crate::model::{Service, SCHEMA_VERSION};
use serde_json::Value;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const BACKUP_COUNT: usize = 5;

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    /// File is valid JSON but from a newer RhemaFlow than this one.
    TooNew { file: u32, app: u32 },
    Corrupt(String),
    /// Main file and every backup were unreadable.
    Unrecoverable(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "I/O error: {e}"),
            StoreError::TooNew { file, app } => {
                write!(f, "file is schema v{file} but this app understands up to v{app}; update RhemaFlow")
            }
            StoreError::Corrupt(m) => write!(f, "corrupt service file: {m}"),
            StoreError::Unrecoverable(m) => write!(f, "no readable copy found: {m}"),
        }
    }
}
impl std::error::Error for StoreError {}
impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

pub struct LoadOutcome {
    pub service: Service,
    /// `Some(path)` when the main file was bad and this backup was used.
    pub recovered_from: Option<PathBuf>,
}

fn backup_path(path: &Path, n: usize) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(format!(".bak.{n}"));
    PathBuf::from(s)
}

/// Write bytes so that `path` is always a complete file.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp-{}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    // Persist the rename itself (unix). Ignore failure on platforms that can't.
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// Save with backup rotation. The previous file is shifted into `.bak.1`
/// only when its content differs, so autosaving an unchanged service never
/// pushes good backups out of the window.
pub fn save(path: &Path, service: &Service) -> Result<(), StoreError> {
    let json = serde_json::to_vec_pretty(service).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    if let Ok(existing) = fs::read(path) {
        if existing == json {
            return Ok(());
        }
        // shift .bak.(N-1) → .bak.N … .bak.1 → .bak.2, then copy current → .bak.1
        for n in (1..BACKUP_COUNT).rev() {
            let from = backup_path(path, n);
            if from.exists() {
                let _ = fs::rename(&from, backup_path(path, n + 1));
            }
        }
        fs::copy(path, backup_path(path, 1))?;
    }
    atomic_write(path, &json)?;
    Ok(())
}

type Migration = (u32, fn(&mut Value));

/// Migrations from version `n` to `n + 1`. Empty while SCHEMA_VERSION == 1.
const MIGRATIONS: &[Migration] = &[];

/// Upgrade raw JSON to the current schema, running each step in order.
pub fn migrate_with(mut v: Value, current: u32, steps: &[Migration]) -> Result<Value, StoreError> {
    let mut ver = v
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| StoreError::Corrupt("missing schema_version".into()))? as u32;
    if ver > current {
        return Err(StoreError::TooNew { file: ver, app: current });
    }
    while ver < current {
        let step = steps
            .iter()
            .find(|(from, _)| *from == ver)
            .ok_or_else(|| StoreError::Corrupt(format!("no migration from v{ver}")))?;
        (step.1)(&mut v);
        ver += 1;
        v["schema_version"] = Value::from(ver);
    }
    Ok(v)
}

fn read_one(path: &Path) -> Result<Service, StoreError> {
    let bytes = fs::read(path)?;
    let v: Value = serde_json::from_slice(&bytes).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    let v = migrate_with(v, SCHEMA_VERSION, MIGRATIONS)?;
    serde_json::from_value(v).map_err(|e| StoreError::Corrupt(e.to_string()))
}

/// Load the service, falling back through the backups if the main file is bad.
/// A `TooNew` file is **not** papered over with an older backup: that would
/// silently discard newer work, so the error is surfaced.
pub fn load(path: &Path) -> Result<LoadOutcome, StoreError> {
    match read_one(path) {
        Ok(service) => return Ok(LoadOutcome { service, recovered_from: None }),
        Err(e @ StoreError::TooNew { .. }) => return Err(e),
        Err(first) => {
            for n in 1..=BACKUP_COUNT {
                let b = backup_path(path, n);
                if !b.exists() {
                    continue;
                }
                if let Ok(service) = read_one(&b) {
                    return Ok(LoadOutcome { service, recovered_from: Some(b) });
                }
            }
            Err(StoreError::Unrecoverable(first.to_string()))
        }
    }
}

/// Debounced autosave policy (pure: the UI passes in a millisecond clock).
/// Saves shortly after edits stop, but never waits longer than `max_wait_ms`
/// during continuous editing.
pub struct Autosave {
    debounce_ms: u64,
    max_wait_ms: u64,
    first_dirty: Option<u64>,
    last_change: u64,
}

impl Autosave {
    pub fn new(debounce_ms: u64, max_wait_ms: u64) -> Self {
        Autosave { debounce_ms, max_wait_ms, first_dirty: None, last_change: 0 }
    }
    pub fn mark_dirty(&mut self, now_ms: u64) {
        self.first_dirty.get_or_insert(now_ms);
        self.last_change = now_ms;
    }
    pub fn should_save(&self, now_ms: u64) -> bool {
        match self.first_dirty {
            None => false,
            Some(first) => {
                now_ms.saturating_sub(self.last_change) >= self.debounce_ms
                    || now_ms.saturating_sub(first) >= self.max_wait_ms
            }
        }
    }
    pub fn saved(&mut self) {
        self.first_dirty = None;
    }
}
