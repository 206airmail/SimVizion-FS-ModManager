//! Plan M8: the Health report across every folder, the game folder the user chose, and the
//! app's own update settings (the updater itself runs in the Tauri layer).
//!
//! **Settings** live in `library.sqlite` (`app_settings`): `game.userDir` (the chosen user
//! folder), `appUpdates.enabled` (`true`/`false`, off by default), `appUpdates.lastCheck`
//! (RFC 3339; every successful check, for display and the 24-hour rule) and `appUpdates.pending`
//! (the version a check found that has not been installed yet, so it is offered again at the
//! next start without waiting a day).

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime};

use super::transfer::BusyGuard;
use super::updates::{iso, parse_iso};
use super::{no_progress, Library};
use crate::discovery::{valid_game_folder, GameContext};
use crate::model::{
    ApiError, AppUpdateSettings, ErrorCode, HealthCount, HealthReport, HealthRow, IssueKind,
};

/// The app checks for its own updates at most this often without "Check now".
pub const APP_UPDATE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

const KEY_GAME_DIR: &str = "game.userDir";
const KEY_ENABLED: &str = "appUpdates.enabled";
const KEY_LAST_CHECK: &str = "appUpdates.lastCheck";
const KEY_PENDING: &str = "appUpdates.pending";

/// Whether the app should ask the release feed now: enabled, and forced, or an update that was
/// found is still waiting (`pending`), or the last check is at least [`APP_UPDATE_INTERVAL`] old
/// (or never happened, or lies in the future).
pub fn app_update_due(
    settings: &AppUpdateSettings,
    pending: bool,
    now: SystemTime,
    force: bool,
) -> bool {
    if !settings.enabled {
        return false;
    }
    if force || pending {
        return true;
    }
    match settings.last_check.as_deref().and_then(parse_iso) {
        None => true,
        // `Err` means the last check lies in the future (the clock was set back).
        Some(last) => now
            .duration_since(last)
            .map_or(true, |age| age >= APP_UPDATE_INTERVAL),
    }
}

/// The position of a kind in [`IssueKind`]'s declaration order. Exhaustive on purpose: a new
/// kind does not compile until it is placed here.
fn kind_order(kind: IssueKind) -> u8 {
    match kind {
        IssueKind::NotAMod => 0,
        IssueKind::InvalidFileName => 1,
        IssueKind::NestedArchive => 2,
        IssueKind::ZipAndFolderTwin => 3,
        IssueKind::ConflictCopy => 4,
        IssueKind::DescVersionMismatch => 5,
        IssueKind::MissingDependency => 6,
    }
}

impl Library {
    /// Every mod with at least one issue in every registered profile, from the profiles' scans
    /// (cache-assisted, as `list_mods`), ordered by profile then title; plus per-kind counts of
    /// mods. The counts agree with each profile's `issueCount` summed over profiles.
    pub fn health_report(&self, ctx: &GameContext) -> Result<HealthReport, ApiError> {
        self.remember(ctx);
        let mut rows = Vec::new();
        for profile in self.store.profiles()? {
            let scan = match self.scan_profile(&profile, ctx, &no_progress) {
                Ok(scan) => scan,
                // A folder that no longer exists holds no mods; it does not fail the report.
                Err(e) if e.code == ErrorCode::FolderMissing => continue,
                Err(e) => return Err(e),
            };
            let mut found: Vec<HealthRow> = scan
                .records
                .iter()
                .filter(|r| !r.entry.issues.is_empty())
                .map(|r| HealthRow {
                    profile_id: profile.id.clone(),
                    profile_name: profile.name.clone(),
                    mod_id: r.entry.id.clone(),
                    mod_name: r.entry.mod_name.clone(),
                    title: r.entry.title.clone(),
                    issues: r.entry.issues.clone(),
                })
                .collect();
            found.sort_by_cached_key(|row| (row.title.to_lowercase(), row.mod_id.clone()));
            rows.extend(found);
        }

        let mut counts: Vec<(u8, HealthCount)> = Vec::new();
        for row in &rows {
            let mut kinds: Vec<IssueKind> = row.issues.iter().map(|i| i.kind).collect();
            kinds.sort_by_key(|k| kind_order(*k));
            kinds.dedup();
            for kind in kinds {
                match counts.iter_mut().find(|(_, c)| c.kind == kind) {
                    Some((_, c)) => c.count += 1,
                    None => counts.push((kind_order(kind), HealthCount { kind, count: 1 })),
                }
            }
        }
        counts.sort_by_key(|(order, _)| *order);
        Ok(HealthReport {
            rows,
            counts: counts.into_iter().map(|(_, c)| c).collect(),
        })
    }

