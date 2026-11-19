// The mock's update catalog and source links (plan M7). The generated mods already carry a
// "catalog truth" in `source` and `update`; this module turns that into what a user sees:
//   - nothing but manual links until the first successful check (the core's catalog overlay applies
//     only to cached results, and the cache is empty before the first check);
//   - after a check, the catalog's sources, update states and page links.

import type { ModEntry, ModSource, SourceLink, UpdateCheck, UpdateState } from '..';
import { ApiFailure } from '../../lib/errors';
import { compareVersions } from '../../lib/version';
import { nameKey } from './files';
import { hashString } from './prng';

/** What the catalog knows about one mod name. */
export interface CatalogEntry {
  modHub: string | null;
  gitHub: { owner: string; repo: string } | null;
  /** The source the Source column shows. */
  primary: 'modHub' | 'gitHub';
  /** The newest version the catalog has. */
  latest: string;
  /** Where an install downloads from: GitHub when the mod has a release there, else ModHub. */
  install: 'modHub' | 'gitHub';
  /** False for a GitHub source without a release asset, or a ModHub page without a download. */
  installable: boolean;
  /** Matched by file fingerprint, not by name. */
  byHash: boolean;
}

export type Catalog = ReadonlyMap<string, CatalogEntry>;

/** A stable 0 to 99 per mod name and purpose, so the data never changes between runs. */
const spread = (name: string, salt: string): number => hashString(`${salt}:${name}`) % 100;

const newest = (versions: readonly string[]): string =>
  versions.reduce((best, version) => (compareVersions(version, best) > 0 ? version : best));

/**
 * The catalog for these mods (every folder together). Mods the generator tracks on ModHub or
 * GitHub are matched; about a third of the GitHub ones are also on ModHub, a few ModHub ones are
 * also on GitHub, about one in six cannot be installed, and a few were matched by fingerprint.
 */
export function buildCatalog(all: Iterable<ModEntry>): Map<string, CatalogEntry> {
  const groups = new Map<string, ModEntry[]>();
  for (const mod of all) {
    if (mod.version === '' || mod.issues.some((issue) => issue.kind === 'notAMod')) continue;
    const key = nameKey(mod.modName);
    const group = groups.get(key);
    if (group) group.push(mod);
    else groups.set(key, [mod]);
  }

  const catalog = new Map<string, CatalogEntry>();
  for (const [key, group] of groups) {
    const tracked = group.find(
      (mod) => mod.source.type === 'modHub' || mod.source.type === 'gitHub',
    );
    if (!tracked) continue;
    let modHub = tracked.source.type === 'modHub' ? tracked.source.id : null;
    let gitHub =
      tracked.source.type === 'gitHub'
        ? { owner: tracked.source.owner, repo: tracked.source.repo }
        : null;
    const primary = gitHub ? 'gitHub' : 'modHub';
    if (gitHub && spread(key, 'both') < 35) modHub = String(100000 + (hashString(key) % 250000));
    if (modHub && !gitHub && spread(key, 'both') < 5) {
      gitHub = { owner: tracked.author.toLowerCase() || 'modder', repo: tracked.modName };
    }
    const offered = group.flatMap((mod) =>
      mod.update.type === 'available' ? [mod.update.version] : [],
    );
    catalog.set(key, {
      modHub,
      gitHub,
      primary,
      latest: newest(offered.length > 0 ? offered : group.map((mod) => mod.version)),
      install: gitHub ? 'gitHub' : 'modHub',
      installable: spread(key, 'install') >= 17,
      byHash: spread(key, 'hash') < 8,
    });
  }
  return catalog;
}

/** A page the user linked by hand. The mock keeps the URL; the contract never carries it. */
export interface ManualLink {
  url: string;
  host: string;
  /** Set for a github.com/<owner>/<repo> address. */
  github: { owner: string; repo: string } | null;
}

const MAX_LINK_LENGTH = 2000;

/** The core's rule: http or https, at most 2,000 characters, no user name or password. */
export function parseSourceLink(raw: string): ManualLink {
  const url = raw.trim();
  if (url === '' || [...url].length > MAX_LINK_LENGTH) throw new ApiFailure('invalidLink');
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new ApiFailure('invalidLink');
  }
  if (
    (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') ||
    parsed.username !== '' ||
    parsed.password !== '' ||
    parsed.hostname === ''
  ) {
    throw new ApiFailure('invalidLink');
  }
  const [owner, repo] = parsed.pathname.split('/').filter(Boolean);
  const onGitHub = parsed.hostname === 'github.com' || parsed.hostname === 'www.github.com';
  return {
    url,
    host: parsed.hostname,
    github: onGitHub && owner && repo ? { owner, repo: repo.replace(/\.git$/, '') } : null,
  };
}

