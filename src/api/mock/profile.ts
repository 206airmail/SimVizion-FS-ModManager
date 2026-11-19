import type { Dependency, IssueKind, ModEntry, ModKind, Severity } from '..';
import {
  GAME_DESC_VERSION,
  MAX_MODIFIED,
  MB,
  KB,
  bumpVersion,
  createMod,
  makeAuthors,
  olderVersion,
  toIso,
  type GenContext,
} from './entry';
import { createRng, type Rng } from './prng';
import { junkStems, missingDependencyNames } from './words';

/** How many of each deliberately broken or unusual entry a generated profile contains. */
export interface Recipe {
  notAMod: number;
  invalidFileName: number;
  nestedArchive: number;
  /** Pairs: a zip and a folder with the same mod name. */
  twins: number;
  conflicts: number;
  mismatches: number;
  /** Mods that depend on something the profile does not contain. */
  missingDeps: number;
  /** Extra folder mods beyond the twins. */
  folders: number;
  /** Generated mods forced to have an update available. */
  forcedUpdates: number;
}

export const emptyRecipe: Recipe = {
  notAMod: 0,
  invalidFileName: 0,
  nestedArchive: 0,
  twins: 0,
  conflicts: 0,
  mismatches: 0,
  missingDeps: 0,
  folders: 0,
  forcedUpdates: 0,
};

export function scaleRecipe(recipe: Recipe, factor: number): Recipe {
  const scaled = { ...recipe };
  for (const key of Object.keys(scaled) as (keyof Recipe)[]) {
    scaled[key] = Math.round(recipe[key] * factor);
  }
  return scaled;
}

export interface ProfileSpec {
  id: string;
  seed: number;
  /** Total mods, including `copies`. */
  count: number;
  recipe: Recipe;
  /** Mods shared with another profile; already relabelled for this one. */
  copies?: ModEntry[];
  /** Lower-case mod names that generated mods must not reuse. */
  reservedNames?: Iterable<string>;
}

function kindSlots(count: number): ModKind[] {
  const maps = Math.round(count * 0.01);
  const vehicles = Math.round(count * 0.55);
  const placeables = Math.round(count * 0.2);
  const scripts = Math.round(count * 0.18);
  const others = Math.max(0, count - maps - vehicles - placeables - scripts);
  const slots: ModKind[] = [];
  const add = (kind: ModKind, n: number) => {
    for (let i = 0; i < n; i++) slots.push(kind);
  };
  add('map', maps);
  add('vehicle', vehicles);
  add('placeable', placeables);
  add('script', scripts);
  add('other', others);
  return slots.slice(0, count);
}

function issue(kind: IssueKind, severity: Severity, detail: string | null) {
  return { kind, severity, detail };
}

function rename(mod: ModEntry, stem: string, ctx: GenContext) {
  ctx.usedNames.add(stem.toLowerCase());
  mod.modName = stem;
  mod.fileName = `${stem}.zip`;
  mod.id = mod.fileName;
}

function stamp(ms: number): string {
  const compact = toIso(ms).replace(/[-:]/g, '');
  return `${compact.slice(0, 8)}-${compact.slice(9, 15)}`;
}

function conflictClone(original: ModEntry, ctx: GenContext): ModEntry {
  const { rng } = ctx;
  const copy = structuredClone(original);
  const modified = Math.min(
    Date.parse(original.modified) + rng.int(1, 8) * 3_600_000,
    MAX_MODIFIED,
  );
  const code = Array.from({ length: 7 }, () => String.fromCharCode(65 + rng.int(0, 25))).join('');
  rename(copy, `${original.modName}.sync-conflict-${stamp(modified)}-${code}`, ctx);
  copy.modified = toIso(modified);
  if (rng.chance(0.5)) copy.version = olderVersion(original.version);
  copy.source = { type: 'unknown' };
  copy.update = { type: 'unknown' };
  copy.dependencies = [];
  copy.issues = [issue('conflictCopy', 'warning', `Copy of ${original.fileName}`)];
  return copy;
}

function folderTwin(original: ModEntry, rng: Rng): ModEntry {
  const copy = structuredClone(original);
  copy.format = 'folder';
  copy.fileName = original.modName;
  copy.id = original.modName;
  copy.sizeBytes = Math.round(original.sizeBytes * 1.03);
  copy.modified = toIso(
    Math.min(Date.parse(original.modified) + rng.int(1, 40) * 86_400_000, MAX_MODIFIED),
  );
  const detail = `A zip and a folder are both named ${original.modName}`;
  copy.issues = [issue('zipAndFolderTwin', 'warning', detail)];
  original.issues = [issue('zipAndFolderTwin', 'warning', detail)];
  return copy;
}

