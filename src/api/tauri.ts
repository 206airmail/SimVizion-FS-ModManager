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
//
// A failing command rejects with an ApiError object `{ code, detail }`.
// Thumbnails are not commands: ModEntry.icon is an http://thumb.localhost/... URL served by a
// custom protocol in src-tauri.

import type { Api } from '.';

export function createTauriApi(): Api {
  throw new Error('WS-C: not implemented yet');
}
