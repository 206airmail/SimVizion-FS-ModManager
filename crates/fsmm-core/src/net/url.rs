//! Strict, hand-written URL handling for the allow-lists, the ModHub page parser and the user's
//! manual links. There is no URL crate: every rule here is spelled out so a lookalike host
//! (`giants-software.com.evil.com`, `cdnx.giants-software.com`), user info, a port or a backslash
//! can never pass for an allowed address.

/// The parts of an `http(s)` URL that the rules look at. The fragment is dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Parts<'a> {
    pub https: bool,
    /// The host exactly as written (compare with `eq_ignore_ascii_case`).
    pub host: &'a str,
    /// Starts with `/`; `/` when the URL has none.
    pub path: &'a str,
    /// The text after `?`, if there was a `?` (possibly empty).
    pub query: Option<&'a str>,
}

/// Splits an `https` URL (and an `http` one when `allow_http`). `None` for anything else, for
/// user info, a port, an empty or oddly spelled host, and for whitespace, control characters or
/// a backslash anywhere (browsers read `\` as `/`).
pub(super) fn split(url: &str, allow_http: bool) -> Option<Parts<'_>> {
    split_with(url, allow_http, false)
}

/// [`split`], optionally letting the host carry a `:port` (one to five digits), which is dropped.
/// Only the user's own links may; the allow-lists never do.
fn split_with(url: &str, allow_http: bool, allow_port: bool) -> Option<Parts<'_>> {
    if url
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return None;
    }
    let (https, rest) = if url
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("https://"))
    {
        (true, &url[8..])
    } else if allow_http
        && url
            .get(..7)
            .is_some_and(|p| p.eq_ignore_ascii_case("http://"))
    {
        (false, &url[7..])
    } else {
        return None;
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    let host = match authority.split_once(':') {
        Some((host, port))
            if allow_port
                && (1..=5).contains(&port.len())
                && port.bytes().all(|b| b.is_ascii_digit()) =>
        {
            host
        }
        _ => authority,
    };
    if !valid_host(host) {
        return None;
    }
    let tail = tail.split('#').next().unwrap_or("");
    let (path, query) = match tail.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (tail, None),
    };
    Some(Parts {
        https,
        host,
        path: if path.is_empty() { "/" } else { path },
        query,
    })
}

/// A host of ASCII letters, digits, `-` and `.` with no empty label. No `@` and no `:` can
/// appear, so there is no user info and no port.
fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.split('.').all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// The host (without a port) of an `http(s)` link the user pasted, lower-cased; `None` when it is
/// not one the app accepts (user info, odd host or port, whitespace, ...).
pub(crate) fn manual_link_host(url: &str) -> Option<String> {
    split_with(url, true, true).map(|p| p.host.to_ascii_lowercase())
}

/// `https://silo-api.hllmr.com/...`
pub(super) fn is_api_url(url: &str) -> bool {
    split(url, false).is_some_and(|p| p.host.eq_ignore_ascii_case("silo-api.hllmr.com"))
}

/// `https://www.farming-simulator.com/mod.php[?...]`
pub(super) fn is_page_url(url: &str) -> bool {
    split(url, false).is_some_and(|p| {
        p.host.eq_ignore_ascii_case("www.farming-simulator.com") && p.path == "/mod.php"
    })
}

/// `cdn<digits>.giants-software.com`, in any letter case.
fn is_cdn_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host.strip_suffix(".giants-software.com")
        .and_then(|name| name.strip_prefix("cdn"))
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// Hosts a download (and its redirects) may come from.
pub(super) fn is_download_url(url: &str) -> bool {
    split(url, false).is_some_and(|p| {
        [
            "github.com",
            "objects.githubusercontent.com",
            "release-assets.githubusercontent.com",
        ]
        .iter()
        .any(|allowed| p.host.eq_ignore_ascii_case(allowed))
            || is_cdn_host(p.host)
    })
}

/// The ModHub mod id in `https://www.farming-simulator.com/mod.php?mod_id=331515&title=fs2025`:
/// digits only (at most 8), from exactly one `mod_id` parameter.
pub(super) fn modhub_id_of(page_url: &str) -> Option<String> {
    let parts = split(page_url, false)?;
    if !is_page_url(page_url) {
        return None;
    }
    let mut ids = parts
        .query?
        .split('&')
        .filter_map(|pair| pair.strip_prefix("mod_id="));
    let id = ids.next()?;
    if ids.next().is_some() {
        return None;
    }
    (!id.is_empty() && id.len() <= 8 && id.bytes().all(|b| b.is_ascii_digit()))
        .then(|| id.to_string())
}

/// `(owner, repo)` of `https://github.com/<owner>/<repo>[/...]` (or `http://` when
/// `allow_http`). A `.git` suffix on the repo is dropped.
pub(super) fn github_repo_of(url: &str, allow_http: bool) -> Option<(String, String)> {
    let parts = split(url, allow_http)?;
    if !parts.host.eq_ignore_ascii_case("github.com") {
        return None;
    }
    let mut segments = parts.path.split('/').filter(|s| !s.is_empty());
    let owner = segments.next()?;
    let repo = segments.next()?;
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let valid = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    (valid(owner) && valid(repo)).then(|| (owner.to_string(), repo.to_string()))
}

