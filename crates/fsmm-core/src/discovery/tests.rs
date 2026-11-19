//! Discovery tests: a fake probe plus generated fixtures in temp directories. Nothing here
//! touches a real Farming Simulator, Steam or Epic folder or the real registry.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;
use crate::testutil::{write_zip, Method};

const STEAM_USER_KEY: &str = r"Software\Valve\Steam";
const STEAM_MACHINE_KEY: &str = r"SOFTWARE\WOW6432Node\Valve\Steam";
const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
const BOM: &str = "\u{feff}";

#[derive(Default)]
struct FakeProbe {
    /// (hive, full key path, value name, data)
    values: Vec<(Hive, String, String, String)>,
    processes: Vec<String>,
    documents: Option<PathBuf>,
    program_data: Option<PathBuf>,
}

impl FakeProbe {
    fn set(&mut self, hive: Hive, key: &str, value: &str, data: &str) {
        self.values
            .push((hive, key.into(), value.into(), data.into()));
    }
}

impl SystemProbe for FakeProbe {
    fn registry_string(&self, hive: Hive, key: &str, value: &str) -> Option<String> {
        self.values
            .iter()
            .find(|(h, k, v, _)| *h == hive && k.eq_ignore_ascii_case(key) && v == value)
            .map(|(_, _, _, d)| d.clone())
    }
    fn registry_subkeys(&self, hive: Hive, key: &str) -> Vec<String> {
        let prefix = format!(r"{}\", key.to_lowercase());
        let mut out: Vec<String> = Vec::new();
        for (h, k, _, _) in &self.values {
            if *h != hive || !k.to_lowercase().starts_with(&prefix) {
                continue;
            }
            let sub = k[prefix.len()..]
                .split('\\')
                .next()
                .unwrap_or("")
                .to_owned();
            if !sub.is_empty() && !out.contains(&sub) {
                out.push(sub);
            }
        }
        out
    }
    fn process_names(&self) -> Vec<String> {
        self.processes.clone()
    }
    fn documents_dir(&self) -> Option<PathBuf> {
        self.documents.clone()
    }
    fn program_data_dir(&self) -> Option<PathBuf> {
        self.program_data.clone()
    }
    fn local_app_data_dir(&self) -> Option<PathBuf> {
        None
    }
}

/// A game folder with the exe and, if given, a `VERSION` file.
fn make_install(dir: &Path, version: Option<&str>) {
    fs::create_dir_all(dir.join("x64")).unwrap();
    fs::write(dir.join(GAME_EXE), b"").unwrap();
    fs::write(dir.join("dedicatedServer.exe"), b"").unwrap();
    if let Some(v) = version {
        fs::write(dir.join("VERSION"), v).unwrap();
    }
}

/// VDF string value: backslashes are escaped.
fn vdf(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "\\\\")
}

fn write_libraryfolders(steam: &Path, libraries: &[&Path]) {
    let mut text = String::from("\"libraryfolders\"\n{\n");
    for (i, lib) in libraries.iter().enumerate() {
        text += &format!(
            "\t\"{i}\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t\t\"label\"\t\t\"\"\n\t\t\"apps\"\n\t\t{{\n\t\t\t\"2300320\"\t\t\"123\"\n\t\t}}\n\t}}\n",
            vdf(lib)
        );
    }
    text += "}\n";
    fs::create_dir_all(steam.join("steamapps")).unwrap();
    fs::write(steam.join("steamapps").join("libraryfolders.vdf"), text).unwrap();
}

fn write_appmanifest(library: &Path, installdir: &str) {
    let steamapps = library.join("steamapps");
    fs::create_dir_all(&steamapps).unwrap();
    let text = format!(
        "\"AppState\"\n{{\n\t\"appid\"\t\t\"2300320\"\n\t\"name\"\t\t\"Farming Simulator 25\"\n\t\"installdir\"\t\t\"{installdir}\"\n}}\n"
    );
    fs::write(steamapps.join("appmanifest_2300320.acf"), text).unwrap();
}

