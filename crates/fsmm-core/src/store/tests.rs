//! Store tests: temp files and in-memory databases only.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use super::*;
use crate::container::IconKind;
use crate::model::{ModFormat, ModKind, ModSource, UpdateState};

fn sample_entry(name: &str) -> ModEntry {
    ModEntry {
        id: format!("{name}.zip"),
        profile_id: "p1".into(),
        mod_name: name.into(),
        file_name: format!("{name}.zip"),
        format: ModFormat::Zip,
        title: format!("Title of {name}"),
        author: "Someone".into(),
        version: "1.0.0.0".into(),
        desc_version: Some(105),
        kind: ModKind::Vehicle,
        description: String::new(),
        icon: None,
        size_bytes: 10,
        modified: "2026-01-01T00:00:00Z".into(),
        multiplayer: true,
        dependencies: Vec::new(),
        issues: Vec::new(),
        source: ModSource::Unknown,
        update: UpdateState::Unknown,
        also_in: Vec::new(),
        tags: Vec::new(),
        note: None,
        links: Vec::new(),
    }
}

fn record(name: &str) -> CachedMod {
    CachedMod {
        entry: sample_entry(name),
        icon: Some(IconLocation {
            rel: "icon.dds".into(),
            kind: IconKind::Dds,
        }),
        fingerprint: format!("fingerprint of {name}"),
    }
}

fn stamp(size: u64, secs: u64) -> FileStamp {
    FileStamp {
        size,
        modified: UNIX_EPOCH + Duration::from_secs(secs),
    }
}

fn mod_path(folder: &str, name: &str) -> PathBuf {
    Path::new(folder).join(name)
}

// ---------------------------------------------------------------- path keys

#[test]
fn path_keys_ignore_case_separator_style_and_trailing_separators() {
    let a = path_key(Path::new("C:/Users/Me/Documents/My Games/FS25/mods"));
    let b = path_key(Path::new(
        "c:\\users\\me\\documents\\my games\\fs25\\MODS\\",
    ));
    assert_eq!(a, b);
    assert_eq!(a, "c:\\users\\me\\documents\\my games\\fs25\\mods");
    assert_ne!(
        a,
        path_key(Path::new("C:/Users/Me/Documents/My Games/FS25/mods2"))
    );
}

// ---------------------------------------------------------------- migrations

fn schema_version(path: &Path) -> i64 {
    let conn = Connection::open(path).unwrap();
    conn.query_row("SELECT version FROM schema_version", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn both_databases_are_created_migrated_and_reopened() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("data").join("library.sqlite");
    let cache = dir.path().join("cache").join("cache.sqlite");

    let store = Store::open(&library, &cache).unwrap();
    let added = store.add_profile(Path::new("C:\\mods"), "mods").unwrap();
    store
        .put_mod(&mod_path("C:\\mods", "a.zip"), stamp(1, 1), &record("a"))
        .unwrap();
    drop(store);
    assert_eq!(schema_version(&library), LIBRARY_MIGRATIONS.len() as i64);
    assert_eq!(schema_version(&cache), CACHE_MIGRATIONS.len() as i64);

    // Opening again applies nothing and loses nothing.
    let store = Store::open(&library, &cache).unwrap();
    assert_eq!(store.profiles().unwrap(), vec![added]);
    assert!(store
        .cached_mod(&mod_path("C:\\mods", "a.zip"), stamp(1, 1))
        .unwrap()
        .is_some());
    drop(store);
    assert_eq!(schema_version(&library), LIBRARY_MIGRATIONS.len() as i64);
    assert_eq!(schema_version(&cache), CACHE_MIGRATIONS.len() as i64);
}

#[test]
fn migrations_run_in_order_from_any_older_version() {
    let mut conn = Connection::open_in_memory().unwrap();
    let steps = [
        "CREATE TABLE a (x)",
        "CREATE TABLE b (x)",
        "CREATE TABLE c (x)",
    ];
    migrate(&mut conn, &steps[..1]).unwrap();
    migrate(&mut conn, &steps[..1]).unwrap();
    migrate(&mut conn, &steps).unwrap();
    let version: i64 = conn
        .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 3);
    let tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name IN ('a', 'b', 'c')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tables, 3);
}

