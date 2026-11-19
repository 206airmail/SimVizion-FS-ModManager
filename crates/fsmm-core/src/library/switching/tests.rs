//! Switching, undo and restore through `Library`: temp folders and in-memory databases only.
//! The settings file is a generated fixture with a fake password; no real game folder is used.

use std::cell::Cell;
use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use super::*;
use crate::ops::tests::{assert_no_secret, fixture, settings_text, with_bom, SECRET};
use crate::store::Store;

fn idle() -> Running {
    Running::default()
}

struct Env {
    _tmp: TempDir,
    user: PathBuf,
    settings: PathBuf,
    backups: PathBuf,
    lib: Library,
    ctx: GameContext,
    /// Profile ids.
    default: String,
    testing: String,
    other: String,
    /// The folders as they appear in the settings file.
    testing_dir: String,
    other_dir: String,
}

impl Env {
    fn new() -> Self {
        Self::with_settings(|testing_dir| fixture("true", testing_dir))
    }

    fn with_settings(make: impl FnOnce(&str) -> Vec<u8>) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let user = tmp.path().join("user");
        let default_dir = user.join("mods");
        let testing_dir = user.join("$testingMods");
        let other_dir = user.join("other");
        for dir in [&default_dir, &testing_dir, &other_dir] {
            fs::create_dir_all(dir).unwrap();
        }
        let testing_text = testing_dir.display().to_string();
        let settings = user.join("gameSettings.xml");
        fs::write(&settings, make(&testing_text)).unwrap();

        let ctx = GameContext {
            user_dir: Some(user.clone()),
            effective_mods_dir: Some(testing_dir.clone()),
            default_mods_dir: Some(default_dir.clone()),
            languages: vec!["en".to_string()],
            ..GameContext::default()
        };
        let lib = Library::new(Store::open_in_memory().unwrap(), "http://thumb.localhost");
        lib.ensure_profiles(&ctx).unwrap();
        lib.add_profile(&other_dir, &ctx).unwrap();
        let id_of = |dir: &PathBuf| {
            lib.store
                .profiles()
                .unwrap()
                .into_iter()
                .find(|p| path_key(&p.path) == path_key(dir))
                .unwrap()
                .id
        };
        Self {
            default: id_of(&default_dir),
            testing: id_of(&testing_dir),
            other: id_of(&other_dir),
            backups: tmp.path().join("backups"),
            testing_dir: testing_text,
            other_dir: other_dir.display().to_string(),
            _tmp: tmp,
            user,
            settings,
            lib,
            ctx,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        fs::read(&self.settings).unwrap()
    }

    fn set(&self, profile: &str) -> Result<SwitchResult, ApiError> {
        self.lib
            .set_active(profile, &self.ctx, &idle, &self.backups)
    }

    fn undo(&self, op_id: &str) -> Result<SwitchResult, ApiError> {
        self.lib.undo_switch(op_id, &self.ctx, &idle, &self.backups)
    }

    fn backup_names(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.backups) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    /// Files (not folders) in the user folder: only the settings file may be there.
    fn assert_no_temp_files(&self) {
        let files: Vec<String> = fs::read_dir(&self.user)
            .unwrap()
            .map(Result::unwrap)
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        assert_eq!(files, vec!["gameSettings.xml".to_string()]);
    }

    fn journal(&self) -> Vec<crate::store::StoredOp> {
        self.lib.store.ops().unwrap()
    }

    fn override_now(&self) -> ModsOverride {
        ops::read_override(&self.bytes()).unwrap()
    }
}

fn assert_clean(e: &ApiError) {
    assert_no_secret(&e.to_string());
    assert_no_secret(&format!("{e:?}"));
}

fn err_code<T: std::fmt::Debug>(result: Result<T, ApiError>) -> ErrorCode {
    let e = result.unwrap_err();
    assert_clean(&e);
    e.code
}

// ---------------------------------------------------------------- set_active