/// Steam root with the game in `steamapps\common\<name>` of the same library.
fn steam_fixture(tmp: &TempDir) -> (FakeProbe, PathBuf) {
    let steam = tmp.path().join("Steam");
    write_libraryfolders(&steam, &[&steam]);
    write_appmanifest(&steam, "Farming Simulator 25");
    let game = steam.join("steamapps/common/Farming Simulator 25");
    make_install(&game, Some("1.25.1.0\r\n"));
    let mut probe = FakeProbe::default();
    // The registry stores the Steam path with forward slashes.
    probe.set(
        Hive::CurrentUser,
        STEAM_USER_KEY,
        "SteamPath",
        &steam.to_string_lossy().replace('\\', "/"),
    );
    (probe, game)
}

fn same_path(a: &Path, b: &Path) -> bool {
    path_key(a) == path_key(b)
}

// ---- installs ----

#[test]
fn steam_via_current_user_registry() {
    let tmp = tempfile::tempdir().unwrap();
    let (probe, game) = steam_fixture(&tmp);
    let installs = find_installs(&probe);
    assert_eq!(installs.len(), 1);
    assert_eq!(installs[0].store, Store::Steam);
    assert!(same_path(&installs[0].dir, &game));
    assert_eq!(installs[0].version.as_deref(), Some("1.25.1.0"));
}

#[test]
fn steam_falls_back_to_the_machine_registry() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut probe, game) = steam_fixture(&tmp);
    let steam = probe.values.remove(0).3;
    probe.set(Hive::LocalMachine, STEAM_MACHINE_KEY, "InstallPath", &steam);
    let installs = find_installs(&probe);
    assert_eq!(installs.len(), 1);
    assert!(same_path(&installs[0].dir, &game));
}

#[test]
fn steam_game_in_the_second_library() {
    let tmp = tempfile::tempdir().unwrap();
    let steam = tmp.path().join("Steam");
    let second = tmp.path().join("Games").join("SteamLibrary");
    write_libraryfolders(&steam, &[&steam, &second]);
    write_appmanifest(&second, "FS25");
    let game = second.join("steamapps/common/FS25");
    make_install(&game, Some("1.25.1.0"));
    let mut probe = FakeProbe::default();
    probe.set(
        Hive::CurrentUser,
        STEAM_USER_KEY,
        "SteamPath",
        &steam.to_string_lossy().replace('\\', "/"),
    );
    let installs = find_installs(&probe);
    assert_eq!(installs.len(), 1);
    assert!(same_path(&installs[0].dir, &game));
}

#[test]
fn steam_without_an_appmanifest_finds_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let steam = tmp.path().join("Steam");
    write_libraryfolders(&steam, &[&steam]);
    make_install(&steam.join("steamapps/common/Farming Simulator 25"), None);
    let mut probe = FakeProbe::default();
    probe.set(
        Hive::CurrentUser,
        STEAM_USER_KEY,
        "SteamPath",
        &steam.to_string_lossy(),
    );
    assert!(find_installs(&probe).is_empty());
}

#[test]
fn steam_without_libraryfolders_still_checks_the_root_library() {
    let tmp = tempfile::tempdir().unwrap();
    let (probe, _) = steam_fixture(&tmp);
    let vdf_file = tmp.path().join("Steam/steamapps/libraryfolders.vdf");
    fs::remove_file(vdf_file).unwrap();
    assert_eq!(find_installs(&probe).len(), 1);
}

#[test]
fn steam_installdir_cannot_escape_the_common_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let steam = tmp.path().join("Steam");
    write_libraryfolders(&steam, &[&steam]);
    write_appmanifest(&steam, "..\\\\..\\\\elsewhere");
    make_install(&tmp.path().join("elsewhere"), None);
    let mut probe = FakeProbe::default();
    probe.set(
        Hive::CurrentUser,
        STEAM_USER_KEY,
        "SteamPath",
        &steam.to_string_lossy(),
    );
    assert!(find_installs(&probe).is_empty());
}

#[test]
fn install_without_the_exe_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let (probe, game) = steam_fixture(&tmp);
    fs::remove_file(game.join(GAME_EXE)).unwrap();
    assert!(find_installs(&probe).is_empty());
}

