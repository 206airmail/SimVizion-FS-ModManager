import { describe, expect, it } from 'vitest';
import type { Api, ModEntry, OpProgress, UpdatePlan } from '..';
import { ApiFailure } from '../../lib/errors';
import { compareVersions } from '../../lib/version';
import { createMockApi, type MockOptions } from '.';
import { parseSourceLink } from './updates';

const make = (options: MockOptions = {}) => createMockApi({ delayMs: 0, ...options });

const codeOf = async (promise: Promise<unknown>) => {
  const error = await promise.then(
    () => null,
    (e: unknown) => e,
  );
  expect(error).toBeInstanceOf(ApiFailure);
  return (error as ApiFailure).code;
};

const isAvailable = (mod: ModEntry) => mod.update.type === 'available';

/** An Api with updates on and the first check done. */
async function checked(options: MockOptions = {}): Promise<Api> {
  const api = make(options);
  await api.setUpdatesEnabled(true);
  await api.checkForUpdates(false);
  return api;
}

/** The first mod of `main` with an installable update, and the plan for it. */
async function installable(
  api: Api,
  source?: UpdatePlan['source'],
): Promise<{ mod: ModEntry; plan: UpdatePlan }> {
  for (const mod of (await api.listMods('main')).filter(isAvailable)) {
    const plan = await api.planUpdate(mod.modName).catch(() => null);
    if (plan && (source === undefined || plan.source === source)) return { mod, plan };
  }
  throw new Error('the mock has no such update');
}

/** A mod with an installable update that two folders hold, both behind: Main Farm and Offline Testing. */
async function twoFolders(api: Api): Promise<{ modName: string; plan: UpdatePlan }> {
  for (const mod of (await api.listMods('main')).filter(
    (candidate) => isAvailable(candidate) && candidate.alsoIn.length === 0,
  )) {
    if (!(await api.planUpdate(mod.modName).catch(() => null))) continue;
    const made = await api.planTransfer('main', [mod.id], 'offline', 'copy');
    await api.executeTransfer(made.planId, []);
    const plan = await api.planUpdate(mod.modName);
    expect(plan.folders.map((folder) => folder.profileId)).toEqual(['main', 'offline']);
    return { modName: mod.modName, plan };
  }
  throw new Error('the mock has no mod to update in two folders');
}

describe('mock update settings', () => {
  it('start switched off, with nothing matched and only manual links shown', async () => {
    const api = make();
    expect(await api.getUpdateSettings()).toEqual({
      enabled: false,
      lastCheck: null,
      retryAfter: null,
    });
    const mods = await api.listMods('main');
    expect(mods.every((mod) => mod.update.type === 'unknown')).toBe(true);
    expect(mods.every((mod) => mod.source.type === 'unknown' || mod.source.type === 'manual')).toBe(
      true,
    );
    expect(mods.every((mod) => mod.links.every((link) => link.kind === 'manual'))).toBe(true);
    expect(mods.some((mod) => mod.links.length > 0)).toBe(true);
    for (const profile of await api.listProfiles()) expect(profile.updateCount).toBe(0);
  });

  it('refuse checks and installs while off', async () => {
    const api = make();
    expect(await codeOf(api.checkForUpdates(false))).toBe('updatesDisabled');
    expect(await codeOf(api.installUpdate('FS25_Any', ['main']))).toBe('updatesDisabled');
  });

  it('switch on and off, and off hides what a check found without forgetting it', async () => {
    const api = await checked();
    expect((await api.getUpdateSettings()).enabled).toBe(true);
    expect((await api.getUpdateSettings()).lastCheck).not.toBeNull();
    expect((await api.listMods('main')).some(isAvailable)).toBe(true);
    expect((await api.setUpdatesEnabled(false)).enabled).toBe(false);
    expect((await api.listMods('main')).some(isAvailable)).toBe(false);
    await api.setUpdatesEnabled(true);
    expect((await api.listMods('main')).some(isAvailable)).toBe(true);
    expect((await api.checkForUpdates(false)).fromCache).toBe(true);
  });

  it('?mock=updatesOn starts on with a cached check from earlier', async () => {
    const api = make({ updatesOn: true });
    const settings = await api.getUpdateSettings();
    expect(settings.enabled).toBe(true);
    const age = Date.now() - Date.parse(settings.lastCheck ?? '');
    expect(age).toBeGreaterThan(30 * 60_000);
    expect(age).toBeLessThan(60 * 60_000);
    expect((await api.listMods('main')).some(isAvailable)).toBe(true);
    expect((await api.checkForUpdates(false)).fromCache).toBe(true);
  });
});

