// The Api backed by the Rust core through Tauri commands (src-tauri/src/commands.rs).
//
// Commands, with their arguments as sent by `invoke` (Tauri maps camelCase to the Rust
// snake_case parameters) and their results:
//
//   get_game_status                                         -> GameStatus
//   list_profiles                                           -> Profile[]
//   list_mods          { profileId, onProgress: Channel<ScanProgress> } -> ModEntry[]
//   add_profile                                             -> Profile | null
//   open_profile_folder { profileId }                       -> null
//   show_mod_in_folder  { profileId, modId }                -> null
//   set_active_profile  { profileId }                       -> SwitchResult
//   undo_switch         { opId }                            -> SwitchResult
//   latest_settings_backup                                  -> string | null (ISO time)
//   restore_settings_backup                                 -> null
//   plan_transfer      { fromProfileId, modIds, toProfileId, mode } -> TransferPlan
//   execute_transfer   { planId, choices, onProgress: Channel<OpProgress> } -> OpResult
//   remove_mods        { profileId, modIds, onProgress: Channel<OpProgress> } -> OpResult
//   cancel_operation                                        -> null
//   rename_profile     { profileId, name }                  -> null
//   remove_profile     { profileId }                        -> null
//   compare_profiles   { leftProfileId, rightProfileId }    -> Comparison
//   list_savegames                                          -> Savegame[]
//   savegame_report    { savegameId, profileId }            -> SavegameReport
//   check_savegame_files { savegameId, profileId, onProgress: Channel<HashProgress> } -> SavegameReport
//   cancel_hashing                                          -> null
//   set_tags           { modName, tags }                    -> string[]
//   set_note           { modName, note }                    -> null
//   list_tags                                               -> string[]
//   get_update_settings                                     -> UpdateSettings
//   set_updates_enabled { enabled }                         -> UpdateSettings
//   check_for_updates  { force }                            -> UpdateCheck
//   plan_update        { modName }                          -> UpdatePlan
//   install_update     { modName, profileIds, onProgress: Channel<OpProgress> } -> OpResult
//   open_mod_link      { modName, kind }                    -> null
//   set_source_link    { modName, url }                     -> string | null
//   launch_game                                             -> null
//   health_report                                           -> HealthReport
//   choose_game_folder                                      -> GameStatus | null
//   clear_game_folder                                       -> GameStatus
//   get_app_update_settings                                 -> AppUpdateSettings
//   set_app_updates_enabled { enabled }                     -> AppUpdateSettings
//   check_app_update   { force }                            -> AppUpdate | null
//   install_app_update { onProgress: Channel<OpProgress> }  -> null (the app restarts)
//
// A failing command rejects with an ApiError object `{ code, detail }`.
// Thumbnails are not commands: ModEntry.icon is an http://thumb.localhost/... URL served by a
// custom protocol in src-tauri.

import { Channel, invoke } from '@tauri-apps/api/core';
import type {
  Api,
  ApiError,
  AppUpdate,
  AppUpdateSettings,
  ClashChoice,
  Comparison,
  GameStatus,
  HashProgress,
  HealthReport,
  LinkKind,
  ModEntry,
  OpProgress,
  OpResult,
  Profile,
  Savegame,
  SavegameReport,
  ScanProgress,
  SwitchResult,
  TransferMode,
  TransferPlan,
  UpdateCheck,
  UpdatePlan,
  UpdateSettings,
} from '.';
import { ApiFailure, isErrorCode } from '../lib/errors';

function isApiError(value: unknown): value is ApiError {
  return typeof value === 'object' && value !== null && 'code' in value;
}

/**
 * What a rejected `invoke` becomes. The core rejects with an `ApiError` object; anything else
 * (a bridge error, a plain string) is an `internal` failure.
 */
export function toApiFailure(error: unknown): ApiFailure {
  if (error instanceof ApiFailure) return error;
  if (isApiError(error) && isErrorCode(error.code)) {
    return new ApiFailure(error.code, typeof error.detail === 'string' ? error.detail : null);
  }
  const detail = error instanceof Error ? error.message : typeof error === 'string' ? error : null;
  return new ApiFailure('internal', detail);
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toApiFailure(error);
  }
}

