//! The network layer without a network: batching against fake catalogs, versions, the allow-lists,
//! the ModHub page parser against a saved real page, header parsing, the redirect decision and
//! the answer parsers. The real `Http` is only exercised on paths that return before any request.

use std::io::Cursor;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ureq::http::{HeaderMap, HeaderValue};

use super::fakes::{CatalogCall, FakeCatalog};
use super::http::{
    check_status, parse_by_hash, parse_detail, parse_http_date, parse_lookup, redirect_step,
    retry_after_from, Step,
};
use super::*;

const PAGE: &str = include_str!("fixtures/modhub_page.html");
const ID: &str = "331515";

fn names(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("FS25_Mod{i}")).collect()
}

fn hash(i: usize) -> String {
    format!("{i:064x}")
}

fn at(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

// ---------------------------------------------------------------- versions

#[test]
fn versions_compare_like_silo() {
    for (a, b) in [
        ("v1.2.0", "1.2"),
        ("1.2.0.0", "1.2"),
        ("V.1.1.5.0", "1.1.5.0"),
        ("v8.1.0.3", "8.1.0.3"),
        ("1.02", "1.2"),
        ("1-2_3+4", "1.2.3.4"),
        ("1.2.3-beta", "1.2.3.0"),
        ("  1.0 ", "1.0.0"),
    ] {
        assert!(versions_equal(a, b), "{a} == {b}");
        assert!(versions_equal(b, a), "{b} == {a}");
        assert_eq!(is_newer(a, b), Some(false), "{a} is not newer than {b}");
    }
    for (a, b) in [
        ("1.2.1", "1.2"),
        ("1.10", "1.9"),
        ("2", "1.99.99"),
        ("v1.0.0.1", "1"),
    ] {
        assert!(!versions_equal(a, b), "{a} != {b}");
        assert_eq!(is_newer(a, b), Some(true), "{a} is newer than {b}");
        assert_eq!(is_newer(b, a), Some(false), "{b} is not newer than {a}");
    }
}

#[test]
fn versions_that_do_not_start_with_a_digit_say_nothing() {
    for odd in ["", "beta", "x1.0", "v", "vv1", ".1.2", "..", "-1"] {
        assert_eq!(is_newer(odd, "1.0"), None, "{odd:?}");
        assert_eq!(is_newer("1.0", odd), None, "{odd:?}");
        assert!(!versions_equal(odd, "1.0"), "{odd:?}");
    }
    // They still match themselves exactly.
    assert!(versions_equal("beta", "beta"));
    assert!(!versions_equal("beta", "Beta"));
    // A number that does not fit is not a version.
    assert_eq!(is_newer("99999999999999999999999.1", "1.0"), None);
}

// ---------------------------------------------------------------- batching

#[test]
fn a_thousand_and_one_names_make_exactly_three_lookups() {
    let catalog = FakeCatalog::new();
    lookup_all(&catalog, &names(1001)).unwrap();
    let sizes: Vec<usize> = catalog.lookups().iter().map(Vec::len).collect();
    assert_eq!(sizes, [500, 500, 1]);
    assert_eq!(catalog.request_count(), 3);
}

#[test]
fn lookup_drops_blanks_and_case_insensitive_repeats_and_asks_nothing_when_empty() {
    let catalog = FakeCatalog::new();
    let asked = [
        "FS25_A".to_string(),
        " ".to_string(),
        "fs25_a".to_string(),
        String::new(),
        " FS25_B ".to_string(),
        "FS25_A".to_string(),
    ];
    lookup_all(&catalog, &asked).unwrap();
    assert_eq!(catalog.lookups(), [["FS25_A", "FS25_B"]]);

    let empty = FakeCatalog::new();
    assert!(lookup_all(&empty, &[]).unwrap().is_empty());
    assert!(lookup_all(&empty, &[" ".to_string()]).unwrap().is_empty());
    assert_eq!(empty.request_count(), 0);
}

#[test]
fn lookup_returns_the_rows_of_every_batch() {
    let rows = (0..600).map(|i| LookupResult {
        tech_name: Some(format!("FS25_Mod{i}")),
        id: i.to_string(),
        title: String::new(),
        latest_version: None,
        download: None,
        page_url: None,
    });
    let catalog = rows.fold(FakeCatalog::new(), FakeCatalog::with_row);
    let found = lookup_all(&catalog, &names(600)).unwrap();
    assert_eq!(found.len(), 600);
}

#[test]
fn lookup_stops_at_the_first_error() {
    let catalog = FakeCatalog::new();
    catalog.fail_request(1, NetError::Status(500));
    let error = lookup_all(&catalog, &names(1500)).unwrap_err();
    assert_eq!(error, NetError::Status(500));
    assert_eq!(
        catalog.request_count(),
        2,
        "no third request after the failure"
    );
}

#[test]
fn by_hash_batches_of_a_hundred_lowercased_and_distinct() {
    let catalog = FakeCatalog::new();
    let mut asked: Vec<String> = (0..250).map(hash).collect();
    asked.push(hash(3).to_uppercase());
    asked.push("  ".to_string());
    let answers = by_hash_all(&catalog, &asked).unwrap().unwrap();
    let sizes: Vec<usize> = catalog.by_hashes().iter().map(Vec::len).collect();
    assert_eq!(sizes, [100, 100, 50]);
    assert_eq!(answers.len(), 250);
    assert!(catalog
        .by_hashes()
        .concat()
        .iter()
        .all(|h| *h == h.to_lowercase()));
}

#[test]
fn by_hash_stops_at_the_first_error_and_asks_nothing_for_nothing() {
    let catalog = FakeCatalog::new();
    catalog.fail_request(1, NetError::Offline);
    let asked: Vec<String> = (0..300).map(hash).collect();
    assert_eq!(
        by_hash_all(&catalog, &asked).unwrap_err(),
        NetError::Offline
    );
    assert_eq!(catalog.request_count(), 2);

    let idle = FakeCatalog::new();
    assert_eq!(by_hash_all(&idle, &[]).unwrap(), Some(Vec::new()));
    assert_eq!(idle.request_count(), 0);
}

#[test]
fn by_hash_without_a_route_is_none() {
    let catalog = FakeCatalog::new().without_by_hash();
    assert_eq!(by_hash_all(&catalog, &[hash(1)]).unwrap(), None);
}

#[test]
fn the_fake_catalog_logs_every_kind_of_request() {
    let catalog = FakeCatalog::new();
    lookup_all(&catalog, &names(1)).unwrap();
    by_hash_all(&catalog, &[hash(1)]).unwrap();
    assert!(catalog.detail("7").unwrap().is_none());
    assert_eq!(
        catalog.calls(),
        [
            CatalogCall::Lookup(names(1)),
            CatalogCall::ByHash(vec![hash(1)]),
            CatalogCall::Detail("7".to_string()),
        ]
    );
}

// ---------------------------------------------------------------- allow-lists

#[test]
fn only_the_catalog_host_is_an_api_url() {
    assert!(allowed_api_url("https://silo-api.hllmr.com/mods/lookup"));
    assert!(allowed_api_url("https://SILO-API.hllmr.com"));
    for bad in [
        "http://silo-api.hllmr.com/mods",
        "https://silo-api.hllmr.com.evil.com/mods",
        "https://evil.com/silo-api.hllmr.com",
        "https://user@silo-api.hllmr.com/mods",
        "https://silo-api.hllmr.com:8443/mods",
        "https://silo-api.hllmr.com\\@evil.com/",
        "https://silo-api.hllmr.com /mods",
        "ftp://silo-api.hllmr.com/mods",
        "//silo-api.hllmr.com/mods",
        "",
    ] {
        assert!(!allowed_api_url(bad), "{bad}");
    }
}

#[test]
fn only_modhub_mod_pages_are_pages() {
    assert!(allowed_page_url(
        "https://www.farming-simulator.com/mod.php?mod_id=331515&title=fs2025"
    ));
    assert!(allowed_page_url(
        "https://www.farming-simulator.com/mod.php"
    ));
    for bad in [
        "https://www.farming-simulator.com/mods.php?title=fs2025",
        "https://www.farming-simulator.com/other",
        "https://farming-simulator.com/mod.php?mod_id=1",
        "http://www.farming-simulator.com/mod.php?mod_id=1",
        "https://www.farming-simulator.com.evil.com/mod.php?mod_id=1",
        "https://evil@www.farming-simulator.com/mod.php?mod_id=1",
        "https://www.farming-simulator.com/mod.php/../x",
        "https://evil.com/?u=https://www.farming-simulator.com/mod.php",
    ] {
        assert!(!allowed_page_url(bad), "{bad}");
    }
}

#[test]
fn only_known_download_hosts_are_downloads() {
    for good in [
        "https://github.com/owner/repo/releases/download/v1/FS25_X.zip",
        "https://objects.githubusercontent.com/github-production-release-asset/1?sig=x",
        "https://release-assets.githubusercontent.com/github-production/2",
        "https://cdn27.giants-software.com/modHub/storage/00331515/FS25_Courseplay.zip",
        "https://cdn1.giants-software.com/x",
        "https://CDN27.Giants-Software.com/x",
    ] {
        assert!(allowed_download_url(good), "{good}");
    }
    for bad in [
        "http://github.com/owner/repo/x.zip",
        "https://giants-software.com.evil.com/x.zip",
        "https://cdn27.giants-software.com.evil.com/x.zip",
        "https://cdnx.giants-software.com/x.zip",
        "https://cdn.giants-software.com/x.zip",
        "https://www.giants-software.com/x.zip",
        "https://evilgithub.com/x.zip",
        "https://github.com.evil.com/x.zip",
        "https://gist.github.com/x.zip",
        "https://user:pw@github.com/x.zip",
        "https://github.com:444/x.zip",
        "https://github.com@evil.com/x.zip",
        "https://evil.com#@github.com/x.zip",
        "https://evil.com\\@github.com/x.zip",
        "https://git\u{2024}hub.com/x.zip",
        "file:///c:/x.zip",
        "javascript:alert(1)",
        "",
    ] {
        assert!(!allowed_download_url(bad), "{bad}");
    }
}

// ---------------------------------------------------------------- the ModHub page

#[test]
fn the_saved_modhub_page_gives_its_download() {
    let found = modhub_download(PAGE, ID).unwrap();
    assert_eq!(
        found.url,
        "https://cdn27.giants-software.com/modHub/storage/00331515/FS25_Courseplay.zip"
    );
    assert_eq!(found.file_name, "FS25_Courseplay.zip");
    assert_eq!(found.version.as_deref(), Some("8.1.0.3"));
    // 3.79 MB, in 1024-based units.
    assert_eq!(found.approx_size, Some(3_974_103));
    assert!(allowed_download_url(&found.url));
}

#[test]
fn the_mod_id_may_be_given_with_its_padding() {
    assert!(modhub_download(PAGE, "00331515").is_some());
    assert!(modhub_download(PAGE, "").is_none());
    assert!(modhub_download(PAGE, "331515x").is_none());
    assert!(modhub_download(PAGE, "1331515111").is_none());
}

#[test]
fn another_mod_id_finds_nothing() {
    assert!(modhub_download(PAGE, "331516").is_none());
    assert!(modhub_download(PAGE, "31515").is_none());
}

#[test]
fn a_link_on_another_host_is_not_a_download() {
    for host in [
        "cdn27.giants-software.com.evil.com",
        "cdnx.giants-software.com",
        "evil.example",
        "giants-software.com",
    ] {
        let page = PAGE.replace(
            "cdn27.giants-software.com/modHub/storage/00331515/FS25_Courseplay.zip",
            &format!("{host}/modHub/storage/00331515/FS25_Courseplay.zip"),
        );
        assert!(modhub_download(&page, ID).is_none(), "{host}");
    }
    let http = PAGE.replace("href=\"https://cdn27", "href=\"http://cdn27");
    assert!(modhub_download(&http, ID).is_none());
}

#[test]
fn only_an_anchor_counts_never_an_image() {
    let img_only = PAGE.replace(
        "<a href=\"https://cdn27.giants-software.com/modHub/storage/00331515/FS25_Courseplay.zip\" class=\"button button-buy button-middle button-no-margin expanded\">DOWNLOAD</a>",
        "<img src=\"https://cdn27.giants-software.com/modHub/storage/00331515/FS25_Courseplay.zip\">",
    );
    assert!(img_only.contains("<img src=\"https://cdn27"));
    assert!(modhub_download(&img_only, ID).is_none());
}

#[test]
fn two_download_links_are_never_guessed_between() {
    let two = PAGE.replace(
        "DOWNLOAD</a>",
        "DOWNLOAD</a><a href=\"https://cdn28.giants-software.com/modHub/storage/00331515/FS25_Courseplay2.zip\">DOWNLOAD</a>",
    );
    assert!(modhub_download(&two, ID).is_none());
}

#[test]
fn a_page_without_a_link_or_with_an_odd_one_finds_nothing() {
    assert!(modhub_download("", ID).is_none());
    assert!(modhub_download("<html><body>no links</body></html>", ID).is_none());
    for href in [
        // Not a zip, no file, a deeper path, a query, a odd file name.
        "https://cdn27.giants-software.com/modHub/storage/00331515/FS25_Courseplay.rar",
        "https://cdn27.giants-software.com/modHub/storage/00331515/",
        "https://cdn27.giants-software.com/modHub/storage/00331515/a/FS25_Courseplay.zip",
        "https://cdn27.giants-software.com/modHub/storage/00331515/FS25_Courseplay.zip?x=1",
        "https://cdn27.giants-software.com/modHub/storage/331515/FS25_Courseplay.zip",
        "https://cdn27.giants-software.com/other/storage/00331515/FS25_Courseplay.zip",
        "https://cdn27.giants-software.com/modHub/storage/00331515/FS25 Courseplay.zip",
        "https://cdn27.giants-software.com/modHub/storage/00331515/.zip",
    ] {
        let html = format!("<a href=\"{href}\" class=\"button\">DOWNLOAD</a>");
        assert!(modhub_download(&html, ID).is_none(), "{href}");
    }
}

#[test]
fn a_link_with_single_quotes_or_an_amp_entity_in_it_is_read() {
    let html = "<a class='button' href='https://cdn27.giants-software.com/modHub/storage/00331515/FS25_X.zip'>DOWNLOAD</a>";
    let found = modhub_download(html, ID).unwrap();
    assert_eq!(found.file_name, "FS25_X.zip");
    assert_eq!(found.version, None);
    assert_eq!(found.approx_size, None);
    // `<abbr>` and `<article>` are not anchors.
    let html = "<abbr href='https://cdn27.giants-software.com/modHub/storage/00331515/FS25_X.zip'>";
    assert!(modhub_download(html, ID).is_none());
}

#[test]
fn modhub_sizes_are_read_in_binary_units() {
    let page = |size: &str| {
        format!(
            "<a href=\"https://cdn27.giants-software.com/modHub/storage/00331515/FS25_X.zip\">D</a>\
             <div class=\"table-cell\"><b>Size</b></div><div class=\"table-cell\">{size}</div>"
        )
    };
    let size = |text: &str| modhub_download(&page(text), ID).unwrap().approx_size;
    assert_eq!(size("1 KB"), Some(1024));
    assert_eq!(size("2 GB"), Some(2 * 1024 * 1024 * 1024));
    assert_eq!(size("850 B"), Some(850));
    assert_eq!(size("1,5 MB"), Some(1_572_864));
    assert_eq!(size("lots"), None);
    assert_eq!(size("3.79 TB"), None);
    assert_eq!(size(""), None);
}

#[test]
fn modhub_page_urls_give_their_mod_id() {
    assert_eq!(
        modhub_id("https://www.farming-simulator.com/mod.php?mod_id=331515&title=fs2025")
            .as_deref(),
        Some("331515")
    );
    assert_eq!(
        modhub_id("https://www.farming-simulator.com/mod.php?title=fs2025&mod_id=7").as_deref(),
        Some("7")
    );
    for bad in [
        "https://www.farming-simulator.com/mod.php?title=fs2025",
        "https://www.farming-simulator.com/mod.php",
        "https://www.farming-simulator.com/mod.php?mod_id=",
        "https://www.farming-simulator.com/mod.php?mod_id=12a",
        "https://www.farming-simulator.com/mod.php?mod_id=123456789",
        "https://www.farming-simulator.com/mod.php?mod_id=1&mod_id=2",
        "https://evil.com/mod.php?mod_id=1",
        "https://www.farming-simulator.com/other.php?mod_id=1",
        "http://www.farming-simulator.com/mod.php?mod_id=1",
    ] {
        assert_eq!(modhub_id(bad), None, "{bad}");
    }
}

#[test]
fn github_repos_are_read_from_https_urls_on_github_com() {
    let repo = |u: &str| github_repo(u);
    assert_eq!(
        repo("https://github.com/Courseplay/Courseplay_FS25"),
        Some(("Courseplay".to_string(), "Courseplay_FS25".to_string()))
    );
    assert_eq!(
        repo("https://github.com/o/r/releases/download/v1/FS25_X.zip?x=1"),
        Some(("o".to_string(), "r".to_string()))
    );
    assert_eq!(
        repo("https://github.com/o/r.git"),
        Some(("o".to_string(), "r".to_string()))
    );
    assert_eq!(
        repo("https://github.com/o/r/"),
        Some(("o".to_string(), "r".to_string()))
    );
    for bad in [
        "https://github.com/o",
        "https://github.com/",
        "https://github.com",
        "http://github.com/o/r",
        "https://gitlab.com/o/r",
        "https://github.com.evil.com/o/r",
        "https://u@github.com/o/r",
        "https://github.com/o/%20r",
        "https://github.com/o/.git",
    ] {
        assert_eq!(repo(bad), None, "{bad}");
    }
    assert!(github_repo_lenient("http://github.com/o/r").is_some());
    assert!(github_repo_lenient("http://github.com.evil.com/o/r").is_none());
}

#[test]
fn download_file_names_are_decoded_from_the_path() {
    assert_eq!(
        download_file_name("https://github.com/o/r/releases/download/v1/FS25_X.zip?a=b").as_deref(),
        Some("FS25_X.zip")
    );
    assert_eq!(
        download_file_name("https://github.com/o/r/FS25%20X.zip").as_deref(),
        Some("FS25 X.zip")
    );
    assert_eq!(download_file_name("https://github.com/o/r/"), None);
    assert_eq!(download_file_name("https://github.com/o/r/a%2Fb.zip"), None);
    assert_eq!(download_file_name("https://github.com/o/r/%ZZ"), None);
    // The user's own links may carry a port; the host is returned without it.
    assert_eq!(
        manual_link_host("https://Forum.Example.com:8080/x").as_deref(),
        Some("forum.example.com")
    );
    for bad in [
        "https://example.com:/x",
        "https://example.com:123456/x",
        "https://example.com:80a/x",
        "https://example.com:80:80/x",
        "https://u@example.com:80/x",
    ] {
        assert_eq!(manual_link_host(bad), None, "{bad}");
    }
    assert_eq!(
        manual_link_host("http://Forum.Example.com/x").as_deref(),
        Some("forum.example.com")
    );
}

// ---------------------------------------------------------------- headers and redirects

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(
            ureq::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_str(value).unwrap(),
        );
    }
    map
}

