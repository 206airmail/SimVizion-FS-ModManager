import { describe, expect, it } from 'vitest';
import type { OpProgress, OpResult } from '../../api';
import { ApiFailure } from '../../lib/errors';
import { runUpdates, type Install } from './runUpdates';

const names = new Map([
  ['main', 'Main Farm'],
  ['testing', 'Testing'],
]);
const never = () => false;
const ok = (done: string[]): OpResult => ({ done, skipped: [], failed: [] });

describe('runUpdates with one mod', () => {
  it('words each entry by folder name and passes progress through', async () => {
    const seen: OpProgress[] = [];
    const install: Install = async (_name, ids, report) => {
      report({ doneBytes: 5, totalBytes: 10, current: 'a.zip', doneItems: 0, totalItems: 2 });
      return {
        done: [ids[0] ?? ''],
        skipped: [],
        failed: [{ modName: 'testing', code: 'verifyFailed' }],
      };
    };
    const result = await runUpdates(
      install,
      [{ modName: 'FS25_A', profileIds: ['main', 'testing'] }],
      names,
      (p) => seen.push(p),
      never,
    );
    expect(result).toEqual({
      done: ['Main Farm'],
      skipped: [],
      failed: [{ modName: 'Testing', code: 'verifyFailed' }],
    });
    expect(seen).toEqual([
      { doneBytes: 5, totalBytes: 10, current: 'a.zip', doneItems: 0, totalItems: 2 },
    ]);
  });

  it('lets a refusal of the whole call through, so it is the failure shown', async () => {
    const install: Install = () => Promise.reject(new ApiFailure('gameRunning'));
    await expect(
      runUpdates(
        install,
        [{ modName: 'FS25_A', profileIds: ['main'] }],
        names,
        () => undefined,
        never,
      ),
    ).rejects.toMatchObject({ code: 'gameRunning' });
  });
});

describe('runUpdates with several mods', () => {
  const jobs = [
    { modName: 'FS25_A', profileIds: ['main'] },
    { modName: 'FS25_B', profileIds: ['main'] },
    { modName: 'FS25_C', profileIds: ['main'] },
  ];

  it('runs them in order and names each entry by mod', async () => {
    const order: string[] = [];
    const install: Install = async (name, ids) => {
      order.push(name);
      return name === 'FS25_B'
        ? { done: [], skipped: [], failed: [{ modName: ids[0] ?? '', code: 'notInstallable' }] }
        : ok(ids);
    };
    const result = await runUpdates(install, jobs, names, () => undefined, never);
    expect(order).toEqual(['FS25_A', 'FS25_B', 'FS25_C']);
    expect(result).toEqual({
      done: ['FS25_A', 'FS25_C'],
      skipped: [],
      failed: [{ modName: 'FS25_B', code: 'notInstallable' }],
    });
  });

  it('counts mods in the progress, one step per mod', async () => {
    const seen: OpProgress[] = [];
    const install: Install = async (_name, ids, report) => {
      report({ doneBytes: 1, totalBytes: 9, current: 'x.zip', doneItems: 0, totalItems: 1 });
      return ok(ids);
    };
    await runUpdates(install, jobs, names, (p) => seen.push(p), never);
    expect(seen.map((p) => [p.doneItems, p.totalItems, p.totalBytes])).toEqual([
      [0, 3, 0],
      [1, 3, 0],
      [2, 3, 0],
    ]);
    expect(seen[0]?.current).toBe('x.zip');
  });

  it('reports the mods not started as cancelled after Cancel', async () => {
    let cancelled = false;
    const install: Install = async (_name, ids) => {
      cancelled = true;
      return ok(ids);
    };
    const result = await runUpdates(
      install,
      jobs,
      names,
      () => undefined,
      () => cancelled,
    );
    expect(result.done).toEqual(['FS25_A']);
    expect(result.failed).toEqual([
      { modName: 'FS25_B', code: 'cancelled' },
      { modName: 'FS25_C', code: 'cancelled' },
    ]);
  });

  it('fails the mods still waiting with the code of a later refusal', async () => {
    let calls = 0;
    const install: Install = async (_name, ids) => {
      calls += 1;
      if (calls === 2) throw new ApiFailure('gameRunning');
      return ok(ids);
    };
    const result = await runUpdates(install, jobs, names, () => undefined, never);
    expect(result.done).toEqual(['FS25_A']);
    expect(result.failed).toEqual([
      { modName: 'FS25_B', code: 'gameRunning' },
      { modName: 'FS25_C', code: 'gameRunning' },
    ]);
    expect(calls).toBe(2);
  });

  it('fails only that mod when the refusal is about the mod itself', async () => {
    // The core refuses before writing anything when, say, the asset has another file name.
    const install: Install = async (name, ids) => {
      if (name !== 'FS25_B') return ok(ids);
      throw new ApiFailure('notInstallable');
    };
    const result = await runUpdates(install, jobs, names, () => undefined, never);
    expect(result.done).toEqual(['FS25_A', 'FS25_C']);
    expect(result.failed).toEqual([{ modName: 'FS25_B', code: 'notInstallable' }]);
  });

  it('carries on past a first mod that cannot be installed', async () => {
    const install: Install = async (name, ids) => {
      if (name === 'FS25_A') throw new ApiFailure('folderMissing');
      return ok(ids);
    };
    const result = await runUpdates(install, jobs, names, () => undefined, never);
    expect(result.done).toEqual(['FS25_B', 'FS25_C']);
    expect(result.failed).toEqual([{ modName: 'FS25_A', code: 'folderMissing' }]);
  });

  it('lets a refusal of the first mod through', async () => {
    const install: Install = () => Promise.reject(new ApiFailure('updatesDisabled'));
    await expect(runUpdates(install, jobs, names, () => undefined, never)).rejects.toMatchObject({
      code: 'updatesDisabled',
    });
  });
});
