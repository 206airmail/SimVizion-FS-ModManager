//! Copy, move, remove and profile management through `Library`: temp folders, an in-memory
//! database, a fake Recycle Bin and a fake disk. No real game folder or real bin is touched.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use super::*;
use crate::files::fakes::{FakeDisk, FakeRecycler};
use crate::model::ModFormat;
use crate::store::{Store, StoredFileOp};
use crate::testutil::{write_folder, write_zip, Method};

const MIB: usize = 1024 * 1024;

fn desc(title: &str, version: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
         <modDesc descVersion=\"105\"><title><en>{title}</en></title>\
         <author>Someone</author><version>{version}</version></modDesc>"
    )
}

fn write_mod(folder: &Path, file: &str, title: &str, version: &str) -> PathBuf {
    let path = folder.join(file);
    write_zip(
        &path,
        &[("modDesc.xml", desc(title, version).as_bytes())],
        Method::Stored,
    );
    path
}

fn write_big(folder: &Path, file: &str, payload: usize) -> PathBuf {
    let path = folder.join(file);
    let data: Vec<u8> = (0..payload).map(|i| (i % 251) as u8).collect();
    write_zip(
        &path,
        &[
            ("modDesc.xml", desc("Big", "1.0.0.0").as_bytes()),
            ("data.bin", &data),
        ],
        Method::Stored,
    );
    path
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

fn choice(name: &str, replace: bool) -> ClashChoice {
    ClashChoice {
        mod_name: name.to_string(),
        replace,
    }
}

fn failed(result: &OpResult) -> Vec<(String, ErrorCode)> {
    result
        .failed
        .iter()
        .map(|f| (f.mod_name.clone(), f.code))
        .collect()
}

/// Two profile folders `a` and `b` (a third, `c`, is registered on request), a library, the fakes
/// and the knobs for the running state. Everything a test needs is an atomic, so progress
/// callbacks, which must be `Sync`, can use the whole fixture.
struct T {
    _tmp: TempDir,
    a: PathBuf,
    b: PathBuf,
    pa: String,
    pb: String,
    lib: Library,
    ctx: GameContext,
    recycler: FakeRecycler,
    disk: FakeDisk,
    game: AtomicBool,
    server: AtomicBool,
    /// Calls of the running check since `exec` began.
    running_calls: AtomicUsize,
    /// The game "starts" from this call of the running check on (the first is call 0).
    game_from_call: AtomicUsize,
    /// A cancel is requested right after this call of the running check.
    cancel_after_call: AtomicUsize,
}

impl T {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let lib = Library::new(Store::open_in_memory().unwrap(), "http://thumb.localhost");
        let ctx = GameContext {
            languages: vec!["en".to_string()],
            ..GameContext::default()
        };
        let pa = lib.add_profile(&a, &ctx).unwrap().id;
        let pb = lib.add_profile(&b, &ctx).unwrap().id;
        let recycler = FakeRecycler::new(&tmp.path().join("bin"));
        Self {
            _tmp: tmp,
            a,
            b,
            pa,
            pb,
            lib,
            ctx,
            recycler,
            disk: FakeDisk::new(u64::MAX),
            game: AtomicBool::new(false),
            server: AtomicBool::new(false),
            running_calls: AtomicUsize::new(0),
            game_from_call: AtomicUsize::new(usize::MAX),
            cancel_after_call: AtomicUsize::new(usize::MAX),
        }
    }

    fn running(&self) -> Running {
        let call = self.running_calls.fetch_add(1, Ordering::SeqCst);
        if call == self.cancel_after_call.load(Ordering::SeqCst) {
            self.lib.cancel_operation();
        }
        Running {
            game: self.game.load(Ordering::SeqCst)
                || call >= self.game_from_call.load(Ordering::SeqCst),
            server: self.server.load(Ordering::SeqCst),
        }
    }

    fn with_env<R>(&self, f: impl FnOnce(&OpEnv<'_>) -> R) -> R {
        let running = || self.running();
        f(&OpEnv {
            running: &running,
            recycler: &self.recycler,
            disk: &self.disk,
        })
    }

    fn plan(&self, mods: &[&str], mode: TransferMode) -> TransferPlan {
        self.try_plan(&self.pa, mods, &self.pb, mode).unwrap()
    }

    fn try_plan(
        &self,
        from: &str,
        mods: &[&str],
        to: &str,
        mode: TransferMode,
    ) -> Result<TransferPlan, ApiError> {
        self.with_env(|env| {
            self.lib
                .plan_transfer(from, &ids(mods), to, mode, &self.ctx, env)
        })
    }

    fn exec(&self, plan_id: &str, choices: &[ClashChoice]) -> Result<OpResult, ApiError> {
        self.exec_with(plan_id, choices, &|_| {})
    }

    fn exec_with(
        &self,
        plan_id: &str,
        choices: &[ClashChoice],
        progress: &(dyn Fn(OpProgress) + Sync),
    ) -> Result<OpResult, ApiError> {
        self.running_calls.store(0, Ordering::SeqCst);
        self.with_env(|env| {
            self.lib
                .execute_transfer(plan_id, choices, &self.ctx, env, progress)
        })
    }

    fn remove(&self, mods: &[&str]) -> Result<OpResult, ApiError> {
        self.remove_with(mods, &|_| {})
    }

    fn remove_with(
        &self,
        mods: &[&str],
        progress: &(dyn Fn(OpProgress) + Sync),
    ) -> Result<OpResult, ApiError> {
        self.running_calls.store(0, Ordering::SeqCst);
        self.with_env(|env| {
            self.lib
                .remove_mods(&self.pa, &ids(mods), &self.ctx, env, progress)
        })
    }

    fn list(&self, profile: &str) -> Vec<ModEntry> {
        self.lib.list_mods(profile, &self.ctx, &|_| {}).unwrap()
    }

    fn journal(&self) -> Vec<StoredFileOp> {
        self.lib.store.file_ops().unwrap()
    }

    fn rows(&self) -> Vec<(String, String)> {
        self.journal()
            .into_iter()
            .map(|r| (r.kind, r.result))
            .collect()
    }
}

fn err_code<R: std::fmt::Debug>(result: Result<R, ApiError>) -> ErrorCode {
    result.unwrap_err().code
}

// ---------------------------------------------------------------- plan

#[test]
fn a_plan_lists_items_and_clashes_case_insensitively_with_both_summaries() {
    let t = T::new();
    write_mod(&t.a, "FS25_Same.zip", "Same A", "2.0.0.0");
    write_mod(&t.a, "FS25_Other.zip", "Other", "1.0.0.0");
    write_mod(&t.b, "fs25_same.ZIP", "Same B", "1.5.0.0");
    write_mod(&t.b, "FS25_Unrelated.zip", "Unrelated", "1.0.0.0");
    let a_list = t.list(&t.pa);
    let b_list = t.list(&t.pb);
    let incoming = a_list.iter().find(|m| m.mod_name == "FS25_Same").unwrap();
    let existing = b_list.iter().find(|m| m.mod_name == "fs25_same").unwrap();

    let plan = t.plan(&["FS25_Same.zip", "FS25_Other.zip"], TransferMode::Copy);

    assert!(plan.plan_id.starts_with('t'));
    assert_eq!((&plan.from_profile_id, &plan.to_profile_id), (&t.pa, &t.pb));
    assert_eq!(plan.mode, TransferMode::Copy);
    let item_names: Vec<&str> = plan.items.iter().map(|i| i.mod_name.as_str()).collect();
    assert_eq!(item_names, ["FS25_Same", "FS25_Other"]);
    assert_eq!(plan.items[0].mod_id, "FS25_Same.zip");
    assert_eq!(plan.items[0].size_bytes, incoming.size_bytes);
    assert_eq!(plan.clashes.len(), 1);
    let clash = &plan.clashes[0];
    assert_eq!(clash.mod_name, "FS25_Same");
    assert_eq!(clash.incoming.file_name, "FS25_Same.zip");
    assert_eq!(clash.incoming.version, "2.0.0.0");
    assert_eq!(clash.incoming.size_bytes, incoming.size_bytes);
    assert_eq!(clash.incoming.modified, incoming.modified);
    assert_eq!(clash.existing.file_name, "fs25_same.ZIP");
    assert_eq!(clash.existing.version, "1.5.0.0");
    assert_eq!(clash.existing.size_bytes, existing.size_bytes);
    assert_eq!(clash.existing.modified, existing.modified);
    // Planning writes nothing.
    assert_eq!(names(&t.b), ["FS25_Unrelated.zip", "fs25_same.ZIP"]);
}

#[test]
fn a_plan_counts_bytes_and_reports_free_space_from_the_disk() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    write_mod(&t.a, "B.zip", "B", "1.0.0.0");
    t.disk.set(12345);
    let plan = t.plan(&["A.zip", "B.zip"], TransferMode::Copy);
    let sizes: u64 = plan.items.iter().map(|i| i.size_bytes).sum();
    assert!(sizes > 0);
    assert_eq!(plan.total_bytes, sizes);
    assert_eq!(plan.free_bytes, 12345);
}

