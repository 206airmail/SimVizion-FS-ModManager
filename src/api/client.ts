// Chooses the Api implementation. Inside the desktop app it is always the Tauri one, backed by
// the Rust core, and the URL switches below are ignored. In a browser it is the mock, with a
// few URL switches for reviewing states:
//   ?mock=stress  adds a 5,000-mod profile      ?mock=slow   3 s latency (loading state)
//   ?mock=error   mod lists fail to load        ?mock=empty  every profile lists no mods
//   ?mock=running the game is running (Set as active is disabled)
//   ?mock=server  the dedicated server is running (Set as active is disabled)
//   ?mock=brokenSettings  gameSettings.xml is unreadable and a backup exists; restoring fixes it
//   ?mock=writeFails      Set as active fails with a writeFailed error

import { isTauri } from '@tauri-apps/api/core';
import type { Api } from '.';
import { createMockApi } from './mock';
import { createTauriApi } from './tauri';

function createApi(): Api {
  if (isTauri()) return createTauriApi();
  const mode = new URLSearchParams(window.location.search).get('mock');
  const mock = createMockApi({
    stress: mode === 'stress',
    delayMs: mode === 'slow' ? 3000 : 250,
    failMods: mode === 'error',
    gameRunning: mode === 'running',
    serverRunning: mode === 'server',
    brokenSettings: mode === 'brokenSettings',
    writeFails: mode === 'writeFails',
  });
  if (mode !== 'empty') return mock;
  // Spread drops nothing the interface needs: `sampleData` is a plain property.
  return { ...mock, listMods: async () => [] };
}

export const api: Api = createApi();
