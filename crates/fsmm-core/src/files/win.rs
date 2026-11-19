//! The real [`Recycler`] and [`DiskSpace`] for Windows (plan M5 decision 1).
//!
//! Recycling uses `IFileOperation` with `FOFX_RECYCLEONDELETE`. Microsoft does not document what
//! happens to an item that cannot be recycled (too large for the bin, or a drive without one),
//! and reports say it is deleted permanently. So a progress sink vetoes every delete that Windows
//! does not flag as `TSF_DELETE_RECYCLE_IF_POSSIBLE` (the technique Electron uses), and an item
//! only counts as recycled when its delete succeeded *and* Windows reports the new item in the
//! Recycle Bin. `FOF_WANTNUKEWARNING` is set as a second line of defence.

use std::cell::Cell;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::rc::Rc;

use windows::core::{implement, Ref, HRESULT, PCWSTR};
use windows::Win32::Foundation::E_ABORT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    FileOperation, IFileOperation, IFileOperationProgressSink, IFileOperationProgressSink_Impl,
    IShellItem, SHCreateItemFromParsingName, FOFX_RECYCLEONDELETE, FOF_ALLOWUNDO,
    FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING,
    TSF_DELETE_RECYCLE_IF_POSSIBLE,
};

use super::{DiskSpace, RecycleError, Recycler};

/// May a delete with these transfer-source flags go ahead? Only when Windows says it will recycle.
pub fn delete_would_recycle(flags: u32) -> bool {
    flags & (TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32) != 0
}

pub struct WindowsRecycler;

impl Recycler for WindowsRecycler {
    fn recycle(&self, path: &Path) -> Result<(), RecycleError> {
        if !path.exists() {
            return Err(RecycleError::Failed("NotFound".to_string()));
        }
        let _com = ComApartment::enter();
        let state = Rc::new(SinkState::default());
        let outcome = recycle_one(path, &state);

        if state.vetoed.get() {
            return Err(RecycleError::Unavailable);
        }
        let still_there = path.exists();
        match outcome {
            Ok(()) if state.recycled.get() && !still_there => Ok(()),
            _ if still_there => Err(RecycleError::Failed(
                state
                    .failure
                    .get()
                    .map_or_else(|| "NotRecycled".to_string(), hresult_text),
            )),
            // Gone without a Recycle Bin item: must never happen with the veto in place.
            _ => Err(RecycleError::Failed("DeletedWithoutRecycling".to_string())),
        }
    }
}

fn hresult_text(hr: HRESULT) -> String {
    format!("0x{:08X}", hr.0 as u32)
}

fn recycle_one(path: &Path, state: &Rc<SinkState>) -> windows::core::Result<()> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: COM is initialised on this thread by the caller's `ComApartment`; `wide` is a
    // NUL-terminated UTF-16 path that outlives the call that reads it; the sink stays alive
    // while advised and is unadvised before returning.
    unsafe {
        let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?;
        op.SetOperationFlags(
            FOF_NOCONFIRMATION
                | FOF_SILENT
                | FOF_NOERRORUI
                | FOF_ALLOWUNDO
                | FOF_WANTNUKEWARNING
                | FOFX_RECYCLEONDELETE,
        )?;
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)?;
        let sink: IFileOperationProgressSink = Sink {
            state: Rc::clone(state),
        }
        .into();
        let cookie = op.Advise(&sink)?;
        let queued = op.DeleteItem(&item, None);
        let performed = queued.and_then(|()| op.PerformOperations());
        let _ = op.Unadvise(cookie);
        performed
    }
}

#[derive(Default)]
struct SinkState {
    /// A delete was refused because Windows would not recycle the item.
    vetoed: Cell<bool>,
    /// The delete succeeded and produced an item in the Recycle Bin.
    recycled: Cell<bool>,
    failure: Cell<Option<HRESULT>>,
}

#[implement(IFileOperationProgressSink)]
struct Sink {
    state: Rc<SinkState>,
}

