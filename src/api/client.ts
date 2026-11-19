// Chooses the Api implementation. Inside the desktop app it is always the Tauri one, backed by
// the Rust core, and the URL switches below are ignored. In a browser it is the mock, with a
// few URL switches for reviewing states. Join several with commas, e.g. ?mock=clashes,recycleFails.
//   ?mock=stress  adds a 5,000-mod profile      ?mock=slow   3 s latency (loading state)
//   ?mock=error   mod lists fail to load        ?mock=empty  every profile lists no mods
//   ?mock=running the game is running (Set as active, Copy, Move and Remove are disabled)
//   ?mock=server  the dedicated server is running (the same actions are disabled)
//   ?mock=brokenSettings  gameSettings.xml is unreadable and a backup exists; restoring fixes it
//   ?mock=writeFails      Set as active fails with a writeFailed error
//   ?mock=clashes         every copy or move plan has clashes (the target gets older twins)
//   ?mock=recycleFails    Remove and Replace fail with recycleUnavailable for some mods
//   ?mock=lowSpace        a copy or move plan shows too little free space on the target drive
//   ?mock=slowHash        checking a savegame's files takes about 5 s (progress and Cancel)
//   ?mock=brokenSave      adds a fourth savegame that cannot be read (savegameUnreadable)
//   ?mock=noSaves         the game has no savegames (the empty state)
//   ?mock=updatesOn       online updates start switched on, with a check from 40 minutes ago cached
//   ?mock=offline         update checks and installs fail with offline
//   ?mock=rateLimited     the first check that reaches the catalog is told to wait 5 minutes
//   ?mock=hashMismatch    installs download fully, then fail with verifyFailed
//   ?mock=noModHubLink    ModHub installs fail with notInstallable (no download link on the page)
//   ?mock=noGame          no game user folder and no profiles: the first-run screen; Launch game is
//                         unavailable; "Choose game folder" then reveals the normal data
//   ?mock=noProfiles      the game's user folder is found but no mod folder is listed (the first-run
//                         screen with "Add mod folder"; adding one lists the sample folders)
//   ?mock=msStore         the game is the Microsoft Store version (its user folder is in the app folder)
//   ?mock=appUpdate       app updates start switched on and a newer version (0.9.0-beta.2) is found;
//                         Install and restart streams progress for about 3 s, then resolves
//   ?mock=appUpdateFails  like appUpdate, but the install fails part-way with appUpdateFailed
//                         (?mock=offline also makes the app update check fail)
// Launch game resolves, and the game shows as running about 3 s later (it stays running until the
// page reloads); with ?mock=running it is refused with gameRunning.
// Online updates start switched off (nothing is matched, only manual links show). Turn them on in
// Settings; a check takes about 1 s and an install about 2 s per folder.
// Copies and removals in the mock run at about 40 MB/s, so big mods take seconds and can be cancelled.
// The sample dataset is compiled in only where __SAMPLE_BUILD__ is true (dev servers and plain
// browser builds); the production bundle that tauri build makes leaves it out (vite.config.ts).

import { isTauri } from '@tauri-apps/api/core';
import type { Api } from '.';
import { createMockApi } from './mock';
import { createTauriApi } from './tauri';

function createApi(): Api {
  if (isTauri()) return createTauriApi();
  if (!__SAMPLE_BUILD__) throw new Error('This build only runs inside the desktop app.');
  const modes = new Set((new URLSearchParams(window.location.search).get('mock') ?? '').split(','));
  const mock = createMockApi({
    stress: modes.has('stress'),
    delayMs: modes.has('slow') ? 3000 : 250,
    failMods: modes.has('error'),
    gameRunning: modes.has('running'),
    serverRunning: modes.has('server'),
    brokenSettings: modes.has('brokenSettings'),
    writeFails: modes.has('writeFails'),
    clashes: modes.has('clashes'),
    recycleFails: modes.has('recycleFails'),
    lowSpace: modes.has('lowSpace'),
    slowHash: modes.has('slowHash'),
    brokenSave: modes.has('brokenSave'),
    noSaves: modes.has('noSaves'),
    updatesOn: modes.has('updatesOn'),
    offline: modes.has('offline'),
    rateLimited: modes.has('rateLimited'),
    hashMismatch: modes.has('hashMismatch'),
    noModHubLink: modes.has('noModHubLink'),
    noGame: modes.has('noGame'),
    msStore: modes.has('msStore'),
    noProfiles: modes.has('noProfiles'),
    appUpdate: modes.has('appUpdate'),
    appUpdateFails: modes.has('appUpdateFails'),
  });
  if (!modes.has('empty')) return mock;
  // Spread drops nothing the interface needs: `sampleData` is a plain property.
  return {
    ...mock,
    listMods: async () => [],
    healthReport: async () => ({ rows: [], counts: [] }),
  };
}

export const api: Api = createApi();
