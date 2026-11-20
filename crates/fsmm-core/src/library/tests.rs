//! Library tests: temp folders and in-memory databases; never a real game folder.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use super::*;
use crate::model::{IssueKind, ModFormat};
use crate::store::FileStamp;
use crate::testutil::{dds, png, write_folder, write_zip, DdsFormat, Method};

fn desc(title: &str, icon: Option<&str>) -> String {
    let icon = icon.map_or(String::new(), |i| {
        format!("<iconFilename>{i}</iconFilename>")
    });
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
         <modDesc descVersion=\"105\"><title><en>{title}</en></title>\
         <author>Someone</author><version>1.0.0.0</version>{icon}</modDesc>"
    )
}

fn write_mod(folder: &Path, file: &str, title: &str) -> PathBuf {
    let path = folder.join(file);
    write_zip(
        &path,
        &[("modDesc.xml", desc(title, None).as_bytes())],
        Method::Stored,
    );
    path
}

/// A 4x4 PNG icon inside the zip.
fn write_mod_with_icon(folder: &Path, file: &str, title: &str) -> PathBuf {
    let path = folder.join(file);
    let icon = png(4, 4, ::png::ColorType::Rgba, &[200u8; 4 * 4 * 4]);
    write_zip(
        &path,
        &[
            ("modDesc.xml", desc(title, Some("icon.png")).as_bytes()),
            ("icon.png", &icon),
        ],
        Method::Stored,
    );
    path
}

fn library() -> Library {
    Library::new(Store::open_in_memory().unwrap(), "http://thumb.localhost")
}

fn ctx() -> GameContext {
    GameContext {
        languages: vec!["en".to_string()],
        ..GameContext::default()
    }
}

fn ctx_with(effective: Option<&Path>, default: Option<&Path>) -> GameContext {
    GameContext {
        effective_mods_dir: effective.map(Path::to_path_buf),
        default_mods_dir: default.map(Path::to_path_buf),
        ..ctx()
    }
}

fn add(lib: &Library, folder: &Path) -> Profile {
    lib.add_profile(folder, &ctx()).unwrap()
}

fn list(lib: &Library, id: &str) -> Vec<ModEntry> {
    lib.list_mods(id, &ctx(), &|_| {}).unwrap()
}

fn titles(mods: &[ModEntry]) -> Vec<&str> {
    mods.iter().map(|m| m.title.as_str()).collect()
}

fn set_mtime(path: &Path, time: SystemTime) {
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

// ---------------------------------------------------------------- compile-time

#[test]
fn library_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Library>();
}

// ---------------------------------------------------------------- ensure_profiles

#[test]
fn an_empty_registry_is_seeded_from_the_suggestions() {
    let user = tempfile::tempdir().unwrap();
    write_mod(
        &{
            let p = user.path().join("mods");
            fs::create_dir_all(&p).unwrap();
            p
        },
        "FS25_A.zip",
        "A",
    );
    let test_dir = user.path().join("$testingMods");
    fs::create_dir_all(&test_dir).unwrap();
    write_mod(&test_dir, "FS25_B.zip", "B");
    let empty_dir = user.path().join("emptyFolder");
    fs::create_dir_all(&empty_dir).unwrap();

    let lib = library();
    let context = GameContext {
        user_dir: Some(user.path().to_path_buf()),
        ..ctx()
    };
    lib.ensure_profiles(&context).unwrap();
    let profiles = lib.list_profiles(&context).unwrap();
    assert_eq!(
        profiles.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["mods", "$testingMods"]
    );
    assert_eq!(profiles[0].id, "p1");
    assert_eq!(profiles[1].mod_count, 1);
}

#[test]
fn registered_profiles_are_not_seeded_again() {
    let user = tempfile::tempdir().unwrap();
    let mods = user.path().join("mods");
    fs::create_dir_all(&mods).unwrap();
    let other = user.path().join("other");
    fs::create_dir_all(&other).unwrap();
    write_mod(&other, "FS25_B.zip", "B");

    let lib = library();
    let own = tempfile::tempdir().unwrap();
    add(&lib, own.path());
    lib.ensure_profiles(&GameContext {
        user_dir: Some(user.path().to_path_buf()),
        ..ctx()
    })
    .unwrap();
    let profiles = lib.list_profiles(&ctx()).unwrap();
    assert_eq!(profiles.len(), 1, "suggestions only seed an empty registry");
}