#[test]
fn retry_after_in_seconds_or_as_a_date() {
    let now = at(1_000_000_000);
    let seconds = headers(&[("retry-after", "120")]);
    assert_eq!(
        retry_after_from(&seconds, now),
        Some(now + Duration::from_secs(120))
    );
    let date = headers(&[("retry-after", "Sun, 06 Nov 1994 08:49:37 GMT")]);
    assert_eq!(
        retry_after_from(&date, at(784_111_000)),
        Some(at(784_111_777))
    );
    // A date a day or more away is cut to a day.
    assert_eq!(retry_after_from(&date, at(0)), Some(at(86_400)));
}

#[test]
fn the_rate_limit_reset_is_the_fallback_and_may_be_a_unix_time() {
    let now = at(1_800_000_000);
    let reset = headers(&[("x-ratelimit-reset", "42")]);
    assert_eq!(
        retry_after_from(&reset, now),
        Some(now + Duration::from_secs(42))
    );
    let unix = headers(&[("x-ratelimit-reset", "1800000090")]);
    assert_eq!(retry_after_from(&unix, now), Some(at(1_800_000_090)));
    // Retry-After wins over the reset header.
    let both = headers(&[("retry-after", "5"), ("x-ratelimit-reset", "42")]);
    assert_eq!(
        retry_after_from(&both, now),
        Some(now + Duration::from_secs(5))
    );
    // Junk in Retry-After falls back to the reset header.
    let junk = headers(&[("retry-after", "soon"), ("x-ratelimit-reset", "9")]);
    assert_eq!(
        retry_after_from(&junk, now),
        Some(now + Duration::from_secs(9))
    );
    assert_eq!(retry_after_from(&headers(&[]), now), None);
    assert_eq!(
        retry_after_from(&headers(&[("retry-after", "-5")]), now),
        None
    );
}

