//! Copy, move and remove mods between profiles, and profile rename / remove-from-list (plan M5).
//! The file work is `crate::files`; this layer resolves ids, finds clashes, applies the user's
//! choices, guards every item, journals it and reports per-item results.
//!
//! Locking: a running operation holds no lock while it copies or recycles. The only locks are
//! the plan map (a lookup, an insert or a removal) and whatever `Library` takes for scans and the
//! journal, so `list_mods`, `thumbnail` and the other reads carry on during a transfer. One
//! operation at a time is enforced by the `busy` flag, released by a guard even if a callback
//! panics. `cancel` is a flag the copy loop polls between chunks; `cancel_operation` only sets
//! it, and each operation clears it when it starts.
//!
//! Journal: `file_ops` rows (see `store`). Every item that reached its per-item checks is one
//! row (`copy`, `move` or `recycle`) with `ok` or the camelCase error code, plus the existing mod
//! a replace sent to the Recycle Bin (`recycle`). Items skipped by the user's choice, and items
//! never attempted because an earlier one stopped the operation, have no row. A journal failure
//! never hides the outcome of the file work.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, UNIX_EPOCH};

use super::switching::code_name;
use super::{no_progress, profile_not_found, Library};
use crate::discovery::{GameContext, Running};
use crate::files::{self, DiskSpace, FileOpError, RecycleError, Recycler};
use crate::model::{
    ApiError, ClashChoice, ErrorCode, ItemFailure, ModEntry, ModSummary, OpProgress, OpResult,
    TransferClash, TransferItem, TransferMode, TransferPlan,
};
use crate::ops;
use crate::store::{path_key, FileStamp, NewFileOp};

#[cfg(test)]
mod tests;

/// A plan can be executed for this long after it was made.
const PLAN_TTL: Duration = Duration::from_secs(10 * 60);
/// Progress events are sent at most this often (the first and the last always are).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);
/// Longest profile display name, in characters.
const MAX_NAME_CHARS: usize = 100;

/// `TargetChanged` detail for a plan that is unknown or older than [`PLAN_TTL`].
pub const PLAN_EXPIRED: &str = "planExpired";
/// `Internal` detail for a profile name that is empty or too long.
pub const INVALID_NAME: &str = "invalidName";

/// What a file operation needs from the outside world; tests pass fakes.
pub struct OpEnv<'a> {
    pub running: &'a dyn Fn() -> Running,
    pub recycler: &'a dyn Recycler,
    pub disk: &'a dyn DiskSpace,
}

/// The busy flag, the cancel flag and the plans of one `Library`.
#[derive(Default)]
pub(super) struct TransferState {
    busy: AtomicBool,
    cancel: AtomicBool,
    next_plan: AtomicU64,
    plans: Mutex<HashMap<String, Plan>>,
}

/// What `plan_transfer` worked out, kept for `execute_transfer`.
#[derive(Clone)]
struct Plan {
    created: Instant,
    from_id: String,
    to_id: String,
    mode: TransferMode,
    to_dir: PathBuf,
    /// A move inside one volume: renames only, no bytes are written.
    renames_only: bool,
    items: Vec<PlanItem>,
}

#[derive(Clone)]
struct PlanItem {
    mod_name: String,
    file_name: String,
    src: PathBuf,
    /// The source as it was when planned; a change fails the item.
    stamp: FileStamp,
    /// The mod with the same name in the target, if any.
    existing: Option<Existing>,
}

#[derive(Clone)]
struct Existing {
    path: PathBuf,
    stamp: FileStamp,
}

/// Holds the `busy` flag until dropped, so a panic in a callback releases it too.
pub(super) struct BusyGuard<'a>(&'a AtomicBool);

impl<'a> BusyGuard<'a> {
    pub(super) fn acquire(flag: &'a AtomicBool) -> Result<Self, ApiError> {
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| Self(flag))
            .map_err(|_| ApiError::new(ErrorCode::Busy, None))
    }
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Throttled, monotonic progress for one operation. Events go out from the working thread, so
/// they reach the callback in order.
struct Tracker<'a> {
    sink: &'a (dyn Fn(OpProgress) + Sync),
    total_bytes: u64,
    total_items: u32,
    done_bytes: u64,
    done_items: u32,
    current: Option<String>,
    last_sent: Option<Instant>,
}

