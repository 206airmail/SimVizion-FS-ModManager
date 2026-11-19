import type { ModEntry, ModKind, ModSource, UpdateState } from '..';
import { placeholderIcon } from './icons';
import { hashString, type Rng } from './prng';
import {
  authorFirst,
  authorSecond,
  authorSuffixes,
  brands,
  descriptions,
  mapPrefixes,
  mapSuffixes,
  modelPrefixes,
  otherNouns,
  otherPrefixes,
  placeableAdjectives,
  placeableNouns,
  placeableSuffixes,
  scriptNouns,
  scriptPrefixes,
  scriptSuffixes,
  vehicleEras,
  vehicleTypes,
} from './words';

export const KB = 1024;
export const MB = 1024 * KB;
export const GB = 1024 * MB;
/** The ModDesc version the mock game reports. */
export const GAME_DESC_VERSION = 106;

const MIN_MODIFIED = Date.UTC(2024, 10, 12);
export const MAX_MODIFIED = Date.UTC(2026, 10, 15);

export interface GenContext {
  rng: Rng;
  profileId: string;
  /** Lower-case mod names already taken, here or in a profile generated earlier. */
  usedNames: Set<string>;
  authors: string[];
  mapsMade: number;
}

export function toIso(ms: number): string {
  return new Date(Math.floor(ms / 1000) * 1000).toISOString().replace('.000Z', 'Z');
}

export function makeAuthors(rng: Rng, count: number): string[] {
  const authors = new Set<string>();
  while (authors.size < count) {
    authors.add(`${rng.pick(authorFirst)}${rng.pick(authorSecond)}${rng.pick(authorSuffixes)}`);
  }
  return [...authors];
}

export function makeVersion(rng: Rng): string {
  const r = rng.next();
  const major = r < 0.72 ? 1 : r < 0.92 ? 2 : r < 0.97 ? 0 : 3;
  const minor = rng.chance(0.45) ? 0 : rng.int(1, 6);
  const patch = rng.chance(0.8) ? 0 : rng.int(1, 3);
  const build = rng.chance(0.5) ? 0 : rng.int(1, 5);
  return `${major}.${minor}.${patch}.${build}`;
}

/** A strictly higher version: bumps one part and resets the parts after it. */
export function bumpVersion(version: string, rng: Rng): string {
  const parts = version.split('.').map(Number);
  const r = rng.next();
  const index = r < 0.5 ? 3 : r < 0.85 ? 1 : 0;
  const bumped = parts.map((part, i) => (i < index ? part : i === index ? part + 1 : 0));
  return bumped.join('.');
}

/** A strictly lower version, or the same one when there is nothing lower. */
export function olderVersion(version: string): string {
  const parts = version.split('.').map(Number);
  for (let i = parts.length - 1; i >= 0; i--) {
    const part = parts[i];
    if (part !== undefined && part > 0) {
      parts[i] = part - 1;
      break;
    }
  }
  return parts.join('.');
}

function lognormal(rng: Rng, median: number, sigma: number, min: number, max: number): number {
  return Math.round(Math.min(max, Math.max(min, median * Math.exp(sigma * rng.gauss()))));
}

function pascal(title: string): string {
  return title
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join('');
}

function snake(title: string): string {
  return title
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean)
    .join('_');
}

function makeStem(rng: Rng, title: string): string {
  const name = rng.chance(0.8) ? pascal(title) : snake(title);
  if (!rng.chance(0.06)) return `FS25_${name}`;
  switch (rng.int(0, 3)) {
    case 0:
      return `FSM_${name}_FS25`;
    case 1:
      return `${name}_FS25`;
    case 2:
      return `fs25_${name}`;
    default:
      return `SVX_${name}`;
  }
}

function makeTitle(rng: Rng, kind: ModKind): { title: string; pack: boolean } {
  switch (kind) {
    case 'vehicle': {
      const brand = rng.pick(brands);
      const type = rng.pick(vehicleTypes);
      const sep = rng.pick(['-', '', ' ']);
      const model = `${rng.pick(modelPrefixes)}${sep}${rng.int(100, 9999)}`;
      const r = rng.next();
      if (r < 0.05) return { title: `${rng.pick(vehicleEras)} ${type} Pack`, pack: true };
      if (r < 0.42) return { title: `${brand} ${model}`, pack: false };
      if (r < 0.8) return { title: `${brand} ${model} ${type}`, pack: false };
      return { title: `${brand} ${type} ${model}`, pack: false };
    }
    case 'placeable': {
      const base = `${rng.pick(placeableAdjectives)} ${rng.pick(placeableNouns)}`;
      return {
        title: rng.chance(0.3) ? `${base} ${rng.pick(placeableSuffixes)}` : base,
        pack: false,
      };
    }
    case 'script': {
      const base = `${rng.pick(scriptPrefixes)} ${rng.pick(scriptNouns)}`;
      return {
        title: rng.chance(0.25) ? `${base} ${rng.pick(scriptSuffixes)}` : base,
        pack: false,
      };
    }
    case 'map':
      return {
        title: `${rng.pick(mapPrefixes)} ${rng.pick(mapSuffixes)}${rng.chance(0.2) ? ' Map' : ''}`,
        pack: false,
      };
    case 'other':
      return { title: `${rng.pick(otherPrefixes)} ${rng.pick(otherNouns)}`, pack: false };
  }
}