#[test]
fn effective_and_default_folders_are_added_without_duplicates() {
    let root = tempfile::tempdir().unwrap();
    let default = root.path().join("mods");
    let effective = root.path().join("custom");
    fs::create_dir_all(&default).unwrap();
    fs::create_dir_all(&effective).unwrap();
    let missing = root.path().join("gone");

    let lib = library();
    let context = ctx_with(Some(&effective), Some(&default));
    lib.ensure_profiles(&context).unwrap();
    lib.ensure_profiles(&context).unwrap();
    // The same folder written with `/` and in another case is not a new profile.
    let respelled = PathBuf::from(
        effective
            .to_string_lossy()
            .to_uppercase()
            .replace('\\', "/"),
    );
    lib.ensure_profiles(&ctx_with(Some(&respelled), Some(&missing)))
        .unwrap();

    let profiles = lib.list_profiles(&context).unwrap();
    assert_eq!(
        profiles.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["custom", "mods"]
    );
}

// ---------------------------------------------------------------- scans

#[test]
fn every_listing_is_fresh() {
    let dir = tempfile::tempdir().unwrap();
    write_mod(dir.path(), "FS25_A.zip", "A");
    write_mod(dir.path(), "FS25_B.zip", "B");
    let lib = library();
    let id = add(&lib, dir.path()).id;
    assert_eq!(titles(&list(&lib, &id)), ["A", "B"]);

    write_mod(dir.path(), "FS25_C.zip", "C");
    fs::remove_file(dir.path().join("FS25_A.zip")).unwrap();
    write_mod(dir.path(), "FS25_B.zip", "B changed");
    set_mtime(
        &dir.path().join("FS25_B.zip"),
        SystemTime::now() + Duration::from_secs(5),
    );
    assert_eq!(titles(&list(&lib, &id)), ["B changed", "C"]);
}

#[test]
fn a_cache_hit_does_not_read_the_mod_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_mod(dir.path(), "FS25_A.zip", "Old");
    let stamp = mtime(&path);
    let lib = library();
    let id = add(&lib, dir.path()).id;
    assert_eq!(titles(&list(&lib, &id)), ["Old"]);

    // Same size, same mtime, different content: only a re-read could show it.
    write_mod(dir.path(), "FS25_A.zip", "New");
    set_mtime(&path, stamp);
    assert_eq!(titles(&list(&lib, &id)), ["Old"]);

    set_mtime(&path, stamp + Duration::from_secs(10));
    assert_eq!(titles(&list(&lib, &id)), ["New"]);
}

#[test]
fn a_changed_size_is_read_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_mod(dir.path(), "FS25_A.zip", "Old");
    let stamp = mtime(&path);
    let lib = library();
    let id = add(&lib, dir.path()).id;
    list(&lib, &id);

    write_mod(dir.path(), "FS25_A.zip", "A much longer title");
    set_mtime(&path, stamp);
    assert_eq!(titles(&list(&lib, &id)), ["A much longer title"]);
}

#[test]
fn health_rules_run_on_cached_records() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_mod(dir.path(), "FS25_Twin.zip", "Twin");
    let stamp = mtime(&path);
    let lib = library();
    let id = add(&lib, dir.path()).id;
    let before = list(&lib, &id);
    assert!(before[0].issues.is_empty());

    // A folder mod with the same name appears next to the cached zip.
    write_folder(
        &dir.path().join("FS25_Twin"),
        &[("modDesc.xml", desc("Twin folder", None).as_bytes())],
    );
    set_mtime(&path, stamp);
    let after = list(&lib, &id);
    assert_eq!(after.len(), 2);
    for entry in &after {
        assert!(
            entry
                .issues
                .iter()
                .any(|i| i.kind == IssueKind::ZipAndFolderTwin),
            "{} should be a twin",
            entry.id
        );
    }
    let zip = after.iter().find(|e| e.format == ModFormat::Zip).unwrap();
    assert_eq!(zip.title, "Twin", "the zip came from the cache");

    // Removing the folder removes the issue from the cached zip again.
    fs::remove_dir_all(dir.path().join("FS25_Twin")).unwrap();
    assert!(list(&lib, &id)[0].issues.is_empty());
}