#[test]
fn a_move_on_one_volume_writes_no_bytes() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    let moved = t.plan(&["A.zip"], TransferMode::Move);
    assert_eq!(moved.total_bytes, 0);
    assert!(moved.items[0].size_bytes > 0);
    let copied = t.plan(&["A.zip"], TransferMode::Copy);
    assert!(copied.total_bytes > 0);
}

#[cfg(windows)]
#[test]
fn volumes_are_drives_or_shares_compared_without_case() {
    assert_eq!(
        volume_of(Path::new(r"D:\Mods\a")),
        volume_of(Path::new(r"d:\other"))
    );
    assert_ne!(
        volume_of(Path::new(r"D:\Mods")),
        volume_of(Path::new(r"E:\Mods"))
    );
    assert_eq!(
        volume_of(Path::new(r"\\NAS\Share\a")),
        volume_of(Path::new(r"\\nas\share\b"))
    );
    assert_ne!(
        volume_of(Path::new(r"\\nas\share\a")),
        volume_of(Path::new(r"\\nas\other\a"))
    );
    assert_eq!(
        volume_of(Path::new(r"\\?\D:\x")),
        volume_of(Path::new(r"D:\y"))
    );
    assert_ne!(
        volume_of(Path::new(r"D:\x")),
        volume_of(Path::new(r"\\nas\share\x"))
    );
}

#[test]
fn planning_rejects_unknown_profiles_unknown_mods_and_the_same_profile() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    let copy = TransferMode::Copy;
    assert_eq!(
        err_code(t.try_plan("p999", &["A.zip"], &t.pb, copy)),
        ErrorCode::ProfileNotFound
    );
    assert_eq!(
        err_code(t.try_plan(&t.pa, &["A.zip"], "p999", copy)),
        ErrorCode::ProfileNotFound
    );
    let e = t
        .try_plan(&t.pa, &["A.zip", "Nope.zip"], &t.pb, copy)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::FolderMissing);
    assert_eq!(e.detail.as_deref(), Some("Nope.zip"));
    let same = t.try_plan(&t.pa, &["A.zip"], &t.pa, copy).unwrap_err();
    assert_eq!(same.code, ErrorCode::TargetChanged);
}

#[test]
fn a_missing_target_folder_cannot_be_planned() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    fs::remove_dir_all(&t.b).unwrap();
    assert_eq!(
        err_code(t.try_plan(&t.pa, &["A.zip"], &t.pb, TransferMode::Copy)),
        ErrorCode::FolderMissing
    );
}

// ---------------------------------------------------------------- execute: copy

