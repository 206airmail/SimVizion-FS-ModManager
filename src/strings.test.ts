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

describe('strings.readingMods', () => {
  it('words the progress of a folder read', () => {
    expect(strings.states.readingMods(120, 590)).toBe('Reading 120 of 590 mods');
    expect(strings.states.readingMods(0, 1)).toBe('Reading 0 of 1 mod');
  });
});