/** The links the user already has: the mods the generator marks as manual (KingMods, itch.io). */
export function seedLinks(all: Iterable<ModEntry>): Map<string, ManualLink> {
  const links = new Map<string, ManualLink>();
  for (const mod of all) {
    if (mod.source.type !== 'manual') continue;
    const key = nameKey(mod.modName);
    if (links.has(key)) continue;
    const slug = mod.modName.replace(/^FS25_/i, '').toLowerCase();
    const url =
      spread(key, 'itch') < 40
        ? `https://${mod.author.toLowerCase() || 'modder'}.itch.io/${slug}`
        : `https://www.kingmods.net/en/fs25/mods/${10000 + (hashString(key) % 80000)}/${slug}`;
    links.set(key, parseSourceLink(url));
  }
  return links;
}

/** What the mock shows for a mod: the catalog overlay (when a check has run) and manual links. */
export function presentMod(
  mod: ModEntry,
  catalog: Catalog | null,
  links: ReadonlyMap<string, ManualLink>,
): ModEntry {
  const key = nameKey(mod.modName);
  const entry = catalog?.get(key) ?? null;
  const manual = links.get(key) ?? null;
  return {
    ...mod,
    source: sourceOf(entry, manual),
    update: updateOf(mod, entry),
    links: linksOf(entry, manual),
  };
}

function sourceOf(entry: CatalogEntry | null, manual: ManualLink | null): ModSource {
  if (entry) {
    if (entry.primary === 'modHub' && entry.modHub) return { type: 'modHub', id: entry.modHub };
    if (entry.gitHub) return { type: 'gitHub', owner: entry.gitHub.owner, repo: entry.gitHub.repo };
  }
  if (manual?.github)
    return { type: 'gitHub', owner: manual.github.owner, repo: manual.github.repo };
  return manual ? { type: 'manual', host: manual.host } : { type: 'unknown' };
}

export function updateOf(mod: ModEntry, entry: CatalogEntry | null): UpdateState {
  if (!entry) return { type: 'unknown' };
  return compareVersions(entry.latest, mod.version) > 0
    ? { type: 'available', version: entry.latest, installable: entry.installable }
    : { type: 'upToDate' };
}

/** ModHub, GitHub, then the manual link, as the core orders them. */
function linksOf(entry: CatalogEntry | null, manual: ManualLink | null): SourceLink[] {
  const links: SourceLink[] = [];
  if (entry?.modHub) links.push({ kind: 'modHub', label: 'ModHub' });
  if (entry?.gitHub) links.push({ kind: 'gitHub', label: 'GitHub' });
  if (manual) links.push({ kind: 'manual', label: manual.host });
  return links;
}

/** The page address the mock "opens" for a link kind, or null when the mod has none. */
export function linkUrl(
  entry: CatalogEntry | null,
  manual: ManualLink | null,
  kind: SourceLink['kind'],
): string | null {
  switch (kind) {
    case 'modHub':
      return entry?.modHub
        ? `https://www.farming-simulator.com/mod.php?mod_id=${entry.modHub}&title=fs2025`
        : null;
    case 'gitHub':
      return entry?.gitHub ? `https://github.com/${entry.gitHub.owner}/${entry.gitHub.repo}` : null;
    case 'manual':
      return manual?.url ?? null;
  }
}

/** The counts of a check that reached the catalog, over every folder's mods. */
export function summarizeCheck(
  all: Iterable<ModEntry>,
  catalog: Catalog,
  checkedAt: string,
  fromCache: boolean,
): UpdateCheck {
  const names = new Set<string>();
  const updates = new Set<string>();
  for (const mod of all) {
    const key = nameKey(mod.modName);
    names.add(key);
    if (updateOf(mod, catalog.get(key) ?? null).type === 'available') updates.add(key);
  }
  let matchedByName = 0;
  let matchedByHash = 0;
  for (const key of names) {
    const entry = catalog.get(key);
    if (!entry) continue;
    if (entry.byHash) matchedByHash += 1;
    else matchedByName += 1;
  }
  return {
    checkedAt,
    checked: names.size,
    matchedByName,
    matchedByHash,
    updates: updates.size,
    fromCache,
  };
}