#[test]
fn a_server_cannot_ask_for_more_than_a_day() {
    let now = at(1_000_000_000);
    let long = headers(&[("retry-after", "99999999")]);
    assert_eq!(
        retry_after_from(&long, now),
        Some(now + Duration::from_secs(86_400))
    );
}

#[test]
fn http_dates_parse_only_in_the_imf_form() {
    assert_eq!(
        parse_http_date("Thu, 01 Jan 1970 00:00:00 GMT"),
        Some(at(0))
    );
    assert_eq!(
        parse_http_date("Wed, 21 Oct 2015 07:28:00 GMT"),
        Some(at(1_445_412_480))
    );
    for bad in [
        "",
        "Wed, 21 Oct 2015 07:28:00",
        "Wed, 21 Foo 2015 07:28:00 GMT",
        "Wed, 32 Oct 2015 07:28:00 GMT",
        "Wed, 21 Oct 2015 25:28:00 GMT",
        "Wed 21 Oct 2015 07:28:00 GMT",
        "Wed, 21 Oct 1969 07:28:00 GMT",
        "Wed, 21 Oct 2015 07:28:00 GMT extra",
    ] {
        assert_eq!(parse_http_date(bad), None, "{bad:?}");
    }
}

#[test]
fn statuses_map_to_errors() {
    let now = at(1_000);
    let h = headers(&[("retry-after", "30")]);
    assert_eq!(check_status(200, &h, now), Ok(()));
    assert_eq!(check_status(204, &h, now), Ok(()));
    assert_eq!(
        check_status(429, &h, now),
        Err(NetError::RateLimited {
            retry_after: Some(now + Duration::from_secs(30))
        })
    );
    assert_eq!(
        check_status(429, &headers(&[]), now),
        Err(NetError::RateLimited { retry_after: None })
    );
    for status in [404, 500, 503, 301, 100] {
        assert_eq!(check_status(status, &h, now), Err(NetError::Status(status)));
    }
}