    /// The user folder the user chose with "Choose game folder", if any. It overrides detection,
    /// and it stays the choice while the folder is unavailable (a disconnected drive): it is
    /// validated when it is set, not when it is read, so a write never silently goes to the
    /// detected folder's `gameSettings.xml` instead of the one the user picked.
    pub fn chosen_game_folder(&self) -> Result<Option<PathBuf>, ApiError> {
        Ok(self.store.app_setting(KEY_GAME_DIR)?.map(PathBuf::from))
    }

    /// Stores (`Some`) or forgets (`None`) the chosen user folder. `InvalidGameFolder` unless
    /// [`crate::discovery::valid_game_folder`] accepts it.
    pub fn set_chosen_game_folder(&self, dir: Option<&Path>) -> Result<(), ApiError> {
        match dir {
            Some(dir) => {
                if !valid_game_folder(dir) {
                    return Err(ApiError::new(
                        ErrorCode::InvalidGameFolder,
                        Some(dir.to_string_lossy().into_owned()),
                    ));
                }
                self.store
                    .set_app_setting(KEY_GAME_DIR, Some(&dir.to_string_lossy()))?;
            }
            None => self.store.set_app_setting(KEY_GAME_DIR, None)?,
        }
        Ok(())
    }

    /// The app-update settings (off by default).
    pub fn app_update_settings(&self) -> Result<AppUpdateSettings, ApiError> {
        Ok(AppUpdateSettings {
            enabled: self.store.app_setting(KEY_ENABLED)?.as_deref() == Some("true"),
            last_check: self.store.app_setting(KEY_LAST_CHECK)?,
        })
    }

    /// Switches app-update checks on or off. Switching them off also forgets an update that was
    /// found: nothing is offered or installed while the user has them off.
    pub fn set_app_updates_enabled(&self, enabled: bool) -> Result<AppUpdateSettings, ApiError> {
        self.store
            .set_app_setting(KEY_ENABLED, Some(if enabled { "true" } else { "false" }))?;
        if !enabled {
            self.store.set_app_setting(KEY_PENDING, None)?;
        }
        self.app_update_settings()
    }

    /// Records a successful app-update check at `at`, whatever it found (the time is for display
    /// and for the 24-hour rule). `found` is the version the release feed offered: it is kept as
    /// the pending update, so the next start asks again at once, until a later check finds
    /// nothing or the app runs that version (`running_version`).
    pub fn note_app_update_check(
        &self,
        at: SystemTime,
        found: Option<&str>,
        running_version: &str,
    ) -> Result<(), ApiError> {
        self.store.set_app_setting(KEY_LAST_CHECK, Some(&iso(at)))?;
        let pending = found.filter(|v| !same_version(v, running_version));
        self.store.set_app_setting(KEY_PENDING, pending)?;
        Ok(())
    }

    /// The version of an update that was found and not installed yet; a stored one that is the
    /// version the app runs now (`running_version`: it was installed) is forgotten here.
    pub fn app_update_pending(&self, running_version: &str) -> Result<Option<String>, ApiError> {
        let Some(version) = self.store.app_setting(KEY_PENDING)? else {
            return Ok(None);
        };
        if same_version(&version, running_version) {
            self.store.set_app_setting(KEY_PENDING, None)?;
            return Ok(None);
        }
        Ok(Some(version))
    }

    /// A file operation, a mod update or check, or savegame hashing is running: the app must not
    /// exit now.
    pub fn operation_running(&self) -> bool {
        self.file_operation_running() || self.hashing.busy.load(Ordering::SeqCst)
    }

    /// A file operation (copy, move, remove, mod update) or a mod-update check is running.
    pub fn file_operation_running(&self) -> bool {
        self.transfer.busy.load(Ordering::SeqCst) || self.updates.busy.load(Ordering::SeqCst)
    }

    /// Claims the flags of file operations and mod updates for the app's own update, which ends
    /// with the app exiting: while it is held, any of those answers `Busy`. `Busy` when one is
    /// running now.
    pub fn begin_app_update(&self) -> Result<AppUpdateHold<'_>, ApiError> {
        Ok(AppUpdateHold {
            _files: BusyGuard::acquire(&self.transfer.busy)?,
            _updates: BusyGuard::acquire(&self.updates.busy)?,
        })
    }
}

/// Keeps file operations and mod updates out until dropped; see [`Library::begin_app_update`].
pub struct AppUpdateHold<'a> {
    _files: BusyGuard<'a>,
    _updates: BusyGuard<'a>,
}

/// The same version, ignoring case, spaces and a leading `v`. (Not `net::is_newer`: it reads the
/// numbers of `0.9.0-beta.1` and cannot tell a release from its own pre-releases.)
fn same_version(a: &str, b: &str) -> bool {
    let plain = |v: &str| v.trim().trim_start_matches(['v', 'V']).to_ascii_lowercase();
    plain(a) == plain(b)
}

#[cfg(test)]
mod tests;
