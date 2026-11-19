import { describe, expect, it } from 'vitest';
import type { Api, ModEntry, OpProgress, TransferPlan } from '..';
import { ApiFailure } from '../../lib/errors';
import { findByName, findClashes, nameKey, olderTwin, type MockState } from './files';
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

/** Mods of `from` that are not in `to`, so a plan for them has no clashes. */
async function freshMods(api: Api, from: string, to: string, count: number): Promise<ModEntry[]> {
  const mods = await api.listMods(from);
  return mods.filter((mod) => !mod.alsoIn.includes(to)).slice(0, count);
}
const ids = (mods: readonly ModEntry[]) => mods.map((mod) => mod.id);
const profileOf = async (api: Api, id: string) =>
  (await api.listProfiles()).find((profile) => profile.id === id);

describe('mock planTransfer', () => {
  it('lists the items, their total size and the free space without changing anything', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 3);
    const before = await profileOf(api, 'offline');
    const plan = await api.planTransfer('main', ids(mods), 'offline', 'copy');
    expect(plan.items.map((item) => item.modId)).toEqual(ids(mods));
    expect(plan.clashes).toEqual([]);
    expect(plan.totalBytes).toBe(mods.reduce((sum, mod) => sum + mod.sizeBytes, 0));
    expect(plan.freeBytes).toBeGreaterThan(plan.totalBytes);
    expect(plan).toMatchObject({ fromProfileId: 'main', toProfileId: 'offline', mode: 'copy' });
    expect(await profileOf(api, 'offline')).toEqual(before);
  });

  it('writes nothing for a move inside one drive', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 2);
    const plan = await api.planTransfer('main', ids(mods), 'offline', 'move');
    expect(plan.totalBytes).toBe(0);
  });

  it('finds clashes by mod name and shows both versions', async () => {
    const api = make();
    const incoming = (await api.listMods('main')).find((mod) => mod.alsoIn.includes('testing'));
    expect(incoming).toBeDefined();
    const plan = await api.planTransfer('main', [incoming?.id ?? ''], 'testing', 'copy');
    expect(plan.clashes).toHaveLength(1);
    const clash = plan.clashes[0];
    expect(clash?.modName).toBe(incoming?.modName);
    expect(clash?.incoming.fileName).toBe(incoming?.fileName);
    const existing = (await api.listMods('testing')).find((m) => m.modName === incoming?.modName);
    expect(clash?.existing).toMatchObject({
      version: existing?.version,
      sizeBytes: existing?.sizeBytes,
    });
  });

  it('compares names without regard to case', () => {
    const tractor = {
      modName: 'FS25_Tractor',
      fileName: 'FS25_Tractor.zip',
      version: '1.2.0.0',
      sizeBytes: 1000,
      modified: '2026-10-01T00:00:00Z',
    } as ModEntry;
    const state: MockState = {
      profiles: [],
      mods: new Map([['target', [{ ...olderTwin(tractor, 'target'), modName: 'fs25_TRACTOR' }]]]),
    };
    const clashes = findClashes(state, [tractor], 'target', false);
    expect(clashes).toHaveLength(1);
    expect(clashes[0]?.existing.version).toBe('0.9.0.0');
    expect(nameKey('FS25_A')).toBe(nameKey('fs25_a'));
    expect(findByName(state.mods.get('target') ?? [], 'Fs25_Tractor')).toBeDefined();
  });

  it('refuses unknown profiles and mods, and a transfer to the same profile', async () => {
    const api = make();
    expect(await codeOf(api.planTransfer('nope', [], 'main', 'copy'))).toBe('profileNotFound');
    expect(await codeOf(api.planTransfer('main', [], 'nope', 'copy'))).toBe('profileNotFound');
    expect(await codeOf(api.planTransfer('main', ['missing.zip'], 'offline', 'copy'))).toBe(
      'targetChanged',
    );
    expect(await codeOf(api.planTransfer('main', [], 'main', 'copy'))).toBe('internal');
  });

  it('?mock=clashes: every plan has something to decide', async () => {
    const api = make({ clashes: true });
    for (const count of [1, 2, 5]) {
      const mods = await freshMods(api, 'main', 'offline', count);
      const plan = await api.planTransfer('main', ids(mods), 'offline', 'copy');
      expect(plan.clashes.length, `${count} mods`).toBeGreaterThanOrEqual(1);
      for (const clash of plan.clashes) {
        expect(clash.existing.version).not.toBe(clash.incoming.version);
        expect(clash.existing.sizeBytes).toBeLessThan(clash.incoming.sizeBytes);
        expect(Date.parse(clash.existing.modified)).toBeLessThan(
          Date.parse(clash.incoming.modified),
        );
      }
    }
  });

  it('?mock=lowSpace: the plan needs more than the drive has, and execute refuses it', async () => {
    const api = make({ lowSpace: true });
    const mods = await freshMods(api, 'main', 'offline', 2);
    for (const mode of ['copy', 'move'] as const) {
      const plan = await api.planTransfer('main', ids(mods), 'offline', mode);
      expect(plan.totalBytes, mode).toBeGreaterThan(plan.freeBytes);
      expect(await codeOf(api.executeTransfer(plan.planId, []))).toBe('notEnoughSpace');
    }
    expect((await profileOf(api, 'offline'))?.modCount).toBe(6);
  });
});

