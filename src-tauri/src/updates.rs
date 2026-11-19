//! Commands for mod updates (plan M7). The logic is in `Library`; the network is
//! `fsmm_core::net::Http`, created on first use so nothing network-related exists until the
//! user turns updates on and something asks. URLs are resolved in the core (`link_url`); the
//! frontend never sends one.

use std::sync::OnceLock;
use std::time::{Instant, SystemTime};

use fsmm_core::library::UpdateEnv;
use fsmm_core::model::{
    ApiError, ErrorCode, LinkKind, OpProgress, OpResult, UpdateCheck, UpdatePlan, UpdateSettings,
};
use fsmm_core::net::Http;
use tauri::ipc::Channel;
use tauri::State;

use crate::commands::{blocking, context, log_timing};
use crate::AppState;

fn http() -> &'static Http {
    static HTTP: OnceLock<Http> = OnceLock::new();
    HTTP.get_or_init(|| Http::new(env!("CARGO_PKG_VERSION")))
}

fn now() -> SystemTime {
    SystemTime::now()
}

#[tauri::command]
pub async fn get_update_settings(state: State<'_, AppState>) -> Result<UpdateSettings, ApiError> {
    let library = state.library.clone();
    blocking(move || library.update_settings()).await
}

#[tauri::command]
pub async fn set_updates_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<UpdateSettings, ApiError> {
    let library = state.library.clone();
    blocking(move || library.set_updates_enabled(enabled)).await
}

#[tauri::command]
pub async fn check_for_updates(
    state: State<'_, AppState>,
    force: bool,
) -> Result<UpdateCheck, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        let started = Instant::now();
        let files = crate::transfer::env();
        let env = UpdateEnv {
            catalog: http(),
            web: http(),
            files: &files,
            now: &now,
        };
        let check = library.check_updates(force, &context(), &env);
        log_timing("check_for_updates", started);
        check
    })
    .await
}

#[tauri::command]
pub async fn plan_update(
    state: State<'_, AppState>,
    mod_name: String,
) -> Result<UpdatePlan, ApiError> {
    let library = state.library.clone();
    blocking(move || library.plan_update(&mod_name, &context())).await
}

#[tauri::command]
pub async fn install_update(
    state: State<'_, AppState>,
    mod_name: String,
    profile_ids: Vec<String>,
    on_progress: Channel<OpProgress>,
) -> Result<OpResult, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        let files = crate::transfer::env();
        let env = UpdateEnv {
            catalog: http(),
            web: http(),
            files: &files,
            now: &now,
        };
        library.install_update(&mod_name, &profile_ids, &context(), &env, &|progress| {
            let _ = on_progress.send(progress);
        })
    })
    .await
}

/// Opens one of the mod's pages in the default browser. The URL comes from the core.
#[tauri::command]
pub async fn open_mod_link(
    state: State<'_, AppState>,
    mod_name: String,
    kind: LinkKind,
) -> Result<(), ApiError> {
    let library = state.library.clone();
    blocking(move || {
        let url = library.link_url(&mod_name, kind)?;
        tauri_plugin_opener::open_url(url, None::<&str>)
            .map_err(|e| ApiError::new(ErrorCode::Internal, Some(e.to_string())))
    })
    .await
}

#[tauri::command]
pub async fn set_source_link(
    state: State<'_, AppState>,
    mod_name: String,
    url: Option<String>,
) -> Result<Option<String>, ApiError> {
    let library = state.library.clone();
    blocking(move || library.set_source_link(&mod_name, url.as_deref())).await
}
