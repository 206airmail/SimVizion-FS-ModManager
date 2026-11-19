//! Savegame comparison (plan M6): a save's mods checked against a profile, with the game's own
//! file hash computed in parallel and cached (decision 2).
//!
//! The report is built from the save's `careerSavegame.xml` and a fresh scan of the profile, and
//! hashes nothing; `check_savegame_files` then hashes only the zips whose hash is still needed:
//! same version as the save, a zip, a non-empty `fileHash` in the save, and no cached hash for
//! the file's current size and mtime.
//!
//! Locking: hashing has its own busy and cancel flags (`HashState`), so it never blocks file
//! operations, reads or scans, and they never block it. It holds no lock while it hashes: the
//! only shared state is the progress throttle (tried, never waited on mid-hash) and the cache
//! connection (one short write per finished file).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use rayon::prelude::*;

use super::compare::side;
use super::transfer::BusyGuard;
use super::{no_progress, Library, Scan};
use crate::discovery::GameContext;
use crate::files::hash::game_md5;
use crate::files::FileOpError;
use crate::model::{
    ApiError, ErrorCode, HashProgress, ModFormat, SaveModRow, SaveModState, Savegame,
    SavegameReport,
};
use crate::savegame::{self, SavegameError, SavegameFile};
use crate::store::{FileStamp, StoredProfile};

#[cfg(test)]
mod tests;

/// Progress events are sent at most this often (the first and the last always are).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

/// The busy and cancel flags of savegame hashing.
#[derive(Default)]
pub(super) struct HashState {
    busy: AtomicBool,
    cancel: AtomicBool,
    #[cfg(test)]
    hooks: Mutex<tests::Hooks>,
}

/// One zip whose hash the report still needs.
struct HashJob {
    /// Index of its row in the report.
    row: usize,
    path: PathBuf,
    stamp: FileStamp,
    /// The mod name the game salts the hash with: the file's own name without `.zip`.
    salt: String,
    /// The save's `fileHash`, lower-case.
    expected: String,
}

/// A report plus the hashing it still needs.
struct Built {
    report: SavegameReport,
    jobs: Vec<HashJob>,
}

/// A saved mod whose zip is in the profile at the save's version with a `fileHash` to compare:
/// the only kind that is ever hashed.
struct Lookup {
    path: PathBuf,
    stamp: FileStamp,
    salt: String,
}

impl Library {
    /// Every save under the game's user folder (empty when there is no user folder).
    pub fn list_savegames(&self, ctx: &GameContext) -> Result<Vec<Savegame>, ApiError> {
        self.remember(ctx);
        Ok(ctx
            .user_dir
            .as_deref()
            .map(savegame::list)
            .unwrap_or_default())
    }

    /// The save's mods against the profile, without hashing anything: rows whose hash is in
    /// the cache are final, the others that need one are `HashPending`. `pdlc_*` names are
    /// DLCs, installed when `<install_dir>/pdlc/<name>.dlc` exists. `found_in` lists the other
    /// profiles that have the mod name; `extras` the profile's mods the save does not use.
    ///
    /// Mods are paired by name ignoring case, as everywhere in the app (the game itself is
    /// case-sensitive). The hash of a paired zip is the game's own, salted with that file's
    /// mod name, so a case-only difference in the name shows as `FileDiffers`.
    pub fn savegame_report(
        &self,
        savegame_id: &str,
        profile_id: &str,
        ctx: &GameContext,
    ) -> Result<SavegameReport, ApiError> {
        Ok(self.build_report(savegame_id, profile_id, ctx)?.report)
    }