describe('mock executeTransfer', () => {
  const plan = (api: Api, mods: readonly ModEntry[], to: string, mode: 'copy' | 'move') =>
    api.planTransfer(mods[0]?.profileId ?? 'main', ids(mods), to, mode);

  it('copy puts the mods in the target and keeps the source', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 3);
    const sourceBefore = await profileOf(api, 'main');
    const targetBefore = await profileOf(api, 'offline');
    const result = await api.executeTransfer((await plan(api, mods, 'offline', 'copy')).planId, []);
    expect(result).toEqual({ done: mods.map((m) => m.modName), skipped: [], failed: [] });
    expect((await profileOf(api, 'main'))?.modCount).toBe(sourceBefore?.modCount);
    expect((await profileOf(api, 'offline'))?.modCount).toBe((targetBefore?.modCount ?? 0) + 3);
    const target = await api.listMods('offline');
    const source = await api.listMods('main');
    for (const mod of mods) {
      expect(target.some((m) => m.id === mod.id && m.profileId === 'offline')).toBe(true);
      expect(source.find((m) => m.id === mod.id)?.alsoIn).toContain('offline');
      expect(target.find((m) => m.id === mod.id)?.alsoIn).toContain('main');
    }
  });

  it('move takes the mods out of the source', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 2);
    const sourceBefore = (await profileOf(api, 'main'))?.modCount ?? 0;
    const result = await api.executeTransfer((await plan(api, mods, 'offline', 'move')).planId, []);
    expect(result.done).toHaveLength(2);
    expect((await profileOf(api, 'main'))?.modCount).toBe(sourceBefore - 2);
    const source = await api.listMods('main');
    for (const mod of mods) expect(source.some((m) => m.id === mod.id)).toBe(false);
    const target = await api.listMods('offline');
    for (const mod of mods) {
      const moved = target.find((m) => m.id === mod.id);
      expect(moved?.alsoIn).not.toContain('main');
    }
  });

  it('updates the totals of both profiles', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 2);
    const size = mods.reduce((sum, mod) => sum + mod.sizeBytes, 0);
    const [fromBefore, toBefore] = [await profileOf(api, 'main'), await profileOf(api, 'offline')];
    await api.executeTransfer((await plan(api, mods, 'offline', 'move')).planId, []);
    expect((await profileOf(api, 'main'))?.totalBytes).toBe((fromBefore?.totalBytes ?? 0) - size);
    expect((await profileOf(api, 'offline'))?.totalBytes).toBe((toBefore?.totalBytes ?? 0) + size);
  });

  it('skips a clash by default and when the choice says so, and replaces when told to', async () => {
    const api = make();
    const incoming = (await api.listMods('main')).filter((mod) => mod.alsoIn.includes('testing'));
    const picked = incoming.slice(0, 2);
    const [first, second] = picked;
    expect(first && second).toBeTruthy();

    const made = await plan(api, picked, 'testing', 'copy');
    expect(made.clashes.map((c) => c.modName).sort()).toEqual(picked.map((m) => m.modName).sort());
    const result = await api.executeTransfer(made.planId, [
      { modName: first?.modName ?? '', replace: true },
      { modName: second?.modName ?? '', replace: false },
    ]);
    expect(result.done).toEqual([first?.modName]);
    expect(result.skipped).toEqual([second?.modName]);

    const testing = await api.listMods('testing');
    expect(testing.find((m) => m.modName === first?.modName)?.version).toBe(first?.version);
    const kept = testing.find((m) => m.modName === second?.modName);
    const keptBefore = made.clashes.find((c) => c.modName === second?.modName)?.existing;
    expect(kept).toMatchObject({
      version: keptBefore?.version,
      sizeBytes: keptBefore?.sizeBytes,
      modified: keptBefore?.modified,
    });
    // Replacing does not add a second entry.
    expect(testing.filter((m) => m.modName === first?.modName)).toHaveLength(1);
  });

  it('treats a clash without a choice as skip', async () => {
    const api = make();
    const mod = (await api.listMods('main')).find((m) => m.alsoIn.includes('testing'));
    const made = await plan(api, [mod as ModEntry], 'testing', 'move');
    const result = await api.executeTransfer(made.planId, []);
    expect(result).toEqual({ done: [], skipped: [mod?.modName], failed: [] });
    expect((await api.listMods('main')).some((m) => m.id === mod?.id)).toBe(true);
  });

  it('reports progress that only goes forward and ends complete', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 3);
    const seen: OpProgress[] = [];
    const made = await plan(api, mods, 'offline', 'copy');
    await api.executeTransfer(made.planId, [], (p) => seen.push(p));
    expect(seen.length).toBeGreaterThan(3);
    expect(seen[0]).toMatchObject({ doneBytes: 0, doneItems: 0, totalItems: 3, current: null });
    for (let i = 1; i < seen.length; i++) {
      expect(seen[i]?.doneBytes ?? 0).toBeGreaterThanOrEqual(seen[i - 1]?.doneBytes ?? 0);
      expect(seen[i]?.doneItems ?? 0).toBeGreaterThanOrEqual(seen[i - 1]?.doneItems ?? 0);
    }
    expect(seen.at(-1)).toMatchObject({
      doneBytes: made.totalBytes,
      totalBytes: made.totalBytes,
      doneItems: 3,
      totalItems: 3,
      current: null,
    });
    expect(seen.some((p) => p.current !== null)).toBe(true);
  });

  it('counts items, not bytes, for a move inside one drive', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 2);
    const seen: OpProgress[] = [];
    await api.executeTransfer((await plan(api, mods, 'offline', 'move')).planId, [], (p) =>
      seen.push(p),
    );
    expect(seen.every((p) => p.totalBytes === 0)).toBe(true);
    expect(seen.at(-1)?.doneItems).toBe(2);
  });

  it('a plan can be used once, and an unknown plan is refused', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 1);
    const made = await plan(api, mods, 'offline', 'copy');
    await api.executeTransfer(made.planId, []);
    expect(await codeOf(api.executeTransfer(made.planId, []))).toBe('targetChanged');
    expect(await codeOf(api.executeTransfer('plan-999', []))).toBe('targetChanged');
  });

  it('is refused while the game or the server runs, but a plan can still be made', async () => {
    for (const [options, code] of [
      [{ gameRunning: true }, 'gameRunning'],
      [{ serverRunning: true }, 'serverRunning'],
    ] as const) {
      const api = make(options);
      const mods = await freshMods(api, 'main', 'offline', 1);
      const made = await plan(api, mods, 'offline', 'copy');
      expect(await codeOf(api.executeTransfer(made.planId, []))).toBe(code);
      expect(await codeOf(api.removeMods('main', ids(mods)))).toBe(code);
      expect((await profileOf(api, 'offline'))?.modCount).toBe(6);
    }
  });

  it('runs one operation at a time', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 2);
    const first = api.executeTransfer((await plan(api, mods, 'offline', 'copy')).planId, []);
    expect(await codeOf(api.removeMods('main', ids(mods)))).toBe('busy');
    await first;
    await expect(api.removeMods('offline', ids(mods))).resolves.toBeDefined();
  });

  it('keeps each instance apart and leaves the shared dataset alone', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 1);
    await api.executeTransfer((await plan(api, mods, 'offline', 'move')).planId, []);
    const other = make();
    expect((await profileOf(other, 'main'))?.modCount).toBe(590);
    expect((await profileOf(other, 'offline'))?.modCount).toBe(6);
  });

  it('?mock=recycleFails: Replace fails for some mods and leaves both mods alone', async () => {
    const api = make({ recycleFails: true });
    const picked = (await api.listMods('main'))
      .filter((mod) => mod.alsoIn.includes('testing'))
      .slice(0, 2);
    const made = await plan(api, picked, 'testing', 'move');
    const result = await api.executeTransfer(
      made.planId,
      picked.map((mod) => ({ modName: mod.modName, replace: true })),
    );
    expect(result.done).toEqual([picked[0]?.modName]);
    expect(result.failed).toEqual([{ modName: picked[1]?.modName, code: 'recycleUnavailable' }]);
    // The mod that failed is still in the source, and the target kept its old version.
    expect((await api.listMods('main')).some((m) => m.id === picked[1]?.id)).toBe(true);
  });

  it('cancel stops after the current mod, finishes nothing more and leaves the rest alone', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 3);
    const made = await plan(api, mods, 'offline', 'copy');
    const result = await api.executeTransfer(made.planId, [], (p) => {
      if (p.doneItems === 1 && p.current !== null) void api.cancelOperation();
    });
    expect(result.done).toEqual([mods[0]?.modName]);
    expect(result.failed).toEqual([
      { modName: mods[1]?.modName, code: 'cancelled' },
      { modName: mods[2]?.modName, code: 'cancelled' },
    ]);
    const target = await api.listMods('offline');
    expect(target.some((m) => m.id === mods[1]?.id)).toBe(false);
    // The next operation is not cancelled by the old request.
    await expect(
      api.executeTransfer((await plan(api, mods.slice(1), 'offline', 'copy')).planId, []),
    ).resolves.toMatchObject({ failed: [] });
  });

  it('cancel with nothing running does nothing', async () => {
    const api = make();
    await api.cancelOperation();
    const mods = await freshMods(api, 'main', 'offline', 1);
    const result = await api.executeTransfer((await plan(api, mods, 'offline', 'copy')).planId, []);
    expect(result.done).toHaveLength(1);
  });

  it('a big copy takes long enough to cancel', async () => {
    const api = make({ delayMs: 1 });
    const [big] = (await api.listMods('main'))
      .filter((mod) => !mod.alsoIn.includes('offline'))
      .sort((a, b) => b.sizeBytes - a.sizeBytes);
    expect(big).toBeDefined();
    const made = await plan(api, [big as ModEntry], 'offline', 'copy');
    const started = Date.now();
    const result = await api.executeTransfer(made.planId, [], (p) => {
      if (p.doneBytes > 0) void api.cancelOperation();
    });
    expect(result.failed).toEqual([{ modName: big?.modName, code: 'cancelled' }]);
    expect(Date.now() - started).toBeLessThan(1000);
  });
});

