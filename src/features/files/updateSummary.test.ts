import { describe, expect, it } from 'vitest';
import type { OpResult } from '../../api';
import { strings } from '../../strings';
import { describeOutcome, emptyResult, type Outcome } from './summary';

const outcome = (result: Partial<OpResult>, target: string | null = 'FS25_Tractor'): Outcome => ({
  kind: 'update',
  target,
  result: { ...emptyResult, ...result },
  replaced: [],
  cancelled: false,
});

describe('describeOutcome for an update', () => {
  it('names the mod and counts the folders, and says where the old versions went', () => {
    const view = describeOutcome(outcome({ done: ['Main Farm', 'Testing'] }));
    expect(view).toMatchObject({ tone: 'success', title: 'Updated FS25_Tractor in 2 folders' });
    expect(view.text).toContain('Windows Recycle Bin');
    expect(describeOutcome(outcome({ done: ['Main Farm'] })).title).toBe(
      'Updated FS25_Tractor in 1 folder',
    );
  });

  it('counts mods when several were updated', () => {
    expect(describeOutcome(outcome({ done: ['FS25_A', 'FS25_B', 'FS25_C'] }, null)).title).toBe(
      'Updated 3 mods',
    );
  });

  it('is a warning with per-folder reasons when some folders failed', () => {
    const view = describeOutcome(
      outcome({
        done: ['Main Farm'],
        failed: [{ modName: 'Testing', code: 'recycleUnavailable' }],
      }),
    );
    expect(view.tone).toBe('warning');
    expect(view.text).toContain('1 folder could not be updated');
    expect(view.problems).toEqual([
      { modName: 'Testing', reason: strings.errors.recycleUnavailable },
    ]);
  });

  it('is an error when nothing was updated, with the verification failure worded for each folder', () => {
    const view = describeOutcome(
      outcome({
        failed: [
          { modName: 'Main Farm', code: 'verifyFailed' },
          { modName: 'Testing', code: 'verifyFailed' },
        ],
      }),
    );
    expect(view.tone).toBe('error');
    expect(view.title).toBe('Nothing was updated');
    expect(view.text).toBe('2 folders could not be updated.');
    expect(view.problems.map((p) => p.reason)).toEqual([
      strings.errors.verifyFailed,
      strings.errors.verifyFailed,
    ]);
  });

  it('counts failures as mods, not folders, when several mods were updated', () => {
    const view = describeOutcome(
      outcome({ done: ['FS25_A'], failed: [{ modName: 'FS25_B', code: 'notInstallable' }] }, null),
    );
    expect(view.text).toContain('1 could not be updated');
    expect(view.text).not.toContain('folder');
  });

  it('keeps a cancelled folder apart from a failed one', () => {
    const view = describeOutcome(
      outcome({
        done: ['Main Farm'],
        failed: [{ modName: 'Testing', code: 'cancelled' }],
      }),
    );
    expect(view.tone).toBe('warning');
    expect(view.text).toContain('1 not done because it was cancelled');
    expect(view.problems).toEqual([]);
  });

  it('words the progress banner for one mod and for several', () => {
    const p = strings.fileOps.progress;
    expect(p.title('update', 'FS25_Tractor')).toBe('Updating FS25_Tractor');
    expect(p.title('update', null)).toBe('Updating mods');
  });
});