#[test]
fn switching_to_the_default_sets_active_false_and_keeps_the_directory() {
    let env = Env::new();
    let original = env.bytes();
    let result = env.set(&env.default).unwrap();

    assert_eq!(result.op_id, "o1");
    assert_eq!(
        result.from_profile_id.as_deref(),
        Some(env.testing.as_str())
    );
    assert_eq!(result.to_profile_id.as_deref(), Some(env.default.as_str()));
    assert_eq!(env.bytes(), fixture("false", &env.testing_dir));
    assert_eq!(
        env.override_now(),
        ModsOverride {
            active: false,
            directory: Some(PathBuf::from(&env.testing_dir))
        }
    );

    // The backup is the file as it was, and the journal knows it.
    let backups = env.backup_names();
    assert_eq!(backups.len(), 1);
    assert_eq!(fs::read(env.backups.join(&backups[0])).unwrap(), original);
    let journal = env.journal();
    assert_eq!(journal.len(), 1);
    assert_eq!(journal[0].id, "o1");
    assert_eq!(journal[0].kind, "switch");
    assert_eq!(journal[0].result, "ok");
    assert_eq!(journal[0].backup_file.as_deref(), Some(backups[0].as_str()));
    assert!(journal[0].from.active && !journal[0].to.active);
    env.assert_no_temp_files();
}

#[test]
fn switching_to_a_folder_sets_active_true_and_its_path() {
    let env = Env::new();
    let result = env.set(&env.other).unwrap();
    assert_eq!(
        result.from_profile_id.as_deref(),
        Some(env.testing.as_str())
    );
    assert_eq!(result.to_profile_id.as_deref(), Some(env.other.as_str()));
    assert_eq!(env.bytes(), fixture("true", &env.other_dir));

    // From the default folder back to a folder: both values change.
    env.set(&env.default).unwrap();
    let result = env.set(&env.testing).unwrap();
    assert_eq!(
        result.from_profile_id.as_deref(),
        Some(env.default.as_str())
    );
    assert_eq!(result.to_profile_id.as_deref(), Some(env.testing.as_str()));
    assert_eq!(env.bytes(), fixture("true", &env.testing_dir));
}

#[test]
fn a_round_trip_through_set_active_is_byte_identical() {
    let env = Env::new();
    let original = env.bytes();
    env.set(&env.default).unwrap();
    assert_ne!(env.bytes(), original);
    env.set(&env.testing).unwrap();
    assert_eq!(env.bytes(), original);

    env.set(&env.other).unwrap();
    env.set(&env.default).unwrap();
    env.set(&env.testing).unwrap();
    assert_eq!(env.bytes(), original);
    assert_eq!(env.journal().len(), 5);
    env.assert_no_temp_files();
}

#[test]
fn setting_the_active_profile_again_writes_nothing() {
    let env = Env::new();
    let original = env.bytes();
    let result = env.set(&env.testing).unwrap();
    assert_eq!(result.op_id, "");
    assert_eq!(result.from_profile_id, result.to_profile_id);
    assert_eq!(result.to_profile_id.as_deref(), Some(env.testing.as_str()));
    assert_eq!(env.bytes(), original);
    assert!(env.backup_names().is_empty());
    assert!(env.journal().is_empty());

    // After a real write, the no-op reports the latest journaled id.
    let switched = env.set(&env.other).unwrap();
    let again = env.set(&env.other).unwrap();
    assert_eq!(again.op_id, switched.op_id);
    assert_eq!(again.from_profile_id, again.to_profile_id);
    assert_eq!(env.journal().len(), 1);
    assert_eq!(env.backup_names().len(), 1);
}

#[test]
fn an_inactive_override_names_the_default_profile_as_from() {
    let env = Env::with_settings(|dir| fixture("false", dir));
    let result = env.set(&env.other).unwrap();
    assert_eq!(
        result.from_profile_id.as_deref(),
        Some(env.default.as_str())
    );
    assert_eq!(result.to_profile_id.as_deref(), Some(env.other.as_str()));

    // Setting the default while inactive is a no-op, even though the directory names another.
    let env = Env::with_settings(|dir| fixture("false", dir));
    let original = env.bytes();
    let result = env.set(&env.default).unwrap();
    assert_eq!(result.from_profile_id, result.to_profile_id);
    assert_eq!(env.bytes(), original);
}

#[test]
fn a_missing_element_is_inserted() {
    let env = Env::with_settings(|_| with_bom(&settings_text("    <extra>1</extra>", "\n")));
    let result = env.set(&env.other).unwrap();
    assert_eq!(
        result.from_profile_id.as_deref(),
        Some(env.default.as_str())
    );
    let text = String::from_utf8(env.bytes()).unwrap();
    assert!(text.contains(&format!(
        "<gameSettings>\n    <modsDirectoryOverride active=\"true\" directory=\"{}\"/>\n    <extra>",
        env.other_dir
    )));
}

