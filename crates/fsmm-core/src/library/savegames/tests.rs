//! Savegame reports and hashing on generated saves, folders and a fake install folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use md5::{Digest, Md5};

use super::super::fixtures::{add, folder, library, write_folder_mod, write_mod};
use super::*;
use crate::testutil::{save_mod_xml, save_xml, write_save_file, write_zip, Method};

type Hook = Arc<dyn Fn(&Path) + Send + Sync>;

/// Test hooks around each hashed file (see `HashState`).
#[derive(Default, Clone)]
pub(super) struct Hooks {
    pub(super) before: Option<Hook>,
    pub(super) after: Option<Hook>,
}

struct Env {
    lib: Arc<Library>,
    _tmp: tempfile::TempDir,
    user: PathBuf,
    install: PathBuf,
    mods: PathBuf,
    profile: String,
    /// Every file `hash_one` started on, in order.
    hashed: Arc<Mutex<Vec<PathBuf>>>,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let user = folder(tmp.path(), "user");
    let install = folder(tmp.path(), "install");
    fs::create_dir_all(install.join("pdlc")).unwrap();
    let mods = folder(tmp.path(), "mods");
    let lib = Arc::new(library());
    let profile = add(&lib, &mods).id;
    let hashed = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&hashed);
    lib.hashing.hooks.lock().unwrap().before = Some(Arc::new(move |path: &Path| {
        log.lock().unwrap().push(path.to_path_buf());
    }));
    Env {
        lib,
        _tmp: tmp,
        user,
        install,
        mods,
        profile,
        hashed,
    }
}

impl Env {
    fn ctx(&self) -> GameContext {
        GameContext {
            user_dir: Some(self.user.clone()),
            install_dir: Some(self.install.clone()),
            languages: vec!["en".to_string()],
            ..GameContext::default()
        }
    }

    fn save(&self, id: &str, mods: &[String]) {
        write_save_file(&self.user, id, &save_xml(mods));
    }

    fn report(&self, save: &str) -> SavegameReport {
        self.lib
            .savegame_report(save, &self.profile, &self.ctx())
            .unwrap()
    }

    fn check(&self, save: &str) -> Result<SavegameReport, ApiError> {
        self.lib
            .check_savegame_files(save, &self.profile, &self.ctx(), &|_| {})
    }

