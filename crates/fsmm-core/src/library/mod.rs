//! Orchestration behind the app's commands: the profile registry, cache-assisted scans,
//! cross-profile `also_in`, profile summaries and thumbnails. Free of Tauri, so it is tested
//! with temp folders. The Tauri layer only converts arguments and results.
//!
//! Every listing is fresh (plan M3 decision 1): folders are re-enumerated on each call and only
//! files whose path, size or mtime changed are read again. Callers identify profiles and mods
//! by id; this module resolves ids to paths (decision 5), so no caller-supplied path is read
//! except the one passed to [`Library::add_profile`] from the folder picker.
//!
//! `Library` is `Send + Sync` and its methods take `&self`: a long scan must not block
//! thumbnail requests for longer than a short database access. A scan therefore holds no lock
//! while it reads mods or decodes icons. The only locks it takes are the cache connection
//! (one query, then one transaction), a progress mutex (tried, never waited on mid-scan) and
//! the map of last scans (inserting one `Arc`).

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::{Duration, Instant, UNIX_EPOCH};

use rayon::prelude::*;

use crate::container::{IconLocation, ModContainer};
use crate::discovery::{suggest_profiles, GameContext};
use crate::icons;
use crate::model::{ApiError, ErrorCode, ModEntry, Profile, ScanProgress};
use crate::scan::{self, ModRecord};
use crate::store::{path_key, CachedMod, FileStamp, Store, StoreError, StoredProfile};

mod switching;
mod transfer;

pub use transfer::OpEnv;
#[cfg(test)]
mod tests;

/// `FolderMissing` detail when a thumbnail is requested for a mod without an icon.
pub const NO_ICON: &str = "noIcon";

/// Progress events are sent at most this often while a scan runs (the final one always is).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        ApiError::new(ErrorCode::Internal, Some(e.to_string()))
    }
}

/// The result of scanning one folder: assessed records and their on-disk stamps, in listing
/// order, with lookups by mod id and by lower-cased mod name.
struct Scan {
    records: Vec<ModRecord>,
    /// `stamps[i]` belongs to `records[i]`.
    stamps: Vec<FileStamp>,
    by_id: HashMap<String, usize>,
    names: HashSet<String>,
}

pub struct Library {
    store: Store,
    thumbnail_base: String,
    /// The last scan per profile id. Locked only to look one up or insert one.
    scans: RwLock<HashMap<String, Arc<Scan>>>,
    /// The context of the last call that had one: `mod_path` and `thumbnail` take only ids but
    /// may have to scan a profile, which needs the languages and the logged descVersion.
    last_ctx: Mutex<Option<GameContext>>,
    #[cfg(test)]
    decodes: std::sync::atomic::AtomicUsize,
}