#[test]
fn unknown_profiles_and_missing_settings_are_refused() {
    let env = Env::new();
    assert_eq!(err_code(env.set("p999")), ErrorCode::ProfileNotFound);

    let mut no_user = env.ctx.clone();
    no_user.user_dir = None;
    let result = env
        .lib
        .set_active(&env.other, &no_user, &idle, &env.backups);
    assert_eq!(err_code(result), ErrorCode::SettingsMissing);

    fs::remove_file(&env.settings).unwrap();
    assert_eq!(err_code(env.set(&env.other)), ErrorCode::SettingsMissing);
    assert!(env.backup_names().is_empty());
    assert!(env.journal().is_empty());
}

#[test]
fn an_unreadable_file_is_refused_without_a_backup() {
    let broken = with_bom(&format!("<gameSettings><x>{SECRET}</x>"));
    let env = Env::with_settings(|_| broken.clone());
    assert_eq!(err_code(env.set(&env.other)), ErrorCode::SettingsUnreadable);
    assert_eq!(env.bytes(), broken);
    assert!(env.backup_names().is_empty());

    let mut not_utf8 = fixture("true", "x");
    not_utf8.push(0xFF);
    let env = Env::with_settings(|_| not_utf8.clone());
    assert_eq!(err_code(env.set(&env.other)), ErrorCode::SettingsUnreadable);
    assert_eq!(env.bytes(), not_utf8);
}

// ---------------------------------------------------------------- guards

#[test]
fn a_running_game_or_server_leaves_no_backup_no_temp_file_and_no_change() {
    for (running, code) in [
        (
            Running {
                game: true,
                server: false,
            },
            ErrorCode::GameRunning,
        ),
        (
            Running {
                game: false,
                server: true,
            },
            ErrorCode::ServerRunning,
        ),
    ] {
        let env = Env::new();
        let original = env.bytes();
        let result = env
            .lib
            .set_active(&env.other, &env.ctx, &|| running, &env.backups);
        assert_eq!(err_code(result), code);
        assert_eq!(env.bytes(), original);
        assert!(!env.backups.exists() || env.backup_names().is_empty());
        assert!(env.journal().is_empty());
        env.assert_no_temp_files();

        // Undo and restore refuse too.
        let result = env
            .lib
            .undo_switch("o1", &env.ctx, &|| running, &env.backups);
        assert!(matches!(
            err_code(result),
            ErrorCode::GameRunning | ErrorCode::ServerRunning | ErrorCode::NothingToUndo
        ));
        let result = env.lib.restore_backup(&env.ctx, &|| running, &env.backups);
        assert_eq!(err_code(result), code);
    }
}

#[test]
fn a_concurrent_change_is_never_overwritten() {
    let env = Env::new();
    let concurrent = fixture("true", r"D:\written\by\the\game");
    let calls = Cell::new(0);
    // The first call is the preflight; the second is right before the rename.
    let running = || {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            fs::write(&env.settings, &concurrent).unwrap();
        }
        idle()
    };
    let result = env
        .lib
        .set_active(&env.other, &env.ctx, &running, &env.backups);
    assert_eq!(err_code(result), ErrorCode::SettingsChanged);
    assert_eq!(env.bytes(), concurrent);
    env.assert_no_temp_files();

    // A backup was taken, so the failed attempt is journaled; it is not undoable.
    assert_eq!(env.backup_names().len(), 1);
    let journal = env.journal();
    assert_eq!(journal.len(), 1);
    assert_eq!(journal[0].result, "settingsChanged");
    assert_eq!(err_code(env.undo("o1")), ErrorCode::NothingToUndo);
}

#[test]
fn the_game_starting_mid_write_leaves_the_original() {
    let env = Env::new();
    let original = env.bytes();
    let calls = Cell::new(0);
    let running = || {
        calls.set(calls.get() + 1);
        Running {
            game: calls.get() >= 2,
            server: false,
        }
    };
    let result = env
        .lib
        .set_active(&env.other, &env.ctx, &running, &env.backups);
    assert_eq!(err_code(result), ErrorCode::GameRunning);
    assert_eq!(env.bytes(), original);
    env.assert_no_temp_files();
    assert_eq!(env.journal()[0].result, "gameRunning");
}

#[test]
fn a_read_only_file_fails_with_write_failed_and_stays_intact() {
    let env = Env::new();
    let original = env.bytes();
    let mut permissions = fs::metadata(&env.settings).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&env.settings, permissions).unwrap();

    let result = env.set(&env.other);

    #[allow(clippy::permissions_set_readonly_false)]
    {
        let mut permissions = fs::metadata(&env.settings).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&env.settings, permissions).unwrap();
    }
    assert_eq!(err_code(result), ErrorCode::WriteFailed);
    assert_eq!(env.bytes(), original);
    env.assert_no_temp_files();
    assert_eq!(env.journal()[0].result, "writeFailed");
}