    /// Hashes the zips the report left `HashPending` (in parallel, cached by path, size and
    /// mtime) and returns the final report. Stops early on `cancel_hashing` (`Cancelled`);
    /// hashes finished before that stay cached. One hashing run at a time (`Busy`).
    ///
    /// A zip that cannot be read (gone, locked) keeps its row `HashPending`: nothing proves it
    /// differs, and one bad file must not fail the whole check. Its bytes count as done so the
    /// progress still reaches the total.
    pub fn check_savegame_files(
        &self,
        savegame_id: &str,
        profile_id: &str,
        ctx: &GameContext,
        progress: &(dyn Fn(HashProgress) + Sync),
    ) -> Result<SavegameReport, ApiError> {
        let _busy = BusyGuard::acquire(&self.hashing.busy)?;
        self.hashing.cancel.store(false, Ordering::SeqCst);
        let Built { mut report, jobs } = self.build_report(savegame_id, profile_id, ctx)?;

        let total_bytes: u64 = jobs.iter().map(|j| j.stamp.size).sum();
        let tracker = Tracker::new(progress, jobs.len(), total_bytes);
        let outcomes: Vec<Outcome> = jobs
            .par_iter()
            .map(|job| self.hash_one(job, &tracker))
            .collect();
        if outcomes.contains(&Outcome::Cancelled) {
            return Err(ApiError::new(ErrorCode::Cancelled, None));
        }
        tracker.finish();

        for (job, outcome) in jobs.iter().zip(outcomes) {
            if let (Outcome::Hash(md5), Some(row)) = (outcome, report.rows.get_mut(job.row)) {
                row.state = hash_state(&md5, &job.expected);
            }
        }
        Ok(report)
    }

