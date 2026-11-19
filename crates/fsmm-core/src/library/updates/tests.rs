//! Update tests (plan M7): temp folders, an in-memory store, a fake catalog and web, a fake
//! Recycle Bin and disk, and a clock the test moves. No network, no real game folder, no real bin.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::*;
use crate::discovery::Running;
use crate::files::fakes::{FakeDisk, FakeRecycler};
use crate::net::fakes::{CatalogCall, FakeCatalog, FakeWeb};
use crate::net::{CatalogMod, CatalogSource, Download, HashMatch};
use crate::store::{Store, StoredFileOp};
use crate::testutil::{write_folder, write_zip, Method};

const COURSEPLAY: &str = "FS25_Courseplay";
const GH_URL: &str =
    "https://github.com/Courseplay/Courseplay_FS25/releases/download/v2.0.0.0/FS25_Courseplay.zip";
const GH_REPO: &str = "https://github.com/Courseplay/Courseplay_FS25";
const MH_PAGE: &str = "https://www.farming-simulator.com/mod.php?mod_id=331515&title=fs2025";
const MH_ZIP: &str =
    "https://cdn27.giants-software.com/modHub/storage/00331515/FS25_Courseplay.zip";
/// A saved real ModHub page: version 8.1.0.3, 3.79 MB, `FS25_Courseplay.zip`.
const PAGE: &str = include_str!("../../net/fixtures/modhub_page.html");

fn bytes_of(version: &str, padding: usize) -> Vec<u8> {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("m.zip");
    let desc = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><modDesc descVersion=\"105\">\
         <title><en>Title</en></title><author>Someone</author><version>{version}</version></modDesc>"
    );
    let pad: Vec<u8> = (0..padding).map(|i| (i % 251) as u8).collect();
    write_zip(
        &path,
        &[("modDesc.xml", desc.as_bytes()), ("data.bin", &pad)],
        Method::Stored,
    );
    fs::read(path).unwrap()
}

fn sha_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn row(
    tech: &str,
    id: &str,
    latest: &str,
    download: Option<(&str, &str)>,
    page: Option<&str>,
) -> LookupResult {
    LookupResult {
        tech_name: Some(tech.to_string()),
        id: id.to_string(),
        title: tech.to_string(),
        latest_version: Some(latest.to_string()),
        download: download.map(|(source, url)| Download {
            source: source.to_string(),
            url: url.to_string(),
        }),
        page_url: page.map(str::to_string),
    }
}

/// The row of Courseplay with a GitHub release and a ModHub page.
fn courseplay_row(latest: &str) -> LookupResult {
    row(
        COURSEPLAY,
        "cp",
        latest,
        Some(("github", GH_URL)),
        Some(MH_PAGE),
    )
}

