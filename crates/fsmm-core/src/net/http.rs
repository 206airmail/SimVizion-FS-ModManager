//! The real clients, on ureq: SiloAPI and the web over one agent.
//!
//! - One [`Agent`]: connect timeout 10 s, per-read timeout 30 s, API and page calls also 30 s in
//!   all (downloads have no overall limit, only the per-read one), the neutral User-Agent, no
//!   proxy from the environment, https only, and **no redirects**.
//! - Downloads follow 3xx themselves, at most 5 hops, each `Location` checked against
//!   [`allowed_download_url`]. Pages may redirect to another allowed page (3 hops); the API never.
//! - Every body is read with a cap (lookup 4 MiB, by-hash 1 MiB, detail 1 MiB, page 2 MiB).
//! - 429 is `RateLimited`, other non-2xx statuses are `Status(n)`, transport errors are `Offline`.
//!   Nothing is retried.
//! - Debug builds print `[net] METHOD host/path -> status`: never a body or a query string.
//!
//! The answers are parsed into private DTOs (unknown fields ignored) and mapped to the `net`
//! types. Nothing here is tested against the real network; the parsing, the header handling and
//! the redirect decision are pure functions with their own tests.

use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::Deserialize;
use ureq::http::{HeaderMap, Response};
use ureq::{Agent, Body};

use super::url::{self, encode_segment};
use super::{
    allowed_api_url, allowed_download_url, allowed_page_url, Catalog, CatalogMod, CatalogSource,
    Download, HashAnswers, HashMatch, LookupResult, NetError, Web, BY_HASH_BATCH, LOOKUP_BATCH,
    SILO_BASE,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Overall limit of an API or page call (not of a download).
const CALL_TIMEOUT: Duration = Duration::from_secs(30);

const LOOKUP_CAP: u64 = 4 * 1024 * 1024;
const BY_HASH_CAP: u64 = 1024 * 1024;
const DETAIL_CAP: u64 = 1024 * 1024;
const PAGE_CAP: u64 = 2 * 1024 * 1024;

const MAX_DOWNLOAD_HOPS: usize = 5;
const MAX_PAGE_HOPS: usize = 3;
/// Bytes read from a download per step; cancellation is checked per step.
const CHUNK: usize = 64 * 1024;
/// A server cannot ask for a longer pause than this.
const MAX_RETRY_WAIT: Duration = Duration::from_secs(24 * 60 * 60);

/// The real clients: SiloAPI and the web, over one shared agent with timeouts, the User-Agent
/// and the allow-list. Built by the Tauri layer; tests use fakes.
pub struct Http {
    agent: Agent,
}

impl Http {
    /// `app_version` goes into the User-Agent: `SimVizion-Mod-Manager/<version>`.
    pub fn new(app_version: &str) -> Self {
        let config = Agent::config_builder()
            .http_status_as_error(false)
            .https_only(true)
            .proxy(None)
            .max_redirects(0)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_per_read(Some(READ_TIMEOUT))
            .user_agent(format!("SimVizion-Mod-Manager/{app_version}"))
            .build();
        Self {
            agent: Agent::new_with_config(config),
        }
    }

    /// One request, no redirects followed. Only a transport failure is an error here; the
    /// caller decides what the status means.
    fn send(
        &self,
        method: &str,
        url: &str,
        body: Option<&serde_json::Value>,
        overall: Option<Duration>,
    ) -> Result<(u16, Response<Body>), NetError> {
        let result = match body {
            Some(json) => self
                .agent
                .post(url)
                .config()
                .timeout_global(overall)
                .build()
                .send_json(json),
            None => self
                .agent
                .get(url)
                .config()
                .timeout_global(overall)
                .build()
                .call(),
        };
        match result {
            Ok(response) => {
                let status = response.status().as_u16();
                log_request(method, url, &status.to_string());
                Ok((status, response))
            }
            Err(e) => {
                log_request(method, url, "error");
                Err(map_error(&e))
            }
        }
    }

    /// An API call: `None` for a 404 when `not_found_ok`, the capped body otherwise.
    fn api(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
        cap: u64,
        not_found_ok: bool,
    ) -> Result<Option<Vec<u8>>, NetError> {
        let url = format!("{SILO_BASE}{path}");
        if !allowed_api_url(&url) {
            return Err(NetError::NotAllowed);
        }
        let (status, mut response) = self.send(method, &url, body, Some(CALL_TIMEOUT))?;
        if status == 404 && not_found_ok {
            return Ok(None);
        }
        check_status(status, response.headers(), SystemTime::now())?;
        read_capped(response.body_mut(), cap).map(Some)
    }
}

impl Catalog for Http {
    fn lookup(&self, tech_names: &[String]) -> Result<Vec<LookupResult>, NetError> {
        let mut seen = HashSet::new();
        let names: Vec<&str> = tech_names
            .iter()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty() && seen.insert(n.to_lowercase()))
            .collect();
        if names.len() > LOOKUP_BATCH {
            return Err(NetError::BadAnswer("batchTooLarge".to_string()));
        }
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let body = serde_json::json!({ "techNames": names });
        let bytes = self
            .api("POST", "/mods/lookup", Some(&body), LOOKUP_CAP, false)?
            .unwrap_or_default();
        parse_lookup(&bytes)
    }

    fn by_hash(&self, hashes: &[String]) -> Result<Option<HashAnswers>, NetError> {
        let asked = clean_hashes(hashes)?;
        if asked.len() > BY_HASH_BATCH {
            return Err(NetError::BadAnswer("batchTooLarge".to_string()));
        }
        if asked.is_empty() {
            return Ok(Some(Vec::new()));
        }
        let body = serde_json::json!({ "hashes": asked });
        let Some(bytes) = self.api("POST", "/mods/by-hash", Some(&body), BY_HASH_CAP, true)? else {
            return Ok(None);
        };
        parse_by_hash(&bytes, &asked).map(Some)
    }

    fn detail(&self, id: &str) -> Result<Option<CatalogMod>, NetError> {
        if id.is_empty() || id.len() > 128 {
            return Err(NetError::BadAnswer("id".to_string()));
        }
        let path = format!("/mods/{}", encode_segment(id));
        match self.api("GET", &path, None, DETAIL_CAP, true)? {
            Some(bytes) => parse_detail(&bytes).map(Some),
            None => Ok(None),
        }
    }
}

