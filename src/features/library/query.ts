import type { ModEntry, ModKind, Severity } from '../../api';

export type SortKey = 'title' | 'author' | 'version' | 'kind' | 'size' | 'modified' | 'source';
export type SortDirection = 'asc' | 'desc';
export type KindFilter = ModKind | 'all';
export type StatusFilter = 'all' | 'issues' | 'updates';

export interface LibraryQuery {
  search: string;
  kind: KindFilter;
  status: StatusFilter;
  /** Only mods with this tag (compared without regard to case); '' for every mod. */
  tag: string;
  sortKey: SortKey;
  sortDirection: SortDirection;
}

export const defaultQuery: LibraryQuery = {
  search: '',
  kind: 'all',
  status: 'all',
  tag: '',
  sortKey: 'title',
  sortDirection: 'asc',
};

const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });

const severityRank: Record<Severity, number> = { info: 1, warning: 2, error: 3 };

function compareParts(a: readonly string[], b: readonly string[]): number {
  const length = Math.max(a.length, b.length);
  for (let i = 0; i < length; i++) {
    const x = a[i] ?? '0';
    const y = b[i] ?? '0';
    if (x === y) continue;
    if (/^\d+$/.test(x) && /^\d+$/.test(y)) {
      const diff = Number(x) - Number(y);
      if (diff !== 0) return diff;
    } else {
      return x < y ? -1 : 1;
    }
  }
  return 0;
}

/** Compares dotted versions part by part: numerically where both parts are numbers. */
export function compareVersions(a: string, b: string): number {
  return compareParts(a.split('.'), b.split('.'));
}

export function worstSeverity(mod: ModEntry): Severity | null {
  let worst: Severity | null = null;
  for (const issue of mod.issues) {
    if (worst === null || severityRank[issue.severity] > severityRank[worst])
      worst = issue.severity;
  }
  return worst;
}

export function countByKind(mods: readonly ModEntry[]): Record<ModKind, number> {
  const counts: Record<ModKind, number> = { map: 0, vehicle: 0, placeable: 0, script: 0, other: 0 };
  for (const mod of mods) counts[mod.kind] += 1;
  return counts;
}

function matchesSearch(mod: ModEntry, terms: readonly string[]): boolean {
  // One lower-cased haystack per mod. A term has no whitespace, so it can never match across the
  // newline separators, which makes this equal to checking each field on its own.
  const haystack = `${mod.title}\n${mod.author}\n${mod.modName}\n${mod.fileName}`.toLowerCase();
  return terms.every((term) => haystack.includes(term));
}

function compareBy(sortKey: SortKey, versions: Map<ModEntry, string[]>) {
  switch (sortKey) {
    case 'title':
      return (a: ModEntry, b: ModEntry) => collator.compare(a.title, b.title);
    case 'author':
      return (a: ModEntry, b: ModEntry) => collator.compare(a.author, b.author);
    case 'kind':
      return (a: ModEntry, b: ModEntry) => collator.compare(a.kind, b.kind);
    case 'source':
      return (a: ModEntry, b: ModEntry) => collator.compare(a.source.type, b.source.type);
    case 'size':
      return (a: ModEntry, b: ModEntry) => a.sizeBytes - b.sizeBytes;
    case 'modified':
      return (a: ModEntry, b: ModEntry) =>
        a.modified < b.modified ? -1 : a.modified > b.modified ? 1 : 0;
    case 'version':
      return (a: ModEntry, b: ModEntry) =>
        compareParts(versions.get(a) ?? [], versions.get(b) ?? []);
  }
}

/** Filters, then sorts. Never mutates `mods`. */
export function queryMods(mods: readonly ModEntry[], query: LibraryQuery): ModEntry[] {
  const terms = query.search.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const tag = query.tag.toLowerCase();
  const result: ModEntry[] = [];
  for (const mod of mods) {
    if (query.kind !== 'all' && mod.kind !== query.kind) continue;
    if (query.status === 'issues' && mod.issues.length === 0) continue;
    if (query.status === 'updates' && mod.update.type !== 'available') continue;
    if (tag !== '' && !mod.tags.some((candidate) => candidate.toLowerCase() === tag)) continue;
    if (terms.length > 0 && !matchesSearch(mod, terms)) continue;
    result.push(mod);
  }

  const versions = new Map<ModEntry, string[]>();
  if (query.sortKey === 'version') {
    for (const mod of result) versions.set(mod, mod.version.split('.'));
  }
  const primary = compareBy(query.sortKey, versions);
  const sign = query.sortDirection === 'desc' ? -1 : 1;
  // Ties fall back to title, then id, both ascending, so the order is total in either direction.
  return result.sort((a, b) => {
    const diff = primary(a, b);
    if (diff !== 0) return diff * sign;
    const byTitle = collator.compare(a.title, b.title);
    if (byTitle !== 0) return byTitle;
    return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
  });
}