impl<'a> Tracker<'a> {
    fn new(sink: &'a (dyn Fn(OpProgress) + Sync), total_bytes: u64, total_items: usize) -> Self {
        Self {
            sink,
            total_bytes,
            total_items: u32::try_from(total_items).unwrap_or(u32::MAX),
            done_bytes: 0,
            done_items: 0,
            current: None,
            last_sent: None,
        }
    }

    fn send(&mut self) {
        (self.sink)(OpProgress {
            done_bytes: self.done_bytes,
            total_bytes: self.total_bytes,
            current: self.current.clone(),
            done_items: self.done_items,
            total_items: self.total_items,
        });
        self.last_sent = Some(Instant::now());
    }

    fn tick(&mut self) {
        if self
            .last_sent
            .is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL)
        {
            self.send();
        }
    }

    /// The mod being worked on.
    fn begin(&mut self, file_name: &str) {
        self.current = Some(file_name.to_string());
        self.tick();
    }

    fn add_bytes(&mut self, n: u64) {
        self.done_bytes = self.done_bytes.saturating_add(n).min(self.total_bytes);
        self.tick();
    }

    /// An item is over, whatever its outcome: counts it, and brings the byte count up to what
    /// the items so far were planned to write (a failed item's bytes will not come).
    fn end_item(&mut self, settled_bytes: u64) {
        self.done_bytes = self.done_bytes.max(settled_bytes.min(self.total_bytes));
        self.done_items = self.done_items.saturating_add(1);
        self.tick();
    }

    fn finish(&mut self) {
        self.current = None;
        self.send();
    }
}

fn io_api(e: &io::Error) -> ApiError {
    ApiError::new(ErrorCode::Internal, Some(format!("{:?}", e.kind())))
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The drive (`d:`) or network share (`\\server\share`) a path is on, lower-cased; empty for a
/// path without one. Two paths on the same volume can be moved by renaming.
fn volume_of(path: &Path) -> String {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                format!("{}:", char::from(letter).to_ascii_lowercase())
            }
            Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => format!(
                "\\\\{}\\{}",
                server.to_string_lossy().to_lowercase(),
                share.to_string_lossy().to_lowercase()
            ),
            _ => prefix.as_os_str().to_string_lossy().to_lowercase(),
        },
        _ => String::new(),
    }
}

/// The size and newest mtime `scan::enumerate` would record for this mod now, or `None` if it is
/// gone or unreadable.
fn current_stamp(path: &Path) -> Option<FileStamp> {
    let (size, newest) = files::stats_of(path).ok()?;
    let modified = newest
        .or_else(|| fs::metadata(path).ok()?.modified().ok())
        .unwrap_or(UNIX_EPOCH);
    Some(FileStamp { size, modified })
}

fn unchanged(path: &Path, stamp: FileStamp) -> bool {
    current_stamp(path) == Some(stamp)
}

fn running_code(env: &OpEnv<'_>) -> Result<(), ErrorCode> {
    ops::check_not_running(env.running).map_err(|e| ApiError::from(e).code)
}

fn recycle_code(e: &RecycleError) -> ErrorCode {
    match e {
        RecycleError::Unavailable => ErrorCode::RecycleUnavailable,
        RecycleError::Failed(_) => ErrorCode::WriteFailed,
    }
}

fn file_op_code(e: &FileOpError) -> ErrorCode {
    match e {
        FileOpError::Cancelled => ErrorCode::Cancelled,
        FileOpError::Exists => ErrorCode::TargetChanged,
        FileOpError::Verify(_) | FileOpError::Io(_) => ErrorCode::WriteFailed,
        FileOpError::Recycle(r) => recycle_code(r),
    }
}

