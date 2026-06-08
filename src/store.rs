//! Persistent on-disk store (spec 005) — a redb key-value file in the app data
//! dir that caches the index and records frecency. redb is pure-Rust (no C
//! toolchain, unlike bundled SQLite) and we need no SQL: nucleo still matches in
//! memory, so this is just a typed cache + a frecency ledger.
//!
//! Two tables key files by absolute path; values are small serde_json blobs so
//! arbitrary paths round-trip cleanly. The store is cheaply `Clone` (an
//! `Arc<Database>`) and redb serializes its own write transactions, so the
//! watcher / reconcile / pick-recording threads share one handle without an
//! external mutex. A corrupt or version-mismatched file is rebuilt rather than
//! crashing (FR7), with an in-memory fallback so atref always launches.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::index::Entry;

/// Bump when the on-disk layout changes; a mismatch wipes + rebuilds (FR7).
const SCHEMA_VERSION: u64 = 1;

const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
const ENTRIES: TableDefinition<&str, &str> = TableDefinition::new("entries");
const FRECENCY: TableDefinition<&str, &str> = TableDefinition::new("frecency");
const SCHEMA_KEY: &str = "schema_version";

/// Cached index row (value side of `ENTRIES`, keyed by absolute path). `mtime` /
/// `size` are kept for future incremental change detection (FR1).
#[derive(Serialize, Deserialize)]
struct StoredEntry {
    root: String,
    rel: String,
    root_rank: usize,
    mtime: u64,
    size: u64,
}

/// Frecency ledger row (value side of `FRECENCY`, keyed by absolute path).
#[derive(Serialize, Deserialize, Clone, Copy)]
struct StoredFrecency {
    count: u32,
    last_picked_unix: u64,
}

/// A handle to the persistent store. Clone freely across threads.
#[derive(Clone)]
pub struct Store {
    db: Arc<Database>,
}

impl Store {
    /// Open the store at `path`, creating it if missing. A corrupt / unreadable
    /// or version-mismatched file is rebuilt; if even that fails, fall back to
    /// an in-memory store so atref still launches (FR7 / AC4).
    pub fn open_or_reset(path: &Path) -> Store {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(store) = Self::open_validated(path) {
            return store;
        }
        // Corrupt / incompatible on disk → start fresh.
        let _ = std::fs::remove_file(path);
        Self::open_validated(path).unwrap_or_else(|_| Self::in_memory())
    }

    fn open_validated(path: &Path) -> Result<Store, Box<dyn std::error::Error>> {
        let db = Database::create(path)?;
        ensure_schema(&db)?;
        Ok(Store { db: Arc::new(db) })
    }

    fn in_memory() -> Store {
        let db = Database::builder()
            .create_with_backend(redb::backends::InMemoryBackend::new())
            .expect("in-memory redb backend");
        let _ = ensure_schema(&db);
        Store { db: Arc::new(db) }
    }

    /// Load the cached index. Empty on first run or any read error (the
    /// background reconcile then repopulates) — never panics (AC1/AC3).
    pub fn load_entries(&self) -> Vec<Entry> {
        self.try_load_entries().unwrap_or_default()
    }

    fn try_load_entries(&self) -> Result<Vec<Entry>, Box<dyn std::error::Error>> {
        let read = self.db.begin_read()?;
        let table = match read.open_table(ENTRIES) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::new();
        for row in table.iter()? {
            let (k, v) = row?;
            if let Ok(s) = serde_json::from_str::<StoredEntry>(v.value()) {
                out.push(Entry {
                    abs: PathBuf::from(k.value()),
                    root: PathBuf::from(s.root),
                    rel: s.rel,
                    root_rank: s.root_rank,
                });
            }
        }
        Ok(out)
    }

    /// Load the frecency ledger as `path -> (pick_count, last_picked)`.
    pub fn load_frecency(&self) -> HashMap<PathBuf, (u32, SystemTime)> {
        self.try_load_frecency().unwrap_or_default()
    }

