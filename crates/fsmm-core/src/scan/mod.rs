//! Turning a mod folder into [`ModEntry`] records: enumerate (stat only), read each mod
//! (the unit a cache miss pays for), then assess folder-level health rules.
//!
//! Read-only: mods are opened with [`ModContainer::open`] and nothing is written.
//!
//! Issue `detail` values are data, not sentences; the UI words the message. The `NotAMod`
//! details are the `NOT_A_MOD_*` constants below.

mod health;
mod moddesc;
#[cfg(test)]
mod tests;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rayon::prelude::*;

use crate::container::{IconLocation, ModContainer};
use crate::model::{
    IssueKind, ModEntry, ModFormat, ModIssue, ModKind, ModSource, ScanSummary, Severity,
    UpdateState,
};

pub use health::assess;

/// `NotAMod` detail: the file is not a readable zip, or the folder cannot be read.
pub const NOT_A_MOD_UNREADABLE: &str = "unreadableArchive";
/// `NotAMod` detail: there is no `modDesc.xml` at the root.
pub const NOT_A_MOD_NO_MOD_DESC: &str = "noModDesc";
/// `NotAMod` detail: `modDesc.xml` is too large, unreadable or not valid XML of the right shape.
pub const NOT_A_MOD_INVALID_MOD_DESC: &str = "invalidModDesc";

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
pub fn enumerate(dir: &Path) -> io::Result<(Vec<FolderEntry>, Vec<Skipped>)> {
    let mut entries = Vec::new();
    let mut skipped = Vec::new();
    for item in fs::read_dir(dir)? {
        let item = item?;
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = item.path();
        // Follows symlinks, like the game does when it opens the entry.
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        if is_hidden_or_system(&meta) {
            continue;
        }
        if meta.is_dir() {
            let (size, newest) = folder_stats(&path);
            let modified = newest
                .or_else(|| meta.modified().ok())
                .unwrap_or(UNIX_EPOCH);
            entries.push(FolderEntry {
                path,
                name,
                format: ModFormat::Folder,
                size,
                modified,
            });
        } else if has_zip_extension(&name) {
            entries.push(FolderEntry {
                path,
                name,
                format: ModFormat::Zip,
                size: meta.len(),
                modified: meta.modified().unwrap_or(UNIX_EPOCH),
            });
        } else {
            skipped.push(Skipped {
                path,
                reason: SkipReason::NotAZip,
            });
        }
    }
    entries.sort_by_cached_key(|e| (e.name.to_lowercase(), e.name.clone()));
    skipped.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((entries, skipped))
}

/// Reads one mod. Never fails: an unreadable mod becomes a record with a `NotAMod` issue.
/// `languages` is the preference order for titles and descriptions; `en` is always tried
/// after it.
pub fn read_mod(entry: &FolderEntry, profile_id: &str, languages: &[&str]) -> ModRecord {
    let mod_name = mod_name_of(&entry.name);
    let mut mod_entry = ModEntry {
        id: entry.name.clone(),
        profile_id: profile_id.to_string(),
        mod_name: mod_name.clone(),
        file_name: entry.name.clone(),
        format: entry.format,
        title: mod_name,
        author: String::new(),
        version: String::new(),
        desc_version: None,
        kind: ModKind::Other,
        description: String::new(),
        icon: None,
        size_bytes: entry.size,
        modified: rfc3339(entry.modified),
        multiplayer: false,
        dependencies: Vec::new(),
        issues: Vec::new(),
        source: ModSource::Unknown,
        update: UpdateState::Unknown,
        also_in: Vec::new(),
    };
    let mut icon = None;

    match ModContainer::open(&entry.path) {
        Err(_) => mod_entry
            .issues
            .push(not_a_mod(NOT_A_MOD_UNREADABLE.to_string())),
        Ok(mut container) => {
            if container.find("modDesc.xml").is_none() {
                mod_entry.issues.push(missing_mod_desc_issue(&container));
            } else {
                match moddesc::read_desc(&mut container, languages) {
                    Err(e) => mod_entry.issues.push(not_a_mod(e.detail().to_string())),
                    Ok(desc) => {
                        if !desc.title.is_empty() {
                            mod_entry.title = desc.title;
                        }
                        mod_entry.author = desc.author;
                        mod_entry.version = desc.version;
                        mod_entry.description = desc.description;
                        mod_entry.desc_version = desc.desc_version;
                        mod_entry.multiplayer = desc.multiplayer;
                        mod_entry.dependencies = desc.dependencies;
                        mod_entry.kind = desc.kind;
                        icon = desc.icon;
                    }
                }
            }
        }
    }

    ModRecord {
        path: entry.path.clone(),
        entry: mod_entry,
        icon,
    }
}