#[test]
fn cached_entries_get_the_profile_id_of_the_profile_they_are_listed_for() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_mod(dir.path(), "FS25_A.zip", "A");
    let lib = library();
    let id = add(&lib, dir.path()).id;
    assert_eq!(list(&lib, &id)[0].profile_id, id);

    // Rewrite the cached record as if it had been read for another profile id.
    let stamp = FileStamp {
        size: fs::metadata(&path).unwrap().len(),
        modified: mtime(&path),
    };
    let mut cached = lib.store.cached_mod(&path, stamp).unwrap().unwrap();
    cached.entry.profile_id = "stale".to_string();
    lib.store.put_mod(&path, stamp, &cached).unwrap();
    assert_eq!(
        lib.store
            .cached_mod(&path, stamp)
            .unwrap()
            .unwrap()
            .entry
            .profile_id,
        "stale"
    );

    assert_eq!(list(&lib, &id)[0].profile_id, id);
}

#[test]
fn deleted_files_are_pruned_from_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    write_mod(dir.path(), "FS25_A.zip", "A");
    write_mod(dir.path(), "FS25_B.zip", "B");
    let lib = library();
    let id = add(&lib, dir.path()).id;
    list(&lib, &id);
    assert_eq!(lib.store.cached_mods_in(dir.path()).unwrap().len(), 2);
    fs::remove_file(dir.path().join("FS25_B.zip")).unwrap();
    list(&lib, &id);
    assert_eq!(lib.store.cached_mods_in(dir.path()).unwrap().len(), 1);
}

#[test]
fn a_case_only_rename_with_the_same_stamp_is_not_served_from_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_mod(dir.path(), "FS25_a.zip", "A");
    let stamp = mtime(&path);
    let lib = library();
    let id = add(&lib, dir.path()).id;
    assert_eq!(list(&lib, &id)[0].id, "FS25_a.zip");

    fs::rename(&path, dir.path().join("FS25_A.zip")).unwrap();
    let renamed = dir.path().join("FS25_A.zip");
    set_mtime(&renamed, stamp);
    let mods = list(&lib, &id);
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].id, "FS25_A.zip");
    assert_eq!(mods[0].mod_name, "FS25_A");
}

// ---------------------------------------------------------------- also_in

#[test]
fn also_in_is_symmetric_case_insensitive_and_in_registry_order() {
    let root = tempfile::tempdir().unwrap();
    let dirs: Vec<PathBuf> = ["a", "b", "c"]
        .iter()
        .map(|n| {
            let d = root.path().join(n);
            fs::create_dir_all(&d).unwrap();
            d
        })
        .collect();
    write_mod(&dirs[0], "FS25_Shared.zip", "Shared in a");
    write_mod(&dirs[0], "FS25_OnlyA.zip", "Only a");
    write_mod(&dirs[1], "fs25_shared.ZIP", "Shared in b");
    write_folder(
        &dirs[2],
        &[(
            "FS25_SHARED/modDesc.xml",
            desc("Shared in c", None).as_bytes(),
        )],
    );

    let lib = library();
    let ids: Vec<String> = dirs.iter().map(|d| add(&lib, d).id).collect();
    let in_a = list(&lib, &ids[0]);
    let in_b = list(&lib, &ids[1]);
    let in_c = list(&lib, &ids[2]);

    let shared = |mods: &[ModEntry]| {
        mods.iter()
            .find(|m| m.mod_name.eq_ignore_ascii_case("fs25_shared"))
            .unwrap()
            .also_in
            .clone()
    };
    assert_eq!(shared(&in_a), [ids[1].clone(), ids[2].clone()]);
    assert_eq!(shared(&in_b), [ids[0].clone(), ids[2].clone()]);
    assert_eq!(shared(&in_c), [ids[0].clone(), ids[1].clone()]);
    let only_a = in_a.iter().find(|m| m.mod_name == "FS25_OnlyA").unwrap();
    assert!(only_a.also_in.is_empty());
}