impl Library {
    /// `thumbnail_base` is the URL prefix for `ModEntry::icon`, e.g. `http://thumb.localhost`.
    /// Icons become `<base>/<profileId>/<modId>?v=<size>-<mtime>`, ids percent-encoded.
    pub fn new(store: Store, thumbnail_base: &str) -> Self {
        Self {
            store,
            thumbnail_base: thumbnail_base.trim_end_matches('/').to_string(),
            scans: RwLock::new(HashMap::new()),
            last_ctx: Mutex::new(None),
            #[cfg(test)]
            decodes: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Plan M3 decision 6: when no profile is registered, registers
    /// `discovery::suggest_profiles(user_dir)`; always registers the effective and default
    /// mods folders when they exist and are missing. The name is the folder name.
    pub fn ensure_profiles(&self, ctx: &GameContext) -> Result<(), ApiError> {
        self.remember(ctx);
        if self.store.profiles()?.is_empty() {
            if let Some(user_dir) = &ctx.user_dir {
                for folder in suggest_profiles(user_dir) {
                    self.register(&folder)?;
                }
            }
        }
        for folder in [&ctx.effective_mods_dir, &ctx.default_mods_dir]
            .into_iter()
            .flatten()
        {
            if folder.is_dir() {
                self.register(folder)?;
            }
        }
        Ok(())
    }

    /// Every registered profile with fresh counts. A folder that no longer exists is listed
    /// with zero counts rather than failing the call.
    pub fn list_profiles(&self, ctx: &GameContext) -> Result<Vec<Profile>, ApiError> {
        self.remember(ctx);
        let profiles = self.store.profiles()?;
        profiles
            .par_iter()
            .map(|profile| self.summarize(profile, ctx))
            .collect()
    }

    /// The profile's mods after `assess`, with `also_in` and icon URLs filled. `progress` is
    /// called with `done` rising from 0 to `total` (the number of entries enumerated).
    pub fn list_mods(
        &self,
        profile_id: &str,
        ctx: &GameContext,
        progress: &(dyn Fn(ScanProgress) + Sync),
    ) -> Result<Vec<ModEntry>, ApiError> {
        self.remember(ctx);
        let profiles = self.store.profiles()?;
        let profile = profiles
            .iter()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| profile_not_found(profile_id))?;
        let scan = self.scan_profile(profile, ctx, progress)?;

        // Other profiles' last scans; a folder that cannot be scanned simply holds no mods.
        let others: Vec<(&str, Arc<Scan>)> = profiles
            .iter()
            .filter(|p| p.id != profile_id)
            .filter_map(|p| {
                let known = self.last_scan(&p.id);
                let scan = known.or_else(|| self.scan_profile(p, ctx, &no_progress).ok())?;
                Some((p.id.as_str(), scan))
            })
            .collect();

        Ok(scan
            .records
            .iter()
            .zip(&scan.stamps)
            .map(|(record, stamp)| {
                let mut entry = record.entry.clone();
                let name = entry.mod_name.to_lowercase();
                entry.also_in = others
                    .iter()
                    .filter(|(_, other)| other.names.contains(&name))
                    .map(|(id, _)| (*id).to_string())
                    .collect();
                entry.icon = record
                    .icon
                    .as_ref()
                    .map(|_| self.icon_url(profile_id, &entry.id, *stamp));
                entry
            })
            .collect())
    }

    /// Registers a folder chosen by the user. Re-adding a registered folder returns it.
    pub fn add_profile(&self, path: &Path, ctx: &GameContext) -> Result<Profile, ApiError> {
        self.remember(ctx);
        if !path.is_dir() {
            return Err(ApiError::new(
                ErrorCode::FolderMissing,
                Some(path.to_string_lossy().into_owned()),
            ));
        }
        let profile = self.register(path)?;
        self.summarize(&profile, ctx)
    }

    /// The folder of a registered profile.
    pub fn profile_path(&self, profile_id: &str) -> Result<PathBuf, ApiError> {
        Ok(self.find_profile(profile_id)?.path)
    }

    /// The file or folder of a mod from the profile's last scan.
    pub fn mod_path(&self, profile_id: &str, mod_id: &str) -> Result<PathBuf, ApiError> {
        let (path, _, _) = self.resolve_mod(profile_id, mod_id)?;
        Ok(path)
    }

    /// PNG thumbnail of a mod from the profile's last scan: from the cache, or decoded and
    /// cached. Unknown ids, and mods without an icon, are errors.
    pub fn thumbnail(&self, profile_id: &str, mod_id: &str) -> Result<Vec<u8>, ApiError> {
        let (path, stamp, icon) = self.resolve_mod(profile_id, mod_id)?;
        let icon =
            icon.ok_or_else(|| ApiError::new(ErrorCode::FolderMissing, Some(NO_ICON.to_string())))?;
        if let Some(png) = self.store.thumbnail(&path, stamp)? {
            return Ok(png);
        }

        // No lock is held from here: opening and decoding is the slow part.
        #[cfg(test)]
        self.decodes.fetch_add(1, Ordering::SeqCst);
        let mut container = ModContainer::open(&path).map_err(|e| {
            let code = if e.kind() == io::ErrorKind::NotFound {
                ErrorCode::FolderMissing
            } else {
                ErrorCode::Internal
            };
            ApiError::new(code, Some(e.to_string()))
        })?;
        let png = icons::thumbnail(&mut container, &icon, icons::THUMBNAIL_EDGE)
            .map_err(|e| ApiError::new(ErrorCode::Internal, Some(e.to_string())))?;
        self.store.put_thumbnail(&path, stamp, &png)?;
        Ok(png)
    }

    // ---- registry helpers ----

    fn register(&self, folder: &Path) -> Result<StoredProfile, ApiError> {
        let name = folder.file_name().map_or_else(
            || folder.to_string_lossy().into_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        Ok(self.store.add_profile(folder, &name)?)
    }

    fn find_profile(&self, profile_id: &str) -> Result<StoredProfile, ApiError> {
        self.store
            .profiles()?
            .into_iter()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| profile_not_found(profile_id))
    }

    fn remember(&self, ctx: &GameContext) {
        *self.last_ctx.lock().unwrap_or_else(PoisonError::into_inner) = Some(ctx.clone());
    }

    fn remembered(&self) -> GameContext {
        self.last_ctx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .unwrap_or_default()
    }

    // ---- scans ----

    fn last_scan(&self, profile_id: &str) -> Option<Arc<Scan>> {
        self.scans
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(profile_id)
            .cloned()
    }

    /// The last scan of a profile, scanning it first when there is none.
    fn scan_of(&self, profile_id: &str) -> Result<Arc<Scan>, ApiError> {
        if let Some(scan) = self.last_scan(profile_id) {
            return Ok(scan);
        }
        let profile = self.find_profile(profile_id)?;
        self.scan_profile(&profile, &self.remembered(), &no_progress)
    }

    /// Path, stamp and icon of a mod in the profile's last scan, copied out so no lock or
    /// `Arc` is held while the caller works.
    fn resolve_mod(
        &self,
        profile_id: &str,
        mod_id: &str,
    ) -> Result<(PathBuf, FileStamp, Option<IconLocation>), ApiError> {
        let scan = self.scan_of(profile_id)?;
        let unknown = || ApiError::new(ErrorCode::FolderMissing, Some(mod_id.to_string()));
        let index = *scan.by_id.get(mod_id).ok_or_else(unknown)?;
        let record = scan.records.get(index).ok_or_else(unknown)?;
        let stamp = *scan.stamps.get(index).ok_or_else(unknown)?;
        Ok((record.path.clone(), stamp, record.icon.clone()))
    }

    /// Scans one profile folder, reading only the mods the cache does not know, and remembers
    /// the result. Holds no lock while reading mods.
    fn scan_profile(
        &self,
        profile: &StoredProfile,
        ctx: &GameContext,
        progress: &(dyn Fn(ScanProgress) + Sync),
    ) -> Result<Arc<Scan>, ApiError> {
        let dir = profile.path.as_path();
        if !dir.is_dir() {
            return Err(ApiError::new(
                ErrorCode::FolderMissing,
                Some(profile.id.clone()),
            ));
        }
        let (entries, _skipped) = scan::enumerate(dir).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                ApiError::new(ErrorCode::FolderMissing, Some(profile.id.clone()))
            } else {
                ApiError::new(ErrorCode::FolderUnreadable, Some(format!("{:?}", e.kind())))
            }
        })?;
        let total = u32::try_from(entries.len()).unwrap_or(u32::MAX);
        let reporter = Reporter::new(&profile.id, total, progress);

        let stamps: Vec<FileStamp> = entries
            .iter()
            .map(|e| FileStamp {
                size: e.size,
                modified: e.modified,
            })
            .collect();

        // One query for the folder, then match each entry's stamp.
        let mut cached = self.store.cached_mods_in(dir)?;
        let mut slots: Vec<Option<CachedMod>> = entries
            .iter()
            .zip(&stamps)
            .map(|(entry, stamp)| {
                // A case-only rename keeps the key, size and mtime but changes the entry's names.
                cached
                    .take(&entry.path, *stamp)
                    .filter(|c| c.entry.file_name == entry.name)
            })
            .collect();
        let misses: Vec<usize> = (0..entries.len()).filter(|&i| slots[i].is_none()).collect();
        reporter.advance(total.saturating_sub(u32::try_from(misses.len()).unwrap_or(total)));

        let languages: Vec<&str> = ctx.languages.iter().map(String::as_str).collect();
        let read: Vec<(usize, ModRecord)> = misses
            .par_iter()
            .map(|&i| {
                let record = scan::read_mod(&entries[i], &profile.id, &languages);
                reporter.advance(1);
                (i, record)
            })
            .collect();
        for (i, record) in read {
            slots[i] = Some(CachedMod {
                entry: record.entry,
                icon: record.icon,
            });
        }

        let to_store: Vec<(&Path, FileStamp, &CachedMod)> = misses
            .iter()
            .filter_map(|&i| Some((entries[i].path.as_path(), stamps[i], slots[i].as_ref()?)))
            .collect();
        self.store.put_mods(&to_store)?;
        let present: Vec<PathBuf> = entries.iter().map(|e| e.path.clone()).collect();
        self.store.prune(dir, &present)?;

        let mut records: Vec<ModRecord> = entries
            .iter()
            .zip(slots)
            .filter_map(|(entry, slot)| {
                let cached = slot?;
                Some(ModRecord {
                    path: entry.path.clone(),
                    entry: cached.entry,
                    icon: cached.icon,
                })
            })
            .collect();
        scan::assess(&mut records, ctx.logged_desc_version);
        for record in &mut records {
            // Cached entries may carry the id of the profile they were first read for.
            record.entry.profile_id.clone_from(&profile.id);
        }
        reporter.finish();

        let scan = Arc::new(Scan {
            by_id: records
                .iter()
                .enumerate()
                .map(|(i, r)| (r.entry.id.clone(), i))
                .collect(),
            names: records
                .iter()
                .map(|r| r.entry.mod_name.to_lowercase())
                .collect(),
            records,
            stamps,
        });
        self.scans
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(profile.id.clone(), Arc::clone(&scan));
        Ok(scan)
    }

    /// A profile's summary from a fresh scan. A missing folder has zero counts.
    fn summarize(&self, profile: &StoredProfile, ctx: &GameContext) -> Result<Profile, ApiError> {
        let (mod_count, total_bytes, issue_count) =
            match self.scan_profile(profile, ctx, &no_progress) {
                Ok(scan) => (
                    scan.records.len(),
                    scan.records.iter().map(|r| r.entry.size_bytes).sum(),
                    scan.records
                        .iter()
                        .filter(|r| !r.entry.issues.is_empty())
                        .count(),
                ),
                Err(e) if e.code == ErrorCode::FolderMissing => (0, 0, 0),
                Err(e) => return Err(e),
            };
        let key = path_key(&profile.path);
        let is_folder = |dir: &Option<PathBuf>| dir.as_deref().is_some_and(|d| path_key(d) == key);
        Ok(Profile {
            id: profile.id.clone(),
            name: profile.name.clone(),
            path: profile.path.to_string_lossy().into_owned(),
            is_active: is_folder(&ctx.effective_mods_dir),
            is_game_default: is_folder(&ctx.default_mods_dir),
            mod_count: u32::try_from(mod_count).unwrap_or(u32::MAX),
            total_bytes,
            issue_count: u32::try_from(issue_count).unwrap_or(u32::MAX),
            update_count: 0,
        })
    }

    fn icon_url(&self, profile_id: &str, mod_id: &str, stamp: FileStamp) -> String {
        let secs = stamp
            .modified
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        format!(
            "{}/{}/{}?v={}-{}",
            self.thumbnail_base,
            encode_segment(profile_id),
            encode_segment(mod_id),
            stamp.size,
            secs
        )
    }
}

