//! Writes to game files. Today exactly one: the `modsDirectoryOverride` element of
//! `gameSettings.xml` (plan M4).
//!
//! Every write goes preflight → read → patch in memory → backup → guarded atomic replace →
//! verify. `gameSettings.xml` holds plain-text multiplayer passwords: no error, log line or
//! return value may contain any of its text other than the override's own values.
//!
//! Fidelity: [`patch_override`] never re-serialises the document. It parses the file only to
//! learn byte positions and then splices new attribute values into the original bytes, so the
//! BOM, line endings, indentation, attribute order, quote style and every other setting stay
//! byte for byte as they were. The result is parsed again and its unchanged prefix and suffix
//! are compared with the original before anything is written.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::discovery::{ModsOverride, Running};
use crate::model::{ApiError, ErrorCode};

#[cfg(test)]
pub(crate) mod tests;

/// How many `gameSettings.xml` backups are kept in the backups folder.
pub const KEEP_BACKUPS: usize = 20;

const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const ELEMENT: &str = "modsDirectoryOverride";
const BACKUP_PREFIX: &str = "gameSettings-";
const BACKUP_SUFFIX: &str = ".xml";
/// Length of `YYYYMMDDTHHMMSSmmmZ`.
const STAMP_LEN: usize = 19;

#[derive(Debug)]
pub enum OpError {
    GameRunning,
    ServerRunning,
    /// The process list could not be read, so nothing can be said about the game: refused.
    RunningUnknown,
    SettingsMissing,
    /// Not UTF-8, not well-formed, or the override element cannot be located. Position only.
    SettingsUnreadable,
    /// The file changed between the read and the write; nothing was written.
    SettingsChanged,
    /// Backup, temp file, replace or verification failed; before the replace the original is in
    /// place, after a failed verification (`verify`) the file is whatever is there now and the
    /// backup holds the original. The detail is an io error kind or a step name, never file text.
    WriteFailed(String),
    NoBackup,
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpError::GameRunning => write!(f, "the game is running"),
            OpError::ServerRunning => write!(f, "the dedicated server is running"),
            OpError::RunningUnknown => write!(f, "the running programs could not be listed"),
            OpError::SettingsMissing => write!(f, "gameSettings.xml does not exist"),
            OpError::SettingsUnreadable => write!(f, "gameSettings.xml could not be read"),
            OpError::SettingsChanged => write!(f, "gameSettings.xml changed during the write"),
            OpError::WriteFailed(d) => write!(f, "writing gameSettings.xml failed: {d}"),
            OpError::NoBackup => write!(f, "no backup of gameSettings.xml exists"),
        }
    }
}

impl std::error::Error for OpError {}

impl From<OpError> for ApiError {
    fn from(e: OpError) -> Self {
        let code = match &e {
            OpError::GameRunning => ErrorCode::GameRunning,
            OpError::ServerRunning => ErrorCode::ServerRunning,
            OpError::RunningUnknown => ErrorCode::RunningUnknown,
            OpError::SettingsMissing => ErrorCode::SettingsMissing,
            OpError::SettingsUnreadable => ErrorCode::SettingsUnreadable,
            OpError::SettingsChanged => ErrorCode::SettingsChanged,
            OpError::WriteFailed(_) => ErrorCode::WriteFailed,
            OpError::NoBackup => ErrorCode::NoBackup,
        };
        let detail = match e {
            OpError::WriteFailed(d) => Some(d),
            _ => None,
        };
        ApiError::new(code, detail)
    }
}

/// An io failure as a `WriteFailed` carrying only the error kind.
fn io_failed(e: &std::io::Error) -> OpError {
    OpError::WriteFailed(format!("{:?}", e.kind()))
}

fn step_failed(step: &str) -> OpError {
    OpError::WriteFailed(step.to_string())
}