/// Errors that end the operation: the items after them are failed with the same code.
fn stops_operation(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::Cancelled | ErrorCode::GameRunning | ErrorCode::ServerRunning
    )
}

pub(super) fn summary(entry: &ModEntry) -> ModSummary {
    ModSummary {
        file_name: entry.file_name.clone(),
        version: entry.version.clone(),
        size_bytes: entry.size_bytes,
        modified: entry.modified.clone(),
    }
}

impl Library {
    /// Works out a copy or move without writing anything: the items, the clashes with mods of
    /// the same name in the target (case-insensitive), bytes to write and free space. The plan is
    /// kept for 10 minutes under `plan_id`.
    pub fn plan_transfer(
        &self,
        from_profile_id: &str,
        mod_ids: &[String],
        to_profile_id: &str,
        mode: TransferMode,
        ctx: &GameContext,
        env: &OpEnv<'_>,
    ) -> Result<TransferPlan, ApiError> {
        self.remember(ctx);
        let from = self.find_profile(from_profile_id)?;
        let to = self.find_profile(to_profile_id)?;
        if from.id == to.id {
            return Err(ApiError::new(
                ErrorCode::TargetChanged,
                Some("sameProfile".to_string()),
            ));
        }

        let source = self.scan_profile(&from, ctx, &no_progress)?;
        let mut seen = HashSet::new();
        let mut picked = Vec::new();
        for id in mod_ids {
            if !seen.insert(id.as_str()) {
                continue;
            }
            let index = source
                .by_id
                .get(id)
                .copied()
                .ok_or_else(|| ApiError::new(ErrorCode::FolderMissing, Some(id.clone())))?;
            picked.push(index);
        }

        let target = self.scan_profile(&to, ctx, &no_progress)?;
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, record) in target.records.iter().enumerate() {
            by_name
                .entry(record.entry.mod_name.to_lowercase())
                .or_default()
                .push(i);
        }

        let mut items = Vec::new();
        let mut clashes = Vec::new();
        let mut plan_items = Vec::new();
        for index in picked {
            let (Some(record), Some(stamp)) = (source.records.get(index), source.stamps.get(index))
            else {
                continue;
            };
            let entry = &record.entry;
            let existing = by_name
                .get(&entry.mod_name.to_lowercase())
                .and_then(|candidates| {
                    // The mod with the very same file name is the one in the way.
                    candidates
                        .iter()
                        .find(|&&i| {
                            target.records.get(i).is_some_and(|r| {
                                r.entry.file_name.eq_ignore_ascii_case(&entry.file_name)
                            })
                        })
                        .or_else(|| candidates.first())
                        .copied()
                })
                .and_then(|i| Some((target.records.get(i)?, *target.stamps.get(i)?)));
            if let Some((other, _)) = existing {
                clashes.push(TransferClash {
                    mod_name: entry.mod_name.clone(),
                    incoming: summary(entry),
                    existing: summary(&other.entry),
                });
            }
            items.push(TransferItem {
                mod_id: entry.id.clone(),
                mod_name: entry.mod_name.clone(),
                size_bytes: stamp.size,
            });
            plan_items.push(PlanItem {
                mod_name: entry.mod_name.clone(),
                file_name: entry.file_name.clone(),
                src: record.path.clone(),
                stamp: *stamp,
                existing: existing.map(|(other, stamp)| Existing {
                    path: other.path.clone(),
                    stamp,
                }),
            });
        }

        let renames_only =
            mode == TransferMode::Move && volume_of(&from.path) == volume_of(&to.path);
        let total_bytes = if renames_only {
            0
        } else {
            items.iter().map(|i| i.size_bytes).sum()
        };
        let free_bytes = env.disk.free_bytes(&to.path).map_err(|e| io_api(&e))?;