function makeSize(ctx: GenContext, kind: ModKind, pack: boolean): number {
  const { rng } = ctx;
  switch (kind) {
    case 'vehicle':
      return pack
        ? lognormal(rng, 250 * MB, 0.6, 80 * MB, 900 * MB)
        : lognormal(rng, 17 * MB, 1.0, 600 * KB, 600 * MB);
    case 'placeable':
      return lognormal(rng, 11 * MB, 1.0, 120 * KB, 350 * MB);
    case 'script':
      return lognormal(rng, 140 * KB, 1.2, 20 * KB, 12 * MB);
    case 'map':
      ctx.mapsMade += 1;
      return ctx.mapsMade === 1
        ? Math.round(3.9 * GB)
        : lognormal(rng, 0.8 * GB, 0.4, 400 * MB, 3 * GB);
    case 'other':
      return lognormal(rng, 14 * MB, 1.2, 100 * KB, 400 * MB);
  }
}

function makeDescription(rng: Rng, kind: ModKind, title: string): string {
  const pool = descriptions[kind];
  const count = rng.int(1, 3);
  return rng
    .sample(pool, count)
    .map((sentence) => sentence.replace('{title}', title))
    .join(' ');
}

function makeSource(ctx: GenContext, kind: ModKind, author: string, modName: string): ModSource {
  const { rng } = ctx;
  if (rng.chance(kind === 'script' ? 0.35 : 0.033)) {
    return { type: 'gitHub', owner: author.toLowerCase(), repo: modName };
  }
  const r = rng.next();
  if (r < 0.68) return { type: 'modHub', id: String(rng.int(100000, 349999)) };
  if (r < 0.72) return { type: 'manual' };
  return { type: 'unknown' };
}

function makeUpdate(rng: Rng, source: ModSource, version: string): UpdateState {
  if (source.type !== 'modHub' && source.type !== 'gitHub') return { type: 'unknown' };
  const r = rng.next();
  if (r < 0.1) return { type: 'available', version: bumpVersion(version, rng) };
  return r < 0.97 ? { type: 'upToDate' } : { type: 'unknown' };
}

/** One ordinary, healthy mod of the given kind, with a name unused in this context. */
export function createMod(ctx: GenContext, kind: ModKind): ModEntry {
  const { rng } = ctx;
  let { title, pack } = makeTitle(rng, kind);
  let stem = makeStem(rng, title);
  for (let attempt = 1; ctx.usedNames.has(stem.toLowerCase()); attempt++) {
    ({ title, pack } = makeTitle(rng, kind));
    if (attempt > 6) title = `${title} ${attempt - 5}`;
    stem = makeStem(rng, title);
  }
  ctx.usedNames.add(stem.toLowerCase());

  const author = ctx.authors[Math.floor(rng.next() ** 2 * ctx.authors.length)] ?? 'Unknown';
  const version = makeVersion(rng);
  const source = makeSource(ctx, kind, author, stem);
  // Never above the game's value: only a newer descVersion is an issue, and those are added
  // deliberately by the profile recipe. The discarded draw keeps the rest of the generated
  // data (sizes, dates) identical to what it was when a few mods here were newer.
  rng.next();
  const descVersion = 96 + Math.floor((GAME_DESC_VERSION - 95) * Math.sqrt(rng.next()));
  const modified = toIso(MIN_MODIFIED + (MAX_MODIFIED - MIN_MODIFIED) * rng.next() ** 0.8);
  const hasIcon = rng.chance(0.92);

  return {
    id: `${stem}.zip`,
    profileId: ctx.profileId,
    modName: stem,
    fileName: `${stem}.zip`,
    format: 'zip',
    title,
    author,
    version,
    descVersion,
    kind,
    description: makeDescription(rng, kind, title),
    icon: hasIcon ? placeholderIcon(hashString(stem), kind) : null,
    sizeBytes: makeSize(ctx, kind, pack),
    modified,
    multiplayer: rng.chance(0.98),
    dependencies: [],
    issues: [],
    source,
    update: makeUpdate(rng, source, version),
    alsoIn: [],
    tags: [],
    note: null,
  };
}
