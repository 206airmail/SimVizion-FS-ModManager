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
//
// A failing command rejects with an ApiError object `{ code, detail }`.
// Thumbnails are not commands: ModEntry.icon is an http://thumb.localhost/... URL served by a
// custom protocol in src-tauri.

import { Channel, invoke } from '@tauri-apps/api/core';
import type { Api, ApiError, GameStatus, ModEntry, Profile, ScanProgress, SwitchResult } from '.';
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
  };
}