/// Refuses while the game or the dedicated server runs, and when that cannot be told (the process
/// list could not be read): a guard fails closed.
pub fn check_not_running(running: &dyn Fn() -> Running) -> Result<(), OpError> {
    let now = running();
    if now.game {
        Err(OpError::GameRunning)
    } else if now.server {
        Err(OpError::ServerRunning)
    } else if now.unknown {
        Err(OpError::RunningUnknown)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------- reading

/// Splits off a UTF-8 BOM and requires the rest to be UTF-8. Returns the BOM length and text.
fn split_text(bytes: &[u8]) -> Result<(usize, &str), OpError> {
    let bom = if bytes.starts_with(BOM) { BOM.len() } else { 0 };
    let text = std::str::from_utf8(&bytes[bom..]).map_err(|_| OpError::SettingsUnreadable)?;
    Ok((bom, text))
}

/// roxmltree's own error text can quote element names, so its text is dropped.
fn parse_document(text: &str) -> Result<roxmltree::Document<'_>, OpError> {
    roxmltree::Document::parse(text).map_err(|_| OpError::SettingsUnreadable)
}

fn find_override<'a, 'i>(doc: &'a roxmltree::Document<'i>) -> Option<roxmltree::Node<'a, 'i>> {
    doc.descendants()
        .find(|n| n.is_element() && n.has_tag_name(ELEMENT))
}

fn state_of(node: Option<roxmltree::Node<'_, '_>>) -> ModsOverride {
    let Some(node) = node else {
        return ModsOverride::default();
    };
    let active = node
        .attribute("active")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("true") || v.trim() == "1");
    let directory = node
        .attribute("directory")
        .filter(|d| !d.trim().is_empty())
        .map(PathBuf::from);
    ModsOverride { active, directory }
}

/// The override currently in `bytes` (a missing element reads as inactive).
pub fn read_override(bytes: &[u8]) -> Result<ModsOverride, OpError> {
    let (_, text) = split_text(bytes)?;
    let doc = parse_document(text)?;
    Ok(state_of(find_override(&doc)))
}

// ---------------------------------------------------------------------------- patching

/// Escapes a value for an attribute delimited by `quote`. Only that quote is escaped, so a
/// path such as `John's Mods` inside double quotes round-trips byte for byte. Line breaks and
/// tabs become character references because attribute-value normalisation would otherwise
/// turn them into spaces.
fn escape_attr(value: &str, quote: u8) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if quote == b'"' => out.push_str("&quot;"),
            '\'' if quote == b'\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c => out.push(c),
        }
    }
    out
}

