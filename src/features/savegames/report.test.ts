import { describe, expect, it } from 'vitest';
import type { SaveModRow, SaveModState } from '../../api';
import {
  copyGroups,
  countReport,
  filterReport,
  foundNowhere,
  idsByName,
  missingRequired,
} from './report';

const row = (
  modName: string,
  state: SaveModState,
  extra: Partial<SaveModRow> = {},
): SaveModRow => ({
  modName,
  title: modName,
  savedVersion: '1.0.0.0',
  required: false,
  state,
  installed: null,
  foundIn: [],
  ...extra,
});

const rows: SaveModRow[] = [
  row('FS25_map', 'missing', { required: true, foundIn: ['main'] }),
  row('FS25_a', 'present'),
  row('FS25_b', 'versionDiffers'),
  row('FS25_c', 'fileDiffers'),
  row('FS25_d', 'hashPending'),
  row('FS25_e', 'missing', { foundIn: ['main', 'offline'] }),
  row('FS25_f', 'missing', { foundIn: ['offline'] }),
  row('FS25_g', 'missing'),
  row('pdlc_one', 'dlcInstalled'),
  row('pdlc_two', 'dlcMissing'),
];

describe('report counts and filters', () => {
  it('counts each filter, with DLC following present or missing', () => {
    expect(countReport(rows)).toEqual({
      all: 10,
      missing: 5,
      version: 1,
      file: 1,
      present: 2,
      checking: 1,
    });
  });

  it('filters by the same buckets, and keeps everything for All', () => {
    expect(filterReport(rows, 'all')).toHaveLength(10);
    expect(filterReport(rows, 'missing').map((r) => r.modName)).toEqual([
      'FS25_map',
      'FS25_e',
      'FS25_f',
      'FS25_g',
      'pdlc_two',
    ]);
    expect(filterReport(rows, 'present').map((r) => r.modName)).toEqual(['FS25_a', 'pdlc_one']);
    expect(filterReport(rows, 'checking').map((r) => r.modName)).toEqual(['FS25_d']);
  });

  it('has counts that match the filters', () => {
    const counts = countReport(rows);
    for (const filter of ['all', 'missing', 'version', 'file', 'present', 'checking'] as const) {
      expect(filterReport(rows, filter)).toHaveLength(counts[filter]);
    }
  });
});

describe('missing required map', () => {
  it('finds the required mod when it is missing', () => {
    expect(missingRequired(rows)?.modName).toBe('FS25_map');
  });

  it('is null when the required mod is present, or a DLC is the only thing missing', () => {
    expect(
      missingRequired([row('FS25_map', 'present', { required: true }), row('FS25_x', 'missing')]),
    ).toBeNull();
    expect(missingRequired([row('pdlc_two', 'dlcMissing')])).toBeNull();
  });
});

describe('copying the missing mods', () => {
  it('groups by the first folder that has the mod, largest group first', () => {
    expect(copyGroups(rows)).toEqual([
      { fromProfileId: 'main', modNames: ['FS25_map', 'FS25_e'] },
      { fromProfileId: 'offline', modNames: ['FS25_f'] },
    ]);
  });

  it('puts every copyable mod in exactly one group, and none of the others', () => {
    const named = copyGroups(rows).flatMap((group) => group.modNames);
    expect(new Set(named).size).toBe(named.length);
    expect(named).not.toContain('FS25_g');
    expect(named).not.toContain('pdlc_two');
    expect(named).not.toContain('FS25_a');
  });

  it('lists what no folder has, without DLC', () => {
    expect(foundNowhere(rows).map((r) => r.modName)).toEqual(['FS25_g']);
  });

  it('is empty when nothing is missing', () => {
    expect(copyGroups([row('FS25_a', 'present')])).toEqual([]);
    expect(foundNowhere([row('FS25_a', 'present')])).toEqual([]);
  });

  it('finds ids by mod name, ignoring case, and skips a name the folder lost', () => {
    const mods = [
      { id: 'FS25_One.zip', modName: 'FS25_One' },
      { id: 'FS25_Two', modName: 'FS25_Two' },
    ];
    expect(idsByName(mods, ['fs25_two', 'FS25_ONE', 'FS25_Gone'])).toEqual([
      'FS25_Two',
      'FS25_One.zip',
    ]);
  });
});
