import { describe, expect, it } from 'vitest';
import type { Api, HashProgress, SaveModState } from '..';
import { ApiFailure } from '../../lib/errors';
import { createMockApi, type MockOptions } from '.';

const make = (options: MockOptions = {}) => createMockApi({ delayMs: 0, ...options });

const codeOf = async (promise: Promise<unknown>) => {
  const error = await promise.then(
    () => null,
    (e: unknown) => e,
  );
  expect(error).toBeInstanceOf(ApiFailure);
  return (error as ApiFailure).code;
};

const detailOf = async (promise: Promise<unknown>) => {
  const error = await promise.then(
    () => null,
    (e: unknown) => e,
  );
  return (error as ApiFailure).detail;
};

describe('mock compareProfiles', () => {
  it('has one row per mod name, sorted by lower-cased name', async () => {
    const comparison = await make().compareProfiles('main', 'testing');
    const keys = comparison.rows.map((row) => row.modName.toLowerCase());
    expect(keys).toEqual([...keys].sort());
    expect(new Set(keys).size).toBe(keys.length);
    expect(comparison).toMatchObject({ leftProfileId: 'main', rightProfileId: 'testing' });
  });

  it('gives every state, and a state matching the two sides', async () => {
    const comparison = await make().compareProfiles('main', 'testing');
    const states = new Set(comparison.rows.map((row) => row.state));
    expect([...states].sort()).toEqual(
      ['contentDiffers', 'identical', 'onlyLeft', 'onlyRight', 'versionDiffers'].sort(),
    );
    for (const { left, right, state } of comparison.rows) {
      if (state === 'onlyLeft') expect([left === null, right === null]).toEqual([false, true]);
      if (state === 'onlyRight') expect([left === null, right === null]).toEqual([true, false]);
      if (left && right) {
        const sameVersion = left.summary.version === right.summary.version;
        expect(state === 'versionDiffers').toBe(!sameVersion);
        if (sameVersion) {
          expect(state === 'contentDiffers').toBe(
            left.summary.sizeBytes !== right.summary.sizeBytes,
          );
        }
      }
    }
  });

  it('uses the title of the left side, else the right', async () => {
    const comparison = await make().compareProfiles('main', 'testing');
    const onlyRight = comparison.rows.find((row) => row.state === 'onlyRight');
    const mods = await make().listMods('testing');
    expect(onlyRight?.title).toBe(mods.find((mod) => mod.modName === onlyRight?.modName)?.title);
  });

  it('is mirrored when the sides are swapped', async () => {
    const api = make();
    const forward = await api.compareProfiles('main', 'offline');
    const backward = await api.compareProfiles('offline', 'main');
    const flip = { onlyLeft: 'onlyRight', onlyRight: 'onlyLeft' } as const;
    expect(backward.rows.map((row) => row.modName)).toEqual(forward.rows.map((r) => r.modName));
    expect(backward.rows.map((row) => row.state)).toEqual(
      forward.rows.map((row) =>
        row.state in flip ? flip[row.state as keyof typeof flip] : row.state,
      ),
    );
  });

  it('sees a copy at once: a copied mod turns identical', async () => {
    const api = make();
    const before = await api.compareProfiles('main', 'offline');
    const only = before.rows.find((row) => row.state === 'onlyLeft');
    expect(only?.left).not.toBeNull();
    const plan = await api.planTransfer('main', [only?.left?.modId ?? ''], 'offline', 'copy');
    await api.executeTransfer(plan.planId, []);
    const after = await api.compareProfiles('main', 'offline');
    expect(after.rows.find((row) => row.modName === only?.modName)?.state).toBe('identical');
  });

  it('rejects an unknown profile', async () => {
    expect(await codeOf(make().compareProfiles('main', 'nope'))).toBe('profileNotFound');
  });
});