#[test]
fn copying_without_clashes_puts_the_mods_in_the_target_and_refreshes_listings() {
    let t = T::new();
    let src = write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    write_mod(&t.a, "B.zip", "B", "1.0.0.0");
    write_mod(&t.b, "Z.zip", "Z", "1.0.0.0");
    // Listings made before the operation fill the last-scan caches.
    assert!(t.list(&t.pa).iter().all(|m| m.also_in.is_empty()));
    assert_eq!(t.list(&t.pb).len(), 1);
    let counts = |t: &T| -> Vec<u32> {
        t.lib
            .list_profiles(&t.ctx)
            .unwrap()
            .iter()
            .map(|p| p.mod_count)
            .collect()
    };
    assert_eq!(counts(&t), [2, 1]);

    let plan = t.plan(&["A.zip", "B.zip"], TransferMode::Copy);
    let result = t.exec(&plan.plan_id, &[]).unwrap();

    assert_eq!(result.done, ["A", "B"]);
    assert!(result.skipped.is_empty() && result.failed.is_empty());
    assert_eq!(names(&t.b), ["A.zip", "B.zip", "Z.zip"]);
    assert_eq!(names(&t.a), ["A.zip", "B.zip"], "a copy leaves the source");
    assert_eq!(
        fs::read(t.b.join("A.zip")).unwrap(),
        fs::read(&src).unwrap()
    );
    assert!(t.recycler.calls().is_empty());

    assert_eq!(counts(&t), [2, 3]);
    let a_after = t.list(&t.pa);
    assert!(a_after.iter().all(|m| m.also_in == [t.pb.clone()]));
    let b_after = t.list(&t.pb);
    let also: Vec<(&str, usize)> = b_after
        .iter()
        .map(|m| (m.mod_name.as_str(), m.also_in.len()))
        .collect();
    assert_eq!(also, [("A", 1), ("B", 1), ("Z", 0)]);
}

#[test]
fn a_folder_mod_is_copied_whole() {
    let t = T::new();
    let src = t.a.join("FS25_Folder");
    write_folder(
        &src,
        &[
            ("modDesc.xml", desc("F", "1.0.0.0").as_bytes()),
            ("scripts/a.lua", b"print(1)"),
        ],
    );
    let plan = t.plan(&["FS25_Folder"], TransferMode::Copy);
    let result = t.exec(&plan.plan_id, &[]).unwrap();
    assert_eq!(result.done, ["FS25_Folder"]);
    assert_eq!(
        fs::read(t.b.join("FS25_Folder/scripts/a.lua")).unwrap(),
        b"print(1)"
    );
    let listed = t.list(&t.pb);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].format, ModFormat::Folder);
    assert_eq!(listed[0].size_bytes, t.list(&t.pa)[0].size_bytes);
}

#[test]
fn a_move_on_one_volume_renames_without_recycling() {
    let t = T::new();
    let src = write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    let bytes = fs::read(&src).unwrap();
    assert_eq!(t.list(&t.pa).len(), 1);

    let plan = t.plan(&["A.zip"], TransferMode::Move);
    let result = t.exec(&plan.plan_id, &[]).unwrap();

    assert_eq!(result.done, ["A"]);
    assert!(names(&t.a).is_empty());
    assert_eq!(fs::read(t.b.join("A.zip")).unwrap(), bytes);
    assert!(t.recycler.calls().is_empty());
    assert!(t.list(&t.pa).is_empty());
    assert_eq!(t.list(&t.pb).len(), 1);
    assert_eq!(t.rows(), [("move".to_string(), "ok".to_string())]);
}

// ---------------------------------------------------------------- execute: clashes

fn clash_fixture() -> (T, TransferPlan) {
    let t = T::new();
    write_mod(&t.a, "FS25_Same.zip", "New", "2.0.0.0");
    write_mod(&t.a, "FS25_Fresh.zip", "Fresh", "1.0.0.0");
    write_mod(&t.b, "fs25_same.zip", "Old", "1.0.0.0");
    let plan = t.plan(&["FS25_Same.zip", "FS25_Fresh.zip"], TransferMode::Copy);
    assert_eq!(plan.clashes.len(), 1);
    (t, plan)
}

fn version_in(dir: &Path, file: &str) -> String {
    let lib = Library::new(Store::open_in_memory().unwrap(), "http://thumb.localhost");
    let ctx = GameContext::default();
    let profile = lib.add_profile(dir, &ctx).unwrap();
    let mods = lib.list_mods(&profile.id, &ctx, &|_| {}).unwrap();
    mods.iter()
        .find(|m| m.file_name.eq_ignore_ascii_case(file))
        .unwrap()
        .version
        .clone()
}

#[test]
fn replacing_a_clash_recycles_the_old_mod_and_puts_the_new_one_in_place() {
    let (t, plan) = clash_fixture();
    let result = t.exec(&plan.plan_id, &[choice("FS25_Same", true)]).unwrap();

    assert_eq!(result.done, ["FS25_Same", "FS25_Fresh"]);
    assert_eq!(t.recycler.calls(), [t.b.join("fs25_same.zip")]);
    assert_eq!(t.recycler.binned(), ["fs25_same.zip"]);
    assert_eq!(names(&t.b), ["FS25_Fresh.zip", "FS25_Same.zip"]);
    assert_eq!(version_in(&t.b, "FS25_Same.zip"), "2.0.0.0");
    assert_eq!(
        t.rows(),
        [
            ("recycle".to_string(), "ok".to_string()),
            ("copy".to_string(), "ok".to_string()),
            ("copy".to_string(), "ok".to_string()),
        ]
    );
}

#[test]
fn skipping_a_clash_leaves_both_mods_alone() {
    let (t, plan) = clash_fixture();
    let result = t
        .exec(&plan.plan_id, &[choice("FS25_Same", false)])
        .unwrap();
    assert_eq!(result.skipped, ["FS25_Same"]);
    assert_eq!(result.done, ["FS25_Fresh"]);
    assert!(t.recycler.calls().is_empty());
    assert_eq!(names(&t.b), ["FS25_Fresh.zip", "fs25_same.zip"]);
    assert_eq!(version_in(&t.b, "fs25_same.zip"), "1.0.0.0");
}

#[test]
fn a_clash_without_a_choice_is_skipped() {
    let (t, plan) = clash_fixture();
    let result = t.exec(&plan.plan_id, &[]).unwrap();
    assert_eq!(result.skipped, ["FS25_Same"]);
    assert_eq!(result.done, ["FS25_Fresh"]);
    // Only the copy that was attempted is journaled.
    assert_eq!(t.rows(), [("copy".to_string(), "ok".to_string())]);
}

#[test]
fn a_choice_matches_the_mod_name_without_regard_to_case() {
    let (t, plan) = clash_fixture();
    let result = t.exec(&plan.plan_id, &[choice("fs25_SAME", true)]).unwrap();
    assert_eq!(result.done, ["FS25_Same", "FS25_Fresh"]);
}

