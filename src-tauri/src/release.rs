//! Commands for plan M8: Launch game, the Health report, the game folder the user chose, and
//! the app's own updates (`tauri-plugin-updater`, wired in Stage 2). Launch targets and every
//! path come from the core; the frontend sends none.

use std::path::PathBuf;
use std::process::Command;
use std::sync::RwLock;

use fsmm_core::discovery::{self, LaunchTarget, WindowsProbe};
use fsmm_core::model::{
    ApiError, AppUpdate, AppUpdateSettings, ErrorCode, GameStatus, HealthReport, OpProgress,
};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::commands::{blocking, context};
use crate::AppState;

/// The user folder chosen with "Choose game folder", loaded at startup and changed by the
/// commands below; `commands::context` and `get_game_status` read it.
static CHOSEN: RwLock<Option<PathBuf>> = RwLock::new(None);

pub(crate) fn chosen_game_folder() -> Option<PathBuf> {
    CHOSEN.read().map(|c| c.clone()).unwrap_or(None)
}

pub(crate) fn set_chosen(dir: Option<PathBuf>) {
    if let Ok(mut chosen) = CHOSEN.write() {
        *chosen = dir;
    }
}

fn status() -> GameStatus {
    discovery::game_status(&WindowsProbe, chosen_game_folder().as_deref())
}

fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError::new(ErrorCode::Internal, Some(e.to_string()))
}

#[tauri::command]
pub async fn launch_game() -> Result<(), ApiError> {
    blocking(|| {
        match discovery::launch_target(&WindowsProbe)? {
            LaunchTarget::Url(url) => {
                tauri_plugin_opener::open_url(url, None::<&str>).map_err(internal)?;
            }
            LaunchTarget::Program { exe, dir } => {
                Command::new(exe)
                    .current_dir(dir)
                    .spawn()
                    .map_err(internal)?;
            }
            LaunchTarget::AppsFolder(id) => {
                Command::new("explorer.exe")
                    .arg(format!(r"shell:AppsFolder\{id}"))
                    .spawn()
                    .map_err(internal)?;
            }
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn health_report(state: State<'_, AppState>) -> Result<HealthReport, ApiError> {
    let library = state.library.clone();
    blocking(move || library.health_report(&context())).await
}

/// Asks for the game's user folder; `None` when the picker was cancelled. The folder is
/// validated and stored in the core, then used for every later command.
#[tauri::command]
pub async fn choose_game_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<GameStatus>, ApiError> {
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
        library.set_chosen_game_folder(Some(&path))?;
        set_chosen(Some(path));
        Ok(Some(status()))
    })
    .await
}

/// Forgets the chosen folder: detection decides again.
#[tauri::command]
pub async fn clear_game_folder(state: State<'_, AppState>) -> Result<GameStatus, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        library.set_chosen_game_folder(None)?;
        set_chosen(None);
        Ok(status())
    })
    .await
}

#[tauri::command]
pub async fn get_app_update_settings(
    state: State<'_, AppState>,
) -> Result<AppUpdateSettings, ApiError> {
    let library = state.library.clone();
    blocking(move || library.app_update_settings()).await
}

#[tauri::command]
pub async fn set_app_updates_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<AppUpdateSettings, ApiError> {
    let library = state.library.clone();
    blocking(move || library.set_app_updates_enabled(enabled)).await
}

/// A newer app version from the release feed, or `None`. Refused while app updates are off;
/// without `force`, nothing is asked within 24 hours of the last check (Stage 2).
#[tauri::command]
pub async fn check_app_update(
    _app: AppHandle,
    _state: State<'_, AppState>,
    _force: bool,
) -> Result<Option<AppUpdate>, ApiError> {
    todo!("Stage 2: tauri-plugin-updater")
}

/// Downloads the update found by `check_app_update`, verifies its signature, runs the installer
/// (passive) and restarts the app (Stage 2).
#[tauri::command]
pub async fn install_app_update(
    _app: AppHandle,
    _on_progress: Channel<OpProgress>,
) -> Result<(), ApiError> {
    todo!("Stage 2: tauri-plugin-updater")
}
