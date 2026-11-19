//! `log.txt`: the game's startup block names its version, language and ModDesc version.

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use super::LogSummary;

const BLOCK_START: &str = "Farming Simulator 25";

/// Reads line by line and stops once the startup block ends. Other `Version:` lines earlier in
/// the log (engine and library banners) are ignored: only lines inside the block count.
pub(super) fn read(log: &Path) -> io::Result<LogSummary> {
    let mut reader = BufReader::new(File::open(log)?);
    let mut summary = LogSummary::default();
    let mut in_block = false;
    let mut raw = Vec::new();
    let mut first = true;
    loop {
        raw.clear();
        if reader.read_until(b'\n', &mut raw)? == 0 {
            break;
        }
        let decoded = String::from_utf8_lossy(&raw);
        let mut line = decoded.trim_end_matches(['\r', '\n']);
        if first {
            line = line.trim_start_matches('\u{feff}');
            first = false;
        }
        if !in_block {
            in_block = line.starts_with(BLOCK_START);
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(char::is_whitespace) {
            break; // the block has ended
        }
        let line = line.trim();
        if let Some(v) = line.strip_prefix("Version:") {
            summary.game_version = non_empty(v);
        } else if let Some(v) = line.strip_prefix("Language:") {
            summary.language = non_empty(v);
        } else if let Some(v) = line.strip_prefix("ModDesc Version:") {
            summary.mod_desc_version = v.trim().parse().ok();
        }
    }
    Ok(summary)
}

fn non_empty(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}