#[test]
fn giants_install_via_an_uninstall_key() {
    let tmp = tempfile::tempdir().unwrap();
    let game = tmp
        .path()
        .join("GIANTS Software")
        .join("FarmingSimulator2025");
    make_install(&game, Some("  1.25.0.0\n"));
    let mut probe = FakeProbe::default();
    let key = format!(r"{UNINSTALL_32}\{{ABC-123}}");
    probe.set(
        Hive::LocalMachine,
        &key,
        "DisplayName",
        "FARMING SIMULATOR 25",
    );
    probe.set(
        Hive::LocalMachine,
        &key,
        "InstallLocation",
        &format!("\"{}\\\"", game.to_string_lossy()),
    );
    // Unrelated products and keys without a location are ignored.
    probe.set(
        Hive::LocalMachine,
        &format!(r"{UNINSTALL}\Other"),
        "DisplayName",
        "Some Other App",
    );
    probe.set(
        Hive::CurrentUser,
        &format!(r"{UNINSTALL}\NoLocation"),
        "DisplayName",
        "Farming Simulator 25",
    );
    let installs = find_installs(&probe);
    assert_eq!(installs.len(), 1);
    assert_eq!(installs[0].store, Store::Giants);
    assert!(same_path(&installs[0].dir, &game));
    assert_eq!(installs[0].version.as_deref(), Some("1.25.0.0"));
}

#[test]
fn the_steam_uninstall_key_is_not_a_giants_install() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut probe, game) = steam_fixture(&tmp);
    let key = format!(r"{UNINSTALL}\Steam App 2300320");
    probe.set(
        Hive::LocalMachine,
        &key,
        "DisplayName",
        "Farming Simulator 25",
    );
    probe.set(
        Hive::LocalMachine,
        &key,
        "InstallLocation",
        &tmp.path().join("nowhere").to_string_lossy(),
    );
    let installs = find_installs(&probe);
    assert_eq!(installs.len(), 1);
    assert_eq!(installs[0].store, Store::Steam);
    assert!(same_path(&installs[0].dir, &game));
}

fn write_epic_manifest(program_data: &Path, name: &str, display: &str, location: &Path) {
    let dir = program_data.join("Epic/EpicGamesLauncher/Data/Manifests");
    fs::create_dir_all(&dir).unwrap();
    let json = serde_json::json!({
        "DisplayName": display,
        "InstallLocation": location.to_string_lossy(),
    });
    fs::write(dir.join(name), json.to_string()).unwrap();
}

#[test]
fn epic_install_via_an_item_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let program_data = tmp.path().join("ProgramData");
    let game = tmp.path().join("Epic Games").join("FarmingSimulator25");
    make_install(&game, Some("1.25.1.0"));
    let other = tmp.path().join("Epic Games").join("Other");
    make_install(&other, Some("9"));
    write_epic_manifest(&program_data, "AAA.item", "Another Game", &other);
    write_epic_manifest(&program_data, "BBB.item", "Farming Simulator 25", &game);
    fs::write(
        program_data.join("Epic/EpicGamesLauncher/Data/Manifests/broken.item"),
        "{ not json",
    )
    .unwrap();
    let probe = FakeProbe {
        program_data: Some(program_data),
        ..FakeProbe::default()
    };
    let installs = find_installs(&probe);
    assert_eq!(installs.len(), 1);
    assert_eq!(installs[0].store, Store::Epic);
    assert!(same_path(&installs[0].dir, &game));
}

#[test]
fn the_same_folder_from_several_stores_is_listed_once_steam_first() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut probe, game) = steam_fixture(&tmp);
    // GIANTS entry: same folder, spelled with a trailing separator and different case.
    let key = format!(r"{UNINSTALL}\{{GUID}}");
    probe.set(
        Hive::LocalMachine,
        &key,
        "DisplayName",
        "Farming Simulator 25",
    );
    probe.set(
        Hive::LocalMachine,
        &key,
        "InstallLocation",
        &format!("{}\\", game.to_string_lossy().to_uppercase()),
    );
    // Epic manifest: same folder with forward slashes.
    let program_data = tmp.path().join("ProgramData");
    write_epic_manifest(
        &program_data,
        "X.item",
        "Farming Simulator 25",
        Path::new(&game.to_string_lossy().replace('\\', "/")),
    );
    probe.program_data = Some(program_data);
    let installs = find_installs(&probe);
    assert_eq!(installs.len(), 1);
    assert_eq!(installs[0].store, Store::Steam);
}