fn profile_not_found(profile_id: &str) -> ApiError {
    ApiError::new(ErrorCode::ProfileNotFound, Some(profile_id.to_string()))
}

fn no_progress(_: ScanProgress) {}

/// Throttled, monotonic progress reporting for one scan. Events are sent while holding
/// `last`, so they reach the callback in order even when several worker threads advance the
/// count. Workers only *try* to take it and move on, so a slow callback never stalls the scan.
struct Reporter<'a> {
    profile_id: &'a str,
    total: u32,
    done: AtomicU32,
    /// `done` and time of the last event sent.
    last: Mutex<(u32, Instant)>,
    sink: &'a (dyn Fn(ScanProgress) + Sync),
}

impl<'a> Reporter<'a> {
    /// Sends the opening event (`done` 0).
    fn new(profile_id: &'a str, total: u32, sink: &'a (dyn Fn(ScanProgress) + Sync)) -> Self {
        let reporter = Self {
            profile_id,
            total,
            done: AtomicU32::new(0),
            last: Mutex::new((0, Instant::now())),
            sink,
        };
        reporter.send(0);
        reporter
    }

    fn send(&self, done: u32) {
        (self.sink)(ScanProgress {
            profile_id: self.profile_id.to_string(),
            done,
            total: self.total,
        });
    }

    /// Counts `n` more mods as done and sends an event if enough time has passed.
    fn advance(&self, n: u32) {
        self.done.fetch_add(n, Ordering::SeqCst);
        let Ok(mut last) = self.last.try_lock() else {
            return;
        };
        let current = self.done.load(Ordering::SeqCst);
        // The closing event is `finish`'s: it is the only one that reaches `total`.
        if current > last.0 && current < self.total && last.1.elapsed() >= PROGRESS_INTERVAL {
            self.send(current);
            *last = (current, Instant::now());
        }
    }

    /// Sends the closing event (`done` == `total`).
    fn finish(&self) {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        self.send(self.total);
        *last = (self.total, Instant::now());
    }
}

/// Percent-encodes an id for use as one URL path segment (everything but `A-Za-z0-9-._~`).
pub fn encode_segment(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for byte in id.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Reverses [`encode_segment`]; `None` for malformed input or invalid UTF-8.
pub fn decode_segment(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = segment.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}
