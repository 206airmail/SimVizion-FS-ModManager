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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use crate::container::ModContainer;
use crate::net;

/// Suffix of a staged copy; the full name is `.<name>.<8 hex digits>.fsmm-partial`. The leading
/// dot makes `scan::enumerate` ignore it.
pub const STAGING_SUFFIX: &str = ".fsmm-partial";

/// How long a rename that publishes a verified file is retried when something else (antivirus, a
/// sync tool) briefly holds the file open.
const RENAME_RETRY: Duration = Duration::from_secs(5);
/// Pause between those retries.
const RENAME_PAUSE: Duration = Duration::from_millis(100);
/// How often a free staging name is looked for before giving up.
const STAGING_TRIES: u32 = 32;

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
    /// The old mod is already in the Recycle Bin but the verified new file could not be given its
    /// name. The new file was kept under the staged name in the detail (a bare file name, in the
    /// target folder), so nothing is lost.
    ReplaceIncomplete(String),
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
            FileOpError::ReplaceIncomplete(name) => write!(
                f,
                "the new file is kept as {name}; the old one is in the Recycle Bin"
            ),
            FileOpError::Io(d) => write!(f, "file operation failed: {d}"),
        }
    }
}

impl std::error::Error for FileOpError {}

/// A new, hard-to-repeat staging name for `name`: `.<name>.<8 hex digits>.fsmm-partial`. Another
/// copy of the same mod (this app twice, or a sync tool's leftovers) gets another one, and the
/// entry is only ever created with a call that fails when the name is taken.
pub fn staging_name(name: &str) -> String {
    staging_os_name(OsStr::new(name))
        .to_string_lossy()
        .into_owned()
}

/// Like [`staging_name`], for names that are not valid UTF-8.
fn staging_os_name(name: &OsStr) -> OsString {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    // `RandomState` is seeded by the operating system; the counter keeps two names from one
    // process apart even if the seed repeated.
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    let mut out = OsString::from(".");
    out.push(name);
    out.push(format!(".{:08x}", hasher.finish() as u32));
    out.push(STAGING_SUFFIX);
    out
}

/// Creates an empty staged file for `name` in `dir`, under a name that did not exist. Whatever is
/// created here is this operation's own; nothing that was already there is touched.
fn reserve_file(dir: &Path, name: &OsStr) -> Result<(PathBuf, File), FileOpError> {
    for _ in 0..STAGING_TRIES {
        let path = dir.join(staging_os_name(name));
        match File::create_new(&path) {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io_err(&e)),
        }
    }
    Err(FileOpError::Io(format!(
        "{:?}",
        io::ErrorKind::AlreadyExists
    )))
}

/// [`reserve_file`] for a folder.
fn reserve_dir(dir: &Path, name: &OsStr) -> Result<PathBuf, FileOpError> {
    for _ in 0..STAGING_TRIES {
        let path = dir.join(staging_os_name(name));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io_err(&e)),
        }
    }
    Err(FileOpError::Io(format!(
        "{:?}",
        io::ErrorKind::AlreadyExists
    )))
}

/// How a verified staged file takes its name: the rename, and how long a refused one is retried.
/// Tests pass a rename that fails.
struct Publish<'a> {
    rename: &'a dyn Fn(&Path, &Path) -> io::Result<()>,
    retry_for: Duration,
    pause: Duration,
}

impl Publish<'static> {
    fn real() -> Self {
        Self {
            rename: &|from, to| fs::rename(from, to),
            retry_for: RENAME_RETRY,
            pause: RENAME_PAUSE,
        }
    }
}

impl Publish<'_> {
    /// Renames, retrying for a while when the error can be a passing sharing violation or an
    /// access-denied from a tool that has the file open.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let started = Instant::now();
        loop {
            match (self.rename)(from, to) {
                Err(e) if transient(&e) && started.elapsed() < self.retry_for => {
                    std::thread::sleep(self.pause);
                }
                other => return other,
            }
        }
    }
}

/// Access denied, a sharing violation or a lock violation: another process has the file open.
fn transient(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(5 | 32 | 33))
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

/// Removes a staged copy that this operation created (see [`reserve_file`]): the app's own
/// partial file, so it is removed directly instead of going to the Recycle Bin. Best effort;
/// nothing else is ever removed this way.
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
        let output = File::create_new(dst).map_err(|e| io_err(&e))?;
        self.file_into(src, output)
    }

    /// Copies `src` into a file that is already created (and is this operation's own).
    fn file_into(&mut self, src: &Path, mut output: File) -> Result<(), FileOpError> {
        let mut input = File::open(src).map_err(|e| io_err(&e))?;
        let modified = input.metadata().and_then(|m| m.modified()).ok();
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
        self.folder_into(src, dst)
    }

    /// Copies the contents of `src` into a folder that is already created (and is this
    /// operation's own).
    fn folder_into(&mut self, src: &Path, dst: &Path) -> Result<(), FileOpError> {
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
    stage(
        src,
        dst_dir,
        progress,
        cancel,
        after_copy,
        &mut || Ok(()),
        false,
        &Publish::real(),
    )
}

