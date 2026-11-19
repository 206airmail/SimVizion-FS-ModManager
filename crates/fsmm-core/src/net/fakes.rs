//! Test doubles for the catalog and the web. They answer from canned data, keep a log of every
//! request (so tests can count them and check batching) and can be told to fail. Nothing here
//! touches the network.

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use super::{
    Catalog, CatalogMod, HashAnswers, HashMatch, LookupResult, NetError, Web, BY_HASH_BATCH,
    LOOKUP_BATCH,
};

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One request the catalog received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogCall {
    Lookup(Vec<String>),
    ByHash(Vec<String>),
    Detail(String),
}

/// A catalog with canned answers. Names are matched case-insensitively.
#[derive(Default)]
pub struct FakeCatalog {
    rows: Mutex<HashMap<String, LookupResult>>,
    hashes: Mutex<HashMap<String, Vec<HashMatch>>>,
    details: Mutex<HashMap<String, CatalogMod>>,
    no_by_hash_route: AtomicBool,
    calls: Mutex<Vec<CatalogCall>>,
    /// Errors for the next requests, whatever they are, oldest first.
    errors: Mutex<VecDeque<NetError>>,
    /// An error for the request with this index (0 = the first request ever made).
    fail_at: Mutex<HashMap<usize, NetError>>,
    /// Every request fails with this.
    fail_all: Mutex<Option<NetError>>,
}

impl FakeCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// `lookup` returns this row for its `tech_name`.
    pub fn with_row(self, row: LookupResult) -> Self {
        let key = row.tech_name.clone().unwrap_or_default().to_lowercase();
        lock(&self.rows).insert(key, row);
        self
    }

    /// `by_hash` returns these builds for the hash.
    pub fn with_hash(self, hash: &str, matches: Vec<HashMatch>) -> Self {
        lock(&self.hashes).insert(hash.to_ascii_lowercase(), matches);
        self
    }

    pub fn with_detail(self, detail: CatalogMod) -> Self {
        lock(&self.details).insert(detail.id.clone(), detail);
        self
    }

    /// `by_hash` answers `Ok(None)`, as a server without the route does.
    pub fn without_by_hash(self) -> Self {
        self.no_by_hash_route.store(true, Ordering::SeqCst);
        self
    }

    /// The next request, whatever it is, fails with this.
    pub fn fail_next(&self, error: NetError) {
        lock(&self.errors).push_back(error);
    }

    /// The request with this index (counting every request since the fake was made, from 0)
    /// fails with this.
    pub fn fail_request(&self, index: usize, error: NetError) {
        lock(&self.fail_at).insert(index, error);
    }

    /// Every request from now on fails with this.
    pub fn fail_all(&self, error: NetError) {
        *lock(&self.fail_all) = Some(error);
    }

    pub fn calls(&self) -> Vec<CatalogCall> {
        lock(&self.calls).clone()
    }

    pub fn request_count(&self) -> usize {
        lock(&self.calls).len()
    }

    pub fn lookups(&self) -> Vec<Vec<String>> {
        self.calls()
            .into_iter()
            .filter_map(|c| match c {
                CatalogCall::Lookup(names) => Some(names),
                _ => None,
            })
            .collect()
    }

    pub fn by_hashes(&self) -> Vec<Vec<String>> {
        self.calls()
            .into_iter()
            .filter_map(|c| match c {
                CatalogCall::ByHash(hashes) => Some(hashes),
                _ => None,
            })
            .collect()
    }

    pub fn details(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter_map(|c| match c {
                CatalogCall::Detail(id) => Some(id),
                _ => None,
            })
            .collect()
    }

    /// Records the request and returns the error it is to fail with, if any.
    fn begin(&self, call: CatalogCall) -> Result<(), NetError> {
        let index = {
            let mut calls = lock(&self.calls);
            calls.push(call);
            calls.len() - 1
        };
        if let Some(error) = lock(&self.fail_all).clone() {
            return Err(error);
        }
        if let Some(error) = lock(&self.fail_at).remove(&index) {
            return Err(error);
        }
        match lock(&self.errors).pop_front() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Catalog for FakeCatalog {
    fn lookup(&self, tech_names: &[String]) -> Result<Vec<LookupResult>, NetError> {
        assert!(
            tech_names.len() <= LOOKUP_BATCH,
            "a lookup batch is at most {LOOKUP_BATCH} names"
        );
        assert!(
            tech_names.iter().all(|n| !n.trim().is_empty()),
            "a blank name fails a real batch"
        );
        self.begin(CatalogCall::Lookup(tech_names.to_vec()))?;
        let rows = lock(&self.rows);
        Ok(tech_names
            .iter()
            .filter_map(|name| rows.get(&name.to_lowercase()).cloned())
            .collect())
    }

    fn by_hash(&self, hashes: &[String]) -> Result<Option<HashAnswers>, NetError> {
        assert!(
            hashes.len() <= BY_HASH_BATCH,
            "a by-hash batch is at most {BY_HASH_BATCH} hashes"
        );
        self.begin(CatalogCall::ByHash(hashes.to_vec()))?;
        if self.no_by_hash_route.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let known = lock(&self.hashes);
        Ok(Some(
            hashes
                .iter()
                .map(|h| (h.clone(), known.get(h).cloned().unwrap_or_default()))
                .collect(),
        ))
    }

    fn detail(&self, id: &str) -> Result<Option<CatalogMod>, NetError> {
        self.begin(CatalogCall::Detail(id.to_string()))?;
        Ok(lock(&self.details).get(id).cloned())
    }
}

/// One request the web received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebCall {
    Page(String),
    Download(String),
}