#[test]
fn redirects_are_followed_only_within_the_list_and_only_five_times() {
    let from = "https://github.com/o/r/releases/download/v1/FS25_X.zip";
    let step = |status, location: Option<&str>, followed| {
        redirect_step(status, location, from, followed, 5, allowed_download_url)
    };
    assert_eq!(step(200, None, 0), Step::Done);
    assert_eq!(step(404, Some("https://evil.com/"), 0), Step::Done);
    assert_eq!(
        step(
            302,
            Some("https://objects.githubusercontent.com/a?sig=1"),
            0
        ),
        Step::Follow("https://objects.githubusercontent.com/a?sig=1".to_string())
    );
    assert_eq!(
        step(307, Some("/other/path.zip"), 4),
        Step::Follow("https://github.com/other/path.zip".to_string())
    );
    assert_eq!(
        step(301, Some("//objects.githubusercontent.com/x"), 0),
        Step::Follow("https://objects.githubusercontent.com/x".to_string())
    );
    assert_eq!(
        step(302, Some("https://objects.githubusercontent.com/a"), 5),
        Step::Fail(NetError::NotAllowed)
    );
    assert_eq!(step(302, None, 0), Step::Fail(NetError::Status(302)));
    for off_list in [
        "https://evil.com/x.zip",
        "http://github.com/x.zip",
        "https://github.com.evil.com/x.zip",
        "ftp://github.com/x",
        "x.zip",
        "",
    ] {
        assert_eq!(
            step(302, Some(off_list), 0),
            Step::Fail(NetError::NotAllowed),
            "{off_list}"
        );
    }
    // A page redirect must stay on an allowed page.
    let page = "https://www.farming-simulator.com/mod.php?mod_id=1";
    assert_eq!(
        redirect_step(
            301,
            Some("/mod.php?mod_id=1&title=fs2025"),
            page,
            0,
            3,
            allowed_page_url
        ),
        Step::Follow("https://www.farming-simulator.com/mod.php?mod_id=1&title=fs2025".to_string())
    );
    assert_eq!(
        redirect_step(
            301,
            Some("https://www.farming-simulator.com/login"),
            page,
            0,
            3,
            allowed_page_url
        ),
        Step::Fail(NetError::NotAllowed)
    );
}