fn github_detail(latest: &str, source_version: &str, sha: Option<&str>, url: &str) -> CatalogMod {
    CatalogMod {
        id: "cp".to_string(),
        tech_name: Some(COURSEPLAY.to_string()),
        latest_version: Some(latest.to_string()),
        archive_sha256: sha.map(str::to_string),
        page_url: Some(MH_PAGE.to_string()),
        sources: vec![
            CatalogSource {
                source: "modhub".to_string(),
                source_url: Some(MH_PAGE.to_string()),
                version: Some(latest.to_string()),
                installable: false,
                download_url: None,
            },
            CatalogSource {
                source: "github".to_string(),
                source_url: Some(GH_REPO.to_string()),
                version: Some(source_version.to_string()),
                installable: true,
                download_url: Some(url.to_string()),
            },
        ],
    }
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn failed(result: &OpResult) -> Vec<(String, ErrorCode)> {
    result
        .failed
        .iter()
        .map(|f| (f.mod_name.clone(), f.code))
        .collect()
}

fn kinds(rows: &[StoredFileOp]) -> Vec<(&str, &str)> {
    rows.iter()
        .map(|r| (r.kind.as_str(), r.result.as_str()))
        .collect()
}

/// Called with every progress event of an install.
type ProgressHook = Box<dyn Fn(&OpProgress) + Send>;

/// Two profile folders `a` and `b`, a library, the fakes and the knobs a test turns.
struct T {
    _tmp: TempDir,
    a: PathBuf,
    b: PathBuf,
    pa: String,
    pb: String,
    lib: Library,
    ctx: GameContext,
    catalog: FakeCatalog,
    web: FakeWeb,
    recycler: FakeRecycler,
    disk: FakeDisk,
    game: AtomicBool,
    /// Calls of the running check so far (the first is call 0).
    running_calls: AtomicUsize,
    /// The game "starts" from this call of the running check on.
    game_from_call: AtomicUsize,
    clock: Mutex<SystemTime>,
    events: Mutex<Vec<OpProgress>>,
    on_progress: Mutex<Option<ProgressHook>>,
}

impl T {
    fn new() -> Self {
        Self::with(FakeCatalog::new(), FakeWeb::new())
    }

    fn with(catalog: FakeCatalog, web: FakeWeb) -> Self {
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
            catalog,
            web,
            recycler,
            disk: FakeDisk::new(u64::MAX),
            game: AtomicBool::new(false),
            running_calls: AtomicUsize::new(0),
            game_from_call: AtomicUsize::new(usize::MAX),
            clock: Mutex::new(UNIX_EPOCH + Duration::from_secs(1_800_000_000)),
            events: Mutex::new(Vec::new()),
            on_progress: Mutex::new(None),
        }
    }

    fn enable(&self) {
        self.lib.set_updates_enabled(true).unwrap();
    }

    fn advance(&self, secs: u64) {
        *self.clock.lock().unwrap() += Duration::from_secs(secs);
    }

    fn now(&self) -> SystemTime {
        *self.clock.lock().unwrap()
    }

    fn run<R>(&self, f: impl FnOnce(&UpdateEnv<'_>) -> R) -> R {
        let running = || {
            let call = self.running_calls.fetch_add(1, Ordering::SeqCst);
            Running {
                game: self.game.load(Ordering::SeqCst)
                    || call >= self.game_from_call.load(Ordering::SeqCst),
                server: false,
            }
        };
        let files = OpEnv {
            running: &running,
            recycler: &self.recycler,
            disk: &self.disk,
        };
        let now = || self.now();
        let env = UpdateEnv {
            catalog: &self.catalog,
            web: &self.web,
            files: &files,
            now: &now,
        };
        f(&env)
    }

    fn check(&self, force: bool) -> Result<UpdateCheck, ApiError> {
        self.run(|env| self.lib.check_updates(force, &self.ctx, env))
    }

    fn install(&self, name: &str, profiles: &[&str]) -> Result<OpResult, ApiError> {
        self.running_calls.store(0, Ordering::SeqCst);
        self.events.lock().unwrap().clear();
        let profiles = ids(profiles);
        self.run(|env| {
            self.lib
                .install_update(name, &profiles, &self.ctx, env, &|p| {
                    if let Some(hook) = self.on_progress.lock().unwrap().as_ref() {
                        hook(&p);
                    }
                    self.events.lock().unwrap().push(p);
                })
        })
    }

    fn plan(&self, name: &str) -> Result<UpdatePlan, ApiError> {
        self.lib.plan_update(name, &self.ctx)
    }

    fn list(&self, profile_id: &str) -> Vec<ModEntry> {
        self.lib.list_mods(profile_id, &self.ctx, &|_| {}).unwrap()
    }

    fn entry(&self, profile_id: &str, name: &str) -> ModEntry {
        self.list(profile_id)
            .into_iter()
            .find(|m| m.mod_name.eq_ignore_ascii_case(name))
            .unwrap()
    }

    fn journal(&self) -> Vec<StoredFileOp> {
        self.lib.store.file_ops().unwrap()
    }

    fn put(&self, folder: &Path, file: &str, version: &str) -> PathBuf {
        let path = folder.join(file);
        fs::write(&path, bytes_of(version, 64)).unwrap();
        path
    }

    fn matches(&self) -> HashMap<String, CatalogMatch> {
        self.lib.store.catalog_matches().unwrap()
    }
}

/// Courseplay v1 in folder `a`; the catalog knows v2 on GitHub (hash of the new file) and on
/// ModHub; the web serves the new file. Updates are on and one check has run.
fn github_world() -> (T, Vec<u8>) {
    github_world_with("2.0.0.0")
}

fn github_world_with(download_version: &str) -> (T, Vec<u8>) {
    let new = bytes_of(download_version, 4000);
    let catalog = FakeCatalog::new()
        .with_row(courseplay_row("v2.0.0.0"))
        .with_detail(github_detail(
            "v2.0.0.0",
            "v2.0.0.0",
            Some(&sha_hex(&new)),
            GH_URL,
        ));
    let web = FakeWeb::new()
        .with_chunk(512)
        .with_payload(GH_URL, new.clone());
    let t = T::with(catalog, web);
    t.put(&t.a, "FS25_Courseplay.zip", "1.0.0.0");
    t.enable();
    t.check(false).unwrap();
    (t, new)
}

// ---------------------------------------------------------------- settings

#[test]
fn updates_are_off_by_default_and_the_switch_is_remembered() {
    let t = T::new();
    assert_eq!(
        t.lib.update_settings().unwrap(),
        UpdateSettings {
            enabled: false,
            last_check: None,
            retry_after: None
        }
    );
    assert!(t.lib.set_updates_enabled(true).unwrap().enabled);
    assert!(t.lib.update_settings().unwrap().enabled);
    assert!(!t.lib.set_updates_enabled(false).unwrap().enabled);
    assert!(!t.lib.update_settings().unwrap().enabled);
}

#[test]
fn nothing_is_sent_while_updates_are_off() {
    let (t, _) = github_world();
    let before = t.catalog.request_count();
    t.lib.set_updates_enabled(false).unwrap();
    t.advance(10 * 3600);
    assert_eq!(t.check(true).unwrap_err().code, ErrorCode::UpdatesDisabled);
    assert_eq!(
        t.install(COURSEPLAY, &[&t.pa]).unwrap_err().code,
        ErrorCode::UpdatesDisabled
    );
    assert_eq!(t.catalog.request_count(), before);
    assert!(t.web.calls().is_empty());
    // Listing and planning need no network either, and the results are still there.
    assert!(matches!(
        t.entry(&t.pa, COURSEPLAY).update,
        UpdateState::Available { .. }
    ));
    assert!(t.plan(COURSEPLAY).is_ok());
    assert_eq!(t.catalog.request_count(), before);
}

#[test]
fn a_never_enabled_library_sends_nothing_at_all() {
    let t = T::new();
    t.put(&t.a, "FS25_A.zip", "1.0");
    assert_eq!(t.check(false).unwrap_err().code, ErrorCode::UpdatesDisabled);
    t.list(&t.pa);
    t.lib.list_profiles(&t.ctx).unwrap();
    assert_eq!(t.catalog.request_count(), 0);
    assert!(t.web.calls().is_empty());
}

// ---------------------------------------------------------------- checking

#[test]
fn a_check_looks_up_each_distinct_name_once_across_profiles() {
    let t = T::new();
    t.put(&t.a, "FS25_One.zip", "1.0");
    t.put(&t.a, "FS25_Two.zip", "1.0");
    t.put(&t.b, "fs25_one.zip", "1.0"); // the same mod, spelled differently
    t.put(&t.b, "FS25_Three.zip", "1.0");
    t.enable();
    let result = t.check(false).unwrap();
    let lookups = t.catalog.lookups();
    assert_eq!(lookups.len(), 1);
    let mut asked: Vec<String> = lookups[0].iter().map(|n| n.to_lowercase()).collect();
    asked.sort();
    assert_eq!(asked, ["fs25_one", "fs25_three", "fs25_two"]);
    assert_eq!(result.checked, 3);
    assert!(!result.from_cache);
    assert_eq!(result.checked_at, iso(t.now()));
}

#[test]
fn a_thousand_and_one_mods_make_exactly_three_lookups() {
    let t = T::new();
    for i in 0..1001 {
        fs::write(t.a.join(format!("FS25_Mod{i}.zip")), bytes_of("1.0", 0)).unwrap();
    }
    t.enable();
    let result = t.check(false).unwrap();
    let sizes: Vec<usize> = t.catalog.lookups().iter().map(Vec::len).collect();
    assert_eq!(sizes, [500, 500, 1]);
    assert_eq!(result.checked, 1001);
}

#[test]
fn what_is_not_a_mod_is_neither_looked_up_nor_hashed() {
    let t = T::new();
    t.put(&t.a, "FS25_Real.zip", "1.0");
    fs::write(t.a.join("holiday_photos.zip"), b"not a zip at all").unwrap();
    t.enable();
    t.check(false).unwrap();
    assert_eq!(t.catalog.lookups(), [["FS25_Real"]]);
    // The real mod is unmatched, so it is hashed; the photos are not.
    assert_eq!(t.catalog.by_hashes().len(), 1);
    assert_eq!(t.catalog.by_hashes()[0].len(), 1);
}

#[test]
fn results_are_cached_for_three_hours_without_force() {
    let (t, _) = github_world();
    let before = t.catalog.request_count();
    t.advance(3 * 3600 - 1);
    let again = t.check(false).unwrap();
    assert!(again.from_cache);
    assert_eq!(t.catalog.request_count(), before, "nothing was sent");
    t.advance(1);
    let fresh = t.check(false).unwrap();
    assert!(!fresh.from_cache);
    assert!(t.catalog.request_count() > before);
}

#[test]
fn the_cached_answer_repeats_the_counts_of_the_check() {
    let (t, _) = github_world();
    let first = t.advance_and_force_check(61);
    assert!(!first.from_cache);
    let cached = t.check(false).unwrap();
    assert!(cached.from_cache);
    assert_eq!(cached.checked_at, first.checked_at);
    assert_eq!(
        (
            cached.checked,
            cached.matched_by_name,
            cached.matched_by_hash,
            cached.updates
        ),
        (
            first.checked,
            first.matched_by_name,
            first.matched_by_hash,
            first.updates
        )
    );
    assert_eq!(
        (cached.checked, cached.matched_by_name, cached.updates),
        (1, 1, 1)
    );
}

impl T {
    fn advance_and_force_check(&self, secs: u64) -> UpdateCheck {
        self.advance(secs);
        self.check(true).unwrap()
    }
}

#[test]
fn a_forced_check_is_still_debounced_for_a_minute() {
    let (t, _) = github_world();
    let before = t.catalog.request_count();
    t.advance(59);
    let forced = t.check(true).unwrap();
    assert!(forced.from_cache, "59 s is too soon even when forced");
    assert_eq!(t.catalog.request_count(), before);
    t.advance(1);
    let forced = t.check(true).unwrap();
    assert!(!forced.from_cache, "60 s is enough");
    assert!(t.catalog.request_count() > before);
}

#[test]
fn a_rate_limit_is_remembered_and_nothing_is_sent_before_it_ends() {
    let t = T::new();
    t.put(&t.a, "FS25_A.zip", "1.0");
    t.enable();
    let until = t.now() + Duration::from_secs(600);
    t.catalog.fail_next(NetError::RateLimited {
        retry_after: Some(until),
    });
    let error = t.check(false).unwrap_err();
    assert_eq!(error.code, ErrorCode::RateLimited);
    assert_eq!(error.detail.as_deref(), Some(iso(until).as_str()));
    assert_eq!(
        t.lib.update_settings().unwrap().retry_after.as_deref(),
        Some(iso(until).as_str())
    );
    assert_eq!(t.catalog.request_count(), 1, "no retry loop");

    for secs in [1, 100, 498] {
        t.advance(secs);
        let error = t.check(true).unwrap_err();
        assert_eq!(error.code, ErrorCode::RateLimited);
        assert_eq!(error.detail.as_deref(), Some(iso(until).as_str()));
    }
    assert_eq!(t.catalog.request_count(), 1, "nothing before retry-after");

    t.advance(200);
    t.check(true).unwrap();
    assert!(t.catalog.request_count() > 1);
    assert_eq!(t.lib.update_settings().unwrap().retry_after, None);
}

#[test]
fn a_rate_limit_without_a_time_means_a_minute() {
    let t = T::new();
    t.put(&t.a, "FS25_A.zip", "1.0");
    t.enable();
    t.catalog
        .fail_next(NetError::RateLimited { retry_after: None });
    let error = t.check(false).unwrap_err();
    assert_eq!(
        error.detail.as_deref(),
        Some(iso(t.now() + Duration::from_secs(60)).as_str())
    );
}

#[test]
fn a_rate_limit_mid_check_leaves_the_previous_results_intact() {
    let (t, _) = github_world();
    let stored = t.matches();
    let settings = t.lib.update_settings().unwrap();
    assert_eq!(stored.len(), 1);
    // The next check gets through the lookup and is refused on the by-hash request.
    t.put(&t.a, "FS25_Extra.zip", "1.0");
    t.advance(4 * 3600);
    let until = t.now() + Duration::from_secs(90);
    t.catalog.fail_request(
        t.catalog.request_count() + 1,
        NetError::RateLimited {
            retry_after: Some(until),
        },
    );
    let error = t.check(false).unwrap_err();
    assert_eq!(error.code, ErrorCode::RateLimited);
    assert_eq!(t.matches(), stored, "the old results stand");
    let after = t.lib.update_settings().unwrap();
    assert_eq!(
        after.last_check, settings.last_check,
        "it did not count as a check"
    );
    assert_eq!(after.retry_after.as_deref(), Some(iso(until).as_str()));
    assert!(matches!(
        t.entry(&t.pa, COURSEPLAY).update,
        UpdateState::Available { .. }
    ));
}

#[test]
fn offline_and_server_errors_fail_the_check_and_keep_the_old_results() {
    let (t, _) = github_world();
    let stored = t.matches();
    for (error, detail) in [
        (NetError::Offline, None),
        (NetError::Status(500), Some("status500")),
        (NetError::Status(503), Some("status503")),
    ] {
        t.advance(4 * 3600);
        t.catalog.fail_next(error);
        let failure = t.check(false).unwrap_err();
        assert_eq!(failure.code, ErrorCode::Offline);
        assert_eq!(failure.detail.as_deref(), detail);
        assert_eq!(t.matches(), stored);
    }
    // A failure does not start a cool-down: the next attempt goes out.
    let before = t.catalog.request_count();
    t.check(false).unwrap();
    assert!(t.catalog.request_count() > before);
}

#[test]
fn an_unreachable_catalog_is_asked_once_per_check_with_no_retry_loop() {
    let t = T::new();
    t.put(&t.a, "FS25_A.zip", "1.0");
    t.enable();
    t.catalog.fail_all(NetError::Offline);
    assert_eq!(t.check(false).unwrap_err().code, ErrorCode::Offline);
    assert_eq!(t.catalog.request_count(), 1);
    assert_eq!(t.check(true).unwrap_err().code, ErrorCode::Offline);
    assert_eq!(t.catalog.request_count(), 2);
    assert_eq!(t.lib.update_settings().unwrap().last_check, None);
}

#[test]
fn a_bad_answer_is_an_internal_error_without_the_answer_in_it() {
    let t = T::new();
    t.put(&t.a, "FS25_A.zip", "1.0");
    t.enable();
    t.catalog
        .fail_next(NetError::BadAnswer("json Syntax".to_string()));
    let error = t.check(false).unwrap_err();
    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.detail.unwrap().contains("json Syntax"));
}

