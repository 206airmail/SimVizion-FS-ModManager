//! Plan M8: the Health report across every folder, the game folder the user chose, and the
//! app's own update settings (the updater itself runs in the Tauri layer).
//!
//! **Settings** live in `library.sqlite` (`app_settings`): `game.userDir` (the chosen user
//! folder), `appUpdates.enabled` (`true`/`false`, off by default) and `appUpdates.lastCheck`
//! (RFC 3339).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

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

/// Whether the app should ask the release feed now: enabled, and forced or the last check is
/// at least [`APP_UPDATE_INTERVAL`] old (or never happened, or lies in the future).
pub fn app_update_due(settings: &AppUpdateSettings, now: SystemTime, force: bool) -> bool {
    if !settings.enabled {
        return false;
    }
    if force {
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

    /// The user folder the user chose with "Choose game folder", if any. It overrides detection.
    /// Only while it is still a valid game folder; otherwise `None`, so detection decides again.
    pub fn chosen_game_folder(&self) -> Result<Option<PathBuf>, ApiError> {
        Ok(self
            .store
            .app_setting(KEY_GAME_DIR)?
            .map(PathBuf::from)
            .filter(|dir| valid_game_folder(dir)))
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

    /// Switches app-update checks on or off.
    pub fn set_app_updates_enabled(&self, enabled: bool) -> Result<AppUpdateSettings, ApiError> {
        self.store
            .set_app_setting(KEY_ENABLED, Some(if enabled { "true" } else { "false" }))?;
        self.app_update_settings()
    }

    /// Records a successful app-update check at `at`.
    pub fn note_app_update_check(&self, at: SystemTime) -> Result<(), ApiError> {
        self.store.set_app_setting(KEY_LAST_CHECK, Some(&iso(at)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