describe('mock checkForUpdates', () => {
  it('matches names, counts updates and fills source, update state and links', async () => {
    const api = make();
    await api.setUpdatesEnabled(true);
    const result = await api.checkForUpdates(false);
    expect(result.fromCache).toBe(false);
    const mods = await api.listMods('main');
    expect(result.checked).toBeGreaterThanOrEqual(mods.length);
    expect(result.matchedByName + result.matchedByHash).toBeLessThanOrEqual(result.checked);
    expect(result.matchedByHash).toBeGreaterThan(0);
    expect(result.updates).toBeGreaterThan(20);

    const tracked = mods.filter((mod) => mod.update.type !== 'unknown');
    expect(tracked.length / mods.length).toBeGreaterThan(0.5);
    for (const mod of tracked) {
      expect(['modHub', 'gitHub']).toContain(mod.source.type);
    }
    const updates = mods.filter(isAvailable);
    expect(updates.length / mods.length).toBeGreaterThan(0.05);
    expect(updates.length / mods.length).toBeLessThan(0.2);
    for (const mod of updates) {
      const update = mod.update;
      if (update.type !== 'available') continue;
      expect(compareVersions(update.version, mod.version)).toBeGreaterThan(0);
    }
    expect(updates.some((mod) => mod.update.type === 'available' && mod.update.installable)).toBe(
      true,
    );
    expect(updates.some((mod) => mod.update.type === 'available' && !mod.update.installable)).toBe(
      true,
    );
  });

  it('shows most mods on ModHub, some on GitHub and a few on both, in link order', async () => {
    const api = await checked();
    const mods = await api.listMods('main');
    const labels = (mod: ModEntry) => mod.links.map((link) => link.label);
    expect(mods.filter((mod) => mod.source.type === 'modHub').length).toBeGreaterThan(
      mods.filter((mod) => mod.source.type === 'gitHub').length,
    );
    const both = mods.filter(
      (mod) => labels(mod).includes('ModHub') && labels(mod).includes('GitHub'),
    );
    expect(both.length).toBeGreaterThan(2);
    for (const mod of both) {
      const kinds = mod.links.map((link) => link.kind);
      expect(kinds.indexOf('modHub')).toBeLessThan(kinds.indexOf('gitHub'));
    }
    const manual = mods.filter((mod) => mod.links.some((link) => link.kind === 'manual'));
    const hosts = new Set(
      manual.flatMap((mod) => mod.links.filter((l) => l.kind === 'manual')).map((l) => l.label),
    );
    expect(hosts.has('www.kingmods.net')).toBe(true);
    expect([...hosts].some((host) => host.endsWith('.itch.io'))).toBe(true);
    for (const mod of manual.filter((m) => m.links.length > 1)) {
      expect(mod.links.at(-1)?.kind).toBe('manual');
    }
  });

  it('updates the profile counts to match the mods', async () => {
    const api = await checked();
    for (const profile of await api.listProfiles()) {
      const mods = await api.listMods(profile.id);
      expect(profile.updateCount).toBe(mods.filter(isAvailable).length);
      expect(profile.updateCount).toBeGreaterThan(0);
    }
  });

  it('answers from the cache within 3 hours, and within 60 seconds when forced', async () => {
    const api = await checked();
    const again = await api.checkForUpdates(false);
    expect(again.fromCache).toBe(true);
    expect(await api.checkForUpdates(true)).toMatchObject({ fromCache: true });
    expect(again.checkedAt).toBe((await api.getUpdateSettings()).lastCheck);
  });

  it('forces a new check when the cached one is older than 60 seconds', async () => {
    const api = make({ updatesOn: true });
    expect((await api.checkForUpdates(false)).fromCache).toBe(true);
    const forced = await api.checkForUpdates(true);
    expect(forced.fromCache).toBe(false);
    expect(Date.now() - Date.parse(forced.checkedAt)).toBeLessThan(5000);
  });

  it('?mock=offline: fails with offline and shows nothing', async () => {
    const api = make({ offline: true });
    await api.setUpdatesEnabled(true);
    expect(await codeOf(api.checkForUpdates(false))).toBe('offline');
    expect((await api.listMods('main')).every((mod) => mod.update.type === 'unknown')).toBe(true);
    expect((await api.getUpdateSettings()).lastCheck).toBeNull();
  });

  it('?mock=rateLimited: fails with the retry time, stores it and does not ask again before it', async () => {
    const api = make({ rateLimited: true });
    await api.setUpdatesEnabled(true);
    const error = await api.checkForUpdates(false).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiFailure);
    const failure = error as ApiFailure;
    expect(failure.code).toBe('rateLimited');
    const retryAt = Date.parse(failure.detail ?? '');
    expect(retryAt).toBeGreaterThan(Date.now() + 4 * 60_000);
    expect((await api.getUpdateSettings()).retryAfter).toBe(failure.detail);
    expect(await codeOf(api.checkForUpdates(true))).toBe('rateLimited');
    expect((await api.listMods('main')).every((mod) => mod.update.type === 'unknown')).toBe(true);
  });
});

