//! The app's own SQLite files (plan M3 decision 3). Never anything of the game's.
//!
//! - `library.sqlite` (app data dir): user data. Today the profile registry; later tags,
//!   notes and mod sources. Losing it loses user choices.
//! - `cache.sqlite` (app cache dir): disposable. Per-mod scan records and thumbnails, keyed
//!   by the mod's path, size and mtime (and `scan::SCAN_VERSION` for records). Deleting it is
//!   always safe; the next scan rebuilds it.
//!
//! Each file has a `schema_version` and a numbered list of migrations applied in order.
//!
//! A scan writes hundreds of records, so the cache has batch methods
//! ([`Store::cached_mods_in`], [`Store::put_mods`]) that use one query or one transaction
//! each. One commit per row would cost one fsync per mod.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

use crate::container::IconLocation;
use crate::discovery::ModsOverride;
use crate::model::ModEntry;
use crate::scan::SCAN_VERSION;

#[cfg(test)]
mod tests;

/// Migrations of `library.sqlite`, applied in order. Index + 1 is the schema version a
/// migration brings the file to. Never edit one that has shipped; append a new one.
const LIBRARY_MIGRATIONS: &[&str] = &[
    "
    CREATE TABLE profiles (
        id       INTEGER PRIMARY KEY,
        name     TEXT NOT NULL,
        path     TEXT NOT NULL,
        path_key TEXT NOT NULL UNIQUE,
        added_at INTEGER NOT NULL
    );
",
    "
    CREATE TABLE ops (
        id          INTEGER PRIMARY KEY,
        at          INTEGER NOT NULL,
        kind        TEXT NOT NULL,
        from_active INTEGER NOT NULL,
        from_dir    TEXT,
        to_active   INTEGER NOT NULL,
        to_dir      TEXT,
        backup_file TEXT,
        result      TEXT NOT NULL
    );
",
    // File operations (plan M5) get their own journal: `ops` rows describe override states and
    // `latest_ok_op` feeds Undo, which a file operation must never become.
    "
    CREATE TABLE file_ops (
        id      INTEGER PRIMARY KEY,
        at      INTEGER NOT NULL,
        kind    TEXT NOT NULL,
        plan_id TEXT,
        source  TEXT NOT NULL,
        target  TEXT,
        result  TEXT NOT NULL
    );
",
];

/// Migrations of `cache.sqlite`. The cache is disposable, so a file that fails to open or
/// migrate is deleted and rebuilt.
const CACHE_MIGRATIONS: &[&str] = &["
    CREATE TABLE mods (
        path_key     TEXT PRIMARY KEY,
        folder_key   TEXT NOT NULL,
        size         INTEGER NOT NULL,
        mtime_ns     INTEGER NOT NULL,
        scan_version INTEGER NOT NULL,
        entry_json   TEXT NOT NULL,
        icon_json    TEXT
    );
    CREATE INDEX mods_folder ON mods (folder_key);
    CREATE TABLE thumbnails (
        path_key   TEXT PRIMARY KEY,
        folder_key TEXT NOT NULL,
        size       INTEGER NOT NULL,
        mtime_ns   INTEGER NOT NULL,
        png        BLOB NOT NULL
    );
    CREATE INDEX thumbnails_folder ON thumbnails (folder_key);
"];

/// The key two paths share when they name the same file or folder: lower-cased, `/` turned
/// into `\`, without a trailing separator. The game log writes `/` and its settings write
/// `\`, and Windows ignores case in paths.
pub fn path_key(path: &Path) -> String {
    let lower = path.to_string_lossy().to_lowercase().replace('/', "\\");
    lower.trim_end_matches('\\').to_string()
}

/// The key of the folder that directly holds the file or folder with this key.
fn parent_key(key: &str) -> &str {
    key.rsplit_once('\\').map_or("", |(parent, _)| parent)
}

/// Nanoseconds since the Unix epoch, clamped at 0 for earlier times.
fn mtime_ns(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
}

fn size_i64(size: u64) -> i64 {
    i64::try_from(size).unwrap_or(i64::MAX)
}

/// A registered profile folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredProfile {
    /// Short stable id, `p<rowid>`.
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

/// A journaled write to `gameSettings.xml` (plan M4): what it replaced, what it wrote and how it
/// ended. Holds the two override states and a backup file name, never any other file text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredOp {
    /// Short stable id, `o<rowid>`.
    pub id: String,
    /// Unix seconds.
    pub at: i64,
    /// `switch`, `undo` or `restore`.
    pub kind: String,
    pub from: ModsOverride,
    pub to: ModsOverride,
    pub backup_file: Option<String>,
    /// `ok`, or the camelCase error code of a failed attempt.
    pub result: String,
}

