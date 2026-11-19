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
/// development session cannot change the developer's real game settings by accident.
fn writes_allowed() -> Result<(), ApiError> {
    if cfg!(debug_assertions) && std::env::var(ALLOW_WRITES_VAR).as_deref() != Ok("1") {
        return Err(ApiError::new(ErrorCode::WritesDisabled, None));
    }
    Ok(())
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
            .latest_backup(&backups)
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