#[test]
fn a_replace_the_bin_refuses_leaves_both_mods_untouched() {
    let (t, plan) = clash_fixture();
    t.recycler.refuse(&t.b.join("fs25_same.zip"));
    let result = t.exec(&plan.plan_id, &[choice("FS25_Same", true)]).unwrap();

    assert_eq!(
        failed(&result),
        [("FS25_Same".to_string(), ErrorCode::RecycleUnavailable)]
    );
    assert_eq!(
        result.done,
        ["FS25_Fresh"],
        "the other mod still goes through"
    );
    assert_eq!(names(&t.b), ["FS25_Fresh.zip", "fs25_same.zip"]);
    assert_eq!(version_in(&t.b, "fs25_same.zip"), "1.0.0.0");
    assert_eq!(version_in(&t.a, "FS25_Same.zip"), "2.0.0.0");
    assert!(t.recycler.binned().is_empty());
    assert_eq!(
        t.rows(),
        [
            ("recycle".to_string(), "recycleUnavailable".to_string()),
            ("copy".to_string(), "recycleUnavailable".to_string()),
            ("copy".to_string(), "ok".to_string()),
        ]
    );
}

#[test]
fn a_replace_that_fails_to_recycle_for_another_reason_is_a_write_failure() {
    let (t, plan) = clash_fixture();
    t.recycler.fail(&t.b.join("fs25_same.zip"));
    let result = t.exec(&plan.plan_id, &[choice("FS25_Same", true)]).unwrap();
    assert_eq!(
        failed(&result),
        [("FS25_Same".to_string(), ErrorCode::WriteFailed)]
    );
    assert_eq!(names(&t.b), ["FS25_Fresh.zip", "fs25_same.zip"]);
}

// ---------------------------------------------------------------- execute: changes since planning

#[test]
fn a_source_that_changed_since_planning_fails_that_item_only() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    write_mod(&t.a, "B.zip", "B", "1.0.0.0");
    let plan = t.plan(&["A.zip", "B.zip"], TransferMode::Copy);
    write_mod(&t.a, "A.zip", "A with a much longer title now", "1.0.0.1");

    let result = t.exec(&plan.plan_id, &[]).unwrap();

    assert_eq!(
        failed(&result),
        [("A".to_string(), ErrorCode::TargetChanged)]
    );
    assert_eq!(result.done, ["B"]);
    assert_eq!(names(&t.b), ["B.zip"]);
}

#[test]
fn an_existing_mod_that_changed_since_planning_is_not_recycled() {
    let (t, plan) = clash_fixture();
    write_mod(&t.b, "fs25_same.zip", "Edited meanwhile, longer", "1.0.0.9");
    let result = t.exec(&plan.plan_id, &[choice("FS25_Same", true)]).unwrap();
    assert_eq!(
        failed(&result),
        [("FS25_Same".to_string(), ErrorCode::TargetChanged)]
    );
    assert!(t.recycler.calls().is_empty());
    assert_eq!(version_in(&t.b, "fs25_same.zip"), "1.0.0.9");
}

#[test]
fn a_name_that_appeared_in_the_target_since_planning_is_not_overwritten() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    let plan = t.plan(&["A.zip"], TransferMode::Copy);
    assert!(plan.clashes.is_empty());
    let theirs = write_mod(&t.b, "A.zip", "Theirs", "9.0.0.0");
    let theirs_bytes = fs::read(&theirs).unwrap();

    let result = t.exec(&plan.plan_id, &[]).unwrap();

    assert_eq!(
        failed(&result),
        [("A".to_string(), ErrorCode::TargetChanged)]
    );
    assert_eq!(fs::read(&theirs).unwrap(), theirs_bytes);
    assert_eq!(names(&t.b), ["A.zip"], "no staged copy is left behind");
}

// ---------------------------------------------------------------- execute: guards

#[test]
fn not_enough_space_refuses_before_anything_is_written() {
    let (t, plan) = clash_fixture();
    t.disk.set(plan.total_bytes - 1);
    let before = names(&t.b);
    assert_eq!(
        err_code(t.exec(&plan.plan_id, &[choice("FS25_Same", true)])),
        ErrorCode::NotEnoughSpace
    );
    assert_eq!(names(&t.b), before);
    assert!(t.recycler.calls().is_empty());
    assert!(t.journal().is_empty());

    // Skipped items need no room, and the plan can be run again once there is.
    t.disk.set(plan.total_bytes);
    assert!(t.exec(&plan.plan_id, &[choice("FS25_Same", true)]).is_ok());
}

#[test]
fn skipped_items_do_not_count_against_free_space() {
    let (t, plan) = clash_fixture();
    let fresh = plan
        .items
        .iter()
        .find(|i| i.mod_name == "FS25_Fresh")
        .unwrap();
    t.disk.set(fresh.size_bytes);
    let result = t.exec(&plan.plan_id, &[]).unwrap();
    assert_eq!(result.done, ["FS25_Fresh"]);
}

#[test]
fn a_same_volume_move_needs_no_free_space() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    t.disk.set(0);
    let plan = t.plan(&["A.zip"], TransferMode::Move);
    assert_eq!(t.exec(&plan.plan_id, &[]).unwrap().done, ["A"]);
}

#[test]
fn the_game_or_server_running_refuses_the_whole_operation() {
    let (t, plan) = clash_fixture();
    t.game.store(true, Ordering::SeqCst);
    assert_eq!(err_code(t.exec(&plan.plan_id, &[])), ErrorCode::GameRunning);
    t.game.store(false, Ordering::SeqCst);
    t.server.store(true, Ordering::SeqCst);
    assert_eq!(
        err_code(t.exec(&plan.plan_id, &[])),
        ErrorCode::ServerRunning
    );
    assert_eq!(names(&t.b), ["fs25_same.zip"]);
    assert!(t.journal().is_empty());
}

