import { describe, expect, it, vi } from 'vitest';
import type { OpProgress } from '..';
import { ApiFailure } from '../../lib/errors';
import { createMockApi } from '.';

const make = (options: Parameters<typeof createMockApi>[0] = {}) =>
  createMockApi({ delayMs: 0, ...options });

const failureCode = async (promise: Promise<unknown>) => {
  try {
    await promise;
  } catch (error) {
    return error instanceof ApiFailure ? error.code : 'not an ApiFailure';
  }
  return null;
};

describe('mock health report', () => {
  it('has one row per mod with issues, so its total equals the rail total', async () => {
    const api = make({ stress: true });
    const [report, profiles] = await Promise.all([api.healthReport(), api.listProfiles()]);
    const railTotal = profiles.reduce((sum, profile) => sum + profile.issueCount, 0);
    expect(railTotal).toBeGreaterThan(0);
    expect(report.rows).toHaveLength(railTotal);
    for (const profile of profiles) {
      expect(report.rows.filter((row) => row.profileId === profile.id)).toHaveLength(
        profile.issueCount,
      );
    }
  });

  it('counts the mods that have each kind, and the rows name real mods', async () => {
    const api = make();
    const report = await api.healthReport();
    for (const { kind, count } of report.counts) {
      expect(report.rows.filter((row) => row.issues.some((i) => i.kind === kind))).toHaveLength(
        count,
      );
    }
    const row = report.rows[0]!;
    const mods = await api.listMods(row.profileId);
    expect(mods.find((mod) => mod.id === row.modId)?.title).toBe(row.title);
  });

  it('follows the folders: a removed folder takes its rows with it', async () => {
    const api = make();
    const before = await api.healthReport();
    await api.removeProfile('offline');
    const after = await api.healthReport();
    expect(after.rows.length).toBeLessThan(before.rows.length);
    expect(after.rows.some((row) => row.profileId === 'offline')).toBe(false);
  });
});

describe('mock launchGame', () => {
  it('resolves, and the game shows as running a few seconds later', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    try {
      const api = make();
      expect((await api.getGameStatus()).launchable).toBe(true);
      await api.launchGame();
      expect((await api.getGameStatus()).gameRunning).toBe(false);
      vi.setSystemTime(Date.now() + 4000);
      expect((await api.getGameStatus()).gameRunning).toBe(true);
      expect(await failureCode(api.launchGame())).toBe('gameRunning');
    } finally {
      vi.useRealTimers();
    }
  });

  it('refuses while the game runs', async () => {
    expect(await failureCode(make({ gameRunning: true }).launchGame())).toBe('gameRunning');
  });

  it('refuses without an installation', async () => {
    expect(await failureCode(make({ noGame: true }).launchGame())).toBe('notLaunchable');
  });
});

describe('mock game folder', () => {
  it('starts with no folder and no profiles in noGame, then reveals the data once chosen', async () => {
    const api = make({ noGame: true });
    const status = await api.getGameStatus();
    expect(status.userDirSource).toBe('none');
    expect(status.userDir).toBeNull();
    expect(status.store).toBeNull();
    expect(status.launchable).toBe(false);
    expect(await api.listProfiles()).toEqual([]);
    expect(await api.listSavegames()).toEqual([]);

    const chosen = await api.chooseGameFolder();
    expect(chosen?.userDirSource).toBe('chosen');
    expect(chosen?.userDir).not.toBeNull();
    expect((await api.listProfiles()).length).toBeGreaterThan(0);
    expect((await api.listSavegames()).length).toBeGreaterThan(0);

    const cleared = await api.clearGameFolder();
    expect(cleared.userDirSource).toBe('none');
  });

  it('goes back to the detected folder when a chosen one is cleared', async () => {
    const api = make();
    expect((await api.getGameStatus()).userDirSource).toBe('detected');
    expect((await api.chooseGameFolder())?.userDirSource).toBe('chosen');
    expect((await api.clearGameFolder()).userDirSource).toBe('detected');
  });

  it('lists no profiles in noProfiles until a folder is added', async () => {
    const api = make({ noProfiles: true });
    expect((await api.getGameStatus()).userDirSource).toBe('detected');
    expect(await api.listProfiles()).toEqual([]);
    expect(await api.addProfile()).not.toBeNull();
    expect((await api.listProfiles()).length).toBeGreaterThan(0);
  });

  it('reports the Microsoft Store version with a user folder in its package folder', async () => {
    const status = await make({ msStore: true }).getGameStatus();
    expect(status.store).toBe('microsoftStore');
    expect(status.userDir).toContain('Packages');
  });
});

describe('mock app updates', () => {
  it('starts off, refuses a check while off, and finds nothing without appUpdate', async () => {
    const api = make();
    expect((await api.getAppUpdateSettings()).enabled).toBe(false);
    expect(await failureCode(api.checkAppUpdate(false))).toBe('updatesDisabled');
    await api.setAppUpdatesEnabled(true);
    expect(await api.checkAppUpdate(true)).toBeNull();
    expect((await api.getAppUpdateSettings()).lastCheck).not.toBeNull();
  });

  it('starts on with appUpdate and finds a version with multi-paragraph notes', async () => {
    const api = make({ appUpdate: true });
    expect((await api.getAppUpdateSettings()).enabled).toBe(true);
    const update = await api.checkAppUpdate(false);
    expect(update?.version).toBe('0.9.0-beta.2');
    expect(update?.notes?.split('\n\n').length).toBeGreaterThan(1);
  });

  it('streams progress to the full size, then resolves', async () => {
    const seen: OpProgress[] = [];
    await make({ appUpdate: true }).installAppUpdate((progress) => seen.push(progress));
    const last = seen.at(-1)!;
    expect(last.doneBytes).toBe(last.totalBytes);
    expect(
      seen.some((progress) => progress.doneBytes > 0 && progress.doneBytes < last.totalBytes),
    ).toBe(true);
  });

  it('fails part-way with appUpdateFails', async () => {
    const seen: OpProgress[] = [];
    const code = await failureCode(
      make({ appUpdateFails: true }).installAppUpdate((progress) => seen.push(progress)),
    );
    expect(code).toBe('appUpdateFailed');
    const last = seen.at(-1)!;
    expect(last.doneBytes).toBeGreaterThan(0);
    expect(last.doneBytes).toBeLessThan(last.totalBytes);
  });
});