#[test]
fn a_corrupt_cache_is_deleted_and_recreated() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("library.sqlite");
    let cache = dir.path().join("cache.sqlite");
    fs::write(
        &cache,
        b"this is not a sqlite database, just text".repeat(50),
    )
    .unwrap();

    let store = Store::open(&library, &cache).unwrap();
    store
        .put_mod(&mod_path("C:\\mods", "a.zip"), stamp(1, 1), &record("a"))
        .unwrap();
    assert!(store
        .cached_mod(&mod_path("C:\\mods", "a.zip"), stamp(1, 1))
        .unwrap()
        .is_some());
}

#[test]
fn a_cache_from_a_newer_schema_is_recreated() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("library.sqlite");
    let cache = dir.path().join("cache.sqlite");
    drop(Store::open(&library, &cache).unwrap());
    {
        let conn = Connection::open(&cache).unwrap();
        conn.execute("UPDATE schema_version SET version = 99", [])
            .unwrap();
    }
    let store = Store::open(&library, &cache).unwrap();
    store
        .put_mod(&mod_path("C:\\mods", "a.zip"), stamp(1, 1), &record("a"))
        .unwrap();
    drop(store);
    assert_eq!(schema_version(&cache), CACHE_MIGRATIONS.len() as i64);
}

#[test]
fn a_corrupt_library_is_an_error_and_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("library.sqlite");
    let cache = dir.path().join("cache.sqlite");
    let garbage = b"definitely not sqlite, but the user's data".repeat(50);
    fs::write(&library, &garbage).unwrap();

    assert!(Store::open(&library, &cache).is_err());
    assert_eq!(fs::read(&library).unwrap(), garbage);
}

#[test]
fn a_library_from_a_newer_schema_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("library.sqlite");
    let cache = dir.path().join("cache.sqlite");
    drop(Store::open(&library, &cache).unwrap());
    {
        let conn = Connection::open(&library).unwrap();
        conn.execute("UPDATE schema_version SET version = 99", [])
            .unwrap();
    }
    assert!(matches!(
        Store::open(&library, &cache),
        Err(StoreError::Corrupt(_))
    ));
}

#[test]
fn the_cache_uses_wal_mode() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache.sqlite");
    let store = Store::open(&dir.path().join("library.sqlite"), &cache).unwrap();
    let mode: String = lock(&store.cache)
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

// ---------------------------------------------------------------- registry

#[test]
fn profile_ids_are_p_plus_rowid_in_registry_order() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.profiles().unwrap().is_empty());
    let a = store.add_profile(Path::new("C:\\a"), "a").unwrap();
    let b = store.add_profile(Path::new("C:\\b"), "b").unwrap();
    let c = store.add_profile(Path::new("C:\\c"), "c").unwrap();
    assert_eq!(
        (a.id.as_str(), b.id.as_str(), c.id.as_str()),
        ("p1", "p2", "p3")
    );
    assert_eq!(store.profiles().unwrap(), vec![a, b, c]);
}

#[test]
fn the_same_folder_in_another_spelling_is_the_same_profile() {
    let store = Store::open_in_memory().unwrap();
    let first = store
        .add_profile(Path::new("C:\\Users\\Me\\FS25\\mods"), "mods")
        .unwrap();
    let again = store
        .add_profile(Path::new("c:/users/me/fs25/MODS/"), "other name")
        .unwrap();
    assert_eq!(first, again);
    assert_eq!(first.name, "mods");
    assert_eq!(store.profiles().unwrap().len(), 1);
    let next = store.add_profile(Path::new("C:\\other"), "other").unwrap();
    assert_eq!(next.id, "p2");
}

// ---------------------------------------------------------------- cache

#[test]
fn a_record_round_trips_when_path_size_and_mtime_match() {
    let store = Store::open_in_memory().unwrap();
    let path = mod_path("C:\\mods", "FS25_A.zip");
    let stored = record("FS25_A");
    assert_eq!(store.cached_mod(&path, stamp(5, 100)).unwrap(), None);
    store.put_mod(&path, stamp(5, 100), &stored).unwrap();
    assert_eq!(
        store.cached_mod(&path, stamp(5, 100)).unwrap(),
        Some(stored.clone())
    );
    // The same file spelled differently is the same key.
    assert_eq!(
        store
            .cached_mod(Path::new("c:/MODS/fs25_a.zip"), stamp(5, 100))
            .unwrap(),
        Some(stored)
    );
}