#[test]
fn path_keys_ignore_case_separators_and_trailing_slash() {
    assert_eq!(
        path_key(Path::new(r"D:\Games\FS25\")),
        path_key(Path::new("d:/games/fs25"))
    );
}

#[test]
fn version_is_trimmed_and_missing_or_empty_is_none() {
    let tmp = tempfile::tempdir().unwrap();
    make_install(tmp.path(), Some("\u{feff} 1.25.1.0 \r\n"));
    assert_eq!(read_version(tmp.path()).as_deref(), Some("1.25.1.0"));
    fs::write(tmp.path().join("VERSION"), "  \n").unwrap();
    assert_eq!(read_version(tmp.path()), None);
    fs::remove_file(tmp.path().join("VERSION")).unwrap();
    assert_eq!(read_version(tmp.path()), None);
}

// ---- gameSettings.xml ----

fn write_settings(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("gameSettings.xml");
    fs::write(&path, body).unwrap();
    path
}

#[test]
fn override_active_with_a_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let path = write_settings(
        tmp.path(),
        &format!(
            r#"{BOM}<?xml version="1.0" encoding="utf-8" standalone="no"?>
<gameSettings>
  <game><mpPassword>hunter2</mpPassword></game>
  <modsDirectoryOverride active="true" directory="D:\Mods\$testingMods"/>
</gameSettings>"#
        ),
    );
    assert_eq!(
        read_mods_override(&path).unwrap(),
        ModsOverride {
            active: true,
            directory: Some(PathBuf::from(r"D:\Mods\$testingMods")),
        }
    );
}

#[test]
fn override_inactive_keeps_its_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let path = write_settings(
        tmp.path(),
        r#"<gameSettings><modsDirectoryOverride active="false" directory="E:\m"/></gameSettings>"#,
    );
    let got = read_mods_override(&path).unwrap();
    assert!(!got.active);
    assert_eq!(got.directory, Some(PathBuf::from(r"E:\m")));
}

#[test]
fn override_element_missing_or_directory_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let path = write_settings(tmp.path(), "<gameSettings><other/></gameSettings>");
    assert_eq!(read_mods_override(&path).unwrap(), ModsOverride::default());
    let path = write_settings(
        tmp.path(),
        r#"<gameSettings><modsDirectoryOverride active="true" directory=""/></gameSettings>"#,
    );
    assert_eq!(
        read_mods_override(&path).unwrap(),
        ModsOverride {
            active: true,
            directory: None
        }
    );
}

#[test]
fn settings_file_missing_is_an_io_error() {
    let tmp = tempfile::tempdir().unwrap();
    let err = read_mods_override(&tmp.path().join("gameSettings.xml")).unwrap_err();
    assert!(matches!(err, SettingsError::Io(io::ErrorKind::NotFound)));
}

#[test]
fn malformed_settings_never_leak_the_file_text() {
    let tmp = tempfile::tempdir().unwrap();
    // Mismatched closing tag: the XML parser's own message would quote element names.
    let path = write_settings(
        tmp.path(),
        &format!(
            "{BOM}<gameSettings>\n<multiplayer><password>hunter2-SECRET</password>\n\
             <modsDirectoryOverride active=\"true\" directory=\"D:\\private\\place\"/>\n\
             </wrongTag>\n</gameSettings>"
        ),
    );
    let err = read_mods_override(&path).unwrap_err();
    assert!(matches!(err, SettingsError::Malformed { line: 4, .. }));
    for text in [format!("{err}"), format!("{err:?}")] {
        for secret in [
            "hunter2",
            "SECRET",
            "password",
            "wrongTag",
            "gameSettings>",
            "modsDirectoryOverride",
            "private",
        ] {
            assert!(!text.contains(secret), "leaked {secret:?} in {text:?}");
        }
    }
}