#[test]
fn also_in_scans_profiles_that_have_no_last_scan_and_ignores_missing_folders() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    let gone = root.path().join("gone");
    for d in [&a, &b, &gone] {
        fs::create_dir_all(d).unwrap();
    }
    write_mod(&a, "FS25_X.zip", "X");
    write_mod(&b, "FS25_X.zip", "X");
    let lib = library();
    let id_a = add(&lib, &a).id;
    let id_b = add(&lib, &b).id;
    add(&lib, &gone);
    fs::remove_dir_all(&gone).unwrap();
    // `add_profile` scanned them all; forget those scans.
    lib.scans.write().unwrap().clear();

    let listing = list(&lib, &id_a);
    assert_eq!(listing[0].also_in, std::slice::from_ref(&id_b));
    assert!(
        lib.last_scan(&id_b).is_some(),
        "the other profile was scanned"
    );
    assert!(lib.last_scan("p3").is_none(), "the missing folder was not");
}

// ---------------------------------------------------------------- profiles

#[test]
fn profiles_report_counts_active_and_default() {
    let root = tempfile::tempdir().unwrap();
    let default = root.path().join("mods");
    let active = root.path().join("Testing");
    for d in [&default, &active] {
        fs::create_dir_all(d).unwrap();
    }
    write_mod(&default, "FS25_A.zip", "A");
    write_mod(&default, "FS25_B.zip", "B");
    fs::write(default.join("FS25_Broken.zip"), b"not a zip").unwrap();
    write_mod(&active, "FS25_C.zip", "C");
    let size_a = fs::metadata(default.join("FS25_A.zip")).unwrap().len();
    let size_b = fs::metadata(default.join("FS25_B.zip")).unwrap().len();
    let size_x = fs::metadata(default.join("FS25_Broken.zip")).unwrap().len();

    let lib = library();
    // The log writes `/` and other case; the settings write `\`.
    let effective = PathBuf::from(active.to_string_lossy().to_lowercase().replace('\\', "/"));
    let default_spelled = PathBuf::from(format!("{}\\", default.to_string_lossy().to_uppercase()));
    let context = ctx_with(Some(&effective), Some(&default_spelled));
    lib.add_profile(&default, &context).unwrap();
    lib.add_profile(&active, &context).unwrap();
    let profiles = lib.list_profiles(&context).unwrap();

    assert_eq!(profiles[0].name, "mods");
    assert!(profiles[0].is_game_default && !profiles[0].is_active);
    assert_eq!(profiles[0].mod_count, 3);
    assert_eq!(profiles[0].total_bytes, size_a + size_b + size_x);
    assert_eq!(profiles[0].issue_count, 1);
    assert_eq!(profiles[0].update_count, 0);
    assert_eq!(profiles[0].path, default.to_string_lossy());
    assert!(profiles[1].is_active && !profiles[1].is_game_default);
    assert_eq!(profiles[1].mod_count, 1);
}

#[test]
fn adding_a_folder_twice_returns_the_same_profile() {
    let dir = tempfile::tempdir().unwrap();
    write_mod(dir.path(), "FS25_A.zip", "A");
    let lib = library();
    let first = add(&lib, dir.path());
    let again = add(
        &lib,
        Path::new(&dir.path().to_string_lossy().to_uppercase()),
    );
    assert_eq!(first.id, again.id);
    assert_eq!(first.mod_count, 1);
    assert_eq!(lib.list_profiles(&ctx()).unwrap().len(), 1);
}

#[test]
fn adding_something_that_is_not_a_folder_fails() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file.txt");
    fs::write(&file, b"x").unwrap();
    let lib = library();
    for path in [file, dir.path().join("missing")] {
        let err = lib.add_profile(&path, &ctx()).unwrap_err();
        assert_eq!(err.code, ErrorCode::FolderMissing);
    }
    assert!(lib.list_profiles(&ctx()).unwrap().is_empty());
}

#[test]
fn a_missing_folder_has_zero_counts_in_the_list_and_is_an_error_when_opened() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("vanishing");
    fs::create_dir_all(&dir).unwrap();
    write_mod(&dir, "FS25_A.zip", "A");
    let lib = library();
    let id = add(&lib, &dir).id;
    fs::remove_dir_all(&dir).unwrap();

    let profiles = lib.list_profiles(&ctx()).unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(
        (
            profiles[0].mod_count,
            profiles[0].total_bytes,
            profiles[0].issue_count
        ),
        (0, 0, 0)
    );
    let err = lib.list_mods(&id, &ctx(), &|_| {}).unwrap_err();
    assert_eq!(err, ApiError::new(ErrorCode::FolderMissing, Some(id)));
}