describe('mock planUpdate', () => {
  it('names the version, the source and every folder holding the mod, in profile order', async () => {
    const api = await checked();
    const { modName, plan } = await twoFolders(api);
    expect(plan.modName).toBe(modName);
    expect(['gitHub', 'modHub']).toContain(plan.source);
    expect(plan.folders.map((folder) => folder.profileId)).toEqual(['main', 'offline']);
    const [first, second] = plan.folders;
    expect(first?.version).not.toBeUndefined();
    expect(compareVersions(second?.version ?? '', plan.toVersion)).toBeLessThan(0);
  });

  it('writes nothing', async () => {
    const api = await checked();
    const before = await api.listMods('main');
    const { plan } = await installable(api);
    expect(plan.folders.length).toBeGreaterThan(0);
    expect(await api.listMods('main')).toEqual(before);
  });

  it('is refused when the update is not installable or there is none', async () => {
    const api = await checked();
    const mods = await api.listMods('main');
    const blocked = mods.find((mod) => mod.update.type === 'available' && !mod.update.installable);
    expect(blocked).toBeDefined();
    expect(await codeOf(api.planUpdate(blocked?.modName ?? ''))).toBe('notInstallable');
    const current = mods.find((mod) => mod.update.type === 'upToDate');
    expect(await codeOf(api.planUpdate(current?.modName ?? ''))).toBe('notInstallable');
    expect(await codeOf(api.planUpdate('FS25_Nobody'))).toBe('notInstallable');
  });
});

