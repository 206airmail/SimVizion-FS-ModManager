//! Savegame comparison (plan M6): a save's mods checked against a profile, with the game's own
//! file hash computed in parallel and cached (decision 2).

use super::Library;
use crate::discovery::GameContext;
use crate::model::{ApiError, HashProgress, Savegame, SavegameReport};

impl Library {
    /// Every save under the game's user folder (empty when there is no user folder).
    pub fn list_savegames(&self, _ctx: &GameContext) -> Result<Vec<Savegame>, ApiError> {
        todo!("WS-A")
    }

    /// The save's mods against the profile, without hashing anything: rows whose hash is in
    /// the cache are final, the others that need one are `HashPending`. `pdlc_*` names are
    /// DLCs, installed when `<install_dir>/pdlc/<name>.dlc` exists. `found_in` lists the other
    /// profiles that have the mod name; `extras` the profile's mods the save does not use.
    pub fn savegame_report(
        &self,
        _savegame_id: &str,
        _profile_id: &str,
        _ctx: &GameContext,
    ) -> Result<SavegameReport, ApiError> {
        todo!("WS-A")
    }

    /// Hashes the zips the report left `HashPending` (in parallel, cached by path, size and
    /// mtime) and returns the final report. Stops early on `cancel_hashing` (`Cancelled`);
    /// hashes finished before that stay cached. One hashing run at a time (`Busy`).
    pub fn check_savegame_files(
        &self,
        _savegame_id: &str,
        _profile_id: &str,
        _ctx: &GameContext,
        _progress: &(dyn Fn(HashProgress) + Sync),
    ) -> Result<SavegameReport, ApiError> {
        todo!("WS-A")
    }

    /// Asks a running `check_savegame_files` to stop; no-op when idle.
    pub fn cancel_hashing(&self) {
        todo!("WS-A")
    }
}