describe('mock savegames', () => {
  it('lists three saves, with their facts', async () => {
    const saves = await make().listSavegames();
    expect(saves.map((save) => save.id)).toEqual(['savegame1', 'savegame4', 'savegame7']);
    const first = saves[0];
    expect(first?.slot).toBe(1);
    expect(first?.mapMod).not.toBeNull();
    expect(Number.isInteger(first?.money)).toBe(true);
    expect(first?.modCount).toBeGreaterThan(100);
  });

  it('has no saves, or an unreadable fourth, behind the switches', async () => {
    expect(await make({ noSaves: true }).listSavegames()).toEqual([]);
    expect(await codeOf(make({ noSaves: true }).savegameReport('savegame1', 'main'))).toBe(
      'savegameMissing',
    );
    const api = make({ brokenSave: true });
    const saves = await api.listSavegames();
    expect(saves).toHaveLength(4);
    const broken = saves.at(-1);
    expect(await codeOf(api.savegameReport(broken?.id ?? '', 'main'))).toBe('savegameUnreadable');
    expect(await codeOf(api.checkSavegameFiles(broken?.id ?? '', 'main'))).toBe(
      'savegameUnreadable',
    );
  });

  it('rejects an unknown save or profile', async () => {
    const api = make();
    expect(await codeOf(api.savegameReport('savegame2', 'main'))).toBe('savegameMissing');
    expect(await codeOf(api.savegameReport('savegame1', 'nope'))).toBe('profileNotFound');
  });

  it('reports at once with the map first and every state present', async () => {
    const api = make();
    const report = await api.savegameReport('savegame1', 'main');
    const [save] = await api.listSavegames();
    expect(report.rows).toHaveLength(save?.modCount ?? -1);
    expect(report.rows[0]).toMatchObject({ required: true, modName: save?.mapMod });
    expect(report.rows.filter((row) => row.required)).toHaveLength(1);
    const rest = report.rows.slice(1).map((row) => row.modName.toLowerCase());
    expect(rest).toEqual([...rest].sort());
    const states = new Set(report.rows.map((row) => row.state));
    for (const state of [
      'present',
      'versionDiffers',
      'hashPending',
      'missing',
      'dlcInstalled',
      'dlcMissing',
    ] satisfies SaveModState[]) {
      expect(states.has(state), state).toBe(true);
    }
    // The first report has no file result yet.
    expect(states.has('fileDiffers')).toBe(false);
  });

  it('reads missing mods as: found elsewhere, or nowhere', async () => {
    const api = make();
    const report = await api.savegameReport('savegame1', 'main');
    const missing = report.rows.filter((row) => row.state === 'missing');
    expect(missing.some((row) => row.foundIn.length > 0)).toBe(true);
    expect(missing.some((row) => row.foundIn.length === 0)).toBe(true);
    for (const row of missing) {
      expect(row.installed).toBeNull();
      expect(row.foundIn).not.toContain('main');
    }
    const testing = await api.savegameReport('savegame1', 'testing');
    expect(testing.rows.find((row) => row.required)?.state).toBe('missing');
    expect(testing.rows.find((row) => row.required)?.foundIn).toEqual(['main']);
  });

  it('lists the profile mods the save does not use as extras', async () => {
    const api = make();
    const report = await api.savegameReport('savegame4', 'testing');
    const mods = await api.listMods('testing');
    const used = new Set(report.rows.map((row) => row.modName));
    expect(report.extras.sort()).toEqual(
      mods
        .filter((mod) => !used.has(mod.modName))
        .map((mod) => mod.id)
        .sort(),
    );
    expect(report.extras.length).toBeGreaterThan(0);
  });

  it('hashes the pending rows with monotonic progress and ends with file results', async () => {
    const api = make();
    const first = await api.savegameReport('savegame1', 'main');
    const pending = first.rows.filter((row) => row.state === 'hashPending').length;
    expect(pending).toBeGreaterThan(0);
    const seen: HashProgress[] = [];
    const final = await api.checkSavegameFiles('savegame1', 'main', (p) => seen.push(p));
    expect(final.rows.filter((row) => row.state === 'hashPending')).toEqual([]);
    expect(final.rows.filter((row) => row.state === 'fileDiffers').length).toBeGreaterThan(0);
    expect(seen.at(-1)).toMatchObject({ doneFiles: pending, totalFiles: pending });
    expect(seen[0]?.doneFiles).toBe(0);
    for (let i = 1; i < seen.length; i++) {
      expect(seen[i]?.doneFiles ?? 0).toBeGreaterThanOrEqual(seen[i - 1]?.doneFiles ?? 0);
      expect(seen[i]?.doneBytes ?? 0).toBeGreaterThanOrEqual(seen[i - 1]?.doneBytes ?? 0);
    }
    // Cached: the next report is final at once, and a check has nothing to do.
    const again = await api.savegameReport('savegame1', 'main');
    expect(again.rows.filter((row) => row.state === 'hashPending')).toEqual([]);
    expect(again.rows.map((row) => row.state)).toEqual(final.rows.map((row) => row.state));
  });

  it('cancels a running check, keeps the first report, and refuses a second at once', async () => {
    const api = createMockApi({ delayMs: 1, slowHash: true });
    const running = api.checkSavegameFiles('savegame1', 'main');
    const second = api.checkSavegameFiles('savegame1', 'main');
    expect(await codeOf(second)).toBe('busy');
    await new Promise((resolve) => setTimeout(resolve, 120));
    await api.cancelHashing();
    expect(await codeOf(running)).toBe('cancelled');
    // Whatever was hashed before the cancel stays; the rest is still pending. Another run works.
    const partial = await api.savegameReport('savegame1', 'main');
    expect(partial.rows.some((row) => row.state === 'hashPending')).toBe(true);
    // The busy flag was released: another run starts, and cancels, the same way.
    const next = api.checkSavegameFiles('savegame1', 'main');
    await new Promise((resolve) => setTimeout(resolve, 60));
    await api.cancelHashing();
    expect(await codeOf(next)).toBe('cancelled');
  });

  it('calls a stale hash and a re-zipped copy a different file', async () => {
    const api: Api = make();
    const report = await api.checkSavegameFiles('savegame1', 'testing');
    expect(report.rows.every((row) => row.state !== 'hashPending')).toBe(true);
    // Testing holds re-zipped copies of Main Farm mods at the same version.
    const comparison = await api.compareProfiles('main', 'testing');
    const rezipped = comparison.rows.filter((row) => row.state === 'contentDiffers');
    expect(rezipped.length).toBeGreaterThan(0);
    const inSave = rezipped.flatMap((row) => report.rows.filter((r) => r.modName === row.modName));
    expect(inSave.length).toBeGreaterThan(0);
    for (const saved of inSave) expect(saved.state).toBe('fileDiffers');
  });

  it('leaves the required map of a save missing in every folder', async () => {
    const api = make();
    const report = await api.savegameReport('savegame7', 'testing');
    expect(report.rows[0]).toMatchObject({ required: true, state: 'missing', foundIn: [] });
  });
});