#[test]
fn truncated_settings_are_malformed_not_a_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let path = write_settings(tmp.path(), "<gameSettings><password>abc");
    assert!(matches!(
        read_mods_override(&path),
        Err(SettingsError::Malformed { .. })
    ));
    let path = write_settings(tmp.path(), "");
    assert!(matches!(
        read_mods_override(&path),
        Err(SettingsError::Malformed { .. })
    ));
}

// ---- log.txt ----

const STARTUP_LOG: &str = "\
Some banner line
  Version: 6.3.7
  Other thing: 1
Library X
  Version: 2.0
Farming Simulator 25 (Steam)
  Version: 1.10.0.0 b34141
  Available Languages: en de jp
  Language: en
  Time: 2025-08-31 22:45:39
  ModDesc Version: 100
  Mod Directory: D:/Users/x/$testingMods/

Later section
  Version: 7.7.7
  ModDesc Version: 5
";

#[test]
fn log_summary_reads_only_the_startup_block() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("log.txt");
    fs::write(&log, STARTUP_LOG).unwrap();
    assert_eq!(
        read_log_summary(&log).unwrap(),
        LogSummary {
            game_version: Some("1.10.0.0 b34141".into()),
            mod_desc_version: Some(100),
            language: Some("en".into()),
        }
    );
}

#[test]
fn log_without_the_block_gives_an_empty_summary() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("log.txt");
    fs::write(&log, "banner\n  Version: 6.3.7\n  ModDesc Version: 9\n").unwrap();
    assert_eq!(read_log_summary(&log).unwrap(), LogSummary::default());
}

#[test]
fn log_tolerates_bom_crlf_and_invalid_utf8() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("log.txt");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(BOM.as_bytes());
    bytes.extend_from_slice(b"Farming Simulator 25 (Steam)\r\n  Version: 1.25.1.0 b1\r\n");
    bytes.extend_from_slice(b"  Mod Directory: C:/caf\xe9/\r\n  ModDesc Version: 105\r\n");
    fs::write(&log, bytes).unwrap();
    let summary = read_log_summary(&log).unwrap();
    assert_eq!(summary.game_version.as_deref(), Some("1.25.1.0 b1"));
    assert_eq!(summary.mod_desc_version, Some(105));
    assert_eq!(summary.language, None);
}

#[test]
fn missing_log_is_an_io_error() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(read_log_summary(&tmp.path().join("log.txt")).is_err());
}

// ---- decision 2 ----

fn summary(version: Option<&str>, desc: Option<u32>) -> LogSummary {
    LogSummary {
        game_version: version.map(Into::into),
        mod_desc_version: desc,
        language: None,
    }
}

#[test]
fn desc_version_is_only_trusted_when_the_log_matches_the_install() {
    let log = summary(Some("1.25.1.0 b34141"), Some(105));
    assert_eq!(current_desc_version(&log, Some("1.25.1.0")), Some(105));
    assert_eq!(current_desc_version(&log, Some(" 1.25.1.0\n")), Some(105));
    // Stale log from an older game version.
    let stale = summary(Some("1.10.0.0 b34141"), Some(100));
    assert_eq!(current_desc_version(&stale, Some("1.25.1.0")), None);
    // Missing pieces.
    assert_eq!(current_desc_version(&log, None), None);
    assert_eq!(
        current_desc_version(&summary(None, Some(105)), Some("1.25.1.0")),
        None
    );
    assert_eq!(
        current_desc_version(&summary(Some("1.25.1.0"), None), Some("1.25.1.0")),
        None
    );
    // A prefix of the version is not a match.
    assert_eq!(
        current_desc_version(&summary(Some("1.25.1"), Some(1)), Some("1.25.1.0")),
        None
    );
}

// ---- running ----

#[test]
fn running_matches_names_case_insensitively() {
    let mut probe = FakeProbe::default();
    assert_eq!(running(&probe), Running::default());
    probe.processes = vec!["explorer.exe".into(), "farmingsimulator2025game.EXE".into()];
    assert_eq!(
        running(&probe),
        Running {
            game: true,
            server: false
        }
    );
    probe.processes = vec![
        "DEDICATEDSERVER.exe".into(),
        "FarmingSimulator2025.exe".into(),
    ];
    assert_eq!(
        running(&probe),
        Running {
            game: true,
            server: true
        }
    );
    probe.processes = vec!["FarmingSimulator2025Helper.exe".into()];
    assert_eq!(running(&probe), Running::default());
}

