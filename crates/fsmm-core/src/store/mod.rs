//! The app's own SQLite files (plan M3 decision 3). Never anything of the game's.
//!
//! - `library.sqlite` (app data dir): user data. Today the profile registry; later tags,
//!   notes and mod sources. Losing it loses user choices.
//! - `cache.sqlite` (app cache dir): disposable. Per-mod scan records and thumbnails, keyed
//!   by the mod's path, size and mtime (and `scan::SCAN_VERSION` for records). Deleting it is
//!   always safe; the next scan rebuilds it.
//!
//! Each file has a `schema_version` and a numbered list of migrations applied in order.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::container::IconLocation;
use crate::model::ModEntry;

/// A registered profile folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredProfile {
    /// Short stable id, `p<rowid>`.
    pub id: String,
    pub name: String,
    pub path: PathBuf,
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
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "database error: {e}"),
            StoreError::Corrupt(d) => write!(f, "stored data could not be read: {d}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

pub struct Store {
    // WS-A: the two connections.
}

impl Store {
    /// Opens (creating if needed) and migrates both files.
    pub fn open(_library_db: &Path, _cache_db: &Path) -> Result<Self, StoreError> {
        todo!("WS-A")
    }

    /// Both databases in memory, for tests.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        todo!("WS-A")
    }

    // ---- profile registry (library.sqlite) ----

    /// All registered profiles in the order they were added.
    pub fn profiles(&self) -> Result<Vec<StoredProfile>, StoreError> {
        todo!("WS-A")
    }

    /// Registers a folder, or returns the existing profile when the same folder (compared
    /// case-insensitively, ignoring separator style and a trailing separator) is registered.
    pub fn add_profile(&self, _path: &Path, _name: &str) -> Result<StoredProfile, StoreError> {
        todo!("WS-A")
    }

    // ---- scan cache (cache.sqlite) ----

    pub fn cached_mod(
        &self,
        _path: &Path,
        _stamp: FileStamp,
    ) -> Result<Option<CachedMod>, StoreError> {
        todo!("WS-A")
    }

    pub fn put_mod(
        &self,
        _path: &Path,
        _stamp: FileStamp,
        _record: &CachedMod,
    ) -> Result<(), StoreError> {
        todo!("WS-A")
    }

    /// Removes cached records and thumbnails for files directly inside `folder` that are not
    /// in `present`. Returns how many records were removed.
    pub fn prune(&self, _folder: &Path, _present: &[PathBuf]) -> Result<usize, StoreError> {
        todo!("WS-A")
    }

    pub fn thumbnail(
        &self,
        _path: &Path,
        _stamp: FileStamp,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        todo!("WS-A")
    }

    pub fn put_thumbnail(
        &self,
        _path: &Path,
        _stamp: FileStamp,
        _png: &[u8],
    ) -> Result<(), StoreError> {
        todo!("WS-A")
    }
}
