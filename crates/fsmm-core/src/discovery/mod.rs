//! Finding the game, the user folder and the effective mods folder, and reading the
//! mods-folder override and the game log. Read-only.
//!
//! Platform calls go through [`SystemProbe`] so the logic can be tested with a fake probe
//! and temp directories. Nothing here may log or return the text of `gameSettings.xml`:
//! it holds plain-text multiplayer passwords.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::model::GameStatus;

mod gamelog;
mod settings;
mod steam;

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

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::WindowsProbe;

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
    /// From the `Language:` line; the scanner uses it as the preferred language.
    pub language: Option<String>,
}

const UNINSTALL_KEYS: [&str; 2] = [
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
];
const GAME_DISPLAY_NAME: &str = "farming simulator 25";

/// Every install found, each validated by the presence of [`GAME_EXE`]. Steam installs come
/// first, then GIANTS, then Epic; the same folder is listed once.
pub fn find_installs(probe: &dyn SystemProbe) -> Vec<GameInstall> {
    let mut candidates: Vec<(Store, PathBuf)> = steam::candidates(probe)
        .into_iter()
        .map(|dir| (Store::Steam, dir))
        .collect();
    candidates.extend(
        giants_candidates(probe)
            .into_iter()
            .map(|d| (Store::Giants, d)),
    );
    candidates.extend(epic_candidates(probe).into_iter().map(|d| (Store::Epic, d)));

    let mut seen: Vec<String> = Vec::new();
    let mut installs = Vec::new();
    for (store, dir) in candidates {
        let key = path_key(&dir);
        if seen.contains(&key) || !dir.join(GAME_EXE).is_file() {
            continue;
        }
        seen.push(key);
        let version = read_version(&dir);
        installs.push(GameInstall {
            store,
            dir,
            version,
        });
    }
    installs
}

/// A path normalised for comparison: separators unified, trailing separator dropped, lowercase.
fn path_key(path: &Path) -> String {
    let s = path.to_string_lossy().replace('/', "\\").to_lowercase();
    s.trim_end_matches('\\').to_owned()
}

fn read_version(dir: &Path) -> Option<String> {
    let text = fs::read(dir.join("VERSION")).ok()?;
    let text = String::from_utf8_lossy(&text);
    let text = text.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    (!text.is_empty()).then(|| text.to_owned())
}

/// Uninstall entries named like the game that are not the Steam one.
fn giants_candidates(probe: &dyn SystemProbe) -> Vec<PathBuf> {
    let steam_key = format!("Steam App {STEAM_APP_ID}");
    let mut out = Vec::new();
    for hive in [Hive::LocalMachine, Hive::CurrentUser] {
        for base in UNINSTALL_KEYS {
            for sub in probe.registry_subkeys(hive, base) {
                if sub.eq_ignore_ascii_case(&steam_key) {
                    continue;
                }
                let key = format!(r"{base}\{sub}");
                let named = probe
                    .registry_string(hive, &key, "DisplayName")
                    .is_some_and(|n| n.to_lowercase().contains(GAME_DISPLAY_NAME));
                if !named {
                    continue;
                }
                if let Some(loc) = probe.registry_string(hive, &key, "InstallLocation") {
                    let loc = loc.trim().trim_matches('"').trim();
                    if !loc.is_empty() {
                        out.push(PathBuf::from(loc));
                    }
                }
            }
        }
    }
    out
}

/// `InstallLocation` of the Epic launcher manifests that name the game.
fn epic_candidates(probe: &dyn SystemProbe) -> Vec<PathBuf> {
    let Some(program_data) = probe.program_data_dir() else {
        return Vec::new();
    };
    let manifests = program_data
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");
    let Ok(entries) = fs::read_dir(manifests) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("item"))
        })
        .collect();
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let Ok(bytes) = fs::read(&file) else { continue };
        let bytes = bytes
            .strip_prefix(&[0xEF, 0xBB, 0xBF][..])
            .unwrap_or(&bytes);
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            continue;
        };
        let named = json
            .get("DisplayName")
            .and_then(|v| v.as_str())
            .is_some_and(|n| n.to_lowercase().contains(GAME_DISPLAY_NAME));
        let location = json.get("InstallLocation").and_then(|v| v.as_str());
        if let (true, Some(loc)) = (named, location.filter(|l| !l.trim().is_empty())) {
            out.push(PathBuf::from(loc.trim()));
        }
    }
    out
}

