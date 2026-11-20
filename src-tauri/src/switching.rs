//! Commands that write `gameSettings.xml` (plan M4): switch the active folder, undo, and the
//! emergency restore. The safety logic is in `fsmm_core::ops` and `Library`; this layer adds
//! the development guard and the real process check.

use fsmm_core::discovery::{self, Running, WindowsProbe};
use fsmm_core::model::{ApiError, ErrorCode, SwitchResult};
use tauri::State;

use crate::commands::{blocking, context};
use crate::AppState;

/// Environment variable that lets a debug build write the real game files.
const ALLOW_WRITES_VAR: &str = "FSMM_ALLOW_GAME_WRITES";

/// Release builds always write; debug builds only when `FSMM_ALLOW_GAME_WRITES=1`, so a
/// development session cannot change the developer's real game settings or mods by accident.
/// Every command that writes game files asks (settings, copy, move, remove, mod updates).
pub(crate) fn writes_allowed() -> Result<(), ApiError> {
    check_writes(
        cfg!(debug_assertions),
        std::env::var(ALLOW_WRITES_VAR).ok().as_deref(),
    )
}

fn check_writes(debug_build: bool, allow: Option<&str>) -> Result<(), ApiError> {
    if debug_build && allow != Some("1") {
        return Err(ApiError::new(ErrorCode::WritesDisabled, None));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_builds_write_only_when_allowed_and_release_builds_always_do() {
        for allow in [None, Some(""), Some("0"), Some("true")] {
            assert_eq!(
                check_writes(true, allow).unwrap_err().code,
                ErrorCode::WritesDisabled,
                "{allow:?}"
            );
        }
        assert!(check_writes(true, Some("1")).is_ok());
        for allow in [None, Some("0"), Some("1")] {
            assert!(check_writes(false, allow).is_ok(), "{allow:?}");
        }
    }
}

fn running() -> Running {
    discovery::running(&WindowsProbe)
}

#[tauri::command]
pub async fn set_active_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<SwitchResult, ApiError> {
    let library = state.library.clone();
    let backups = state.backups_dir.clone();
    blocking(move || {
        writes_allowed()?;
        library.set_active(&profile_id, &context(), &running, &backups)
    })
    .await
}

#[tauri::command]
pub async fn undo_switch(
    state: State<'_, AppState>,
    op_id: String,
) -> Result<SwitchResult, ApiError> {
    let library = state.library.clone();
    let backups = state.backups_dir.clone();
    blocking(move || {
        writes_allowed()?;
        library.undo_switch(&op_id, &context(), &running, &backups)
    })
    .await
}

#[tauri::command]
pub async fn latest_settings_backup(
    state: State<'_, AppState>,
) -> Result<Option<String>, ApiError> {
    let library = state.library.clone();
    let backups = state.backups_dir.clone();
    blocking(move || {
        Ok(library
            .latest_backup(&context(), &backups)
            .map(|time| humantime::format_rfc3339_seconds(time).to_string()))
    })
    .await
}

#[tauri::command]
pub async fn restore_settings_backup(state: State<'_, AppState>) -> Result<(), ApiError> {
    let library = state.library.clone();
    let backups = state.backups_dir.clone();
    blocking(move || {
        writes_allowed()?;
        library.restore_backup(&context(), &running, &backups)
    })
    .await
}
