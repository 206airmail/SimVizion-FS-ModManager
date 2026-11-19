import { describe, expect, it } from 'vitest';
import type { HealthRow, IssueKind, Severity } from '../../api';
import {
  compareRows,
  countKinds,
  defaultFilter,
  effectiveFilter,
  filterRows,
  folderOptions,
  presentKinds,
  worstSeverity,
} from './rows';

const row = (
  profileId: string,
  modName: string,
  issues: [IssueKind, Severity][],
  title = modName,
): HealthRow => ({
  profileId,
  profileName: profileId === 'main' ? 'Main Farm' : 'Testing',
  modId: `${modName}.zip`,
  modName,
  title,
  issues: issues.map(([kind, severity]) => ({ kind, severity, detail: null })),
});

const rows: HealthRow[] = [
  row('main', 'FS25_A', [['descVersionMismatch', 'info']], 'Alpha'),
  row(
    'main',
    'FS25_B',
    [
      ['missingDependency', 'error'],
      ['descVersionMismatch', 'info'],
    ],
    'Beta',
  ),
  row('testing', 'FS25_C', [['conflictCopy', 'warning']], 'Charlie'),
  row('testing', 'FS25_D', [['notAMod', 'error']], 'Delta'),
];

describe('countKinds', () => {
  it('counts mods per kind, and a mod with two kinds once in each', () => {
    const counts = countKinds(rows);
    expect(counts.descVersionMismatch).toBe(2);
    expect(counts.missingDependency).toBe(1);
    expect(counts.conflictCopy).toBe(1);
    expect(counts.notAMod).toBe(1);
    expect(counts.nestedArchive).toBe(0);
  });

  it('counts a mod once even when it repeats a kind', () => {
    const twice = row('main', 'FS25_X', [
      ['invalidFileName', 'error'],
      ['invalidFileName', 'error'],
    ]);
    expect(countKinds([twice]).invalidFileName).toBe(1);
  });

  it('reports the number of mods with issues as "all" (the rail total)', () => {
    expect(countKinds(rows).all).toBe(4);
    expect(countKinds([]).all).toBe(0);
  });
});

describe('presentKinds', () => {
  it('lists only the kinds that occur, worst first', () => {
    expect(presentKinds(rows)).toEqual([
      'notAMod',
      'missingDependency',
      'conflictCopy',
      'descVersionMismatch',
    ]);
  });
});

describe('filterRows', () => {
  it('keeps everything by default, worst issue first', () => {
    const shown = filterRows(rows, defaultFilter);
    expect(shown.map((r) => r.modName)).toEqual(['FS25_B', 'FS25_D', 'FS25_C', 'FS25_A']);
  });

  it('filters by kind, and a mod with several kinds matches each of them', () => {
    expect(filterRows(rows, { kind: 'descVersionMismatch', folder: 'all' })).toHaveLength(2);
    expect(filterRows(rows, { kind: 'missingDependency', folder: 'all' })).toHaveLength(1);
  });

  it('filters by folder and combines with the kind', () => {
    expect(filterRows(rows, { kind: 'all', folder: 'testing' }).map((r) => r.modName)).toEqual([
      'FS25_D',
      'FS25_C',
    ]);
    expect(filterRows(rows, { kind: 'descVersionMismatch', folder: 'testing' })).toEqual([]);
  });

  it('does not reorder the report it was given', () => {
    const before = rows.map((r) => r.modName);
    filterRows(rows, defaultFilter);
    expect(rows.map((r) => r.modName)).toEqual(before);
  });
});

describe('compareRows and worstSeverity', () => {
  it('takes the worst severity of a row', () => {
    expect(worstSeverity(rows[1]!)).toBe('error');
    expect(worstSeverity(rows[2]!)).toBe('warning');
  });

  it('orders equal severities by title, then folder', () => {
    const a = row('testing', 'FS25_Z', [['notAMod', 'error']], 'Same');
    const b = row('main', 'FS25_Y', [['notAMod', 'error']], 'Same');
    expect([a, b].sort(compareRows).map((r) => r.profileId)).toEqual(['main', 'testing']);
  });
});

describe('folderOptions', () => {
  it('lists the folders with issues and their counts, in report order', () => {
    expect(folderOptions(rows)).toEqual([
      { profileId: 'main', name: 'Main Farm', count: 2 },
      { profileId: 'testing', name: 'Testing', count: 2 },
    ]);
  });
});

describe('effectiveFilter', () => {
  it('keeps a filter that still matches', () => {
    const filter = { kind: 'conflictCopy', folder: 'testing' } as const;
    expect(effectiveFilter(rows, filter)).toEqual(filter);
  });

  it('drops a folder that has no rows left, and a kind nothing has in that folder', () => {
    expect(effectiveFilter(rows, { kind: 'all', folder: 'gone' })).toEqual(defaultFilter);
    expect(effectiveFilter(rows, { kind: 'nestedArchive', folder: 'all' })).toEqual(defaultFilter);
    expect(effectiveFilter(rows, { kind: 'conflictCopy', folder: 'main' })).toEqual({
      kind: 'all',
      folder: 'main',
    });
  });
});