// ---------------------------------------------------------------- undo

#[test]
fn undo_puts_back_the_exact_previous_override() {
    let env = Env::new();
    let original = env.bytes();
    let switched = env.set(&env.default).unwrap();
    let undone = env.undo(&switched.op_id).unwrap();

    assert_eq!(env.bytes(), original);
    assert_eq!(
        undone.from_profile_id.as_deref(),
        Some(env.default.as_str())
    );
    assert_eq!(undone.to_profile_id.as_deref(), Some(env.testing.as_str()));
    assert_eq!(undone.op_id, "o2");

    let journal = env.journal();
    assert_eq!(journal[1].kind, "undo");
    assert_eq!(journal[1].result, "ok");
    assert_eq!(env.backup_names().len(), 2);
    env.assert_no_temp_files();

    // An undo can itself be undone.
    let redone = env.undo(&undone.op_id).unwrap();
    assert_eq!(redone.to_profile_id.as_deref(), Some(env.default.as_str()));
    assert_eq!(env.bytes(), fixture("false", &env.testing_dir));
}

#[test]
fn undo_of_a_folder_to_folder_switch_round_trips() {
    let env = Env::new();
    let original = env.bytes();
    let switched = env.set(&env.other).unwrap();
    env.undo(&switched.op_id).unwrap();
    assert_eq!(env.bytes(), original);
}

#[test]
fn undo_is_refused_after_an_external_change() {
    let env = Env::new();
    let switched = env.set(&env.other).unwrap();
    let external = fixture("true", r"D:\the\game\changed\it");
    fs::write(&env.settings, &external).unwrap();

    assert_eq!(
        err_code(env.undo(&switched.op_id)),
        ErrorCode::SettingsChanged
    );
    assert_eq!(env.bytes(), external);
    assert_eq!(env.journal().len(), 1);
    assert_eq!(env.backup_names().len(), 1);
}

#[test]
fn undo_is_refused_for_unknown_and_non_latest_operations() {
    let env = Env::new();
    assert_eq!(err_code(env.undo("o1")), ErrorCode::NothingToUndo);

    let first = env.set(&env.other).unwrap();
    let second = env.set(&env.default).unwrap();
    assert_eq!(err_code(env.undo(&first.op_id)), ErrorCode::NothingToUndo);
    assert_eq!(err_code(env.undo("o99")), ErrorCode::NothingToUndo);
    assert_eq!(err_code(env.undo("")), ErrorCode::NothingToUndo);
    assert_eq!(err_code(env.undo("p1")), ErrorCode::NothingToUndo);
    env.undo(&second.op_id).unwrap();
}

#[test]
fn undo_is_refused_once_a_restore_is_the_latest_write() {
    let env = Env::new();
    let switched = env.set(&env.other).unwrap();
    env.lib
        .restore_backup(&env.ctx, &idle, &env.backups)
        .unwrap();
    assert_eq!(
        err_code(env.undo(&switched.op_id)),
        ErrorCode::NothingToUndo
    );
}

// ---------------------------------------------------------------- restore

#[test]
fn restore_backs_up_the_broken_file_then_restores_the_newest_backup() {
    let env = Env::new();
    let original = env.bytes();
    env.set(&env.other).unwrap();
    let broken = with_bom(&format!("<gameSettings><password>{SECRET}</password>"));
    fs::write(&env.settings, &broken).unwrap();
    let before = env.backup_names();
    assert_eq!(before.len(), 1);

    env.lib
        .restore_backup(&env.ctx, &idle, &env.backups)
        .unwrap();

    // The restored file is the backup made by the switch: the original.
    assert_eq!(env.bytes(), original);
    assert_eq!(
        env.override_now().directory,
        Some(PathBuf::from(&env.testing_dir))
    );
    // The broken file was backed up first.
    let after = env.backup_names();
    assert_eq!(after.len(), 2);
    assert_eq!(fs::read(env.backups.join(&after[1])).unwrap(), broken);
    let journal = env.journal();
    assert_eq!(journal.last().unwrap().kind, "restore");
    assert_eq!(journal.last().unwrap().result, "ok");
    assert_eq!(
        journal.last().unwrap().backup_file.as_deref(),
        Some(after[1].as_str())
    );
    env.assert_no_temp_files();
}