export function createTauriApi(): Api {
  return {
    sampleData: false,
    getGameStatus: () => call<GameStatus>('get_game_status'),
    listProfiles: () => call<Profile[]>('list_profiles'),
    listMods(profileId: string, onProgress?: (progress: ScanProgress) => void) {
      const channel = new Channel<ScanProgress>();
      channel.onmessage = (progress) => onProgress?.(progress);
      return call<ModEntry[]>('list_mods', { profileId, onProgress: channel });
    },
    addProfile: () => call<Profile | null>('add_profile'),
    async openProfileFolder(profileId: string) {
      await call('open_profile_folder', { profileId });
    },
    async showModInFolder(profileId: string, modId: string) {
      await call('show_mod_in_folder', { profileId, modId });
    },
    setActiveProfile: (profileId: string) =>
      call<SwitchResult>('set_active_profile', { profileId }),
    undoSwitch: (opId: string) => call<SwitchResult>('undo_switch', { opId }),
    latestSettingsBackup: () => call<string | null>('latest_settings_backup'),
    async restoreSettingsBackup() {
      await call('restore_settings_backup');
    },
    planTransfer: (
      fromProfileId: string,
      modIds: string[],
      toProfileId: string,
      mode: TransferMode,
    ) => call<TransferPlan>('plan_transfer', { fromProfileId, modIds, toProfileId, mode }),
    executeTransfer(planId: string, choices: ClashChoice[], onProgress?: (p: OpProgress) => void) {
      const channel = new Channel<OpProgress>();
      channel.onmessage = (progress) => onProgress?.(progress);
      return call<OpResult>('execute_transfer', { planId, choices, onProgress: channel });
    },
    removeMods(profileId: string, modIds: string[], onProgress?: (p: OpProgress) => void) {
      const channel = new Channel<OpProgress>();
      channel.onmessage = (progress) => onProgress?.(progress);
      return call<OpResult>('remove_mods', { profileId, modIds, onProgress: channel });
    },
    async cancelOperation() {
      await call('cancel_operation');
    },
    async renameProfile(profileId: string, name: string) {
      await call('rename_profile', { profileId, name });
    },
    async removeProfile(profileId: string) {
      await call('remove_profile', { profileId });
    },
    compareProfiles(leftProfileId: string, rightProfileId: string) {
      return call<Comparison>('compare_profiles', { leftProfileId, rightProfileId });
    },
    listSavegames() {
      return call<Savegame[]>('list_savegames');
    },
    savegameReport(savegameId: string, profileId: string) {
      return call<SavegameReport>('savegame_report', { savegameId, profileId });
    },
    checkSavegameFiles(
      savegameId: string,
      profileId: string,
      onProgress?: (p: HashProgress) => void,
    ) {
      const channel = new Channel<HashProgress>();
      channel.onmessage = (progress) => onProgress?.(progress);
      return call<SavegameReport>('check_savegame_files', {
        savegameId,
        profileId,
        onProgress: channel,
      });
    },
    async cancelHashing() {
      await call('cancel_hashing');
    },
    setTags(modName: string, tags: string[]) {
      return call<string[]>('set_tags', { modName, tags });
    },
    async setNote(modName: string, note: string) {
      await call('set_note', { modName, note });
    },
    listTags() {
      return call<string[]>('list_tags');
    },
    getUpdateSettings() {
      return call<UpdateSettings>('get_update_settings');
    },
    setUpdatesEnabled(enabled: boolean) {
      return call<UpdateSettings>('set_updates_enabled', { enabled });
    },
    checkForUpdates(force: boolean) {
      return call<UpdateCheck>('check_for_updates', { force });
    },
    planUpdate(modName: string) {
      return call<UpdatePlan>('plan_update', { modName });
    },
    installUpdate(modName: string, profileIds: string[], onProgress?: (p: OpProgress) => void) {
      const channel = new Channel<OpProgress>();
      channel.onmessage = (progress) => onProgress?.(progress);
      return call<OpResult>('install_update', { modName, profileIds, onProgress: channel });
    },
    async openModLink(modName: string, kind: LinkKind) {
      await call('open_mod_link', { modName, kind });
    },
    setSourceLink(modName: string, url: string | null) {
      return call<string | null>('set_source_link', { modName, url });
    },
    async launchGame() {
      await call('launch_game');
    },
    healthReport() {
      return call<HealthReport>('health_report');
    },
    chooseGameFolder() {
      return call<GameStatus | null>('choose_game_folder');
    },
    clearGameFolder() {
      return call<GameStatus>('clear_game_folder');
    },
    getAppUpdateSettings() {
      return call<AppUpdateSettings>('get_app_update_settings');
    },
    setAppUpdatesEnabled(enabled: boolean) {
      return call<AppUpdateSettings>('set_app_updates_enabled', { enabled });
    },
    checkAppUpdate(force: boolean) {
      return call<AppUpdate | null>('check_app_update', { force });
    },
    async installAppUpdate(onProgress?: (p: OpProgress) => void) {
      const channel = new Channel<OpProgress>();
      channel.onmessage = (progress) => onProgress?.(progress);
      await call('install_app_update', { onProgress: channel });
    },
  };
}
