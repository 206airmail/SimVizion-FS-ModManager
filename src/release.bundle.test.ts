// The release bundle check (`npm run check:bundle`): builds the production bundle twice, in memory,
// and looks inside. The bundle `tauri build` makes (the Tauri CLI sets TAURI_ENV_PLATFORM while it
// runs the frontend build) must hold neither the sample dataset nor the design gallery; the plain
// browser build must still hold both, which proves the markers below can be found at all.
// It is slow (two full builds), so `npm test` skips it: see vite.config.ts.

import { build } from 'vite';
import { describe, expect, it } from 'vitest';

// The Node global, declared here because the project carries no Node typings.
declare const process: { env: Record<string, string | undefined> };

/** Strings that only the sample dataset has (profile names, invented brands, the mock's path). */
const MOCK_MARKERS = [
  'Offline Testing',
  'Stress Test',
  'Ostrander',
  'Brannock',
  'FarmingSimulator25PC_0000000000000',
];
/** Strings that only the gallery has. */
const GALLERY_MARKERS = [
  'Buttons, compact (28px)',
  'Checkbox and switch (lime when on',
  'mm-gallery',
];

async function bundleText(tauriBuild: boolean): Promise<string> {
  const before = process.env.TAURI_ENV_PLATFORM;
  if (tauriBuild) process.env.TAURI_ENV_PLATFORM = 'windows';
  else delete process.env.TAURI_ENV_PLATFORM;
  try {
    const result = await build({
      configFile: 'vite.config.ts',
      mode: 'production',
      logLevel: 'silent',
      build: { write: false },
    });
    const outputs = (Array.isArray(result) ? result : [result]).flatMap((entry) =>
      'output' in entry ? entry.output : [],
    );
    return outputs
      .map((file) => (file.type === 'chunk' ? file.code : String(file.source)))
      .join('\n');
  } finally {
    if (before === undefined) delete process.env.TAURI_ENV_PLATFORM;
    else process.env.TAURI_ENV_PLATFORM = before;
  }
}

describe('release bundle', () => {
  it('has neither the sample dataset nor the gallery when built for the desktop app', async () => {
    const text = await bundleText(true);
    // A bundle that is empty or broken would pass the checks below for the wrong reason.
    expect(text.length).toBeGreaterThan(100_000);
    expect(text).toContain('createRoot');
    for (const marker of [...MOCK_MARKERS, ...GALLERY_MARKERS]) {
      expect(text.includes(marker), `release bundle contains "${marker}"`).toBe(false);
    }
  });

  it('keeps both in the plain browser build', async () => {
    const text = await bundleText(false);
    for (const marker of [...MOCK_MARKERS, ...GALLERY_MARKERS]) {
      expect(text.includes(marker), `browser build lacks "${marker}"`).toBe(true);
    }
  });
});