    /// Asks a running `check_savegame_files` to stop; no-op when idle.
    pub fn cancel_hashing(&self) {
        // Only while a run holds the flag: a stale request must not cancel the next run.
        if self.hashing.busy.load(Ordering::SeqCst) {
            self.hashing.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Hashes one zip and caches the result as soon as it is known, so a cancel keeps it.
    fn hash_one(&self, job: &HashJob, tracker: &Tracker<'_>) -> Outcome {
        #[cfg(test)]
        self.hashing.hook_before(&job.path);
        let mut read = 0u64;
        let result = game_md5(
            &job.path,
            &job.salt,
            &mut |n| {
                read += n;
                tracker.add_bytes(n);
            },
            &self.hashing.cancel,
        );
        let outcome = match result {
            Ok(md5) => {
                // A cache that cannot be written only costs a rehash later.
                let _ = self
                    .store
                    .put_hashes(&[((job.path.as_path(), job.stamp, &job.salt), &md5)]);
                tracker.file_done(job.stamp.size.saturating_sub(read));
                Outcome::Hash(md5)
            }
            Err(FileOpError::Cancelled) => Outcome::Cancelled,
            Err(_) => {
                tracker.file_done(job.stamp.size.saturating_sub(read));
                Outcome::Unreadable
            }
        };
        #[cfg(test)]
        self.hashing.hook_after(&job.path);
        outcome
    }

    fn build_report(
        &self,
        savegame_id: &str,
        profile_id: &str,
        ctx: &GameContext,
    ) -> Result<Built, ApiError> {
        self.remember(ctx);
        let profile = self.find_profile(profile_id)?;
        let save = read_save(ctx, savegame_id)?;
        let scan = self.scan_profile(&profile, ctx, &no_progress)?;
        let others = self.other_scans(&profile, ctx)?;

        // One cache query for every zip that could need a hash.
        let mut hashes = self.cached_lookups(&save, &scan)?;

        let mut entries: Vec<(SaveModRow, Option<HashJob>)> = Vec::with_capacity(save.mods.len());
        for (mod_index, saved) in save.mods.iter().enumerate() {
            let key = saved.mod_name.to_lowercase();
            let mut row = SaveModRow {
                mod_name: saved.mod_name.clone(),
                title: saved.title.clone(),
                saved_version: saved.version.clone(),
                required: saved.required,
                state: SaveModState::Missing,
                installed: None,
                found_in: Vec::new(),
            };
            let mut job = None;

            if let Some(dlc) = saved.mod_name.strip_prefix("pdlc_") {
                row.state = if dlc_installed(ctx, dlc) {
                    SaveModState::DlcInstalled
                } else {
                    SaveModState::DlcMissing
                };
            } else {
                row.found_in = others
                    .iter()
                    .filter(|(_, other)| other.names.contains_key(&key))
                    .map(|(id, _)| id.clone())
                    .collect();
                if let Some(record) = scan.names.get(&key).and_then(|&i| scan.records.get(i)) {
                    row.installed = Some(side(record));
                    row.state = if record.entry.version.trim() != saved.version.trim() {
                        SaveModState::VersionDiffers
                    } else {
                        match hashes.remove(&mod_index) {
                            // A folder mod, or a save entry without a hash: nothing to compare.
                            None => SaveModState::Present,
                            Some((_, Some(md5))) => hash_state(&md5, &saved.file_hash),
                            Some((lookup, None)) => {
                                job = Some(HashJob {
                                    row: 0,
                                    path: lookup.path,
                                    stamp: lookup.stamp,
                                    salt: lookup.salt,
                                    expected: saved.file_hash.clone(),
                                });
                                SaveModState::HashPending
                            }
                        }
                    };
                }
            }
            entries.push((row, job));
        }

        // Required first, then by name; stable, so equal names keep the save's order.
        entries.sort_by_cached_key(|(row, _)| (!row.required, row.mod_name.to_lowercase()));
        let mut rows = Vec::with_capacity(entries.len());
        let mut jobs = Vec::new();
        for (position, (row, job)) in entries.into_iter().enumerate() {
            rows.push(row);
            if let Some(mut job) = job {
                job.row = position;
                jobs.push(job);
            }
        }

        let used: HashSet<String> = save
            .mods
            .iter()
            .map(|m| m.mod_name.to_lowercase())
            .collect();
        let mut extras: Vec<_> = scan
            .records
            .iter()
            .filter(|r| !used.contains(&r.entry.mod_name.to_lowercase()))
            .collect();
        extras.sort_by_cached_key(|r| (r.entry.mod_name.to_lowercase(), r.entry.file_name.clone()));

        Ok(Built {
            report: SavegameReport {
                savegame_id: savegame_id.to_string(),
                profile_id: profile.id,
                rows,
                extras: extras.into_iter().map(|r| r.entry.id.clone()).collect(),
            },
            jobs,
        })
    }

    /// For each saved mod that may need a hash (a zip in the profile at the save's version, with
    /// a `fileHash` in the save; never a DLC), what to hash and what the cache already holds,
    /// by index into the save's mods.
    fn cached_lookups(
        &self,
        save: &SavegameFile,
        scan: &Scan,
    ) -> Result<HashMap<usize, (Lookup, Option<String>)>, ApiError> {
        let lookups: Vec<(usize, Lookup)> = save
            .mods
            .iter()
            .enumerate()
            .filter(|(_, saved)| {
                !saved.file_hash.is_empty() && !saved.mod_name.starts_with("pdlc_")
            })
            .filter_map(|(mod_index, saved)| {
                let index = *scan.names.get(&saved.mod_name.to_lowercase())?;
                let record = scan.records.get(index)?;
                if record.entry.format != ModFormat::Zip
                    || record.entry.version.trim() != saved.version.trim()
                {
                    return None;
                }
                let lookup = Lookup {
                    path: record.path.clone(),
                    stamp: *scan.stamps.get(index)?,
                    salt: record.entry.mod_name.clone(),
                };
                Some((mod_index, lookup))
            })
            .collect();
        let queries: Vec<_> = lookups
            .iter()
            .map(|(_, l)| (l.path.as_path(), l.stamp, l.salt.as_str()))
            .collect();
        let cached = self.store.cached_hashes(&queries)?;
        Ok(lookups
            .into_iter()
            .zip(cached)
            .map(|((mod_index, lookup), md5)| (mod_index, (lookup, md5)))
            .collect())
    }

    /// The other profiles' last scans, in registry order, scanning a profile that has none. A
    /// folder that cannot be scanned simply holds no mods.
    fn other_scans(
        &self,
        profile: &StoredProfile,
        ctx: &GameContext,
    ) -> Result<Vec<(String, Arc<Scan>)>, ApiError> {
        Ok(self
            .store
            .profiles()?
            .into_iter()
            .filter(|p| p.id != profile.id)
            .filter_map(|p| {
                let scan = self
                    .last_scan(&p.id)
                    .or_else(|| self.scan_profile(&p, ctx, &no_progress).ok())?;
                Some((p.id, scan))
            })
            .collect())
    }
}

/// `Present` when the zip's hash is the save's (hex compared ignoring case), else `FileDiffers`.
fn hash_state(md5: &str, expected: &str) -> SaveModState {
    if md5.eq_ignore_ascii_case(expected) {
        SaveModState::Present
    } else {
        SaveModState::FileDiffers
    }
}

fn read_save(ctx: &GameContext, savegame_id: &str) -> Result<SavegameFile, ApiError> {
    let user_dir = ctx
        .user_dir
        .as_deref()
        .ok_or_else(|| ApiError::new(ErrorCode::SavegameMissing, Some("noUserDir".to_string())))?;
    savegame::read(user_dir, savegame_id).map_err(|e| match e {
        SavegameError::Missing => ApiError::new(ErrorCode::SavegameMissing, None),
        SavegameError::Unreadable(detail) => {
            ApiError::new(ErrorCode::SavegameUnreadable, Some(detail))
        }
    })
}

/// Whether `<install_dir>/pdlc/<name>.dlc` exists. The name comes from a save file, so anything
/// that could leave the `pdlc` folder is treated as not installed.
fn dlc_installed(ctx: &GameContext, name: &str) -> bool {
    let plain = !name.is_empty()
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    plain
        && ctx
            .install_dir
            .as_deref()
            .is_some_and(|dir| dir.join("pdlc").join(format!("{name}.dlc")).is_file())
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Hash(String),
    /// The file could not be read; its row stays `HashPending`.
    Unreadable,
    Cancelled,
}

/// Throttled, monotonic progress for one hashing run, fed from many threads. Counters are
/// atomics; an event is built and sent while holding `last`, so events reach the callback in
/// order and carry non-decreasing counts. Workers only *try* to take the lock and move on, so a
/// slow callback never stalls the hashing.
struct Tracker<'a> {
    sink: &'a (dyn Fn(HashProgress) + Sync),
    total_files: u32,
    total_bytes: u64,
    done_files: AtomicU64,
    done_bytes: AtomicU64,
    last: Mutex<Instant>,
}

impl<'a> Tracker<'a> {
    /// Sends the opening event (nothing done).
    fn new(sink: &'a (dyn Fn(HashProgress) + Sync), files: usize, total_bytes: u64) -> Self {
        let tracker = Self {
            sink,
            total_files: u32::try_from(files).unwrap_or(u32::MAX),
            total_bytes,
            done_files: AtomicU64::new(0),
            done_bytes: AtomicU64::new(0),
            last: Mutex::new(Instant::now()),
        };
        tracker.send();
        tracker
    }

