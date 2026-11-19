//! Orchestration behind the app's commands: the profile registry, cache-assisted scans,
//! cross-profile `also_in`, profile summaries and thumbnails. Free of Tauri, so it is tested
//! with temp folders. The Tauri layer only converts arguments and results.
//!
//! Every listing is fresh (plan M3 decision 1): folders are re-enumerated on each call and only
//! files whose path, size or mtime changed are read again. Callers identify profiles and mods
//! by id; this module resolves ids to paths (decision 5), so no caller-supplied path is read
//! except the one passed to [`Library::add_profile`] from the folder picker.
//!
//! `Library` is `Send + Sync` and its methods take `&self`: a long scan must not block
//! thumbnail requests for longer than a short database access.

use std::path::{Path, PathBuf};

use crate::discovery::GameContext;
use crate::model::{ApiError, ModEntry, Profile, ScanProgress};
use crate::store::Store;

pub struct Library {
    // WS-A: the store, the thumbnail URL base and the last scan per profile.
}

impl Library {
    /// `thumbnail_base` is the URL prefix for `ModEntry::icon`, e.g. `http://thumb.localhost`.
    /// Icons become `<base>/<profileId>/<modId>?v=<size>-<mtime>`, ids percent-encoded.
    pub fn new(_store: Store, _thumbnail_base: &str) -> Self {
        todo!("WS-A")
    }

    /// Plan M3 decision 6: when no profile is registered, registers
    /// `discovery::suggest_profiles(user_dir)`; always registers the effective and default
    /// mods folders when they exist and are missing. The name is the folder name.
    pub fn ensure_profiles(&self, _ctx: &GameContext) -> Result<(), ApiError> {
        todo!("WS-A")
    }

    /// Every registered profile with fresh counts. A folder that no longer exists is listed
    /// with zero counts rather than failing the call.
    pub fn list_profiles(&self, _ctx: &GameContext) -> Result<Vec<Profile>, ApiError> {
        todo!("WS-A")
    }

    /// The profile's mods after `assess`, with `also_in` and icon URLs filled. `progress` is
    /// called with `done` rising from 0 to `total` (the number of entries enumerated).
    pub fn list_mods(
        &self,
        _profile_id: &str,
        _ctx: &GameContext,
        _progress: &(dyn Fn(ScanProgress) + Sync),
    ) -> Result<Vec<ModEntry>, ApiError> {
        todo!("WS-A")
    }

    /// Registers a folder chosen by the user. Re-adding a registered folder returns it.
    pub fn add_profile(&self, _path: &Path, _ctx: &GameContext) -> Result<Profile, ApiError> {
        todo!("WS-A")
    }

    /// The folder of a registered profile.
    pub fn profile_path(&self, _profile_id: &str) -> Result<PathBuf, ApiError> {
        todo!("WS-A")
    }

    /// The file or folder of a mod from the profile's last scan.
    pub fn mod_path(&self, _profile_id: &str, _mod_id: &str) -> Result<PathBuf, ApiError> {
        todo!("WS-A")
    }

    /// PNG thumbnail of a mod from the profile's last scan: from the cache, or decoded and
    /// cached. Unknown ids, and mods without an icon, are errors.
    pub fn thumbnail(&self, _profile_id: &str, _mod_id: &str) -> Result<Vec<u8>, ApiError> {
        todo!("WS-A")
    }
}

/// Percent-encodes an id for use as one URL path segment (everything but `A-Za-z0-9-._~`).
pub fn encode_segment(_id: &str) -> String {
    todo!("WS-A")
}

/// Reverses [`encode_segment`]; `None` for malformed input or invalid UTF-8.
pub fn decode_segment(_segment: &str) -> Option<String> {
    todo!("WS-A")
}
