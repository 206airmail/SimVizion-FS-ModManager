// Chooses the Api implementation. Until the Tauri commands exist (milestone 3) this is always
// the mock, with a few URL switches for reviewing states:
//   ?mock=stress  adds a 5,000-mod profile      ?mock=slow   3 s latency (loading state)
//   ?mock=error   mod lists fail to load        ?mock=empty  every profile lists no mods

import type { Api } from '.';
import { createMockApi } from './mock';

function createApi(): Api {
  const mode = new URLSearchParams(window.location.search).get('mock');
  const mock = createMockApi({
    stress: mode === 'stress',
    delayMs: mode === 'slow' ? 3000 : 250,
    failMods: mode === 'error',
  });
  if (mode !== 'empty') return mock;
  return { ...mock, listMods: async () => [] };
}

export const api: Api = createApi();
