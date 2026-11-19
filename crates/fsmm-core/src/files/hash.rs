//! The hash the game records for each mod in a savegame (plan M6 decision 2):
//! `fileHash = md5(zip bytes ++ modName as UTF-8)`, lower-case hex. The engine's
//! `getFileMD5(path, salt)` appends the salt (the mod name, without `.zip`) to the file's bytes.
//! Verified against real savegame entries on 2026-11-19.
//!
//! Read-only: the file is opened for reading and streamed in chunks; nothing is written.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::FileOpError;

/// The game's `fileHash` for a mod zip: MD5 over the file's bytes followed by `mod_name`.
/// `progress` receives bytes read since the last call. Checks `cancel` between chunks.
pub fn game_md5(
    _path: &Path,
    _mod_name: &str,
    _progress: &mut dyn FnMut(u64),
    _cancel: &AtomicBool,
) -> Result<String, FileOpError> {
    todo!("WS-A")
}
