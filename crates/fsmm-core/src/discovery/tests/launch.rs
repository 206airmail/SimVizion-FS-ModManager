//! Plan M8: launch targets, Microsoft Store detection (fixtures only; unverified on a real
//! install), the store's user folder and game folder validation.

use super::*;

const STORE_FAMILY: &str = "GIANTSSoftware.FarmingSimulator25PC_abcd1234efgh";
const STORE_FULL_NAME: &str = "GIANTSSoftware.FarmingSimulator25PC_1.2.3.4_x64__abcd1234efgh";

fn running_game(probe: &mut FakeProbe) {
    probe.processes = vec!["FarmingSimulator2025Game.exe".into()];
}

fn giants_fixture(tmp: &TempDir) -> (FakeProbe, PathBuf) {
    let game = tmp.path().join("GIANTS").join("Farming Simulator 2025");
    make_install(&game, Some("1.25.1.0"));
    let mut probe = FakeProbe::default();
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
        &game.to_string_lossy(),
    );
    (probe, game)
}

/// An Epic manifest with an `AppName` (`None` leaves the field out).
fn epic_fixture(tmp: &TempDir, app_name: Option<&str>) -> FakeProbe {
    let program_data = tmp.path().join("ProgramData");
    let game = tmp.path().join("Epic Games").join("FarmingSimulator25");
    make_install(&game, Some("1.25.1.0"));
    let dir = program_data.join("Epic/EpicGamesLauncher/Data/Manifests");
    fs::create_dir_all(&dir).unwrap();
    let mut json = serde_json::json!({
        "DisplayName": "Farming Simulator 25",
        "InstallLocation": game.to_string_lossy(),
    });
    if let Some(name) = app_name {
        json["AppName"] = name.into();
    }
    fs::write(dir.join("A.item"), json.to_string()).unwrap();
    FakeProbe {
        program_data: Some(program_data),
        ..FakeProbe::default()
    }
}

/// Registers a Microsoft Store package: its registry entry, folder and, if given, manifest.
fn store_package(
    probe: &mut FakeProbe,
    tmp: &TempDir,
    full_name: &str,
    manifest: Option<&str>,
) -> PathBuf {
    let root = tmp.path().join("WindowsApps").join(full_name);
    fs::create_dir_all(&root).unwrap();
    probe.set(
        Hive::CurrentUser,
        &format!(r"{PACKAGES_KEY}\{full_name}"),
        "PackageRootFolder",
        &root.to_string_lossy(),
    );
    if let Some(manifest) = manifest {
        fs::write(root.join("AppxManifest.xml"), manifest).unwrap();
    }
    root
}

fn manifest(app_id: &str) -> String {
    format!(
        "{BOM}<?xml version=\"1.0\" encoding=\"utf-8\"?>\
         <Package xmlns=\"http://schemas.microsoft.com/appx/manifest/foundation/windows10\">\
         <Identity Name=\"GIANTSSoftware.FarmingSimulator25PC\"/>\
         <Applications><Application Id=\"{app_id}\" Executable=\"x.exe\"/>\
         <Application Id=\"Second\" Executable=\"y.exe\"/></Applications></Package>"
    )
}

fn code(result: Result<LaunchTarget, ApiError>) -> ErrorCode {
    result.unwrap_err().code
}

// ---- launch targets ----

#[test]
fn steam_launches_by_its_app_id() {
    let tmp = tempfile::tempdir().unwrap();
    let (probe, _) = steam_fixture(&tmp);
    assert_eq!(
        launch_target(&probe).unwrap(),
        LaunchTarget::Url("steam://rungameid/2300320".to_string())
    );
}