#[test]
fn a_second_operation_while_one_runs_is_busy_and_reads_still_work() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    write_mod(&t.a, "B.zip", "B", "1.0.0.0");
    let plan = t.plan(&["A.zip", "B.zip"], TransferMode::Copy);
    let other = t.plan(&["A.zip"], TransferMode::Copy);

    let seen = Mutex::new(Vec::new());
    let result = t
        .exec_with(&plan.plan_id, &[], &|_| {
            // Called from inside the operation, so no lock may be held around this callback.
            let nested = t.with_env(|env| {
                t.lib
                    .execute_transfer(&other.plan_id, &[], &t.ctx, env, &|_| {})
            });
            let removal = t.with_env(|env| {
                t.lib
                    .remove_mods(&t.pa, &ids(&["A.zip"]), &t.ctx, env, &|_| {})
            });
            let listed = t.lib.list_mods(&t.pa, &t.ctx, &|_| {}).unwrap().len();
            let profiles = t.lib.list_profiles(&t.ctx).unwrap().len();
            seen.lock().unwrap().push((
                nested.map(|_| ()).unwrap_err().code,
                removal.map(|_| ()).unwrap_err().code,
                listed,
                profiles,
            ));
        })
        .unwrap();

    assert_eq!(result.done, ["A", "B"]);
    let seen = seen.into_inner().unwrap();
    assert!(!seen.is_empty());
    assert!(seen
        .iter()
        .all(|s| *s == (ErrorCode::Busy, ErrorCode::Busy, 2, 2)));
    assert!(
        names(&t.a).contains(&"A.zip".to_string()),
        "nothing was removed"
    );

    // The flag is released afterwards.
    assert!(t.exec(&other.plan_id, &[]).is_ok());
}

#[test]
fn a_panic_in_a_progress_callback_releases_the_busy_flag() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    let plan = t.plan(&["A.zip"], TransferMode::Copy);
    let again = t.plan(&["A.zip"], TransferMode::Copy);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = t.exec_with(&plan.plan_id, &[], &|_| panic!("callback failed"));
    }));
    assert!(outcome.is_err());
    // Not busy any more (a stale plan is the only reason left to fail).
    assert!(t.exec(&again.plan_id, &[]).is_ok());
}

#[test]
fn the_game_starting_mid_operation_stops_the_remaining_items() {
    let t = T::new();
    for n in ["A", "B", "C", "D"] {
        write_mod(&t.a, &format!("{n}.zip"), n, "1.0.0.0");
    }
    let plan = t.plan(&["A.zip", "B.zip", "C.zip", "D.zip"], TransferMode::Copy);
    // Call 0 is the up-front check, calls 1 and 2 belong to A and B: the game starts at C's.
    t.game_from_call.store(3, Ordering::SeqCst);

    let result = t.exec(&plan.plan_id, &[]).unwrap();

    assert_eq!(result.done, ["A", "B"]);
    assert_eq!(
        failed(&result),
        [
            ("C".to_string(), ErrorCode::GameRunning),
            ("D".to_string(), ErrorCode::GameRunning)
        ]
    );
    assert_eq!(names(&t.b), ["A.zip", "B.zip"]);
    // C was attempted and journaled; D was never reached.
    assert_eq!(
        t.rows(),
        [
            ("copy".to_string(), "ok".to_string()),
            ("copy".to_string(), "ok".to_string()),
            ("copy".to_string(), "gameRunning".to_string()),
        ]
    );
}

#[test]
fn the_server_starting_mid_operation_reports_the_server() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    write_mod(&t.a, "B.zip", "B", "1.0.0.0");
    let plan = t.plan(&["A.zip", "B.zip"], TransferMode::Copy);
    // The server flag is raised by the first per-item check.
    let result = t.with_env(|env| {
        let running = || {
            let n = t.running_calls.fetch_add(1, Ordering::SeqCst);
            Running {
                game: false,
                server: n >= 1,
            }
        };
        let env = OpEnv {
            running: &running,
            recycler: env.recycler,
            disk: env.disk,
        };
        t.running_calls.store(0, Ordering::SeqCst);
        t.lib
            .execute_transfer(&plan.plan_id, &[], &t.ctx, &env, &|_| {})
    });
    let result = result.unwrap();
    assert!(result.done.is_empty());
    assert_eq!(
        failed(&result),
        [
            ("A".to_string(), ErrorCode::ServerRunning),
            ("B".to_string(), ErrorCode::ServerRunning)
        ]
    );
}

// ---------------------------------------------------------------- execute: cancel

#[test]
fn cancelling_mid_copy_leaves_no_partial_mod_and_fails_the_rest_as_cancelled() {
    let t = T::new();
    write_big(&t.a, "Big.zip", 9 * MIB);
    write_mod(&t.a, "Small.zip", "Small", "1.0.0.0");
    let plan = t.plan(&["Big.zip", "Small.zip"], TransferMode::Copy);
    // Requested right after Big's pre-copy check, so the copy itself notices it.
    t.cancel_after_call.store(1, Ordering::SeqCst);

    let result = t.exec(&plan.plan_id, &[]).unwrap();

    assert!(result.done.is_empty());
    assert_eq!(
        failed(&result),
        [
            ("Big".to_string(), ErrorCode::Cancelled),
            ("Small".to_string(), ErrorCode::Cancelled)
        ]
    );
    assert!(names(&t.b).is_empty(), "no staged and no final copy");
    assert_eq!(t.rows(), [("copy".to_string(), "cancelled".to_string())]);
    assert_eq!(names(&t.a), ["Big.zip", "Small.zip"]);
}

#[test]
fn a_cancel_between_items_fails_the_items_not_started() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    write_mod(&t.a, "B.zip", "B", "1.0.0.0");
    let plan = t.plan(&["A.zip", "B.zip"], TransferMode::Copy);
    // Call 0 is the up-front check and call 1 is A's; the cancel comes with B's check, so A is
    // already done and B is stopped before it writes anything.
    t.cancel_after_call.store(2, Ordering::SeqCst);
    let result = t.exec(&plan.plan_id, &[]).unwrap();
    assert_eq!(result.done, ["A"]);
    assert_eq!(failed(&result), [("B".to_string(), ErrorCode::Cancelled)]);
    assert_eq!(names(&t.b), ["A.zip"]);
}

#[test]
fn cancelling_when_idle_is_harmless_and_the_next_operation_starts_clean() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    t.lib.cancel_operation();
    t.lib.cancel_operation();
    let plan = t.plan(&["A.zip"], TransferMode::Copy);
    assert_eq!(t.exec(&plan.plan_id, &[]).unwrap().done, ["A"]);
}

