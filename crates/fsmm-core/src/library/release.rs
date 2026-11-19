//! Plan M8: the Health report across every folder, the game folder the user chose, and the
//! app's own update settings (the updater itself runs in the Tauri layer).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::Library;
use crate::discovery::GameContext;
use crate::model::{ApiError, AppUpdateSettings, HealthReport};

/// The app checks for its own updates at most this often without "Check now".
pub const APP_UPDATE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Whether the app should ask the release feed now: enabled, and forced or the last check is
/// at least [`APP_UPDATE_INTERVAL`] old (or never happened, or lies in the future).
pub fn app_update_due(_settings: &AppUpdateSettings, _now: SystemTime, _force: bool) -> bool {
    todo!("WS-A")
}

impl Library {
    /// Every mod with at least one issue in every registered profile, from the profiles' scans
    /// (cache-assisted, as `list_mods`), ordered by profile then title; plus per-kind counts of
    /// mods. The counts agree with each profile's `issueCount` summed over profiles.
    pub fn health_report(&self, _ctx: &GameContext) -> Result<HealthReport, ApiError> {
        todo!("WS-A")
    }

    /// The user folder the user chose with "Choose game folder", if any. It overrides detection.
    pub fn chosen_game_folder(&self) -> Result<Option<PathBuf>, ApiError> {
        todo!("WS-A")
    }

    /// Stores (`Some`) or forgets (`None`) the chosen user folder. `InvalidGameFolder` unless
    /// [`crate::discovery::valid_game_folder`] accepts it.
    pub fn set_chosen_game_folder(&self, _dir: Option<&Path>) -> Result<(), ApiError> {
        todo!("WS-A")
    }

    /// The app-update settings (off by default).
    pub fn app_update_settings(&self) -> Result<AppUpdateSettings, ApiError> {
        todo!("WS-A")
    }

    /// Switches app-update checks on or off.
    pub fn set_app_updates_enabled(&self, _enabled: bool) -> Result<AppUpdateSettings, ApiError> {
        todo!("WS-A")
    }

    /// Records a successful app-update check at `at`.
    pub fn note_app_update_check(&self, _at: SystemTime) -> Result<(), ApiError> {
        todo!("WS-A")
    }
}