function toNotAMod(mod: ModEntry, ctx: GenContext) {
  const { rng } = ctx;
  let stem = `${rng.pick(junkStems)}_${rng.int(1, 99)}`;
  while (ctx.usedNames.has(stem.toLowerCase())) stem = `${rng.pick(junkStems)}_${rng.int(1, 99)}`;
  rename(mod, stem, ctx);
  Object.assign(mod, {
    title: mod.fileName,
    author: '',
    version: '',
    descVersion: null,
    kind: 'other',
    description: '',
    icon: null,
    sizeBytes: rng.int(40 * KB, 180 * MB),
    multiplayer: false,
    source: { type: 'unknown' },
    update: { type: 'unknown' },
    issues: [issue('notAMod', 'error', 'No modDesc.xml found in the archive')],
  } satisfies Partial<ModEntry>);
}

function toInvalidName(mod: ModEntry, ctx: GenContext, variant: number) {
  const choices: [string, string][] = [
    [mod.modName.replace('_', ' '), 'Contains a space'],
    [`${ctx.rng.int(2, 9)}${mod.modName}`, 'Starts with a digit'],
    [`${mod.modName} (2)`, 'Contains a space and parentheses'],
    [`${mod.modName.replace('_', '-')}-v2`, 'Contains a hyphen'],
  ];
  const choice = choices[variant % choices.length];
  if (!choice) return;
  rename(mod, choice[0], ctx);
  mod.issues.push(issue('invalidFileName', 'error', choice[1]));
}

/** Applies the recipe's special entries. Everything touched ends up in `locked`. */
function applySpecials(mods: ModEntry[], recipe: Recipe, ctx: GenContext, locked: Set<ModEntry>) {
  const { rng } = ctx;
  const eligible = (extra: (m: ModEntry) => boolean) =>
    mods.filter((m) => !locked.has(m) && m.format === 'zip' && m.issues.length === 0 && extra(m));
  const take = (extra: (m: ModEntry) => boolean): ModEntry | null => {
    const pool = eligible(extra);
    if (pool.length === 0) return null;
    const mod = rng.pick(pool);
    locked.add(mod);
    return mod;
  };
  const notMap = (m: ModEntry) => m.kind !== 'map';
  const notMapOrOther = (m: ModEntry) => m.kind !== 'map' && m.kind !== 'other';

  for (let i = 0; i < recipe.notAMod; i++) {
    const mod = take((m) => m.kind === 'other') ?? take(notMap);
    if (mod) toNotAMod(mod, ctx);
  }

  // A zip and a folder with one name, and sync-conflict copies, both replace a same-kind mod
  // so the profile keeps its size.
  const replaceWithClone = (build: (original: ModEntry) => ModEntry, count: number) => {
    for (let i = 0; i < count; i++) {
      const original = take(notMapOrOther);
      const victim = original ? take((m) => m.kind === original.kind) : null;
      if (!original || !victim) continue;
      const clone = build(original);
      mods[mods.indexOf(victim)] = clone;
      locked.add(clone);
    }
  };
  replaceWithClone((original) => folderTwin(original, rng), recipe.twins);
  replaceWithClone((original) => conflictClone(original, ctx), recipe.conflicts);

  for (let i = 0; i < recipe.invalidFileName; i++) {
    const mod = take(notMap);
    if (mod) toInvalidName(mod, ctx, i);
  }
  for (let i = 0; i < recipe.nestedArchive; i++) {
    const mod = take(notMapOrOther);
    if (mod)
      mod.issues.push(
        issue('nestedArchive', 'warning', `Contains ${mod.fileName} inside the archive`),
      );
  }
  for (let i = 0; i < recipe.mismatches; i++) {
    const mod = take(notMap);
    if (!mod) continue;
    mod.descVersion = rng.int(90, 95);
    mod.issues.push(
      issue(
        'descVersionMismatch',
        'info',
        `descVersion ${mod.descVersion}, game reported ${GAME_DESC_VERSION}`,
      ),
    );
  }
  for (let i = 0; i < recipe.folders; i++) {
    const mod = take(notMapOrOther);
    if (!mod) continue;
    mod.format = 'folder';
    mod.fileName = mod.modName;
    mod.id = mod.modName;
  }
}