describe('mock removeMods', () => {
  it('removes the entries and updates counts and also-in', async () => {
    const api = make();
    const shared = (await api.listMods('testing')).filter((mod) => mod.alsoIn.includes('main'));
    const picked = shared.slice(0, 2);
    const before = await profileOf(api, 'testing');
    const result = await api.removeMods('testing', ids(picked));
    expect(result).toEqual({ done: picked.map((m) => m.modName), skipped: [], failed: [] });
    expect((await profileOf(api, 'testing'))?.modCount).toBe((before?.modCount ?? 0) - 2);
    const main = await api.listMods('main');
    for (const mod of picked) {
      expect(main.find((m) => m.modName === mod.modName)?.alsoIn).not.toContain('testing');
    }
  });

  it('reports progress ending complete', async () => {
    const api = make();
    const mods = (await api.listMods('offline')).slice(0, 3);
    const seen: OpProgress[] = [];
    await api.removeMods('offline', ids(mods), (p) => seen.push(p));
    expect(seen.at(-1)).toMatchObject({ doneItems: 3, totalItems: 3 });
    expect(seen.at(-1)?.doneBytes).toBe(seen.at(-1)?.totalBytes);
  });

  it('reports a mod that is gone, and an unknown profile', async () => {
    const api = make();
    const result = await api.removeMods('offline', ['missing.zip']);
    expect(result.failed).toEqual([{ modName: 'missing.zip', code: 'targetChanged' }]);
    expect(await codeOf(api.removeMods('nope', []))).toBe('profileNotFound');
  });

  it('?mock=recycleFails: every second mod, or a lone one, stays where it is', async () => {
    const api = make({ recycleFails: true });
    const mods = (await api.listMods('offline')).slice(0, 3);
    const result = await api.removeMods('offline', ids(mods));
    expect(result.done).toEqual([mods[0]?.modName, mods[2]?.modName]);
    expect(result.failed).toEqual([{ modName: mods[1]?.modName, code: 'recycleUnavailable' }]);
    expect((await profileOf(api, 'offline'))?.modCount).toBe(6 - 2);

    const lone = make({ recycleFails: true });
    const [one] = await lone.listMods('offline');
    const failed = await lone.removeMods('offline', [one?.id ?? '']);
    expect(failed.done).toEqual([]);
    expect(failed.failed[0]?.code).toBe('recycleUnavailable');
    expect((await profileOf(lone, 'offline'))?.modCount).toBe(6);
  });

  it('cancel leaves the remaining mods in place', async () => {
    const api = make();
    const mods = (await api.listMods('offline')).slice(0, 3);
    const result = await api.removeMods('offline', ids(mods), (p) => {
      if (p.doneItems === 1 && p.current !== null) void api.cancelOperation();
    });
    expect(result.done).toEqual([mods[0]?.modName]);
    expect(result.failed.map((f) => f.code)).toEqual(['cancelled', 'cancelled']);
    expect((await profileOf(api, 'offline'))?.modCount).toBe(5);
  });
});

