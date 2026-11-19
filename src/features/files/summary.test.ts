import { describe, expect, it } from 'vitest';
import type { ErrorCode, OpResult } from '../../api';
import { strings } from '../../strings';
import { describeOutcome, emptyResult, type OpKind, type Outcome } from './summary';

const outcome = (
  kind: OpKind,
  result: Partial<OpResult>,
  extra: Partial<Outcome> = {},
): Outcome => ({
  kind,
  target: kind === 'remove' ? null : 'Offline Testing',
  result: { ...emptyResult, ...result },
  replaced: [],
  cancelled: false,
  ...extra,
});

describe('describeOutcome', () => {
  it('says a clean copy or move name the target and count', () => {
    const copied = describeOutcome(outcome('copy', { done: ['A', 'B', 'C'] }));
    expect(copied).toMatchObject({ tone: 'success', title: 'Copied 3 mods to Offline Testing' });
    expect(copied.text).toBe('');
    expect(describeOutcome(outcome('move', { done: ['A'] })).title).toBe(
      'Moved 1 mod to Offline Testing',
    );
  });

  it('notes after a remove that the mods are in the Recycle Bin', () => {
    const view = describeOutcome(outcome('remove', { done: ['A', 'B'] }));
    expect(view.title).toBe('Removed 2 mods');
    expect(view.text).toContain('Windows Recycle Bin');
    expect(view.tone).toBe('success');
  });

  it('does not mention the Recycle Bin when nothing was removed', () => {
    const view = describeOutcome(
      outcome('remove', { failed: [{ modName: 'A', code: 'recycleUnavailable' }] }),
    );
    expect(view.text).not.toContain('Recycle Bin');
    expect(view.title).toBe('Nothing was removed');
  });

  it('counts done, skipped and failed, and words each failed mod', () => {
    const view = describeOutcome(
      outcome('copy', {
        done: ['A'],
        skipped: ['B', 'C'],
        failed: [
          { modName: 'D', code: 'recycleUnavailable' },
          { modName: 'E', code: 'notEnoughSpace' },
        ],
      }),
    );
    expect(view.tone).toBe('warning');
    expect(view.title).toBe('Copied 1 mod to Offline Testing');
    expect(view.text).toBe('2 skipped, 2 could not be copied.');
    expect(view.problems).toEqual([
      { modName: 'D', reason: strings.errors.recycleUnavailable },
      { modName: 'E', reason: strings.errors.notEnoughSpace },
    ]);
  });

  it('is an error when every mod failed', () => {
    const view = describeOutcome(
      outcome('remove', { failed: [{ modName: 'A', code: 'recycleUnavailable' }] }),
    );
    expect(view.tone).toBe('error');
    expect(view.text).toBe('1 could not be removed.');
  });

  it('says a transfer where everything was skipped did nothing', () => {
    const view = describeOutcome(outcome('copy', { skipped: ['A', 'B'] }));
    expect(view).toMatchObject({
      tone: 'success',
      title: 'Nothing was copied',
      text: strings.fileOps.result.allSkipped,
    });
  });

  it('notes the replaced mods that went to the Recycle Bin, only those that were done', () => {
    const view = describeOutcome(
      outcome(
        'copy',
        { done: ['A', 'B'], failed: [{ modName: 'C', code: 'recycleUnavailable' }] },
        {
          replaced: ['A', 'C'],
        },
      ),
    );
    expect(view.text).toContain('The replaced mod is in the Windows Recycle Bin.');
    const many = describeOutcome(outcome('move', { done: ['A', 'B'] }, { replaced: ['A', 'B'] }));
    expect(many.text).toContain('2 replaced mods are in the Windows Recycle Bin.');
  });

  it('counts a cancelled item separately and does not list it as a failure', () => {
    const view = describeOutcome(
      outcome('copy', {
        done: ['A'],
        failed: [
          { modName: 'B', code: 'cancelled' },
          { modName: 'C', code: 'cancelled' },
        ],
      }),
    );
    expect(view.tone).toBe('warning');
    expect(view.text).toBe('2 not done because it was cancelled.');
    expect(view.problems).toEqual([]);
  });

  it('words a call that ended with the cancelled code', () => {
    const view = describeOutcome(outcome('copy', {}, { cancelled: true }));
    expect(view).toMatchObject({
      tone: 'warning',
      title: 'Cancelled',
      text: strings.errors.cancelled,
    });
  });

  it('words a failure with a code it does not know as an internal error', () => {
    const view = describeOutcome(
      outcome('copy', { failed: [{ modName: 'A', code: 'brandNew' as ErrorCode }] }),
    );
    expect(view.problems[0]?.reason).toBe(strings.errors.internal);
  });

  it('words every kind of result for every error code a mod can fail with', () => {
    const codes = Object.keys(strings.errors) as ErrorCode[];
    for (const code of codes) {
      const view = describeOutcome(outcome('copy', { failed: [{ modName: 'A', code }] }));
      if (code === 'cancelled') expect(view.problems).toEqual([]);
      else expect(view.problems[0]?.reason, code).toBe(strings.errors[code]);
    }
  });
});