// ---------------------------------------------------------------- execute: plans

#[test]
fn an_unknown_or_used_plan_is_reported_as_expired() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    let e = t.exec("t999", &[]).unwrap_err();
    assert_eq!(e.code, ErrorCode::TargetChanged);
    assert_eq!(e.detail.as_deref(), Some(PLAN_EXPIRED));

    let plan = t.plan(&["A.zip"], TransferMode::Copy);
    t.exec(&plan.plan_id, &[]).unwrap();
    let again = t.exec(&plan.plan_id, &[]).unwrap_err();
    assert_eq!(again.detail.as_deref(), Some(PLAN_EXPIRED));
}

#[test]
fn a_plan_older_than_ten_minutes_is_expired_and_forgotten() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    let plan = t.plan(&["A.zip"], TransferMode::Copy);
    lock(&t.lib.transfer.plans)
        .get_mut(&plan.plan_id)
        .unwrap()
        .created = Instant::now()
        .checked_sub(PLAN_TTL + Duration::from_secs(1))
        .unwrap();

    let e = t.exec(&plan.plan_id, &[]).unwrap_err();
    assert_eq!(e.detail.as_deref(), Some(PLAN_EXPIRED));
    assert!(names(&t.b).is_empty());
    assert!(lock(&t.lib.transfer.plans).is_empty());
}

#[test]
fn plan_ids_are_distinct_and_expired_plans_are_dropped_when_planning() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    let first = t.plan(&["A.zip"], TransferMode::Copy);
    let second = t.plan(&["A.zip"], TransferMode::Copy);
    assert_ne!(first.plan_id, second.plan_id);
    lock(&t.lib.transfer.plans)
        .get_mut(&first.plan_id)
        .unwrap()
        .created = Instant::now()
        .checked_sub(PLAN_TTL + Duration::from_secs(1))
        .unwrap();
    let third = t.plan(&["A.zip"], TransferMode::Copy);
    let plans = lock(&t.lib.transfer.plans);
    assert!(!plans.contains_key(&first.plan_id));
    assert!(plans.contains_key(&second.plan_id) && plans.contains_key(&third.plan_id));
}

// ---------------------------------------------------------------- execute: progress and journal

#[test]
fn progress_is_monotonic_and_has_a_first_and_a_final_event() {
    let t = T::new();
    write_big(&t.a, "Big.zip", 9 * MIB);
    write_mod(&t.a, "Small.zip", "Small", "1.0.0.0");
    let plan = t.plan(&["Big.zip", "Small.zip"], TransferMode::Copy);
    let events = Mutex::new(Vec::new());

    t.exec_with(&plan.plan_id, &[], &|p| events.lock().unwrap().push(p))
        .unwrap();

    let events = events.into_inner().unwrap();
    assert!(events.len() >= 2);
    let first = &events[0];
    assert_eq!((first.done_bytes, first.done_items), (0, 0));
    assert_eq!(first.total_items, 2);
    assert_eq!(first.total_bytes, plan.total_bytes);
    let last = events.last().unwrap();
    assert_eq!(last.done_bytes, plan.total_bytes);
    assert_eq!((last.done_items, last.total_items), (2, 2));
    assert_eq!(last.current, None);
    for pair in events.windows(2) {
        assert!(pair[1].done_bytes >= pair[0].done_bytes);
        assert!(pair[1].done_items >= pair[0].done_items);
    }
    assert!(events.iter().all(|e| e.done_bytes <= e.total_bytes));
    // Events are throttled, so which ones carry a name varies; any name must be an item's.
    assert!(events
        .iter()
        .filter_map(|e| e.current.as_deref())
        .all(|c| c == "Big.zip" || c == "Small.zip"));
}

#[test]
fn progress_for_a_failed_item_still_ends_complete() {
    let (t, plan) = clash_fixture();
    t.recycler.refuse(&t.b.join("fs25_same.zip"));
    let events = Mutex::new(Vec::new());
    t.exec_with(&plan.plan_id, &[choice("FS25_Same", true)], &|p| {
        events.lock().unwrap().push(p);
    })
    .unwrap();
    let events = events.into_inner().unwrap();
    let last = events.last().unwrap();
    assert_eq!((last.done_items, last.total_items), (2, 2));
    assert_eq!(last.done_bytes, last.total_bytes);
}

#[test]
fn the_journal_records_each_item_with_paths_plan_and_result() {
    let (t, plan) = clash_fixture();
    t.exec(&plan.plan_id, &[choice("FS25_Same", true)]).unwrap();

    let rows = t.journal();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| r.id.starts_with('f') && r.at > 0));
    assert!(rows
        .iter()
        .all(|r| r.plan_id.as_deref() == Some(plan.plan_id.as_str())));
    let recycle = &rows[0];
    assert_eq!(recycle.kind, "recycle");
    assert_eq!(recycle.source, t.b.join("fs25_same.zip"));
    assert_eq!(recycle.target, None);
    let copy = &rows[1];
    assert_eq!(copy.kind, "copy");
    assert_eq!(copy.source, t.a.join("FS25_Same.zip"));
    assert_eq!(copy.target, Some(t.b.join("FS25_Same.zip")));
    assert_eq!(copy.result, "ok");
}

#[test]
fn a_recycle_refusal_after_a_good_cross_drive_copy_is_reported_as_recycle_unavailable() {
    assert_eq!(
        file_op_code(&FileOpError::Recycle(RecycleError::Unavailable)),
        ErrorCode::RecycleUnavailable
    );
    assert_eq!(
        file_op_code(&FileOpError::Recycle(RecycleError::Failed("x".into()))),
        ErrorCode::WriteFailed
    );
    assert_eq!(file_op_code(&FileOpError::Cancelled), ErrorCode::Cancelled);
    assert_eq!(file_op_code(&FileOpError::Exists), ErrorCode::TargetChanged);
    assert_eq!(
        file_op_code(&FileOpError::Verify("size".into())),
        ErrorCode::WriteFailed
    );
    assert_eq!(
        file_op_code(&FileOpError::Io("NotFound".into())),
        ErrorCode::WriteFailed
    );
}

// ---------------------------------------------------------------- remove_mods

