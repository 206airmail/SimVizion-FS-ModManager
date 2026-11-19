import { describe, expect, it } from 'vitest';
import type { CompareRow, CompareSide, CompareState } from '../../api';
import { countRows, filterRows, matchesFilter, sourceModIds } from './rows';

const side = (modId: string, version = '1.0.0.0'): CompareSide => ({
  modId,
  summary: { fileName: `${modId}.zip`, version, sizeBytes: 1000, modified: '2026-01-01T00:00:00Z' },
});

const row = (modName: string, state: CompareState, title = modName): CompareRow => ({
  modName,
  title,
  state,
  left: state === 'onlyRight' ? null : side(`L_${modName}`),
  right: state === 'onlyLeft' ? null : side(`R_${modName}`),
});

const rows: CompareRow[] = [
  row('FS25_a', 'onlyLeft', 'Alpha Baler'),
  row('FS25_b', 'onlyRight', 'Beta Plough'),
  row('FS25_c', 'versionDiffers', 'Gamma Loader'),
  row('FS25_d', 'contentDiffers', 'Delta Seeder'),
  row('FS25_e', 'identical', 'Epsilon Trailer'),
  row('FS25_f', 'identical', 'Zeta Header'),
];

describe('compare filters', () => {
  it('shows every difference by default, and the others by their state', () => {
    const names = (filter: Parameters<typeof matchesFilter>[1]) =>
      rows.filter((r) => matchesFilter(r, filter)).map((r) => r.modName);
    expect(names('differences')).toEqual(['FS25_a', 'FS25_b', 'FS25_c', 'FS25_d']);
    expect(names('onlyLeft')).toEqual(['FS25_a']);
    expect(names('onlyRight')).toEqual(['FS25_b']);
    expect(names('different')).toEqual(['FS25_c', 'FS25_d']);
    expect(names('all')).toHaveLength(6);
  });

  it('counts what each filter would show', () => {
    expect(countRows(rows)).toEqual({
      differences: 4,
      onlyLeft: 1,
      onlyRight: 1,
      different: 2,
      all: 6,
    });
    expect(countRows([])).toEqual({
      differences: 0,
      onlyLeft: 0,
      onlyRight: 0,
      different: 0,
      all: 0,
    });
  });

  it('keeps the counts in step with filterRows', () => {
    const counts = countRows(rows);
    for (const filter of ['differences', 'onlyLeft', 'onlyRight', 'different', 'all'] as const) {
      expect(filterRows(rows, filter, '')).toHaveLength(counts[filter]);
    }
  });

  it('searches title, mod name and file names with every word, ignoring case', () => {
    expect(filterRows(rows, 'all', 'baler').map((r) => r.modName)).toEqual(['FS25_a']);
    expect(filterRows(rows, 'all', 'FS25_C').map((r) => r.modName)).toEqual(['FS25_c']);
    expect(filterRows(rows, 'all', 'l_fs25_d').map((r) => r.modName)).toEqual(['FS25_d']);
    expect(filterRows(rows, 'all', 'epsilon trailer')).toHaveLength(1);
    expect(filterRows(rows, 'all', 'epsilon baler')).toHaveLength(0);
    expect(filterRows(rows, 'all', '   ')).toHaveLength(6);
  });

  it('applies the filter and the search together', () => {
    expect(filterRows(rows, 'differences', 'e')).toHaveLength(4);
    expect(filterRows(rows, 'differences', 'epsilon')).toHaveLength(0);
  });
});

describe('sourceModIds', () => {
  it('sends the left copies to the right, and the right copies to the left', () => {
    expect(sourceModIds(rows, 'toRight')).toEqual([
      'L_FS25_a',
      'L_FS25_c',
      'L_FS25_d',
      'L_FS25_e',
      'L_FS25_f',
    ]);
    expect(sourceModIds(rows, 'toLeft')).toEqual([
      'R_FS25_b',
      'R_FS25_c',
      'R_FS25_d',
      'R_FS25_e',
      'R_FS25_f',
    ]);
  });

  it('is empty when no selected row has a copy on the source side', () => {
    const onlyRight = rows.filter((r) => r.state === 'onlyRight');
    expect(sourceModIds(onlyRight, 'toRight')).toEqual([]);
    expect(sourceModIds([], 'toLeft')).toEqual([]);
  });
});
