//! The hash the game records for each mod in a savegame (plan M6 decision 2):
//! `fileHash = md5(zip bytes ++ modName as UTF-8)`, lower-case hex. The engine's
//! `getFileMD5(path, salt)` appends the salt (the mod name, without `.zip`) to the file's bytes.
//! Verified against real savegame entries on 2026-11-19.
//!
//! Read-only: the file is opened for reading and streamed in chunks; nothing is written.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use md5::{Digest, Md5};
use sha2::Sha256;

use super::{io_err, FileOpError, CHUNK};

#[cfg(test)]
mod tests;

/// The game's `fileHash` for a mod zip: MD5 over the file's bytes followed by `mod_name`.
/// `progress` receives bytes read since the last call. Checks `cancel` between chunks.
pub fn game_md5(
    path: &Path,
    mod_name: &str,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> Result<String, FileOpError> {
    let mut input = File::open(path).map_err(|e| io_err(&e))?;
    let mut hasher = Md5::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        // Checked before the end-of-file read too, so a cancel during the last chunk counts.
        if cancel.load(Ordering::SeqCst) {
            return Err(FileOpError::Cancelled);
        }
        let n = match input.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_err(&e)),
        };
        hasher.update(&buf[..n]);
        progress(n as u64);
    }
    hasher.update(mod_name.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

/// Plain SHA-256 of a file, lower-case hex: the whole-zip hash the update catalog uses
/// (`by-hash`, `archiveSha256`, plan M7). Streams like [`game_md5`]; checks `cancel` per chunk.
pub fn sha256(
    path: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> Result<String, FileOpError> {
    let mut input = File::open(path).map_err(|e| io_err(&e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        // Checked before the end-of-file read too, so a cancel during the last chunk counts.
        if cancel.load(Ordering::SeqCst) {
            return Err(FileOpError::Cancelled);
        }
        let n = match input.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_err(&e)),
        };
        hasher.update(&buf[..n]);
        progress(n as u64);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
