// Pure rules for the Health screen: what the chips count, what the filters keep, and the order.

import type { HealthRow, IssueKind, Severity } from '../../api';

/** Every issue kind, in the order the chips list them (worst first). */
export const kindOrder: readonly IssueKind[] = [
  'notAMod',
  'invalidFileName',
  'missingDependency',
  'zipAndFolderTwin',
  'conflictCopy',
  'nestedArchive',
  'descVersionMismatch',
];

export type KindFilter = IssueKind | 'all';
/** A folder's profile id, or 'all'. */
export type FolderFilter = string;

export interface HealthFilter {
  kind: KindFilter;
  folder: FolderFilter;
}

export const defaultFilter: HealthFilter = { kind: 'all', folder: 'all' };

const severityRank: Record<Severity, number> = { error: 3, warning: 2, info: 1 };

export function worstSeverity(row: HealthRow): Severity {
  let worst: Severity = 'info';
  for (const issue of row.issues) {
    if (severityRank[issue.severity] > severityRank[worst]) worst = issue.severity;
  }
  return worst;
}

/** Rows kept by the folder filter only (what the chips count, so they follow the folder). */
export function inFolder(rows: readonly HealthRow[], folder: FolderFilter): HealthRow[] {
  return folder === 'all' ? [...rows] : rows.filter((row) => row.profileId === folder);
}

/**
 * How many mods have each kind of issue, among the mods in `rows` (a mod with two issues of
 * different kinds counts once in each). `all` is the number of mods with any issue: the same
 * number as the rail's "With issues" total when `rows` is the whole report.
 */
export function countKinds(rows: readonly HealthRow[]): Record<KindFilter, number> {
  const counts = Object.fromEntries(kindOrder.map((kind) => [kind, 0])) as Record<
    IssueKind,
    number
  >;
  for (const row of rows) {
    for (const kind of new Set(row.issues.map((issue) => issue.kind))) counts[kind] += 1;
  }
  return { ...counts, all: rows.length };
}

/** The kinds that occur at all, in chip order. */
export function presentKinds(rows: readonly HealthRow[]): IssueKind[] {
  const counts = countKinds(rows);
  return kindOrder.filter((kind) => counts[kind] > 0);
}

const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });

/** Worst issue first, then by title, then by folder: a stable order for the table. */
export function compareRows(a: HealthRow, b: HealthRow): number {
  return (
    severityRank[worstSeverity(b)] - severityRank[worstSeverity(a)] ||
    collator.compare(a.title, b.title) ||
    collator.compare(a.profileName, b.profileName)
  );
}

export function filterRows(rows: readonly HealthRow[], filter: HealthFilter): HealthRow[] {
  return inFolder(rows, filter.folder)
    .filter(
      (row) => filter.kind === 'all' || row.issues.some((issue) => issue.kind === filter.kind),
    )
    .sort(compareRows);
}

export interface FolderOption {
  profileId: string;
  name: string;
  count: number;
}

/** The folders that have rows, in the order they first appear in the report. */
export function folderOptions(rows: readonly HealthRow[]): FolderOption[] {
  const found = new Map<string, FolderOption>();
  for (const row of rows) {
    const entry = found.get(row.profileId);
    if (entry) entry.count += 1;
    else found.set(row.profileId, { profileId: row.profileId, name: row.profileName, count: 1 });
  }
  return [...found.values()];
}

/**
 * A filter that points at nothing any more (the last mod of that kind was fixed, or the folder
 * has no issues left after a re-read) falls back to showing everything instead of an empty table.
 */
export function effectiveFilter(rows: readonly HealthRow[], filter: HealthFilter): HealthFilter {
  const folder =
    filter.folder === 'all' || rows.some((row) => row.profileId === filter.folder)
      ? filter.folder
      : 'all';
  const kind =
    filter.kind === 'all' || countKinds(inFolder(rows, folder))[filter.kind] > 0
      ? filter.kind
      : 'all';
  return { kind, folder };
}