fn bool_text(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn directory_text(dir: Option<&Path>, quote: u8) -> Result<String, OpError> {
    match dir {
        None => Ok(String::new()),
        Some(d) => d
            .to_str()
            .map(|v| escape_attr(v, quote))
            .ok_or_else(|| step_failed("path")),
    }
}

fn canonical_element(wanted: &ModsOverride) -> Result<String, OpError> {
    Ok(format!(
        "<{ELEMENT} active=\"{}\" directory=\"{}\"/>",
        bool_text(wanted.active),
        directory_text(wanted.directory.as_deref(), b'"')?
    ))
}

/// The attribute value's byte range, checked to sit between two matching quotes.
fn value_range(text: &str, attr: &roxmltree::Attribute<'_, '_>) -> Result<Range<usize>, OpError> {
    let range = attr.range_value();
    let bytes = text.as_bytes();
    let quote = range
        .start
        .checked_sub(1)
        .and_then(|i| bytes.get(i))
        .copied();
    let closing = bytes.get(range.end).copied();
    match quote {
        Some(q @ (b'"' | b'\'')) if closing == Some(q) && range.start <= range.end => Ok(range),
        _ => Err(OpError::SettingsUnreadable),
    }
}

/// The line break plus indentation copied from the whitespace that precedes the root's first
/// child, so a new element sits on its own line in the file's style.
fn leading_whitespace(raw: &str) -> &str {
    match raw.rfind('\n') {
        Some(i) => {
            let start = if i > 0 && raw.as_bytes()[i - 1] == b'\r' {
                i - 1
            } else {
                i
            };
            &raw[start..]
        }
        None => raw,
    }
}

/// The edit that inserts the canonical element as the root's first child.
fn insertion(text: &str, doc: &roxmltree::Document<'_>, element: &str) -> (Range<usize>, String) {
    let root = doc.root_element();
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    if let Some(first) = root.first_child() {
        let at = first.range().start;
        let lead = if first.is_text() {
            let raw = &text[first.range()];
            if raw.trim().is_empty() {
                leading_whitespace(raw)
            } else {
                ""
            }
        } else {
            ""
        };
        return (at..at, format!("{lead}{element}"));
    }
    // An empty root: `<gameSettings></gameSettings>` or `<gameSettings/>`.
    let range = root.range();
    let tag = &text[range.clone()];
    if let Some(open) = tag.strip_suffix("/>") {
        let name: String = open
            .trim_start_matches('<')
            .chars()
            .take_while(|c| !c.is_whitespace())
            .collect();
        let end = range.end;
        return (end - 2..end, format!(">{eol}    {element}{eol}</{name}>"));
    }
    let close = range.start + tag.rfind("</").unwrap_or(tag.len());
    (close..close, format!("{eol}    {element}{eol}"))
}

/// Returns `bytes` with the override set to `target`, changing only the attribute values
/// (plan M4 decision 1). A missing attribute or element is written in canonical form.
/// Fails with `SettingsUnreadable` for non-UTF-8 or malformed input.
///
/// `target.directory == None` leaves the `directory` value as it is: that is how the game's
/// default folder is selected (`active="false"`, plan M4 decision 2).
pub fn patch_override(bytes: &[u8], target: &ModsOverride) -> Result<Vec<u8>, OpError> {
    let (bom, text) = split_text(bytes)?;
    let doc = parse_document(text)?;
    let node = find_override(&doc);
    let current = state_of(node);
    let wanted = ModsOverride {
        active: target.active,
        directory: target
            .directory
            .clone()
            .or_else(|| current.directory.clone()),
    };
    if current == wanted {
        return Ok(bytes.to_vec());
    }

    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    match node {
        Some(node) => match (
            node.attribute_node("active"),
            node.attribute_node("directory"),
        ) {
            (Some(active), Some(directory)) => {
                if current.active != wanted.active {
                    edits.push((
                        value_range(text, &active)?,
                        bool_text(wanted.active).to_string(),
                    ));
                }
                if current.directory != wanted.directory {
                    let range = value_range(text, &directory)?;
                    let quote = text.as_bytes()[range.start - 1];
                    edits.push((range, directory_text(wanted.directory.as_deref(), quote)?));
                }
            }
            _ => edits.push((node.range(), canonical_element(&wanted)?)),
        },
        None => edits.push(insertion(text, &doc, &canonical_element(&wanted)?)),
    }
    edits.sort_by_key(|(range, _)| range.start);

    let result = apply_edits(bytes, bom, &edits)?;
    let first = bom + edits.first().map_or(0, |(r, _)| r.start);
    let last = bom + edits.last().map_or(0, |(r, _)| r.end);
    verify_patch(bytes, &result, first..last, &wanted)?;
    Ok(result)
}

/// Splices the edits (text coordinates, sorted, not overlapping) into the original bytes.
fn apply_edits(
    bytes: &[u8],
    bom: usize,
    edits: &[(Range<usize>, String)],
) -> Result<Vec<u8>, OpError> {
    let mut out = Vec::with_capacity(bytes.len() + 64);
    let mut pos = 0;
    for (range, replacement) in edits {
        let (start, end) = (bom + range.start, bom + range.end);
        if start < pos || end < start || end > bytes.len() {
            return Err(step_failed("patch"));
        }
        out.extend_from_slice(&bytes[pos..start]);
        out.extend_from_slice(replacement.as_bytes());
        pos = end;
    }
    out.extend_from_slice(&bytes[pos..]);
    Ok(out)
}

/// Re-parses `result`, checks it reads as `wanted`, and checks that everything outside the
/// edited span `span` (original coordinates, BOM included) is byte-identical to `original`.
fn verify_patch(
    original: &[u8],
    result: &[u8],
    span: Range<usize>,
    wanted: &ModsOverride,
) -> Result<(), OpError> {
    if read_override(result)? != *wanted {
        return Err(step_failed("verify"));
    }
    let tail = original.len() - span.end;
    let prefix_same = result.len() >= span.start && result[..span.start] == original[..span.start];
    let suffix_same = result.len() >= span.start + tail
        && result[result.len() - tail..] == original[original.len() - tail..];
    if prefix_same && suffix_same {
        Ok(())
    } else {
        Err(step_failed("verify"))
    }
}

// ---------------------------------------------------------------------------- backups

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from((month + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The inverse of [`days_from_civil`].
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// `YYYYMMDDTHHMMSSmmmZ` in UTC: sorts in time order.
fn stamp_of(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let millis = since.as_millis();
    let secs = i64::try_from(millis / 1000).unwrap_or(0);
    let ms = millis % 1000;
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}{ms:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Reads a stamp written by [`stamp_of`] from the start of `s`.
fn time_of_stamp(s: &str) -> Option<SystemTime> {
    let b = s.as_bytes();
    if b.len() < STAMP_LEN || b[8] != b'T' || b[STAMP_LEN - 1] != b'Z' {
        return None;
    }
    let num = |range: Range<usize>| -> Option<i64> {
        let part = s.get(range)?;
        if part.bytes().all(|c| c.is_ascii_digit()) {
            part.parse().ok()
        } else {
            None
        }
    };
    let (year, month, day) = (num(0..4)?, num(4..6)?, num(6..8)?);
    let (hour, minute, second, ms) = (num(9..11)?, num(11..13)?, num(13..15)?, num(15..18)?);
    let days = days_from_civil(year, u32::try_from(month).ok()?, u32::try_from(day).ok()?);
    let seconds = ((days * 24 + hour) * 60 + minute) * 60 + second;
    u64::try_from(seconds * 1000 + ms)
        .ok()
        .map(|ms| UNIX_EPOCH + Duration::from_millis(ms))
}

fn is_backup_name(name: &str) -> bool {
    name.len() > BACKUP_PREFIX.len() + BACKUP_SUFFIX.len()
        && name.starts_with(BACKUP_PREFIX)
        && name.to_ascii_lowercase().ends_with(BACKUP_SUFFIX)
}

/// Backup files in `dir` as `(file name, path)`, oldest name first.
fn backups_in(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| Some((e.file_name().into_string().ok()?, e.path())))
        .filter(|(name, _)| is_backup_name(name))
        .collect();
    found.sort();
    found
}

/// Copies `bytes` (the current file) into `dir` as `gameSettings-<UTC timestamp>.xml`, prunes to
/// `keep` newest, and returns the backup's file name. A name that already exists gets a `_NNN`
/// suffix, which sorts after the plain name. Only `gameSettings-*.xml` files are ever pruned.
pub fn backup(bytes: &[u8], dir: &Path, keep: usize) -> Result<String, OpError> {
    fs::create_dir_all(dir).map_err(|e| io_failed(&e))?;
    let stamp = stamp_of(SystemTime::now());
    let mut name = format!("{BACKUP_PREFIX}{stamp}{BACKUP_SUFFIX}");
    let mut n = 0u32;
    while dir.join(&name).exists() {
        n += 1;
        if n > 999 {
            return Err(step_failed("backup name"));
        }
        name = format!("{BACKUP_PREFIX}{stamp}_{n:03}{BACKUP_SUFFIX}");
    }

    // Written under a name that is not a backup name, so a crash never leaves a half-written
    // file that would count as the newest backup.
    let part = dir.join(format!("{name}.part"));
    let written = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&part, dir.join(&name))
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&part);
        return Err(io_failed(&e));
    }

    // Pruning is best effort: failing to delete an old backup must not fail the switch.
    let all = backups_in(dir);
    let excess = all.len().saturating_sub(keep.max(1));
    for (_, path) in all.into_iter().take(excess) {
        let _ = fs::remove_file(path);
    }
    Ok(name)
}