#[test]
fn unknown_profile_ids_are_profile_not_found() {
    let lib = library();
    let err = lib.list_mods("p99", &ctx(), &|_| {}).unwrap_err();
    assert_eq!(
        err,
        ApiError::new(ErrorCode::ProfileNotFound, Some("p99".into()))
    );
    assert_eq!(
        lib.profile_path("p99").unwrap_err().code,
        ErrorCode::ProfileNotFound
    );
    assert_eq!(
        lib.mod_path("p99", "x").unwrap_err().code,
        ErrorCode::ProfileNotFound
    );
    assert_eq!(
        lib.thumbnail("p99", "x").unwrap_err().code,
        ErrorCode::ProfileNotFound
    );
}

#[test]
fn paths_resolve_from_ids_and_scan_when_there_is_no_last_scan() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_mod(dir.path(), "FS25_A.zip", "A");
    let lib = library();
    let id = add(&lib, dir.path()).id;

    assert_eq!(lib.profile_path(&id).unwrap(), dir.path());
    assert_eq!(lib.mod_path(&id, "FS25_A.zip").unwrap(), path);
    let err = lib.mod_path(&id, "FS25_Nope.zip").unwrap_err();
    assert_eq!(
        err,
        ApiError::new(ErrorCode::FolderMissing, Some("FS25_Nope.zip".into()))
    );

    // A library that has not listed anything scans on demand.
    let fresh = library();
    let id = add(&fresh, dir.path()).id;
    fresh.scans.write().unwrap().clear();
    assert_eq!(fresh.mod_path(&id, "FS25_A.zip").unwrap(), path);
}

// ---------------------------------------------------------------- progress

fn collect_progress(lib: &Library, id: &str) -> Vec<ScanProgress> {
    let events = Mutex::new(Vec::new());
    lib.list_mods(id, &ctx(), &|p| events.lock().unwrap().push(p))
        .unwrap();
    events.into_inner().unwrap()
}

#[test]
fn progress_is_monotonic_and_runs_from_zero_to_the_total() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..120 {
        write_mod(dir.path(), &format!("FS25_M{i:03}.zip"), "M");
    }
    let lib = library();
    let id = add(&lib, dir.path()).id;
    // The first scan reads everything, the second is all cache hits.
    for _ in 0..2 {
        let events = collect_progress(&lib, &id);
        assert!(events.len() >= 2);
        assert!(events.len() < 120, "progress is throttled");
        assert_eq!(events[0].done, 0);
        let last = events.last().unwrap();
        assert_eq!((last.done, last.total), (120, 120));
        assert!(events.iter().all(|e| e.total == 120 && e.profile_id == id));
        assert!(events.windows(2).all(|w| w[0].done <= w[1].done));
    }
}

#[test]
fn an_empty_folder_reports_zero_of_zero() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library();
    let id = add(&lib, dir.path()).id;
    let events = collect_progress(&lib, &id);
    assert!(!events.is_empty());
    assert!(events.iter().all(|e| e.done == 0 && e.total == 0));
}

// ---------------------------------------------------------------- icon URLs

#[test]
fn icon_urls_have_the_documented_format_and_encoding() {
    let dir = tempfile::tempdir().unwrap();
    let plain = write_mod_with_icon(dir.path(), "FS25_Plain.zip", "Plain");
    write_mod_with_icon(dir.path(), "FS25_Odd (1) #2 & 100%.zip", "Odd");
    write_mod_with_icon(dir.path(), "FS25_Ünï.zip", "Unicode");
    write_mod(dir.path(), "FS25_NoIcon.zip", "No icon");
    let lib = Library::new(Store::open_in_memory().unwrap(), "http://thumb.localhost/");
    let id = add(&lib, dir.path()).id;
    let mods = list(&lib, &id);

    let by_id = |name: &str| mods.iter().find(|m| m.id == name).unwrap();
    let secs = mtime(&plain)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let size = fs::metadata(&plain).unwrap().len();
    assert_eq!(
        by_id("FS25_Plain.zip").icon.as_deref(),
        Some(format!("http://thumb.localhost/{id}/FS25_Plain.zip?v={size}-{secs}").as_str())
    );
    let odd = by_id("FS25_Odd (1) #2 & 100%.zip").icon.clone().unwrap();
    assert!(
        odd.contains("/FS25_Odd%20%281%29%20%232%20%26%20100%25.zip?v="),
        "{odd}"
    );
    let unicode = by_id("FS25_Ünï.zip").icon.clone().unwrap();
    assert!(unicode.contains("/FS25_%C3%9Cn%C3%AF.zip?v="), "{unicode}");
    assert_eq!(by_id("FS25_NoIcon.zip").icon, None);
}

