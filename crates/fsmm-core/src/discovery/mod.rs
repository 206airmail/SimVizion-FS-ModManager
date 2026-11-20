//! Finding the game, the user folder and the effective mods folder, and reading the
//! mods-folder override and the game log. Read-only.
//!
//! Platform calls go through [`SystemProbe`] so the logic can be tested with a fake probe
//! and temp directories. Nothing here may log or return the text of `gameSettings.xml`:
//! it holds plain-text multiplayer passwords.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::container::ModContainer;
use crate::model::{ApiError, ErrorCode, GameStatus, GameStore, UserDirSource};

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
    /// Executable file names of running processes, e.g. `dedicatedServer.exe`; `None` when the
    /// list could not be read (which is not the same as an empty list).
    fn process_names(&self) -> Option<Vec<String>>;
    /// The Documents known folder, honouring redirection.
    fn documents_dir(&self) -> Option<PathBuf>;
    /// `%ProgramData%`, where the Epic launcher keeps its manifests.
    fn program_data_dir(&self) -> Option<PathBuf>;
    /// `%LOCALAPPDATA%`, where Microsoft Store apps keep their data (`Packages<package>`).
    fn local_app_data_dir(&self) -> Option<PathBuf>;
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
    /// Plan M8: detected from the package registration; not verified on a real install.
    MicrosoftStore,
}

impl Store {
    pub fn to_model(self) -> GameStore {
        match self {
            Store::Steam => GameStore::Steam,
            Store::Giants => GameStore::Giants,
            Store::Epic => GameStore::Epic,
            Store::MicrosoftStore => GameStore::MicrosoftStore,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameInstall {
    pub store: Store,
    pub dir: PathBuf,
    /// Contents of the install's `VERSION` file, trimmed.
    pub version: Option<String>,
    /// What the store launches the game by: Epic's `AppName`, or the Microsoft Store app's
    /// `<PackageFamilyName>!<AppId>`. `None` for Steam and GIANTS.
    pub launch_id: Option<String>,
}

/// How "Launch game" starts an install (plan M8). Built here from the detected install; the
/// frontend never sends a URL or a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchTarget {
    /// A store URL opened with the default handler: `steam://rungameid/2300320`, or Epic's
    /// `com.epicgames.launcher://apps/<AppName>?action=launch&silent=true`.
    Url(String),
    /// The game's exe, started with its install folder as the working directory (GIANTS).
    Program { exe: PathBuf, dir: PathBuf },
    /// A Microsoft Store app, started as `explorer.exe shell:AppsFolder<id>`.
    AppsFolder(String),
}

/// What "Launch game" would start: the first install found. `GameRunning` while the game runs,
/// `NotLaunchable` when nothing was found (or the install lacks what its store needs).
pub fn launch_target(probe: &dyn SystemProbe) -> Result<LaunchTarget, ApiError> {
    if running(probe).game {
        return Err(ApiError::new(ErrorCode::GameRunning, None));
    }
    find_installs(probe)
        .first()
        .and_then(target_for)
        .ok_or_else(|| ApiError::new(ErrorCode::NotLaunchable, None))
}

/// How an install is started, or `None` when it lacks what its store needs. The ids that end
/// up in a URL or a shell path are restricted to a safe alphabet, so nothing read from a
/// manifest or the registry can add to them.
fn target_for(install: &GameInstall) -> Option<LaunchTarget> {
    let id = install.launch_id.as_deref();
    match install.store {
        Store::Steam => Some(LaunchTarget::Url(format!(
            "steam://rungameid/{STEAM_APP_ID}"
        ))),
        Store::Epic => {
            let app_name = id.filter(|id| safe_id(id, |c| matches!(c, '.' | '_' | '-')))?;
            Some(LaunchTarget::Url(format!(
                "com.epicgames.launcher://apps/{app_name}?action=launch&silent=true"
            )))
        }
        Store::Giants => Some(LaunchTarget::Program {
            exe: install.dir.join(GAME_EXE),
            dir: install.dir.clone(),
        }),
        Store::MicrosoftStore => id
            .filter(|id| safe_id(id, |c| matches!(c, '.' | '_' | '!' | '-')))
            .map(|id| LaunchTarget::AppsFolder(id.to_owned())),
    }
}

/// Non-empty, ASCII letters and digits plus the characters `extra` accepts.
fn safe_id(id: &str, extra: fn(char) -> bool) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || extra(c))
}

/// Whether a folder the user chose can be the game's user folder: it holds
/// `gameSettings.xml` or a `mods` folder.
pub fn valid_game_folder(dir: &Path) -> bool {
    dir.join("gameSettings.xml").is_file() || dir.join("mods").is_dir()
}

