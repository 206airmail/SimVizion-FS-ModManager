// What the Compare screen does with the rows of a comparison: the filters and their counts, the
// search, and which mod ids a Copy or Move in one direction would send. Pure, so it is tested
// without rendering.

import type { CompareRow, CompareState } from '../../api';

export type CompareFilter = 'differences' | 'onlyLeft' | 'onlyRight' | 'different' | 'all';

/** The order the filters are shown in; Differences is the default. */
export const compareFilters: readonly CompareFilter[] = [
  'differences',
  'onlyLeft',
  'onlyRight',
  'different',
  'all',
];

export const defaultCompareFilter: CompareFilter = 'differences';

/** Same name in both folders, but not the same mod: another version, or the same version re-zipped. */
const isDifferent = (state: CompareState) =>
  state === 'versionDiffers' || state === 'contentDiffers';

export function matchesFilter(row: CompareRow, filter: CompareFilter): boolean {
  switch (filter) {
    case 'all':
      return true;
    case 'differences':
      return row.state !== 'identical';
    case 'onlyLeft':
      return row.state === 'onlyLeft';
    case 'onlyRight':
      return row.state === 'onlyRight';
    case 'different':
      return isDifferent(row.state);
  }
}

/** How many rows each filter shows (before any search), for the counts on the segments. */
export function countRows(rows: readonly CompareRow[]): Record<CompareFilter, number> {
  const counts: Record<CompareFilter, number> = {
    differences: 0,
    onlyLeft: 0,
    onlyRight: 0,
    different: 0,
    all: rows.length,
  };
  for (const row of rows) {
    if (row.state !== 'identical') counts.differences += 1;
    if (row.state === 'onlyLeft') counts.onlyLeft += 1;
    if (row.state === 'onlyRight') counts.onlyRight += 1;
    if (isDifferent(row.state)) counts.different += 1;
  }
  return counts;
}

/** Rows that fit the filter and every search word (matched against title, mod name and file names). */
export function filterRows(
  rows: readonly CompareRow[],
  filter: CompareFilter,
  search: string,
): CompareRow[] {
  const terms = search.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return rows.filter((row) => {
    if (!matchesFilter(row, filter)) return false;
    if (terms.length === 0) return true;
    const haystack = [
      row.title,
      row.modName,
      row.left?.summary.fileName ?? '',
      row.right?.summary.fileName ?? '',
    ]
      .join('\n')
      .toLowerCase();
    return terms.every((term) => haystack.includes(term));
  });
}

export type Direction = 'toRight' | 'toLeft';

/**
 * The ids a transfer in this direction sends: the selected rows that have a copy on the source
 * side (left for `toRight`), as the source folder knows them. Rows without one are left out.
 */
export function sourceModIds(rows: readonly CompareRow[], direction: Direction): string[] {
  const ids: string[] = [];
  for (const row of rows) {
    const side = direction === 'toRight' ? row.left : row.right;
    if (side) ids.push(side.modId);
  }
  return ids;
}