/// What a downloaded update must be before it may replace the installed mod (plan M7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadCheck {
    /// Lower-case hex SHA-256 the file must have, when the catalog knows it.
    pub sha256: Option<String>,
    /// The `modDesc.xml` version must be equal to or newer than this.
    pub min_version: String,
    /// Approximate size from a ModHub page; the file must be within 5% of it.
    pub approx_size: Option<u64>,
}

/// Installs a downloaded update under `file_name` in `dst_dir` (plan M7 decision 5): `fill`
/// writes the download into a hidden staged file; then the file is synced and verified
/// (`check`: SHA-256 when given, opens as a zip with a `modDesc.xml` whose version is at least
/// `min_version`, size near `approx_size`); then `before_rename` runs (the installed mod goes to
/// the Recycle Bin there) and the staged file takes its name. Any failure or cancel before that
/// removes the staged file (a new, unused name made by this call, never an existing entry) and
/// leaves the installed mod alone. `FileOpError::Verify` names the failed check. After
/// `before_rename` has succeeded the verified file is never deleted: a rename that still fails
/// after a few seconds of retries is `FileOpError::ReplaceIncomplete` with the staged name.
pub fn install_download(
    dst_dir: &Path,
    file_name: &str,
    fill: &mut dyn FnMut(&mut File) -> Result<(), FileOpError>,
    check: &DownloadCheck,
    before_rename: &mut dyn FnMut() -> Result<(), FileOpError>,
) -> Result<PathBuf, FileOpError> {
    install_download_with(
        dst_dir,
        file_name,
        fill,
        check,
        before_rename,
        &Publish::real(),
    )
}

/// [`install_download`] with the final rename injected.
fn install_download_with(
    dst_dir: &Path,
    file_name: &str,
    fill: &mut dyn FnMut(&mut File) -> Result<(), FileOpError>,
    check: &DownloadCheck,
    before_rename: &mut dyn FnMut() -> Result<(), FileOpError>,
    publish: &Publish<'_>,
) -> Result<PathBuf, FileOpError> {
    // A bare file name: it becomes both the staging name and the target.
    if file_name.is_empty() || Path::new(file_name).file_name() != Some(OsStr::new(file_name)) {
        return Err(FileOpError::Io(format!(
            "{:?}",
            io::ErrorKind::InvalidInput
        )));
    }
    let target = dst_dir.join(file_name);
    // A new name that did not exist; leftovers of earlier, interrupted downloads are left alone.
    let (staging, mut file) = reserve_file(dst_dir, OsStr::new(file_name))?;

    let outcome = (|| {
        fill(&mut file)?;
        // On disk before the rename makes it visible, as for a copy.
        file.sync_all().map_err(|e| io_err(&e))?;
        drop(file);
        verify_download(&staging, check)?;
        // The installed mod goes to the Recycle Bin here, only once its replacement is good.
        before_rename()?;
        // The last look before the rename, which would replace a file that appeared meanwhile.
        if taken(&target) {
            return Err(FileOpError::Exists);
        }
        // The installed mod has gone to the Recycle Bin by now: the staged file is the new
        // version's only copy, and a failing rename must not throw it away.
        publish_staged(&staging, &target, publish, true)
    })();
    finish(outcome, &staging, target)
}

/// Gives a verified staged file its name. When `old_gone` (the installed mod is already in the
/// Recycle Bin) a rename that keeps failing is [`FileOpError::ReplaceIncomplete`], naming the
/// staged file that is left in place; otherwise it is a plain io error.
fn publish_staged(
    staging: &Path,
    target: &Path,
    publish: &Publish<'_>,
    old_gone: bool,
) -> Result<(), FileOpError> {
    publish.rename(staging, target).map_err(|e| {
        if old_gone {
            let name = staging
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            FileOpError::ReplaceIncomplete(name)
        } else {
            io_err(&e)
        }
    })
}

/// The end of a staged write. A failure removes the staged entry this operation created, except
/// [`FileOpError::ReplaceIncomplete`]: the old mod is already in the Recycle Bin, so the verified
/// new file stays.
fn finish(
    outcome: Result<(), FileOpError>,
    staging: &Path,
    target: PathBuf,
) -> Result<PathBuf, FileOpError> {
    match outcome {
        Ok(()) => Ok(target),
        Err(e @ FileOpError::ReplaceIncomplete(_)) => Err(e),
        Err(e) => {
            remove_staging(staging);
            Err(e)
        }
    }
}