#[test]
fn removing_mods_sends_them_to_the_bin_and_refreshes_the_list() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    write_mod(&t.a, "B.zip", "B", "1.0.0.0");
    let folder = t.a.join("FS25_Folder");
    write_folder(
        &folder,
        &[
            ("modDesc.xml", desc("F", "1.0.0.0").as_bytes()),
            ("x/y.lua", b"1"),
        ],
    );
    write_mod(&t.b, "A.zip", "A", "1.0.0.0");
    // `also_in` uses the other profiles' last scans, so b is read before a.
    assert_eq!(t.list(&t.pb).len(), 1);
    let first = t.list(&t.pa);
    assert_eq!(first.len(), 3);
    assert_eq!(first[0].also_in, std::slice::from_ref(&t.pb));
    let events = Mutex::new(Vec::new());

    let result = t
        .remove_with(&["A.zip", "FS25_Folder"], &|p| {
            events.lock().unwrap().push(p)
        })
        .unwrap();

    assert_eq!(result.done, ["A", "FS25_Folder"]);
    assert!(result.failed.is_empty());
    assert_eq!(names(&t.a), ["B.zip"]);
    assert_eq!(t.recycler.binned(), ["A.zip", "FS25_Folder"]);
    assert_eq!(names(&t.b), ["A.zip"], "the other profile is untouched");
    assert_eq!(t.list(&t.pa).len(), 1);
    assert!(t.list(&t.pb)[0].also_in.is_empty());
    assert_eq!(
        t.rows(),
        [
            ("recycle".to_string(), "ok".to_string()),
            ("recycle".to_string(), "ok".to_string())
        ]
    );
    let journal = t.journal();
    assert_eq!(journal[0].source, t.a.join("A.zip"));
    assert_eq!(
        (journal[0].target.as_ref(), journal[0].plan_id.as_ref()),
        (None, None)
    );

    let events = events.into_inner().unwrap();
    let last = events.last().unwrap();
    assert_eq!((last.done_items, last.total_items), (2, 2));
    assert_eq!(last.done_bytes, last.total_bytes);
    assert_eq!(events[0].done_items, 0);
}

#[test]
fn a_mod_the_bin_refuses_stays_where_it_is() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    write_mod(&t.a, "B.zip", "B", "1.0.0.0");
    t.list(&t.pa);
    t.recycler.refuse(&t.a.join("A.zip"));

    let result = t.remove(&["A.zip", "B.zip"]).unwrap();

    assert_eq!(
        failed(&result),
        [("A".to_string(), ErrorCode::RecycleUnavailable)]
    );
    assert_eq!(result.done, ["B"]);
    assert_eq!(names(&t.a), ["A.zip"]);
    assert_eq!(
        t.rows(),
        [
            ("recycle".to_string(), "recycleUnavailable".to_string()),
            ("recycle".to_string(), "ok".to_string())
        ]
    );
}

#[test]
fn a_recycle_that_fails_for_another_reason_is_a_write_failure() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    t.list(&t.pa);
    t.recycler.fail(&t.a.join("A.zip"));
    let result = t.remove(&["A.zip"]).unwrap();
    assert_eq!(failed(&result), [("A".to_string(), ErrorCode::WriteFailed)]);
    assert_eq!(names(&t.a), ["A.zip"]);
}

#[test]
fn a_mod_that_changed_since_the_last_scan_is_not_removed() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    t.list(&t.pa);
    write_mod(&t.a, "A.zip", "A edited since the list was read", "1.0.0.1");
    let result = t.remove(&["A.zip"]).unwrap();
    assert_eq!(
        failed(&result),
        [("A".to_string(), ErrorCode::TargetChanged)]
    );
    assert_eq!(names(&t.a), ["A.zip"]);
    assert!(t.recycler.calls().is_empty());
}

#[test]
fn removing_is_refused_while_the_game_runs_and_for_unknown_mods_or_profiles() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    t.list(&t.pa);

    t.game.store(true, Ordering::SeqCst);
    assert_eq!(err_code(t.remove(&["A.zip"])), ErrorCode::GameRunning);
    t.game.store(false, Ordering::SeqCst);
    t.server.store(true, Ordering::SeqCst);
    assert_eq!(err_code(t.remove(&["A.zip"])), ErrorCode::ServerRunning);
    t.server.store(false, Ordering::SeqCst);

    // One unknown id refuses everything before anything is recycled.
    let e = t.remove(&["A.zip", "Nope.zip"]).unwrap_err();
    assert_eq!(e.code, ErrorCode::FolderMissing);
    assert_eq!(names(&t.a), ["A.zip"]);

    let unknown = t.with_env(|env| {
        t.lib
            .remove_mods("p999", &ids(&["A.zip"]), &t.ctx, env, &|_| {})
    });
    assert_eq!(err_code(unknown), ErrorCode::ProfileNotFound);
    assert!(t.recycler.calls().is_empty());
}

#[test]
fn the_game_starting_mid_removal_stops_the_rest() {
    let t = T::new();
    for n in ["A", "B", "C"] {
        write_mod(&t.a, &format!("{n}.zip"), n, "1.0.0.0");
    }
    t.list(&t.pa);
    t.game_from_call.store(2, Ordering::SeqCst);
    let result = t.remove(&["A.zip", "B.zip", "C.zip"]).unwrap();
    assert_eq!(result.done, ["A"]);
    assert_eq!(
        failed(&result),
        [
            ("B".to_string(), ErrorCode::GameRunning),
            ("C".to_string(), ErrorCode::GameRunning)
        ]
    );
    assert_eq!(names(&t.a), ["B.zip", "C.zip"]);
}

#[test]
fn a_removal_runs_one_at_a_time_too() {
    let t = T::new();
    write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    write_mod(&t.a, "B.zip", "B", "1.0.0.0");
    t.list(&t.pa);
    let seen = Mutex::new(Vec::new());
    let result = t
        .remove_with(&["A.zip"], &|_| {
            let nested = t.with_env(|env| {
                t.lib
                    .remove_mods(&t.pa, &ids(&["B.zip"]), &t.ctx, env, &|_| {})
            });
            seen.lock().unwrap().push(nested.unwrap_err().code);
        })
        .unwrap();
    assert_eq!(result.done, ["A"]);
    assert!(seen
        .into_inner()
        .unwrap()
        .iter()
        .all(|c| *c == ErrorCode::Busy));
    assert_eq!(names(&t.a), ["B.zip"]);
}