        let plan_id = format!(
            "t{}",
            self.transfer.next_plan.fetch_add(1, Ordering::SeqCst) + 1
        );
        {
            let mut plans = lock(&self.transfer.plans);
            plans.retain(|_, p| p.created.elapsed() <= PLAN_TTL);
            plans.insert(
                plan_id.clone(),
                Plan {
                    created: Instant::now(),
                    from_id: from.id.clone(),
                    to_id: to.id.clone(),
                    mode,
                    to_dir: to.path.clone(),
                    renames_only,
                    items: plan_items,
                },
            );
        }
        Ok(TransferPlan {
            plan_id,
            from_profile_id: from.id,
            to_profile_id: to.id,
            mode,
            items,
            clashes,
            total_bytes,
            free_bytes,
        })
    }

    /// Carries out a plan with the user's clash choices (a clash without a choice is skipped).
    /// Refuses up front when busy, when the game or server runs, or when the drive lacks space;
    /// re-checks the guards and the item before each item. Per-item failures go into the result.
    ///
    /// A refused call leaves the plan in place (it can be retried until it expires); once items
    /// are being processed the plan is used up.
    pub fn execute_transfer(
        &self,
        plan_id: &str,
        choices: &[ClashChoice],
        ctx: &GameContext,
        env: &OpEnv<'_>,
        progress: &(dyn Fn(OpProgress) + Sync),
    ) -> Result<OpResult, ApiError> {
        self.remember(ctx);
        let plan = self.live_plan(plan_id)?;
        let _busy = BusyGuard::acquire(&self.transfer.busy)?;
        ops::check_not_running(env.running)?;

        let replace: HashMap<String, bool> = choices
            .iter()
            .map(|c| (c.mod_name.to_lowercase(), c.replace))
            .collect();
        let mut result = OpResult::default();
        let mut todo = Vec::new();
        for item in &plan.items {
            let replaced = replace
                .get(&item.mod_name.to_lowercase())
                .copied()
                .unwrap_or(false);
            if item.existing.is_some() && !replaced {
                result.skipped.push(item.mod_name.clone());
            } else {
                todo.push(item);
            }
        }

        let to_write: u64 = if plan.renames_only {
            0
        } else {
            todo.iter().map(|i| i.stamp.size).sum()
        };
        let free = env.disk.free_bytes(&plan.to_dir).map_err(|e| io_api(&e))?;
        if to_write > free {
            return Err(ApiError::new(ErrorCode::NotEnoughSpace, None));
        }

        self.transfer.cancel.store(false, Ordering::SeqCst);
        let mut tracker = Tracker::new(progress, to_write, todo.len());
        tracker.send();
        let kind = match plan.mode {
            TransferMode::Copy => "copy",
            TransferMode::Move => "move",
        };
        let mut stopped: Option<ErrorCode> = None;
        let mut settled = 0u64;
        for item in todo {
            if stopped.is_none() && self.transfer.cancel.load(Ordering::SeqCst) {
                stopped = Some(ErrorCode::Cancelled);
            }
            if let Some(code) = stopped {
                result.failed.push(ItemFailure {
                    mod_name: item.mod_name.clone(),
                    code,
                });
                continue;
            }
            tracker.begin(&item.file_name);
            let target = plan.to_dir.join(&item.file_name);
            let outcome = self.transfer_item(&plan, plan_id, item, env, &mut tracker);
            self.journal_file_op(
                kind,
                Some(plan_id),
                &item.src,
                Some(&target),
                &outcome.map_or_else(code_name, |()| "ok".to_string()),
            );
            match outcome {
                Ok(()) => result.done.push(item.mod_name.clone()),
                Err(code) => {
                    if stops_operation(code) {
                        stopped = Some(code);
                    }
                    result.failed.push(ItemFailure {
                        mod_name: item.mod_name.clone(),
                        code,
                    });
                }
            }
            if !plan.renames_only {
                settled = settled.saturating_add(item.stamp.size);
            }
            tracker.end_item(settled);
        }
        tracker.finish();

        lock(&self.transfer.plans).remove(plan_id);
        self.forget_scan(&plan.from_id);
        self.forget_scan(&plan.to_id);
        Ok(result)
    }

    /// One item of a transfer: guards, the source and existing-mod checks, the recycle for a
    /// replace, then the copy or move.
    fn transfer_item(
        &self,
        plan: &Plan,
        plan_id: &str,
        item: &PlanItem,
        env: &OpEnv<'_>,
        tracker: &mut Tracker<'_>,
    ) -> Result<(), ErrorCode> {
        running_code(env)?;
        if !unchanged(&item.src, item.stamp) {
            return Err(ErrorCode::TargetChanged);
        }
        let cancel = &self.transfer.cancel;
        let mut on_bytes = |n| tracker.add_bytes(n);
        let Some(existing) = &item.existing else {
            return match plan.mode {
                TransferMode::Copy => {
                    files::copy_staged(&item.src, &plan.to_dir, &mut on_bytes, cancel)
                }
                TransferMode::Move => {
                    files::move_item(&item.src, &plan.to_dir, &mut on_bytes, cancel, env.recycler)
                }
            }
            .map(|_| ())
            .map_err(|e| file_op_code(&e));
        };

        if !unchanged(&existing.path, existing.stamp) {
            return Err(ErrorCode::TargetChanged);
        }
        let mut recycle_existing = || {
            let recycled = env.recycler.recycle(&existing.path);
            self.journal_file_op(
                "recycle",
                Some(plan_id),
                &existing.path,
                None,
                &recycled
                    .as_ref()
                    .map_or_else(|e| code_name(recycle_code(e)), |_| "ok".to_string()),
            );
            recycled.map_err(FileOpError::Recycle)
        };
        // The mod being replaced goes to the Recycle Bin only once its replacement is copied and
        // verified, so a cancelled or failed copy leaves it in place. A move on one volume is an
        // instant rename, so there the old mod can go first.
        match plan.mode {
            TransferMode::Move if plan.renames_only => recycle_existing().and_then(|()| {
                files::move_item(&item.src, &plan.to_dir, &mut on_bytes, cancel, env.recycler)
                    .map(|_| ())
            }),
            TransferMode::Copy => files::copy_staged_replacing(
                &item.src,
                &plan.to_dir,
                &mut on_bytes,
                cancel,
                &mut recycle_existing,
            )
            .map(|_| ()),
            // Across drives: as `files::move_item` does, the source goes to the bin once its copy
            // is in place.
            TransferMode::Move => files::copy_staged_replacing(
                &item.src,
                &plan.to_dir,
                &mut on_bytes,
                cancel,
                &mut recycle_existing,
            )
            .and_then(|_| {
                env.recycler
                    .recycle(&item.src)
                    .map_err(FileOpError::Recycle)
            }),
        }
        .map_err(|e| file_op_code(&e))
    }

    /// Sends mods of a profile to the Recycle Bin, item by item.
    pub fn remove_mods(
        &self,
        profile_id: &str,
        mod_ids: &[String],
        ctx: &GameContext,
        env: &OpEnv<'_>,
        progress: &(dyn Fn(OpProgress) + Sync),
    ) -> Result<OpResult, ApiError> {
        self.remember(ctx);
        let profile = self.find_profile(profile_id)?;
        let _busy = BusyGuard::acquire(&self.transfer.busy)?;
        ops::check_not_running(env.running)?;

        // Mods are those of the profile's last scan, the list the user chose from.
        let scan = self.scan_of(&profile.id)?;
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        for id in mod_ids {
            if !seen.insert(id.as_str()) {
                continue;
            }
            let unknown = || ApiError::new(ErrorCode::FolderMissing, Some(id.clone()));
            let index = scan.by_id.get(id).copied().ok_or_else(unknown)?;
            let record = scan.records.get(index).ok_or_else(unknown)?;
            let stamp = *scan.stamps.get(index).ok_or_else(unknown)?;
            items.push((
                record.entry.mod_name.clone(),
                record.entry.file_name.clone(),
                record.path.clone(),
                stamp,
            ));
        }

        self.transfer.cancel.store(false, Ordering::SeqCst);
        let total: u64 = items.iter().map(|(_, _, _, s)| s.size).sum();
        let mut tracker = Tracker::new(progress, total, items.len());
        tracker.send();
        let mut result = OpResult::default();
        let mut stopped: Option<ErrorCode> = None;
        let mut settled = 0u64;
        for (mod_name, file_name, path, stamp) in &items {
            if stopped.is_none() && self.transfer.cancel.load(Ordering::SeqCst) {
                stopped = Some(ErrorCode::Cancelled);
            }
            if let Some(code) = stopped {
                result.failed.push(ItemFailure {
                    mod_name: mod_name.clone(),
                    code,
                });
                continue;
            }
            tracker.begin(file_name);
            let outcome = running_code(env).and_then(|()| {
                if !unchanged(path, *stamp) {
                    return Err(ErrorCode::TargetChanged);
                }
                env.recycler.recycle(path).map_err(|e| recycle_code(&e))
            });
            self.journal_file_op(
                "recycle",
                None,
                path,
                None,
                &outcome.map_or_else(code_name, |()| "ok".to_string()),
            );
            match outcome {
                Ok(()) => result.done.push(mod_name.clone()),
                Err(code) => {
                    if stops_operation(code) {
                        stopped = Some(code);
                    }
                    result.failed.push(ItemFailure {
                        mod_name: mod_name.clone(),
                        code,
                    });
                }
            }
            settled = settled.saturating_add(stamp.size);
            tracker.end_item(settled);
        }
        tracker.finish();

        self.forget_scan(&profile.id);
        Ok(result)
    }

    /// Asks the running file operation to stop after the current chunk; no-op when idle.
    pub fn cancel_operation(&self) {
        self.transfer.cancel.store(true, Ordering::SeqCst);
    }

    /// Changes a profile's display name; the folder is untouched.
    pub fn rename_profile(&self, profile_id: &str, name: &str) -> Result<(), ApiError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
            return Err(ApiError::new(
                ErrorCode::Internal,
                Some(INVALID_NAME.to_string()),
            ));
        }
        if self.store.rename_profile(profile_id, name)? {
            Ok(())
        } else {
            Err(profile_not_found(profile_id))
        }
    }

    /// Removes a profile from the list; the folder is untouched. Refuses the active folder and
    /// the game's default folder (`CannotRemoveProfile`).
    pub fn remove_profile(&self, profile_id: &str, ctx: &GameContext) -> Result<(), ApiError> {
        self.remember(ctx);
        let profile = self.find_profile(profile_id)?;
        let key = path_key(&profile.path);
        let is_folder = |dir: &Option<PathBuf>| dir.as_deref().is_some_and(|d| path_key(d) == key);
        if is_folder(&ctx.effective_mods_dir) || is_folder(&ctx.default_mods_dir) {
            return Err(ApiError::new(ErrorCode::CannotRemoveProfile, None));
        }
        if !self.store.remove_profile(&profile.id)? {
            return Err(profile_not_found(profile_id));
        }
        self.forget_scan(&profile.id);
        Ok(())
    }

    /// The plan, if it exists and has not expired.
    fn live_plan(&self, plan_id: &str) -> Result<Plan, ApiError> {
        let expired = || ApiError::new(ErrorCode::TargetChanged, Some(PLAN_EXPIRED.to_string()));
        let mut plans = lock(&self.transfer.plans);
        let plan = plans.get(plan_id).cloned().ok_or_else(expired)?;
        if plan.created.elapsed() > PLAN_TTL {
            plans.remove(plan_id);
            return Err(expired());
        }
        Ok(plan)
    }

    /// Drops the profile's last scan, so the next listing, `also_in` and mod lookup read the
    /// folder again.
    fn forget_scan(&self, profile_id: &str) {
        self.scans
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(profile_id);
    }

    fn journal_file_op(
        &self,
        kind: &str,
        plan_id: Option<&str>,
        source: &Path,
        target: Option<&Path>,
        result: &str,
    ) {
        let _ = self.store.add_file_op(&NewFileOp {
            kind,
            plan_id,
            source,
            target,
            result,
        });
    }
}