/// The checks of [`DownloadCheck`], in the order SHA-256, zip, `modDesc.xml`, version, size. A
/// failure's detail is `sha256`, `notAZip`, `noModDesc`, `version` or `size`.
fn verify_download(path: &Path, check: &DownloadCheck) -> Result<(), FileOpError> {
    let fail = |what: &str| FileOpError::Verify(what.to_string());
    if let Some(want) = &check.sha256 {
        let got = hash::sha256(path, &mut |_| {}, &AtomicBool::new(false))?;
        if !got.eq_ignore_ascii_case(want.trim()) {
            return Err(fail("sha256"));
        }
    }
    let mut container = ModContainer::open(path).map_err(|_| fail("notAZip"))?;
    let version = crate::scan::desc_version(&mut container).ok_or_else(|| fail("noModDesc"))?;
    let current = net::versions_equal(&version, &check.min_version)
        || net::is_newer(&version, &check.min_version) == Some(true);
    if !current {
        return Err(fail("version"));
    }
    if let Some(approx) = check.approx_size.filter(|&a| a > 0) {
        let size = fs::metadata(path).map_err(|e| io_err(&e))?.len();
        // Within 5% of what the page said.
        if u128::from(size.abs_diff(approx)) * 100 > u128::from(approx) * 5 {
            return Err(fail("size"));
        }
    }
    Ok(())
}

/// [`copy_staged`] for a replace. `before_rename` runs once the copy has verified, just before it
/// takes its name; that is where the mod being replaced goes to the Recycle Bin, so a cancelled or
/// failed copy leaves it in place. An error from it removes the staged copy and is returned. The
/// target name may be taken until `before_rename` has run. If the rename after it keeps failing the
/// verified copy is kept under its staged name (`FileOpError::ReplaceIncomplete`).
pub fn copy_staged_replacing(
    src: &Path,
    dst_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
    before_rename: &mut dyn FnMut() -> Result<(), FileOpError>,
) -> Result<PathBuf, FileOpError> {
    stage(
        src,
        dst_dir,
        progress,
        cancel,
        &|_| {},
        before_rename,
        true,
        &Publish::real(),
    )
}

fn stage(
    src: &Path,
    dst_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
    after_copy: &dyn Fn(&Path),
    before_rename: &mut dyn FnMut() -> Result<(), FileOpError>,
    replacing: bool,
    publish: &Publish<'_>,
) -> Result<PathBuf, FileOpError> {
    let name = name_of(src)?;
    let target = dst_dir.join(name);
    let is_dir = fs::metadata(src).map_err(|e| io_err(&e))?.is_dir();
    // A new name that did not exist; leftovers of earlier, interrupted copies are left alone.
    let (staging, file) = if is_dir {
        (reserve_dir(dst_dir, name)?, None)
    } else {
        let (path, file) = reserve_file(dst_dir, name)?;
        (path, Some(file))
    };

    let outcome = (|| {
        let mut copier = Copier {
            buf: vec![0; CHUNK],
            progress,
            cancel,
        };
        match file {
            Some(file) => copier.file_into(src, file)?,
            None => copier.folder_into(src, &staging)?,
        }
        after_copy(&staging);
        verify(src, &staging, is_dir, is_zip_name(name))?;
        before_rename()?;
        // The last look before the rename, which would replace a file that appeared meanwhile.
        if taken(&target) {
            return Err(FileOpError::Exists);
        }
        // For a replace the old mod is in the Recycle Bin by now, and a failing rename must not
        // throw the staged copy away.
        publish_staged(&staging, &target, publish, replacing)
    })();
    finish(outcome, &staging, target)
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
    move_item_guarded(src, dst_dir, progress, cancel, recycler, &mut || Ok(()))
}

/// [`move_item`] with a last look before the source is disposed of after a copy across drives:
/// `before_dispose` runs once the copy is in place; an error from it keeps the source and is
/// returned (the copy stays too). A rename on one drive moves nothing to dispose of.
pub fn move_item_guarded(
    src: &Path,
    dst_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
    recycler: &dyn Recycler,
    before_dispose: &mut dyn FnMut() -> Result<(), FileOpError>,
) -> Result<PathBuf, FileOpError> {
    move_item_guarded_with(
        src,
        dst_dir,
        progress,
        cancel,
        recycler,
        before_dispose,
        &|from, to| fs::rename(from, to),
    )
}

/// [`move_item`] with the rename injected, so the cross-drive path can be tested on one drive.
#[cfg(test)]
fn move_item_with(
    src: &Path,
    dst_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
    recycler: &dyn Recycler,
    rename: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> Result<PathBuf, FileOpError> {
    move_item_guarded_with(
        src,
        dst_dir,
        progress,
        cancel,
        recycler,
        &mut || Ok(()),
        rename,
    )
}

fn move_item_guarded_with(
    src: &Path,
    dst_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
    recycler: &dyn Recycler,
    before_dispose: &mut dyn FnMut() -> Result<(), FileOpError>,
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
            before_dispose()?;
            recycler.recycle(src).map_err(FileOpError::Recycle)?;
            Ok(copied)
        }
        Err(e) => Err(io_err(&e)),
    }
}
