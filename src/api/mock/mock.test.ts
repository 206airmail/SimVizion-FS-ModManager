import { beforeAll, describe, expect, it } from 'vitest';
import type { ModEntry, ScanProgress } from '..';
import { ApiFailure } from '../../lib/errors';
import { GAME_DESC_VERSION } from './entry';
import { createMockApi } from '.';

describe('mock issue details', () => {
  let mods: ModEntry[] = [];

  beforeAll(async () => {
    const api = createMockApi({ delayMs: 0, stress: true });
    const profiles = await api.listProfiles();
    mods = (await Promise.all(profiles.map((p) => api.listMods(p.id)))).flat();
  });

  const issues = (kind: string) =>
    mods.flatMap((mod) =>
      mod.issues.filter((issue) => issue.kind === kind).map((i) => ({ mod, i })),
    );

  it('has data, not sentences: no detail contains a space except dependency lists', () => {
    for (const mod of mods) {
      for (const issue of mod.issues) {
        if (issue.detail === null || issue.kind === 'missingDependency') continue;
        expect(issue.detail, `${issue.kind} on ${mod.fileName}`).not.toMatch(/\s/);
      }
    }
  });

  it('uses only the documented formats', () => {
    for (const { i } of issues('notAMod')) {
      expect(['unreadableArchive', 'noModDesc', 'invalidModDesc']).toContain(i.detail);
    }
    for (const { mod, i } of issues('conflictCopy')) {
      expect(mod.modName.startsWith(`${i.detail ?? '?'}.sync-conflict-`)).toBe(true);
    }
    for (const { mod, i } of issues('zipAndFolderTwin')) {
      const other = mods.find((m) => m.profileId === mod.profileId && m.fileName === i.detail);
      expect(other?.modName, mod.fileName).toBe(mod.modName);
    }
    for (const { i } of issues('nestedArchive')) expect(i.detail).toMatch(/\.zip$/);
    for (const { i } of issues('invalidFileName')) expect(i.detail).toBeNull();
    for (const { mod, i } of issues('missingDependency')) {
      const missing = mod.dependencies.filter((d) => !d.satisfied).map((d) => d.modName);
      expect(i.detail).toBe(missing.join(', '));
    }
  });

  it('has every issue kind', () => {
    for (const kind of [
      'notAMod',
      'invalidFileName',
      'nestedArchive',
      'zipAndFolderTwin',
      'conflictCopy',
      'descVersionMismatch',
      'missingDependency',
    ]) {
      expect(issues(kind).length, kind).toBeGreaterThan(0);
    }
  });

  it('flags only a descVersion above the game value, as a warning with the game value', () => {
    const flagged = issues('descVersionMismatch');
    for (const { mod, i } of flagged) {
      expect(mod.descVersion ?? 0, mod.fileName).toBeGreaterThan(GAME_DESC_VERSION);
      expect(i.severity).toBe('warning');
      expect(i.detail).toBe(String(GAME_DESC_VERSION));
    }
    for (const mod of mods) {
      const isFlagged = mod.issues.some((i) => i.kind === 'descVersionMismatch');
      expect((mod.descVersion ?? 0) > GAME_DESC_VERSION, mod.fileName).toBe(isFlagged);
    }
  });
});

describe('mock listMods', () => {
  it('reports progress ending at the full count', async () => {
    const api = createMockApi({ delayMs: 0 });
    const seen: ScanProgress[] = [];
    const mods = await api.listMods('main', (progress) => seen.push(progress));
    expect(seen.length).toBeGreaterThan(1);
    expect(seen.map((p) => p.done)).toEqual([...seen.map((p) => p.done)].sort((a, b) => a - b));
    expect(seen.at(-1)).toEqual({ profileId: 'main', done: mods.length, total: mods.length });
  });

  it('fails with a worded ApiFailure when asked to', async () => {
    const error = await createMockApi({ delayMs: 0, failMods: true })
      .listMods('main')
      .catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiFailure);
    expect((error as ApiFailure).code).toBe('folderUnreadable');
  });

  it('keeps the new actions harmless', async () => {
    const api = createMockApi({ delayMs: 0 });
    expect(api.sampleData).toBe(true);
    expect(await api.addProfile()).toBeNull();
    await expect(api.openProfileFolder('main')).resolves.toBeUndefined();
    await expect(api.showModInFolder('main', 'x')).resolves.toBeUndefined();
  });
});