describe('mock tags and notes', () => {
  it('stores trimmed, de-duplicated, sorted tags and returns them', async () => {
    const api = make();
    const stored = await api.setTags('FS25_Example', ['  zebra ', 'Alpha', 'alpha', '', 'beta']);
    expect(stored).toEqual(['Alpha', 'beta', 'zebra']);
  });

  it('is case-insensitive by mod name and shared by every profile', async () => {
    const api = make();
    const inMain = (await api.listMods('main')).find((mod) => mod.alsoIn.includes('testing'));
    expect(inMain).toBeDefined();
    const name = inMain?.modName ?? '';
    await api.setTags(name.toUpperCase(), ['Shared']);
    await api.setNote(name.toLowerCase(), 'A note');
    for (const profile of ['main', 'testing']) {
      const mod = (await api.listMods(profile)).find((m) => m.modName === name);
      expect(mod?.tags).toEqual(['Shared']);
      expect(mod?.note).toBe('A note');
    }
  });

  it('clears a note with an empty string and removes tags with an empty list', async () => {
    const api = make();
    const mod = (await api.listMods('main'))[0];
    const name = mod?.modName ?? '';
    await api.setTags(name, ['One']);
    await api.setNote(name, 'Hello');
    await api.setTags(name, []);
    await api.setNote(name, '');
    const after = (await api.listMods('main')).find((m) => m.modName === name);
    expect(after).toMatchObject({ tags: [], note: null });
  });

  it('refuses an over-long tag, too many tags and an over-long note', async () => {
    const api = make();
    expect(await codeOf(api.setTags('FS25_X', ['x'.repeat(41)]))).toBe('internal');
    expect(await detailOf(api.setTags('FS25_X', ['x'.repeat(41)]))).toBe('invalidTag');
    await expect(api.setTags('FS25_X', ['x'.repeat(40)])).resolves.toHaveLength(1);
    const many = Array.from({ length: 21 }, (_, i) => `tag${i}`);
    expect(await detailOf(api.setTags('FS25_X', many))).toBe('invalidTag');
    await expect(api.setTags('FS25_X', many.slice(0, 20))).resolves.toHaveLength(20);
    expect(await detailOf(api.setNote('FS25_X', 'n'.repeat(4001)))).toBe('invalidNote');
    await expect(api.setNote('FS25_X', 'n'.repeat(4000))).resolves.toBeUndefined();
  });

  it('lists every distinct tag, sorted, and drops a tag nothing uses', async () => {
    const api = make();
    await api.setTags('FS25_A', ['Zulu', 'mike']);
    await api.setTags('FS25_B', ['ZULU', 'Alpha']);
    const tags = await api.listTags();
    expect(tags).toEqual(
      [...tags].sort((a, b) => a.localeCompare(b, undefined, { sensitivity: 'base' })),
    );
    expect(tags.filter((tag) => tag.toLowerCase() === 'zulu')).toHaveLength(1);
    await api.setTags('FS25_A', []);
    await api.setTags('FS25_B', []);
    expect(await api.listTags()).not.toContain('mike');
  });

  it('keeps each instance on its own', async () => {
    const one = make();
    const two = make();
    await one.setTags('FS25_Only', ['Mine']);
    expect(await two.listTags()).not.toContain('Mine');
  });
});
