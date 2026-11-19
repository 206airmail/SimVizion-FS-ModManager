//! Steam: registry path, `libraryfolders.vdf`, `appmanifest_<id>.acf`.

use std::fs;
use std::path::{Component, Path, PathBuf};

use keyvalues_parser::Value;

use super::{Hive, SystemProbe, STEAM_APP_ID};

const USER_KEY: &str = r"Software\Valve\Steam";
const MACHINE_KEY: &str = r"SOFTWARE\WOW6432Node\Valve\Steam";

/// Candidate game folders in every Steam library. Not yet validated against the game exe.
pub(super) fn candidates(probe: &dyn SystemProbe) -> Vec<PathBuf> {
    let Some(root) = steam_root(probe) else {
        return Vec::new();
    };
    let mut libraries = vec![root.clone()];
    for lib in read_libraries(&root.join("steamapps").join("libraryfolders.vdf")) {
        if !libraries.contains(&lib) {
            libraries.push(lib);
        }
    }
    libraries
        .iter()
        .filter_map(|lib| game_dir_in_library(lib))
        .collect()
}

fn steam_root(probe: &dyn SystemProbe) -> Option<PathBuf> {
    probe
        .registry_string(Hive::CurrentUser, USER_KEY, "SteamPath")
        .or_else(|| probe.registry_string(Hive::LocalMachine, MACHINE_KEY, "InstallPath"))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// The `path` of each library listed in `libraryfolders.vdf`; empty if unreadable.
fn read_libraries(vdf: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(vdf) else {
        return Vec::new();
    };
    let Ok(parsed) = keyvalues_parser::parse(&text) else {
        return Vec::new();
    };
    let Some(folders) = parsed.value.get_obj() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (key, values) in folders.iter() {
        // Entries are keyed "0", "1", ...
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        for value in values {
            let path = match value {
                Value::Obj(lib) => lib
                    .get("path")
                    .and_then(|v| v.first())
                    .and_then(Value::get_str),
                // The pre-2021 format stored the path directly.
                Value::Str(s) => Some(s.as_ref()),
            };
            if let Some(p) = path.filter(|p| !p.is_empty()) {
                out.push(PathBuf::from(p));
            }
        }
    }
    out
}

/// `<library>\steamapps\common\<installdir>` per the app manifest, if the manifest exists.
fn game_dir_in_library(library: &Path) -> Option<PathBuf> {
    let steamapps = library.join("steamapps");
    let text =
        fs::read_to_string(steamapps.join(format!("appmanifest_{STEAM_APP_ID}.acf"))).ok()?;
    let parsed = keyvalues_parser::parse(&text).ok()?;
    let installdir = parsed
        .value
        .get_obj()?
        .get("installdir")?
        .first()?
        .get_str()?
        .to_owned();
    // The folder name must be a single plain component, never a path that escapes `common`.
    let mut components = Path::new(&installdir).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Some(steamapps.join("common").join(installdir)),
        _ => None,
    }
}