/// The newest backup in `dir` (by name) that is readable settings, and its time, taken from
/// the name, else the file's modification time. Unreadable backups are skipped: restoring
/// backs up the broken file first, and that copy must never become the restore source.
pub fn latest_backup(dir: &Path) -> Option<(PathBuf, SystemTime)> {
    let (name, path) = backups_in(dir)
        .into_iter()
        .rev()
        .find(|(_, path)| fs::read(path).is_ok_and(|bytes| read_override(&bytes).is_ok()))?;
    let from_name = name.strip_prefix(BACKUP_PREFIX).and_then(time_of_stamp);
    let time = from_name
        .or_else(|| fs::metadata(&path).and_then(|m| m.modified()).ok())
        .unwrap_or(UNIX_EPOCH);
    Some((path, time))
}

// ---------------------------------------------------------------------------- writing

/// Failure injection points around the rename, used by tests.
#[derive(Default)]
pub(crate) struct Hooks<'a> {
    /// Runs after the temp file is written and before the guards and the rename.
    pub before_rename: Option<&'a dyn Fn(&Path)>,
    /// Runs after the rename and before the verification read.
    pub after_rename: Option<&'a dyn Fn(&Path)>,
}

/// A temp file that is removed when dropped, whatever happened to it.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        // After a successful rename the name no longer exists; that is fine.
        let _ = fs::remove_file(&self.0);
    }
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to a new, uniquely named file next to `path` and syncs it.
fn stage_temp(path: &Path, bytes: &[u8]) -> Result<TempFile, OpError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| step_failed("path"))?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_name = format!("{name}.fsmm-{}-{nanos}-{seq}.tmp", std::process::id());
    let temp = TempFile(path.with_file_name(temp_name));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp.0)
        .map_err(|e| io_failed(&e))?;
    // From here the guard removes the file on any early return.
    file.write_all(bytes).map_err(|e| io_failed(&e))?;
    file.sync_all().map_err(|e| io_failed(&e))?;
    Ok(temp)
}

