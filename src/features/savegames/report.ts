// What the Savegames screen does with a report: the filters and their counts, the required map
// that is missing, and which missing mods can be copied from which folder. Pure, so it is tested
// without rendering.

import type { SaveModRow, SaveModState } from '../../api';

export type ReportFilter = 'all' | 'missing' | 'version' | 'file' | 'present' | 'checking';

/** The order the filters are shown in. "Checking" is shown only while some row is still pending. */
export const reportFilters: readonly ReportFilter[] = [
  'all',
  'missing',
  'version',
  'file',
  'present',
  'checking',
];

/** Which filter a state counts under. DLC follows its mod counterpart (installed: present). */
export const filterOfState: Record<SaveModState, Exclude<ReportFilter, 'all'>> = {
  present: 'present',
  dlcInstalled: 'present',
  versionDiffers: 'version',
  fileDiffers: 'file',
  hashPending: 'checking',
  missing: 'missing',
  dlcMissing: 'missing',
};

export function countReport(rows: readonly SaveModRow[]): Record<ReportFilter, number> {
  const counts: Record<ReportFilter, number> = {
    all: rows.length,
    missing: 0,
    version: 0,
    file: 0,
    present: 0,
    checking: 0,
  };
  for (const row of rows) counts[filterOfState[row.state]] += 1;
  return counts;
}

export function filterReport(rows: readonly SaveModRow[], filter: ReportFilter): SaveModRow[] {
  return filter === 'all' ? [...rows] : rows.filter((row) => filterOfState[row.state] === filter);
}

/** The required mod (the map) when the folder does not have it: the game will not load the save. */
export function missingRequired(rows: readonly SaveModRow[]): SaveModRow | null {
  return rows.find((row) => row.required && row.state === 'missing') ?? null;
}

const isDlc = (row: SaveModRow) => row.state === 'dlcInstalled' || row.state === 'dlcMissing';

/** Missing mods (not DLC) that no other folder has either. */
export function foundNowhere(rows: readonly SaveModRow[]): SaveModRow[] {
  return rows.filter((row) => row.state === 'missing' && !isDlc(row) && row.foundIn.length === 0);
}

export interface CopyGroup {
  /** The folder to copy from. */
  fromProfileId: string;
  /** Mod names, in the order of the report. */
  modNames: string[];
}

/**
 * The missing mods that another folder has, grouped by the folder to copy them from: each mod
 * comes from the first folder (in the folders' own order) that has it, so a mod is in exactly one
 * group. The largest group comes first.
 */
export function copyGroups(rows: readonly SaveModRow[]): CopyGroup[] {
  const groups = new Map<string, string[]>();
  for (const row of rows) {
    const from = row.state === 'missing' ? row.foundIn[0] : undefined;
    if (from === undefined) continue;
    groups.set(from, [...(groups.get(from) ?? []), row.modName]);
  }
  return [...groups]
    .map(([fromProfileId, modNames]) => ({ fromProfileId, modNames }))
    .sort((a, b) => b.modNames.length - a.modNames.length);
}

/** The mod ids a copy needs, found by mod name (case-insensitive) in the source folder's mods. */
export function idsByName(
  mods: readonly { id: string; modName: string }[],
  modNames: readonly string[],
): string[] {
  const byName = new Map(mods.map((mod) => [mod.modName.toLowerCase(), mod.id]));
  const ids: string[] = [];
  for (const name of modNames) {
    const id = byName.get(name.toLowerCase());
    if (id !== undefined) ids.push(id);
  }
  return ids;
}