describe('file operation wording', () => {
  const t = strings.fileOps;

  it('titles and confirm buttons follow the mode and the count', () => {
    expect(t.transfer.title('copy', 1)).toBe('Copy 1 mod to another folder');
    expect(t.transfer.title('move', 4)).toBe('Move 4 mods to another folder');
    expect(t.transfer.confirm('copy', 3)).toBe('Copy 3 mods');
    expect(t.transfer.confirm('move', 0)).toBe('Nothing to move');
    expect(t.remove.confirm(1)).toBe('Remove 1 mod');
  });

  it('says what a skipped or empty plan will do', () => {
    expect(t.transfer.confirmNote(0, 3)).toContain('nothing to do');
    expect(t.transfer.confirmNote(2, 0)).toBe('2 mods will be transferred.');
    expect(t.transfer.confirmNote(2, 1)).toBe('2 mods will be transferred and 1 skipped.');
    expect(t.transfer.clashTitle(1, 'Testing')).toBe('1 mod already exists in Testing');
    expect(t.transfer.clashTitle(3, 'Testing')).toBe('3 mods already exist in Testing');
  });

  it('states the Recycle Bin, the left-in-place rule and the sync note in the confirmation', () => {
    expect(t.remove.recycle).toContain('Recycle Bin');
    expect(t.remove.leftInPlace).toContain('left where it is');
    expect(t.remove.synced).toContain('synced');
    expect(t.remove.lead(3, '12 MB', 'Testing')).toBe(
      '3 mods (12 MB) will be removed from Testing.',
    );
  });

  it('words the progress banner for each operation', () => {
    expect(t.progress.title('copy', 'Testing')).toBe('Copying mods to Testing');
    expect(t.progress.title('move', 'Testing')).toBe('Moving mods to Testing');
    expect(t.progress.title('remove', null)).toContain('Recycle Bin');
    expect(t.progress.items(2, 5)).toBe('2 of 5 mods');
    expect(t.progress.bytes('1.2 GB', '3.4 GB')).toBe('1.2 GB of 3.4 GB');
    expect(t.progress.percent(0.5)).toBe('50%');
  });

  it('explains why the actions are blocked, and every worded reason is distinct', () => {
    const reasons = [t.blockedGame, t.blockedServer, t.blockedRunning];
    expect(new Set(reasons).size).toBe(3);
    for (const reason of reasons) expect(reason.length).toBeGreaterThan(20);
  });

  it('words the profile menu rules', () => {
    const m = strings.profileMenu;
    expect(m.removeTitle('Testing')).toBe('Remove Testing from the list?');
    expect(m.removeText).toContain('not touched');
    expect(m.renameTooLong(100)).toBe('Use at most 100 characters.');
    expect(new Set([m.blockedActive, m.blockedDefault, m.blockedRunning]).size).toBe(3);
  });
});
