//! Mod updates (plan M7): checking the SiloAPI catalog, installing GitHub and ModHub updates
//! through the M5 write path, the user's manual source links and the online setting.
//!
//! Nothing here goes online unless `updates.enabled` is set; every request goes through
//! `UpdateEnv::catalog` / `UpdateEnv::web` (`crate::net`).
//!
//! **Settings** live in `library.sqlite` (`app_settings`): `updates.enabled` (`true`/`false`, off
//! by default), `updates.lastCheck` and `updates.retryAfter` (RFC 3339), and `updates.checked`
//! (how many names the last check looked up, for the cached answer).
//!
//! **Check results** live in `cache.sqlite` (`catalog_matches`), one row per lower-cased local mod
//! name, replaced as a whole by each successful check. They are never part of the scan cache:
//! `source`, `links` and `update` of a `ModEntry` are overlaid on every listing from these rows and
//! the user's links, so a change in either shows at once and turning updates off forgets nothing.
//!
//! **One request at a time.** `check_updates` and `install_update` share one flag
//! (`NetState::busy`), so two of them never overlap. `install_update` also takes the M5 busy flag
//! and uses the M5 cancel flag, so it never overlaps a copy, move or remove.
//!
//! **Journal.** Each folder an update was attempted in is one `update` row in `file_ops` (source:
//! the mod that was replaced, target: the new file), plus the `recycle` row of the old zip, as for
//! a replacing copy. Paths and codes only, never URLs.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rayon::prelude::*;

use super::meta::name_key;
use super::switching::code_name;
use super::transfer::{
    io_api, recycle_code, running_code, stops_operation, unchanged, BusyGuard, Tracker,
};
use super::{no_progress, Library, OpEnv, Scan};
use crate::discovery::GameContext;
use crate::files::{self, DownloadCheck, FileOpError};
use crate::model::{
    ApiError, ErrorCode, IssueKind, ItemFailure, LinkKind, ModEntry, ModFormat, ModSource,
    OpProgress, OpResult, SourceLink, UpdateCheck, UpdateFolder, UpdatePlan, UpdateSettings,
    UpdateState,
};
use crate::net::{self, Catalog, LookupResult, NetError, Web};
use crate::ops;
use crate::scan::ModRecord;
use crate::store::{CatalogMatch, FileStamp, MatchedBy, StoredProfile};

#[cfg(test)]
mod tests;

const KEY_ENABLED: &str = "updates.enabled";
const KEY_LAST_CHECK: &str = "updates.lastCheck";
const KEY_RETRY_AFTER: &str = "updates.retryAfter";
const KEY_CHECKED: &str = "updates.checked";

/// A check younger than this is not repeated unless forced.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(3 * 60 * 60);
/// Even a forced check is not repeated within this.
pub const FORCE_DEBOUNCE: Duration = Duration::from_secs(60);
/// How long to stay away after a 429 that did not say when to come back.
const DEFAULT_RETRY: Duration = Duration::from_secs(60);
/// Longest link the user may store, in characters.
pub const MAX_LINK_CHARS: usize = 2000;
/// The most a download may be when no size is known (GitHub): no mod comes near it.
const MAX_DOWNLOAD_BYTES: u64 = 8 * 1024 * 1024 * 1024;
/// Free space a download always leaves on the volume it is written to.
const SPACE_RESERVE: u64 = 256 * 1024 * 1024;
/// What a ModHub download may be over the size its page shows, besides 10%.
const SIZE_SLACK: u64 = 1024 * 1024;

/// What updates need from the outside world; tests pass fakes and a fixed clock.
pub struct UpdateEnv<'a> {
    pub catalog: &'a dyn Catalog,
    pub web: &'a dyn Web,
    /// The M5 file-operation environment (running check, Recycle Bin, free space).
    pub files: &'a OpEnv<'a>,
    pub now: &'a dyn Fn() -> SystemTime,
}

/// The flag that keeps requests to one at a time.
#[derive(Default)]
pub(super) struct NetState {
    busy: AtomicBool,
}

pub(super) fn iso(time: SystemTime) -> String {
    humantime::format_rfc3339_seconds(time).to_string()
}