    fn try_load_frecency(
        &self,
    ) -> Result<HashMap<PathBuf, (u32, SystemTime)>, Box<dyn std::error::Error>> {
        let read = self.db.begin_read()?;
        let table = match read.open_table(FRECENCY) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(HashMap::new()),
            Err(e) => return Err(e.into()),
        };
        let mut map = HashMap::new();
        for row in table.iter()? {
            let (k, v) = row?;
            if let Ok(s) = serde_json::from_str::<StoredFrecency>(v.value()) {
                let when = UNIX_EPOCH + Duration::from_secs(s.last_picked_unix);
                map.insert(PathBuf::from(k.value()), (s.count, when));
            }
        }
        Ok(map)
    }

    /// Replace the cached index with `entries` and prune frecency rows for paths
    /// that no longer exist (NFR4). Full-replace is simplest + correct at
    /// personal scale; best-effort (a transient write error is swallowed).
    pub fn persist(&self, entries: &[Entry]) {
        let _ = self.try_persist(entries);
    }

    fn try_persist(&self, entries: &[Entry]) -> Result<(), Box<dyn std::error::Error>> {
        let present: HashSet<String> = entries
            .iter()
            .map(|e| e.abs.to_string_lossy().into_owned())
            .collect();
        let write = self.db.begin_write()?;
        {
            // Rewrite ENTRIES wholesale (drop deleted, add new — AC2).
            let _ = write.delete_table(ENTRIES);
            let mut table = write.open_table(ENTRIES)?;
            for e in entries {
                let (mtime, size) = metadata_mtime_size(&e.abs);
                let stored = StoredEntry {
                    root: e.root.to_string_lossy().into_owned(),
                    rel: e.rel.clone(),
                    root_rank: e.root_rank,
                    mtime,
                    size,
                };
                let json = serde_json::to_string(&stored).unwrap_or_default();
                let key = e.abs.to_string_lossy();
                table.insert(key.as_ref(), json.as_str())?;
            }
            // Prune frecency rows whose path is no longer indexed (NFR4).
            let mut frec = write.open_table(FRECENCY)?;
            let stale: Vec<String> = frec
                .iter()?
                .filter_map(|row| row.ok())
                .map(|(k, _)| k.value().to_string())
                .filter(|k| !present.contains(k))
                .collect();
            for k in stale {
                frec.remove(k.as_str())?;
            }
        }
        write.commit()?;
        Ok(())
    }

    /// Record a pick for `abs`: increment its count and set last-picked to now
    /// (FR4/AC5). Best-effort, intended to run off the UI thread.
    pub fn record_pick(&self, abs: &Path) {
        let _ = self.try_record_pick(abs);
    }

    fn try_record_pick(&self, abs: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let key = abs.to_string_lossy().into_owned();
        let write = self.db.begin_write()?;
        {
            let mut frec = write.open_table(FRECENCY)?;
            let prev = frec
                .get(key.as_str())?
                .and_then(|g| serde_json::from_str::<StoredFrecency>(g.value()).ok());
            let count = prev.map(|p| p.count).unwrap_or(0).saturating_add(1);
            let updated = StoredFrecency {
                count,
                last_picked_unix: now_unix(),
            };
            let json = serde_json::to_string(&updated).unwrap_or_default();
            frec.insert(key.as_str(), json.as_str())?;
        }
        write.commit()?;
        Ok(())
    }
}

/// Read the stored schema version; set it on first run, wipe + reset on a
/// mismatch (so a layout change can't surface garbage rows — FR7).
fn ensure_schema(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    let current: Option<u64> = {
        let read = db.begin_read()?;
        match read.open_table(META) {
            Ok(t) => t.get(SCHEMA_KEY)?.map(|g| g.value()),
            Err(redb::TableError::TableDoesNotExist(_)) => None,
            Err(e) => return Err(e.into()),
        }
    };
    match current {
        Some(v) if v == SCHEMA_VERSION => Ok(()),
        Some(_) => {
            wipe_all(db)?;
            set_schema(db)
        }
        None => set_schema(db),
    }
}

fn set_schema(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    let write = db.begin_write()?;
    {
        let mut t = write.open_table(META)?;
        t.insert(SCHEMA_KEY, SCHEMA_VERSION)?;
    }
    write.commit()?;
    Ok(())
}

fn wipe_all(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    let write = db.begin_write()?;
    let _ = write.delete_table(ENTRIES);
    let _ = write.delete_table(FRECENCY);
    write.commit()?;
    Ok(())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Last-modified (unix secs) + size of `abs`, best-effort (0 if unavailable).
fn metadata_mtime_size(abs: &Path) -> (u64, u64) {
    match std::fs::metadata(abs) {
        Ok(md) => {
            let mtime = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (mtime, md.len())
        }
        Err(_) => (0, 0),
    }
}