#[test]
fn each_successful_check_replaces_the_stored_results() {
    let t = T::with(
        FakeCatalog::new()
            .with_row(row("FS25_One", "1", "2.0", None, Some(MH_PAGE)))
            .with_row(row("FS25_Two", "2", "2.0", None, Some(MH_PAGE))),
        FakeWeb::new(),
    );
    let one = t.put(&t.a, "FS25_One.zip", "1.0");
    t.put(&t.a, "FS25_Two.zip", "1.0");
    t.enable();
    t.check(false).unwrap();
    assert_eq!(t.matches().len(), 2);
    fs::remove_file(one).unwrap();
    t.advance(4 * 3600);
    t.check(false).unwrap();
    let keys: Vec<String> = t.matches().into_keys().collect();
    assert_eq!(keys, ["fs25_two"]);
}

#[test]
fn names_match_the_catalog_ignoring_case_and_the_first_spelling_is_asked() {
    let t = T::with(
        FakeCatalog::new().with_row(row("FS25_Courseplay", "cp", "2.0", None, Some(MH_PAGE))),
        FakeWeb::new(),
    );
    t.put(&t.a, "fs25_courseplay.zip", "1.0");
    t.enable();
    let result = t.check(false).unwrap();
    assert_eq!(t.catalog.lookups(), [["fs25_courseplay"]]);
    assert_eq!(result.matched_by_name, 1);
    assert!(t.matches().contains_key("fs25_courseplay"));
}

#[test]
fn counts_say_what_matched_how() {
    let t = T::with(
        FakeCatalog::new()
            .with_row(courseplay_row("v2.0.0.0"))
            .with_row(row("FS25_Same", "s", "1.0.0.0", None, Some(MH_PAGE))),
        FakeWeb::new(),
    );
    t.put(&t.a, "FS25_Courseplay.zip", "1.0.0.0");
    t.put(&t.a, "FS25_Same.zip", "1.0.0.0");
    t.put(&t.a, "FS25_Unknown.zip", "1.0.0.0");
    t.put(&t.b, "FS25_Courseplay.zip", "1.5.0.0");
    t.enable();
    let result = t.check(false).unwrap();
    assert_eq!(result.checked, 3);
    assert_eq!(result.matched_by_name, 2);
    assert_eq!(result.matched_by_hash, 0);
    assert_eq!(result.updates, 1, "one name, though two folders are older");
}

// ---------------------------------------------------------------- hash matching

#[test]
fn a_renamed_zip_is_matched_by_its_hash_and_gets_its_update() {
    let content = bytes_of("1.0.0.0", 64);
    let hash = sha_hex(&content);
    let catalog = FakeCatalog::new()
        .with_hash(
            &hash,
            vec![HashMatch {
                id: "cp".to_string(),
                tech_name: Some(COURSEPLAY.to_string()),
                version: "v1.0.0.0".to_string(),
                source: "github".to_string(),
                file_name: Some("FS25_Courseplay.zip".to_string()),
                download_url: None,
            }],
        )
        .with_row(courseplay_row("v2.0.0.0"));
    let t = T::with(catalog, FakeWeb::new());
    fs::write(t.a.join("FS25_Courseplay_renamed.zip"), &content).unwrap();
    t.enable();
    let result = t.check(false).unwrap();
    assert_eq!((result.matched_by_name, result.matched_by_hash), (0, 1));
    assert_eq!(result.updates, 1);

    // Requests: the names, the hash, then the catalog's own name for the latest version.
    assert_eq!(
        t.catalog.calls(),
        [
            CatalogCall::Lookup(ids(&["FS25_Courseplay_renamed"])),
            CatalogCall::ByHash(vec![hash.clone()]),
            CatalogCall::Lookup(ids(&[COURSEPLAY])),
        ]
    );
    // Stored under the local name.
    let stored = &t.matches()["fs25_courseplay_renamed"];
    assert_eq!(stored.matched_by, MatchedBy::Hash);
    assert_eq!(stored.catalog_id, "cp");
    assert_eq!(stored.tech_name.as_deref(), Some(COURSEPLAY));
    assert_eq!(stored.latest_version.as_deref(), Some("v2.0.0.0"));
    let entry = t.entry(&t.pa, "FS25_Courseplay_renamed");
    assert_eq!(
        entry.update,
        UpdateState::Available {
            version: "v2.0.0.0".to_string(),
            installable: true
        }
    );
    assert_eq!(
        entry.source,
        ModSource::ModHub {
            id: "331515".to_string()
        }
    );
    // The same file under a name the catalog knows would not need the hash at all, and a
    // renamed zip is still never installed over under another name (see the file-name tests).
}