#[test]
fn restore_reports_no_backup_and_missing_settings() {
    let env = Env::new();
    assert_eq!(
        err_code(env.lib.restore_backup(&env.ctx, &idle, &env.backups)),
        ErrorCode::NoBackup
    );
    assert!(env.journal().is_empty());

    env.set(&env.other).unwrap();
    fs::remove_file(&env.settings).unwrap();
    assert_eq!(
        err_code(env.lib.restore_backup(&env.ctx, &idle, &env.backups)),
        ErrorCode::SettingsMissing
    );
    assert_eq!(env.backup_names().len(), 1);

    let mut no_user = env.ctx.clone();
    no_user.user_dir = None;
    assert_eq!(
        err_code(env.lib.restore_backup(&no_user, &idle, &env.backups)),
        ErrorCode::SettingsMissing
    );
}

#[test]
fn restore_refuses_a_concurrent_change() {
    let env = Env::new();
    env.set(&env.other).unwrap();
    let broken = b"not xml".to_vec();
    fs::write(&env.settings, &broken).unwrap();
    let concurrent = fixture("true", r"D:\other");
    let calls = Cell::new(0);
    let running = || {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            fs::write(&env.settings, &concurrent).unwrap();
        }
        idle()
    };
    let result = env.lib.restore_backup(&env.ctx, &running, &env.backups);
    assert_eq!(err_code(result), ErrorCode::SettingsChanged);
    assert_eq!(env.bytes(), concurrent);
    assert_eq!(env.journal().last().unwrap().result, "settingsChanged");
    env.assert_no_temp_files();
}

// ---------------------------------------------------------------- backups and journal

#[test]
fn latest_backup_reports_the_newest_time() {
    let env = Env::new();
    assert!(env.lib.latest_backup(&env.backups).is_none());
    env.set(&env.other).unwrap();
    let time = env.lib.latest_backup(&env.backups).unwrap();
    let age = SystemTime::now()
        .duration_since(time)
        .unwrap_or_default()
        .as_secs();
    assert!(age < 120, "backup time is {age}s old");
}

#[test]
fn backups_are_kept_to_the_newest_twenty_across_many_switches() {
    let env = Env::new();
    let targets = [&env.default, &env.other, &env.testing];
    for i in 0..25 {
        env.set(targets[i % 3]).unwrap();
    }
    assert_eq!(env.backup_names().len(), KEEP_BACKUPS);
    assert_eq!(env.journal().len(), 25);
    // The newest backup is the state before the last switch.
    let last = env.lib.latest_backup(&env.backups);
    assert!(last.is_some());
}

#[test]
fn journal_rows_carry_ids_states_and_backup_names() {
    let env = Env::new();
    let a = env.set(&env.other).unwrap();
    let b = env.set(&env.default).unwrap();
    assert_eq!((a.op_id.as_str(), b.op_id.as_str()), ("o1", "o2"));

    let journal = env.journal();
    assert_eq!(
        journal[0].from.directory,
        Some(PathBuf::from(&env.testing_dir))
    );
    assert_eq!(journal[0].to.directory, Some(PathBuf::from(&env.other_dir)));
    assert!(journal[0].to.active);
    // The default keeps the directory it had.
    assert_eq!(
        journal[1].from.directory,
        Some(PathBuf::from(&env.other_dir))
    );
    assert_eq!(journal[1].to.directory, Some(PathBuf::from(&env.other_dir)));
    assert!(!journal[1].to.active);
    for op in &journal {
        assert!(env.backups.join(op.backup_file.as_ref().unwrap()).is_file());
        assert!(op.at > 0);
    }
}

// ---------------------------------------------------------------- secrets

#[test]
fn no_error_text_contains_file_contents() {
    // Every refusal above is checked with `assert_clean`; this adds the file-shaped ones.
    let env = Env::new();
    let samples: Vec<ApiError> = vec![
        env.set("p999").unwrap_err(),
        env.undo("o1").unwrap_err(),
        env.lib
            .restore_backup(&env.ctx, &idle, &env.backups)
            .unwrap_err(),
    ];
    let env2 = Env::with_settings(|_| with_bom(&format!("<a>{SECRET}</b>")));
    let more = [
        env2.set(&env2.other).unwrap_err(),
        env2.lib
            .set_active(
                &env2.other,
                &env2.ctx,
                &|| Running {
                    game: true,
                    server: false,
                },
                &env2.backups,
            )
            .unwrap_err(),
    ];
    for e in samples.iter().chain(more.iter()) {
        assert_clean(e);
    }
}