#[test]
fn a_record_without_an_icon_round_trips() {
    let store = Store::open_in_memory().unwrap();
    let path = mod_path("C:\\mods", "a.zip");
    let stored = CachedMod {
        entry: sample_entry("a"),
        icon: None,
        fingerprint: String::new(),
    };
    store.put_mod(&path, stamp(1, 1), &stored).unwrap();
    assert_eq!(store.cached_mod(&path, stamp(1, 1)).unwrap(), Some(stored));
}

#[test]
fn a_changed_size_or_mtime_is_a_miss() {
    let store = Store::open_in_memory().unwrap();
    let path = mod_path("C:\\mods", "a.zip");
    store.put_mod(&path, stamp(5, 100), &record("a")).unwrap();
    assert_eq!(store.cached_mod(&path, stamp(6, 100)).unwrap(), None);
    assert_eq!(store.cached_mod(&path, stamp(5, 101)).unwrap(), None);
    assert!(store.cached_mod(&path, stamp(5, 100)).unwrap().is_some());

    let mut folder = store.cached_mods_in(Path::new("C:\\mods")).unwrap();
    assert!(folder.take(&path, stamp(6, 100)).is_none());
    assert!(folder.take(&path, stamp(5, 101)).is_none());
    assert!(folder.take(&path, stamp(5, 100)).is_some());
    assert!(folder.take(&path, stamp(5, 100)).is_none(), "taken once");
}

#[test]
fn a_different_scan_version_is_a_miss() {
    let store = Store::open_in_memory().unwrap();
    let path = mod_path("C:\\mods", "a.zip");
    let item = record("a");
    store
        .put_mods_versioned(&[(path.as_path(), stamp(1, 1), &item)], SCAN_VERSION)
        .unwrap();
    assert!(store
        .cached_mod_versioned(&path, stamp(1, 1), SCAN_VERSION)
        .unwrap()
        .is_some());
    assert!(store
        .cached_mod_versioned(&path, stamp(1, 1), SCAN_VERSION + 1)
        .unwrap()
        .is_none());
    let folder = Path::new("C:\\mods");
    assert_eq!(
        store
            .cached_mods_in_versioned(folder, SCAN_VERSION)
            .unwrap()
            .len(),
        1
    );
    assert!(store
        .cached_mods_in_versioned(folder, SCAN_VERSION + 1)
        .unwrap()
        .is_empty());
}

#[test]
fn a_row_that_cannot_be_decoded_is_a_miss() {
    let store = Store::open_in_memory().unwrap();
    let path = mod_path("C:\\mods", "a.zip");
    store.put_mod(&path, stamp(1, 1), &record("a")).unwrap();
    lock(&store.cache)
        .execute(
            "UPDATE mods SET entry_json = '{\"from\":\"the future\"}'",
            [],
        )
        .unwrap();
    assert_eq!(store.cached_mod(&path, stamp(1, 1)).unwrap(), None);
    assert!(store
        .cached_mods_in(Path::new("C:\\mods"))
        .unwrap()
        .is_empty());
}

#[test]
fn batches_are_stored_and_read_per_folder() {
    let store = Store::open_in_memory().unwrap();
    let in_a: Vec<(PathBuf, CachedMod)> = (0..50)
        .map(|i| {
            (
                mod_path("C:\\a", &format!("m{i}.zip")),
                record(&format!("m{i}")),
            )
        })
        .collect();
    let in_b = (mod_path("C:\\b", "m0.zip"), record("m0"));
    let mut items: Vec<(&Path, FileStamp, &CachedMod)> = in_a
        .iter()
        .map(|(p, r)| (p.as_path(), stamp(7, 7), r))
        .collect();
    items.push((in_b.0.as_path(), stamp(7, 7), &in_b.1));
    store.put_mods(&items).unwrap();

    let mut folder = store.cached_mods_in(Path::new("c:/A/")).unwrap();
    assert_eq!(folder.len(), 50);
    for (path, expected) in &in_a {
        assert_eq!(folder.take(path, stamp(7, 7)).as_ref(), Some(expected));
    }
    assert!(folder.is_empty());
    assert_eq!(store.cached_mods_in(Path::new("C:\\b")).unwrap().len(), 1);
    assert!(store.cached_mods_in(Path::new("C:\\c")).unwrap().is_empty());
}

