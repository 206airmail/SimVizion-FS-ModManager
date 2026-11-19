//! Copying, moving and recycling mods (plan M5). Nothing here deletes anything permanently:
//! removal is the [`Recycler`]'s job, and a recycler must fail rather than delete.
//!
//! A copy is written under a hidden staging name in the target folder, verified, and only then
//! renamed to the real name, so neither the game nor the scanner ever sees half a mod.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// Suffix of a staged copy; the full name is `.<name>.fsmm-partial`. The leading dot makes
/// `scan::enumerate` ignore it.
pub const STAGING_SUFFIX: &str = ".fsmm-partial";

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::{delete_would_recycle, WindowsDiskSpace, WindowsRecycler};

/// Moves files and folders to the Recycle Bin. Implementations must return
/// [`RecycleError::Unavailable`] instead of deleting permanently.
pub trait Recycler: Send + Sync {
    fn recycle(&self, path: &Path) -> Result<(), RecycleError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecycleError {
    /// Recycling is not possible for this item (too large for the bin, or a drive without
    /// one); it was left in place.
    Unavailable,
    /// Anything else; the item is still in place. The detail is an error code or kind.
    Failed(String),
}

impl fmt::Display for RecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecycleError::Unavailable => write!(f, "the item cannot go to the Recycle Bin"),
            RecycleError::Failed(d) => write!(f, "recycling failed: {d}"),
        }
    }
}

impl std::error::Error for RecycleError {}

/// Free space on the drive holding a folder.
pub trait DiskSpace: Send + Sync {
    fn free_bytes(&self, dir: &Path) -> io::Result<u64>;
}

#[derive(Debug)]
pub enum FileOpError {
    Cancelled,
    /// The target name is already taken.
    Exists,
    /// The copy did not match the source; it was removed. The detail names the check.
    Verify(String),
    Recycle(RecycleError),
    /// The detail is an io error kind, never file content.
    Io(String),
}

impl fmt::Display for FileOpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileOpError::Cancelled => write!(f, "cancelled"),
            FileOpError::Exists => write!(f, "the target already exists"),
            FileOpError::Verify(d) => write!(f, "the copy did not verify: {d}"),
            FileOpError::Recycle(e) => write!(f, "{e}"),
            FileOpError::Io(d) => write!(f, "file operation failed: {d}"),
        }
    }
}

impl std::error::Error for FileOpError {}

/// `.<name>.fsmm-partial`.
pub fn staging_name(_name: &str) -> String {
    todo!("WS-A")
}

/// Total bytes of a file, or of every file in a folder.
pub fn size_of(_path: &Path) -> io::Result<u64> {
    todo!("WS-A")
}

/// Copies a mod (zip file or folder) into `dst_dir` under its own name: staged, verified
/// (size; a zip opens with the same entry count; a folder has the same files and sizes), source
/// mtimes kept, then renamed into place. `progress` receives bytes written since the last call.
/// Checks `cancel` between chunks. On any failure the staged copy is removed.
pub fn copy_staged(
    _src: &Path,
    _dst_dir: &Path,
    _progress: &mut dyn FnMut(u64),
    _cancel: &AtomicBool,
) -> Result<PathBuf, FileOpError> {
    todo!("WS-A")
}

/// Moves a mod into `dst_dir`: a rename on the same drive; otherwise `copy_staged`, then the
/// source goes to the Recycle Bin. A recycle failure after a good copy leaves both in place and
/// returns the error, so nothing is lost.
pub fn move_item(
    _src: &Path,
    _dst_dir: &Path,
    _progress: &mut dyn FnMut(u64),
    _cancel: &AtomicBool,
    _recycler: &dyn Recycler,
) -> Result<PathBuf, FileOpError> {
    todo!("WS-A")
}