/// Whether the game and the dedicated server run. When the process list could not be read
/// (`unknown`), `game` and `server` are best-effort `false`; a write guard must then refuse
/// (`ops::check_not_running`), while a display or a launch may carry on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Running {
    pub game: bool,
    pub server: bool,
    pub unknown: bool,
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

/// Every install found. Steam, GIANTS and Epic installs are validated by the presence of
/// [`GAME_EXE`]; a Microsoft Store install only needs its package folder, because WindowsApps
/// is often unreadable. Steam installs come first, then GIANTS, Epic and the Microsoft Store;
/// the same folder is listed once.
pub fn find_installs(probe: &dyn SystemProbe) -> Vec<GameInstall> {
    let mut candidates: Vec<(Store, PathBuf, Option<String>)> = steam::candidates(probe)
        .into_iter()
        .map(|dir| (Store::Steam, dir, None))
        .collect();
    candidates.extend(
        giants_candidates(probe)
            .into_iter()
            .map(|d| (Store::Giants, d, None)),
    );
    candidates.extend(
        epic_candidates(probe)
            .into_iter()
            .map(|(d, app_name)| (Store::Epic, d, app_name)),
    );
    candidates.extend(
        store_candidates(probe)
            .into_iter()
            .map(|(d, id)| (Store::MicrosoftStore, d, Some(id))),
    );

    let mut seen: Vec<String> = Vec::new();
    let mut installs = Vec::new();
    for (store, dir, launch_id) in candidates {
        let key = path_key(&dir);
        let present = match store {
            Store::MicrosoftStore => dir.is_dir(),
            _ => dir.join(GAME_EXE).is_file(),
        };
        if seen.contains(&key) || !present {
            continue;
        }
        seen.push(key);
        let version = read_version(&dir);
        installs.push(GameInstall {
            store,
            dir,
            version,
            launch_id,
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

/// `InstallLocation` and `AppName` of the Epic launcher manifests that name the game.
fn epic_candidates(probe: &dyn SystemProbe) -> Vec<(PathBuf, Option<String>)> {
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
            let app_name = json
                .get("AppName")
                .and_then(|v| v.as_str())
                .map(|n| n.trim().to_owned());
            out.push((PathBuf::from(loc.trim()), app_name));
        }
    }
    out
}

/// The Microsoft Store package registrations, below HKCU.
const PACKAGES_KEY: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";
/// Package name of the Microsoft Store version (unverified: no real install was available).
const STORE_PACKAGE_PREFIX: &str = "GIANTSSoftware.FarmingSimulator25PC_";
/// The `Application` id used when the package's manifest cannot be read.
const STORE_FALLBACK_APP_ID: &str = "App";

/// Package folders and `<PackageFamilyName>!<AppId>` of the Microsoft Store version, from its
/// package registration. Unverified on a real install: only fixtures cover it.
///
/// A package full name is `Name_Version_Arch_ResourceId_PublisherId` and the family name is
/// `Name_PublisherId`. The app id is the `Id` of the first `<Application>` in
/// `AppxManifest.xml`, or `App` when that file cannot be read.
fn store_candidates(probe: &dyn SystemProbe) -> Vec<(PathBuf, String)> {
    let mut names: Vec<String> = probe
        .registry_subkeys(Hive::CurrentUser, PACKAGES_KEY)
        .into_iter()
        .filter(|n| has_prefix_ignore_case(n, STORE_PACKAGE_PREFIX))
        .collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let parts: Vec<&str> = name.split('_').collect();
        let (Some(first), Some(publisher)) = (parts.first(), parts.last()) else {
            continue;
        };
        if parts.len() < 5 {
            continue;
        }
        let key = format!(r"{PACKAGES_KEY}\{name}");
        let Some(root) = probe.registry_string(Hive::CurrentUser, &key, "PackageRootFolder") else {
            continue;
        };
        let root = root.trim().trim_matches('"').trim();
        if root.is_empty() {
            continue;
        }
        let root = PathBuf::from(root);
        let app_id = manifest_app_id(&root).unwrap_or_else(|| STORE_FALLBACK_APP_ID.to_owned());
        out.push((root, format!("{first}_{publisher}!{app_id}")));
    }
    out
}

/// The `Id` of the first `<Application>` in `<root>\AppxManifest.xml`, if readable.
fn manifest_app_id(root: &Path) -> Option<String> {
    let bytes = fs::read(root.join("AppxManifest.xml")).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    let doc = roxmltree::Document::parse(text).ok()?;
    doc.descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "Application")
        .and_then(|n| n.attribute("Id"))
        .map(str::to_owned)
        .filter(|id| !id.is_empty())
}