#[test]
fn only_files_directly_inside_the_folder_are_in_its_batch() {
    let store = Store::open_in_memory().unwrap();
    store
        .put_mod(&mod_path("C:\\mods", "a.zip"), stamp(1, 1), &record("a"))
        .unwrap();
    store
        .put_mod(
            &mod_path("C:\\mods\\sub", "b.zip"),
            stamp(1, 1),
            &record("b"),
        )
        .unwrap();
    assert_eq!(
        store.cached_mods_in(Path::new("C:\\mods")).unwrap().len(),
        1
    );
    assert_eq!(
        store
            .cached_mods_in(Path::new("C:\\mods\\sub"))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn times_before_the_epoch_are_clamped_to_zero() {
    let store = Store::open_in_memory().unwrap();
    let path = mod_path("C:\\mods", "old.zip");
    let before = FileStamp {
        size: 1,
        modified: UNIX_EPOCH - Duration::from_secs(10),
    };
    store.put_mod(&path, before, &record("old")).unwrap();
    assert!(store.cached_mod(&path, stamp(1, 0)).unwrap().is_some());
}

#[test]
fn prune_removes_records_and_thumbnails_of_absent_files_only_in_that_folder() {
    let store = Store::open_in_memory().unwrap();
    let keep = mod_path("C:\\mods", "keep.zip");
    let gone = mod_path("C:\\mods", "gone.zip");
    let elsewhere = mod_path("C:\\other", "gone.zip");
    for path in [&keep, &gone, &elsewhere] {
        store.put_mod(path, stamp(1, 1), &record("x")).unwrap();
        store.put_thumbnail(path, stamp(1, 1), b"png").unwrap();
    }

    let removed = store
        .prune(Path::new("c:/mods"), std::slice::from_ref(&keep))
        .unwrap();
    assert_eq!(removed, 1);
    assert!(store.cached_mod(&keep, stamp(1, 1)).unwrap().is_some());
    assert!(store.thumbnail(&keep, stamp(1, 1)).unwrap().is_some());
    assert!(store.cached_mod(&gone, stamp(1, 1)).unwrap().is_none());
    assert!(store.thumbnail(&gone, stamp(1, 1)).unwrap().is_none());
    assert!(store.cached_mod(&elsewhere, stamp(1, 1)).unwrap().is_some());
    assert!(store.thumbnail(&elsewhere, stamp(1, 1)).unwrap().is_some());

    assert_eq!(store.prune(Path::new("C:\\mods"), &[keep]).unwrap(), 0);
}

#[test]
fn thumbnails_round_trip_and_miss_on_a_changed_stamp() {
    let store = Store::open_in_memory().unwrap();
    let path = mod_path("C:\\mods", "a.zip");
    assert_eq!(store.thumbnail(&path, stamp(1, 1)).unwrap(), None);
    store.put_thumbnail(&path, stamp(1, 1), &[1, 2, 3]).unwrap();
    assert_eq!(
        store.thumbnail(&path, stamp(1, 1)).unwrap(),
        Some(vec![1, 2, 3])
    );
    assert_eq!(store.thumbnail(&path, stamp(2, 1)).unwrap(), None);
    assert_eq!(store.thumbnail(&path, stamp(1, 2)).unwrap(), None);
    store.put_thumbnail(&path, stamp(2, 1), &[9]).unwrap();
    assert_eq!(store.thumbnail(&path, stamp(2, 1)).unwrap(), Some(vec![9]));
    assert_eq!(store.thumbnail(&path, stamp(1, 1)).unwrap(), None);
}

#[test]
fn journal_rows_round_trip_and_ids_follow_the_rowid() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.latest_ok_op().unwrap(), None);
    assert!(store.ops().unwrap().is_empty());

    let a = ModsOverride {
        active: true,
        directory: Some(PathBuf::from("C:\\mods\\a")),
    };
    let default = ModsOverride {
        active: false,
        directory: None,
    };
    let id1 = store
        .add_op(&NewOp {
            kind: "switch",
            from: &a,
            to: &default,
            backup_file: Some("gameSettings-1.xml"),
            result: "ok",
        })
        .unwrap();
    let id2 = store
        .add_op(&NewOp {
            kind: "undo",
            from: &default,
            to: &a,
            backup_file: None,
            result: "settingsChanged",
        })
        .unwrap();
    assert_eq!((id1.as_str(), id2.as_str()), ("o1", "o2"));

    let ops = store.ops().unwrap();
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0].kind, "switch");
    assert_eq!(ops[0].from, a);
    assert_eq!(ops[0].to, default);
    assert_eq!(ops[0].backup_file.as_deref(), Some("gameSettings-1.xml"));
    assert!(ops[0].at > 0);
    assert_eq!(ops[1].backup_file, None);

    // Failed attempts are rows but are never the latest successful write.
    assert_eq!(store.latest_ok_op().unwrap().unwrap().id, "o1");
}