// ---------------------------------------------------------------- nothing is deleted outright

/// The bytes of every file directly inside `dir`.
fn contents(dir: &Path) -> BTreeSet<Vec<u8>> {
    names(dir)
        .iter()
        .map(|name| fs::read(dir.join(name)).unwrap())
        .collect()
}

fn all_files(t: &T) -> BTreeSet<Vec<u8>> {
    let mut out = contents(&t.a);
    out.extend(contents(&t.b));
    out
}

#[test]
fn every_mod_that_disappears_is_in_the_bin_or_in_its_new_home() {
    let t = T::new();
    write_mod(&t.a, "Moved.zip", "Moved", "1.0.0.0");
    write_mod(&t.a, "Replaced.zip", "New", "2.0.0.0");
    write_mod(&t.a, "Removed.zip", "Removed", "1.0.0.0");
    write_mod(&t.a, "Kept.zip", "Kept", "1.0.0.0");
    write_mod(&t.b, "Replaced.zip", "Old", "1.0.0.0");
    t.list(&t.pa);
    let before = all_files(&t);

    let plan = t.plan(&["Replaced.zip"], TransferMode::Copy);
    t.exec(&plan.plan_id, &[choice("Replaced", true)]).unwrap();
    let plan = t.plan(&["Moved.zip"], TransferMode::Move);
    t.exec(&plan.plan_id, &[]).unwrap();
    t.remove(&["Removed.zip"]).unwrap();

    // Compared by content: the replaced mod keeps its name, but its old bytes must be in the bin.
    let after = all_files(&t);
    let in_bin = contents(&t._tmp.path().join("bin"));
    let lost: Vec<&Vec<u8>> = before.difference(&after).collect();
    assert_eq!(lost.len(), 2, "the old Replaced and Removed");
    for content in lost {
        assert!(
            in_bin.contains(content),
            "a mod vanished without being recycled"
        );
    }
    assert_eq!(t.recycler.binned(), ["Removed.zip", "Replaced.zip"]);
    assert_eq!(t.recycler.calls().len(), 2);
    assert_eq!(names(&t.a), ["Kept.zip", "Replaced.zip"]);
    assert_eq!(names(&t.b), ["Moved.zip", "Replaced.zip"]);
}

// ---------------------------------------------------------------- profiles

#[test]
fn renaming_a_profile_trims_validates_and_leaves_the_folder_alone() {
    let t = T::new();
    let mod_path = write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    let bytes = fs::read(&mod_path).unwrap();

    t.lib.rename_profile(&t.pa, "  Multiplayer farm  ").unwrap();
    let profiles = t.lib.list_profiles(&t.ctx).unwrap();
    assert_eq!(profiles[0].name, "Multiplayer farm");
    assert_eq!(profiles[1].name, "b");
    assert_eq!(profiles[0].path, t.a.to_string_lossy());

    let invalid = |name: &str| {
        let e = t.lib.rename_profile(&t.pa, name).unwrap_err();
        (e.code, e.detail)
    };
    let bad = (ErrorCode::Internal, Some(INVALID_NAME.to_string()));
    assert_eq!(invalid(""), bad);
    assert_eq!(invalid("   \t "), bad);
    assert_eq!(invalid(&"x".repeat(101)), bad);
    t.lib.rename_profile(&t.pa, &"x".repeat(100)).unwrap();
    t.lib.rename_profile(&t.pa, &"é".repeat(100)).unwrap();
    assert_eq!(invalid(&"é".repeat(101)), bad);

    assert_eq!(
        err_code(t.lib.rename_profile("p999", "Name")),
        ErrorCode::ProfileNotFound
    );
    assert_eq!(
        err_code(t.lib.rename_profile("nonsense", "Name")),
        ErrorCode::ProfileNotFound
    );
    assert_eq!(fs::read(&mod_path).unwrap(), bytes);
    assert_eq!(names(&t.a), ["A.zip"]);
}

#[test]
fn removing_a_profile_forgets_it_but_never_touches_its_folder() {
    let t = T::new();
    let mod_path = write_mod(&t.a, "A.zip", "A", "1.0.0.0");
    let bytes = fs::read(&mod_path).unwrap();
    t.list(&t.pa);

    t.lib.remove_profile(&t.pa, &t.ctx).unwrap();

    let profiles = t.lib.list_profiles(&t.ctx).unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id, t.pb);
    assert_eq!(
        err_code(t.lib.list_mods(&t.pa, &t.ctx, &|_| {})),
        ErrorCode::ProfileNotFound
    );
    assert_eq!(
        err_code(t.lib.mod_path(&t.pa, "A.zip")),
        ErrorCode::ProfileNotFound
    );
    assert!(t.lib.last_scan(&t.pa).is_none());
    assert!(t.a.is_dir());
    assert_eq!(names(&t.a), ["A.zip"]);
    assert_eq!(fs::read(&mod_path).unwrap(), bytes);
    assert!(t.recycler.calls().is_empty());
    assert_eq!(
        err_code(t.lib.remove_profile(&t.pa, &t.ctx)),
        ErrorCode::ProfileNotFound
    );
}

#[test]
fn the_active_and_default_folders_cannot_be_removed_from_the_list() {
    let t = T::new();
    // The same folders, spelled differently: path keys ignore case and separator style.
    let active = PathBuf::from(t.a.to_string_lossy().to_uppercase().replace('\\', "/"));
    let ctx = GameContext {
        effective_mods_dir: Some(active),
        default_mods_dir: Some(t.b.clone()),
        ..t.ctx.clone()
    };
    assert_eq!(
        err_code(t.lib.remove_profile(&t.pa, &ctx)),
        ErrorCode::CannotRemoveProfile
    );
    assert_eq!(
        err_code(t.lib.remove_profile(&t.pb, &ctx)),
        ErrorCode::CannotRemoveProfile
    );
    assert_eq!(t.lib.list_profiles(&ctx).unwrap().len(), 2);
    assert!(t.lib.remove_profile(&t.pa, &t.ctx).is_ok());
}
