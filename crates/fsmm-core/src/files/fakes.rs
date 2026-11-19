//! Test doubles for the Recycle Bin and free disk space. Nothing here touches the real bin.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use super::{DiskSpace, RecycleError, Recycler};
use crate::store::path_key;

/// "Recycles" by moving into a temp `bin` folder (which must be on the same drive as what it
/// receives). It can be told to refuse or fail specific paths, and records every call.
pub struct FakeRecycler {
    bin: PathBuf,
    refuse: Mutex<HashSet<String>>,
    fail: Mutex<HashSet<String>>,
    calls: Mutex<Vec<PathBuf>>,
    counter: AtomicUsize,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl FakeRecycler {
    pub fn new(bin: &Path) -> Self {
        fs::create_dir_all(bin).unwrap();
        Self {
            bin: bin.to_path_buf(),
            refuse: Mutex::new(HashSet::new()),
            fail: Mutex::new(HashSet::new()),
            calls: Mutex::new(Vec::new()),
            counter: AtomicUsize::new(0),
        }
    }

    /// `recycle(path)` returns `Unavailable` and leaves the item in place.
    pub fn refuse(&self, path: &Path) {
        lock(&self.refuse).insert(path_key(path));
    }

    /// `recycle(path)` returns `Failed` and leaves the item in place.
    pub fn fail(&self, path: &Path) {
        lock(&self.fail).insert(path_key(path));
    }

    pub fn calls(&self) -> Vec<PathBuf> {
        lock(&self.calls).clone()
    }

    /// Names of the items in the bin (without the numeric prefix the fake adds), sorted.
    pub fn binned(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.bin)
            .unwrap()
            .map(|e| {
                let name = e.unwrap().file_name().to_string_lossy().into_owned();
                name.split_once('-').map(|(_, n)| n.to_string()).unwrap()
            })
            .collect();
        names.sort();
        names
    }
}

impl Recycler for FakeRecycler {
    fn recycle(&self, path: &Path) -> Result<(), RecycleError> {
        lock(&self.calls).push(path.to_path_buf());
        let key = path_key(path);
        if lock(&self.refuse).contains(&key) {
            return Err(RecycleError::Unavailable);
        }
        if lock(&self.fail).contains(&key) {
            return Err(RecycleError::Failed("Fake".to_string()));
        }
        if !path.exists() {
            return Err(RecycleError::Failed("NotFound".to_string()));
        }
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        fs::rename(path, self.bin.join(format!("{n}-{name}")))
            .map_err(|e| RecycleError::Failed(format!("{:?}", e.kind())))
    }
}

/// Reports a settable amount of free space for any folder.
pub struct FakeDisk {
    free: AtomicU64,
}

impl FakeDisk {
    pub fn new(free: u64) -> Self {
        Self {
            free: AtomicU64::new(free),
        }
    }

    pub fn set(&self, free: u64) {
        self.free.store(free, Ordering::SeqCst);
    }
}

impl DiskSpace for FakeDisk {
    fn free_bytes(&self, _dir: &Path) -> io::Result<u64> {
        Ok(self.free.load(Ordering::SeqCst))
    }
}
