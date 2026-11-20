//! Commands for copying, moving and removing mods and for managing the profile list (plan M5).
//! The logic is in `Library`; this layer supplies the real Recycle Bin, disk space and process
//! check. Mod files are only ever recycled, never deleted (`fsmm_core::files::WindowsRecycler`).

use fsmm_core::discovery::{self, Running, WindowsProbe};
use fsmm_core::files::{WindowsDiskSpace, WindowsRecycler};
use fsmm_core::library::OpEnv;
use fsmm_core::model::{ApiError, ClashChoice, OpProgress, OpResult, TransferMode, TransferPlan};
use tauri::ipc::Channel;
use tauri::State;

use crate::commands::{blocking, context};
use crate::switching::writes_allowed;
use crate::AppState;

fn running() -> Running {
    discovery::running(&WindowsProbe)
}

pub(crate) fn env() -> OpEnv<'static> {
    OpEnv {
        running: &running,
        recycler: &WindowsRecycler,
        disk: &WindowsDiskSpace,
    }
}

#[tauri::command]
pub async fn plan_transfer(
    state: State<'_, AppState>,
    from_profile_id: String,
    mod_ids: Vec<String>,
    to_profile_id: String,
    mode: TransferMode,
) -> Result<TransferPlan, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        library.plan_transfer(
            &from_profile_id,
            &mod_ids,
            &to_profile_id,
            mode,
            &context(),
            &env(),
        )
    })
    .await
}

#[tauri::command]
pub async fn execute_transfer(
    state: State<'_, AppState>,
    plan_id: String,
    choices: Vec<ClashChoice>,
    on_progress: Channel<OpProgress>,
) -> Result<OpResult, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        writes_allowed()?;
        library.execute_transfer(&plan_id, &choices, &context(), &env(), &|progress| {
            let _ = on_progress.send(progress);
        })
    })
    .await
}

#[tauri::command]
pub async fn remove_mods(
    state: State<'_, AppState>,
    profile_id: String,
    mod_ids: Vec<String>,
    on_progress: Channel<OpProgress>,
) -> Result<OpResult, ApiError> {
    let library = state.library.clone();
    blocking(move || {
        writes_allowed()?;
        library.remove_mods(&profile_id, &mod_ids, &context(), &env(), &|progress| {
            let _ = on_progress.send(progress);
        })
    })
    .await
}

/// Not on the blocking pool: it only sets a flag, and must not queue behind the operation.
#[tauri::command]
pub fn cancel_operation(state: State<'_, AppState>) {
    state.library.cancel_operation();
}

#[tauri::command]
pub async fn rename_profile(
    state: State<'_, AppState>,
    profile_id: String,
    name: String,
) -> Result<(), ApiError> {
    let library = state.library.clone();
    blocking(move || library.rename_profile(&profile_id, &name)).await
}

#[tauri::command]
pub async fn remove_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<(), ApiError> {
    let library = state.library.clone();
    blocking(move || library.remove_profile(&profile_id, &context())).await
}