#[test]
fn the_journal_survives_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("library.sqlite");
    let cache = dir.path().join("cache.sqlite");
    let state = ModsOverride::default();
    {
        let store = Store::open(&library, &cache).unwrap();
        store
            .add_op(&NewOp {
                kind: "restore",
                from: &state,
                to: &state,
                backup_file: Some("b.xml"),
                result: "ok",
            })
            .unwrap();
    }
    let store = Store::open(&library, &cache).unwrap();
    assert_eq!(store.ops().unwrap().len(), 1);
    assert_eq!(store.latest_ok_op().unwrap().unwrap().kind, "restore");
}

#[test]
fn profiles_can_be_renamed_and_removed_by_id() {
    let store = Store::open_in_memory().unwrap();
    let a = store.add_profile(Path::new(r"C:\mods\a"), "a").unwrap();
    let b = store.add_profile(Path::new(r"C:\mods\b"), "b").unwrap();

    assert!(store.rename_profile(&a.id, "Renamed").unwrap());
    assert!(!store.rename_profile("p999", "x").unwrap());
    assert!(!store.rename_profile("not-an-id", "x").unwrap());
    let names: Vec<String> = store
        .profiles()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["Renamed", "b"]);

    assert!(store.remove_profile(&a.id).unwrap());
    assert!(!store.remove_profile(&a.id).unwrap());
    let left: Vec<String> = store
        .profiles()
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(left, [b.id]);
}

#[test]
fn file_operations_are_journaled_apart_from_settings_writes() {
    let store = Store::open_in_memory().unwrap();
    let id = store
        .add_file_op(&NewFileOp {
            kind: "copy",
            plan_id: Some("t1"),
            source: Path::new(r"C:\a\X.zip"),
            target: Some(Path::new(r"C:\b\X.zip")),
            result: "ok",
        })
        .unwrap();
    assert_eq!(id, "f1");
    store
        .add_file_op(&NewFileOp {
            kind: "recycle",
            plan_id: None,
            source: Path::new(r"C:\a\Y.zip"),
            target: None,
            result: "recycleUnavailable",
        })
        .unwrap();

    let rows = store.file_ops().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].kind, "copy");
    assert_eq!(rows[0].plan_id.as_deref(), Some("t1"));
    assert_eq!(rows[0].target.as_deref(), Some(Path::new(r"C:\b\X.zip")));
    assert!(rows[0].at > 0);
    assert_eq!(
        (rows[1].plan_id.clone(), rows[1].target.clone()),
        (None, None)
    );
    assert_eq!(rows[1].result, "recycleUnavailable");
    // Undo only ever sees settings writes.
    assert!(store.latest_ok_op().unwrap().is_none());
    assert!(store.ops().unwrap().is_empty());
}

// ---------------------------------------------------------------- M6: migrations

fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |r| r.get::<_, i64>(0),
    )
    .unwrap()
        == 1
}

#[test]
fn the_real_library_migrations_apply_from_every_older_version() {
    for from in 0..=LIBRARY_MIGRATIONS.len() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn, &LIBRARY_MIGRATIONS[..from]).unwrap();
        if from >= 1 {
            conn.execute(
                "INSERT INTO profiles (name, path, path_key, added_at) VALUES ('m', 'C:\\m', 'c:\\m', 1)",
                [],
            )
            .unwrap();
        }
        migrate(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, LIBRARY_MIGRATIONS.len() as i64, "from {from}");
        for table in ["profiles", "ops", "file_ops", "mod_meta"] {
            assert!(table_exists(&conn, table), "{table} from {from}");
        }
        if from >= 1 {
            let kept: i64 = conn
                .query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))
                .unwrap();
            assert_eq!(kept, 1, "profiles survive a migration from {from}");
        }
    }
}