impl Web for Http {
    fn fetch_page(&self, url: &str) -> Result<String, NetError> {
        let mut current = url.to_string();
        let mut followed = 0;
        loop {
            if !allowed_page_url(&current) {
                return Err(NetError::NotAllowed);
            }
            let (status, mut response) = self.send("GET", &current, None, Some(CALL_TIMEOUT))?;
            let location = location_of(response.headers());
            match redirect_step(
                status,
                location,
                &current,
                followed,
                MAX_PAGE_HOPS,
                allowed_page_url,
            ) {
                Step::Follow(next) => {
                    current = next;
                    followed += 1;
                    continue;
                }
                Step::Fail(e) => return Err(e),
                Step::Done => {}
            }
            check_status(status, response.headers(), SystemTime::now())?;
            let bytes = read_capped(response.body_mut(), PAGE_CAP)?;
            return Ok(String::from_utf8_lossy(&bytes).into_owned());
        }
    }

    fn download(
        &self,
        url: &str,
        sink: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
        cancel: &AtomicBool,
    ) -> Result<u64, NetError> {
        let mut current = url.to_string();
        let mut followed = 0;
        loop {
            if !allowed_download_url(&current) {
                return Err(NetError::NotAllowed);
            }
            if cancel.load(Ordering::SeqCst) {
                return Err(NetError::Cancelled);
            }
            let (status, mut response) = self.send("GET", &current, None, None)?;
            let location = location_of(response.headers());
            match redirect_step(
                status,
                location,
                &current,
                followed,
                MAX_DOWNLOAD_HOPS,
                allowed_download_url,
            ) {
                Step::Follow(next) => {
                    current = next;
                    followed += 1;
                    continue;
                }
                Step::Fail(e) => return Err(e),
                Step::Done => {}
            }
            check_status(status, response.headers(), SystemTime::now())?;
            return stream(&mut response.body_mut().as_reader(), sink, progress, cancel);
        }
    }
}

