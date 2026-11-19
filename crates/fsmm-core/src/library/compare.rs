//! Compare mode (plan M6): every mod name of two profiles side by side.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::transfer::summary;
use super::{no_progress, Library, Scan};
use crate::discovery::GameContext;
use crate::model::{ApiError, CompareRow, CompareSide, CompareState, Comparison};
use crate::scan::ModRecord;

#[cfg(test)]
mod tests;

impl Library {
    /// One row per mod name (case-insensitive) found in either profile, sorted by mod name.
    /// Both sides having it: `VersionDiffers` when the `modDesc` versions differ (compared as
    /// strings, trimmed), else `ContentDiffers` when the content fingerprints differ (plan M6
    /// decision 1), else `Identical`. Both profiles are scanned now (reading only mods the cache
    /// does not know), like a listing, so the result is never staler than the disk.
    ///
    /// A profile with several mods of one name (case-only twins, or a zip and an unpacked
    /// folder) is represented by one of them: the zip, then the smaller file name (see
    /// `Scan::names`). The same profile on both sides is allowed and gives only `Identical`.
    pub fn compare_profiles(
        &self,
        left_profile_id: &str,
        right_profile_id: &str,
        ctx: &GameContext,
    ) -> Result<Comparison, ApiError> {
        self.remember(ctx);
        let left = self.find_profile(left_profile_id)?;
        let right = self.find_profile(right_profile_id)?;
        let left_scan = self.scan_profile(&left, ctx, &no_progress)?;
        let right_scan = if right.id == left.id {
            Arc::clone(&left_scan)
        } else {
            self.scan_profile(&right, ctx, &no_progress)?
        };

        let keys: BTreeSet<&String> = left_scan
            .names
            .keys()
            .chain(right_scan.names.keys())
            .collect();
        let rows = keys
            .into_iter()
            .map(|key| compare_row(key, &left_scan, &right_scan))
            .collect();
        Ok(Comparison {
            left_profile_id: left.id,
            right_profile_id: right.id,
            rows,
        })
    }
}

/// The side's record for a lower-cased mod name, with its fingerprint.
fn pick<'a>(scan: &'a Scan, key: &str) -> Option<&'a ModRecord> {
    scan.records.get(*scan.names.get(key)?)
}

pub(super) fn side(record: &ModRecord) -> CompareSide {
    CompareSide {
        mod_id: record.entry.id.clone(),
        summary: summary(&record.entry),
    }
}

fn compare_row(key: &str, left: &Scan, right: &Scan) -> CompareRow {
    let l = pick(left, key);
    let r = pick(right, key);
    let state = match (l, r) {
        (Some(_), None) => CompareState::OnlyLeft,
        (None, Some(_)) => CompareState::OnlyRight,
        (Some(l), Some(r)) => {
            if l.entry.version.trim() != r.entry.version.trim() {
                CompareState::VersionDiffers
            } else if content_differs(l, r) {
                CompareState::ContentDiffers
            } else {
                CompareState::Identical
            }
        }
        // The key came from one of the two maps.
        (None, None) => CompareState::Identical,
    };
    let first = l.or(r);
    CompareRow {
        mod_name: first.map(|f| f.entry.mod_name.clone()).unwrap_or_default(),
        title: first.map(|f| f.entry.title.clone()).unwrap_or_default(),
        left: l.map(side),
        right: r.map(side),
        state,
    }
}

/// Fingerprints differ. A mod that could not be opened has none (empty), and two of those only
/// count as the same when their sizes agree, since nothing else can tell them apart.
fn content_differs(l: &ModRecord, r: &ModRecord) -> bool {
    l.fingerprint != r.fingerprint
        || (l.fingerprint.is_empty() && l.entry.size_bytes != r.entry.size_bytes)
}