    fn hashed(&self) -> Vec<String> {
        self.hashed
            .lock()
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    /// A second registered folder.
    fn other_profile(&self, name: &str) -> (String, PathBuf) {
        let dir = folder(self.mods.parent().unwrap(), name);
        (add(&self.lib, &dir).id, dir)
    }
}

/// The game's hash, computed independently of `game_md5`: md5(file bytes ++ mod name).
fn game_hash(path: &Path, mod_name: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(fs::read(path).unwrap());
    hasher.update(mod_name.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn state_of(report: &SavegameReport, name: &str) -> SaveModState {
    row(report, name).state
}

fn row<'a>(report: &'a SavegameReport, name: &str) -> &'a SaveModRow {
    report.rows.iter().find(|r| r.mod_name == name).unwrap()
}

fn entry(name: &str, version: &str, required: bool, hash: &str) -> String {
    save_mod_xml(name, version, Some(required), hash)
}

// ---------------------------------------------------------------- states

#[test]
fn every_state_is_reported_and_only_the_needed_zips_are_hashed() {
    let e = env();
    let map = write_mod(&e.mods, "FS25_Map.zip", "1.0", b"map");
    write_mod(&e.mods, "FS25_Differs.zip", "1.0", b"differs");
    write_mod(&e.mods, "FS25_Version.zip", "2.0", b"v");
    write_folder_mod(&e.mods, "FS25_Folder", "1.0", b"f");
    write_mod(&e.mods, "FS25_NoHash.zip", "1.0", b"n");
    write_mod(&e.mods, "FS25_Extra.zip", "1.0", b"extra");
    fs::write(e.install.join("pdlc").join("installed.dlc"), b"x").unwrap();
    e.save(
        "savegame1",
        &[
            entry("FS25_Map", "1.0", true, &game_hash(&map, "FS25_Map")),
            entry("FS25_Differs", "1.0", false, &"0".repeat(32)),
            // A hash that would be wrong, but the versions differ, so it is never computed.
            entry("FS25_Version", "1.0", false, &"1".repeat(32)),
            entry("FS25_Folder", "1.0", false, &"2".repeat(32)),
            entry("FS25_NoHash", "1.0", false, ""),
            entry("FS25_Gone", "1.0", false, &"3".repeat(32)),
            entry("pdlc_installed", "1.0.0.0", false, ""),
            entry("pdlc_gone", "1.0.0.0", false, ""),
        ],
    );

    let before = e.report("savegame1");
    assert_eq!(state_of(&before, "FS25_Map"), SaveModState::HashPending);
    assert_eq!(state_of(&before, "FS25_Differs"), SaveModState::HashPending);
    assert!(e.hashed().is_empty(), "the report hashes nothing");

    let report = e.check("savegame1").unwrap();
    let expected = [
        ("FS25_Map", SaveModState::Present),
        ("FS25_Differs", SaveModState::FileDiffers),
        ("FS25_Version", SaveModState::VersionDiffers),
        ("FS25_Folder", SaveModState::Present),
        ("FS25_NoHash", SaveModState::Present),
        ("FS25_Gone", SaveModState::Missing),
        ("pdlc_installed", SaveModState::DlcInstalled),
        ("pdlc_gone", SaveModState::DlcMissing),
    ];
    for (name, state) in expected {
        assert_eq!(state_of(&report, name), state, "{name}");
    }
    assert!(report
        .rows
        .iter()
        .all(|r| r.state != SaveModState::HashPending));

    let mut hashed = e.hashed();
    hashed.sort();
    assert_eq!(hashed, ["FS25_Differs.zip", "FS25_Map.zip"]);
}

#[test]
fn installed_copies_are_described_for_every_row_that_has_one() {
    let e = env();
    write_mod(&e.mods, "FS25_A.zip", "1.0", b"a");
    write_mod(&e.mods, "FS25_B.zip", "2.0", b"b");
    e.save(
        "savegame1",
        &[
            entry("FS25_A", "1.0", false, ""),
            entry("FS25_B", "1.0", false, ""),
            entry("FS25_C", "1.0", false, ""),
        ],
    );
    let report = e.report("savegame1");
    let a = row(&report, "FS25_A").installed.as_ref().unwrap();
    assert_eq!(a.mod_id, "FS25_A.zip");
    assert_eq!(a.summary.version, "1.0");
    let b = row(&report, "FS25_B").installed.as_ref().unwrap();
    assert_eq!(b.summary.version, "2.0");
    assert!(row(&report, "FS25_C").installed.is_none());
    assert_eq!(row(&report, "FS25_B").saved_version, "1.0");
    assert_eq!(row(&report, "FS25_B").title, "Title of FS25_B");
}

#[test]
fn versions_are_compared_trimmed() {
    let e = env();
    write_mod(&e.mods, "FS25_A.zip", "1.0 ", b"a");
    e.save("savegame1", &[entry("FS25_A", " 1.0", false, "")]);
    assert_eq!(
        state_of(&e.report("savegame1"), "FS25_A"),
        SaveModState::Present
    );
}

#[test]
fn names_pair_ignoring_case_and_the_hash_uses_the_files_own_name() {
    let e = env();
    let path = write_mod(&e.mods, "fs25_case.zip", "1.0", b"c");
    e.save(
        "savegame1",
        &[
            entry("FS25_Case", "1.0", false, &game_hash(&path, "fs25_case")),
            entry("FS25_Other", "1.0", false, ""),
        ],
    );
    let report = e.check("savegame1").unwrap();
    assert_eq!(state_of(&report, "FS25_Case"), SaveModState::Present);
    // The mod the save names with another case is not an extra.
    assert!(report.extras.is_empty());
}

#[test]
fn a_hash_in_upper_case_hex_still_matches() {
    let e = env();
    let path = write_mod(&e.mods, "FS25_A.zip", "1.0", b"a");
    e.save(
        "savegame1",
        &[entry(
            "FS25_A",
            "1.0",
            false,
            &game_hash(&path, "FS25_A").to_uppercase(),
        )],
    );
    assert_eq!(
        state_of(&e.check("savegame1").unwrap(), "FS25_A"),
        SaveModState::Present
    );
}

#[test]
fn a_missing_required_attribute_makes_the_mod_required_and_first() {
    let e = env();
    e.save(
        "savegame1",
        &[
            entry("FS25_A", "1", false, ""),
            save_mod_xml("FS25_Z", "1", None, ""),
        ],
    );
    let names: Vec<_> = e
        .report("savegame1")
        .rows
        .into_iter()
        .map(|r| (r.mod_name, r.required))
        .collect();
    assert_eq!(
        names,
        [("FS25_Z".to_string(), true), ("FS25_A".to_string(), false)]
    );
}

#[test]
fn rows_are_required_first_then_by_lower_cased_name() {
    let e = env();
    e.save(
        "savegame1",
        &[
            entry("FS25_b", "1", false, ""),
            entry("FS25_Map", "1", true, ""),
            entry("FS25_C", "1", false, ""),
            entry("fs25_a", "1", false, ""),
            entry("pdlc_x", "1", false, ""),
        ],
    );
    let names: Vec<_> = e
        .report("savegame1")
        .rows
        .into_iter()
        .map(|r| r.mod_name)
        .collect();
    assert_eq!(names, ["FS25_Map", "fs25_a", "FS25_b", "FS25_C", "pdlc_x"]);
}

// ---------------------------------------------------------------- DLCs

#[test]
fn a_dlc_is_installed_when_its_dlc_file_exists_in_the_install_folder() {
    let e = env();
    fs::write(
        e.install.join("pdlc").join("highlandsFishingPack.dlc"),
        b"x",
    )
    .unwrap();
    e.save(
        "savegame1",
        &[
            entry("pdlc_highlandsFishingPack", "1.0.0.0", false, ""),
            entry("pdlc_other", "1.0.0.0", false, "abc"),
        ],
    );
    let report = e.check("savegame1").unwrap();
    assert_eq!(
        state_of(&report, "pdlc_highlandsFishingPack"),
        SaveModState::DlcInstalled
    );
    assert_eq!(state_of(&report, "pdlc_other"), SaveModState::DlcMissing);
    assert!(row(&report, "pdlc_other").installed.is_none());
    assert!(e.hashed().is_empty(), "DLCs are never hashed");
}

#[test]
fn without_an_install_folder_every_dlc_is_missing() {
    let e = env();
    fs::write(e.install.join("pdlc").join("pack.dlc"), b"x").unwrap();
    e.save("savegame1", &[entry("pdlc_pack", "1", false, "")]);
    let ctx = GameContext {
        install_dir: None,
        ..e.ctx()
    };
    let report = e
        .lib
        .savegame_report("savegame1", &e.profile, &ctx)
        .unwrap();
    assert_eq!(state_of(&report, "pdlc_pack"), SaveModState::DlcMissing);
}

#[test]
fn a_dlc_name_cannot_leave_the_pdlc_folder() {
    let e = env();
    fs::write(e.install.join("secret.dlc"), b"x").unwrap();
    e.save(
        "savegame1",
        &[
            entry("pdlc_..\\secret", "1", false, ""),
            entry("pdlc_../secret", "1", false, ""),
            entry("pdlc_", "1", false, ""),
        ],
    );
    let report = e.report("savegame1");
    assert!(report
        .rows
        .iter()
        .all(|r| r.state == SaveModState::DlcMissing));
}

#[test]
fn a_dlc_is_not_an_extra_and_does_not_need_a_folder_copy() {
    let e = env();
    write_mod(&e.mods, "pdlc_oddity.zip", "1.0", b"x");
    e.save("savegame1", &[entry("pdlc_oddity", "1.0", false, "")]);
    let report = e.report("savegame1");
    // The save's own name is a save entry, so the profile's file is not an extra either.
    assert!(report.extras.is_empty());
    assert_eq!(state_of(&report, "pdlc_oddity"), SaveModState::DlcMissing);
}

// ---------------------------------------------------------------- found_in and extras

#[test]
fn found_in_lists_the_other_profiles_that_have_the_name() {
    let e = env();
    let (p2, dir2) = e.other_profile("second");
    let (p3, dir3) = e.other_profile("third");
    write_mod(&e.mods, "FS25_Here.zip", "1.0", b"h");
    write_mod(&dir2, "FS25_Here.zip", "1.0", b"h");
    write_mod(&dir2, "FS25_Missing.zip", "1.0", b"m");
    write_mod(&dir3, "fs25_missing.zip", "1.0", b"m");
    write_folder_mod(&dir3, "FS25_Here", "1.0", b"h");
    // Other profiles are read from their last scans, as `also_in` is; a listing refreshes them.
    e.lib.list_profiles(&e.ctx()).unwrap();
    e.save(
        "savegame1",
        &[
            entry("FS25_Here", "1.0", false, ""),
            entry("FS25_Missing", "1.0", false, ""),
            entry("FS25_Nowhere", "1.0", false, ""),
            entry("pdlc_x", "1.0", false, ""),
        ],
    );
    let report = e.report("savegame1");
    assert_eq!(row(&report, "FS25_Here").found_in, [p2.clone(), p3.clone()]);
    assert_eq!(row(&report, "FS25_Missing").found_in, [p2, p3]);
    assert!(row(&report, "FS25_Nowhere").found_in.is_empty());
    assert!(row(&report, "pdlc_x").found_in.is_empty());
    // The profile itself is never in its own list.
    assert!(!row(&report, "FS25_Here").found_in.contains(&e.profile));
}

#[test]
fn extras_are_the_profiles_mods_the_save_does_not_use_sorted_by_name() {
    let e = env();
    write_mod(&e.mods, "FS25_Used.zip", "1.0", b"u");
    write_mod(&e.mods, "FS25_zeta.zip", "1.0", b"z");
    write_mod(&e.mods, "FS25_Alpha.zip", "1.0", b"a");
    write_folder_mod(&e.mods, "FS25_Mid", "1.0", b"m");
    e.save("savegame1", &[entry("FS25_Used", "1.0", false, "")]);
    let report = e.report("savegame1");
    assert_eq!(
        report.extras,
        ["FS25_Alpha.zip", "FS25_Mid", "FS25_zeta.zip"]
    );
}

// ---------------------------------------------------------------- the cache

#[test]
fn a_cached_hash_makes_the_report_final_and_is_not_computed_again() {
    let e = env();
    let a = write_mod(&e.mods, "FS25_A.zip", "1.0", b"a");
    e.save(
        "savegame1",
        &[entry("FS25_A", "1.0", false, &game_hash(&a, "FS25_A"))],
    );
    assert_eq!(
        state_of(&e.report("savegame1"), "FS25_A"),
        SaveModState::HashPending
    );
    e.check("savegame1").unwrap();
    assert_eq!(e.hashed().len(), 1);

    // Now the report alone is final, and another check has nothing to do.
    assert_eq!(
        state_of(&e.report("savegame1"), "FS25_A"),
        SaveModState::Present
    );
    let again = e.check("savegame1").unwrap();
    assert_eq!(state_of(&again, "FS25_A"), SaveModState::Present);
    assert_eq!(
        e.hashed().len(),
        1,
        "an unchanged zip is never hashed twice"
    );
}

#[test]
fn a_changed_zip_is_hashed_again() {
    let e = env();
    let a = write_mod(&e.mods, "FS25_A.zip", "1.0", b"a");
    let hash = game_hash(&a, "FS25_A");
    e.save("savegame1", &[entry("FS25_A", "1.0", false, &hash)]);
    e.check("savegame1").unwrap();

    // Another file of the same version: the size or mtime differs.
    write_mod(&e.mods, "FS25_A.zip", "1.0", b"changed content, longer");
    let report = e.report("savegame1");
    assert_eq!(state_of(&report, "FS25_A"), SaveModState::HashPending);
    let report = e.check("savegame1").unwrap();
    assert_eq!(state_of(&report, "FS25_A"), SaveModState::FileDiffers);
    assert_eq!(e.hashed().len(), 2);
}

#[test]
fn the_hash_cache_is_per_file_not_per_save() {
    let e = env();
    let a = write_mod(&e.mods, "FS25_A.zip", "1.0", b"a");
    let hash = game_hash(&a, "FS25_A");
    e.save("savegame1", &[entry("FS25_A", "1.0", false, &hash)]);
    e.save(
        "savegame2",
        &[entry("FS25_A", "1.0", false, &"f".repeat(32))],
    );
    e.check("savegame1").unwrap();
    // The second save reads the same cached hash and finds it different.
    assert_eq!(
        state_of(&e.report("savegame2"), "FS25_A"),
        SaveModState::FileDiffers
    );
    assert_eq!(e.hashed().len(), 1);
}

// ---------------------------------------------------------------- end to end and progress

#[test]
fn hashing_end_to_end_matches_an_independent_computation() {
    let e = env();
    // Several files, one of more than one chunk.
    let big = vec![9u8; 5 * 1024 * 1024];
    let paths = [
        write_mod(&e.mods, "FS25_One.zip", "1.0", b"one"),
        write_mod(&e.mods, "FS25_Two.zip", "1.0", &big),
        write_mod(&e.mods, "FS25_Three.zip", "1.0", b"three"),
    ];
    let names = ["FS25_One", "FS25_Two", "FS25_Three"];
    let mods: Vec<String> = paths
        .iter()
        .zip(names)
        .map(|(p, n)| entry(n, "1.0", false, &game_hash(p, n)))
        .collect();
    e.save("savegame1", &mods);
    let report = e.check("savegame1").unwrap();
    for name in names {
        assert_eq!(state_of(&report, name), SaveModState::Present, "{name}");
    }
    // The stored hash is the independent one.
    let stamp = |p: &Path| {
        let meta = fs::metadata(p).unwrap();
        crate::store::FileStamp {
            size: meta.len(),
            modified: meta.modified().unwrap(),
        }
    };
    let stored = e
        .lib
        .store
        .cached_hashes(&[(paths[1].as_path(), stamp(&paths[1]), "FS25_Two")])
        .unwrap();
    assert_eq!(stored, [Some(game_hash(&paths[1], "FS25_Two"))]);
}

#[test]
fn progress_has_a_first_and_a_last_event_and_never_goes_backwards() {
    let e = env();
    let mut mods = Vec::new();
    let mut total = 0;
    for i in 0..4u8 {
        let name = format!("FS25_M{i}");
        let path = write_mod(
            &e.mods,
            &format!("{name}.zip"),
            "1.0",
            &vec![i; 3 * 1024 * 1024],
        );
        total += fs::metadata(&path).unwrap().len();
        mods.push(entry(&name, "1.0", false, &game_hash(&path, &name)));
    }
    e.save("savegame1", &mods);
    let events = Mutex::new(Vec::new());
    e.lib
        .check_savegame_files("savegame1", &e.profile, &e.ctx(), &|p| {
            events.lock().unwrap().push(p);
        })
        .unwrap();
    let events = events.into_inner().unwrap();
    let first = events.first().unwrap();
    assert_eq!((first.done_files, first.done_bytes), (0, 0));
    assert_eq!((first.total_files, first.total_bytes), (4, total));
    let last = events.last().unwrap();
    assert_eq!((last.done_files, last.done_bytes), (4, total));
    for pair in events.windows(2) {
        assert!(pair[0].done_files <= pair[1].done_files);
        assert!(pair[0].done_bytes <= pair[1].done_bytes);
        assert_eq!(pair[1].total_files, 4);
        assert_eq!(pair[1].total_bytes, total);
    }
    assert!(events.iter().all(|p| p.done_bytes <= p.total_bytes));
}

#[test]
fn nothing_to_hash_still_reports_a_first_and_a_last_event() {
    let e = env();
    e.save("savegame1", &[entry("FS25_Gone", "1.0", false, "abc")]);
    let events = Mutex::new(Vec::new());
    let report = e
        .lib
        .check_savegame_files("savegame1", &e.profile, &e.ctx(), &|p| {
            events.lock().unwrap().push(p);
        })
        .unwrap();
    assert_eq!(state_of(&report, "FS25_Gone"), SaveModState::Missing);
    let events = events.into_inner().unwrap();
    assert!(events.len() >= 2);
    assert!(events
        .iter()
        .all(|p| p.total_files == 0 && p.total_bytes == 0));
}

// ---------------------------------------------------------------- cancel, busy, unreadable

#[test]
fn cancel_stops_the_run_and_keeps_the_hashes_finished_before() {
    let e = env();
    let a = write_mod(&e.mods, "FS25_A.zip", "1.0", b"a");
    let b = write_mod(&e.mods, "FS25_B.zip", "1.0", &vec![7u8; 6 * 1024 * 1024]);
    e.save(
        "savegame1",
        &[
            entry("FS25_A", "1.0", false, &game_hash(&a, "FS25_A")),
            entry("FS25_B", "1.0", false, &game_hash(&b, "FS25_B")),
        ],
    );
    // A finishes first and asks for the cancel; B is held back until then, so it is stopped.
    let a_done = Arc::new(AtomicBool::new(false));
    {
        let mut hooks = e.lib.hashing.hooks.lock().unwrap();
        let waiting = Arc::clone(&a_done);
        hooks.before = Some(Arc::new(move |path: &Path| {
            if path.ends_with("FS25_B.zip") {
                let started = Instant::now();
                while !waiting.load(Ordering::SeqCst) && started.elapsed() < Duration::from_secs(5)
                {
                    std::thread::yield_now();
                }
            }
        }));
        let lib = Arc::clone(&e.lib);
        let done = Arc::clone(&a_done);
        hooks.after = Some(Arc::new(move |path: &Path| {
            if path.ends_with("FS25_A.zip") {
                lib.cancel_hashing();
                done.store(true, Ordering::SeqCst);
            }
        }));
    }

    let err = e.check("savegame1").unwrap_err();
    assert_eq!(err.code, ErrorCode::Cancelled);

    let report = e.report("savegame1");
    assert_eq!(state_of(&report, "FS25_A"), SaveModState::Present, "kept");
    assert_eq!(state_of(&report, "FS25_B"), SaveModState::HashPending);

    // A later run starts clean (the cancel does not linger) and finishes B.
    *e.lib.hashing.hooks.lock().unwrap() = Hooks::default();
    let report = e.check("savegame1").unwrap();
    assert_eq!(state_of(&report, "FS25_B"), SaveModState::Present);
}

#[test]
fn cancel_while_idle_does_nothing() {
    let e = env();
    let a = write_mod(&e.mods, "FS25_A.zip", "1.0", b"a");
    e.save(
        "savegame1",
        &[entry("FS25_A", "1.0", false, &game_hash(&a, "FS25_A"))],
    );
    e.lib.cancel_hashing();
    let report = e.check("savegame1").unwrap();
    assert_eq!(state_of(&report, "FS25_A"), SaveModState::Present);
}

#[test]
fn a_second_run_is_busy_and_the_flag_is_released_afterwards() {
    let e = env();
    let a = write_mod(&e.mods, "FS25_A.zip", "1.0", b"a");
    e.save(
        "savegame1",
        &[entry("FS25_A", "1.0", false, &game_hash(&a, "FS25_A"))],
    );
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let entered_tx = Mutex::new(entered_tx);
    let release_rx = Mutex::new(release_rx);
    let first = AtomicBool::new(true);

    std::thread::scope(|scope| {
        let running = scope.spawn(|| {
            e.lib
                .check_savegame_files("savegame1", &e.profile, &e.ctx(), &|_| {
                    // Hold the first run on its opening event.
                    if first.swap(false, Ordering::SeqCst) {
                        entered_tx.lock().unwrap().send(()).unwrap();
                        release_rx.lock().unwrap().recv().unwrap();
                    }
                })
        });
        entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();

        let err = e.check("savegame1").unwrap_err();
        assert_eq!(err.code, ErrorCode::Busy);
        // Reports and file reads are not blocked by a hashing run.
        assert_eq!(
            state_of(&e.report("savegame1"), "FS25_A"),
            SaveModState::HashPending
        );
        let mods = e.lib.list_mods(&e.profile, &e.ctx(), &|_| {}).unwrap();
        assert_eq!(mods.len(), 1);

        release_tx.send(()).unwrap();
        let report = running.join().unwrap().unwrap();
        assert_eq!(state_of(&report, "FS25_A"), SaveModState::Present);
    });
    assert!(e.check("savegame1").is_ok());
}

#[test]
fn a_failed_call_releases_the_busy_flag() {
    let e = env();
    assert_eq!(
        e.check("savegame7").unwrap_err().code,
        ErrorCode::SavegameMissing
    );
    assert_eq!(
        e.check("savegame7").unwrap_err().code,
        ErrorCode::SavegameMissing,
        "not Busy"
    );
}

#[test]
fn an_unreadable_zip_stays_pending_and_does_not_fail_the_check() {
    let e = env();
    let a = write_mod(&e.mods, "FS25_A.zip", "1.0", b"a");
    let b = write_mod(&e.mods, "FS25_B.zip", "1.0", b"b");
    e.save(
        "savegame1",
        &[
            entry("FS25_A", "1.0", false, &game_hash(&a, "FS25_A")),
            entry("FS25_B", "1.0", false, &game_hash(&b, "FS25_B")),
        ],
    );
    // B disappears after the report was built and before it is hashed.
    e.lib.hashing.hooks.lock().unwrap().before = Some(Arc::new(|path: &Path| {
        if path.ends_with("FS25_B.zip") {
            fs::remove_file(path).unwrap();
        }
    }));
    let events = Mutex::new(Vec::new());
    let report = e
        .lib
        .check_savegame_files("savegame1", &e.profile, &e.ctx(), &|p| {
            events.lock().unwrap().push(p);
        })
        .unwrap();
    assert_eq!(state_of(&report, "FS25_A"), SaveModState::Present);
    assert_eq!(state_of(&report, "FS25_B"), SaveModState::HashPending);
    let last = events.into_inner().unwrap().pop().unwrap();
    assert_eq!(last.done_files, last.total_files);
    assert_eq!(last.done_bytes, last.total_bytes);
}

// ---------------------------------------------------------------- errors and listing

#[test]
fn errors_are_savegame_missing_unreadable_and_profile_not_found() {
    let e = env();
    e.save("savegame1", &[]);
    write_save_file(&e.user, "savegame2", "<broken");
    let code = |r: Result<SavegameReport, ApiError>| r.unwrap_err().code;

    let no_user = GameContext {
        user_dir: None,
        ..e.ctx()
    };
    assert_eq!(
        code(e.lib.savegame_report("savegame1", &e.profile, &no_user)),
        ErrorCode::SavegameMissing
    );
    assert_eq!(
        code(
            e.lib
                .check_savegame_files("savegame1", &e.profile, &no_user, &|_| {})
        ),
        ErrorCode::SavegameMissing
    );
    for id in ["savegame9", "..\\user", "savegameX", ""] {
        assert_eq!(
            code(e.lib.savegame_report(id, &e.profile, &e.ctx())),
            ErrorCode::SavegameMissing,
            "{id}"
        );
    }
    assert_eq!(
        code(e.lib.savegame_report("savegame2", &e.profile, &e.ctx())),
        ErrorCode::SavegameUnreadable
    );
    assert_eq!(
        code(e.lib.savegame_report("savegame1", "p99", &e.ctx())),
        ErrorCode::ProfileNotFound
    );
    assert_eq!(
        code(
            e.lib
                .check_savegame_files("savegame1", "p99", &e.ctx(), &|_| {})
        ),
        ErrorCode::ProfileNotFound
    );
}

#[test]
fn an_unreadable_save_detail_is_a_kind_not_content() {
    let e = env();
    write_save_file(&e.user, "savegame1", "<careerSavegame secret-password");
    let err = e
        .lib
        .savegame_report("savegame1", &e.profile, &e.ctx())
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::SavegameUnreadable);
    assert!(!format!("{err:?}").contains("secret"));
}