    fn send(&self) {
        (self.sink)(HashProgress {
            done_files: u32::try_from(self.done_files.load(Ordering::SeqCst))
                .unwrap_or(u32::MAX)
                .min(self.total_files),
            total_files: self.total_files,
            done_bytes: self.done_bytes.load(Ordering::SeqCst).min(self.total_bytes),
            total_bytes: self.total_bytes,
        });
    }

    fn tick(&self) {
        let Ok(mut last) = self.last.try_lock() else {
            return;
        };
        if last.elapsed() >= PROGRESS_INTERVAL {
            self.send();
            *last = Instant::now();
        }
    }

    fn add_bytes(&self, n: u64) {
        self.done_bytes.fetch_add(n, Ordering::SeqCst);
        self.tick();
    }

    /// A file is over, however it ended; `unread` is what of its bytes will never be read.
    fn file_done(&self, unread: u64) {
        self.done_bytes.fetch_add(unread, Ordering::SeqCst);
        self.done_files.fetch_add(1, Ordering::SeqCst);
        self.tick();
    }

    /// Sends the closing event.
    fn finish(&self) {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        self.send();
        *last = Instant::now();
    }
}

#[cfg(test)]
impl HashState {
    fn hook_before(&self, path: &std::path::Path) {
        let hook = self
            .hooks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .before
            .clone();
        if let Some(hook) = hook {
            hook(path);
        }
    }

    fn hook_after(&self, path: &std::path::Path) {
        let hook = self
            .hooks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .after
            .clone();
        if let Some(hook) = hook {
            hook(path);
        }
    }
}
