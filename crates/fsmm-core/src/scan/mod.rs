//! Turning a mod folder into [`ModEntry`] records: enumerate (stat only), read each mod
//! (the unit a cache miss pays for), then assess folder-level health rules.
//!
//! Read-only: mods are opened with [`ModContainer::open`] and nothing is written.

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::container::IconLocation;
use crate::model::{ModEntry, ModFormat, ScanSummary};

/// One candidate mod found by [`enumerate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderEntry {
    pub path: PathBuf,
    /// File or folder name, e.g. `FS25_Example.zip`.
    pub name: String,
    pub format: ModFormat,
    /// File size, or the total size of a folder's files.
    pub size: u64,
    pub modified: SystemTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModRecord {
    pub path: PathBuf,
    pub entry: ModEntry,
    pub icon: Option<IconLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub path: PathBuf,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// A file that is not a `.zip`.
    NotAZip,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FolderScan {
    pub mods: Vec<ModRecord>,
    pub skipped: Vec<Skipped>,
    pub summary: ScanSummary,
}

/// Lists candidate mods without opening them. Ignores Syncthing internals and hidden or
/// system entries (plan decision 5).
pub fn enumerate(_dir: &Path) -> io::Result<(Vec<FolderEntry>, Vec<Skipped>)> {
    todo!("WS-B")
}

/// Reads one mod. Never fails: an unreadable mod becomes a record with a `NotAMod` issue.
/// `languages` is the preference order for titles and descriptions; `en` is always tried
/// after it.
pub fn read_mod(_entry: &FolderEntry, _profile_id: &str, _languages: &[&str]) -> ModRecord {
    todo!("WS-B")
}

/// Applies the rules that need the whole folder (twins, conflict copies, dependencies) and
/// the descVersion check against `logged_desc_version` (plan decision 2).
pub fn assess(_records: &mut [ModRecord], _logged_desc_version: Option<u32>) {
    todo!("WS-B")
}

/// `enumerate`, then `read_mod` in parallel, then `assess`.
pub fn scan_folder(
    _dir: &Path,
    _profile_id: &str,
    _languages: &[&str],
    _logged_desc_version: Option<u32>,
) -> io::Result<FolderScan> {
    todo!("WS-B")
}