/// Copies a body into `sink` in chunks, checking `cancel` before every read (the last included).
pub(super) fn stream(
    reader: &mut dyn Read,
    sink: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> Result<u64, NetError> {
    let mut buf = vec![0u8; CHUNK];
    let mut total = 0u64;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(NetError::Cancelled);
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => return Ok(total),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(NetError::Offline),
        };
        sink.write_all(&buf[..n])
            .map_err(|_| NetError::BadAnswer("sinkWrite".to_string()))?;
        total += n as u64;
        progress(n as u64);
    }
}

/// Reads at most `cap` bytes; a longer body is an error, never a truncated answer.
fn read_capped(body: &mut Body, cap: u64) -> Result<Vec<u8>, NetError> {
    let mut out = Vec::new();
    body.as_reader()
        .take(cap + 1)
        .read_to_end(&mut out)
        .map_err(|_| NetError::Offline)?;
    if out.len() as u64 > cap {
        return Err(NetError::BadAnswer("tooLarge".to_string()));
    }
    Ok(out)
}

/// Transport failures are `Offline`; an address the agent refuses is `NotAllowed`.
fn map_error(e: &ureq::Error) -> NetError {
    match e {
        ureq::Error::StatusCode(n) => NetError::Status(*n),
        ureq::Error::BadUri(_)
        | ureq::Error::TooManyRedirects
        | ureq::Error::RequireHttpsOnly(_) => NetError::NotAllowed,
        _ => NetError::Offline,
    }
}

/// Debug builds only: method, host and path, never a body or a query (GitHub's redirect URLs
/// carry signed tokens in theirs).
fn log_request(method: &str, url: &str, outcome: &str) {
    if cfg!(debug_assertions) {
        let target = url::split(url, true)
            .map_or_else(|| "?".to_string(), |p| format!("{}{}", p.host, p.path));
        eprintln!("[net] {method} {target} -> {outcome}");
    }
}

// ---------------------------------------------------------------- statuses and headers

/// 2xx is fine; 429 is `RateLimited` with the wait the headers ask for; anything else is
/// `Status`.
pub(super) fn check_status(
    status: u16,
    headers: &HeaderMap,
    now: SystemTime,
) -> Result<(), NetError> {
    match status {
        200..=299 => Ok(()),
        429 => Err(NetError::RateLimited {
            retry_after: retry_after_from(headers, now),
        }),
        other => Err(NetError::Status(other)),
    }
}

fn header_text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

fn location_of(headers: &HeaderMap) -> Option<&str> {
    header_text(headers, "location")
}

/// When a 429 says to come back: `Retry-After` (seconds or an HTTP date), else
/// `x-ratelimit-reset` (seconds from now, or a Unix time when it is that large). Never more than
/// a day away.
pub(super) fn retry_after_from(headers: &HeaderMap, now: SystemTime) -> Option<SystemTime> {
    let at = header_text(headers, "retry-after")
        .and_then(|v| parse_wait(v, now))
        .or_else(|| header_text(headers, "x-ratelimit-reset").and_then(|v| parse_wait(v, now)))?;
    Some(at.min(now + MAX_RETRY_WAIT))
}

fn parse_wait(value: &str, now: SystemTime) -> Option<SystemTime> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<f64>() {
        if !seconds.is_finite() || seconds < 0.0 {
            return None;
        }
        // A reset header may hold a Unix time instead of a delay.
        return Some(if seconds >= 1_000_000_000.0 {
            UNIX_EPOCH + Duration::from_secs_f64(seconds)
        } else {
            now + Duration::from_secs_f64(seconds)
        });
    }
    parse_http_date(value)
}