#[test]
fn the_real_cache_migrations_apply_from_every_older_version() {
    for from in 0..=CACHE_MIGRATIONS.len() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn, &CACHE_MIGRATIONS[..from]).unwrap();
        if from >= 1 {
            conn.execute(
                "INSERT INTO mods (path_key, folder_key, size, mtime_ns, scan_version, entry_json) \
                 VALUES ('c:\\m\\a.zip', 'c:\\m', 1, 1, 1, '{}')",
                [],
            )
            .unwrap();
        }
        migrate(&mut conn, CACHE_MIGRATIONS).unwrap();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, CACHE_MIGRATIONS.len() as i64, "from {from}");
        for table in ["mods", "thumbnails", "file_hashes"] {
            assert!(table_exists(&conn, table), "{table} from {from}");
        }
        // The fingerprint column exists, and an old row gets the empty default.
        conn.execute(
            "INSERT OR REPLACE INTO mods (path_key, folder_key, size, mtime_ns, scan_version, \
             entry_json, fingerprint) VALUES ('c:\\m\\b.zip', 'c:\\m', 1, 1, 1, '{}', 'f')",
            [],
        )
        .unwrap();
        if from >= 1 {
            let fingerprint: String = conn
                .query_row(
                    "SELECT fingerprint FROM mods WHERE path_key = 'c:\\m\\a.zip'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(fingerprint, "", "from {from}");
        }
    }
}

#[test]
fn the_scan_record_fingerprint_round_trips() {
    let store = Store::open_in_memory().unwrap();
    let path = mod_path("C:\\mods", "a.zip");
    store.put_mod(&path, stamp(1, 1), &record("a")).unwrap();
    let single = store.cached_mod(&path, stamp(1, 1)).unwrap().unwrap();
    assert_eq!(single.fingerprint, "fingerprint of a");
    let mut folder = store.cached_mods_in(Path::new("C:\\mods")).unwrap();
    assert_eq!(
        folder.take(&path, stamp(1, 1)).unwrap().fingerprint,
        "fingerprint of a"
    );
}

// ---------------------------------------------------------------- M6: hash cache

#[test]
fn a_cached_hash_hits_only_for_the_same_size_mtime_and_salt() {
    let store = Store::open_in_memory().unwrap();
    let path = mod_path("C:\\mods", "FS25_A.zip");
    store
        .put_hashes(&[((path.as_path(), stamp(5, 100), "FS25_A"), "abc123")])
        .unwrap();
    let get = |stamp: FileStamp, salt: &str| {
        store
            .cached_hashes(&[(path.as_path(), stamp, salt)])
            .unwrap()
            .remove(0)
    };
    assert_eq!(get(stamp(5, 100), "FS25_A").as_deref(), Some("abc123"));
    assert_eq!(get(stamp(6, 100), "FS25_A"), None, "size changed");
    assert_eq!(get(stamp(5, 101), "FS25_A"), None, "mtime changed");
    assert_eq!(get(stamp(5, 100), "FS25_B"), None, "other salt");
}

#[test]
fn hash_lookups_keep_the_order_asked_and_ignore_path_spelling() {
    let store = Store::open_in_memory().unwrap();
    let a = mod_path("C:\\mods", "FS25_A.zip");
    let b = mod_path("C:\\mods", "FS25_B.zip");
    store
        .put_hashes(&[
            ((a.as_path(), stamp(1, 1), "FS25_A"), "aaa"),
            ((b.as_path(), stamp(2, 2), "FS25_B"), "bbb"),
        ])
        .unwrap();
    let shouted = mod_path("c:/MODS", "fs25_a.ZIP");
    let found = store
        .cached_hashes(&[
            (b.as_path(), stamp(2, 2), "FS25_B"),
            (
                mod_path("C:\\mods", "none.zip").as_path(),
                stamp(1, 1),
                "none",
            ),
            (shouted.as_path(), stamp(1, 1), "FS25_A"),
        ])
        .unwrap();
    assert_eq!(
        found,
        vec![Some("bbb".to_string()), None, Some("aaa".to_string())]
    );
}

