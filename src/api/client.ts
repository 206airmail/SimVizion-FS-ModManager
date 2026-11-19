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
// Copies and removals in the mock run at about 40 MB/s, so big mods take seconds and can be cancelled.

import { isTauri } from '@tauri-apps/api/core';
import type { Api } from '.';
import { createMockApi } from './mock';
import { createTauriApi } from './tauri';

function createApi(): Api {
  if (isTauri()) return createTauriApi();
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
  });
  if (!modes.has('empty')) return mock;
  // Spread drops nothing the interface needs: `sampleData` is a plain property.
  return { ...mock, listMods: async () => [] };
}

export const api: Api = createApi();