pub(super) fn parse_iso(text: &str) -> Option<SystemTime> {
    humantime::parse_rfc3339(text).ok()
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn not_installable(detail: &str) -> ApiError {
    ApiError::new(ErrorCode::NotInstallable, Some(detail.to_string()))
}

// ---------------------------------------------------------------- overlay

/// A GitHub release asset the catalog offers directly (and the app may download).
fn github_download(found: &CatalogMatch) -> Option<&str> {
    if found.download_source.as_deref() != Some("github") {
        return None;
    }
    found
        .download_url
        .as_deref()
        .filter(|url| net::allowed_download_url(url))
}

/// The ModHub page of the match and its mod id.
fn modhub_page(found: &CatalogMatch) -> Option<(&str, String)> {
    let url = found.modhub_url.as_deref()?;
    Some((url, net::modhub_id(url)?))
}

/// Where an update of this match would be installed from: GitHub when it has a direct release
/// asset, else ModHub when its page is known.
fn install_source(found: &CatalogMatch) -> Option<LinkKind> {
    if github_download(found).is_some() {
        Some(LinkKind::GitHub)
    } else if modhub_page(found).is_some() {
        Some(LinkKind::ModHub)
    } else {
        None
    }
}

/// The update state of one mod: `Available` when the catalog's latest version is newer than the
/// installed one, `UpToDate` when matched otherwise, `Unknown` when not matched. Only a zip can
/// be installed over.
pub(super) fn update_state(entry: &ModEntry, found: Option<&CatalogMatch>) -> UpdateState {
    let Some(found) = found else {
        return UpdateState::Unknown;
    };
    match found.latest_version.as_deref() {
        Some(latest) if net::is_newer(latest, &entry.version) == Some(true) => {
            UpdateState::Available {
                version: latest.to_string(),
                installable: entry.format == ModFormat::Zip && install_source(found).is_some(),
            }
        }
        _ => UpdateState::UpToDate,
    }
}

/// Fills `source`, `links` and `update` of a listing entry from the catalog match of its name and
/// the user's link. The catalog comes first; a manual link on github.com reads as GitHub.
pub(super) fn decorate(entry: &mut ModEntry, found: Option<&CatalogMatch>, manual: Option<&str>) {
    let manual_host = manual.and_then(net::manual_link_host);
    let catalog_source = found.and_then(|f| {
        if let Some(id) = f.modhub_url.as_deref().and_then(net::modhub_id) {
            return Some(ModSource::ModHub { id });
        }
        let (owner, repo) = f.github_url.as_deref().and_then(net::github_repo)?;
        Some(ModSource::GitHub { owner, repo })
    });
    entry.source = catalog_source
        .or_else(|| {
            let (owner, repo) = manual.and_then(net::github_repo_lenient)?;
            Some(ModSource::GitHub { owner, repo })
        })
        .or_else(|| manual_host.clone().map(|host| ModSource::Manual { host }))
        .unwrap_or(ModSource::Unknown);

    entry.links.clear();
    if found.is_some_and(|f| f.modhub_url.is_some()) {
        entry.links.push(SourceLink {
            kind: LinkKind::ModHub,
            label: "ModHub".to_string(),
        });
    }
    if found.is_some_and(|f| f.github_url.is_some()) {
        entry.links.push(SourceLink {
            kind: LinkKind::GitHub,
            label: "GitHub".to_string(),
        });
    }
    if let Some(host) = manual_host {
        entry.links.push(SourceLink {
            kind: LinkKind::Manual,
            label: host,
        });
    }
    entry.update = update_state(entry, found);
}

/// How many of these mods have an update available.
pub(super) fn count_available(
    records: &[ModRecord],
    matches: &HashMap<String, CatalogMatch>,
) -> usize {
    records
        .iter()
        .filter(|r| {
            let key = r.entry.mod_name.to_lowercase();
            matches!(
                update_state(&r.entry, matches.get(&key)),
                UpdateState::Available { .. }
            )
        })
        .count()
}

// ---------------------------------------------------------------- check helpers

/// A catalog row as the cache keeps it. The ModHub page is kept only when it is one the app
/// may fetch (and has a mod id); the GitHub page as the repository's canonical address.
fn to_match(
    key: &str,
    row: &LookupResult,
    matched_by: MatchedBy,
    checked_at: &str,
) -> CatalogMatch {
    let download = row.download.as_ref();
    let source = download.map(|d| d.source.to_ascii_lowercase());
    let modhub_url = row
        .page_url
        .as_deref()
        .filter(|u| net::modhub_id(u).is_some())
        .or_else(|| {
            download
                .filter(|_| source.as_deref() == Some("modhub"))
                .map(|d| d.url.as_str())
                .filter(|u| net::modhub_id(u).is_some())
        })
        .map(str::to_string);
    let github_url = download
        .filter(|_| source.as_deref() == Some("github"))
        .and_then(|d| net::github_repo(&d.url))
        .or_else(|| row.page_url.as_deref().and_then(net::github_repo))
        .map(|(owner, repo)| format!("https://github.com/{owner}/{repo}"));
    CatalogMatch {
        name_key: key.to_string(),
        catalog_id: row.id.clone(),
        tech_name: row.tech_name.clone(),
        latest_version: row.latest_version.clone(),
        download_source: source,
        download_url: download.map(|d| d.url.clone()),
        modhub_url,
        github_url,
        matched_by,
        checked_at: checked_at.to_string(),
    }
}

/// A zip mod found on disk, for hash matching.
struct ZipFile {
    key: String,
    path: PathBuf,
    stamp: FileStamp,
}

/// One profile's copy of a mod.
struct Holder {
    profile: StoredProfile,
    entry: ModEntry,
    path: PathBuf,
    stamp: FileStamp,
}

fn holder_of(profile: &StoredProfile, scan: &Scan, key: &str) -> Option<Holder> {
    let index = *scan.names.get(key)?;
    let record = scan.records.get(index)?;
    Some(Holder {
        profile: profile.clone(),
        entry: record.entry.clone(),
        path: record.path.clone(),
        stamp: *scan.stamps.get(index)?,
    })
}

/// Counts what was learned, for the result and the cached answer.
fn api_error(error: &NetError) -> ApiError {
    match error {
        NetError::Offline => ApiError::new(ErrorCode::Offline, None),
        NetError::Status(n) => ApiError::new(ErrorCode::Offline, Some(format!("status{n}"))),
        NetError::RateLimited { .. } => ApiError::new(ErrorCode::RateLimited, None),
        NetError::NotAllowed => ApiError::new(ErrorCode::Internal, Some("notAllowed".to_string())),
        NetError::BadAnswer(detail) => {
            ApiError::new(ErrorCode::Internal, Some(format!("badAnswer: {detail}")))
        }
        NetError::Cancelled => ApiError::new(ErrorCode::Cancelled, None),
        // A file that is not the size it promised cannot be the update: nothing was replaced.
        NetError::TooLarge => ApiError::new(ErrorCode::VerifyFailed, Some("tooLarge".to_string())),
    }
}

/// The error code a failed download reports for one folder. A page or file that is gone or
/// refused (4xx) is not an outage: the app cannot install it, but its page can still be opened.
fn net_code(error: &NetError) -> ErrorCode {
    match error {
        NetError::NotAllowed => ErrorCode::NotInstallable,
        NetError::Status(n) if (400..500).contains(n) => ErrorCode::NotInstallable,
        NetError::BadAnswer(_) => ErrorCode::Internal,
        other => api_error(other).code,
    }
}

/// The error code of a failed file step.
fn install_code(error: &FileOpError) -> ErrorCode {
    match error {
        FileOpError::Cancelled => ErrorCode::Cancelled,
        FileOpError::Exists => ErrorCode::TargetChanged,
        FileOpError::Verify(_) => ErrorCode::VerifyFailed,
        FileOpError::Io(_) => ErrorCode::WriteFailed,
        FileOpError::ReplaceIncomplete(_) => ErrorCode::ReplaceIncomplete,
        FileOpError::Recycle(r) => recycle_code(r),
    }
}

/// A sink that remembers why a write failed, so a full disk is not mistaken for a network error.
struct Sink<'a> {
    file: &'a mut File,
    failed: Option<String>,
}

