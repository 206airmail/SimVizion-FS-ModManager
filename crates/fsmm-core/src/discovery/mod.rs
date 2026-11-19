//! Finding the game, the user folder and the effective mods folder, and reading the
//! mods-folder override and the game log. Read-only.
//!
//! Platform calls go through [`SystemProbe`] so the logic can be tested with a fake probe
//! and temp directories. Nothing here may log or return the text of `gameSettings.xml`:
//! it holds plain-text multiplayer passwords.

use std::io;
use std::path::{Path, PathBuf};

use crate::model::GameStatus;

/// Steam app id of Farming Simulator 25.
pub const STEAM_APP_ID: u32 = 2300320;
pub const GAME_EXE: &str = "FarmingSimulator2025.exe";
pub const GAME_PROCESS_NAMES: &[&str] =
    &["FarmingSimulator2025.exe", "FarmingSimulator2025Game.exe"];
pub const SERVER_PROCESS_NAME: &str = "dedicatedServer.exe";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

/// The operating-system facts discovery depends on.
pub trait SystemProbe {
    /// A string value, or `None` if the key or value is missing.
    /// `key` is a full path below the hive, e.g. `SOFTWARE\WOW6432Node\Valve\Steam`.
    fn registry_string(&self, hive: Hive, key: &str, value: &str) -> Option<String>;
    /// Names of the direct subkeys of `key`; empty if it is missing.
    fn registry_subkeys(&self, hive: Hive, key: &str) -> Vec<String>;
    /// Executable file names of running processes, e.g. `dedicatedServer.exe`.
    fn process_names(&self) -> Vec<String>;
    /// The Documents known folder, honouring redirection.
    fn documents_dir(&self) -> Option<PathBuf>;
    /// `%ProgramData%`, where the Epic launcher keeps its manifests.
    fn program_data_dir(&self) -> Option<PathBuf>;
}

/// The real probe for Windows.
#[cfg(windows)]
pub struct WindowsProbe;

#[cfg(windows)]
impl SystemProbe for WindowsProbe {
    fn registry_string(&self, _hive: Hive, _key: &str, _value: &str) -> Option<String> {
        todo!("WS-A")
    }
    fn registry_subkeys(&self, _hive: Hive, _key: &str) -> Vec<String> {
        todo!("WS-A")
    }
    fn process_names(&self) -> Vec<String> {
        todo!("WS-A")
    }
    fn documents_dir(&self) -> Option<PathBuf> {
        todo!("WS-A")
    }
    fn program_data_dir(&self) -> Option<PathBuf> {
        todo!("WS-A")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    Steam,
    Giants,
    Epic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameInstall {
    pub store: Store,
    pub dir: PathBuf,
    /// Contents of the install's `VERSION` file, trimmed.
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Running {
    pub game: bool,
    pub server: bool,
}

/// The `<modsDirectoryOverride active=".." directory=".."/>` element of `gameSettings.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModsOverride {
    pub active: bool,
    pub directory: Option<PathBuf>,
}

/// Why `gameSettings.xml` could not be read. Never carries any of the file's text.
#[derive(Debug)]
pub enum SettingsError {
    Io(io::ErrorKind),
    /// Not well-formed XML; only the position is kept.
    Malformed {
        line: u32,
        column: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogSummary {
    /// e.g. `1.10.0.0 b34141`, from the `Version:` line of the game's startup block.
    pub game_version: Option<String>,
    /// From the `ModDesc Version: N` line.
    pub mod_desc_version: Option<u32>,
}

/// Every install found, each validated by the presence of [`GAME_EXE`].
pub fn find_installs(_probe: &dyn SystemProbe) -> Vec<GameInstall> {
    todo!("WS-A")
}

/// `Documents\My Games\FarmingSimulator2025`, if it exists.
pub fn user_dir(_probe: &dyn SystemProbe) -> Option<PathBuf> {
    todo!("WS-A")
}

pub fn running(_probe: &dyn SystemProbe) -> Running {
    todo!("WS-A")
}

pub fn read_mods_override(_game_settings: &Path) -> Result<ModsOverride, SettingsError> {
    todo!("WS-A")
}

pub fn read_log_summary(_log: &Path) -> io::Result<LogSummary> {
    todo!("WS-A")
}

/// Folders worth offering as profiles: `mods` plus direct subfolders of the user folder
/// that contain at least one mod.
pub fn suggest_profiles(_user_dir: &Path) -> Vec<PathBuf> {
    todo!("WS-A")
}

/// Assembles the frontend's [`GameStatus`]. `logged_desc_version` is only set when the
/// log was written by the installed game version (plan decision 2).
pub fn game_status(_probe: &dyn SystemProbe) -> GameStatus {
    todo!("WS-A")
}