#[test]
fn list_savegames_lists_valid_slots_and_is_empty_without_a_user_folder() {
    let e = env();
    e.save("savegame2", &[entry("FS25_A", "1", false, "")]);
    e.save("savegame1", &[]);
    write_save_file(&e.user, "savegame3", "<broken");
    let saves = e.lib.list_savegames(&e.ctx()).unwrap();
    assert_eq!(
        saves
            .iter()
            .map(|s| (s.id.as_str(), s.mod_count))
            .collect::<Vec<_>>(),
        [("savegame1", 0), ("savegame2", 1)]
    );
    let none = GameContext {
        user_dir: None,
        ..e.ctx()
    };
    assert!(e.lib.list_savegames(&none).unwrap().is_empty());
}

#[test]
fn the_report_scans_the_profile_fresh() {
    // A mod added since the last listing is seen.
    let e = env();
    e.save("savegame1", &[entry("FS25_A", "1.0", false, "")]);
    assert_eq!(
        state_of(&e.report("savegame1"), "FS25_A"),
        SaveModState::Missing
    );
    write_zip(
        &e.mods.join("FS25_A.zip"),
        &[(
            "modDesc.xml",
            crate::library::fixtures::desc("1.0").as_bytes(),
        )],
        Method::Stored,
    );
    assert_eq!(
        state_of(&e.report("savegame1"), "FS25_A"),
        SaveModState::Present
    );
}
