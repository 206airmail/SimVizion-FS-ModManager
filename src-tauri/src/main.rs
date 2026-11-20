// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod insight;
mod release;
mod switching;
mod thumbs;
mod transfer;
mod updates;
#[cfg(test)]
mod version_sync;

use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fsmm_core::library::Library;
use fsmm_core::store::Store;
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

/// Shared by every command and the thumbnail protocol.
pub struct AppState {
    pub library: Arc<Library>,
    /// Where `gameSettings.xml` is backed up before every write (plan M4).
    pub backups_dir: PathBuf,
}

const APP_TITLE: &str = "SimVizion Mod Manager";

/// Opens the app's own files and sets up the state every command shares.
fn start(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // The app's own files only: user data and a disposable cache (plan M3 decision 3).
    let data_dir = app.path().app_data_dir()?;
    let cache_dir = app.path().app_cache_dir()?;
    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(&cache_dir)?;
    let store = Store::open(
        &data_dir.join("library.sqlite"),
        &cache_dir.join("cache.sqlite"),
    )?;
    let library = Arc::new(Library::new(store, thumbs::BASE_URL));
    // The game folder the user chose (plan M8) applies from the first command on.
    release::set_chosen(library.chosen_game_folder()?);
    app.manage(AppState {
        library,
        backups_dir: data_dir.join("backups"),
    });
    Ok(())
}

/// What the user is told when the app cannot start. A release build has no console, so without
/// this the window never appears and nothing says why.
fn startup_message(data_dir: Option<&Path>, error: &dyn Display) -> String {
    let folder = data_dir.map_or_else(String::new, |dir| {
        format!(
            "\n\nThe app keeps its library in this folder:\n{}\n\nA library file from a newer \
             version of the app, or a damaged one, can stop it from opening. Install the newest \
             version, or move the file library.sqlite out of that folder (the app then starts \
             with a fresh library; your mods are not touched).",
            dir.display()
        )
    });
    format!("{APP_TITLE} could not start.\n\n{error}{folder}")
}

/// Shows the start-up failure in a native message box, then returns so the app exits. Runs on
/// the main thread during setup, where the blocking form is safe: the dialog runs on its own
/// thread.
fn show_startup_error(app: &tauri::App, error: &dyn Display) {
    let data_dir = app.path().app_data_dir().ok();
    app.dialog()
        .message(startup_message(data_dir.as_deref(), error))
        .title(APP_TITLE)
        .kind(MessageDialogKind::Error)
        .blocking_show();
}

/// A second launch brings the first window forward.
fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Closing the window ends the process. While a file operation, a mod update or check, or
/// savegame hashing is running that would stop it half way (a copy would leave its partial file
/// behind; a replace could leave the old mod in the Recycle Bin and the new one under a hidden
/// name), so the close is refused and the user is told why.
fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    let app = window.app_handle();
    let running = app
        .try_state::<AppState>()
        .is_some_and(|state| state.library.operation_running());
    if running {
        api.prevent_close();
        // Not the blocking form: this runs on the main thread, which must keep running.
        app.dialog()
            .message(
                "An operation is still running (copying, moving, removing or updating mods, \
                 or checking a save game). Wait for it to finish, or cancel it first, then \
                 close the app.",
            )
            .title(APP_TITLE)
            .kind(MessageDialogKind::Warning)
            .show(|_| {});
    }
}

fn main() {
    tauri::Builder::default()
        // First, so a second copy hands over before it opens anything of its own.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| start(app).inspect_err(|error| show_startup_error(app, error)))
        .on_window_event(on_window_event)
        .register_asynchronous_uri_scheme_protocol("thumb", thumbs::handle)
        .invoke_handler(tauri::generate_handler![
            commands::get_game_status,
            commands::list_profiles,
            commands::list_mods,
            commands::add_profile,
            commands::open_profile_folder,
            commands::show_mod_in_folder,
            switching::set_active_profile,
            switching::undo_switch,
            switching::latest_settings_backup,
            switching::restore_settings_backup,
            transfer::plan_transfer,
            transfer::execute_transfer,
            transfer::remove_mods,
            transfer::cancel_operation,
            transfer::rename_profile,
            transfer::remove_profile,
            insight::compare_profiles,
            insight::list_savegames,
            insight::savegame_report,
            insight::check_savegame_files,
            insight::cancel_hashing,
            insight::set_tags,
            insight::set_note,
            insight::list_tags,
            updates::get_update_settings,
            updates::set_updates_enabled,
            updates::check_for_updates,
            updates::plan_update,
            updates::install_update,
            updates::open_mod_link,
            updates::set_source_link,
            release::launch_game,
            release::health_report,
            release::choose_game_folder,
            release::clear_game_folder,
            release::get_app_update_settings,
            release::set_app_updates_enabled,
            release::check_app_update,
            release::install_app_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SimVizion Mod Manager");
}