#[test]
fn a_changed_file_changes_the_icon_url_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_mod_with_icon(dir.path(), "FS25_A.zip", "A");
    let lib = library();
    let id = add(&lib, dir.path()).id;
    let before = list(&lib, &id)[0].icon.clone().unwrap();
    set_mtime(&path, mtime(&path) + Duration::from_secs(60));
    let after = list(&lib, &id)[0].icon.clone().unwrap();
    assert_ne!(before, after);
}

// ---------------------------------------------------------------- thumbnails

#[test]
fn a_thumbnail_is_decoded_once_and_then_served_from_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    write_mod_with_icon(dir.path(), "FS25_A.zip", "A");
    let lib = library();
    let id = add(&lib, dir.path()).id;
    list(&lib, &id);

    let first = lib.thumbnail(&id, "FS25_A.zip").unwrap();
    assert!(first.starts_with(&[0x89, b'P', b'N', b'G']));
    assert_eq!(lib.decodes.load(Ordering::SeqCst), 1);
    let second = lib.thumbnail(&id, "FS25_A.zip").unwrap();
    assert_eq!(second, first);
    assert_eq!(
        lib.decodes.load(Ordering::SeqCst),
        1,
        "second call is cached"
    );

    // A new library over the same cache would also not decode: the store holds the PNG.
    let stamp = lib.scans.read().unwrap()[&id].stamps[0];
    let path = lib.mod_path(&id, "FS25_A.zip").unwrap();
    assert_eq!(lib.store.thumbnail(&path, stamp).unwrap(), Some(first));
}

#[test]
fn a_dds_icon_is_decoded_to_png() {
    let dir = tempfile::tempdir().unwrap();
    let rgb = DdsFormat::Rgb {
        bits: 32,
        masks: [0xff0000, 0xff00, 0xff, 0xff00_0000],
    };
    let icon = dds(2, 2, 0, rgb, &[255u8; 16]);
    write_zip(
        &dir.path().join("FS25_D.zip"),
        &[
            ("modDesc.xml", desc("D", Some("icon.dds")).as_bytes()),
            ("icon.dds", &icon),
        ],
        Method::Stored,
    );
    let lib = library();
    let id = add(&lib, dir.path()).id;
    list(&lib, &id);
    let png = lib.thumbnail(&id, "FS25_D.zip").unwrap();
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
}

#[test]
fn thumbnail_errors() {
    let dir = tempfile::tempdir().unwrap();
    write_mod(dir.path(), "FS25_NoIcon.zip", "No icon");
    write_zip(
        &dir.path().join("FS25_Bad.zip"),
        &[
            ("modDesc.xml", desc("Bad", Some("icon.png")).as_bytes()),
            ("icon.png", b"this is not an image"),
        ],
        Method::Stored,
    );
    let lib = library();
    let id = add(&lib, dir.path()).id;
    list(&lib, &id);

    let err = lib.thumbnail(&id, "FS25_NoIcon.zip").unwrap_err();
    assert_eq!(
        err,
        ApiError::new(ErrorCode::FolderMissing, Some(NO_ICON.into()))
    );
    let err = lib.thumbnail(&id, "FS25_Missing.zip").unwrap_err();
    assert_eq!(err.code, ErrorCode::FolderMissing);
    assert_eq!(err.detail.as_deref(), Some("FS25_Missing.zip"));
    let err = lib.thumbnail(&id, "FS25_Bad.zip").unwrap_err();
    assert_eq!(err.code, ErrorCode::Internal);
    assert!(err.detail.is_some_and(|d| !d.is_empty()));
}

#[test]
fn a_decoder_that_panics_gives_no_thumbnail_instead_of_unwinding() {
    let err = guard_decoder::<()>(|| panic!("a decoder bug")).unwrap_err();
    assert_eq!(err.code, ErrorCode::Internal);
    assert_eq!(err.detail.as_deref(), Some(DECODER_PANIC));
    // An ordinary error and a result pass through as before.
    let err = guard_decoder::<()>(|| Err(icons::IconError::Encode("x".into()))).unwrap_err();
    assert_eq!(err.code, ErrorCode::Internal);
    assert_eq!(guard_decoder(|| Ok(5)).unwrap(), 5);
}