/// `Sun, 06 Nov 1994 08:49:37 GMT`, the form HTTP servers send.
pub(super) fn parse_http_date(text: &str) -> Option<SystemTime> {
    let mut words = text.split_whitespace();
    let _weekday = words.next()?.strip_suffix(',')?;
    let day: i64 = words.next()?.parse().ok()?;
    let month_name = words.next()?;
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| m.eq_ignore_ascii_case(month_name))? as i64
        + 1;
    let year: i64 = words.next()?.parse().ok()?;
    let mut clock = words.next()?.split(':');
    let hour: i64 = clock.next()?.parse().ok()?;
    let minute: i64 = clock.next()?.parse().ok()?;
    let second: i64 = clock.next()?.parse().ok()?;
    if !words.next()?.eq_ignore_ascii_case("GMT")
        || words.next().is_some()
        || !(1..=31).contains(&day)
        || !(0..24).contains(&hour)
        || !(0..60).contains(&minute)
        || !(0..61).contains(&second)
        || year < 1970
    {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let seconds = u64::try_from(days * 86_400 + hour * 3600 + minute * 60 + second).ok()?;
    Some(UNIX_EPOCH + Duration::from_secs(seconds))
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

// ---------------------------------------------------------------- redirects

/// What to do with a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Step {
    /// Not a redirect: the caller looks at the status.
    Done,
    /// Ask again at this (allowed) address.
    Follow(String),
    Fail(NetError),
}

/// The redirect decision. A 301/302/303/307/308 with a `Location` is followed only to an
/// address `allowed` accepts and only `max_hops` times; a redirect without a `Location` is a
/// `Status`; one that leaves the list, or comes after too many hops, is `NotAllowed`.
pub(super) fn redirect_step(
    status: u16,
    location: Option<&str>,
    current: &str,
    followed: usize,
    max_hops: usize,
    allowed: fn(&str) -> bool,
) -> Step {
    if !matches!(status, 301 | 302 | 303 | 307 | 308) {
        return Step::Done;
    }
    let Some(location) = location else {
        return Step::Fail(NetError::Status(status));
    };
    if followed >= max_hops {
        return Step::Fail(NetError::NotAllowed);
    }
    match url::resolve_location(current, location) {
        Some(next) if allowed(&next) => Step::Follow(next),
        _ => Step::Fail(NetError::NotAllowed),
    }
}

// ---------------------------------------------------------------- answers

fn parse_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, NetError> {
    serde_json::from_slice(bytes)
        .map_err(|e| NetError::BadAnswer(format!("json {:?}", e.classify())))
}

/// A catalog id, which the API may send as text or as a number.
#[derive(Deserialize)]
#[serde(untagged)]
enum IdValue {
    Text(String),
    Number(i64),
}

impl IdValue {
    fn into_text(self) -> String {
        match self {
            IdValue::Text(text) => text,
            IdValue::Number(number) => number.to_string(),
        }
    }
}