fn has_prefix_ignore_case(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
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
    let Some(names) = probe.process_names() else {
        return Running {
            unknown: true,
            ..Running::default()
        };
    };
    let has = |wanted: &[&str]| {
        names
            .iter()
            .any(|n| wanted.iter().any(|w| w.eq_ignore_ascii_case(n)))
    };
    Running {
        game: has(GAME_PROCESS_NAMES),
        server: has(&[SERVER_PROCESS_NAME]),
        unknown: false,
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

/// True at the first subfolder or `*.zip` file with a `modDesc.xml` at its root. Other zips
/// do not count: the GIANTS debugger keeps a `gameSource.zip` in its own folder.
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
                && ModContainer::open(&path).is_ok_and(|c| c.find("modDesc.xml").is_some())
        }
    })
}

/// Assembles the frontend's [`GameStatus`]. `logged_desc_version` is only set when the
/// log was written by the installed game version (plan decision 2).
/// `chosen` is the user folder the user picked (plan M8); it wins over detection.
pub fn game_status(probe: &dyn SystemProbe, chosen: Option<&Path>) -> GameStatus {
    let install = find_installs(probe).into_iter().next();
    let store = install.as_ref().map(|i| i.store.to_model());
    let launchable = install.as_ref().and_then(target_for).is_some();
    let game_version = install.and_then(|i| i.version);
    let run = running(probe);
    let (user, user_dir_source) = resolve_user_dir(probe, chosen);
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
        store,
        launchable,
        user_dir_source,
        user_dir: user.map(|u| u.to_string_lossy().into_owned()),
    }
}

/// The user folder and how it was found: the chosen one, else detection ([`user_dir`]), else
/// the Microsoft Store package's `LocalCache\Local` folder (unverified on a real install).
fn resolve_user_dir(
    probe: &dyn SystemProbe,
    chosen: Option<&Path>,
) -> (Option<PathBuf>, UserDirSource) {
    if let Some(dir) = chosen {
        return (Some(dir.to_path_buf()), UserDirSource::Chosen);
    }
    match user_dir(probe).or_else(|| store_user_dir(probe)) {
        Some(dir) => (Some(dir), UserDirSource::Detected),
        None => (None, UserDirSource::None),
    }
}

/// `%LOCALAPPDATA%\Packages\GIANTSSoftware.FarmingSimulator25PC_<id>\LocalCache\Local`, if it
/// exists. When several packages match, the first by path.
fn store_user_dir(probe: &dyn SystemProbe) -> Option<PathBuf> {
    let packages = probe.local_app_data_dir()?.join("Packages");
    let mut found: Vec<PathBuf> = fs::read_dir(packages)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| has_prefix_ignore_case(&e.file_name().to_string_lossy(), STORE_PACKAGE_PREFIX))
        .map(|e| e.path().join("LocalCache").join("Local"))
        .filter(|p| p.is_dir())
        .collect();
    found.sort();
    found.into_iter().next()
}

/// What the library needs to know about the game to scan and label profiles.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameContext {
    pub user_dir: Option<PathBuf>,
    /// The first game install found; its `pdlc` folder holds the installed DLCs.
    pub install_dir: Option<PathBuf>,
    /// The folder the game currently loads mods from.
    pub effective_mods_dir: Option<PathBuf>,
    /// `<user folder>\mods`, used when no override is active.
    pub default_mods_dir: Option<PathBuf>,
    /// Preferred languages for titles and descriptions, most preferred first.
    pub languages: Vec<String>,
    /// Plan M2 decision 2: only set when the log matches the installed game.
    pub logged_desc_version: Option<u32>,
}

/// Gathers the [`GameContext`]. Reads `gameSettings.xml` (only the override element) and the
/// head of `log.txt`; the language comes from the log and falls back to `en`. `chosen` as in
/// [`game_status`].
pub fn game_context(probe: &dyn SystemProbe, chosen: Option<&Path>) -> GameContext {
    let install = find_installs(probe).into_iter().next();
    let install_dir = install.as_ref().map(|i| i.dir.clone());
    let installed = install.and_then(|i| i.version);
    let (user, _) = resolve_user_dir(probe, chosen);
    let mods_override = user
        .as_ref()
        .and_then(|u| read_mods_override(&u.join("gameSettings.xml")).ok());
    let log = user
        .as_ref()
        .and_then(|u| read_log_summary(&u.join("log.txt")).ok());
    let language = log
        .as_ref()
        .and_then(|l| l.language.clone())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| "en".to_string());
    GameContext {
        effective_mods_dir: effective_mods_dir(user.as_deref(), mods_override.as_ref()),
        default_mods_dir: user.as_ref().map(|u| u.join("mods")),
        languages: vec![language],
        logged_desc_version: log
            .as_ref()
            .and_then(|l| current_desc_version(l, installed.as_deref())),
        user_dir: user,
        install_dir,
    }
}

#[cfg(test)]
mod tests;