#[allow(non_snake_case)]
impl IFileOperationProgressSink_Impl for Sink_Impl {
    fn PreDeleteItem(&self, dwflags: u32, _item: Ref<IShellItem>) -> windows::core::Result<()> {
        if delete_would_recycle(dwflags) {
            Ok(())
        } else {
            self.state.vetoed.set(true);
            Err(E_ABORT.into())
        }
    }

    fn PostDeleteItem(
        &self,
        _dwflags: u32,
        _item: Ref<IShellItem>,
        hrdelete: HRESULT,
        newly_created: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        if hrdelete.is_ok() && !newly_created.is_null() {
            self.state.recycled.set(true);
        } else {
            self.state.failure.set(Some(hrdelete));
        }
        Ok(())
    }

    fn StartOperations(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn FinishOperations(&self, _hr: HRESULT) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreRenameItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _n: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostRenameItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _n: &PCWSTR,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreMoveItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostMoveItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreCopyItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostCopyItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreNewItem(&self, _f: u32, _d: Ref<IShellItem>, _n: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostNewItem(
        &self,
        _f: u32,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
        _t: &PCWSTR,
        _a: u32,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn UpdateProgress(&self, _total: u32, _so_far: u32) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResetTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
}

/// Initialises COM on this thread for the guard's lifetime. A thread already in another
/// apartment keeps it; only a successful initialisation is balanced on drop.
struct ComApartment(bool);

impl ComApartment {
    fn enter() -> Self {
        // SAFETY: plain COM initialisation for the current thread, balanced in `drop`.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        ComApartment(hr.is_ok())
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful `CoInitializeEx` above, on the same thread.
            unsafe { CoUninitialize() };
        }
    }
}

pub struct WindowsDiskSpace;

impl DiskSpace for WindowsDiskSpace {
    fn free_bytes(&self, dir: &Path) -> io::Result<u64> {
        let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut available: u64 = 0;
        // SAFETY: `wide` is NUL-terminated and outlives the call; the out pointer is valid and
        // the two optional outputs are null.
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut available,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(available)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_deletes_windows_will_recycle_may_proceed() {
        assert!(delete_would_recycle(0x80));
        assert!(delete_would_recycle(0x80 | 0x01));
        assert!(!delete_would_recycle(0));
        assert!(!delete_would_recycle(0x01 | 0x40));
    }

    #[test]
    fn free_space_of_a_temp_folder_is_positive() {
        let dir = tempfile::tempdir().unwrap();
        assert!(WindowsDiskSpace.free_bytes(dir.path()).unwrap() > 0);
    }

    /// Recycles a throwaway file into the real Recycle Bin. Run on purpose only:
    /// `cargo test -p fsmm-core recycles_a_temp_file -- --ignored`.
    #[test]
    #[ignore = "puts a small file into the real Recycle Bin"]
    fn recycles_a_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fsmm-recycle-test.txt");
        std::fs::write(&path, b"SimVizion Mod Manager recycle test").unwrap();
        WindowsRecycler.recycle(&path).unwrap();
        assert!(!path.exists());
    }

    /// Recycling on a path without a Recycle Bin must leave the file in place. Set
    /// `FSMM_NO_BIN_DIR` to a folder on such a drive (for example a network share).
    #[test]
    #[ignore = "needs a folder on a drive without a Recycle Bin"]
    fn refuses_where_there_is_no_recycle_bin() {
        let dir = std::path::PathBuf::from(std::env::var("FSMM_NO_BIN_DIR").unwrap());
        let path = dir.join("fsmm-recycle-test.txt");
        std::fs::write(&path, b"SimVizion Mod Manager recycle test").unwrap();
        let result = WindowsRecycler.recycle(&path);
        let survived = path.exists();
        if survived {
            std::fs::remove_file(&path).unwrap();
        }
        assert_eq!(result, Err(RecycleError::Unavailable));
        assert!(survived, "the file must not be deleted permanently");
    }
}