#[test]
fn the_hash_is_cached_by_size_and_mtime_so_the_next_check_does_not_rehash() {
    let t = T::new();
    let path = t.put(&t.a, "FS25_Mystery.zip", "1.0");
    t.enable();
    t.check(false).unwrap();
    let stamp = {
        let meta = fs::metadata(&path).unwrap();
        FileStamp {
            size: meta.len(),
            modified: meta.modified().unwrap(),
        }
    };
    let cached = t
        .lib
        .store
        .cached_sha256(&[(path.as_path(), stamp)])
        .unwrap();
    let expected = files::hash::sha256(&path, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(cached, [Some(expected)]);

    // A planted value for this size and mtime is used as is, which shows no rehash happens.
    t.lib
        .store
        .put_sha256(&[((path.as_path(), stamp), "planted")])
        .unwrap();
    t.advance(4 * 3600);
    t.check(false).unwrap();
    assert_eq!(t.catalog.by_hashes()[1], ["planted"]);
}

#[test]
fn the_server_without_a_by_hash_route_still_gives_the_name_matches() {
    let t = T::with(
        FakeCatalog::new()
            .with_row(courseplay_row("v2.0.0.0"))
            .without_by_hash(),
        FakeWeb::new(),
    );
    t.put(&t.a, "FS25_Courseplay.zip", "1.0.0.0");
    t.put(&t.a, "FS25_Mystery.zip", "1.0.0.0");
    t.enable();
    let result = t.check(false).unwrap();
    assert_eq!((result.matched_by_name, result.matched_by_hash), (1, 0));
    assert_eq!(t.catalog.by_hashes().len(), 1);
}

#[test]
fn only_zips_without_a_name_match_are_hashed_and_folders_are_not() {
    let t = T::with(
        FakeCatalog::new().with_row(courseplay_row("v2.0.0.0")),
        FakeWeb::new(),
    );
    t.put(&t.a, "FS25_Courseplay.zip", "1.0.0.0");
    t.put(&t.a, "FS25_Mystery.zip", "1.0.0.0");
    write_folder(
        &t.a.join("FS25_Unpacked"),
        &[(
            "modDesc.xml",
            crate::library::fixtures::desc("1.0").as_bytes(),
        )],
    );
    t.enable();
    t.check(false).unwrap();
    assert_eq!(t.catalog.by_hashes().len(), 1);
    assert_eq!(t.catalog.by_hashes()[0].len(), 1, "only the mystery zip");
    // The unpacked folder was looked up by name like the others.
    let mut asked = t.catalog.lookups().concat();
    asked.sort();
    assert_eq!(asked, ["FS25_Courseplay", "FS25_Mystery", "FS25_Unpacked"]);
}

#[test]
fn a_hash_the_catalog_names_but_cannot_look_up_is_left_unmatched() {
    let content = bytes_of("1.0", 64);
    let catalog = FakeCatalog::new().with_hash(
        &sha_hex(&content),
        vec![HashMatch {
            id: "x".to_string(),
            tech_name: Some("FS25_Gone".to_string()),
            version: "1".to_string(),
            source: "github".to_string(),
            file_name: None,
            download_url: None,
        }],
    );
    let t = T::with(catalog, FakeWeb::new());
    fs::write(t.a.join("FS25_Renamed.zip"), &content).unwrap();
    t.enable();
    let result = t.check(false).unwrap();
    assert_eq!((result.matched_by_name, result.matched_by_hash), (0, 0));
    assert!(t.matches().is_empty());
}

// ---------------------------------------------------------------- the overlay

fn manual(t: &T, name: &str, url: &str) {
    t.lib.set_source_link(name, Some(url)).unwrap();
}

#[test]
fn a_matched_mod_shows_its_source_links_and_update() {
    let (t, _) = github_world();
    manual(&t, COURSEPLAY, "https://forum.example.com/courseplay");
    let entry = t.entry(&t.pa, COURSEPLAY);
    assert_eq!(
        entry.source,
        ModSource::ModHub {
            id: "331515".to_string()
        },
        "the catalog first, ModHub before GitHub"
    );
    let links: Vec<(LinkKind, &str)> = entry
        .links
        .iter()
        .map(|l| (l.kind, l.label.as_str()))
        .collect();
    assert_eq!(
        links,
        [
            (LinkKind::ModHub, "ModHub"),
            (LinkKind::GitHub, "GitHub"),
            (LinkKind::Manual, "forum.example.com"),
        ]
    );
    assert_eq!(
        entry.update,
        UpdateState::Available {
            version: "v2.0.0.0".to_string(),
            installable: true
        }
    );
}

#[test]
fn the_source_is_github_when_the_catalog_has_no_modhub_page() {
    let t = T::with(
        FakeCatalog::new().with_row(row(COURSEPLAY, "cp", "2.0", Some(("github", GH_URL)), None)),
        FakeWeb::new(),
    );
    t.put(&t.a, "FS25_Courseplay.zip", "1.0");
    t.enable();
    t.check(false).unwrap();
    let entry = t.entry(&t.pa, COURSEPLAY);
    assert_eq!(
        entry.source,
        ModSource::GitHub {
            owner: "Courseplay".to_string(),
            repo: "Courseplay_FS25".to_string()
        }
    );
    assert_eq!(entry.links.len(), 1);
    assert_eq!(entry.links[0].kind, LinkKind::GitHub);
    assert_eq!(
        t.lib.link_url(COURSEPLAY, LinkKind::GitHub).unwrap(),
        GH_REPO
    );
    assert_eq!(
        t.lib
            .link_url(COURSEPLAY, LinkKind::ModHub)
            .unwrap_err()
            .code,
        ErrorCode::FolderMissing
    );
}

#[test]
fn a_manual_link_is_the_source_when_the_catalog_has_none() {
    let t = T::new();
    t.put(&t.a, "FS25_Local.zip", "1.0");
    assert_eq!(t.entry(&t.pa, "FS25_Local").source, ModSource::Unknown);
    assert!(t.entry(&t.pa, "FS25_Local").links.is_empty());
    manual(&t, "FS25_Local", "https://www.KingMods.net/mods/123");
    let entry = t.entry(&t.pa, "FS25_Local");
    assert_eq!(
        entry.source,
        ModSource::Manual {
            host: "www.kingmods.net".to_string()
        }
    );
    assert_eq!(entry.links.len(), 1);
    assert_eq!(entry.links[0].kind, LinkKind::Manual);
    assert_eq!(entry.links[0].label, "www.kingmods.net");
    assert_eq!(entry.update, UpdateState::Unknown, "no catalog, no checks");
    // The same name in another profile shows it too.
    t.put(&t.b, "FS25_Local.zip", "1.0");
    assert_eq!(t.entry(&t.pb, "FS25_Local").links.len(), 1);
}

#[test]
fn a_manual_link_to_a_github_repository_reads_as_github() {
    let t = T::new();
    t.put(&t.a, "FS25_Local.zip", "1.0");
    manual(
        &t,
        "FS25_Local",
        "https://github.com/someone/FS25_Local/releases",
    );
    let entry = t.entry(&t.pa, "FS25_Local");
    assert_eq!(
        entry.source,
        ModSource::GitHub {
            owner: "someone".to_string(),
            repo: "FS25_Local".to_string()
        }
    );
    // The link itself stays the user's: its button is the manual one.
    assert_eq!(entry.links.len(), 1);
    assert_eq!(entry.links[0].kind, LinkKind::Manual);
    assert_eq!(entry.links[0].label, "github.com");
    // Only a repository counts: a bare owner page is just another site.
    manual(&t, "FS25_Local", "https://github.com/someone");
    assert_eq!(
        t.entry(&t.pa, "FS25_Local").source,
        ModSource::Manual {
            host: "github.com".to_string()
        }
    );
    // An http link to a repository is read the same way.
    manual(&t, "FS25_Local", "http://github.com/someone/FS25_Local");
    assert!(matches!(
        t.entry(&t.pa, "FS25_Local").source,
        ModSource::GitHub { .. }
    ));
}

#[test]
fn the_catalog_beats_a_manual_link_for_the_source_but_both_show() {
    let (t, _) = github_world();
    manual(&t, COURSEPLAY, "https://github.com/other/fork");
    let entry = t.entry(&t.pa, COURSEPLAY);
    assert!(matches!(entry.source, ModSource::ModHub { .. }));
    assert_eq!(entry.links.len(), 3);
}

#[test]
fn update_states_follow_the_versions() {
    let t = T::with(
        FakeCatalog::new()
            .with_row(row("FS25_Newer", "1", "v2.0.0.0", None, Some(MH_PAGE)))
            .with_row(row("FS25_Equal", "2", "v1.0", None, Some(MH_PAGE)))
            .with_row(row("FS25_Older", "3", "0.9", None, Some(MH_PAGE)))
            .with_row(row("FS25_Odd", "4", "nightly", None, Some(MH_PAGE))),
        FakeWeb::new(),
    );
    for name in ["Newer", "Equal", "Older", "Odd", "Unmatched"] {
        t.put(&t.a, &format!("FS25_{name}.zip"), "1.0.0.0");
    }
    t.enable();
    t.check(false).unwrap();
    let state = |name: &str| t.entry(&t.pa, &format!("FS25_{name}")).update;
    assert!(matches!(state("Newer"), UpdateState::Available { .. }));
    assert_eq!(state("Equal"), UpdateState::UpToDate);
    assert_eq!(state("Older"), UpdateState::UpToDate);
    assert_eq!(
        state("Odd"),
        UpdateState::UpToDate,
        "matched, nothing to say"
    );
    assert_eq!(state("Unmatched"), UpdateState::Unknown);
}

#[test]
fn installable_needs_a_download_or_a_modhub_page_and_a_zip() {
    let t = T::with(
        FakeCatalog::new()
            // GitHub asset only.
            .with_row(row("FS25_Gh", "1", "2.0", Some(("github", GH_URL)), None))
            // ModHub page only.
            .with_row(row("FS25_Mh", "2", "2.0", None, Some(MH_PAGE)))
            // Only a GitHub page: nothing to download.
            .with_row(row("FS25_Page", "3", "2.0", None, Some(GH_REPO)))
            // A download from a host the app does not use.
            .with_row(row(
                "FS25_Evil",
                "4",
                "2.0",
                Some(("github", "https://evil.example/FS25_Evil.zip")),
                None,
            ))
            .with_row(row("FS25_Folder", "5", "2.0", None, Some(MH_PAGE))),
        FakeWeb::new(),
    );
    for name in ["Gh", "Mh", "Page", "Evil"] {
        t.put(&t.a, &format!("FS25_{name}.zip"), "1.0");
    }
    write_folder(
        &t.a.join("FS25_Folder"),
        &[(
            "modDesc.xml",
            crate::library::fixtures::desc("1.0").as_bytes(),
        )],
    );
    t.enable();
    t.check(false).unwrap();
    let installable = |name: &str| match t.entry(&t.pa, &format!("FS25_{name}")).update {
        UpdateState::Available { installable, .. } => installable,
        other => panic!("{name}: {other:?}"),
    };
    assert!(installable("Gh"));
    assert!(installable("Mh"));
    assert!(!installable("Page"));
    assert!(!installable("Evil"));
    assert!(
        !installable("Folder"),
        "an unpacked mod cannot be replaced by a zip"
    );
}

#[test]
fn profiles_count_their_available_updates() {
    let t = T::with(
        FakeCatalog::new()
            .with_row(courseplay_row("v2.0.0.0"))
            .with_row(row("FS25_Other", "o", "3.0", None, Some(MH_PAGE)))
            .with_row(row("FS25_Same", "s", "1.0", None, Some(MH_PAGE))),
        FakeWeb::new(),
    );
    t.put(&t.a, "FS25_Courseplay.zip", "1.0");
    t.put(&t.a, "FS25_Other.zip", "1.0");
    t.put(&t.a, "FS25_Same.zip", "1.0");
    t.put(&t.b, "FS25_Courseplay.zip", "2.0.0.0");
    t.put(&t.b, "FS25_Other.zip", "2.0");
    t.enable();
    let counts = |t: &T| -> Vec<u32> {
        t.lib
            .list_profiles(&t.ctx)
            .unwrap()
            .iter()
            .map(|p| p.update_count)
            .collect()
    };
    assert_eq!(counts(&t), [0, 0], "nothing is known before a check");
    t.check(false).unwrap();
    assert_eq!(counts(&t), [2, 1]);
}

#[test]
fn update_states_are_never_part_of_the_scan_cache() {
    let (t, _) = github_world();
    assert!(matches!(
        t.entry(&t.pa, COURSEPLAY).update,
        UpdateState::Available { .. }
    ));
    let scan = t.lib.scan_of(&t.pa).unwrap();
    let record = &scan.records[0].entry;
    assert_eq!(record.update, UpdateState::Unknown);
    assert_eq!(record.source, ModSource::Unknown);
    assert!(record.links.is_empty());
    // A fresh library over the same scan cache would list the same without a check.
    let cached = t
        .lib
        .store
        .cached_mod(&scan.records[0].path, scan.stamps[0])
        .unwrap()
        .unwrap();
    assert_eq!(cached.entry.update, UpdateState::Unknown);
}

#[test]
fn turning_updates_off_forgets_nothing() {
    let (t, _) = github_world();
    t.lib.set_updates_enabled(false).unwrap();
    let entry = t.entry(&t.pa, COURSEPLAY);
    assert!(matches!(entry.update, UpdateState::Available { .. }));
    assert_eq!(entry.links.len(), 2);
    assert!(t.lib.link_url(COURSEPLAY, LinkKind::ModHub).is_ok());
}

// ---------------------------------------------------------------- planning

#[test]
fn a_plan_lists_every_folder_with_its_version_and_asks_nothing() {
    let (t, _) = github_world();
    t.put(&t.b, "FS25_Courseplay.zip", "1.5.0.0");
    let requests = t.catalog.request_count();
    let plan = t.plan(COURSEPLAY).unwrap();
    assert_eq!(plan.mod_name, COURSEPLAY);
    assert_eq!(plan.to_version, "v2.0.0.0");
    assert_eq!(plan.source, LinkKind::GitHub);
    assert_eq!(
        plan.folders,
        [
            UpdateFolder {
                profile_id: t.pa.clone(),
                mod_id: "FS25_Courseplay.zip".to_string(),
                version: "1.0.0.0".to_string()
            },
            UpdateFolder {
                profile_id: t.pb.clone(),
                mod_id: "FS25_Courseplay.zip".to_string(),
                version: "1.5.0.0".to_string()
            },
        ]
    );
    assert_eq!(t.catalog.request_count(), requests);
    assert!(t.web.calls().is_empty());
    // Any spelling of the name finds it.
    assert!(t.plan("fs25_COURSEPLAY").is_ok());
}

#[test]
fn a_plan_prefers_the_github_asset_and_falls_back_to_the_modhub_page() {
    let t = T::with(
        FakeCatalog::new().with_row(row(COURSEPLAY, "cp", "2.0", None, Some(MH_PAGE))),
        FakeWeb::new(),
    );
    t.put(&t.a, "FS25_Courseplay.zip", "1.0");
    t.enable();
    t.check(false).unwrap();
    assert_eq!(t.plan(COURSEPLAY).unwrap().source, LinkKind::ModHub);
}

#[test]
fn a_plan_says_why_there_is_nothing_to_install() {
    let t = T::with(
        FakeCatalog::new()
            .with_row(row("FS25_Current", "1", "1.0", None, Some(MH_PAGE)))
            .with_row(row("FS25_Pageonly", "2", "2.0", None, Some(GH_REPO))),
        FakeWeb::new(),
    );
    for name in ["Current", "Pageonly", "Mystery"] {
        t.put(&t.a, &format!("FS25_{name}.zip"), "1.0");
    }
    t.enable();
    t.check(false).unwrap();
    let error = |name: &str| t.plan(name).unwrap_err();
    let unknown = error("FS25_Nowhere");
    assert_eq!(unknown.code, ErrorCode::FolderMissing);
    let mystery = error("FS25_Mystery");
    assert_eq!(
        (mystery.code, mystery.detail.as_deref()),
        (ErrorCode::NotInstallable, Some("noMatch"))
    );
    let current = error("FS25_Current");
    assert_eq!(
        (current.code, current.detail.as_deref()),
        (ErrorCode::NotInstallable, Some("upToDate"))
    );
    let pageonly = error("FS25_Pageonly");
    assert_eq!(
        (pageonly.code, pageonly.detail.as_deref()),
        (ErrorCode::NotInstallable, Some("noDirectDownload"))
    );
    assert_eq!(t.plan("  ").unwrap_err().code, ErrorCode::Internal);
}

// ---------------------------------------------------------------- installing from GitHub

#[test]
fn a_github_update_replaces_the_old_zip_through_the_recycle_bin_and_is_journaled() {
    let (t, new) = github_world();
    let old_bytes = fs::read(t.a.join("FS25_Courseplay.zip")).unwrap();
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(result.done, vec![t.pa.clone()]);
    assert!(result.failed.is_empty() && result.skipped.is_empty());

    assert_eq!(fs::read(t.a.join("FS25_Courseplay.zip")).unwrap(), new);
    assert_eq!(
        names(&t.a),
        ["FS25_Courseplay.zip"],
        "no staged file is left"
    );
    assert_eq!(t.recycler.binned(), ["FS25_Courseplay.zip"]);
    assert_eq!(fs::read(&t.recycler.binned_files()[0]).unwrap(), old_bytes);

    // GitHub: one detail, one download, no page.
    assert_eq!(t.catalog.details(), ["cp"]);
    assert_eq!(t.web.downloads(), [GH_URL]);
    assert_eq!(
        t.web.download_referers(),
        [None],
        "GitHub needs no referring page"
    );
    assert!(t.web.pages_fetched().is_empty());

    // The journal holds the recycle of the old zip and the update, paths and codes only.
    let journal = t.journal();
    assert_eq!(kinds(&journal), [("recycle", "ok"), ("update", "ok")]);
    assert_eq!(journal[1].source, t.a.join("FS25_Courseplay.zip"));
    assert_eq!(
        journal[1].target.as_deref(),
        Some(t.a.join("FS25_Courseplay.zip").as_path())
    );
    assert_eq!(journal[0].plan_id, journal[1].plan_id);
    assert!(journal
        .iter()
        .all(|r| !r.source.to_string_lossy().contains("http")));

    // The listing is fresh: the new version, and nothing left to update.
    let entry = t.entry(&t.pa, COURSEPLAY);
    assert_eq!(entry.version, "2.0.0.0");
    assert_eq!(entry.update, UpdateState::UpToDate);
}

#[test]
fn progress_is_reported_from_zero_to_done() {
    let (t, new) = github_world();
    t.install(COURSEPLAY, &[&t.pa]).unwrap();
    let events = t.events.lock().unwrap();
    let first = events.first().unwrap();
    assert_eq!(
        (first.done_bytes, first.done_items, first.total_items),
        (0, 0, 1)
    );
    let last = events.last().unwrap();
    assert_eq!(
        (last.done_items, last.total_items, last.current.clone()),
        (1, 1, None)
    );
    assert!(events
        .windows(2)
        .all(|w| w[0].done_bytes <= w[1].done_bytes));
    assert!(last.done_bytes >= last.total_bytes.min(new.len() as u64));
}

#[test]
fn the_catalog_hash_is_checked_only_for_the_version_it_belongs_to() {
    // The catalog's hash is for 2.1; the GitHub asset is 2.0, so the hash does not apply.
    let new = bytes_of("2.0.0.0", 100);
    let t = T::with(
        FakeCatalog::new()
            .with_row(courseplay_row("v2.0.0.0"))
            .with_detail(github_detail(
                "v2.1.0.0",
                "v2.0.0.0",
                Some(&sha_hex(b"for 2.1")),
                GH_URL,
            )),
        FakeWeb::new().with_payload(GH_URL, new),
    );
    t.put(&t.a, "FS25_Courseplay.zip", "1.0");
    t.enable();
    t.check(false).unwrap();
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(result.done, vec![t.pa.clone()]);
}

#[test]
fn a_wrong_hash_fails_the_install_and_leaves_the_old_mod() {
    let new = bytes_of("2.0.0.0", 100);
    let t = T::with(
        FakeCatalog::new()
            .with_row(courseplay_row("v2.0.0.0"))
            .with_detail(github_detail(
                "v2.0.0.0",
                "v2.0.0.0",
                Some(&sha_hex(b"other")),
                GH_URL,
            )),
        FakeWeb::new().with_payload(GH_URL, new),
    );
    let old = t.put(&t.a, "FS25_Courseplay.zip", "1.0.0.0");
    let old_bytes = fs::read(&old).unwrap();
    t.enable();
    t.check(false).unwrap();
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert!(result.done.is_empty());
    assert_eq!(failed(&result), [(t.pa.clone(), ErrorCode::VerifyFailed)]);
    assert_eq!(fs::read(&old).unwrap(), old_bytes);
    assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
    assert!(t.recycler.calls().is_empty());
    assert_eq!(kinds(&t.journal()), [("update", "verifyFailed")]);
}

#[test]
fn a_download_older_than_promised_or_not_a_zip_fails_verification() {
    for payload in [
        bytes_of("1.5.0.0", 100),
        b"<html>an error page</html>".to_vec(),
        Vec::new(),
    ] {
        let t = T::with(
            FakeCatalog::new()
                .with_row(courseplay_row("v2.0.0.0"))
                .with_detail(github_detail("v2.0.0.0", "v2.0.0.0", None, GH_URL)),
            FakeWeb::new().with_payload(GH_URL, payload),
        );
        let old = t.put(&t.a, "FS25_Courseplay.zip", "1.0.0.0");
        let old_bytes = fs::read(&old).unwrap();
        t.enable();
        t.check(false).unwrap();
        let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
        assert_eq!(failed(&result), [(t.pa.clone(), ErrorCode::VerifyFailed)]);
        assert_eq!(fs::read(&old).unwrap(), old_bytes);
        assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
        assert!(t.recycler.calls().is_empty());
    }
}

#[test]
fn an_asset_with_another_file_name_is_never_installed() {
    for url in [
        "https://github.com/o/r/releases/download/v2/FS25_Courseplay_v2.zip",
        "https://github.com/o/r/releases/download/v2/FS25_CoursePlayX.zip",
        "https://github.com/o/r/releases/download/v2/",
    ] {
        let t = T::with(
            FakeCatalog::new()
                .with_row(courseplay_row("v2.0.0.0"))
                .with_detail(github_detail("v2.0.0.0", "v2.0.0.0", None, url)),
            FakeWeb::new().with_payload(url, bytes_of("2.0.0.0", 10)),
        );
        let old = t.put(&t.a, "FS25_Courseplay.zip", "1.0.0.0");
        t.enable();
        t.check(false).unwrap();
        let error = t.install(COURSEPLAY, &[&t.pa]).unwrap_err();
        assert_eq!(error.code, ErrorCode::NotInstallable, "{url}");
        assert_eq!(error.detail.as_deref(), Some("fileName"));
        assert!(t.web.downloads().is_empty(), "nothing was downloaded");
        assert!(old.exists() && t.journal().is_empty());
    }
}

#[test]
fn the_file_name_is_compared_ignoring_case_and_percent_encoding() {
    for url in [
        "https://github.com/o/r/releases/download/v2/fs25_courseplay.ZIP",
        "https://github.com/o/r/releases/download/v2/FS25%5FCourseplay.zip",
    ] {
        let new = bytes_of("2.0.0.0", 10);
        let t = T::with(
            FakeCatalog::new()
                .with_row(courseplay_row("v2.0.0.0"))
                .with_detail(github_detail("v2.0.0.0", "v2.0.0.0", None, url)),
            FakeWeb::new().with_payload(url, new.clone()),
        );
        t.put(&t.a, "FS25_Courseplay.zip", "1.0.0.0");
        t.enable();
        t.check(false).unwrap();
        let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
        assert_eq!(result.done, vec![t.pa.clone()], "{url}");
        // The installed name stays the one the folder had.
        assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
    }
}

#[test]
fn a_download_address_off_the_list_is_refused_before_any_request() {
    let url = "https://github.com.evil.example/FS25_Courseplay.zip";
    let t = T::with(
        FakeCatalog::new()
            .with_row(courseplay_row("v2.0.0.0"))
            .with_detail(github_detail("v2.0.0.0", "v2.0.0.0", None, url)),
        FakeWeb::new().with_payload(url, bytes_of("2.0.0.0", 10)),
    );
    t.put(&t.a, "FS25_Courseplay.zip", "1.0.0.0");
    t.enable();
    t.check(false).unwrap();
    let error = t.install(COURSEPLAY, &[&t.pa]).unwrap_err();
    assert_eq!(error.code, ErrorCode::NotInstallable);
    assert!(t.web.calls().is_empty());
}

#[test]
fn the_catalog_dropping_its_github_asset_means_not_installable() {
    let mut detail = github_detail("v2.0.0.0", "v2.0.0.0", None, GH_URL);
    detail.sources.retain(|s| s.source != "github");
    let t = T::with(
        FakeCatalog::new()
            .with_row(courseplay_row("v2.0.0.0"))
            .with_detail(detail),
        FakeWeb::new(),
    );
    t.put(&t.a, "FS25_Courseplay.zip", "1.0.0.0");
    t.enable();
    t.check(false).unwrap();
    let error = t.install(COURSEPLAY, &[&t.pa]).unwrap_err();
    assert_eq!(error.code, ErrorCode::NotInstallable);
    // An id the catalog no longer knows is the same.
    let t = T::with(
        FakeCatalog::new().with_row(courseplay_row("v2.0.0.0")),
        FakeWeb::new(),
    );
    t.put(&t.a, "FS25_Courseplay.zip", "1.0.0.0");
    t.enable();
    t.check(false).unwrap();
    assert_eq!(
        t.install(COURSEPLAY, &[&t.pa]).unwrap_err().code,
        ErrorCode::NotInstallable
    );
}

// ---------------------------------------------------------------- installing from ModHub

fn modhub_world(download: Vec<u8>) -> T {
    modhub_world_with_page(PAGE, download)
}

fn modhub_world_with_page(html: &str, download: Vec<u8>) -> T {
    let catalog =
        FakeCatalog::new().with_row(row(COURSEPLAY, "cp", "8.1.0.3", None, Some(MH_PAGE)));
    let web = FakeWeb::new()
        .with_chunk(64 * 1024)
        .with_page(MH_PAGE, html)
        .with_payload(MH_ZIP, download);
    let t = T::with(catalog, web);
    t.put(&t.a, "FS25_Courseplay.zip", "8.0.0.0");
    t.enable();
    t.check(false).unwrap();
    t
}

/// A mod zip of about 3.79 MB, which is what the saved page promises.
fn modhub_file(version: &str) -> Vec<u8> {
    let overhead = bytes_of(version, 0).len();
    bytes_of(version, 3_974_103 - overhead)
}

#[test]
fn a_modhub_update_goes_through_the_page_link() {
    let file = modhub_file("8.1.0.3");
    assert_eq!(file.len(), 3_974_103, "exactly what the page says");
    let t = modhub_world(file.clone());
    assert_eq!(t.plan(COURSEPLAY).unwrap().source, LinkKind::ModHub);
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(result.done, vec![t.pa.clone()]);
    assert_eq!(fs::read(t.a.join("FS25_Courseplay.zip")).unwrap(), file);
    assert_eq!(t.web.pages_fetched(), [MH_PAGE]);
    assert_eq!(t.web.downloads(), [MH_ZIP]);
    // ModHub's CDN serves a file only to its page, as a click on DOWNLOAD there would.
    assert_eq!(t.web.download_referers(), [Some(MH_PAGE.to_string())]);
    assert!(
        t.catalog.details().is_empty(),
        "ModHub needs no catalog detail"
    );
    assert_eq!(kinds(&t.journal()), [("recycle", "ok"), ("update", "ok")]);
    assert_eq!(t.recycler.binned(), ["FS25_Courseplay.zip"]);
}

#[test]
fn the_modhub_size_must_match_the_page() {
    // Far smaller than the 3.79 MB the page shows.
    let t = modhub_world(bytes_of("8.1.0.3", 1000));
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(failed(&result), [(t.pa.clone(), ErrorCode::VerifyFailed)]);
    assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
    assert!(t.recycler.calls().is_empty());
}

#[test]
fn the_modhub_version_must_be_what_the_page_says() {
    let t = modhub_world(modhub_file("8.0.5.0"));
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(failed(&result), [(t.pa.clone(), ErrorCode::VerifyFailed)]);
    assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
}

#[test]
fn a_modhub_page_without_a_usable_link_falls_back_to_not_installable() {
    for html in [
        "<html>no link at all</html>".to_string(),
        PAGE.replace("00331515", "00999999"),
        PAGE.replace(
            "cdn27.giants-software.com",
            "cdn27.giants-software.com.evil.example",
        ),
        PAGE.replace(
            "FS25_Courseplay.zip\" class",
            "FS25_Courseplay_Other.zip\" class",
        ),
    ] {
        let t = modhub_world_with_page(&html, modhub_file("8.1.0.3"));
        let error = t.install(COURSEPLAY, &[&t.pa]).unwrap_err();
        assert_eq!(error.code, ErrorCode::NotInstallable);
        assert!(t.web.downloads().is_empty());
        assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
        assert!(t.journal().is_empty());
    }
}

#[test]
fn a_modhub_page_that_cannot_be_fetched_fails_the_call_with_the_reason() {
    let t = modhub_world(modhub_file("8.1.0.3"));
    for (error, code) in [
        (NetError::Offline, ErrorCode::Offline),
        (NetError::Status(500), ErrorCode::Offline),
        (NetError::Status(404), ErrorCode::NotInstallable),
        (
            NetError::RateLimited { retry_after: None },
            ErrorCode::RateLimited,
        ),
    ] {
        t.web.fail_next(error);
        assert_eq!(t.install(COURSEPLAY, &[&t.pa]).unwrap_err().code, code);
    }
    assert!(t.web.downloads().is_empty());
    assert!(t.recycler.calls().is_empty());
}

// ---------------------------------------------------------------- failures and guards

#[test]
fn cancelling_mid_download_removes_the_partial_file_and_keeps_the_old_mod() {
    let (t, _) = github_world();
    let old_bytes = fs::read(t.a.join("FS25_Courseplay.zip")).unwrap();
    t.web.cancel_after_chunks(2);
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(failed(&result), [(t.pa.clone(), ErrorCode::Cancelled)]);
    assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
    assert_eq!(
        fs::read(t.a.join("FS25_Courseplay.zip")).unwrap(),
        old_bytes
    );
    assert!(t.recycler.calls().is_empty());
    assert_eq!(kinds(&t.journal()), [("update", "cancelled")]);
}

#[test]
fn a_cancel_stops_the_folders_not_done_yet() {
    let (t, _) = github_world();
    t.put(&t.b, "FS25_Courseplay.zip", "1.0.0.0");
    t.web.cancel_after_chunks(1);
    let result = t.install(COURSEPLAY, &[&t.pa, &t.pb]).unwrap();
    assert_eq!(
        failed(&result),
        [
            (t.pa.clone(), ErrorCode::Cancelled),
            (t.pb.clone(), ErrorCode::Cancelled)
        ]
    );
    assert_eq!(t.entry(&t.pb, COURSEPLAY).version, "1.0.0.0");
    assert_eq!(names(&t.b), ["FS25_Courseplay.zip"]);
}

#[test]
fn a_dropped_connection_fails_the_install_and_cleans_up() {
    let (t, _) = github_world();
    t.web.fail_mid_download(NetError::Offline);
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(failed(&result), [(t.pa.clone(), ErrorCode::Offline)]);
    assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
    assert!(t.recycler.calls().is_empty());
    assert_eq!(t.entry(&t.pa, COURSEPLAY).version, "1.0.0.0");
}

#[test]
fn a_download_that_the_server_refuses_fails_the_install() {
    let (t, _) = github_world();
    t.web.fail_next(NetError::Status(500));
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(failed(&result), [(t.pa.clone(), ErrorCode::Offline)]);
    // A file that is gone is not an outage: its page can still be opened.
    t.web.fail_next(NetError::Status(404));
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(failed(&result), [(t.pa.clone(), ErrorCode::NotInstallable)]);
    t.web.fail_next(NetError::NotAllowed);
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(failed(&result), [(t.pa.clone(), ErrorCode::NotInstallable)]);
    assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
}

#[test]
fn the_game_running_refuses_the_install_before_anything_is_asked() {
    let (t, _) = github_world();
    let requests = t.catalog.request_count();
    t.game.store(true, Ordering::SeqCst);
    let error = t.install(COURSEPLAY, &[&t.pa]).unwrap_err();
    assert_eq!(error.code, ErrorCode::GameRunning);
    assert_eq!(t.catalog.request_count(), requests);
    assert!(t.web.calls().is_empty());
    assert!(t.journal().is_empty());
}

#[test]
fn the_game_starting_during_the_download_keeps_the_old_mod() {
    let (t, _) = github_world();
    let old_bytes = fs::read(t.a.join("FS25_Courseplay.zip")).unwrap();
    // The first check (before the download) passes, the one before the replace does not.
    t.game_from_call.store(1, Ordering::SeqCst);
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(failed(&result), [(t.pa.clone(), ErrorCode::GameRunning)]);
    assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
    assert_eq!(
        fs::read(t.a.join("FS25_Courseplay.zip")).unwrap(),
        old_bytes
    );
    assert!(t.recycler.calls().is_empty());
}

#[test]
fn a_refused_recycle_keeps_the_old_mod_and_fails_the_folder() {
    let (t, _) = github_world();
    t.recycler.refuse(&t.a.join("FS25_Courseplay.zip"));
    let old_bytes = fs::read(t.a.join("FS25_Courseplay.zip")).unwrap();
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(
        failed(&result),
        [(t.pa.clone(), ErrorCode::RecycleUnavailable)]
    );
    assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
    assert_eq!(
        fs::read(t.a.join("FS25_Courseplay.zip")).unwrap(),
        old_bytes
    );
    assert_eq!(
        kinds(&t.journal()),
        [
            ("recycle", "recycleUnavailable"),
            ("update", "recycleUnavailable")
        ]
    );
}

#[test]
fn a_mod_that_changed_during_the_download_is_not_replaced() {
    let (t, _) = github_world();
    let old = t.a.join("FS25_Courseplay.zip");
    let changed = bytes_of("1.0.0.0", 5000);
    *t.on_progress.lock().unwrap() = Some(Box::new({
        let old = old.clone();
        let changed = changed.clone();
        move |event| {
            // The first event is sent after the download is resolved and before it starts.
            if event.done_items == 0 {
                fs::write(&old, &changed).unwrap();
            }
        }
    }));
    let result = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert_eq!(failed(&result), [(t.pa.clone(), ErrorCode::TargetChanged)]);
    assert_eq!(
        fs::read(&old).unwrap(),
        changed,
        "the user's file is untouched"
    );
    assert!(t.recycler.calls().is_empty());
    assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
}

#[test]
fn problems_resolving_the_download_fail_the_call_before_any_file_is_touched() {
    let (t, _) = github_world();
    for (error, code) in [
        (NetError::Offline, ErrorCode::Offline),
        (NetError::Status(502), ErrorCode::Offline),
    ] {
        t.catalog.fail_next(error);
        assert_eq!(t.install(COURSEPLAY, &[&t.pa]).unwrap_err().code, code);
    }
    assert!(t.web.calls().is_empty());
    assert!(t.journal().is_empty());
    assert_eq!(names(&t.a), ["FS25_Courseplay.zip"]);
}

#[test]
fn a_rate_limit_on_the_catalog_during_an_install_is_remembered() {
    let (t, _) = github_world();
    let until = t.now() + Duration::from_secs(5 * 3600);
    t.catalog.fail_next(NetError::RateLimited {
        retry_after: Some(until),
    });
    let error = t.install(COURSEPLAY, &[&t.pa]).unwrap_err();
    assert_eq!(error.code, ErrorCode::RateLimited);
    assert_eq!(
        t.lib.update_settings().unwrap().retry_after.as_deref(),
        Some(iso(until).as_str())
    );
    // Until then the catalog is not asked again, by a check or by an install.
    let requests = t.catalog.request_count();
    assert_eq!(
        t.install(COURSEPLAY, &[&t.pa]).unwrap_err().code,
        ErrorCode::RateLimited
    );
    t.advance(4 * 3600);
    assert_eq!(t.check(false).unwrap_err().code, ErrorCode::RateLimited);
    assert_eq!(t.catalog.request_count(), requests);
}

#[test]
fn an_unmatched_name_cannot_be_installed() {
    let (t, _) = github_world();
    t.put(&t.a, "FS25_Mystery.zip", "1.0");
    assert_eq!(
        t.install("FS25_Mystery", &[&t.pa]).unwrap_err().code,
        ErrorCode::NotInstallable
    );
    assert_eq!(
        t.install("FS25_Nowhere", &[&t.pa]).unwrap_err().code,
        ErrorCode::NotInstallable
    );
    assert_eq!(
        t.install("  ", &[&t.pa]).unwrap_err().code,
        ErrorCode::Internal
    );
}

#[test]
fn an_install_never_overlaps_another_file_operation_or_request() {
    let (t, _) = github_world();
    {
        let _busy = BusyGuard::acquire(&t.lib.transfer.busy).unwrap();
        assert_eq!(
            t.install(COURSEPLAY, &[&t.pa]).unwrap_err().code,
            ErrorCode::Busy
        );
    }
    {
        let _net = BusyGuard::acquire(&t.lib.updates.busy).unwrap();
        assert_eq!(
            t.install(COURSEPLAY, &[&t.pa]).unwrap_err().code,
            ErrorCode::Busy
        );
        t.advance(4 * 3600);
        assert_eq!(t.check(false).unwrap_err().code, ErrorCode::Busy);
    }
    // Both flags are released when a call ends, whatever its outcome.
    assert!(t.install(COURSEPLAY, &[&t.pa]).is_ok());
    t.advance(4 * 3600);
    assert!(t.check(false).is_ok());
}

#[test]
fn the_busy_flags_are_released_after_a_failed_install() {
    let (t, _) = github_world();
    t.web.fail_next(NetError::Offline);
    let _ = t.install(COURSEPLAY, &[&t.pa]).unwrap();
    assert!(t.install(COURSEPLAY, &[&t.pa]).is_ok());
}

#[test]
fn a_full_disk_stops_the_install_before_the_download() {
    let (t, _) = github_world();
    t.disk.set(10);
    let error = t.install(COURSEPLAY, &[&t.pa]).unwrap_err();
    assert_eq!(error.code, ErrorCode::NotEnoughSpace);
    assert!(t.web.downloads().is_empty());
}

// ---------------------------------------------------------------- several folders

fn two_folder_world() -> (T, Vec<u8>) {
    let (t, new) = github_world();
    t.put(&t.b, "FS25_Courseplay.zip", "1.2.0.0");
    (t, new)
}

#[test]
fn several_folders_are_filled_from_one_download() {
    let (t, new) = two_folder_world();
    let result = t.install(COURSEPLAY, &[&t.pa, &t.pb]).unwrap();
    assert_eq!(result.done, [t.pa.clone(), t.pb.clone()]);
    assert!(result.failed.is_empty());
    assert_eq!(t.web.downloads(), [GH_URL], "downloaded once");
    for dir in [&t.a, &t.b] {
        assert_eq!(fs::read(dir.join("FS25_Courseplay.zip")).unwrap(), new);
        assert_eq!(names(dir), ["FS25_Courseplay.zip"]);
    }
    assert_eq!(
        t.recycler.binned(),
        ["FS25_Courseplay.zip", "FS25_Courseplay.zip"]
    );
    // One recycle and one update row per folder, in order.
    assert_eq!(
        kinds(&t.journal()),
        [
            ("recycle", "ok"),
            ("update", "ok"),
            ("recycle", "ok"),
            ("update", "ok")
        ]
    );
    let journal = t.journal();
    assert_eq!(journal[1].source, t.a.join("FS25_Courseplay.zip"));
    assert_eq!(journal[3].source, t.b.join("FS25_Courseplay.zip"));
    let plan_ids: HashSet<_> = journal.iter().map(|r| r.plan_id.clone()).collect();
    assert_eq!(plan_ids.len(), 1);
    assert_eq!(t.entry(&t.pb, COURSEPLAY).version, "2.0.0.0");
}

#[test]
fn only_the_ticked_folders_are_updated_and_a_repeated_one_counts_once() {
    let (t, new) = two_folder_world();
    let result = t.install(COURSEPLAY, &[&t.pb, &t.pb]).unwrap();
    assert_eq!(result.done, vec![t.pb.clone()]);
    assert_eq!(fs::read(t.b.join("FS25_Courseplay.zip")).unwrap(), new);
    assert_eq!(
        t.entry(&t.pa, COURSEPLAY).version,
        "1.0.0.0",
        "the other folder is untouched"
    );
    assert_eq!(t.recycler.binned().len(), 1);
}

#[test]
fn a_folder_that_refuses_to_recycle_fails_alone() {
    let (t, new) = two_folder_world();
    t.recycler.refuse(&t.b.join("FS25_Courseplay.zip"));
    let old_b = fs::read(t.b.join("FS25_Courseplay.zip")).unwrap();
    let result = t.install(COURSEPLAY, &[&t.pa, &t.pb]).unwrap();
    assert_eq!(result.done, vec![t.pa.clone()]);
    assert_eq!(
        failed(&result),
        [(t.pb.clone(), ErrorCode::RecycleUnavailable)]
    );
    assert_eq!(fs::read(t.a.join("FS25_Courseplay.zip")).unwrap(), new);
    assert_eq!(fs::read(t.b.join("FS25_Courseplay.zip")).unwrap(), old_b);
    assert_eq!(names(&t.b), ["FS25_Courseplay.zip"], "no staged file");
}

#[test]
fn a_failed_first_folder_fails_the_others_that_needed_its_file() {
    let (t, _) = two_folder_world();
    t.recycler.refuse(&t.a.join("FS25_Courseplay.zip"));
    let result = t.install(COURSEPLAY, &[&t.pa, &t.pb]).unwrap();
    assert!(result.done.is_empty());
    assert_eq!(
        failed(&result),
        [
            (t.pa.clone(), ErrorCode::RecycleUnavailable),
            (t.pb.clone(), ErrorCode::RecycleUnavailable)
        ]
    );
    assert_eq!(t.entry(&t.pb, COURSEPLAY).version, "1.2.0.0");
}

#[test]
fn folders_without_the_mod_or_holding_it_unpacked_fail_by_themselves() {
    let (t, new) = github_world();
    fs::create_dir_all(t.b.parent().unwrap().join("c")).unwrap();
    let pc = t
        .lib
        .add_profile(&t.b.parent().unwrap().join("c"), &t.ctx)
        .unwrap()
        .id;
    write_folder(
        &t.b.join("FS25_Courseplay"),
        &[(
            "modDesc.xml",
            crate::library::fixtures::desc("1.0").as_bytes(),
        )],
    );
    let result = t.install(COURSEPLAY, &[&pc, &t.pb, "p999", &t.pa]).unwrap();
    assert_eq!(result.done, vec![t.pa.clone()]);
    assert_eq!(
        failed(&result),
        [
            (pc.clone(), ErrorCode::FolderMissing),
            (t.pb.clone(), ErrorCode::NotInstallable),
            ("p999".to_string(), ErrorCode::ProfileNotFound),
        ]
    );
    assert_eq!(fs::read(t.a.join("FS25_Courseplay.zip")).unwrap(), new);
}

#[test]
fn asking_for_no_folders_or_only_bad_ones_downloads_nothing() {
    let (t, _) = github_world();
    let requests = t.catalog.request_count();
    assert_eq!(t.install(COURSEPLAY, &[]).unwrap(), OpResult::default());
    let result = t.install(COURSEPLAY, &[&t.pb]).unwrap();
    assert_eq!(failed(&result), [(t.pb.clone(), ErrorCode::FolderMissing)]);
    assert_eq!(t.catalog.request_count(), requests);
    assert!(t.web.calls().is_empty());
}

#[test]
fn a_manual_link_and_tags_follow_the_mod_through_an_update() {
    let (t, _) = github_world();
    t.lib.set_tags(COURSEPLAY, &ids(&["farming"])).unwrap();
    manual(&t, COURSEPLAY, "https://forum.example.com/cp");
    t.install(COURSEPLAY, &[&t.pa]).unwrap();
    let entry = t.entry(&t.pa, COURSEPLAY);
    assert_eq!(entry.tags, ["farming"]);
    assert_eq!(entry.links.len(), 3);
}

// ---------------------------------------------------------------- manual links

#[test]
fn a_manual_link_is_validated_stored_and_returns_its_host() {
    let t = T::new();
    for (input, host) in [
        (
            "https://www.kingmods.net/en/fs25/mods/1",
            "www.kingmods.net",
        ),
        ("http://Example.COM/path?x=1#frag", "example.com"),
        ("  https://itch.io/mod  ", "itch.io"),
        ("http://localhost:8080/mods", "localhost"),
        ("HTTPS://forum.example.org", "forum.example.org"),
        ("https://xn--bcher-kva.example/", "xn--bcher-kva.example"),
        ("https://example.com/path with%20encoded", ""),
    ] {
        let result = t.lib.set_source_link("FS25_Local", Some(input));
        if host.is_empty() {
            assert_eq!(result.unwrap_err().code, ErrorCode::InvalidLink, "{input}");
        } else {
            assert_eq!(result.unwrap().as_deref(), Some(host), "{input}");
        }
    }
    // The stored link is the trimmed text.
    t.lib
        .set_source_link("FS25_Local", Some("  https://itch.io/mod  "))
        .unwrap();
    assert_eq!(
        t.lib.link_url("FS25_Local", LinkKind::Manual).unwrap(),
        "https://itch.io/mod"
    );
}

#[test]
fn bad_links_are_invalid_and_leave_the_stored_one_alone() {
    let t = T::new();
    t.lib
        .set_source_link("FS25_Local", Some("https://good.example/a"))
        .unwrap();
    let too_long = format!("https://example.com/{}", "a".repeat(2000));
    for bad in [
        "",
        "   ",
        "example.com",
        "ftp://example.com/x",
        "javascript:alert(1)",
        "file:///c:/windows/system32",
        "data:text/html,hi",
        "https://",
        "https:///path",
        "https://user@example.com/",
        "https://user:pw@example.com/",
        "https://example.com:/",
        "https://example.com:123456/",
        "https://exa mple.com/",
        "https://example.com\\evil",
        "https://example.com/\nx",
        "https://.example.com/",
        "https://exam_ple.com/",
        "//example.com/x",
        "mailto:someone@example.com",
        too_long.as_str(),
    ] {
        let error = t.lib.set_source_link("FS25_Local", Some(bad)).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidLink, "{bad:?}");
    }
    assert_eq!(
        t.lib.link_url("FS25_Local", LinkKind::Manual).unwrap(),
        "https://good.example/a"
    );
}

