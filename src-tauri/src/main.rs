// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod thumbs;

use std::fs;
use std::sync::Arc;

use fsmm_core::library::Library;
use fsmm_core::store::Store;
use tauri::Manager;

/// Shared by every command and the thumbnail protocol.
pub struct AppState {
    pub library: Arc<Library>,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running SimVizion Mod Manager");
}