#[test]
fn giants_launches_the_exe_in_its_install_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let (probe, game) = giants_fixture(&tmp);
    match launch_target(&probe).unwrap() {
        LaunchTarget::Program { exe, dir } => {
            assert!(same_path(&dir, &game));
            assert!(same_path(&exe, &game.join(GAME_EXE)));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn epic_launches_by_the_manifests_app_name() {
    let tmp = tempfile::tempdir().unwrap();
    let probe = epic_fixture(&tmp, Some("Sugarbush_1.2-b3"));
    let installs = find_installs(&probe);
    assert_eq!(installs[0].launch_id.as_deref(), Some("Sugarbush_1.2-b3"));
    assert_eq!(
        launch_target(&probe).unwrap(),
        LaunchTarget::Url(
            "com.epicgames.launcher://apps/Sugarbush_1.2-b3?action=launch&silent=true".to_string()
        )
    );
    assert!(game_status(&probe, None).launchable);
}

#[test]
fn epic_without_an_app_name_or_with_an_unsafe_one_is_not_launchable() {
    for app_name in [
        None,
        Some(""),
        Some("a b"),
        Some("a/b"),
        Some("a?b=c"),
        Some("a&b"),
        Some("a%20b"),
        Some("a#b"),
        Some("a:b"),
        Some("a\\b"),
        Some("é"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let probe = epic_fixture(&tmp, app_name);
        assert_eq!(
            code(launch_target(&probe)),
            ErrorCode::NotLaunchable,
            "{app_name:?}"
        );
        // The install is still found and shown; it just cannot be launched.
        let status = game_status(&probe, None);
        assert_eq!(status.store, Some(GameStore::Epic));
        assert!(!status.launchable, "{app_name:?}");
    }
}

#[test]
fn nothing_installed_is_not_launchable() {
    let probe = FakeProbe::default();
    assert_eq!(code(launch_target(&probe)), ErrorCode::NotLaunchable);
    assert!(!game_status(&probe, None).launchable);
}

#[test]
fn a_running_game_refuses_to_launch_for_every_store() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut steam, _) = steam_fixture(&tmp);
    let (mut giants, _) = giants_fixture(&tmp);
    let mut epic = epic_fixture(&tmp, Some("App"));
    let mut ms = FakeProbe::default();
    store_package(&mut ms, &tmp, STORE_FULL_NAME, None);
    for probe in [&mut steam, &mut giants, &mut epic, &mut ms] {
        assert!(launch_target(probe).is_ok());
        running_game(probe);
        assert_eq!(code(launch_target(probe)), ErrorCode::GameRunning);
        // `launchable` says what would work if the game were not running.
        assert!(game_status(probe, None).launchable);
    }
    // The dedicated server is not the game.
    steam.processes = vec![SERVER_PROCESS_NAME.into()];
    assert!(launch_target(&steam).is_ok());
}

#[test]
fn the_first_install_decides_the_launch_target() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut probe, _) = steam_fixture(&tmp);
    store_package(&mut probe, &tmp, STORE_FULL_NAME, None);
    assert_eq!(
        launch_target(&probe).unwrap(),
        LaunchTarget::Url("steam://rungameid/2300320".to_string())
    );
}

// ---- Microsoft Store (fixtures only; unverified on a real install) ----

#[test]
fn a_microsoft_store_install_uses_the_family_name_and_the_manifests_first_app_id() {
    let tmp = tempfile::tempdir().unwrap();
    let mut probe = FakeProbe::default();
    let root = store_package(&mut probe, &tmp, STORE_FULL_NAME, Some(&manifest("Game")));
    let installs = find_installs(&probe);
    assert_eq!(installs.len(), 1);
    assert_eq!(installs[0].store, Store::MicrosoftStore);
    assert_eq!(installs[0].store.to_model(), GameStore::MicrosoftStore);
    assert!(same_path(&installs[0].dir, &root));
    // No exe in the folder: WindowsApps is often unreadable, so it is not required.
    assert!(!root.join(GAME_EXE).exists());
    assert_eq!(
        installs[0].launch_id.as_deref(),
        Some(&*format!("{STORE_FAMILY}!Game"))
    );
    assert_eq!(
        launch_target(&probe).unwrap(),
        LaunchTarget::AppsFolder(format!("{STORE_FAMILY}!Game"))
    );
    let status = game_status(&probe, None);
    assert_eq!(status.store, Some(GameStore::MicrosoftStore));
    assert!(status.launchable);
}

