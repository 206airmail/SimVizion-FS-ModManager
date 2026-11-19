import { beforeAll, describe, expect, it } from 'vitest';
import type { ModEntry, ScanProgress } from '..';
import { ApiFailure } from '../../lib/errors';
import { GAME_DESC_VERSION } from './entry';
import { createMockApi, MOCK_BACKUP_AT } from '.';

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

describe('mock switching', () => {
  const failureOf = async (promise: Promise<unknown>) => {
    const error = await promise.then(
      () => null,
      (e: unknown) => e,
    );
    expect(error).toBeInstanceOf(ApiFailure);
    return (error as ApiFailure).code;
  };
  const activeIds = async (api: ReturnType<typeof createMockApi>) =>
    (await api.listProfiles()).filter((p) => p.isActive).map((p) => p.id);

  it('starts with Testing active and the override on', async () => {
    const api = createMockApi({ delayMs: 0 });
    expect(await activeIds(api)).toEqual(['testing']);
    const game = await api.getGameStatus();
    expect(game.overrideActive).toBe(true);
    expect(game.effectiveModsDir).toContain('$testingMods');
  });

  it('moves isActive and the game status, and returns increasing op ids', async () => {
    const api = createMockApi({ delayMs: 0 });
    const first = await api.setActiveProfile('offline');
    expect(first).toMatchObject({ fromProfileId: 'testing', toProfileId: 'offline' });
    expect(await activeIds(api)).toEqual(['offline']);
    const game = await api.getGameStatus();
    expect(game.overrideActive).toBe(true);
    expect(game.effectiveModsDir).toContain('$offlineTestingMods');

    const second = await api.setActiveProfile('testing');
    expect(Number(second.opId)).toBeGreaterThan(Number(first.opId));
  });

  it('turns the override off for the default profile', async () => {
    const api = createMockApi({ delayMs: 0 });
    await api.setActiveProfile('main');
    expect(await activeIds(api)).toEqual(['main']);
    const game = await api.getGameStatus();
    expect(game.overrideActive).toBe(false);
    expect(game.effectiveModsDir).toMatch(/\\mods$/);
  });

  it('does not share switching state between instances or change the dataset', async () => {
    await createMockApi({ delayMs: 0 }).setActiveProfile('offline');
    expect(await activeIds(createMockApi({ delayMs: 0 }))).toEqual(['testing']);
  });

  it('refuses an unknown profile', async () => {
    const api = createMockApi({ delayMs: 0 });
    expect(await failureOf(api.setActiveProfile('nope'))).toBe('profileNotFound');
  });

  it('undoes the latest switch to the exact previous state', async () => {
    const api = createMockApi({ delayMs: 0 });
    const done = await api.setActiveProfile('main');
    const undone = await api.undoSwitch(done.opId);
    expect(undone).toMatchObject({ fromProfileId: 'main', toProfileId: 'testing' });
    expect(await activeIds(api)).toEqual(['testing']);
    expect((await api.getGameStatus()).overrideActive).toBe(true);
  });

  it('undoes only the latest operation, once', async () => {
    const api = createMockApi({ delayMs: 0 });
    const first = await api.setActiveProfile('offline');
    const second = await api.setActiveProfile('main');
    expect(await failureOf(api.undoSwitch(first.opId))).toBe('nothingToUndo');
    expect(await failureOf(api.undoSwitch('999'))).toBe('nothingToUndo');
    expect(await activeIds(api)).toEqual(['main']);

    await api.undoSwitch(second.opId);
    expect(await activeIds(api)).toEqual(['offline']);
    expect(await failureOf(api.undoSwitch(second.opId))).toBe('nothingToUndo');
  });

  it('has nothing to undo before any switch', async () => {
    const api = createMockApi({ delayMs: 0 });
    expect(await failureOf(api.undoSwitch('1'))).toBe('nothingToUndo');
  });

  it('?mock=running: the status says so and switching is refused', async () => {
    const api = createMockApi({ delayMs: 0, gameRunning: true });
    expect((await api.getGameStatus()).gameRunning).toBe(true);
    expect(await failureOf(api.setActiveProfile('offline'))).toBe('gameRunning');
    expect(await activeIds(api)).toEqual(['testing']);
  });

  it('?mock=server: the status says so and switching is refused', async () => {
    const api = createMockApi({ delayMs: 0, serverRunning: true });
    const game = await api.getGameStatus();
    expect(game.serverRunning).toBe(true);
    expect(game.gameRunning).toBe(false);
    expect(await failureOf(api.setActiveProfile('offline'))).toBe('serverRunning');
  });

  it('?mock=writeFails: switching fails and changes nothing', async () => {
    const api = createMockApi({ delayMs: 0, writeFails: true });
    expect(await failureOf(api.setActiveProfile('offline'))).toBe('writeFailed');
    expect(await activeIds(api)).toEqual(['testing']);
    expect(await failureOf(api.undoSwitch('1'))).toBe('nothingToUndo');
  });

  it('?mock=brokenSettings: unreadable until the backup is restored', async () => {
    const api = createMockApi({ delayMs: 0, brokenSettings: true });
    const broken = await api.getGameStatus();
    expect(broken.settingsReadable).toBe(false);
    expect(broken.effectiveModsDir).toBeNull();
    expect(await activeIds(api)).toEqual([]);
    expect(await api.latestSettingsBackup()).toBe(MOCK_BACKUP_AT);
    expect(await failureOf(api.setActiveProfile('offline'))).toBe('settingsUnreadable');

    await api.restoreSettingsBackup();
    const fixed = await api.getGameStatus();
    expect(fixed.settingsReadable).toBe(true);
    expect(fixed.overrideActive).toBe(true);
    expect(await activeIds(api)).toEqual(['testing']);
    expect((await api.setActiveProfile('offline')).toProfileId).toBe('offline');
  });

  it('has no backup to offer or restore when the settings are fine', async () => {
    const api = createMockApi({ delayMs: 0 });
    expect(await api.latestSettingsBackup()).toBeNull();
    expect(await failureOf(api.restoreSettingsBackup())).toBe('noBackup');
  });
});