// ---------------------------------------------------------------- answers

#[test]
fn a_lookup_answer_is_mapped_and_unknown_fields_are_ignored() {
    let json = br#"{"results":[
        {"techName":"FS25_Courseplay","id":"abc","title":"Courseplay","latestVersion":"v8.1.0.3",
         "download":{"source":"github","url":"https://github.com/o/r/releases/download/v8.1.0.3/FS25_Courseplay.zip","size":5},
         "pageUrl":"https://www.farming-simulator.com/mod.php?mod_id=331515&title=fs2025","tags":["a"],"extra":{"x":1}},
        {"techName":"FS25_Other","id":42,"latestVersion":"","download":null,"pageUrl":null}
    ],"more":true}"#;
    let rows = parse_lookup(json).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "abc");
    assert_eq!(rows[0].latest_version.as_deref(), Some("v8.1.0.3"));
    let download = rows[0].download.as_ref().unwrap();
    assert_eq!(download.source, "github");
    assert!(download.url.ends_with("FS25_Courseplay.zip"));
    assert_eq!(rows[1].id, "42", "a numeric id becomes text");
    assert_eq!(
        rows[1].latest_version, None,
        "an empty version is no version"
    );
    assert!(rows[1].download.is_none() && rows[1].page_url.is_none());
}

