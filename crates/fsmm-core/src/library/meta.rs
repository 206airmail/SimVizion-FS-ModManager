//! Tags and notes (plan M6 decision 5): user data keyed by lower-cased mod name in
//! `library.sqlite`, so they follow a mod into every profile and survive rescans, copies,
//! moves and updates. `list_mods` fills `ModEntry::tags` and `note` from the same table.
//!
//! Limits are errors, not silent truncation: the user typed it, so they are told. The
//! `Internal` details are the `INVALID_*` constants below.

use std::collections::HashSet;

use super::transfer::INVALID_NAME;
use super::Library;
use crate::model::{ApiError, ErrorCode};

#[cfg(test)]
mod tests;

/// Longest tag, in characters.
pub const MAX_TAG_CHARS: usize = 40;
/// Most tags on one mod name.
pub const MAX_TAGS: usize = 20;
/// Longest note, in characters.
pub const MAX_NOTE_CHARS: usize = 4000;

/// `Internal` detail for a tag that is too long, or a mod name with too many tags.
pub const INVALID_TAG: &str = "invalidTag";
/// `Internal` detail for a note that is too long.
pub const INVALID_NOTE: &str = "invalidNote";

impl Library {
    /// Replaces the mod name's tags. Each tag is trimmed; empty ones are dropped, duplicates
    /// (case-insensitive) kept once with the first spelling. Returns the stored tags, sorted
    /// case-insensitively. A tag over 40 characters, or more than 20 tags, is `Internal` with
    /// the detail `invalidTag` and nothing is stored. No tags (and no note) removes the row.
    pub fn set_tags(&self, mod_name: &str, tags: &[String]) -> Result<Vec<String>, ApiError> {
        let key = name_key(mod_name)?;
        let mut seen = HashSet::new();
        let mut kept: Vec<String> = Vec::new();
        for tag in tags {
            let tag = tag.trim();
            if tag.is_empty() {
                continue;
            }
            if tag.chars().count() > MAX_TAG_CHARS {
                return Err(invalid(INVALID_TAG));
            }
            if seen.insert(tag.to_lowercase()) {
                kept.push(tag.to_string());
            }
        }
        if kept.len() > MAX_TAGS {
            return Err(invalid(INVALID_TAG));
        }
        kept.sort_by_cached_key(|t| (t.to_lowercase(), t.clone()));
        self.store.set_mod_tags(&key, &kept)?;
        Ok(kept)
    }

    /// Sets the mod name's note; an empty or whitespace-only note clears it. A note over 4000
    /// characters is `Internal` with the detail `invalidNote` and nothing is stored.
    pub fn set_note(&self, mod_name: &str, note: &str) -> Result<(), ApiError> {
        let key = name_key(mod_name)?;
        let note = note.trim();
        if note.chars().count() > MAX_NOTE_CHARS {
            return Err(invalid(INVALID_NOTE));
        }
        let note = (!note.is_empty()).then_some(note);
        Ok(self.store.set_mod_note(&key, note)?)
    }

    /// Every tag in use, sorted case-insensitively (for suggestions and the tag filter).
    /// Spellings that differ only by case appear once.
    pub fn list_tags(&self) -> Result<Vec<String>, ApiError> {
        Ok(self.store.all_tags()?)
    }
}

/// The key a mod name is stored under: trimmed and lower-cased, like every name match here.
pub(super) fn name_key(mod_name: &str) -> Result<String, ApiError> {
    let key = mod_name.trim().to_lowercase();
    if key.is_empty() {
        return Err(invalid(INVALID_NAME));
    }
    Ok(key)
}

fn invalid(detail: &str) -> ApiError {
    ApiError::new(ErrorCode::Internal, Some(detail.to_string()))
}
