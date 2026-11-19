//! Everything that goes online (plan M7). Nothing else in the app makes a network request.
//!
//! - **SiloAPI** (`https://silo-api.hllmr.com`), a public, read-only, rate-limited mod catalog
//!   (300 requests a minute per IP, `cache-control: max-age=300`): `POST /mods/lookup` by mod
//!   name, `POST /mods/by-hash` by whole-zip SHA-256, `GET /mods/:id` for one record.
//! - **Downloads** of updates from GitHub releases and from ModHub's public download links, and
//!   fetching a ModHub mod page to find that link (only when the user clicks Update).
//!
//! Good-citizen rules: one request at a time, a neutral User-Agent, connect and read timeouts,
//! no proxy from the environment, an allow-list of hosts (no redirects for the API, redirects
//! for downloads only within the list), capped response sizes, and a 429 or 5xx stops the work
//! (no retry loop) and reports when to try again.
//!
//! The traits below make one request per call, so tests count requests and batching is done by
//! [`lookup_all`] and [`by_hash_all`].

use std::fmt;
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::time::SystemTime;

/// The catalog's base URL.
pub const SILO_BASE: &str = "https://silo-api.hllmr.com";
/// `POST /mods/lookup` accepts at most this many names per request.
pub const LOOKUP_BATCH: usize = 500;
/// `POST /mods/by-hash` accepts at most this many hashes per request.
pub const BY_HASH_BATCH: usize = 100;

/// One row of `POST /mods/lookup`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupResult {
    /// The game's mod name (zip name without `.zip`) the row answers.
    pub tech_name: Option<String>,
    /// The catalog id (`GET /mods/:id`).
    pub id: String,
    pub title: String,
    pub latest_version: Option<String>,
    /// The best direct download, if any: `source` is `github` or `modhub`.
    pub download: Option<Download>,
    /// The mod's page (ModHub first) for when nothing is directly installable.
    pub page_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub source: String,
    pub url: String,
}

/// One catalog build whose whole-zip SHA-256 is the one asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashMatch {
    pub id: String,
    pub tech_name: Option<String>,
    pub version: String,
    pub source: String,
    pub file_name: Option<String>,
    pub download_url: Option<String>,
}

/// The by-hash answer: each hash asked, with the catalog builds that have it (empty if none).
pub type HashAnswers = Vec<(String, Vec<HashMatch>)>;

/// `GET /mods/:id`, the fields the app uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogMod {
    pub id: String,
    pub tech_name: Option<String>,
    pub latest_version: Option<String>,
    /// Lower-case hex SHA-256 of the latest build's zip, when the catalog has it.
    pub archive_sha256: Option<String>,
    pub page_url: Option<String>,
    pub sources: Vec<CatalogSource>,
}

/// One place a catalog mod is published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSource {
    /// `github` or `modhub`.
    pub source: String,
    pub source_url: Option<String>,
    pub version: Option<String>,
    pub installable: bool,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    /// No connection, DNS failure, timeout.
    Offline,
    /// 429: the server asked to wait, until this time when it said.
    RateLimited { retry_after: Option<SystemTime> },
    /// Any other non-success status (5xx included). Not retried.
    Status(u16),
    /// The answer was not the expected shape, or too large. The detail is technical.
    BadAnswer(String),
    /// The URL is not on the allow-list (or redirected off it).
    NotAllowed,
    /// The caller's cancel flag was set.
    Cancelled,
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for NetError {}

/// The catalog, one request per call.
pub trait Catalog: Send + Sync {
    /// `POST /mods/lookup` for at most [`LOOKUP_BATCH`] non-blank, distinct names.
    fn lookup(&self, tech_names: &[String]) -> Result<Vec<LookupResult>, NetError>;
    /// `POST /mods/by-hash` for at most [`BY_HASH_BATCH`] distinct lower-case hashes. Every hash
    /// asked has an entry (empty when no build has it). `Ok(None)` from a server without the
    /// route (404).
    fn by_hash(&self, hashes: &[String]) -> Result<Option<HashAnswers>, NetError>;
    /// `GET /mods/:id`. `Ok(None)` when the id is unknown (404).
    fn detail(&self, id: &str) -> Result<Option<CatalogMod>, NetError>;
}

/// Pages and downloads, one request per call. Implementations refuse URLs that are not
/// [`allowed_page_url`] / [`allowed_download_url`].
pub trait Web: Send + Sync {
    /// The HTML of an allowed page (a ModHub mod page), capped in size.
    fn fetch_page(&self, url: &str) -> Result<String, NetError>;
    /// Streams an allowed download into `sink`, reporting bytes as they arrive and checking
    /// `cancel` between chunks. Returns the bytes written.
    fn download(
        &self,
        url: &str,
        sink: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
        cancel: &AtomicBool,
    ) -> Result<u64, NetError>;
}

