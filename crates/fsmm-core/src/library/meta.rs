//! Tags and notes (plan M6 decision 5): user data keyed by lower-cased mod name in
//! `library.sqlite`, so they follow a mod into every profile and survive rescans, copies,
//! moves and updates.

use super::Library;
use crate::model::ApiError;

impl Library {
    /// Replaces the mod name's tags. Each tag is trimmed; empty ones are dropped, duplicates
    /// (case-insensitive) kept once with the first spelling. Returns the stored tags, sorted.
    pub fn set_tags(&self, _mod_name: &str, _tags: &[String]) -> Result<Vec<String>, ApiError> {
        todo!("WS-A")
    }

    /// Sets the mod name's note; an empty or whitespace-only note clears it.
    pub fn set_note(&self, _mod_name: &str, _note: &str) -> Result<(), ApiError> {
        todo!("WS-A")
    }

    /// Every tag in use, sorted case-insensitively (for suggestions and the tag filter).
    pub fn list_tags(&self) -> Result<Vec<String>, ApiError> {
        todo!("WS-A")
    }
}
