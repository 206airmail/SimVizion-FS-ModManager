//! Version strings (`v8.1.0.3`, `V.1.1.5.0`, `1.2-beta`), compared the way the Silo client does:
//! a leading `v`/`V` is dropped, the rest is split on `.`, `-`, `_` and `+`, each segment's
//! leading digits are a number (a segment without digits is 0), and missing trailing segments
//! count as 0. Written from the rules, not copied.

use std::cmp::Ordering;

/// The numeric segments of a version, or `None` when it does not start with a digit (after an
/// optional `v`, and after the empty segment of `V.`) or a number does not fit.
fn segments(version: &str) -> Option<Vec<u64>> {
    let version = version.trim();
    let rest = match version.strip_prefix(['v', 'V']) {
        Some(rest) => rest.strip_prefix('.').unwrap_or(rest),
        None => version,
    };
    if !rest.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    rest.split(['.', '-', '_', '+'])
        .map(|segment| {
            let end = segment
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(segment.len());
            match &segment[..end] {
                "" => Some(0),
                digits => digits.parse().ok(),
            }
        })
        .collect()
}

fn compare(a: &[u64], b: &[u64]) -> Ordering {
    (0..a.len().max(b.len()))
        .map(|i| {
            let x = a.get(i).copied().unwrap_or(0);
            let y = b.get(i).copied().unwrap_or(0);
            x.cmp(&y)
        })
        .find(|ordering| ordering.is_ne())
        .unwrap_or(Ordering::Equal)
}

pub(super) fn equal(a: &str, b: &str) -> bool {
    match (segments(a), segments(b)) {
        (Some(x), Some(y)) => compare(&x, &y).is_eq(),
        _ => a == b,
    }
}

pub(super) fn newer(candidate: &str, installed: &str) -> Option<bool> {
    let candidate = segments(candidate)?;
    let installed = segments(installed)?;
    Some(compare(&candidate, &installed).is_gt())
}