/// Every name looked up in batches of [`LOOKUP_BATCH`], one request at a time: blanks and
/// repeats (case-insensitive) dropped first. Stops at the first error.
pub fn lookup_all(
    _catalog: &dyn Catalog,
    _tech_names: &[String],
) -> Result<Vec<LookupResult>, NetError> {
    todo!("WS-A")
}

/// Every hash asked in batches of [`BY_HASH_BATCH`], one request at a time, lower-cased and
/// deduplicated. `Ok(None)` when the server has no by-hash route.
pub fn by_hash_all(
    _catalog: &dyn Catalog,
    _hashes: &[String],
) -> Result<Option<HashAnswers>, NetError> {
    todo!("WS-A")
}

/// Versions are equal ignoring a leading `v`, the separators `.`, `-`, `_`, `+`, leading zeros
/// and trailing `.0`s (`v1.2.0` == `1.2`). Versions not starting with a digit only match
/// exactly. (Silo's `versions_equal`.)
pub fn versions_equal(_a: &str, _b: &str) -> bool {
    todo!("WS-A")
}

/// `Some(true)` when `candidate` is a newer version than `installed` (numeric segments compared
/// left to right, missing ones as 0), `Some(false)` when equal or older, `None` when either does
/// not parse (then the app says nothing about an update).
pub fn is_newer(_candidate: &str, _installed: &str) -> Option<bool> {
    todo!("WS-A")
}

/// API calls: only `https://silo-api.hllmr.com/...`.
pub fn allowed_api_url(_url: &str) -> bool {
    todo!("WS-A")
}

/// Pages fetched by the app: only `https://www.farming-simulator.com/mod.php?...`.
pub fn allowed_page_url(_url: &str) -> bool {
    todo!("WS-A")
}

/// Downloads and their redirects: `https` on `github.com`, `objects.githubusercontent.com`,
/// `release-assets.githubusercontent.com` or `cdn<digits>.giants-software.com`.
pub fn allowed_download_url(_url: &str) -> bool {
    todo!("WS-A")
}

/// What a ModHub mod page says about its download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModHubDownload {
    pub url: String,
    pub file_name: String,
    /// The page's "Version" row, if present.
    pub version: Option<String>,
    /// The page's "Size" row in bytes (approximate, from e.g. `3.79 MB`), if present.
    pub approx_size: Option<u64>,
}

/// The one DOWNLOAD link of a ModHub mod page: an `<a href>` to
/// `https://cdn<n>.giants-software.com/modHub/storage/<8-digit mod_id>/<File>.zip` whose mod id
/// is `mod_id`. `None` when there is not exactly one such link (never a guess). Screenshots on
/// the same storage path are not links and are ignored. See `fixtures/modhub_page.html`.
pub fn modhub_download(_html: &str, _mod_id: &str) -> Option<ModHubDownload> {
    todo!("WS-A")
}

/// The ModHub mod id in a page URL (`mod.php?mod_id=331515&...`).
pub fn modhub_id(_page_url: &str) -> Option<String> {
    todo!("WS-A")
}

/// `(owner, repo)` of a `https://github.com/<owner>/<repo>[/...]` URL.
pub fn github_repo(_url: &str) -> Option<(String, String)> {
    todo!("WS-A")
}

/// The real clients: SiloAPI and the web, over one shared agent with timeouts, the User-Agent
/// and the allow-list. Built by the Tauri layer; tests use fakes.
pub struct Http {
    _private: (),
}

impl Http {
    /// `app_version` goes into the User-Agent: `SimVizion-Mod-Manager/<version>`.
    pub fn new(_app_version: &str) -> Self {
        todo!("WS-A")
    }
}

impl Catalog for Http {
    fn lookup(&self, _tech_names: &[String]) -> Result<Vec<LookupResult>, NetError> {
        todo!("WS-A")
    }
    fn by_hash(&self, _hashes: &[String]) -> Result<Option<HashAnswers>, NetError> {
        todo!("WS-A")
    }
    fn detail(&self, _id: &str) -> Result<Option<CatalogMod>, NetError> {
        todo!("WS-A")
    }
}

impl Web for Http {
    fn fetch_page(&self, _url: &str) -> Result<String, NetError> {
        todo!("WS-A")
    }
    fn download(
        &self,
        _url: &str,
        _sink: &mut dyn Write,
        _progress: &mut dyn FnMut(u64),
        _cancel: &AtomicBool,
    ) -> Result<u64, NetError> {
        todo!("WS-A")
    }
}