/// The file name in the last path segment of a URL, percent-decoded; `None` when it has none
/// or is not valid UTF-8.
pub(crate) fn download_file_name(url: &str) -> Option<String> {
    let parts = split(url, true)?;
    let last = parts.path.rsplit('/').next()?;
    let decoded = percent_decode(last)?;
    (!decoded.is_empty() && !decoded.contains(['/', '\\'])).then_some(decoded)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Percent-encodes everything but `A-Za-z0-9-._~`, for one path segment.
pub(super) fn encode_segment(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Resolves a redirect `Location` against the URL it came from: an absolute `https` URL, a
/// `//host/path` one or a `/path` one. Anything else is `None`.
pub(super) fn resolve_location(current: &str, location: &str) -> Option<String> {
    let location = location.trim();
    if location.starts_with("//") {
        return Some(format!("https:{location}"));
    }
    if location.starts_with('/') {
        let parts = split(current, true)?;
        let scheme = if parts.https { "https" } else { "http" };
        return Some(format!("{scheme}://{}{location}", parts.host));
    }
    Some(location.to_string())
}

// ---------------------------------------------------------------- the ModHub page

/// The one DOWNLOAD link of a ModHub mod page. See [`super::modhub_download`].
pub(super) fn modhub_download_of(html: &str, mod_id: &str) -> Option<super::ModHubDownload> {
    if mod_id.is_empty() || mod_id.len() > 8 || !mod_id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let want = format!("{mod_id:0>8}");
    let lower = html.to_ascii_lowercase();

    let mut links = Vec::new();
    let mut from = 0;
    while let Some(found) = lower[from..].find("<a") {
        let start = from + found;
        from = start + 2;
        // `<a` followed by whitespace: not `<abbr>` or `<article>`.
        if !lower[from..].starts_with(|c: char| c.is_ascii_whitespace()) {
            continue;
        }
        let Some(end) = tag_end(&html[start..]) else {
            break;
        };
        let tag = &html[start + 2..start + end];
        from = start + end;
        if let Some(href) = attribute(tag, "href") {
            if let Some((id, file)) = storage_zip(&href) {
                if id == want {
                    links.push((href, file));
                }
            }
        }
    }
    let [(url, file_name)] = <[_; 1]>::try_from(links).ok()?;

    let version = row_value(html, &lower, "version").filter(|v| !v.is_empty());
    let approx_size = row_value(html, &lower, "size").and_then(|s| parse_size(&s));
    Some(super::ModHubDownload {
        url,
        file_name,
        version,
        approx_size,
    })
}

/// `(8-digit id, file name)` of `https://cdn<n>.giants-software.com/modHub/storage/<id>/<File>.zip`
/// with no query. The file name is limited to letters, digits, `_`, `-` and `.`.
fn storage_zip(url: &str) -> Option<(String, String)> {
    let parts = split(url, false)?;
    if !is_cdn_host(parts.host) || parts.query.is_some() || url.contains('#') {
        return None;
    }
    let rest = parts.path.strip_prefix("/modHub/storage/")?;
    let (id, file) = rest.split_once('/')?;
    if id.len() != 8 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let long_enough = file.len() > 4 && file.is_char_boundary(file.len() - 4);
    if !long_enough || !file[file.len() - 4..].eq_ignore_ascii_case(".zip") {
        return None;
    }
    let safe = file
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'));
    safe.then(|| (id.to_string(), file.to_string()))
}

/// The index just past the `>` that closes the tag starting at `tag[0]`, skipping quoted values.
fn tag_end(tag: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in tag.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return Some(i + 1),
            _ => {}
        }
    }
    None
}

/// The value of attribute `name` in the text between `<a` and `>`. `&amp;` is unescaped.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        let name_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && !matches!(bytes[i], b'=' | b'>' | b'/')
        {
            i += 1;
        }
        let attr = &tag[name_start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = match bytes.get(i) {
            Some(&q) if q == b'"' || q == b'\'' => {
                let begin = i + 1;
                let len = tag[begin..].find(char::from(q))?;
                i = begin + len + 1;
                &tag[begin..begin + len]
            }
            _ => {
                let begin = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                    i += 1;
                }
                &tag[begin..i]
            }
        };
        if attr.eq_ignore_ascii_case(name) {
            return Some(value.replace("&amp;", "&"));
        }
    }
    None
}

/// The text of the cell after `<b>label</b>` in a details row (`<div class="table-cell">`).
fn row_value(html: &str, lower: &str, label: &str) -> Option<String> {
    let marker = format!("<b>{label}</b>");
    let after = lower.find(&marker)? + marker.len();
    // The value is the next cell, right after this one closes.
    let close = after + lower[after..].find("</div>")?;
    let next = close + lower[close..].find("<div")?;
    if next - close > 200 {
        return None;
    }
    let open_end = next + lower[next..].find('>')? + 1;
    let value_end = open_end + lower[open_end..].find("</div>")?;
    Some(strip_tags(&html[open_end..value_end]).trim().to_string())
}

fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// `3.79 MB` -> bytes (1024-based, as ModHub shows sizes); `None` for anything else.
fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let split = text.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))?;
    let (number, unit) = text.split_at(split);
    let number = if number.contains('.') {
        number.replace(',', "")
    } else {
        number.replace(',', ".")
    };
    let value: f64 = number.parse().ok()?;
    let factor = match unit.trim().to_ascii_uppercase().as_str() {
        "B" => 1.0,
        "KB" => 1024.0,
        "MB" => 1024.0 * 1024.0,
        "GB" => 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    let bytes = value * factor;
    (bytes.is_finite() && (0.0..1e13).contains(&bytes)).then(|| bytes.round() as u64)
}
