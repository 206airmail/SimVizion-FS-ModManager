//! Writes to game files. Today exactly one: the `modsDirectoryOverride` element of
//! `gameSettings.xml` (plan M4).
//!
//! Every write goes preflight → read → patch in memory → backup → guarded atomic replace →
//! verify. `gameSettings.xml` holds plain-text multiplayer passwords: no error, log line or
//! return value may contain any of its text other than the override's own values.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::discovery::{ModsOverride, Running};
use crate::model::{ApiError, ErrorCode};

/// How many `gameSettings.xml` backups are kept in the backups folder.
pub const KEEP_BACKUPS: usize = 20;

#[derive(Debug)]
pub enum OpError {
    GameRunning,
    ServerRunning,
    SettingsMissing,
    /// Not UTF-8, not well-formed, or the override element cannot be located. Position only.
    SettingsUnreadable,
    /// The file changed between the read and the write; nothing was written.
    SettingsChanged,
    /// Backup, temp file, replace or verification failed; the original is in place.
    /// The detail is an io error kind or a step name, never file text.
    WriteFailed(String),
    NoBackup,
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpError::GameRunning => write!(f, "the game is running"),
            OpError::ServerRunning => write!(f, "the dedicated server is running"),
            OpError::SettingsMissing => write!(f, "gameSettings.xml does not exist"),
            OpError::SettingsUnreadable => write!(f, "gameSettings.xml could not be read"),
            OpError::SettingsChanged => write!(f, "gameSettings.xml changed during the write"),
            OpError::WriteFailed(d) => write!(f, "writing gameSettings.xml failed: {d}"),
            OpError::NoBackup => write!(f, "no backup of gameSettings.xml exists"),
        }
    }
}

impl std::error::Error for OpError {}

impl From<OpError> for ApiError {
    fn from(e: OpError) -> Self {
        let code = match &e {
            OpError::GameRunning => ErrorCode::GameRunning,
            OpError::ServerRunning => ErrorCode::ServerRunning,
            OpError::SettingsMissing => ErrorCode::SettingsMissing,
            OpError::SettingsUnreadable => ErrorCode::SettingsUnreadable,
            OpError::SettingsChanged => ErrorCode::SettingsChanged,
            OpError::WriteFailed(_) => ErrorCode::WriteFailed,
            OpError::NoBackup => ErrorCode::NoBackup,
        };
        let detail = match e {
            OpError::WriteFailed(d) => Some(d),
            _ => None,
        };
        ApiError::new(code, detail)
    }
}

/// Refuses while the game or the dedicated server runs.
pub fn check_not_running(running: &dyn Fn() -> Running) -> Result<(), OpError> {
    let now = running();
    if now.game {
        Err(OpError::GameRunning)
    } else if now.server {
        Err(OpError::ServerRunning)
    } else {
        Ok(())
    }
}

/// Returns `bytes` with the override set to `target`, changing only the attribute values
/// (plan M4 decision 1). A missing attribute or element is written in canonical form.
/// Fails with `SettingsUnreadable` for non-UTF-8 or malformed input.
pub fn patch_override(_bytes: &[u8], _target: &ModsOverride) -> Result<Vec<u8>, OpError> {
    todo!("WS-A")
}

/// The override currently in `bytes` (a missing element reads as inactive).
pub fn read_override(_bytes: &[u8]) -> Result<ModsOverride, OpError> {
    todo!("WS-A")
}

/// Copies `bytes` (the current file) into `dir` as `gameSettings-<UTC timestamp>.xml`, prunes to
/// `keep` newest, and returns the backup's file name.
pub fn backup(_bytes: &[u8], _dir: &Path, _keep: usize) -> Result<String, OpError> {
    todo!("WS-A")
}

/// The newest backup in `dir` and its time, if any.
pub fn latest_backup(_dir: &Path) -> Option<(PathBuf, SystemTime)> {
    todo!("WS-A")
}

/// Replaces `path` with `new_bytes` atomically (temp file in the same folder, flush, rename),
/// but only if the file still holds `expected` and `running` reports neither the game nor the
/// server, both checked right before the rename. Re-reads and verifies afterwards; on a
/// verification failure, puts `expected` back. Never leaves the temp file behind.
pub fn write_settings(
    _path: &Path,
    _new_bytes: &[u8],
    _expected: &[u8],
    _running: &dyn Fn() -> Running,
) -> Result<(), OpError> {
    todo!("WS-A")
}