/// `Documents\My Games\FarmingSimulator2025`, if it exists.
pub fn user_dir(probe: &dyn SystemProbe) -> Option<PathBuf> {
    let dir = probe
        .documents_dir()?
        .join("My Games")
        .join("FarmingSimulator2025");
    dir.is_dir().then_some(dir)
}

pub fn running(probe: &dyn SystemProbe) -> Running {
    let names = probe.process_names();
    let has = |wanted: &[&str]| {
        names
            .iter()
            .any(|n| wanted.iter().any(|w| w.eq_ignore_ascii_case(n)))
    };
    Running {
        game: has(GAME_PROCESS_NAMES),
        server: has(&[SERVER_PROCESS_NAME]),
    }
}

/// Reads only the `modsDirectoryOverride` element. A missing element means inactive.
pub fn read_mods_override(game_settings: &Path) -> Result<ModsOverride, SettingsError> {
    settings::read(game_settings)
}

/// Reads the game's startup block from `log.txt`; an absent block gives an empty summary.
pub fn read_log_summary(log: &Path) -> io::Result<LogSummary> {
    gamelog::read(log)
}

/// The log's `ModDesc Version`, but only if the log was written by the installed game
/// version (plan decision 2): the log's version is its first whitespace-separated token,
/// e.g. `1.10.0.0` from `1.10.0.0 b34141`.
pub fn current_desc_version(log: &LogSummary, installed: Option<&str>) -> Option<u32> {
    let logged = log.game_version.as_deref()?.split_whitespace().next()?;
    let installed = installed?.trim();
    if logged == installed {
        log.mod_desc_version
    } else {
        None
    }
}

/// The folder the game reads mods from: the override directory if active and set, else
/// `<user folder>\mods`. A relative override is taken relative to the user folder.
pub fn effective_mods_dir(
    user_dir: Option<&Path>,
    mods_override: Option<&ModsOverride>,
) -> Option<PathBuf> {
    if let Some(ModsOverride {
        active: true,
        directory: Some(dir),
    }) = mods_override
    {
        return Some(match user_dir {
            Some(user) if dir.is_relative() => user.join(dir),
            _ => dir.clone(),
        });
    }
    user_dir.map(|u| u.join("mods"))
}

/// Folders worth offering as profiles: `mods` plus direct subfolders of the user folder
/// that contain at least one mod.
pub fn suggest_profiles(user_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mods = user_dir.join("mods");
    if mods.is_dir() {
        out.push(mods);
    }
    let Ok(entries) = fs::read_dir(user_dir) else {
        return out;
    };
    let mut folders: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .filter(|(name, path)| !name.eq_ignore_ascii_case("mods") && holds_a_mod(path))
        .collect();
    folders.sort_by_key(|(name, _)| name.to_lowercase());
    out.extend(folders.into_iter().map(|(_, path)| path));
    out
}

/// True at the first `*.zip` file or subfolder with a `modDesc.xml`.
fn holds_a_mod(folder: &Path) -> bool {
    let Ok(entries) = fs::read_dir(folder) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|e| {
        let path = e.path();
        if path.is_dir() {
            path.join("modDesc.xml").is_file()
        } else {
            path.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("zip"))
        }
    })
}

/// Assembles the frontend's [`GameStatus`]. `logged_desc_version` is only set when the
/// log was written by the installed game version (plan decision 2).
pub fn game_status(probe: &dyn SystemProbe) -> GameStatus {
    let game_version = find_installs(probe)
        .into_iter()
        .next()
        .and_then(|i| i.version);
    let run = running(probe);
    let user = user_dir(probe);
    let mods_override = user
        .as_ref()
        .and_then(|u| read_mods_override(&u.join("gameSettings.xml")).ok());
    let log = user
        .as_ref()
        .and_then(|u| read_log_summary(&u.join("log.txt")).ok());
    GameStatus {
        logged_desc_version: log
            .as_ref()
            .and_then(|l| current_desc_version(l, game_version.as_deref())),
        game_version,
        game_running: run.game,
        server_running: run.server,
        settings_readable: mods_override.is_some(),
        override_active: mods_override.as_ref().is_some_and(|o| o.active),
        effective_mods_dir: effective_mods_dir(user.as_deref(), mods_override.as_ref())
            .map(|p| p.to_string_lossy().into_owned()),
    }
}

#[cfg(test)]
mod tests;