#[test]
fn a_microsoft_store_install_without_a_readable_manifest_uses_app() {
    for text in [
        None,
        Some("not xml"),
        Some("<Package><Applications/></Package>"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let mut probe = FakeProbe::default();
        store_package(&mut probe, &tmp, STORE_FULL_NAME, text);
        assert_eq!(
            launch_target(&probe).unwrap(),
            LaunchTarget::AppsFolder(format!("{STORE_FAMILY}!App")),
            "{text:?}"
        );
    }
}

#[test]
fn a_manifest_app_id_with_unsafe_characters_is_not_launchable() {
    let tmp = tempfile::tempdir().unwrap();
    let mut probe = FakeProbe::default();
    store_package(
        &mut probe,
        &tmp,
        STORE_FULL_NAME,
        Some(&manifest("Game&amp;calc")),
    );
    assert_eq!(code(launch_target(&probe)), ErrorCode::NotLaunchable);
    assert!(!game_status(&probe, None).launchable);
}

#[test]
fn a_package_full_name_with_unsafe_characters_is_not_launchable() {
    let tmp = tempfile::tempdir().unwrap();
    let mut probe = FakeProbe::default();
    // Not a name Windows would register, but nothing read from the registry is trusted.
    store_package(
        &mut probe,
        &tmp,
        "GIANTSSoftware.FarmingSimulator25PC_1.0.0.0_x64__ab cd",
        None,
    );
    assert_eq!(code(launch_target(&probe)), ErrorCode::NotLaunchable);
}

#[test]
fn only_the_farming_simulator_package_counts_and_the_prefix_ignores_case() {
    let tmp = tempfile::tempdir().unwrap();
    let mut probe = FakeProbe::default();
    store_package(
        &mut probe,
        &tmp,
        "GIANTSSoftware.FarmingSimulator22PC_1.0.0.0_x64__abcd1234efgh",
        None,
    );
    store_package(&mut probe, &tmp, "Contoso.App_1.0.0.0_x64__zzzz", None);
    // Too few parts to derive a family name.
    store_package(
        &mut probe,
        &tmp,
        "GIANTSSoftware.FarmingSimulator25PC_1.0.0.0",
        None,
    );
    assert!(find_installs(&probe).is_empty());

    let lower = STORE_FULL_NAME.to_lowercase();
    let root = store_package(&mut probe, &tmp, &lower, None);
    let installs = find_installs(&probe);
    assert_eq!(installs.len(), 1);
    assert!(same_path(&installs[0].dir, &root));
    assert_eq!(
        installs[0].launch_id.as_deref(),
        Some("giantssoftware.farmingsimulator25pc_abcd1234efgh!App")
    );
}

#[test]
fn a_registered_package_with_a_missing_root_folder_is_not_an_install() {
    let tmp = tempfile::tempdir().unwrap();
    let mut probe = FakeProbe::default();
    let root = store_package(&mut probe, &tmp, STORE_FULL_NAME, None);
    fs::remove_dir(&root).unwrap();
    assert!(find_installs(&probe).is_empty());
    // A package key without `PackageRootFolder` is skipped too.
    probe.set(
        Hive::CurrentUser,
        &format!(r"{PACKAGES_KEY}\GIANTSSoftware.FarmingSimulator25PC_2.0.0.0_x64__abcd1234efgh"),
        "Other",
        "x",
    );
    assert!(find_installs(&probe).is_empty());
}

#[test]
fn the_microsoft_store_comes_after_epic_and_a_shared_folder_is_listed_once() {
    let tmp = tempfile::tempdir().unwrap();
    let mut probe = epic_fixture(&tmp, Some("EpicApp"));
    let ms_root = store_package(&mut probe, &tmp, STORE_FULL_NAME, None);
    let stores: Vec<Store> = find_installs(&probe).iter().map(|i| i.store).collect();
    assert_eq!(stores, [Store::Epic, Store::MicrosoftStore]);

    // The package folder is also found by an earlier source: it is listed once, as that one.
    let mut probe = FakeProbe::default();
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
        &ms_root.to_string_lossy(),
    );
    make_install(&ms_root, None);
    store_package(&mut probe, &tmp, STORE_FULL_NAME, None);
    let installs = find_installs(&probe);
    assert_eq!(installs.len(), 1);
    assert_eq!(installs[0].store, Store::Giants);
}

// ---- the user folder of the Microsoft Store version ----

/// `%LOCALAPPDATA%\Packages\<package>\LocalCache\Local`, created.
fn store_user_folder(tmp: &TempDir, package: &str) -> (PathBuf, PathBuf) {
    let local = tmp.path().join("LocalAppData");
    let user = local
        .join("Packages")
        .join(package)
        .join("LocalCache")
        .join("Local");
    fs::create_dir_all(&user).unwrap();
    (local, user)
}

const STORE_PACKAGE_DIR: &str = "GIANTSSoftware.FarmingSimulator25PC_abcd1234efgh";

