import { describe, expect, it } from 'vitest';
import type { IssueKind } from './api';
import { strings } from './strings';

describe('strings.issueDetail', () => {
  const t = strings.issueDetail;

  it('has a wording function for every issue kind', () => {
    const kinds: IssueKind[] = [
      'notAMod',
      'invalidFileName',
      'nestedArchive',
      'zipAndFolderTwin',
      'conflictCopy',
      'descVersionMismatch',
      'missingDependency',
    ];
    expect(Object.keys(t).sort()).toEqual([...kinds].sort());
  });

  it('words every notAMod reason code and ignores an unknown one', () => {
    for (const code of ['unreadableArchive', 'noModDesc', 'invalidModDesc']) {
      const text = t.notAMod(code);
      expect(text, code).toBeTruthy();
      expect(text, code).not.toContain(code);
    }
    expect(t.notAMod('somethingNew')).toBeNull();
    expect(t.notAMod('constructor')).toBeNull();
    expect(t.notAMod(null)).toBeNull();
  });

  it('puts the data of the other kinds into a sentence', () => {
    expect(t.nestedArchive('Mods/FS25_Inner.zip')).toContain('Mods/FS25_Inner.zip');
    expect(t.conflictCopy('FS25_AgroForests')).toBe('Copy of FS25_AgroForests');
    expect(t.zipAndFolderTwin('FS25_Twin.zip')).toContain('FS25_Twin.zip');
    expect(t.missingDependency('FS25_A, FS25_B')).toContain('FS25_A, FS25_B');
    expect(t.descVersionMismatch('106')).toContain('106');
  });

  it('shows nothing for an absent detail', () => {
    for (const kind of Object.keys(t) as IssueKind[]) expect(t[kind](null), kind).toBeNull();
  });

  it('shows nothing for a descVersion that is not a number', () => {
    expect(t.descVersionMismatch('newer')).toBeNull();
    expect(t.descVersionMismatch('')).toBeNull();
  });

  it('has no detail for invalidFileName', () => {
    expect(t.invalidFileName()).toBeNull();
  });
});

describe('strings.switching and strings.restore', () => {
  it('names the new active folder and the restore date', () => {
    expect(strings.switching.doneTitle('Offline Testing')).toContain('Offline Testing');
    expect(strings.restore.action('Nov 17, 2026, 6:42 PM')).toBe(
      'Restore the backup from Nov 17, 2026, 6:42 PM',
    );
  });

  it('words an undo with and without a known folder', () => {
    expect(strings.switching.undoneText('Testing')).toContain('Testing');
    expect(strings.switching.undoneText(null)).toBeTruthy();
  });

  it('has a title for each action that can fail', () => {
    for (const title of Object.values(strings.actionFailed)) expect(title).toBeTruthy();
    expect(strings.actionFailed.setActive).not.toBe(strings.actionFailed.undoSwitch);
  });
});

describe('strings.readingMods', () => {
  it('words the progress of a folder read', () => {
    expect(strings.states.readingMods(120, 590)).toBe('Reading 120 of 590 mods');
    expect(strings.states.readingMods(0, 1)).toBe('Reading 0 of 1 mod');
  });
});

describe('strings.savegames', () => {
  const t = strings.savegames;

  it('shows play time as hours and minutes', () => {
    expect(t.playTime(0)).toBe('0 min');
    expect(t.playTime(59)).toBe('59 min');
    expect(t.playTime(60)).toBe('1 h 0 min');
    expect(t.playTime(14872)).toBe(`${(247).toLocaleString()} h 52 min`);
    expect(t.playTime(null)).toBe('–');
  });

  it('shows a large money value with separators, and a missing one as a dash', () => {
    expect(t.money(48215730)).toBe((48215730).toLocaleString());
    expect(t.money(null)).toBe('–');
  });

  it('words the copy buttons and the summary', () => {
    expect(t.copyFrom(12, 'Main Farm')).toBe('Copy 12 from Main Farm');
    expect(t.summary(477, 1)).toBe('477 mods, 1 required');
    expect(t.summary(1, 0)).toBe('1 mod');
  });

  it('has wording for every state, and for the two new error codes', () => {
    for (const state of Object.values(t.states)) expect(state).toBeTruthy();
    expect(strings.errors.savegameMissing).toBeTruthy();
    expect(strings.errors.savegameUnreadable).toBeTruthy();
  });
});
