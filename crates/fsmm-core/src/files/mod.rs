//! Copying, moving and recycling mods (plan M5). Nothing here deletes anything permanently:
//! removal is the [`Recycler`]'s job, and a recycler must fail rather than delete.
//!
//! A copy is written under a hidden staging name in the target folder, verified, and only then
//! renamed to the real name, so neither the game nor the scanner ever sees half a mod.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use crate::container::ModContainer;

/// Suffix of a staged copy; the full name is `.<name>.fsmm-partial`. The leading dot makes
/// `scan::enumerate` ignore it.
pub const STAGING_SUFFIX: &str = ".fsmm-partial";

/// Bytes read and written per step; progress and cancellation are per chunk.
const CHUNK: usize = 4 * 1024 * 1024;

#[cfg(test)]
pub(crate) mod fakes;
pub mod hash;
#[cfg(test)]
mod tests;
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
pub fn staging_name(name: &str) -> String {
    format!(".{name}{STAGING_SUFFIX}")
}

/// Like [`staging_name`], for names that are not valid UTF-8.
fn staging_os_name(name: &OsStr) -> OsString {
    let mut out = OsString::from(".");
    out.push(name);
    out.push(STAGING_SUFFIX);
    out
}

/// Total bytes of a file, or of every file in a folder (found by walking it; links are not
/// followed, as in `scan::enumerate`).
pub fn size_of(path: &Path) -> io::Result<u64> {
    Ok(stats_of(path)?.0)
}

/// Total bytes and the newest file mtime of a file or folder (a file's own mtime). A folder with
/// no files has no newest time. These are the size and mtime `scan::enumerate` records for a
/// mod, except that an unreadable part is an error here.
pub fn stats_of(path: &Path) -> io::Result<(u64, Option<SystemTime>)> {
    let meta = fs::metadata(path)?;
    if !meta.is_dir() {
        return Ok((meta.len(), meta.modified().ok()));
    }
    let mut size = 0u64;
    let mut newest: Option<SystemTime> = None;
    let mut pending = vec![path.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for item in fs::read_dir(&dir)? {
            let item = item?;
            let file_type = item.file_type()?;
            if file_type.is_dir() {
                pending.push(item.path());
            } else if file_type.is_file() {
                let meta = item.metadata()?;
                size = size.saturating_add(meta.len());
                if let Ok(m) = meta.modified() {
                    newest = Some(newest.map_or(m, |n| n.max(m)));
                }
            }
        }
    }
    Ok((size, newest))
}

fn io_err(e: &io::Error) -> FileOpError {
    FileOpError::Io(format!("{:?}", e.kind()))
}

/// The name a mod has in its folder; a path without one cannot be copied.
fn name_of(path: &Path) -> Result<&OsStr, FileOpError> {
    path.file_name()
        .ok_or_else(|| FileOpError::Io(format!("{:?}", io::ErrorKind::InvalidInput)))
}

fn taken(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Removes a staged copy: this app's own partial file, so it is removed directly instead of
/// going to the Recycle Bin. Best effort; nothing else is ever removed this way.
fn remove_staging(path: &Path) {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => {
            let _ = fs::remove_dir_all(path);
        }
        Ok(_) => {
            let _ = fs::remove_file(path);
        }
        Err(_) => {}
    }
}

fn is_zip_name(name: &OsStr) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

/// Copies files in [`CHUNK`]s with progress and cancellation.
struct Copier<'a> {
    buf: Vec<u8>,
    progress: &'a mut dyn FnMut(u64),
    cancel: &'a AtomicBool,
}

impl Copier<'_> {
    fn file(&mut self, src: &Path, dst: &Path) -> Result<(), FileOpError> {
        let mut input = File::open(src).map_err(|e| io_err(&e))?;
        let modified = input.metadata().and_then(|m| m.modified()).ok();
        let mut output = File::create_new(dst).map_err(|e| io_err(&e))?;
        loop {
            // Checked before the end-of-file read too, so a cancel during the last chunk counts.
            if self.cancel.load(Ordering::SeqCst) {
                return Err(FileOpError::Cancelled);
            }
            let n = match input.read(&mut self.buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_err(&e)),
            };
            output.write_all(&self.buf[..n]).map_err(|e| io_err(&e))?;
            (self.progress)(n as u64);
        }
        if let Some(modified) = modified {
            output.set_modified(modified).map_err(|e| io_err(&e))?;
        }
        // On disk before the rename makes it visible, so a power cut cannot leave a mod under its
        // real name with missing data (and, for a move, the source already in the Recycle Bin).
        output.sync_all().map_err(|e| io_err(&e))
    }

    fn folder(&mut self, src: &Path, dst: &Path) -> Result<(), FileOpError> {
        fs::create_dir(dst).map_err(|e| io_err(&e))?;
        for item in fs::read_dir(src).map_err(|e| io_err(&e))? {
            let item = item.map_err(|e| io_err(&e))?;
            let file_type = item.file_type().map_err(|e| io_err(&e))?;
            let target = dst.join(item.file_name());
            if file_type.is_dir() {
                self.folder(&item.path(), &target)?;
            } else if file_type.is_file() {
                self.file(&item.path(), &target)?;
            }
        }
        Ok(())
    }
}