#[test]
fn a_lookup_answer_of_the_wrong_shape_is_a_bad_answer() {
    for bad in [
        &b""[..],
        b"[]",
        b"{}",
        b"{\"results\":[{\"techName\":\"x\"}]}",
        b"<html>",
    ] {
        assert!(matches!(parse_lookup(bad), Err(NetError::BadAnswer(_))));
    }
    // The detail never includes the text of the answer.
    let Err(NetError::BadAnswer(detail)) = parse_lookup(b"{\"results\":\"secret-token\"}") else {
        panic!("expected a bad answer");
    };
    assert!(!detail.contains("secret"));
}

fn hash_answer(rows: &[(&str, &str)]) -> Vec<u8> {
    let rows: Vec<String> = rows
        .iter()
        .map(|(h, matches)| format!("{{\"sha256\":\"{h}\",\"matches\":{matches}}}"))
        .collect();
    format!("{{\"results\":[{}]}}", rows.join(",")).into_bytes()
}

#[test]
fn a_by_hash_answer_comes_back_in_the_order_asked() {
    let (a, b) = (hash(1), hash(2));
    let json = hash_answer(&[
        (&b, "[]"),
        (
            &a.to_uppercase(),
            r#"[{"id":"7","techName":"FS25_X","version":"1.0.0.0","source":"github","fileName":"FS25_X.zip","downloadUrl":null,"x":1}]"#,
        ),
    ]);
    let answers = parse_by_hash(&json, &[a.clone(), b.clone()]).unwrap();
    assert_eq!(answers.len(), 2);
    assert_eq!(answers[0].0, a);
    assert_eq!(answers[0].1[0].tech_name.as_deref(), Some("FS25_X"));
    assert_eq!(answers[0].1[0].version, "1.0.0.0");
    assert_eq!(answers[1], (b, Vec::new()));
}