#[test]
fn a_link_of_exactly_two_thousand_characters_is_accepted() {
    let t = T::new();
    let url = format!(
        "https://example.com/{}",
        "a".repeat(2000 - "https://example.com/".len())
    );
    assert_eq!(url.chars().count(), 2000);
    assert_eq!(
        t.lib
            .set_source_link("FS25_Local", Some(&url))
            .unwrap()
            .as_deref(),
        Some("example.com")
    );
}

#[test]
fn a_link_is_stored_per_lowercased_name_and_none_clears_it() {
    let t = T::new();
    t.put(&t.a, "FS25_Local.zip", "1.0");
    t.lib
        .set_source_link("FS25_LOCAL", Some("https://itch.io/x"))
        .unwrap();
    assert_eq!(
        t.lib.link_url("fs25_local", LinkKind::Manual).unwrap(),
        "https://itch.io/x"
    );
    assert_eq!(t.entry(&t.pa, "FS25_Local").links.len(), 1);
    assert_eq!(t.lib.set_source_link("fs25_Local", None).unwrap(), None);
    assert_eq!(
        t.lib
            .link_url("FS25_Local", LinkKind::Manual)
            .unwrap_err()
            .code,
        ErrorCode::FolderMissing
    );
    assert!(t.entry(&t.pa, "FS25_Local").links.is_empty());
    assert!(
        t.lib.store.all_mod_meta().unwrap().is_empty(),
        "the row is gone"
    );
    // Clearing what was never set is fine; a blank name is not.
    assert_eq!(t.lib.set_source_link("FS25_Nothing", None).unwrap(), None);
    assert_eq!(
        t.lib.set_source_link(" ", None).unwrap_err().code,
        ErrorCode::Internal
    );
}