#[test]
fn a_new_hash_replaces_the_old_one_for_the_same_path_and_salt() {
    let store = Store::open_in_memory().unwrap();
    let path = mod_path("C:\\mods", "FS25_A.zip");
    store
        .put_hashes(&[((path.as_path(), stamp(1, 1), "FS25_A"), "old")])
        .unwrap();
    store
        .put_hashes(&[((path.as_path(), stamp(2, 2), "FS25_A"), "new")])
        .unwrap();
    let query = |size, secs| {
        store
            .cached_hashes(&[(path.as_path(), stamp(size, secs), "FS25_A")])
            .unwrap()
            .remove(0)
    };
    assert_eq!(query(1, 1), None);
    assert_eq!(query(2, 2).as_deref(), Some("new"));
}

#[test]
fn cached_hashes_survive_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("library.sqlite");
    let cache = dir.path().join("cache.sqlite");
    let path = mod_path("C:\\mods", "FS25_A.zip");
    let store = Store::open(&library, &cache).unwrap();
    store
        .put_hashes(&[((path.as_path(), stamp(1, 1), "FS25_A"), "abc")])
        .unwrap();
    drop(store);
    let store = Store::open(&library, &cache).unwrap();
    assert_eq!(
        store
            .cached_hashes(&[(path.as_path(), stamp(1, 1), "FS25_A")])
            .unwrap(),
        vec![Some("abc".to_string())]
    );
}

// ---------------------------------------------------------------- M6: tags and notes

fn tags(list: &[&str]) -> Vec<String> {
    list.iter().map(|t| (*t).to_string()).collect()
}

#[test]
fn tags_and_a_note_are_stored_per_name_key() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.mod_meta("fs25_a").unwrap(), ModMeta::default());
    store
        .set_mod_tags("fs25_a", &tags(&["Tractor", "Old"]))
        .unwrap();
    store.set_mod_note("fs25_a", Some("my note")).unwrap();
    let meta = store.mod_meta("fs25_a").unwrap();
    assert_eq!(meta.tags, tags(&["Tractor", "Old"]));
    assert_eq!(meta.note.as_deref(), Some("my note"));
    // Setting one half leaves the other.
    store.set_mod_tags("fs25_a", &tags(&["New"])).unwrap();
    let meta = store.mod_meta("fs25_a").unwrap();
    assert_eq!(meta.tags, tags(&["New"]));
    assert_eq!(meta.note.as_deref(), Some("my note"));
    assert_eq!(store.all_mod_meta().unwrap().len(), 1);
}

#[test]
fn the_row_goes_when_no_tag_and_no_note_remain() {
    let store = Store::open_in_memory().unwrap();
    store.set_mod_tags("fs25_a", &tags(&["x"])).unwrap();
    store.set_mod_note("fs25_a", Some("n")).unwrap();
    store.set_mod_tags("fs25_a", &[]).unwrap();
    assert_eq!(store.all_mod_meta().unwrap().len(), 1, "the note keeps it");
    store.set_mod_note("fs25_a", None).unwrap();
    assert!(store.all_mod_meta().unwrap().is_empty());

    // Clearing what was never set leaves no row either.
    store.set_mod_tags("fs25_b", &[]).unwrap();
    store.set_mod_note("fs25_b", None).unwrap();
    assert!(store.all_mod_meta().unwrap().is_empty());
}

#[test]
fn all_tags_are_distinct_case_insensitively_and_sorted() {
    let store = Store::open_in_memory().unwrap();
    store
        .set_mod_tags("fs25_a", &tags(&["beta", "Alpha"]))
        .unwrap();
    store
        .set_mod_tags("fs25_b", &tags(&["ALPHA", "gamma", "Beta"]))
        .unwrap();
    // The first spelling in mod-name order is kept.
    assert_eq!(store.all_tags().unwrap(), tags(&["Alpha", "beta", "gamma"]));
}

#[test]
fn metadata_survives_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("library.sqlite");
    let cache = dir.path().join("cache.sqlite");
    let store = Store::open(&library, &cache).unwrap();
    store.set_mod_tags("fs25_a", &tags(&["x"])).unwrap();
    store.set_mod_note("fs25_a", Some("n")).unwrap();
    drop(store);
    let store = Store::open(&library, &cache).unwrap();
    let meta = store.all_mod_meta().unwrap();
    assert_eq!(meta["fs25_a"].tags, tags(&["x"]));
    assert_eq!(meta["fs25_a"].note.as_deref(), Some("n"));
}

#[test]
fn store_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Store>();
}