/// What [`Store::add_op`] needs to journal a write.
#[derive(Debug, Clone, Copy)]
pub struct NewOp<'a> {
    pub kind: &'a str,
    pub from: &'a ModsOverride,
    pub to: &'a ModsOverride,
    pub backup_file: Option<&'a str>,
    pub result: &'a str,
}

/// One journaled step of a copy, move or remove (plan M5): one row per item attempted, and one
/// for each existing mod a replace sent to the Recycle Bin. Paths and codes only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFileOp {
    /// Short stable id, `f<rowid>`.
    pub id: String,
    /// Unix seconds.
    pub at: i64,
    /// `copy`, `move` or `recycle`.
    pub kind: String,
    /// The transfer plan this belonged to; none for a remove.
    pub plan_id: Option<String>,
    pub source: PathBuf,
    /// Where a copy or move put the mod; none for a recycle.
    pub target: Option<PathBuf>,
    /// `ok`, or the camelCase error code of the failed item.
    pub result: String,
}

/// What [`Store::add_file_op`] needs to journal a file operation.
#[derive(Debug, Clone, Copy)]
pub struct NewFileOp<'a> {
    pub kind: &'a str,
    pub plan_id: Option<&'a str>,
    pub source: &'a Path,
    pub target: Option<&'a Path>,
    pub result: &'a str,
}

/// What `scan::read_mod` produced for one file, before folder-level `assess`.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedMod {
    pub entry: ModEntry,
    pub icon: Option<IconLocation>,
}

/// Identity of one version of a mod file on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    pub size: u64,
    pub modified: SystemTime,
}

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    /// A stored value could not be decoded (e.g. JSON written by a newer version).
    Corrupt(String),
    /// A file could not be created or removed.
    Io(io::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "database error: {e}"),
            StoreError::Corrupt(d) => write!(f, "stored data could not be read: {d}"),
            StoreError::Io(e) => write!(f, "database file error: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Corrupt(e.to_string())
    }
}

/// The cached records of one folder, read with a single query. Each record can be taken once.
#[derive(Debug)]
pub struct CachedFolder {
    rows: HashMap<String, CachedRow>,
}

#[derive(Debug)]
struct CachedRow {
    size: i64,
    mtime_ns: i64,
    record: CachedMod,
}