/// Pages and download payloads served from memory.
pub struct FakeWeb {
    pages: Mutex<HashMap<String, String>>,
    payloads: Mutex<HashMap<String, Vec<u8>>>,
    calls: Mutex<Vec<WebCall>>,
    /// The `referer` of each download, in order.
    referers: Mutex<Vec<Option<String>>>,
    errors: Mutex<VecDeque<NetError>>,
    /// Bytes handed to the sink per step; progress and the cancel flag are per step.
    chunk: usize,
    /// After this many steps of a download the fake raises the cancel flag, as the user would.
    cancel_after_chunks: Mutex<Option<usize>>,
    /// The download fails with this error after the first chunk (a dropped connection).
    fail_mid_download: Mutex<Option<NetError>>,
}

impl Default for FakeWeb {
    fn default() -> Self {
        Self {
            pages: Mutex::default(),
            payloads: Mutex::default(),
            calls: Mutex::default(),
            referers: Mutex::default(),
            errors: Mutex::default(),
            chunk: 1024,
            cancel_after_chunks: Mutex::default(),
            fail_mid_download: Mutex::default(),
        }
    }
}

impl FakeWeb {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_page(self, url: &str, html: &str) -> Self {
        lock(&self.pages).insert(url.to_string(), html.to_string());
        self
    }

    pub fn with_payload(self, url: &str, bytes: Vec<u8>) -> Self {
        lock(&self.payloads).insert(url.to_string(), bytes);
        self
    }

    pub fn with_chunk(mut self, chunk: usize) -> Self {
        self.chunk = chunk;
        self
    }

    /// The next request, page or download, fails with this.
    pub fn fail_next(&self, error: NetError) {
        lock(&self.errors).push_back(error);
    }

    /// The cancel flag is raised after this many chunks of the next download.
    pub fn cancel_after_chunks(&self, chunks: usize) {
        *lock(&self.cancel_after_chunks) = Some(chunks);
    }

    /// The next download breaks off after its first chunk with this error.
    pub fn fail_mid_download(&self, error: NetError) {
        *lock(&self.fail_mid_download) = Some(error);
    }

    pub fn calls(&self) -> Vec<WebCall> {
        lock(&self.calls).clone()
    }

    pub fn downloads(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter_map(|c| match c {
                WebCall::Download(url) => Some(url),
                WebCall::Page(_) => None,
            })
            .collect()
    }

    pub fn download_referers(&self) -> Vec<Option<String>> {
        lock(&self.referers).clone()
    }

    pub fn pages_fetched(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter_map(|c| match c {
                WebCall::Page(url) => Some(url),
                WebCall::Download(_) => None,
            })
            .collect()
    }
}

impl Web for FakeWeb {
    fn fetch_page(&self, url: &str) -> Result<String, NetError> {
        lock(&self.calls).push(WebCall::Page(url.to_string()));
        if let Some(error) = lock(&self.errors).pop_front() {
            return Err(error);
        }
        lock(&self.pages)
            .get(url)
            .cloned()
            .ok_or(NetError::Status(404))
    }

    fn download(
        &self,
        url: &str,
        referer: Option<&str>,
        sink: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
        cancel: &AtomicBool,
    ) -> Result<u64, NetError> {
        lock(&self.calls).push(WebCall::Download(url.to_string()));
        lock(&self.referers).push(referer.map(str::to_string));
        if let Some(error) = lock(&self.errors).pop_front() {
            return Err(error);
        }
        let payload = lock(&self.payloads)
            .get(url)
            .cloned()
            .ok_or(NetError::Status(404))?;
        let cancel_after = lock(&self.cancel_after_chunks).take();
        let fail_after_first = lock(&self.fail_mid_download).take();
        let mut written = 0u64;
        for (step, chunk) in payload.chunks(self.chunk.max(1)).enumerate() {
            if cancel.load(Ordering::SeqCst) {
                return Err(NetError::Cancelled);
            }
            if step == 1 {
                if let Some(error) = &fail_after_first {
                    return Err(error.clone());
                }
            }
            sink.write_all(chunk)
                .map_err(|_| NetError::BadAnswer("sinkWrite".to_string()))?;
            written += chunk.len() as u64;
            progress(chunk.len() as u64);
            if cancel_after == Some(step + 1) {
                cancel.store(true, Ordering::SeqCst);
            }
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(NetError::Cancelled);
        }
        Ok(written)
    }
}