#[cfg(windows)]
#[test]
fn windows_probe_lists_running_processes() {
    assert!(!WindowsProbe.process_names().is_empty());
}

// ---- user folder, profiles, status ----

fn fake_user_folder(tmp: &TempDir) -> (FakeProbe, PathBuf) {
    let documents = tmp.path().join("Documents");
    let user = documents.join("My Games").join("FarmingSimulator2025");
    fs::create_dir_all(&user).unwrap();
    let probe = FakeProbe {
        documents: Some(documents),
        ..FakeProbe::default()
    };
    (probe, user)
}

#[test]
fn user_dir_needs_the_folder_to_exist() {
    let tmp = tempfile::tempdir().unwrap();
    let probe = FakeProbe {
        documents: Some(tmp.path().to_path_buf()),
        ..FakeProbe::default()
    };
    assert_eq!(user_dir(&probe), None);
    assert_eq!(user_dir(&FakeProbe::default()), None);
    let (probe, user) = fake_user_folder(&tmp);
    assert_eq!(user_dir(&probe), Some(user));
}

#[test]
fn suggest_profiles_offers_mods_and_folders_that_hold_mods() {
    let tmp = tempfile::tempdir().unwrap();
    let user = tmp.path();
    fs::create_dir_all(user.join("mods")).unwrap();
    // Holds a mod zip.
    fs::create_dir_all(user.join("zeta pack")).unwrap();
    write_zip(
        &user.join("zeta pack/FS25_A.ZIP"),
        &[("modDesc.xml", b"<modDesc/>")],
        Method::Stored,
    );
    // Holds zips that are not mods: the GIANTS debugger's game source, and an unreadable file.
    fs::create_dir_all(user.join("debugger")).unwrap();
    write_zip(
        &user.join("debugger/gameSource.zip"),
        &[("dataS/scripts/main.lua", b"--")],
        Method::Stored,
    );
    fs::write(user.join("debugger/broken.zip"), b"").unwrap();
    // Holds an unpacked mod.
    fs::create_dir_all(user.join("Alpha/FS25_Folder")).unwrap();
    fs::write(user.join("Alpha/FS25_Folder/modDesc.xml"), b"").unwrap();
    // Ordinary game folders and files: no mods.
    fs::create_dir_all(user.join("savegame1")).unwrap();
    fs::write(user.join("savegame1/careerSavegame.xml"), b"").unwrap();
    fs::create_dir_all(user.join("screenshots")).unwrap();
    fs::create_dir_all(user.join("empty/sub")).unwrap();
    fs::write(user.join("log.txt"), b"").unwrap();
    fs::write(user.join("stray.zip"), b"").unwrap();

    assert_eq!(
        suggest_profiles(user),
        vec![
            user.join("mods"),
            user.join("Alpha"),
            user.join("zeta pack")
        ]
    );
}

#[test]
fn suggest_profiles_without_a_mods_folder_or_user_dir() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(suggest_profiles(tmp.path()).is_empty());
    assert!(suggest_profiles(&tmp.path().join("missing")).is_empty());
}

#[test]
fn effective_mods_dir_rules() {
    let user = Path::new(r"C:\u");
    let active = |d: &str| ModsOverride {
        active: true,
        directory: Some(PathBuf::from(d)),
    };
    assert_eq!(
        effective_mods_dir(Some(user), Some(&active(r"D:\mods"))),
        Some(PathBuf::from(r"D:\mods"))
    );
    assert_eq!(
        effective_mods_dir(Some(user), Some(&active(r"sub\mods"))),
        Some(user.join(r"sub\mods"))
    );
    let inactive = ModsOverride {
        active: false,
        directory: Some(PathBuf::from(r"D:\mods")),
    };
    assert_eq!(
        effective_mods_dir(Some(user), Some(&inactive)),
        Some(user.join("mods"))
    );
    assert_eq!(
        effective_mods_dir(Some(user), None),
        Some(user.join("mods"))
    );
    assert_eq!(effective_mods_dir(None, None), None);
    assert_eq!(
        effective_mods_dir(None, Some(&active(r"D:\mods"))),
        Some(PathBuf::from(r"D:\mods"))
    );
}