/// `enumerate`, then `read_mod` in parallel, then `assess`.
pub fn scan_folder(
    dir: &Path,
    profile_id: &str,
    languages: &[&str],
    logged_desc_version: Option<u32>,
) -> io::Result<FolderScan> {
    let started = Instant::now();
    let (entries, skipped) = enumerate(dir)?;
    let mut mods: Vec<ModRecord> = entries
        .par_iter()
        .map(|e| read_mod(e, profile_id, languages))
        .collect();
    mods.sort_by_cached_key(|r| r.entry.file_name.to_lowercase());
    assess(&mut mods, logged_desc_version);

    let summary = ScanSummary {
        profile_id: profile_id.to_string(),
        mod_count: u32::try_from(mods.len()).unwrap_or(u32::MAX),
        total_bytes: mods.iter().map(|r| r.entry.size_bytes).sum(),
        issue_count: u32::try_from(mods.iter().filter(|r| !r.entry.issues.is_empty()).count())
            .unwrap_or(u32::MAX),
        duration_ms: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
        scanned_at: rfc3339(SystemTime::now()),
    };
    Ok(FolderScan {
        mods,
        skipped,
        summary,
    })
}

/// The mod's identity to the game: the entry name without a `.zip` extension.
fn mod_name_of(file_name: &str) -> String {
    if has_zip_extension(file_name) {
        file_name[..file_name.len() - ".zip".len()].to_string()
    } else {
        file_name.to_string()
    }
}

fn has_zip_extension(name: &str) -> bool {
    name.len() > ".zip".len()
        && name
            .get(name.len() - ".zip".len()..)
            .is_some_and(|ext| ext.eq_ignore_ascii_case(".zip"))
}

fn not_a_mod(detail: String) -> ModIssue {
    ModIssue {
        kind: IssueKind::NotAMod,
        severity: Severity::Error,
        detail: Some(detail),
    }
}

/// A container without a root `modDesc.xml`: either a mod packed inside a wrapper (an inner
/// `.zip`, or `modDesc.xml` one folder down) or simply not a mod.
fn missing_mod_desc_issue(container: &ModContainer) -> ModIssue {
    let inner_zip = container.names().find(|n| has_zip_extension(n));
    let wrapped = || {
        container.names().find(|n| {
            n.split_once('/').is_some_and(|(dir, rest)| {
                !dir.is_empty() && rest.eq_ignore_ascii_case("modDesc.xml")
            })
        })
    };
    match inner_zip.or_else(wrapped) {
        Some(inner) => ModIssue {
            kind: IssueKind::NestedArchive,
            severity: Severity::Warning,
            detail: Some(inner.to_string()),
        },
        None => not_a_mod(NOT_A_MOD_NO_MOD_DESC.to_string()),
    }
}

/// Total bytes of all files under `root` and the newest file mtime. Unreadable parts are
/// skipped.
fn folder_stats(root: &Path) -> (u64, Option<SystemTime>) {
    let mut size = 0u64;
    let mut newest: Option<SystemTime> = None;
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for item in read.flatten() {
            let Ok(file_type) = item.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                pending.push(item.path());
            } else if file_type.is_file() {
                let Ok(meta) = item.metadata() else { continue };
                size = size.saturating_add(meta.len());
                if let Ok(m) = meta.modified() {
                    newest = Some(newest.map_or(m, |n| n.max(m)));
                }
            }
        }
    }
    (size, newest)
}

#[cfg(windows)]
fn is_hidden_or_system(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN_OR_SYSTEM: u32 = 0x2 | 0x4;
    meta.file_attributes() & HIDDEN_OR_SYSTEM != 0
}

#[cfg(not(windows))]
fn is_hidden_or_system(_meta: &fs::Metadata) -> bool {
    false
}

/// RFC 3339 UTC with second precision. Times outside 1970..9999 are clamped, because the
/// formatter cannot represent them.
fn rfc3339(time: SystemTime) -> String {
    const MAX_SECS: u64 = 253_402_300_799;
    let secs = time
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs().min(MAX_SECS));
    let clamped = UNIX_EPOCH + std::time::Duration::from_secs(secs);
    humantime::format_rfc3339_seconds(clamped).to_string()
}