#[derive(Deserialize)]
struct LookupAnswer {
    results: Vec<LookupRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LookupRow {
    #[serde(default)]
    tech_name: Option<String>,
    id: IdValue,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    latest_version: Option<String>,
    #[serde(default)]
    download: Option<DownloadRow>,
    #[serde(default)]
    page_url: Option<String>,
}

#[derive(Deserialize)]
struct DownloadRow {
    #[serde(default)]
    source: String,
    #[serde(default)]
    url: String,
}

pub(super) fn parse_lookup(bytes: &[u8]) -> Result<Vec<LookupResult>, NetError> {
    let answer: LookupAnswer = parse_json(bytes)?;
    Ok(answer
        .results
        .into_iter()
        .map(|row| LookupResult {
            tech_name: row.tech_name,
            id: row.id.into_text(),
            title: row.title.unwrap_or_default(),
            latest_version: row.latest_version.filter(|v| !v.trim().is_empty()),
            download: row
                .download
                .filter(|d| !d.url.is_empty())
                .map(|d| Download {
                    source: d.source,
                    url: d.url,
                }),
            page_url: row.page_url.filter(|u| !u.is_empty()),
        })
        .collect())
}

/// Hashes as the API wants them: trimmed, lower-case, distinct, 64 hex digits.
fn clean_hashes(hashes: &[String]) -> Result<Vec<String>, NetError> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for hash in hashes {
        let hash = hash.trim().to_ascii_lowercase();
        if hash.is_empty() {
            continue;
        }
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(NetError::BadAnswer("hash".to_string()));
        }
        if seen.insert(hash.clone()) {
            out.push(hash);
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
struct HashAnswer {
    results: Vec<HashRow>,
}

#[derive(Deserialize)]
struct HashRow {
    sha256: String,
    #[serde(default)]
    matches: Vec<MatchRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchRow {
    id: IdValue,
    #[serde(default)]
    tech_name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    file_name: Option<String>,
    #[serde(default)]
    download_url: Option<String>,
}

/// Every hash asked must come back exactly once and nothing else may: an answer that omits a
/// hash, adds one or repeats one is `BadAnswer`. The entries are returned in the order asked.
pub(super) fn parse_by_hash(bytes: &[u8], asked: &[String]) -> Result<HashAnswers, NetError> {
    let answer: HashAnswer = parse_json(bytes)?;
    let mut by_hash: HashMap<String, Vec<HashMatch>> = HashMap::new();
    for row in answer.results {
        let hash = row.sha256.trim().to_ascii_lowercase();
        let matches = row
            .matches
            .into_iter()
            .map(|m| HashMatch {
                id: m.id.into_text(),
                tech_name: m.tech_name,
                version: m.version.unwrap_or_default(),
                source: m.source.unwrap_or_default(),
                file_name: m.file_name,
                download_url: m.download_url,
            })
            .collect();
        if !asked.contains(&hash) || by_hash.insert(hash, matches).is_some() {
            return Err(NetError::BadAnswer("hashAnswer".to_string()));
        }
    }
    asked
        .iter()
        .map(|hash| {
            by_hash
                .remove(hash)
                .map(|matches| (hash.clone(), matches))
                .ok_or_else(|| NetError::BadAnswer("hashAnswer".to_string()))
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetailRow {
    id: IdValue,
    #[serde(default)]
    tech_name: Option<String>,
    #[serde(default)]
    latest_version: Option<String>,
    #[serde(default)]
    archive_sha256: Option<String>,
    #[serde(default)]
    page_url: Option<String>,
    #[serde(default)]
    sources: Vec<SourceRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceRow {
    #[serde(default)]
    source: String,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    installable: bool,
    #[serde(default)]
    download_url: Option<String>,
}

pub(super) fn parse_detail(bytes: &[u8]) -> Result<CatalogMod, NetError> {
    let row: DetailRow = parse_json(bytes)?;
    Ok(CatalogMod {
        id: row.id.into_text(),
        tech_name: row.tech_name,
        latest_version: row.latest_version.filter(|v| !v.trim().is_empty()),
        archive_sha256: row
            .archive_sha256
            .map(|h| h.trim().to_ascii_lowercase())
            .filter(|h| !h.is_empty()),
        page_url: row.page_url.filter(|u| !u.is_empty()),
        sources: row
            .sources
            .into_iter()
            .map(|s| CatalogSource {
                source: s.source.to_ascii_lowercase(),
                source_url: s.source_url.filter(|u| !u.is_empty()),
                version: s.version.filter(|v| !v.trim().is_empty()),
                installable: s.installable,
                download_url: s.download_url.filter(|u| !u.is_empty()),
            })
            .collect(),
    })
}
