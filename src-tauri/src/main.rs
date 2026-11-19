// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod insight;
mod switching;
mod thumbs;
mod transfer;
mod updates;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use fsmm_core::library::Library;
use fsmm_core::store::Store;
use tauri::Manager;

/// Shared by every command and the thumbnail protocol.
pub struct AppState {
    pub library: Arc<Library>,
    /// Where `gameSettings.xml` is backed up before every write (plan M4).
    pub backups_dir: PathBuf,
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // The app's own files only: user data and a disposable cache (plan M3 decision 3).
            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            fs::create_dir_all(&data_dir)?;
            fs::create_dir_all(&cache_dir)?;
            let store = Store::open(
                &data_dir.join("library.sqlite"),
                &cache_dir.join("cache.sqlite"),
            )?;
            app.manage(AppState {
                library: Arc::new(Library::new(store, thumbs::BASE_URL)),
                backups_dir: data_dir.join("backups"),
            });
            Ok(())
        })
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running SimVizion Mod Manager");
}