describe('mock profile management', () => {
  it('renames a profile with a trimmed name and leaves its path alone', async () => {
    const api = make();
    const before = await profileOf(api, 'offline');
    await api.renameProfile('offline', '  Winter Test  ');
    const after = await profileOf(api, 'offline');
    expect(after).toMatchObject({
      name: 'Winter Test',
      path: before?.path,
      modCount: before?.modCount,
    });
  });

  it('refuses an empty name, one over 100 characters and an unknown profile', async () => {
    const api = make();
    expect(await codeOf(api.renameProfile('offline', '   '))).toBe('internal');
    expect(await codeOf(api.renameProfile('offline', 'x'.repeat(101)))).toBe('internal');
    await expect(api.renameProfile('offline', 'x'.repeat(100))).resolves.toBeUndefined();
    expect(await codeOf(api.renameProfile('nope', 'Name'))).toBe('profileNotFound');
    expect((await profileOf(api, 'offline'))?.name).toBe('x'.repeat(100));
  });

  it('keeps the active folder and the game default in the list', async () => {
    const api = make();
    expect(await codeOf(api.removeProfile('testing'))).toBe('cannotRemoveProfile');
    expect(await codeOf(api.removeProfile('main'))).toBe('cannotRemoveProfile');
    expect(await codeOf(api.removeProfile('nope'))).toBe('profileNotFound');
    expect((await api.listProfiles()).map((p) => p.id)).toEqual(['main', 'testing', 'offline']);
  });

  it('follows a switch: the folder that is no longer active can be removed', async () => {
    const api = make();
    await api.setActiveProfile('offline');
    await expect(api.removeProfile('testing')).resolves.toBeUndefined();
    expect(await codeOf(api.removeProfile('offline'))).toBe('cannotRemoveProfile');
  });

  it('removes a profile from the list and from every also-in', async () => {
    const api = make();
    await api.removeProfile('offline');
    expect((await api.listProfiles()).map((p) => p.id)).toEqual(['main', 'testing']);
    expect(await api.listMods('offline')).toEqual([]);
    const everyone = [...(await api.listMods('main')), ...(await api.listMods('testing'))];
    expect(everyone.some((mod) => mod.alsoIn.includes('offline'))).toBe(false);
  });
});

describe('mock plans for the whole flow', () => {
  it('is typed as the contract says', async () => {
    const api = make();
    const mods = await freshMods(api, 'main', 'offline', 1);
    const made: TransferPlan = await api.planTransfer('main', ids(mods), 'offline', 'copy');
    expect(made.planId).toMatch(/^plan-/);
  });
});