impl CachedFolder {
    /// The cached record for `path` when it was stored for exactly this size and mtime.
    pub fn take(&mut self, path: &Path, stamp: FileStamp) -> Option<CachedMod> {
        let key = path_key(path);
        let row = self.rows.get(&key)?;
        if row.size != size_i64(stamp.size) || row.mtime_ns != mtime_ns(stamp.modified) {
            return None;
        }
        self.rows.remove(&key).map(|row| row.record)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

pub struct Store {
    library: Mutex<Connection>,
    cache: Mutex<Connection>,
}

impl Store {
    /// Opens (creating if needed) and migrates both files. A `cache.sqlite` that cannot be
    /// opened or migrated is deleted and recreated; a bad `library.sqlite` is an error.
    pub fn open(library_db: &Path, cache_db: &Path) -> Result<Self, StoreError> {
        create_parent(library_db)?;
        create_parent(cache_db)?;
        let mut library = Connection::open(library_db)?;
        migrate(&mut library, LIBRARY_MIGRATIONS)?;
        let cache = match open_cache(cache_db) {
            Ok(cache) => cache,
            Err(_) => {
                remove_cache_files(cache_db)?;
                open_cache(cache_db)?
            }
        };
        Ok(Self::from_connections(library, cache))
    }

    /// Both databases in memory, for tests.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let mut library = Connection::open_in_memory()?;
        migrate(&mut library, LIBRARY_MIGRATIONS)?;
        let mut cache = Connection::open_in_memory()?;
        migrate(&mut cache, CACHE_MIGRATIONS)?;
        Ok(Self::from_connections(library, cache))
    }

    fn from_connections(library: Connection, cache: Connection) -> Self {
        Self {
            library: Mutex::new(library),
            cache: Mutex::new(cache),
        }
    }

    // ---- profile registry (library.sqlite) ----

    /// All registered profiles in the order they were added.
    pub fn profiles(&self) -> Result<Vec<StoredProfile>, StoreError> {
        let conn = lock(&self.library);
        let mut stmt = conn.prepare_cached("SELECT id, name, path FROM profiles ORDER BY id")?;
        let rows = stmt.query_map([], profile_from_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Registers a folder, or returns the existing profile when the same folder (compared
    /// case-insensitively, ignoring separator style and a trailing separator) is registered.
    pub fn add_profile(&self, path: &Path, name: &str) -> Result<StoredProfile, StoreError> {
        let key = path_key(path);
        let added_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let conn = lock(&self.library);
        conn.execute(
            "INSERT OR IGNORE INTO profiles (name, path, path_key, added_at) VALUES (?1, ?2, ?3, ?4)",
            params![name, path.to_string_lossy(), key, added_at],
        )?;
        Ok(conn.query_row(
            "SELECT id, name, path FROM profiles WHERE path_key = ?1",
            [&key],
            profile_from_row,
        )?)
    }

    /// Changes a profile's display name. Returns false when there is no such profile.
    pub fn rename_profile(&self, id: &str, name: &str) -> Result<bool, StoreError> {
        let Some(rowid) = profile_rowid(id) else {
            return Ok(false);
        };
        let conn = lock(&self.library);
        let changed = conn.execute(
            "UPDATE profiles SET name = ?1 WHERE id = ?2",
            params![name, rowid],
        )?;
        Ok(changed > 0)
    }

    /// Removes a profile from the registry; its folder is not touched. Returns false when there
    /// is no such profile.
    pub fn remove_profile(&self, id: &str) -> Result<bool, StoreError> {
        let Some(rowid) = profile_rowid(id) else {
            return Ok(false);
        };
        let conn = lock(&self.library);
        let changed = conn.execute("DELETE FROM profiles WHERE id = ?1", [rowid])?;
        Ok(changed > 0)
    }

    // ---- journal of file operations (library.sqlite) ----

    /// Appends a file-operation journal row and returns its id (`f<rowid>`).
    pub fn add_file_op(&self, op: &NewFileOp<'_>) -> Result<String, StoreError> {
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let conn = lock(&self.library);
        conn.execute(
            "INSERT INTO file_ops (at, kind, plan_id, source, target, result) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                at,
                op.kind,
                op.plan_id,
                op.source.to_string_lossy(),
                op.target.map(|t| t.to_string_lossy().into_owned()),
                op.result
            ],
        )?;
        Ok(format!("f{}", conn.last_insert_rowid()))
    }

    /// Every file-operation journal row, oldest first.
    pub fn file_ops(&self) -> Result<Vec<StoredFileOp>, StoreError> {
        let conn = lock(&self.library);
        let mut stmt = conn.prepare_cached(
            "SELECT id, at, kind, plan_id, source, target, result FROM file_ops ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(StoredFileOp {
                id: format!("f{}", row.get::<_, i64>(0)?),
                at: row.get(1)?,
                kind: row.get(2)?,
                plan_id: row.get(3)?,
                source: PathBuf::from(row.get::<_, String>(4)?),
                target: row.get::<_, Option<String>>(5)?.map(PathBuf::from),
                result: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // ---- journal of writes to gameSettings.xml (library.sqlite) ----

    /// Appends a journal row and returns its id (`o<rowid>`).
    pub fn add_op(&self, op: &NewOp<'_>) -> Result<String, StoreError> {
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let dir = |o: &ModsOverride| {
            o.directory
                .as_ref()
                .map(|d| d.to_string_lossy().into_owned())
        };
        let conn = lock(&self.library);
        conn.execute(
            "INSERT INTO ops \
             (at, kind, from_active, from_dir, to_active, to_dir, backup_file, result) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                at,
                op.kind,
                op.from.active,
                dir(op.from),
                op.to.active,
                dir(op.to),
                op.backup_file,
                op.result
            ],
        )?;
        Ok(format!("o{}", conn.last_insert_rowid()))
    }

    /// The newest journal row whose result is `ok`.
    pub fn latest_ok_op(&self) -> Result<Option<StoredOp>, StoreError> {
        let conn = lock(&self.library);
        Ok(conn
            .query_row(
                "SELECT id, at, kind, from_active, from_dir, to_active, to_dir, backup_file, \
                 result FROM ops WHERE result = 'ok' ORDER BY id DESC LIMIT 1",
                [],
                op_from_row,
            )
            .optional()?)
    }

    /// Every journal row, oldest first.
    pub fn ops(&self) -> Result<Vec<StoredOp>, StoreError> {
        let conn = lock(&self.library);
        let mut stmt = conn.prepare_cached(
            "SELECT id, at, kind, from_active, from_dir, to_active, to_dir, backup_file, result \
             FROM ops ORDER BY id",
        )?;
        let rows = stmt.query_map([], op_from_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // ---- scan cache (cache.sqlite) ----

    /// The cached record for one file, when stored for this size, mtime and the current
    /// `scan::SCAN_VERSION`. Prefer [`Store::cached_mods_in`] when reading a whole folder.
    pub fn cached_mod(
        &self,
        path: &Path,
        stamp: FileStamp,
    ) -> Result<Option<CachedMod>, StoreError> {
        self.cached_mod_versioned(path, stamp, SCAN_VERSION)
    }

    fn cached_mod_versioned(
        &self,
        path: &Path,
        stamp: FileStamp,
        version: u32,
    ) -> Result<Option<CachedMod>, StoreError> {
        let key = path_key(path);
        let row = {
            let conn = lock(&self.cache);
            conn.query_row(
                "SELECT entry_json, icon_json FROM mods \
                 WHERE path_key = ?1 AND size = ?2 AND mtime_ns = ?3 AND scan_version = ?4",
                params![
                    key,
                    size_i64(stamp.size),
                    mtime_ns(stamp.modified),
                    i64::from(version)
                ],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
            )
            .optional()?
        };
        Ok(row.and_then(|(entry, icon)| decode_record(&entry, icon.as_deref())))
    }

    /// Every cached record of the files directly inside `folder` for the current
    /// `scan::SCAN_VERSION`, in one query. Match each file with [`CachedFolder::take`].
    pub fn cached_mods_in(&self, folder: &Path) -> Result<CachedFolder, StoreError> {
        self.cached_mods_in_versioned(folder, SCAN_VERSION)
    }

    fn cached_mods_in_versioned(
        &self,
        folder: &Path,
        version: u32,
    ) -> Result<CachedFolder, StoreError> {
        let conn = lock(&self.cache);
        let mut stmt = conn.prepare_cached(
            "SELECT path_key, size, mtime_ns, entry_json, icon_json FROM mods \
             WHERE folder_key = ?1 AND scan_version = ?2",
        )?;
        let raw = stmt.query_map(params![path_key(folder), i64::from(version)], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        let mut rows = HashMap::new();
        for row in raw {
            let (key, size, mtime_ns, entry, icon) = row?;
            // A row this build cannot decode is just a miss; the scan overwrites it.
            if let Some(record) = decode_record(&entry, icon.as_deref()) {
                rows.insert(
                    key,
                    CachedRow {
                        size,
                        mtime_ns,
                        record,
                    },
                );
            }
        }
        Ok(CachedFolder { rows })
    }

    pub fn put_mod(
        &self,
        path: &Path,
        stamp: FileStamp,
        record: &CachedMod,
    ) -> Result<(), StoreError> {
        self.put_mods(&[(path, stamp, record)])
    }

    /// Stores many records in one transaction.
    pub fn put_mods(&self, items: &[(&Path, FileStamp, &CachedMod)]) -> Result<(), StoreError> {
        self.put_mods_versioned(items, SCAN_VERSION)
    }

    fn put_mods_versioned(
        &self,
        items: &[(&Path, FileStamp, &CachedMod)],
        version: u32,
    ) -> Result<(), StoreError> {
        // Encode before taking the lock, so the connection is busy only for the writes.
        let mut encoded = Vec::with_capacity(items.len());
        for (path, stamp, record) in items {
            let key = path_key(path);
            let folder = parent_key(&key).to_string();
            let entry = serde_json::to_string(&record.entry)?;
            let icon = record
                .icon
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;
            encoded.push((key, folder, *stamp, entry, icon));
        }
        let mut conn = lock(&self.cache);
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO mods \
                 (path_key, folder_key, size, mtime_ns, scan_version, entry_json, icon_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for (key, folder, stamp, entry, icon) in &encoded {
                stmt.execute(params![
                    key,
                    folder,
                    size_i64(stamp.size),
                    mtime_ns(stamp.modified),
                    i64::from(version),
                    entry,
                    icon
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Removes cached records and thumbnails for files directly inside `folder` that are not
    /// in `present`. Returns how many records were removed.
    pub fn prune(&self, folder: &Path, present: &[PathBuf]) -> Result<usize, StoreError> {
        let folder_key = path_key(folder);
        let keep: HashSet<String> = present.iter().map(|p| path_key(p)).collect();
        let mut conn = lock(&self.cache);
        let tx = conn.transaction()?;
        let mut removed_records = 0;
        for table in ["mods", "thumbnails"] {
            let stale: Vec<String> = {
                let mut stmt = tx.prepare_cached(&format!(
                    "SELECT path_key FROM {table} WHERE folder_key = ?1"
                ))?;
                let keys = stmt.query_map([&folder_key], |r| r.get::<_, String>(0))?;
                let mut stale = Vec::new();
                for key in keys {
                    let key = key?;
                    if !keep.contains(&key) {
                        stale.push(key);
                    }
                }
                stale
            };
            let mut delete =
                tx.prepare_cached(&format!("DELETE FROM {table} WHERE path_key = ?1"))?;
            for key in &stale {
                delete.execute([key])?;
            }
            if table == "mods" {
                removed_records = stale.len();
            }
        }
        tx.commit()?;
        Ok(removed_records)
    }

    /// The cached PNG thumbnail, when stored for exactly this size and mtime.
    pub fn thumbnail(&self, path: &Path, stamp: FileStamp) -> Result<Option<Vec<u8>>, StoreError> {
        let conn = lock(&self.cache);
        Ok(conn
            .query_row(
                "SELECT png FROM thumbnails WHERE path_key = ?1 AND size = ?2 AND mtime_ns = ?3",
                params![
                    path_key(path),
                    size_i64(stamp.size),
                    mtime_ns(stamp.modified)
                ],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .optional()?)
    }

    pub fn put_thumbnail(
        &self,
        path: &Path,
        stamp: FileStamp,
        png: &[u8],
    ) -> Result<(), StoreError> {
        let key = path_key(path);
        let folder = parent_key(&key).to_string();
        let conn = lock(&self.cache);
        conn.execute(
            "INSERT OR REPLACE INTO thumbnails (path_key, folder_key, size, mtime_ns, png) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                key,
                folder,
                size_i64(stamp.size),
                mtime_ns(stamp.modified),
                png
            ],
        )?;
        Ok(())
    }
}

/// A poisoned lock only means another thread panicked mid-call; every call here is a single
/// statement or transaction, so the connection is still consistent.
fn lock(conn: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    conn.lock().unwrap_or_else(PoisonError::into_inner)
}

fn profile_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredProfile> {
    Ok(StoredProfile {
        id: format!("p{}", row.get::<_, i64>(0)?),
        name: row.get(1)?,
        path: PathBuf::from(row.get::<_, String>(2)?),
    })
}

/// The row id inside a profile id (`p<rowid>`).
fn profile_rowid(id: &str) -> Option<i64> {
    id.strip_prefix('p')?.parse().ok()
}

fn op_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredOp> {
    let state = |active: usize, dir: usize| -> rusqlite::Result<ModsOverride> {
        Ok(ModsOverride {
            active: row.get(active)?,
            directory: row.get::<_, Option<String>>(dir)?.map(PathBuf::from),
        })
    };
    Ok(StoredOp {
        id: format!("o{}", row.get::<_, i64>(0)?),
        at: row.get(1)?,
        kind: row.get(2)?,
        from: state(3, 4)?,
        to: state(5, 6)?,
        backup_file: row.get(7)?,
        result: row.get(8)?,
    })
}

fn decode_record(entry_json: &str, icon_json: Option<&str>) -> Option<CachedMod> {
    let entry = serde_json::from_str(entry_json).ok()?;
    let icon = match icon_json {
        Some(json) => Some(serde_json::from_str(json).ok()?),
        None => None,
    };
    Some(CachedMod { entry, icon })
}

fn create_parent(file: &Path) -> Result<(), StoreError> {
    match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => Ok(fs::create_dir_all(parent)?),
        _ => Ok(()),
    }
}

fn open_cache(path: &Path) -> Result<Connection, StoreError> {
    let mut conn = Connection::open(path)?;
    // Reads the journal mode back: this is also the first statement to fail on a garbage file.
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    migrate(&mut conn, CACHE_MIGRATIONS)?;
    Ok(conn)
}

/// Deletes `cache.sqlite` and its WAL side files. A missing file is fine.
fn remove_cache_files(path: &Path) -> Result<(), StoreError> {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = OsString::from(path.as_os_str());
        name.push(suffix);
        match fs::remove_file(PathBuf::from(name)) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Applies the migrations the file has not seen, each in its own transaction, and records the
/// new `schema_version`. A file from a newer app version is an error.
fn migrate(conn: &mut Connection, migrations: &[&str]) -> Result<(), StoreError> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")?;
    let current: i64 = conn
        .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
        .optional()?
        .unwrap_or(0);
    let applied = usize::try_from(current)
        .map_err(|_| StoreError::Corrupt(format!("schema version {current} is not valid")))?;
    if applied > migrations.len() {
        return Err(StoreError::Corrupt(format!(
            "schema version {applied} is newer than this app supports ({})",
            migrations.len()
        )));
    }
    for (index, sql) in migrations.iter().enumerate().skip(applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute("DELETE FROM schema_version", [])?;
        tx.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            [index as i64 + 1],
        )?;
        tx.commit()?;
    }
    Ok(())
}