#[test]
fn a_by_hash_answer_that_omits_adds_or_repeats_a_hash_is_bad() {
    let (a, b, c) = (hash(1), hash(2), hash(3));
    let asked = [a.clone(), b.clone()];
    let omits = hash_answer(&[(&a, "[]")]);
    let extra = hash_answer(&[(&a, "[]"), (&b, "[]"), (&c, "[]")]);
    let repeats = hash_answer(&[(&a, "[]"), (&a, "[]"), (&b, "[]")]);
    let other = hash_answer(&[(&a, "[]"), (&c, "[]")]);
    for (name, bytes) in [
        ("omits", omits),
        ("extra", extra),
        ("repeats", repeats),
        ("other", other),
    ] {
        assert!(
            matches!(parse_by_hash(&bytes, &asked), Err(NetError::BadAnswer(_))),
            "{name}"
        );
    }
    assert!(matches!(
        parse_by_hash(b"{}", &asked),
        Err(NetError::BadAnswer(_))
    ));
}

#[test]
fn a_detail_answer_is_mapped() {
    let json = br#"{"id":"abc","techName":"FS25_Courseplay","latestVersion":"v8.1.0.3",
        "archiveSha256":"ABCDEF","verified":true,"pageUrl":"https://www.farming-simulator.com/mod.php?mod_id=331515",
        "sources":[
          {"source":"GitHub","sourceUrl":"https://github.com/o/r","version":"v8.1.0.3","installable":true,
           "downloadUrl":"https://github.com/o/r/releases/download/v8.1.0.3/FS25_Courseplay.zip"},
          {"source":"modhub","sourceUrl":"https://www.farming-simulator.com/mod.php?mod_id=331515","version":"8.1.0.3"}
        ],"unknown":[1,2]}"#;
    let detail = parse_detail(json).unwrap();
    assert_eq!(detail.id, "abc");
    assert_eq!(detail.archive_sha256.as_deref(), Some("abcdef"));
    assert_eq!(detail.sources.len(), 2);
    assert_eq!(detail.sources[0].source, "github");
    assert!(detail.sources[0].installable);
    assert!(
        !detail.sources[1].installable,
        "missing means not installable"
    );
    assert_eq!(detail.sources[1].download_url, None);
    let minimal = parse_detail(br#"{"id":9}"#).unwrap();
    assert_eq!((minimal.id.as_str(), minimal.sources.len()), ("9", 0));
    assert!(matches!(parse_detail(b"{}"), Err(NetError::BadAnswer(_))));
}

// ---------------------------------------------------------------- the real client, offline

