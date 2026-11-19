//! Compare mode (plan M6): every mod name of two profiles side by side.

use super::Library;
use crate::discovery::GameContext;
use crate::model::{ApiError, Comparison};

impl Library {
    /// One row per mod name (case-insensitive) found in either profile, sorted by mod name.
    /// Both sides having it: `VersionDiffers` when the `modDesc` versions differ, else
    /// `ContentDiffers` when the content fingerprints differ (plan M6 decision 1), else
    /// `Identical`. Uses the profiles' last scans, scanning a profile that has none.
    pub fn compare_profiles(
        &self,
        _left_profile_id: &str,
        _right_profile_id: &str,
        _ctx: &GameContext,
    ) -> Result<Comparison, ApiError> {
        todo!("WS-A")
    }
}