describe('mock installUpdate', () => {
  it('installs into the folders and names each one by its profile id', async () => {
    const api = await checked();
    const { modName, plan } = await twoFolders(api);
    const ids = plan.folders.map((folder) => folder.profileId);
    const before = (await api.listProfiles()).map((p) => p.updateCount);
    const result = await api.installUpdate(modName, ids);
    expect(result).toEqual({ done: ids, skipped: [], failed: [] });
    for (const id of ids) {
      const mod = (await api.listMods(id)).find((m) => m.modName === modName);
      expect(mod?.version).toBe(plan.toVersion);
      expect(mod?.update).toEqual({ type: 'upToDate' });
    }
    const after = (await api.listProfiles()).map((p) => p.updateCount);
    expect(after.reduce((a, b) => a + b, 0)).toBeLessThan(before.reduce((a, b) => a + b, 0));
  });

  it('updates only the folders asked for, and the others stay behind', async () => {
    const api = await checked();
    const { modName, plan } = await twoFolders(api);
    const behind = plan.folders.filter((f) => compareVersions(f.version, plan.toVersion) < 0);
    expect(behind.length).toBeGreaterThanOrEqual(1);
    const only = behind[0]?.profileId ?? '';
    await api.installUpdate(modName, [only]);
    for (const folder of behind.slice(1)) {
      const mod = (await api.listMods(folder.profileId)).find((m) => m.modName === modName);
      expect(mod?.update.type).toBe('available');
    }
  });

  it('skips a folder that is already current and fails one that does not hold the mod', async () => {
    const api = await checked();
    const { mod, plan } = await installable(api);
    await api.installUpdate(mod.modName, ['main']);
    const result = await api.installUpdate(mod.modName, ['main', 'nowhere']);
    expect(result.skipped).toEqual(['main']);
    expect(result.failed).toEqual([{ modName: 'nowhere', code: 'targetChanged' }]);
    expect(plan.folders[0]?.profileId).toBe('main');
  });

  it('streams progress by bytes and items, ending complete', async () => {
    const api = await checked();
    const { modName, plan } = await twoFolders(api);
    const seen: OpProgress[] = [];
    await api.installUpdate(
      modName,
      plan.folders.map((f) => f.profileId),
      (p) => seen.push(p),
    );
    expect(seen[0]).toMatchObject({ doneBytes: 0, doneItems: 0, totalItems: 2, current: null });
    for (let i = 1; i < seen.length; i++) {
      expect(seen[i]?.doneBytes ?? 0).toBeGreaterThanOrEqual(seen[i - 1]?.doneBytes ?? 0);
    }
    const last = seen.at(-1);
    expect(last).toMatchObject({ doneItems: 2, totalItems: 2, current: null });
    expect(last?.doneBytes).toBe(last?.totalBytes);
    expect(seen.some((p) => p.current !== null)).toBe(true);
  });

  it('is refused up front while the game or the server runs, or another operation is running', async () => {
    for (const [options, code] of [
      [{ gameRunning: true }, 'gameRunning'],
      [{ serverRunning: true }, 'serverRunning'],
    ] as const) {
      const api = await checked(options);
      const { mod } = await installable(api);
      expect(await codeOf(api.installUpdate(mod.modName, ['main']))).toBe(code);
    }
    const api = await checked();
    const { mod } = await installable(api);
    const first = api.installUpdate(mod.modName, ['main']);
    expect(await codeOf(api.installUpdate(mod.modName, ['main']))).toBe('busy');
    await first;
  });

  it('cancel stops after the current folder and leaves the rest alone', async () => {
    const api = await checked();
    const { modName, plan } = await twoFolders(api);
    const ids = plan.folders.map((f) => f.profileId);
    const result = await api.installUpdate(modName, ids, (p) => {
      if (p.doneItems === 1 && p.current !== null) void api.cancelOperation();
    });
    expect(result.done).toEqual([ids[0]]);
    expect(result.failed).toEqual([{ modName: ids[1], code: 'cancelled' }]);
    expect((await api.listMods(ids[1] ?? '')).find((m) => m.modName === modName)?.update.type).toBe(
      'available',
    );
  });

  it('?mock=hashMismatch: downloads, then fails every folder with verifyFailed and changes nothing', async () => {
    const api = await checked({ hashMismatch: true });
    const { modName, plan } = await twoFolders(api);
    const ids = plan.folders.map((f) => f.profileId);
    const before = await api.listMods('offline');
    const result = await api.installUpdate(modName, ids);
    expect(result.done).toEqual([]);
    expect(result.failed).toEqual(ids.map((id) => ({ modName: id, code: 'verifyFailed' })));
    expect(await api.listMods('offline')).toEqual(before);
  });

  it('?mock=noModHubLink: a ModHub install fails with notInstallable, a GitHub one succeeds', async () => {
    const api = await checked({ noModHubLink: true });
    const hub = await installable(api, 'modHub');
    expect((await api.installUpdate(hub.mod.modName, ['main'])).failed).toEqual([
      { modName: 'main', code: 'notInstallable' },
    ]);
    const git = await installable(api, 'gitHub');
    expect((await api.installUpdate(git.mod.modName, ['main'])).done).toEqual(['main']);
  });

  it('?mock=offline: every folder fails with offline', async () => {
    const api = make({ updatesOn: true, offline: true });
    const { modName, plan } = await twoFolders(api);
    const ids = plan.folders.map((f) => f.profileId);
    const result = await api.installUpdate(modName, ids);
    expect(result.failed).toEqual(ids.map((id) => ({ modName: id, code: 'offline' })));
  });

  it('?mock=recycleFails: some folders keep their old version', async () => {
    const api = await checked({ recycleFails: true });
    const { modName, plan } = await twoFolders(api);
    const ids = plan.folders.map((f) => f.profileId);
    const result = await api.installUpdate(modName, ids);
    expect(result.done).toEqual([ids[0]]);
    expect(result.failed).toEqual([{ modName: ids[1], code: 'recycleUnavailable' }]);
  });

  it('keeps each instance apart', async () => {
    const api = await checked();
    const { mod } = await installable(api);
    await api.installUpdate(mod.modName, ['main']);
    const other = await checked();
    expect((await other.listMods('main')).find((m) => m.id === mod.id)?.update.type).toBe(
      'available',
    );
  });
});

