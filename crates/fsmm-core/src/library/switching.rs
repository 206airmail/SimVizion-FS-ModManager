//! Switching the active mod folder, undo and emergency restore (plan M4). The game file write
//! itself is `crate::ops`; this layer resolves profiles, journals each write in
//! `library.sqlite` and returns what the frontend shows.
//!
//! Journal: every successful write, and every failed attempt that got as far as taking a
//! backup, is one row in `ops`. A failed journal insert after a successful write is not an
//! error for the caller (the file did change); the result then carries an empty `op_id`, so
//! the frontend offers no Undo.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::Library;
use crate::discovery::{GameContext, ModsOverride, Running};
use crate::model::{ApiError, ErrorCode, SwitchResult};
use crate::ops::{self, OpError, KEEP_BACKUPS};
use crate::store::{path_key, NewOp, StoredProfile};

const SETTINGS_FILE: &str = "gameSettings.xml";
const KIND_SWITCH: &str = "switch";
const KIND_UNDO: &str = "undo";
const KIND_RESTORE: &str = "restore";

/// `<user folder>\gameSettings.xml`; `SettingsMissing` when there is no user folder or file.
fn settings_path(ctx: &GameContext) -> Result<PathBuf, ApiError> {
    let path = ctx
        .user_dir
        .as_ref()
        .map(|dir| dir.join(SETTINGS_FILE))
        .filter(|p| p.is_file())
        .ok_or_else(|| ApiError::from(OpError::SettingsMissing))?;
    Ok(path)
}

/// Reads the file; only the io error kind is kept if that fails.
fn read_settings(path: &Path) -> Result<Vec<u8>, ApiError> {
    fs::read(path).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            OpError::SettingsMissing.into()
        } else {
            ApiError::new(
                ErrorCode::SettingsUnreadable,
                Some(format!("{:?}", e.kind())),
            )
        }
    })
}

/// The registered profile whose folder is the one the game loads in this override state: the
/// default folder when inactive (or active without a directory), else the directory.
fn profile_for(
    profiles: &[StoredProfile],
    state: &ModsOverride,
    ctx: &GameContext,
) -> Option<String> {
    let folder: PathBuf = match (&state.directory, state.active) {
        (Some(dir), true) => match &ctx.user_dir {
            Some(user) if dir.is_relative() => user.join(dir),
            _ => dir.clone(),
        },
        _ => ctx.default_mods_dir.clone()?,
    };
    let key = path_key(&folder);
    profiles
        .iter()
        .find(|p| path_key(&p.path) == key)
        .map(|p| p.id.clone())
}

/// The camelCase name of an error code, as stored in the journal's `result`.
pub(super) fn code_name(code: ErrorCode) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "internal".to_string())
}

impl Library {
    /// Makes the profile the folder the game loads: `active="true"` and its folder as
    /// `directory`, or `active="false"` for `ctx.default_mods_dir` (plan M4 decision 2).
    /// Already active → no write, and a result whose `from` and `to` are the same profile.
    pub fn set_active(
        &self,
        profile_id: &str,
        ctx: &GameContext,
        running: &dyn Fn() -> Running,
        backups_dir: &Path,
    ) -> Result<SwitchResult, ApiError> {
        let profile = self.find_profile(profile_id)?;
        let settings = settings_path(ctx)?;
        ops::check_not_running(running)?;
        let original = read_settings(&settings)?;
        let current = ops::read_override(&original)?;

        let is_default = ctx
            .default_mods_dir
            .as_deref()
            .is_some_and(|d| path_key(d) == path_key(&profile.path));
        let target = if is_default {
            ModsOverride {
                active: false,
                directory: None,
            }
        } else {
            ModsOverride {
                active: true,
                directory: Some(profile.path.clone()),
            }
        };
        let profiles = self.store.profiles()?;
        let from_profile_id = profile_for(&profiles, &current, ctx);

        let patched = ops::patch_override(&original, &target)?;
        if patched == original {
            return Ok(SwitchResult {
                op_id: self.latest_ok_op_id(),
                to_profile_id: from_profile_id.clone(),
                from_profile_id,
            });
        }
        let (op_id, to) = self.commit(
            KIND_SWITCH,
            &settings,
            &original,
            &patched,
            running,
            backups_dir,
        )?;
        Ok(SwitchResult {
            op_id,
            from_profile_id,
            to_profile_id: profile_for(&profiles, &to, ctx),
        })
    }