#[test]
fn a_link_only_row_survives_tag_and_note_edits_and_rescans() {
    let t = T::new();
    t.put(&t.a, "FS25_Local.zip", "1.0");
    t.lib
        .set_source_link("FS25_Local", Some("https://itch.io/x"))
        .unwrap();
    t.lib.set_tags("FS25_Local", &ids(&["a"])).unwrap();
    t.lib.set_tags("FS25_Local", &[]).unwrap();
    t.lib.set_note("FS25_Local", "note").unwrap();
    t.lib.set_note("FS25_Local", "").unwrap();
    assert_eq!(t.lib.store.all_mod_meta().unwrap().len(), 1);
    t.lib.forget_scan(&t.pa);
    fs::write(t.a.join("FS25_Local.zip"), bytes_of("1.1", 10)).unwrap();
    let entry = t.entry(&t.pa, "FS25_Local");
    assert_eq!(entry.version, "1.1");
    assert_eq!(entry.links.len(), 1);
    assert!(entry.tags.is_empty() && entry.note.is_none());
}

#[test]
fn links_open_only_http_and_https_and_only_what_is_known() {
    let (t, _) = github_world();
    assert_eq!(
        t.lib.link_url(COURSEPLAY, LinkKind::ModHub).unwrap(),
        MH_PAGE
    );
    assert_eq!(
        t.lib.link_url("fs25_courseplay", LinkKind::GitHub).unwrap(),
        GH_REPO
    );
    assert_eq!(
        t.lib
            .link_url(COURSEPLAY, LinkKind::Manual)
            .unwrap_err()
            .code,
        ErrorCode::FolderMissing
    );
    assert_eq!(
        t.lib
            .link_url("FS25_Nope", LinkKind::ModHub)
            .unwrap_err()
            .code,
        ErrorCode::FolderMissing
    );
    // A value that is not an http(s) link, however it got there, is not opened.
    for bad in [
        "javascript:alert(1)",
        "file:///c:/x",
        "ftp://example.com/x",
        "ms-settings:",
    ] {
        t.lib
            .store
            .set_mod_source_url("fs25_courseplay", Some(bad))
            .unwrap();
        assert_eq!(
            t.lib
                .link_url(COURSEPLAY, LinkKind::Manual)
                .unwrap_err()
                .code,
            ErrorCode::FolderMissing,
            "{bad}"
        );
        // And it is not shown as a link either.
        assert!(t
            .entry(&t.pa, COURSEPLAY)
            .links
            .iter()
            .all(|l| l.kind != LinkKind::Manual));
    }
}

