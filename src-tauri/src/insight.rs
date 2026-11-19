//! Commands for Compare mode, savegame comparison, and tags and notes (plan M6). The logic is
//! in `Library`; savegames are only ever read.

use std::time::Instant;

use fsmm_core::model::{ApiError, Comparison, HashProgress, Savegame, SavegameReport};
use tauri::ipc::Channel;
use tauri::State;

use crate::commands::{blocking, context, log_timing};
use crate::AppState;

#[tauri::command]
pub async fn compare_profiles(
    state: State<'_, AppState>,
    left_profile_id: String,
    right_profile_id: String,
) -> Result<Comparison, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        let started = Instant::now();
        let comparison = library.compare_profiles(&left_profile_id, &right_profile_id, &context());
        log_timing("compare_profiles", started);
        comparison
    })
    .await
}

#[tauri::command]
pub async fn list_savegames(state: State<'_, AppState>) -> Result<Vec<Savegame>, ApiError> {
    let library = state.library.clone();
    blocking(move || library.list_savegames(&context())).await
}

#[tauri::command]
pub async fn savegame_report(
    state: State<'_, AppState>,
    savegame_id: String,
    profile_id: String,
) -> Result<SavegameReport, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        let started = Instant::now();
        let report = library.savegame_report(&savegame_id, &profile_id, &context());
        log_timing(
            &format!("savegame_report {savegame_id} {profile_id}"),
            started,
        );
        report
    })
    .await
}

#[tauri::command]
pub async fn check_savegame_files(
    state: State<'_, AppState>,
    savegame_id: String,
    profile_id: String,
    on_progress: Channel<HashProgress>,
) -> Result<SavegameReport, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        let started = Instant::now();
        let report =
            library.check_savegame_files(&savegame_id, &profile_id, &context(), &|progress| {
                let _ = on_progress.send(progress);
            });
        log_timing(
            &format!("check_savegame_files {savegame_id} {profile_id}"),
            started,
        );
        report
    })
    .await
}

/// Not on the blocking pool: it only sets a flag, and must not queue behind the hashing.
#[tauri::command]
pub fn cancel_hashing(state: State<'_, AppState>) {
    state.library.cancel_hashing();
}

#[tauri::command]
pub async fn set_tags(
    state: State<'_, AppState>,
    mod_name: String,
    tags: Vec<String>,
) -> Result<Vec<String>, ApiError> {
    let library = state.library.clone();
    blocking(move || library.set_tags(&mod_name, &tags)).await
}

#[tauri::command]
pub async fn set_note(
    state: State<'_, AppState>,
    mod_name: String,
    note: String,
) -> Result<(), ApiError> {
    let library = state.library.clone();
    blocking(move || library.set_note(&mod_name, &note)).await
}

#[tauri::command]
pub async fn list_tags(state: State<'_, AppState>) -> Result<Vec<String>, ApiError> {
    let library = state.library.clone();
    blocking(move || library.list_tags()).await
}