#[test]
fn game_status_end_to_end_with_a_stale_log() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut probe, _game) = steam_fixture(&tmp);
    let (user_probe, user) = fake_user_folder(&tmp);
    probe.documents = user_probe.documents;
    probe.processes = vec!["FarmingSimulator2025Game.exe".into()];
    fs::write(
        user.join("gameSettings.xml"),
        format!(
            "{BOM}<gameSettings><modsDirectoryOverride active=\"true\" directory=\"{}\"/></gameSettings>",
            tmp.path().join("profile-a").to_string_lossy()
        ),
    )
    .unwrap();
    fs::write(user.join("log.txt"), STARTUP_LOG).unwrap();

    let status = game_status(&probe, None);
    assert_eq!(status.game_version.as_deref(), Some("1.25.1.0"));
    assert!(status.game_running);
    assert!(!status.server_running);
    assert!(status.settings_readable);
    assert!(status.override_active);
    assert_eq!(
        status.effective_mods_dir.as_deref(),
        Some(&*tmp.path().join("profile-a").to_string_lossy())
    );
    // The log is from 1.10.0.0 but 1.25.1.0 is installed.
    assert_eq!(status.logged_desc_version, None);

    // Once the log comes from the installed version, the value is trusted.
    fs::write(
        user.join("log.txt"),
        STARTUP_LOG.replace("1.10.0.0 b34141", "1.25.1.0 b99"),
    )
    .unwrap();
    assert_eq!(game_status(&probe, None).logged_desc_version, Some(100));
}

#[test]
fn game_context_collects_folders_language_and_the_current_desc_version() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut probe, _game) = steam_fixture(&tmp);
    let (user_probe, user) = fake_user_folder(&tmp);
    probe.documents = user_probe.documents;
    fs::write(
        user.join("gameSettings.xml"),
        format!(
            "<gameSettings><modsDirectoryOverride active=\"true\" directory=\"{}\"/></gameSettings>",
            tmp.path().join("profile-a").to_string_lossy()
        ),
    )
    .unwrap();
    fs::write(
        user.join("log.txt"),
        STARTUP_LOG
            .replace("1.10.0.0 b34141", "1.25.1.0 b99")
            .replace("Language: en", "Language: de"),
    )
    .unwrap();

    let ctx = game_context(&probe, None);
    assert_eq!(ctx.user_dir.as_deref(), Some(user.as_path()));
    assert_eq!(ctx.effective_mods_dir, Some(tmp.path().join("profile-a")));
    assert_eq!(ctx.default_mods_dir, Some(user.join("mods")));
    assert_eq!(ctx.languages, ["de"]);
    assert_eq!(ctx.logged_desc_version, Some(100));

    // Nothing known: English, no folders.
    let empty = game_context(&FakeProbe::default(), None);
    assert_eq!(empty.languages, ["en"]);
    assert_eq!(empty.user_dir, None);
    assert_eq!(empty.effective_mods_dir, None);
}

#[test]
fn game_status_degrades_gracefully() {
    // No game, no user folder, nothing readable.
    let status = game_status(&FakeProbe::default(), None);
    assert_eq!(status.game_version, None);
    assert!(!status.settings_readable);
    assert!(!status.override_active);
    assert_eq!(status.effective_mods_dir, None);
    assert_eq!(status.logged_desc_version, None);

    // A user folder without settings: default mods folder, settings unreadable.
    let tmp = tempfile::tempdir().unwrap();
    let (probe, user) = fake_user_folder(&tmp);
    let status = game_status(&probe, None);
    assert!(!status.settings_readable);
    assert_eq!(
        status.effective_mods_dir.as_deref(),
        Some(&*user.join("mods").to_string_lossy())
    );

    // Malformed settings are also "not readable", and never an active override.
    fs::write(user.join("gameSettings.xml"), "<a><b></a>").unwrap();
    let status = game_status(&probe, None);
    assert!(!status.settings_readable);
    assert!(!status.override_active);
}