#[test]
fn a_crafted_palette_png_icon_is_an_error_not_a_crash() {
    // 1x1, colour type 3 and a 4-byte PLTE: the `png` crate panics on it.
    let mut crafted = Vec::new();
    {
        let mut encoder = ::png::Encoder::new(&mut crafted, 1, 1);
        encoder.set_color(::png::ColorType::Indexed);
        encoder.set_depth(::png::BitDepth::Eight);
        encoder.set_palette(vec![1, 2, 3, 4]);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0]).unwrap();
        writer.finish().unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    write_zip(
        &dir.path().join("FS25_Crafted.zip"),
        &[
            ("modDesc.xml", desc("Crafted", Some("icon.png")).as_bytes()),
            ("icon.png", &crafted),
        ],
        Method::Stored,
    );
    let lib = library();
    let id = add(&lib, dir.path()).id;
    list(&lib, &id);
    let err = lib.thumbnail(&id, "FS25_Crafted.zip").unwrap_err();
    assert_eq!(err.code, ErrorCode::Internal);
}

#[test]
fn a_thumbnail_for_a_file_that_vanished_is_folder_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_mod_with_icon(dir.path(), "FS25_A.zip", "A");
    let lib = library();
    let id = add(&lib, dir.path()).id;
    list(&lib, &id);
    fs::remove_file(path).unwrap();
    assert_eq!(
        lib.thumbnail(&id, "FS25_A.zip").unwrap_err().code,
        ErrorCode::FolderMissing
    );
}

// ---------------------------------------------------------------- segments

#[test]
fn segments_round_trip() {
    for id in [
        "FS25_Plain.zip",
        "with space (1).zip",
        "a#b&c%d.zip",
        "Ünïcödé_日本語.zip",
        "~tilde-dash_dot.",
        "",
        "emoji 🚜.zip",
        "100%25",
    ] {
        let encoded = encode_segment(id);
        assert!(
            encoded
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~%".contains(&b)),
            "{encoded}"
        );
        assert_eq!(decode_segment(&encoded).as_deref(), Some(id), "{id}");
    }
    assert_eq!(encode_segment("a b"), "a%20b");
    assert_eq!(encode_segment("é"), "%C3%A9");
    assert_eq!(encode_segment("A-z_0.9~"), "A-z_0.9~");
}

#[test]
fn malformed_segments_do_not_decode() {
    for bad in [
        "%", "%2", "%zz", "abc%G1", "%4", "a%", "%C3", "%FF%FF", "%E2%82",
    ] {
        assert_eq!(decode_segment(bad), None, "{bad}");
    }
    assert_eq!(decode_segment("%e2%82%ac").as_deref(), Some("€"));
    assert_eq!(decode_segment("%C3%A9").as_deref(), Some("é"));
    assert_eq!(decode_segment("plain").as_deref(), Some("plain"));
}

// ---------------------------------------------------------------- concurrency

#[test]
fn thumbnails_are_served_while_a_scan_runs() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..60 {
        write_mod_with_icon(dir.path(), &format!("FS25_M{i:02}.zip"), "M");
    }
    let lib = library();
    let id = add(&lib, dir.path()).id;
    list(&lib, &id);

    std::thread::scope(|s| {
        let scan = s.spawn(|| {
            for _ in 0..5 {
                list(&lib, &id);
            }
        });
        let thumbs = s.spawn(|| {
            for i in 0..60 {
                lib.thumbnail(&id, &format!("FS25_M{i:02}.zip")).unwrap();
            }
        });
        scan.join().unwrap();
        thumbs.join().unwrap();
    });
    assert_eq!(lib.decodes.load(Ordering::SeqCst), 60);
}

#[test]
fn a_progress_callback_may_request_a_thumbnail() {
    // The callback runs mid-scan; it must not be holding a lock the thumbnail needs.
    let dir = tempfile::tempdir().unwrap();
    write_mod_with_icon(dir.path(), "FS25_A.zip", "A");
    let lib = library();
    let id = add(&lib, dir.path()).id;
    list(&lib, &id);
    lib.list_mods(&id, &ctx(), &|_| {
        lib.thumbnail(&id, "FS25_A.zip").unwrap();
    })
    .unwrap();
}