function addDependencies(mods: ModEntry[], recipe: Recipe, rng: Rng, locked: Set<ModEntry>) {
  const names = new Set(mods.map((m) => m.modName));
  const valid = mods.filter((m) => !m.issues.some((i) => i.kind === 'notAMod'));
  const libraries = valid.filter((m) => m.kind === 'script' || m.kind === 'other');
  const pool = libraries.length >= 8 ? libraries : valid;
  const targets = (self: ModEntry, count: number): Dependency[] =>
    rng
      .sample(pool, count + 1)
      .filter((t) => t !== self)
      .slice(0, count)
      .map((t) => ({ modName: t.modName, satisfied: true }));

  let generated = 0;
  const missingNames = (count: number): string[] => {
    const free = missingDependencyNames.filter((n) => !names.has(n));
    const chosen: string[] = rng.sample(free, count);
    while (chosen.length < count) chosen.push(`FS25_MissingLibrary${++generated}`);
    return chosen;
  };

  for (let i = 0; i < recipe.missingDeps; i++) {
    const healthy = mods.filter(
      (m) => !locked.has(m) && m.format === 'zip' && m.issues.length === 0,
    );
    const maps = healthy.filter((m) => m.kind === 'map');
    const others = healthy.filter((m) => m.kind !== 'map');
    // The first one goes to a map when there is one: maps are the mods with the most dependencies.
    const candidates = i === 0 && maps.length > 0 ? maps : others;
    const mod = candidates.length > 0 ? rng.pick(candidates) : null;
    if (!mod) continue;
    locked.add(mod);
    const missing = missingNames(i % 3 === 2 ? 2 : 1);
    const present = targets(mod, mod.kind === 'map' ? rng.int(3, 7) : rng.int(1, 2));
    mod.dependencies = [...present, ...missing.map((modName) => ({ modName, satisfied: false }))];
    mod.issues.push(
      issue(
        'missingDependency',
        'warning',
        `Requires ${missing.join(', ')}, which is not in this profile`,
      ),
    );
  }

  for (const mod of mods) {
    if (locked.has(mod) || mod.dependencies.length > 0) continue;
    if (mod.kind === 'map') mod.dependencies = targets(mod, rng.int(3, 10));
    else if (rng.chance(0.03)) mod.dependencies = targets(mod, rng.int(1, 3));
  }
}

function forceUpdates(mods: ModEntry[], count: number, rng: Rng, locked: Set<ModEntry>) {
  const wanted = mods.filter((m) => m.update.type !== 'available');
  const free = wanted.filter((m) => !locked.has(m));
  const fallback = wanted.filter(
    (m) =>
      m.format === 'zip' &&
      !m.issues.some((i) => i.kind === 'notAMod' || i.kind === 'conflictCopy'),
  );
  const pool = rng.sample(free.length >= count ? free : fallback, count);
  for (const mod of pool) {
    if (mod.source.type !== 'modHub' && mod.source.type !== 'gitHub') {
      mod.source = { type: 'modHub', id: String(rng.int(100000, 349999)) };
    }
    mod.update = { type: 'available', version: bumpVersion(mod.version, rng) };
  }
}

/** Builds one profile's mods, sorted by file name like a directory listing. */
export function buildProfileMods(spec: ProfileSpec): ModEntry[] {
  const rng = createRng(spec.seed);
  const copies = spec.copies ?? [];
  const ctx: GenContext = {
    rng,
    profileId: spec.id,
    usedNames: new Set([...(spec.reservedNames ?? [])].map((n) => n.toLowerCase())),
    authors: makeAuthors(rng, Math.max(8, Math.round(Math.sqrt(spec.count) * 4))),
    mapsMade: 0,
  };
  for (const copy of copies) ctx.usedNames.add(copy.modName.toLowerCase());

  const mods = kindSlots(spec.count - copies.length).map((kind) => createMod(ctx, kind));
  const locked = new Set<ModEntry>(copies);
  applySpecials(mods, spec.recipe, ctx, locked);
  mods.push(...copies);
  addDependencies(mods, spec.recipe, rng, locked);
  forceUpdates(mods, spec.recipe.forcedUpdates, rng, locked);

  return mods.sort((a, b) => {
    const x = a.fileName.toLowerCase();
    const y = b.fileName.toLowerCase();
    return x < y ? -1 : x > y ? 1 : 0;
  });
}

/**
 * Picks `count` healthy mods from `source` to appear in another profile too. `older` of them get
 * an older version there, and so show an update.
 */
export function pickCopies(
  source: readonly ModEntry[],
  rng: Rng,
  count: number,
  older: number,
  profileId: string,
): ModEntry[] {
  const healthy = source.filter(
    (m) =>
      m.format === 'zip' &&
      m.kind !== 'map' &&
      m.issues.length === 0 &&
      m.dependencies.length === 0 &&
      m.sizeBytes < 250 * MB,
  );
  const tracked = (m: ModEntry) =>
    (m.source.type === 'modHub' || m.source.type === 'gitHub') &&
    m.update.type === 'upToDate' &&
    !/^\d+\.0\.0\.0$/.test(m.version);
  const olderOnes = rng.sample(healthy.filter(tracked), older);
  const rest = rng.sample(
    healthy.filter((m) => !olderOnes.includes(m)),
    count - olderOnes.length,
  );
  return [...olderOnes, ...rest].map((original) => {
    const copy = structuredClone(original);
    copy.profileId = profileId;
    copy.alsoIn = [];
    if (olderOnes.includes(original)) {
      copy.version = olderVersion(original.version);
      copy.update = { type: 'available', version: original.version };
      copy.modified = toIso(
        Math.max(
          Date.parse(original.modified) - rng.int(15, 90) * 86_400_000,
          Date.UTC(2024, 10, 12),
        ),
      );
    }
    return copy;
  });
}
