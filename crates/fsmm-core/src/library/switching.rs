//! Switching the active mod folder, undo and emergency restore (plan M4). The game file write
//! itself is `crate::ops`; this layer resolves profiles, journals each write in
//! `library.sqlite` and returns what the frontend shows.

use std::path::Path;
use std::time::SystemTime;

use super::Library;
use crate::discovery::{GameContext, Running};
use crate::model::{ApiError, SwitchResult};

impl Library {
    /// Makes the profile the folder the game loads: `active="true"` and its folder as
    /// `directory`, or `active="false"` for `ctx.default_mods_dir` (plan M4 decision 2).
    /// Already active → no write, and a result whose `from` and `to` are the same profile.
    pub fn set_active(
        &self,
        _profile_id: &str,
        _ctx: &GameContext,
        _running: &dyn Fn() -> Running,
        _backups_dir: &Path,
    ) -> Result<SwitchResult, ApiError> {
        todo!("WS-A")
    }

    /// Puts back the override that operation `op_id` replaced, if `op_id` is the most recent
    /// journaled write and the file still holds what it wrote (plan M4 decision 6).
    pub fn undo_switch(
        &self,
        _op_id: &str,
        _ctx: &GameContext,
        _running: &dyn Fn() -> Running,
        _backups_dir: &Path,
    ) -> Result<SwitchResult, ApiError> {
        todo!("WS-A")
    }

    /// Time of the newest `gameSettings.xml` backup, if any.
    pub fn latest_backup(&self, _backups_dir: &Path) -> Option<SystemTime> {
        todo!("WS-A")
    }

    /// Replaces `gameSettings.xml` with the newest backup, backing up the current file first.
    pub fn restore_backup(
        &self,
        _ctx: &GameContext,
        _running: &dyn Fn() -> Running,
        _backups_dir: &Path,
    ) -> Result<(), ApiError> {
        todo!("WS-A")
    }
}
