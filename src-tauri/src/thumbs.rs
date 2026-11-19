//! The `thumb` protocol (plan M3 decision 4): `http://thumb.localhost/<profileId>/<modId>?v=..`
//! returns the mod's icon as a PNG thumbnail. Ids are resolved through the library's last
//! scan, so no request can make the app read an arbitrary path.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use fsmm_core::library::decode_segment;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::AppState;

#[cfg(windows)]
pub const BASE_URL: &str = "http://thumb.localhost";
#[cfg(not(windows))]
pub const BASE_URL: &str = "thumb://localhost";

pub fn handle<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let library = ctx.app_handle().state::<AppState>().library.clone();
    let path = request.uri().path().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let ids = parse(&path);
        let png = ids.and_then(|(profile, id)| library.thumbnail(&profile, &id).ok());
        log_timing(started);
        responder.respond(
            match png {
                Some(png) => Response::builder()
                    .header(header::CONTENT_TYPE, "image/png")
                    // The URL carries the file's size and mtime, so a cached response never goes stale.
                    .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
                    .body(png),
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new()),
            }
            .unwrap_or_else(|_| Response::new(Vec::new())),
        );
    });
}

/// `/<profileId>/<modId>` with both segments percent-encoded.
fn parse(path: &str) -> Option<(String, String)> {
    let mut parts = path.trim_start_matches('/').split('/');
    let profile = decode_segment(parts.next()?)?;
    let id = decode_segment(parts.next()?)?;
    parts.next().is_none().then_some((profile, id))
}

/// Debug builds print an aggregate every 50 thumbnails: decoding takes milliseconds, a cache
/// hit well under one, so the average shows whether the cache is working.
fn log_timing(started: Instant) {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    static MICROS: AtomicU64 = AtomicU64::new(0);
    if !cfg!(debug_assertions) {
        return;
    }
    let micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let total = MICROS.fetch_add(micros, Ordering::Relaxed) + micros;
    let count = COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    if count.is_multiple_of(50) {
        eprintln!(
            "[timing] thumbnails: {count} served, average {:.2} ms",
            total as f64 / count as f64 / 1000.0
        );
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_two_encoded_segments() {
        assert_eq!(
            parse("/p1/FS25_A%20%282%29.zip"),
            Some(("p1".to_string(), "FS25_A (2).zip".to_string()))
        );
        assert_eq!(parse("/p1"), None);
        assert_eq!(parse("/p1/a/b"), None);
    }
}
