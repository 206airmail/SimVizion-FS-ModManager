//! The real [`SystemProbe`] for Windows: registry, process list and known folders.

use std::path::PathBuf;

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

use super::{Hive, SystemProbe};

/// The real probe for Windows.
pub struct WindowsProbe;

fn open(hive: Hive, key: &str) -> Option<RegKey> {
    let root = match hive {
        Hive::CurrentUser => RegKey::predef(HKEY_CURRENT_USER),
        Hive::LocalMachine => RegKey::predef(HKEY_LOCAL_MACHINE),
    };
    root.open_subkey_with_flags(key, KEY_READ).ok()
}

impl SystemProbe for WindowsProbe {
    fn registry_string(&self, hive: Hive, key: &str, value: &str) -> Option<String> {
        open(hive, key)?.get_value::<String, _>(value).ok()
    }

    fn registry_subkeys(&self, hive: Hive, key: &str) -> Vec<String> {
        match open(hive, key) {
            Some(k) => k.enum_keys().filter_map(Result::ok).collect(),
            None => Vec::new(),
        }
    }

    fn process_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        // SAFETY: `snapshot` is checked against INVALID_HANDLE_VALUE before use and closed
        // exactly once. `entry` is a zeroed plain-data struct whose `dwSize` is set as the API
        // requires, and it outlives every call that receives a pointer to it.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return names;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                names.push(String::from_utf16_lossy(&entry.szExeFile[..len]));
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        names
    }

    fn documents_dir(&self) -> Option<PathBuf> {
        dirs::document_dir()
    }

    fn program_data_dir(&self) -> Option<PathBuf> {
        std::env::var_os("ProgramData").map(PathBuf::from)
    }
    fn local_app_data_dir(&self) -> Option<PathBuf> {
        dirs::data_local_dir()
    }
}