    /// Puts back the override that operation `op_id` replaced, if `op_id` is the most recent
    /// journaled write and the file still holds what it wrote (plan M4 decision 6).
    pub fn undo_switch(
        &self,
        op_id: &str,
        ctx: &GameContext,
        running: &dyn Fn() -> Running,
        backups_dir: &Path,
    ) -> Result<SwitchResult, ApiError> {
        let nothing = || ApiError::new(ErrorCode::NothingToUndo, Some(op_id.to_string()));
        let op = self
            .store
            .latest_ok_op()?
            .filter(|op| op.id == op_id && (op.kind == KIND_SWITCH || op.kind == KIND_UNDO))
            .ok_or_else(nothing)?;

        let settings = settings_path(ctx)?;
        ops::check_not_running(running)?;
        let original = read_settings(&settings)?;
        let current = ops::read_override(&original)?;
        if current != op.to {
            return Err(OpError::SettingsChanged.into());
        }

        let profiles = self.store.profiles()?;
        let from_profile_id = profile_for(&profiles, &current, ctx);
        let patched = ops::patch_override(&original, &op.from)?;
        if patched == original {
            return Ok(SwitchResult {
                op_id: self.latest_ok_op_id(),
                to_profile_id: from_profile_id.clone(),
                from_profile_id,
            });
        }
        let (new_id, to) = self.commit(
            KIND_UNDO,
            &settings,
            &original,
            &patched,
            running,
            backups_dir,
        )?;
        Ok(SwitchResult {
            op_id: new_id,
            from_profile_id,
            to_profile_id: profile_for(&profiles, &to, ctx),
        })
    }

    /// Time of the newest `gameSettings.xml` backup, if any.
    pub fn latest_backup(&self, backups_dir: &Path) -> Option<SystemTime> {
        ops::latest_backup(backups_dir).map(|(_, time)| time)
    }

    /// Replaces `gameSettings.xml` with the newest backup, backing up the current file first.
    pub fn restore_backup(
        &self,
        ctx: &GameContext,
        running: &dyn Fn() -> Running,
        backups_dir: &Path,
    ) -> Result<(), ApiError> {
        ops::check_not_running(running)?;
        // The source is chosen before the safety backup, which would otherwise be the newest.
        let (source, _) = ops::latest_backup(backups_dir).ok_or(OpError::NoBackup)?;
        let settings = settings_path(ctx)?;
        let restored = fs::read(&source)
            .map_err(|e| ApiError::new(ErrorCode::WriteFailed, Some(format!("{:?}", e.kind()))))?;
        let current = read_settings(&settings)?;

        // Whatever the current file holds, it is kept first; it may be what needs rescuing.
        let from = ops::read_override(&current).unwrap_or_default();
        let to = ops::read_override(&restored).unwrap_or_default();
        let backup_file = ops::backup(&current, backups_dir, KEEP_BACKUPS)?;
        match ops::write_settings(&settings, &restored, &current, running) {
            Ok(()) => {
                self.journal(KIND_RESTORE, &from, &to, &backup_file, "ok");
                Ok(())
            }
            Err(e) => {
                let e = ApiError::from(e);
                self.journal(KIND_RESTORE, &from, &to, &backup_file, &code_name(e.code));
                Err(e)
            }
        }
    }

    /// Backup → guarded write → journal, for a patch of `original`. Returns the journal id
    /// (empty if the journal could not be written) and the override state that was written.
    fn commit(
        &self,
        kind: &str,
        settings: &Path,
        original: &[u8],
        patched: &[u8],
        running: &dyn Fn() -> Running,
        backups_dir: &Path,
    ) -> Result<(String, ModsOverride), ApiError> {
        let from = ops::read_override(original)?;
        let to = ops::read_override(patched)?;
        let backup_file = ops::backup(original, backups_dir, KEEP_BACKUPS)?;
        match ops::write_settings(settings, patched, original, running) {
            Ok(()) => {
                let id = self.journal(kind, &from, &to, &backup_file, "ok");
                Ok((id, to))
            }
            Err(e) => {
                let e = ApiError::from(e);
                self.journal(kind, &from, &to, &backup_file, &code_name(e.code));
                Err(e)
            }
        }
    }

    /// Appends a journal row. A journal failure never hides the outcome of the write, so it
    /// yields an empty id instead of an error.
    fn journal(
        &self,
        kind: &str,
        from: &ModsOverride,
        to: &ModsOverride,
        backup_file: &str,
        result: &str,
    ) -> String {
        self.store
            .add_op(&NewOp {
                kind,
                from,
                to,
                backup_file: Some(backup_file),
                result,
            })
            .unwrap_or_default()
    }

    /// The id of the newest journaled write that succeeded, or empty.
    fn latest_ok_op_id(&self) -> String {
        self.store
            .latest_ok_op()
            .ok()
            .flatten()
            .map(|op| op.id)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