#[test]
fn a_manual_link_alone_never_causes_a_request() {
    let t = T::new();
    t.put(&t.a, "FS25_Local.zip", "1.0");
    t.lib
        .set_source_link("FS25_Local", Some("https://itch.io/x"))
        .unwrap();
    t.entry(&t.pa, "FS25_Local");
    t.lib.list_profiles(&t.ctx).unwrap();
    assert_eq!(t.catalog.request_count(), 0);
    assert!(t.web.calls().is_empty());
}

// ---------------------------------------------------------------- pure helpers

#[test]
fn a_catalog_row_is_kept_with_validated_pages_only() {
    let github = to_match("k", &courseplay_row("v2"), MatchedBy::Name, "t");
    assert_eq!(github.modhub_url.as_deref(), Some(MH_PAGE));
    assert_eq!(github.github_url.as_deref(), Some(GH_REPO));
    assert_eq!(github.download_source.as_deref(), Some("github"));
    assert_eq!(github.download_url.as_deref(), Some(GH_URL));

    // A page that is not a ModHub page (or has no id) is not kept as one.
    let odd = to_match(
        "k",
        &row(
            "X",
            "1",
            "1",
            None,
            Some("https://www.farming-simulator.com/mod.php"),
        ),
        MatchedBy::Hash,
        "t",
    );
    assert_eq!((odd.modhub_url, odd.github_url), (None, None));
    let evil = to_match(
        "k",
        &row(
            "X",
            "1",
            "1",
            None,
            Some("https://evil.example/mod.php?mod_id=1"),
        ),
        MatchedBy::Hash,
        "t",
    );
    assert_eq!(evil.modhub_url, None);

    // A ModHub download whose address is the page itself.
    let from_download = to_match(
        "k",
        &row("X", "1", "1", Some(("modhub", MH_PAGE)), None),
        MatchedBy::Name,
        "t",
    );
    assert_eq!(from_download.modhub_url.as_deref(), Some(MH_PAGE));
}