/// Replaces `path` with `new_bytes` atomically (temp file in the same folder, flush, rename),
/// but only if the file still holds `expected` and `running` reports neither the game nor the
/// server, both checked right before the rename. Re-reads and verifies afterwards; when the file
/// is no longer what was written it is left as it is (`WriteFailed("verify")`; the caller's backup
/// keeps the old bytes). Never leaves the temp file behind.
///
/// A read-only file is never made writable: it fails with `WriteFailed` and stays untouched.
pub fn write_settings(
    path: &Path,
    new_bytes: &[u8],
    expected: &[u8],
    running: &dyn Fn() -> Running,
) -> Result<(), OpError> {
    write_settings_with(path, new_bytes, expected, running, &Hooks::default())
}

pub(crate) fn write_settings_with(
    path: &Path,
    new_bytes: &[u8],
    expected: &[u8],
    running: &dyn Fn() -> Running,
    hooks: &Hooks<'_>,
) -> Result<(), OpError> {
    match fs::metadata(path) {
        Ok(meta) if meta.permissions().readonly() => {
            return Err(OpError::WriteFailed("PermissionDenied".into()));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(OpError::SettingsMissing),
        Err(e) => return Err(io_failed(&e)),
    }

    let temp = stage_temp(path, new_bytes)?;
    if let Some(hook) = hooks.before_rename {
        hook(path);
    }

    // The last moment before the replace: nothing may have started or changed.
    check_not_running(running)?;
    match fs::read(path) {
        Ok(current) if current == expected => {}
        Ok(_) => return Err(OpError::SettingsChanged),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(OpError::SettingsChanged),
        Err(e) => return Err(io_failed(&e)),
    }
    fs::rename(&temp.0, path).map_err(|e| io_failed(&e))?;

    if let Some(hook) = hooks.after_rename {
        hook(path);
    }
    match fs::read(path) {
        Ok(now) if now == new_bytes => Ok(()),
        // The replace happened but the file is not what was written: someone else wrote to it
        // since (a settings editor, the game), or it cannot be read right now. Whatever is there
        // is left alone, never overwritten with the old bytes; the backup made before this write
        // still holds them.
        _ => Err(step_failed("verify")),
    }
}