describe('mock source links', () => {
  it('sets, replaces and removes a link, returning its host', async () => {
    const api = make();
    const mod = (await api.listMods('main')).find((m) => m.links.length === 0);
    expect(mod).toBeDefined();
    const name = mod?.modName ?? '';
    expect(await api.setSourceLink(name, ' https://www.example.org/mods/1 ')).toBe(
      'www.example.org',
    );
    const linked = (await api.listMods('main')).find((m) => m.modName === name);
    expect(linked?.links).toEqual([{ kind: 'manual', label: 'www.example.org' }]);
    expect(linked?.source).toEqual({ type: 'manual', host: 'www.example.org' });
    expect(await api.setSourceLink(name, 'http://other.example.net/x')).toBe('other.example.net');
    expect(await api.setSourceLink(name, null)).toBeNull();
    expect((await api.listMods('main')).find((m) => m.modName === name)?.links).toEqual([]);
  });

  it('shares a link between every folder that holds the mod, whatever the case of the name', async () => {
    const api = make();
    const mod = (await api.listMods('testing')).find(
      (m) => m.alsoIn.includes('main') && m.links.length === 0,
    );
    const name = mod?.modName ?? '';
    await api.setSourceLink(name.toUpperCase(), 'https://example.org/p');
    for (const id of ['main', 'testing']) {
      const found = (await api.listMods(id)).find((m) => m.modName === name);
      expect(found?.links).toHaveLength(1);
    }
  });

  it('refuses addresses the core refuses', async () => {
    const api = make();
    for (const bad of [
      '',
      '   ',
      'www.example.org',
      'ftp://example.org/x',
      'javascript:alert(1)',
      'https://user:secret@example.org/x',
      `https://example.org/${'a'.repeat(2000)}`,
      'https://',
    ]) {
      expect(await codeOf(api.setSourceLink('FS25_X', bad)), bad).toBe('invalidLink');
    }
  });

  it('shows a github.com link as a GitHub source and any other host as manual', () => {
    expect(parseSourceLink('https://github.com/owner/repo.git')).toMatchObject({
      host: 'github.com',
      github: { owner: 'owner', repo: 'repo' },
    });
    expect(parseSourceLink('https://github.com/owner')).toMatchObject({ github: null });
    expect(parseSourceLink('https://itch.io/x').github).toBeNull();
  });

  it('opens a page by mod name and kind, and refuses a kind the mod has no page for', async () => {
    const opened: [string, string][] = [];
    const api = await checked({ onOpenLink: (url, kind) => opened.push([kind, url]) });
    const mod = (await api.listMods('main')).find((m) =>
      m.links.some((link) => link.kind === 'modHub'),
    );
    await api.openModLink(mod?.modName ?? '', 'modHub');
    expect(opened).toHaveLength(1);
    expect(opened[0]?.[1]).toMatch(/^https:\/\/www\.farming-simulator\.com\/mod\.php\?mod_id=\d+/);
    const bare = (await api.listMods('main')).find((m) => m.links.length === 0);
    expect(await codeOf(api.openModLink(bare?.modName ?? '', 'manual'))).toBe('folderMissing');
    expect(await codeOf(api.openModLink(bare?.modName ?? '', 'gitHub'))).toBe('folderMissing');
  });

  it('does not offer catalog pages while updates are off', async () => {
    const api = make();
    const hub = await checked();
    const mod = (await hub.listMods('main')).find((m) => m.links.some((l) => l.kind === 'modHub'));
    expect(await codeOf(api.openModLink(mod?.modName ?? '', 'modHub'))).toBe('folderMissing');
  });
});

describe('mock update data', () => {
  it('is deterministic', async () => {
    const [a, b] = [await checked(), await checked()];
    expect(await a.listMods('main')).toEqual(await b.listMods('main'));
    expect(await a.listProfiles()).toEqual(await b.listProfiles());
  });
});