#[test]
fn the_real_client_refuses_off_list_addresses_before_any_request() {
    let http = Http::new("0.0.0-test");
    let cancel = AtomicBool::new(false);
    let mut sink = Vec::new();
    for url in [
        "http://github.com/o/r/x.zip",
        "https://evil.example/x.zip",
        "https://github.com.evil.com/x.zip",
        "https://www.farming-simulator.com/mod.php?mod_id=1",
    ] {
        let result = http.download(url, &mut sink, &mut |_| {}, &cancel);
        assert_eq!(result, Err(NetError::NotAllowed), "{url}");
    }
    for url in [
        "https://github.com/o/r/x.zip",
        "http://www.farming-simulator.com/mod.php?mod_id=1",
        "https://www.farming-simulator.com/other.php",
    ] {
        assert_eq!(http.fetch_page(url), Err(NetError::NotAllowed), "{url}");
    }
    assert!(sink.is_empty());
}

#[test]
fn the_real_client_honours_a_cancel_before_connecting() {
    let http = Http::new("0.0.0-test");
    let cancel = AtomicBool::new(true);
    let result = http.download(
        "https://github.com/o/r/x.zip",
        &mut Vec::new(),
        &mut |_| {},
        &cancel,
    );
    assert_eq!(result, Err(NetError::Cancelled));
}

#[test]
fn the_real_client_refuses_oversized_or_malformed_requests_before_any_request() {
    let http = Http::new("0.0.0-test");
    assert!(matches!(
        http.lookup(&names(501)),
        Err(NetError::BadAnswer(_))
    ));
    assert_eq!(http.lookup(&[" ".to_string()]), Ok(Vec::new()));
    assert!(matches!(
        http.by_hash(&["not a hash".to_string()]),
        Err(NetError::BadAnswer(_))
    ));
    let too_many: Vec<String> = (0..101).map(hash).collect();
    assert!(matches!(
        http.by_hash(&too_many),
        Err(NetError::BadAnswer(_))
    ));
    assert_eq!(http.by_hash(&[]), Ok(Some(Vec::new())));
    assert!(matches!(http.detail(""), Err(NetError::BadAnswer(_))));
}

// ---------------------------------------------------------------- the fakes

#[test]
fn the_fake_web_streams_in_chunks_counts_bytes_and_cancels_between_them() {
    use super::fakes::{FakeWeb, WebCall};
    let web = FakeWeb::new()
        .with_chunk(10)
        .with_payload("https://github.com/o/r/a.zip", vec![7; 95])
        .with_page(
            "https://www.farming-simulator.com/mod.php?mod_id=1",
            "<p>x</p>",
        );
    let mut sink = Vec::new();
    let mut seen = Vec::new();
    let total = web
        .download(
            "https://github.com/o/r/a.zip",
            &mut sink,
            &mut |n| seen.push(n),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!((total, sink.len()), (95, 95));
    assert_eq!(seen.len(), 10);
    assert_eq!(seen.iter().sum::<u64>(), 95);

    web.cancel_after_chunks(3);
    let mut sink = Vec::new();
    let result = web.download(
        "https://github.com/o/r/a.zip",
        &mut sink,
        &mut |_| {},
        &AtomicBool::new(false),
    );
    assert_eq!(result, Err(NetError::Cancelled));
    assert_eq!(sink.len(), 30);

    assert_eq!(
        web.fetch_page("https://www.farming-simulator.com/mod.php?mod_id=1")
            .unwrap(),
        "<p>x</p>"
    );
    web.fail_next(NetError::Offline);
    assert_eq!(
        web.fetch_page("https://www.farming-simulator.com/mod.php?mod_id=1"),
        Err(NetError::Offline)
    );
    assert_eq!(web.calls().len(), 4);
    assert!(matches!(web.calls()[0], WebCall::Download(_)));
    assert_eq!(
        web.fetch_page("https://nowhere"),
        Err(NetError::Status(404))
    );
}

#[test]
fn reading_a_body_stops_at_the_cap_and_a_cancel() {
    // `stream` is the loop under `Http::download`.
    let data = vec![1u8; 200_000];
    let mut sink = Vec::new();
    let mut total = 0;
    let written = super::http::stream(
        &mut Cursor::new(&data),
        &mut sink,
        &mut |n| total += n,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!((written, total, sink.len()), (200_000, 200_000, 200_000));

    let mut sink = Vec::new();
    let result = super::http::stream(
        &mut Cursor::new(&data),
        &mut sink,
        &mut |_| {},
        &AtomicBool::new(true),
    );
    assert_eq!(result, Err(NetError::Cancelled));
    assert!(sink.is_empty());
}
