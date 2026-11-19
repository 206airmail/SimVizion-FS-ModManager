//! Commands for plan M8: Launch game, the Health report, the game folder the user chose, and
//! the app's own updates (`tauri-plugin-updater`, wired in Stage 2). Launch targets and every
//! path come from the core; the frontend sends none.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, RwLock};
use std::time::SystemTime;

use fsmm_core::discovery::{self, LaunchTarget, WindowsProbe};
use fsmm_core::library::app_update_due;
use fsmm_core::model::{
    ApiError, AppUpdate, AppUpdateSettings, ErrorCode, GameStatus, HealthReport, OpProgress,
};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::{Update, UpdaterExt};

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

/// The update the last check found, kept for `install_app_update` so the frontend never names
/// what to install.
static PENDING: Mutex<Option<Update>> = Mutex::new(None);

fn update_failed(e: impl std::fmt::Display) -> ApiError {
    ApiError::new(ErrorCode::AppUpdateFailed, Some(e.to_string()))
}

/// A newer app version from the release feed (`plugins.updater.endpoints` in
/// `tauri.conf.json`), or `None`. Refused with `UpdatesDisabled` while app updates are off;
/// without `force`, nothing is asked within 24 hours of the last successful check.
#[tauri::command]
pub async fn check_app_update(
    app: AppHandle,
    state: State<'_, AppState>,
    force: bool,
) -> Result<Option<AppUpdate>, ApiError> {
    let library = state.library.clone();
    let settings = {
        let library = library.clone();
        blocking(move || library.app_update_settings()).await?
    };
    if !settings.enabled {
        return Err(ApiError::new(ErrorCode::UpdatesDisabled, None));
    }
    if !app_update_due(&settings, SystemTime::now(), force) {
        return Ok(None);
    }
    let found = app
        .updater()
        .map_err(update_failed)?
        .check()
        .await
        .map_err(update_failed)?;
    blocking(move || library.note_app_update_check(SystemTime::now())).await?;
    let answer = found.as_ref().map(|update| AppUpdate {
        version: update.version.clone(),
        notes: update.body.clone(),
        date: update.raw_json["pub_date"].as_str().map(str::to_owned),
    });
    if let Ok(mut pending) = PENDING.lock() {
        *pending = found;
    }
    Ok(answer)
}

/// Downloads the update the last check found, verifies its signature against the public key in
/// `tauri.conf.json`, and runs its installer (passive); on Windows the plugin exits the app and
/// the installer starts the new version. Progress is reported in bytes.
#[tauri::command]
pub async fn install_app_update(
    app: AppHandle,
    on_progress: Channel<OpProgress>,
) -> Result<(), ApiError> {
    let update = PENDING
        .lock()
        .ok()
        .and_then(|pending| pending.clone())
        .ok_or_else(|| update_failed("noUpdatePending"))?;
    let mut done: u64 = 0;
    update
        .download_and_install(
            |chunk, total| {
                done += chunk as u64;
                let _ = on_progress.send(OpProgress {
                    done_bytes: done,
                    total_bytes: total.unwrap_or(0),
                    current: Some(update.version.clone()),
                    done_items: 0,
                    total_items: 1,
                });
            },
            || {},
        )
        .await
        .map_err(update_failed)?;
    // Windows exits inside `download_and_install`; elsewhere the new version needs a restart.
    app.restart()
}