/// Every file under `root` as `(relative path with '/', size)`, sorted.
fn file_list(root: &Path) -> io::Result<Vec<(String, u64)>> {
    let mut out = Vec::new();
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel)) = pending.pop() {
        for item in fs::read_dir(&dir)? {
            let item = item?;
            let file_type = item.file_type()?;
            let name = format!("{rel}{}", item.file_name().to_string_lossy());
            if file_type.is_dir() {
                pending.push((item.path(), format!("{name}/")));
            } else if file_type.is_file() {
                out.push((name, item.metadata()?.len()));
            }
        }
    }
    out.sort();
    Ok(out)
}

fn entry_count(path: &Path) -> Option<usize> {
    ModContainer::open(path).ok().map(|c| c.names().count())
}

/// Compares a staged copy with its source.
fn verify(src: &Path, staged: &Path, is_dir: bool, zip: bool) -> Result<(), FileOpError> {
    let mismatch = |what: &str| FileOpError::Verify(what.to_string());
    if is_dir {
        let a = file_list(src).map_err(|e| io_err(&e))?;
        let b = file_list(staged).map_err(|e| io_err(&e))?;
        return if a == b {
            Ok(())
        } else {
            Err(mismatch("files"))
        };
    }
    let want = fs::metadata(src).map_err(|e| io_err(&e))?.len();
    let got = fs::metadata(staged).map_err(|e| io_err(&e))?.len();
    if want != got {
        return Err(mismatch("size"));
    }
    // A source that is not a readable zip (a broken download) is still copied faithfully.
    if zip {
        if let Some(count) = entry_count(src) {
            if entry_count(staged) != Some(count) {
                return Err(mismatch("zipEntries"));
            }
        }
    }
    Ok(())
}

/// Copies a mod (zip file or folder) into `dst_dir` under its own name: staged, verified
/// (size; a zip opens with the same entry count; a folder has the same files and sizes), source
/// mtimes kept, then renamed into place. `progress` receives bytes written since the last call.
/// Checks `cancel` between chunks. On any failure the staged copy is removed.
pub fn copy_staged(
    src: &Path,
    dst_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, FileOpError> {
    copy_staged_with(src, dst_dir, progress, cancel, &|_| {})
}

/// [`copy_staged`] with a hook that runs on the staged copy after the data is written and before
/// it is verified (tests use it to damage the copy or look at the folder mid-copy).
fn copy_staged_with(
    src: &Path,
    dst_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
    after_copy: &dyn Fn(&Path),
) -> Result<PathBuf, FileOpError> {
    if taken(&dst_dir.join(name_of(src)?)) {
        return Err(FileOpError::Exists);
    }
    stage(src, dst_dir, progress, cancel, after_copy, &mut || Ok(()))
}

/// [`copy_staged`] for a replace. `before_rename` runs once the copy has verified, just before it
/// takes its name; that is where the mod being replaced goes to the Recycle Bin, so a cancelled or
/// failed copy leaves it in place. An error from it removes the staged copy and is returned. The
/// target name may be taken until `before_rename` has run.
pub fn copy_staged_replacing(
    src: &Path,
    dst_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
    before_rename: &mut dyn FnMut() -> Result<(), FileOpError>,
) -> Result<PathBuf, FileOpError> {
    stage(src, dst_dir, progress, cancel, &|_| {}, before_rename)
}

fn stage(
    src: &Path,
    dst_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
    after_copy: &dyn Fn(&Path),
    before_rename: &mut dyn FnMut() -> Result<(), FileOpError>,
) -> Result<PathBuf, FileOpError> {
    let name = name_of(src)?;
    let target = dst_dir.join(name);
    let is_dir = fs::metadata(src).map_err(|e| io_err(&e))?.is_dir();
    let staging = dst_dir.join(staging_os_name(name));
    // A leftover of an earlier, interrupted copy of this same mod.
    remove_staging(&staging);

    let outcome = (|| {
        let mut copier = Copier {
            buf: vec![0; CHUNK],
            progress,
            cancel,
        };
        if is_dir {
            copier.folder(src, &staging)?;
        } else {
            copier.file(src, &staging)?;
        }
        after_copy(&staging);
        verify(src, &staging, is_dir, is_zip_name(name))?;
        before_rename()?;
        // The last look before the rename, which would replace a file that appeared meanwhile.
        if taken(&target) {
            return Err(FileOpError::Exists);
        }
        fs::rename(&staging, &target).map_err(|e| io_err(&e))
    })();
    match outcome {
        Ok(()) => Ok(target),
        Err(e) => {
            remove_staging(&staging);
            Err(e)
        }
    }
}

/// Moves a mod into `dst_dir`: a rename on the same drive; otherwise `copy_staged`, then the
/// source goes to the Recycle Bin. A recycle failure after a good copy leaves both in place and
/// returns the error, so nothing is lost.
pub fn move_item(
    src: &Path,
    dst_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
    recycler: &dyn Recycler,
) -> Result<PathBuf, FileOpError> {
    move_item_with(src, dst_dir, progress, cancel, recycler, &|from, to| {
        fs::rename(from, to)
    })
}

/// [`move_item`] with the rename injected, so the cross-drive path can be tested on one drive.
fn move_item_with(
    src: &Path,
    dst_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
    recycler: &dyn Recycler,
    rename: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> Result<PathBuf, FileOpError> {
    let target = dst_dir.join(name_of(src)?);
    if taken(&target) {
        return Err(FileOpError::Exists);
    }
    match rename(src, &target) {
        Ok(()) => Ok(target),
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            let copied = copy_staged(src, dst_dir, progress, cancel)?;
            recycler.recycle(src).map_err(FileOpError::Recycle)?;
            Ok(copied)
        }
        Err(e) => Err(io_err(&e)),
    }
}