#[test]
fn the_store_package_folder_is_the_user_folder_when_my_games_is_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let (local, user) = store_user_folder(&tmp, STORE_PACKAGE_DIR);
    let probe = FakeProbe {
        documents: Some(tmp.path().join("Documents")),
        local_app_data: Some(local),
        ..FakeProbe::default()
    };
    assert_eq!(user_dir(&probe), None);
    let (dir, source) = resolve_user_dir(&probe, None);
    assert_eq!(dir.as_deref(), Some(user.as_path()));
    assert_eq!(source, UserDirSource::Detected);

    let status = game_status(&probe, None);
    assert_eq!(status.user_dir_source, UserDirSource::Detected);
    assert_eq!(status.user_dir.as_deref(), Some(&*user.to_string_lossy()));
    // The package's `mods` folder is the default one.
    assert_eq!(
        status.effective_mods_dir.as_deref(),
        Some(&*user.join("mods").to_string_lossy())
    );
    assert_eq!(game_context(&probe, None).user_dir, Some(user));
}

#[test]
fn my_games_wins_over_the_store_package_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut probe, user) = fake_user_folder(&tmp);
    let (local, _) = store_user_folder(&tmp, STORE_PACKAGE_DIR);
    probe.local_app_data = Some(local);
    let (dir, source) = resolve_user_dir(&probe, None);
    assert_eq!(dir, Some(user));
    assert_eq!(source, UserDirSource::Detected);
}

#[test]
fn a_chosen_folder_wins_over_everything() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut probe, _) = fake_user_folder(&tmp);
    let (local, _) = store_user_folder(&tmp, STORE_PACKAGE_DIR);
    probe.local_app_data = Some(local);
    let chosen = tmp.path().join("Chosen");
    fs::create_dir_all(&chosen).unwrap();
    let (dir, source) = resolve_user_dir(&probe, Some(&chosen));
    assert_eq!(dir, Some(chosen.clone()));
    assert_eq!(source, UserDirSource::Chosen);
    let status = game_status(&probe, Some(&chosen));
    assert_eq!(status.user_dir_source, UserDirSource::Chosen);
    assert_eq!(game_context(&probe, Some(&chosen)).user_dir, Some(chosen));
}

#[test]
fn other_packages_and_package_folders_without_local_cache_are_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let local = tmp.path().join("LocalAppData");
    // Another app's package and the Farming Simulator 22 one, each with data...
    for package in [
        "Contoso.App_abcd",
        "GIANTSSoftware.FarmingSimulator22PC_abcd1234efgh",
    ] {
        fs::create_dir_all(
            local
                .join("Packages")
                .join(package)
                .join("LocalCache")
                .join("Local"),
        )
        .unwrap();
    }
    // ...and a 25 package that has no data folder yet.
    fs::create_dir_all(local.join("Packages").join(STORE_PACKAGE_DIR)).unwrap();
    let probe = FakeProbe {
        local_app_data: Some(local),
        ..FakeProbe::default()
    };
    assert_eq!(resolve_user_dir(&probe, None), (None, UserDirSource::None));
    assert_eq!(game_status(&probe, None).user_dir, None);
    // No local app data folder at all.
    assert_eq!(
        resolve_user_dir(&FakeProbe::default(), None),
        (None, UserDirSource::None)
    );
}

// ---- game folder validation ----

#[test]
fn a_game_folder_needs_game_settings_or_a_mods_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let with_settings = tmp.path().join("a");
    fs::create_dir_all(&with_settings).unwrap();
    fs::write(with_settings.join("gameSettings.xml"), "<gameSettings/>").unwrap();
    let with_mods = tmp.path().join("b");
    fs::create_dir_all(with_mods.join("mods")).unwrap();
    assert!(valid_game_folder(&with_settings));
    assert!(valid_game_folder(&with_mods));

    let empty = tmp.path().join("c");
    fs::create_dir_all(&empty).unwrap();
    assert!(!valid_game_folder(&empty));
    assert!(!valid_game_folder(&tmp.path().join("missing")));
    // A file named `mods` and a folder named `gameSettings.xml` do not count.
    let odd = tmp.path().join("d");
    fs::create_dir_all(odd.join("gameSettings.xml")).unwrap();
    fs::write(odd.join("mods"), "x").unwrap();
    assert!(!valid_game_folder(&odd));
}