impl Write for Sink<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf).inspect_err(|e| {
            self.failed = Some(format!("{:?}", e.kind()));
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

/// Where a resolved update comes from and what it must be.
struct Resolved {
    /// The most bytes the download may be; set from the size the source promised and the space
    /// on the volume once the target is known.
    max_bytes: u64,
    url: String,
    /// The ModHub page the link was found on, sent with the download.
    referer: Option<String>,
    check: DownloadCheck,
}

impl Library {
    /// The online-update settings (off by default).
    pub fn update_settings(&self) -> Result<UpdateSettings, ApiError> {
        Ok(UpdateSettings {
            enabled: self.store.app_setting(KEY_ENABLED)?.as_deref() == Some("true"),
            last_check: self.store.app_setting(KEY_LAST_CHECK)?,
            retry_after: self.store.app_setting(KEY_RETRY_AFTER)?,
        })
    }

    /// Switches online checks on or off. Off forgets nothing but sends nothing.
    pub fn set_updates_enabled(&self, enabled: bool) -> Result<UpdateSettings, ApiError> {
        self.store
            .set_app_setting(KEY_ENABLED, Some(if enabled { "true" } else { "false" }))?;
        self.update_settings()
    }

    /// Checks every registered profile's mods against the catalog (plan M7 outcome 2–3).
    ///
    /// Refused with `UpdatesDisabled` when off. Without `force`, cached results less than 3 hours
    /// old are returned (`from_cache`); with `force`, a check less than 60 s old is too. Before
    /// a stored `retry_after`, nothing is sent and `RateLimited` is returned. Otherwise: distinct
    /// mod names (case-insensitive) go to `lookup` in batches; zips not matched by name are
    /// hashed (SHA-256, cached) and go to `by-hash`; results are stored per lower-cased name,
    /// replacing the previous ones only when the whole check succeeded. A 429 stores
    /// `retry_after` and fails the check; nothing is retried.
    ///
    /// The names come from fresh, cache-assisted scans of every registered profile (`scan_profile`,
    /// which only re-reads files whose size or mtime changed): a mod copied in since the last
    /// listing is still checked, and the scans that answer the question are the ones the rest of
    /// the app then lists from. Mods that are not mods (`NotAMod`) are neither looked up nor
    /// hashed.
    pub fn check_updates(
        &self,
        force: bool,
        ctx: &GameContext,
        env: &UpdateEnv<'_>,
    ) -> Result<UpdateCheck, ApiError> {
        self.remember(ctx);
        let settings = self.update_settings()?;
        if !settings.enabled {
            return Err(ApiError::new(ErrorCode::UpdatesDisabled, None));
        }
        let now = (env.now)();
        let window = if force {
            FORCE_DEBOUNCE
        } else {
            CHECK_INTERVAL
        };
        if let Some(last) = settings.last_check.as_deref().and_then(parse_iso) {
            // A last check in the future (the clock was set back) is not a fresh one.
            if now.duration_since(last).is_ok_and(|age| age < window) {
                return self.cached_check(&iso(last));
            }
        }
        self.guard_retry(now)?;
        let _net = BusyGuard::acquire(&self.updates.busy)?;

        let scans = self.fresh_scans(ctx)?;
        let (names, zips) = names_and_zips(&scans);
        let checked_at = iso(now);
        let outcome = self.look_up(&names, &zips, &checked_at, env);
        let matches = match outcome {
            Ok(matches) => matches,
            Err(error) => return Err(self.net_failure(&error, now)),
        };

        let by_name = matches
            .iter()
            .filter(|m| m.matched_by == MatchedBy::Name)
            .count();
        let by_hash = matches.len() - by_name;
        self.store.replace_catalog_matches(&matches)?;
        self.store
            .set_app_setting(KEY_LAST_CHECK, Some(&checked_at))?;
        self.store.set_app_setting(KEY_RETRY_AFTER, None)?;
        self.store
            .set_app_setting(KEY_CHECKED, Some(&names.len().to_string()))?;

        let by_key: HashMap<String, CatalogMatch> = matches
            .into_iter()
            .map(|m| (m.name_key.clone(), m))
            .collect();
        Ok(UpdateCheck {
            checked_at,
            checked: count(names.len()),
            matched_by_name: count(by_name),
            matched_by_hash: count(by_hash),
            updates: count(updates_in(&scans, &by_key)),
            from_cache: false,
        })
    }

    /// The stored answer of the last check, with nothing sent.
    fn cached_check(&self, checked_at: &str) -> Result<UpdateCheck, ApiError> {
        let matches = self.store.catalog_matches()?;
        let by_name = matches
            .values()
            .filter(|m| m.matched_by == MatchedBy::Name)
            .count();
        let checked = self
            .store
            .app_setting(KEY_CHECKED)?
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(0);
        let mut scans = Vec::new();
        for profile in self.store.profiles()? {
            if let Ok(scan) = self.scan_of(&profile.id) {
                scans.push(scan);
            }
        }
        Ok(UpdateCheck {
            checked_at: checked_at.to_string(),
            checked: count(checked),
            matched_by_name: count(by_name),
            matched_by_hash: count(matches.len() - by_name),
            updates: count(updates_in(&scans, &matches)),
            from_cache: true,
        })
    }

    /// `RateLimited` while a stored `retry_after` is still ahead.
    fn guard_retry(&self, now: SystemTime) -> Result<(), ApiError> {
        let until = self
            .store
            .app_setting(KEY_RETRY_AFTER)?
            .and_then(|t| parse_iso(&t))
            .filter(|until| *until > now);
        match until {
            Some(until) => Err(ApiError::new(ErrorCode::RateLimited, Some(iso(until)))),
            None => Ok(()),
        }
    }

    /// Turns a failed request into the error the caller gets. A 429 also stores when to come back
    /// (the server's word, or a minute).
    fn net_failure(&self, error: &NetError, now: SystemTime) -> ApiError {
        if let NetError::RateLimited { retry_after } = error {
            let until = retry_after.unwrap_or(now + DEFAULT_RETRY);
            let _ = self
                .store
                .set_app_setting(KEY_RETRY_AFTER, Some(&iso(until)));
            return ApiError::new(ErrorCode::RateLimited, Some(iso(until)));
        }
        api_error(error)
    }

    /// Every registered profile scanned now, in registry order. A profile whose folder cannot be
    /// read simply holds no mods here.
    fn fresh_scans(&self, ctx: &GameContext) -> Result<Vec<Arc<Scan>>, ApiError> {
        let profiles = self.store.profiles()?;
        Ok(profiles
            .par_iter()
            .map(|p| self.scan_profile(p, ctx, &no_progress).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .flatten()
            .collect())
    }

    /// The network half of a check: by name, then by hash for the zips left over. Returns the
    /// matches to store; nothing is stored here.
    fn look_up(
        &self,
        names: &[(String, String)],
        zips: &[ZipFile],
        checked_at: &str,
        env: &UpdateEnv<'_>,
    ) -> Result<Vec<CatalogMatch>, NetError> {
        let asked: Vec<String> = names.iter().map(|(_, name)| name.clone()).collect();
        let wanted: HashSet<&str> = names.iter().map(|(key, _)| key.as_str()).collect();
        let mut by_name: HashMap<String, LookupResult> = HashMap::new();
        for row in net::lookup_all(env.catalog, &asked)? {
            if let Some(key) = row.tech_name.as_deref().map(str::to_lowercase) {
                if wanted.contains(key.as_str()) {
                    by_name.entry(key).or_insert(row);
                }
            }
        }
        let mut matches: Vec<CatalogMatch> = names
            .iter()
            .filter_map(|(key, _)| {
                Some(to_match(
                    key,
                    by_name.get(key)?,
                    MatchedBy::Name,
                    checked_at,
                ))
            })
            .collect();

        // The zips no name explained: their whole-file hash may still be a build the catalog knows
        // (a renamed copy).
        let unmatched: Vec<&ZipFile> = zips
            .iter()
            .filter(|z| !by_name.contains_key(&z.key))
            .collect();
        if unmatched.is_empty() {
            return Ok(matches);
        }
        let hashes = self.hashes_of(&unmatched);
        let asked: Vec<String> = hashes.iter().flatten().cloned().collect();
        let Some(answers) = net::by_hash_all(env.catalog, &asked)? else {
            return Ok(matches);
        };
        // The catalog's name for the first build that has each hash.
        let tech_of_hash: HashMap<&str, &str> = answers
            .iter()
            .filter_map(|(hash, builds)| {
                let tech = builds.iter().find_map(|b| b.tech_name.as_deref())?;
                Some((hash.as_str(), tech))
            })
            .collect();
        let mut via_hash: Vec<(&str, &str)> = Vec::new();
        let mut done: HashSet<&str> = HashSet::new();
        for (zip, hash) in unmatched.iter().zip(&hashes) {
            let Some(tech) = hash.as_deref().and_then(|h| tech_of_hash.get(h).copied()) else {
                continue;
            };
            if done.insert(zip.key.as_str()) {
                via_hash.push((zip.key.as_str(), tech));
            }
        }
        if via_hash.is_empty() {
            return Ok(matches);
        }
        // The by-hash answer names builds; the latest version and download come from a lookup of
        // the catalog's own names.
        let techs: Vec<String> = via_hash
            .iter()
            .map(|(_, tech)| (*tech).to_string())
            .collect();
        let latest: HashMap<String, LookupResult> = net::lookup_all(env.catalog, &techs)?
            .into_iter()
            .filter_map(|row| Some((row.tech_name.as_deref()?.to_lowercase(), row)))
            .collect();
        for (key, tech) in via_hash {
            if let Some(row) = latest.get(&tech.to_lowercase()) {
                matches.push(to_match(key, row, MatchedBy::Hash, checked_at));
            }
        }
        Ok(matches)
    }

    /// The SHA-256 of each zip, from the cache when its size and mtime still match, else computed
    /// (in parallel, as the M6 hashing is) and cached. A file that cannot be read has none.
    fn hashes_of(&self, zips: &[&ZipFile]) -> Vec<Option<String>> {
        let queries: Vec<(&Path, FileStamp)> =
            zips.iter().map(|z| (z.path.as_path(), z.stamp)).collect();
        let mut hashes = self
            .store
            .cached_sha256(&queries)
            .unwrap_or_else(|_| vec![None; zips.len()]);
        let missing: Vec<usize> = (0..zips.len()).filter(|&i| hashes[i].is_none()).collect();
        let never = AtomicBool::new(false);
        let computed: Vec<(usize, Option<String>)> = missing
            .par_iter()
            .map(|&i| {
                (
                    i,
                    files::hash::sha256(&zips[i].path, &mut |_| {}, &never).ok(),
                )
            })
            .collect();
        let mut fresh = Vec::new();
        for (i, hash) in computed {
            if let Some(hash) = hash {
                fresh.push(((zips[i].path.as_path(), zips[i].stamp), hash.clone()));
                hashes[i] = Some(hash);
            }
        }
        // A cache that cannot be written only costs a rehash later.
        let rows: Vec<((&Path, FileStamp), &str)> =
            fresh.iter().map(|(q, h)| (*q, h.as_str())).collect();
        let _ = self.store.put_sha256(&rows);
        hashes
    }

    /// The folders holding `mod_name` and where its update would come from, without any
    /// network request. `NotInstallable` when the cached match has no direct download.
    ///
    /// `FolderMissing` when no registered profile holds the name; `NotInstallable` when the
    /// name was not matched, no holder is older than the catalog's version, or there is neither
    /// a GitHub release asset nor a ModHub page. The details are `noMatch`, `upToDate` and
    /// `noDirectDownload`.
    pub fn plan_update(&self, mod_name: &str, ctx: &GameContext) -> Result<UpdatePlan, ApiError> {
        self.remember(ctx);
        let key = name_key(mod_name)?;
        let mut holders = Vec::new();
        for profile in self.store.profiles()? {
            if let Ok(scan) = self.scan_profile(&profile, ctx, &no_progress) {
                holders.extend(holder_of(&profile, &scan, &key));
            }
        }
        let Some(first) = holders.first() else {
            return Err(ApiError::new(
                ErrorCode::FolderMissing,
                Some(mod_name.to_string()),
            ));
        };
        let found = self
            .store
            .catalog_match(&key)?
            .ok_or_else(|| not_installable("noMatch"))?;
        let to_version = found
            .latest_version
            .clone()
            .ok_or_else(|| not_installable("noMatch"))?;
        if !holders
            .iter()
            .any(|h| net::is_newer(&to_version, &h.entry.version) == Some(true))
        {
            return Err(not_installable("upToDate"));
        }
        let source = install_source(&found).ok_or_else(|| not_installable("noDirectDownload"))?;
        Ok(UpdatePlan {
            mod_name: first.entry.mod_name.clone(),
            to_version,
            source,
            folders: holders
                .iter()
                .map(|h| UpdateFolder {
                    profile_id: h.profile.id.clone(),
                    mod_id: h.entry.id.clone(),
                    version: h.entry.version.clone(),
                })
                .collect(),
        })
    }

    /// Downloads, verifies and installs the update of `mod_name` into each of `profile_ids`
    /// (plan M7 outcome 5): resolves the download (GitHub `downloadUrl` from `detail`, or the
    /// ModHub page's download link), refuses an asset with another file name (`NotInstallable`),
    /// streams it into the first folder via `files::install_download` with the M5 busy/cancel
    /// state and progress, recycles the old version just before the rename, then fills the other
    /// folders with `copy_staged_replacing`. Refused while the game or server runs. Each folder
    /// is one journal row (`update`) and one entry in the result.
    ///
    /// Entries of the result are profile ids (`failed` items carry the profile id in `mod_name`).
    /// Whatever stops the whole call before a file is touched is an error: updates off, the game
    /// or server running, another operation busy, a mod name the catalog did not match, a
    /// download that cannot be resolved (`NotInstallable`, `Offline`, `RateLimited`) or lacks
    /// space. Once the download starts, problems are per folder: a folder without the mod, or
    /// holding it unpacked, fails by itself; a failed download or verification fails every
    /// folder with the same code, because they all depend on that one file; a cancel or the game
    /// starting stops the folders not done yet. The old zip is recycled only after the new file
    /// verified; a refused recycle (`RecycleUnavailable`) or a mod that changed meanwhile
    /// (`TargetChanged`) keeps it.
    pub fn install_update(
        &self,
        mod_name: &str,
        profile_ids: &[String],
        ctx: &GameContext,
        env: &UpdateEnv<'_>,
        progress: &(dyn Fn(OpProgress) + Sync),
    ) -> Result<OpResult, ApiError> {
        self.remember(ctx);
        if !self.update_settings()?.enabled {
            return Err(ApiError::new(ErrorCode::UpdatesDisabled, None));
        }
        let key = name_key(mod_name)?;
        let _busy = BusyGuard::acquire(&self.transfer.busy)?;
        let _net = BusyGuard::acquire(&self.updates.busy)?;
        ops::check_not_running(env.files.running)?;
        // Cleared here, not after the download is resolved: that takes network requests, and a
        // Cancel pressed meanwhile must still count (the loop below sees it before it downloads).
        self.transfer.cancel.store(false, Ordering::SeqCst);

        let found = self
            .store
            .catalog_match(&key)?
            .ok_or_else(|| not_installable("noMatch"))?;
        let source = install_source(&found).ok_or_else(|| not_installable("noDirectDownload"))?;

        // The folders asked for, as the profiles hold the mod now.
        let mut result = OpResult::default();
        let mut seen = HashSet::new();
        let mut targets: Vec<Holder> = Vec::new();
        for id in profile_ids.iter().filter(|id| seen.insert(id.as_str())) {
            let held = self.find_profile(id).and_then(|profile| {
                let scan = self.scan_profile(&profile, ctx, &no_progress)?;
                Ok(holder_of(&profile, &scan, &key))
            });
            let code = match held {
                Ok(Some(h)) if h.entry.format == ModFormat::Zip => {
                    targets.push(h);
                    continue;
                }
                Ok(Some(_)) => ErrorCode::NotInstallable,
                Ok(None) => ErrorCode::FolderMissing,
                Err(e) => e.code,
            };
            result.failed.push(ItemFailure {
                mod_name: id.clone(),
                code,
            });
        }
        let Some(first) = targets.first() else {
            return Ok(result);
        };

        let file_name = first.entry.file_name.clone();
        let mut resolved = self.resolve_download(&found, source, &file_name, &targets, env)?;
        let estimate = resolved.check.approx_size.unwrap_or(first.stamp.size);
        let free = env
            .files
            .disk
            .free_bytes(&first.profile.path)
            .map_err(|e| io_api(&e))?;
        // The download never fills the volume: it stops at what its source promised, or at the
        // space there is less a reserve, whichever is smaller.
        resolved.max_bytes = resolved.max_bytes.min(free.saturating_sub(SPACE_RESERVE));
        if estimate > resolved.max_bytes {
            return Err(ApiError::new(ErrorCode::NotEnoughSpace, None));
        }

        // A Cancel pressed while the download was being resolved is still set here, and the loop
        // below stops before anything is downloaded.
        let plan_id = format!(
            "u{}",
            self.transfer.next_plan.fetch_add(1, Ordering::SeqCst) + 1
        );
        let total = estimate.saturating_mul(targets.len() as u64);
        let mut tracker = Tracker::new(progress, total, targets.len());
        tracker.send();

        let mut stopped: Option<ErrorCode> = None;
        let mut installed: Option<PathBuf> = None;
        for (i, target) in targets.iter().enumerate() {
            if stopped.is_none() && self.transfer.cancel.load(Ordering::SeqCst) {
                stopped = Some(ErrorCode::Cancelled);
            }
            if let Some(code) = stopped {
                result.failed.push(ItemFailure {
                    mod_name: target.profile.id.clone(),
                    code,
                });
                continue;
            }
            tracker.begin(&file_name);
            // The verified file that was kept under its staged name, when it could not be renamed.
            let mut kept: Option<PathBuf> = None;
            let outcome = match &installed {
                None => self.install_first(
                    target,
                    &file_name,
                    &resolved,
                    &plan_id,
                    env,
                    &mut tracker,
                    &mut kept,
                ),
                Some(from) => self
                    .install_copy(target, from, &plan_id, env, &mut tracker, &mut kept)
                    .map(|()| from.clone()),
            };
            let new_path = kept.unwrap_or_else(|| target.profile.path.join(&file_name));
            self.journal_file_op(
                "update",
                Some(&plan_id),
                &target.path,
                Some(&new_path),
                &outcome
                    .as_ref()
                    .map_or_else(|code| code_name(*code), |_| "ok".to_string()),
            );
            match outcome {
                Ok(path) => {
                    installed.get_or_insert(path);
                    result.done.push(target.profile.id.clone());
                }
                Err(code) => {
                    // Without the first folder's file there is nothing to copy from.
                    if i == 0 || stops_operation(code) {
                        stopped = Some(code);
                    }
                    result.failed.push(ItemFailure {
                        mod_name: target.profile.id.clone(),
                        code,
                    });
                }
            }
            tracker.end_item(estimate.saturating_mul(i as u64 + 1));
        }
        tracker.finish();

        for target in &targets {
            self.forget_scan(&target.profile.id);
        }
        Ok(result)
    }

    /// Finds the address and the checks of the update, asking the catalog (GitHub) or the ModHub
    /// page. Nothing is written. The asset must be one the app may download and must have the
    /// installed file's name, ignoring case: renaming would change the mod's identity.
    fn resolve_download(
        &self,
        found: &CatalogMatch,
        source: LinkKind,
        installed_name: &str,
        targets: &[Holder],
        env: &UpdateEnv<'_>,
    ) -> Result<Resolved, ApiError> {
        let now = (env.now)();
        let (url, referer, version, sha256, approx_size, max_bytes) = match source {
            LinkKind::ModHub => {
                let (page, id) =
                    modhub_page(found).ok_or_else(|| not_installable("noDirectDownload"))?;
                let html = env.web.fetch_page(page).map_err(|e| match e {
                    NetError::Status(n) if (400..500).contains(&n) => not_installable("pageStatus"),
                    other => api_error(&other),
                })?;
                let link = net::modhub_download(&html, &id)
                    .ok_or_else(|| not_installable("noDownloadLink"))?;
                let version = link.version.or_else(|| found.latest_version.clone());
                // The page's size, 10% more and a megabyte; without a size, the hard maximum.
                let max_bytes = link.approx_size.map_or(MAX_DOWNLOAD_BYTES, |size| {
                    size.saturating_add(size / 10)
                        .saturating_add(SIZE_SLACK)
                        .min(MAX_DOWNLOAD_BYTES)
                });
                (
                    link.url,
                    Some(page.to_string()),
                    version,
                    None,
                    link.approx_size,
                    max_bytes,
                )
            }
            _ => {
                self.guard_retry(now)?;
                let detail = env
                    .catalog
                    .detail(&found.catalog_id)
                    .map_err(|e| self.net_failure(&e, now))?
                    .ok_or_else(|| not_installable("unknownMod"))?;
                let asset = detail
                    .sources
                    .iter()
                    .find(|s| s.source == "github" && s.installable && s.download_url.is_some())
                    .ok_or_else(|| not_installable("noDirectDownload"))?;
                let version = asset
                    .version
                    .clone()
                    .or_else(|| detail.latest_version.clone())
                    .or_else(|| found.latest_version.clone());
                // The catalog's hash is for its latest build: only trust it for that version.
                let sha256 = detail.archive_sha256.clone().filter(|_| {
                    match (&detail.latest_version, &version) {
                        (Some(latest), Some(version)) => net::versions_equal(latest, version),
                        _ => false,
                    }
                });
                // No size is known for a GitHub asset: only the hard maximum.
                (
                    asset.download_url.clone().unwrap_or_default(),
                    None,
                    version,
                    sha256,
                    None,
                    MAX_DOWNLOAD_BYTES,
                )
            }
        };
        let version = version.ok_or_else(|| not_installable("noVersion"))?;
        if !net::allowed_download_url(&url) {
            return Err(not_installable("notAllowed"));
        }
        let same_name = net::download_file_name(&url)
            .is_some_and(|name| name.eq_ignore_ascii_case(installed_name));
        if !same_name {
            return Err(not_installable("fileName"));
        }
        // A GitHub asset must be a release of the repository the catalog's own lookup named (the
        // one the page shows): anyone can publish an asset on github.com, and one answer of a
        // third-party service must not be enough to install it.
        if source != LinkKind::ModHub {
            let matched = found.github_url.as_deref().and_then(net::github_repo);
            let same_repo = matched.is_some_and(|(owner, repo)| {
                net::github_release_repo(&url).is_some_and(|(o, r)| {
                    o.eq_ignore_ascii_case(&owner) && r.eq_ignore_ascii_case(&repo)
                })
            });
            if !same_repo {
                return Err(not_installable("repoMismatch"));
            }
        }
        if !targets
            .iter()
            .any(|t| net::is_newer(&version, &t.entry.version) == Some(true))
        {
            return Err(not_installable("upToDate"));
        }
        Ok(Resolved {
            max_bytes,
            url,
            referer,
            check: DownloadCheck {
                sha256,
                min_version: version,
                approx_size,
            },
        })
    }

    /// Downloads into the first folder, verifies, recycles the old zip and takes its place.
    #[allow(clippy::too_many_arguments)]
    fn install_first(
        &self,
        target: &Holder,
        file_name: &str,
        resolved: &Resolved,
        plan_id: &str,
        env: &UpdateEnv<'_>,
        tracker: &mut Tracker<'_>,
        kept: &mut Option<PathBuf>,
    ) -> Result<PathBuf, ErrorCode> {
        let cancel = &self.transfer.cancel;
        let mut net_error: Option<NetError> = None;
        let mut refused: Option<ErrorCode> = None;
        let outcome = files::install_download(
            &target.profile.path,
            file_name,
            &mut |file: &mut File| {
                let mut sink = Sink { file, failed: None };
                let result = env.web.download(
                    &resolved.url,
                    resolved.referer.as_deref(),
                    resolved.max_bytes,
                    &mut sink,
                    &mut |n| tracker.add_bytes(n),
                    cancel,
                );
                match result {
                    Ok(_) => Ok(()),
                    Err(NetError::Cancelled) => Err(FileOpError::Cancelled),
                    Err(error) => match sink.failed {
                        Some(kind) => Err(FileOpError::Io(kind)),
                        None => {
                            net_error = Some(error);
                            Err(FileOpError::Io("Network".to_string()))
                        }
                    },
                }
            },
            &resolved.check,
            &mut || self.replace_old(target, plan_id, env, &mut refused),
        );
        outcome.map_err(|e| {
            if let FileOpError::ReplaceIncomplete(name) = &e {
                *kept = Some(target.profile.path.join(name));
            }
            refused
                .or_else(|| net_error.as_ref().map(net_code))
                .unwrap_or_else(|| install_code(&e))
        })
    }

    /// Fills another folder from the verified file of the first, replacing the old zip.
    fn install_copy(
        &self,
        target: &Holder,
        source: &Path,
        plan_id: &str,
        env: &UpdateEnv<'_>,
        tracker: &mut Tracker<'_>,
        kept: &mut Option<PathBuf>,
    ) -> Result<(), ErrorCode> {
        running_code(env.files)?;
        let size = fs::metadata(source)
            .map_err(|_| ErrorCode::WriteFailed)?
            .len();
        let free = env
            .files
            .disk
            .free_bytes(&target.profile.path)
            .map_err(|_| ErrorCode::WriteFailed)?;
        if size > free {
            return Err(ErrorCode::NotEnoughSpace);
        }
        let mut refused: Option<ErrorCode> = None;
        files::copy_staged_replacing(
            source,
            &target.profile.path,
            &mut |n| tracker.add_bytes(n),
            &self.transfer.cancel,
            &mut || self.replace_old(target, plan_id, env, &mut refused),
        )
        .map(|_| ())
        .map_err(|e| {
            if let FileOpError::ReplaceIncomplete(name) = &e {
                *kept = Some(target.profile.path.join(name));
            }
            refused.unwrap_or_else(|| install_code(&e))
        })
    }

    /// The step just before a new file takes its name: the last look at the guards, then the old
    /// zip to the Recycle Bin (journaled). Why it said no is left in `refused` when it is not
    /// one of the file errors.
    fn replace_old(
        &self,
        target: &Holder,
        plan_id: &str,
        env: &UpdateEnv<'_>,
        refused: &mut Option<ErrorCode>,
    ) -> Result<(), FileOpError> {
        if self.transfer.cancel.load(Ordering::SeqCst) {
            return Err(FileOpError::Cancelled);
        }
        if let Err(code) = running_code(env.files) {
            *refused = Some(code);
            return Err(FileOpError::Io("Guard".to_string()));
        }
        if !unchanged(&target.path, target.stamp) {
            *refused = Some(ErrorCode::TargetChanged);
            return Err(FileOpError::Io("Changed".to_string()));
        }
        let recycled = env.files.recycler.recycle(&target.path);
        self.journal_file_op(
            "recycle",
            Some(plan_id),
            &target.path,
            None,
            &recycled
                .as_ref()
                .map_or_else(|e| code_name(recycle_code(e)), |_| "ok".to_string()),
        );
        recycled.map_err(FileOpError::Recycle)
    }

    /// The URL behind one of the mod's links: the cached catalog page for `ModHub`/`GitHub`,
    /// the user's link for `Manual`. Only `http`/`https`. `FolderMissing` when there is none.
    pub fn link_url(&self, mod_name: &str, kind: LinkKind) -> Result<String, ApiError> {
        let key = name_key(mod_name)?;
        let url = match kind {
            LinkKind::Manual => self.store.mod_meta(&key)?.source_url,
            LinkKind::ModHub => self.store.catalog_match(&key)?.and_then(|m| m.modhub_url),
            LinkKind::GitHub => self.store.catalog_match(&key)?.and_then(|m| m.github_url),
        };
        url.filter(|u| net::manual_link_host(u).is_some())
            .ok_or_else(|| ApiError::new(ErrorCode::FolderMissing, Some(mod_name.to_string())))
    }

    /// Sets or clears (`None`) the user's source link for a mod name. The URL must be
    /// `http`/`https`, at most 2000 characters, without user info; otherwise `InvalidLink`.
    /// Returns the stored link's host.
    ///
    /// The link is trimmed and stored as written. Whitespace inside it, a port, a backslash and
    /// an empty or odd host are `InvalidLink` too (see `net::manual_link_host`).
    pub fn set_source_link(
        &self,
        mod_name: &str,
        url: Option<&str>,
    ) -> Result<Option<String>, ApiError> {
        let key = name_key(mod_name)?;
        let Some(url) = url else {
            self.store.set_mod_source_url(&key, None)?;
            return Ok(None);
        };
        let url = url.trim();
        let invalid = || ApiError::new(ErrorCode::InvalidLink, None);
        if url.chars().count() > MAX_LINK_CHARS {
            return Err(invalid());
        }
        let host = net::manual_link_host(url).ok_or_else(invalid)?;
        self.store.set_mod_source_url(&key, Some(url))?;
        Ok(Some(host))
    }
}

/// The distinct names and the zips of these scans, skipping what is not a mod. The name kept
/// for each lower-cased key is the first spelling met.
fn names_and_zips(scans: &[Arc<Scan>]) -> (Vec<(String, String)>, Vec<ZipFile>) {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut zips = Vec::new();
    for scan in scans {
        for (record, stamp) in scan.records.iter().zip(&scan.stamps) {
            let entry = &record.entry;
            if entry.issues.iter().any(|i| i.kind == IssueKind::NotAMod) {
                continue;
            }
            let key = entry.mod_name.to_lowercase();
            if seen.insert(key.clone()) {
                names.push((key.clone(), entry.mod_name.clone()));
            }
            if entry.format == ModFormat::Zip {
                zips.push(ZipFile {
                    key,
                    path: record.path.clone(),
                    stamp: *stamp,
                });
            }
        }
    }
    (names, zips)
}

/// How many distinct mod names have an installed copy with an update available.
fn updates_in(scans: &[Arc<Scan>], matches: &HashMap<String, CatalogMatch>) -> usize {
    let mut names = HashSet::new();
    for scan in scans {
        for record in &scan.records {
            let key = record.entry.mod_name.to_lowercase();
            if matches!(
                update_state(&record.entry, matches.get(&key)),
                UpdateState::Available { .. }
            ) {
                names.insert(key);
            }
        }
    }
    names.len()
}
