//! Mod updates (plan M7): checking the SiloAPI catalog, installing GitHub and ModHub updates
//! through the M5 write path, the user's manual source links and the online setting.
//!
//! Nothing here goes online unless `updates.enabled` is set; every request goes through
//! `UpdateEnv::catalog` / `UpdateEnv::web` (`crate::net`).

use std::time::SystemTime;

use super::{Library, OpEnv};
use crate::discovery::GameContext;
use crate::model::{
    ApiError, LinkKind, OpProgress, OpResult, UpdateCheck, UpdatePlan, UpdateSettings,
};
use crate::net::{Catalog, Web};

/// What updates need from the outside world; tests pass fakes and a fixed clock.
pub struct UpdateEnv<'a> {
    pub catalog: &'a dyn Catalog,
    pub web: &'a dyn Web,
    /// The M5 file-operation environment (running check, Recycle Bin, free space).
    pub files: &'a OpEnv<'a>,
    pub now: &'a dyn Fn() -> SystemTime,
}

impl Library {
    /// The online-update settings (off by default).
    pub fn update_settings(&self) -> Result<UpdateSettings, ApiError> {
        todo!("WS-A")
    }

    /// Switches online checks on or off. Off forgets nothing but sends nothing.
    pub fn set_updates_enabled(&self, _enabled: bool) -> Result<UpdateSettings, ApiError> {
        todo!("WS-A")
    }

    /// Checks every registered profile's mods against the catalog (plan M7 outcome 2–3).
    ///
    /// Refused with `UpdatesDisabled` when off. Without `force`, cached results less than 3 hours
    /// old are returned (`from_cache`); with `force`, a check less than 60 s old is too. Before
    /// a stored `retry_after`, nothing is sent and `RateLimited` is returned. Otherwise: distinct
    /// mod names (case-insensitive) go to `lookup` in batches; zips not matched by name are
    /// hashed (SHA-256, cached) and go to `by-hash`; results are stored per lower-cased name,
    /// replacing the previous ones only when the whole check succeeded. A 429 stores
    /// `retry_after` and fails the check; nothing is retried.
    pub fn check_updates(
        &self,
        _force: bool,
        _ctx: &GameContext,
        _env: &UpdateEnv<'_>,
    ) -> Result<UpdateCheck, ApiError> {
        todo!("WS-A")
    }

    /// The folders holding `mod_name` and where its update would come from, without any
    /// network request. `NotInstallable` when the cached match has no direct download.
    pub fn plan_update(&self, _mod_name: &str, _ctx: &GameContext) -> Result<UpdatePlan, ApiError> {
        todo!("WS-A")
    }

    /// Downloads, verifies and installs the update of `mod_name` into each of `profile_ids`
    /// (plan M7 outcome 5): resolves the download (GitHub `downloadUrl` from `detail`, or the
    /// ModHub page's download link), refuses an asset with another file name (`NotInstallable`),
    /// streams it into the first folder via `files::install_download` with the M5 busy/cancel
    /// state and progress, recycles the old version just before the rename, then fills the other
    /// folders with `copy_staged_replacing`. Refused while the game or server runs. Each folder
    /// is one journal row (`update`) and one entry in the result.
    pub fn install_update(
        &self,
        _mod_name: &str,
        _profile_ids: &[String],
        _ctx: &GameContext,
        _env: &UpdateEnv<'_>,
        _progress: &(dyn Fn(OpProgress) + Sync),
    ) -> Result<OpResult, ApiError> {
        todo!("WS-A")
    }

    /// The URL behind one of the mod's links: the cached catalog page for `ModHub`/`GitHub`,
    /// the user's link for `Manual`. Only `http`/`https`. `FolderMissing` when there is none.
    pub fn link_url(&self, _mod_name: &str, _kind: LinkKind) -> Result<String, ApiError> {
        todo!("WS-A")
    }

    /// Sets or clears (`None`) the user's source link for a mod name. The URL must be
    /// `http`/`https`, at most 2000 characters, without user info; otherwise `InvalidLink`.
    /// Returns the stored link's host.
    pub fn set_source_link(
        &self,
        _mod_name: &str,
        _url: Option<&str>,
    ) -> Result<Option<String>, ApiError> {
        todo!("WS-A")
    }
}
