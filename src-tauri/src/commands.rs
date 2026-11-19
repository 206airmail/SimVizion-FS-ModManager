//! One command per method of the frontend's `Api` (src/api/index.ts). Each runs the core work
//! on a blocking thread and fails with an `ApiError` the frontend words. Commands take ids,
//! never paths (plan M3 decision 5).

use std::time::Instant;

use fsmm_core::discovery::{self, GameContext, WindowsProbe};
use fsmm_core::model::{ApiError, ErrorCode, GameStatus, ModEntry, Profile, ScanProgress};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::AppState;

/// Runs `work` on the blocking pool; a panic there becomes an `internal` error.
pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or_else(|e| Err(internal(e)))
}

fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError::new(ErrorCode::Internal, Some(e.to_string()))
}

pub(crate) fn context() -> GameContext {
    discovery::game_context(&WindowsProbe)
}

/// Debug builds print how long the main calls take (plan M3 verification). Ids only.
fn log_timing(what: &str, started: Instant) {
    if cfg!(debug_assertions) {
        eprintln!(
            "[timing] {what}: {:.1} ms",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
}

#[tauri::command]
pub async fn get_game_status() -> Result<GameStatus, ApiError> {
    blocking(|| Ok(discovery::game_status(&WindowsProbe))).await
}

#[tauri::command]
pub async fn list_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        let started = Instant::now();
        let ctx = context();
        library.ensure_profiles(&ctx)?;
        let profiles = library.list_profiles(&ctx);
        log_timing("list_profiles", started);
        profiles
    })
    .await
}

#[tauri::command]
pub async fn list_mods(
    state: State<'_, AppState>,
    profile_id: String,
    on_progress: Channel<ScanProgress>,
) -> Result<Vec<ModEntry>, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        let started = Instant::now();
        let mods = library.list_mods(&profile_id, &context(), &|progress| {
            // A closed channel only means the screen moved on; the scan still completes.
            let _ = on_progress.send(progress);
        });
        log_timing(&format!("list_mods {profile_id}"), started);
        mods
    })
    .await
}

#[tauri::command]
pub async fn add_profile(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<Profile>, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        let mut picker = app.dialog().file();
        if let Some(window) = app.get_webview_window("main") {
            picker = picker.set_parent(&window);
        }
        let Some(chosen) = picker.blocking_pick_folder() else {
            return Ok(None);
        };
        let path = chosen.into_path().map_err(internal)?;
        library.add_profile(&path, &context()).map(Some)
    })
    .await
}

#[tauri::command]
pub async fn open_profile_folder(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<(), ApiError> {
    let library = state.library.clone();
    blocking(move || {
        let path = library.profile_path(&profile_id)?;
        tauri_plugin_opener::open_path(&path, None::<&str>).map_err(internal)
    })
    .await
}

#[tauri::command]
pub async fn show_mod_in_folder(
    state: State<'_, AppState>,
    profile_id: String,
    mod_id: String,
) -> Result<(), ApiError> {
    let library = state.library.clone();
    blocking(move || {
        let path = library.mod_path(&profile_id, &mod_id)?;
        tauri_plugin_opener::reveal_item_in_dir(path).map_err(internal)
    })
    .await
}
