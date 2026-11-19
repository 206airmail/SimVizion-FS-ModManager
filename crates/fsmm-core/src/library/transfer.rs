//! Copy, move and remove mods between profiles, and profile rename / remove-from-list (plan M5).
//! The file work is `crate::files`; this layer resolves ids, finds clashes, applies the user's
//! choices, guards every item, journals it and reports per-item results.

use super::Library;
use crate::discovery::{GameContext, Running};
use crate::files::{DiskSpace, Recycler};
use crate::model::{ApiError, ClashChoice, OpProgress, OpResult, TransferMode, TransferPlan};

/// What a file operation needs from the outside world; tests pass fakes.
pub struct OpEnv<'a> {
    pub running: &'a dyn Fn() -> Running,
    pub recycler: &'a dyn Recycler,
    pub disk: &'a dyn DiskSpace,
}

impl Library {
    /// Works out a copy or move without writing anything: the items, the clashes with mods of
    /// the same name in the target (case-insensitive), bytes to write and free space. The plan is
    /// kept for 10 minutes under `plan_id`.
    pub fn plan_transfer(
        &self,
        _from_profile_id: &str,
        _mod_ids: &[String],
        _to_profile_id: &str,
        _mode: TransferMode,
        _ctx: &GameContext,
        _env: &OpEnv<'_>,
    ) -> Result<TransferPlan, ApiError> {
        todo!("WS-A")
    }

    /// Carries out a plan with the user's clash choices (a clash without a choice is skipped).
    /// Refuses up front when busy, when the game or server runs, or when the drive lacks space;
    /// re-checks the guards and the item before each item. Per-item failures go into the result.
    pub fn execute_transfer(
        &self,
        _plan_id: &str,
        _choices: &[ClashChoice],
        _ctx: &GameContext,
        _env: &OpEnv<'_>,
        _progress: &(dyn Fn(OpProgress) + Sync),
    ) -> Result<OpResult, ApiError> {
        todo!("WS-A")
    }

    /// Sends mods of a profile to the Recycle Bin, item by item.
    pub fn remove_mods(
        &self,
        _profile_id: &str,
        _mod_ids: &[String],
        _ctx: &GameContext,
        _env: &OpEnv<'_>,
        _progress: &(dyn Fn(OpProgress) + Sync),
    ) -> Result<OpResult, ApiError> {
        todo!("WS-A")
    }

    /// Asks the running file operation to stop after the current chunk; no-op when idle.
    pub fn cancel_operation(&self) {
        todo!("WS-A")
    }

    /// Changes a profile's display name; the folder is untouched.
    pub fn rename_profile(&self, _profile_id: &str, _name: &str) -> Result<(), ApiError> {
        todo!("WS-A")
    }

    /// Removes a profile from the list; the folder is untouched. Refuses the active folder and
    /// the game's default folder (`CannotRemoveProfile`).
    pub fn remove_profile(&self, _profile_id: &str, _ctx: &GameContext) -> Result<(), ApiError> {
        todo!("WS-A")
    }
}
