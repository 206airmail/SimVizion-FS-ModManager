// The mock's savegames: saves built from the mock profiles' mod names, and the report of a save
// against a profile, with the two phases of the core (a version check at once; file hashes after).
// A zip's hash "matches" the save when its size is the size the save was played with and the
// save is not one of the deliberate bad-hash cases.

import type { CompareSide, ModEntry, Profile, Savegame, SavegameReport, SaveModRow } from '..';
import { olderVersion } from './entry';
import { findByName, nameKey, toSummary } from './files';
import { createRng } from './prng';

/** The DLCs installed in the mock's game folder. */
const INSTALLED_DLC = new Set(['pdlc_strawharvestpack']);

/** One mod in a save's list (what the game wrote to careerSavegame.xml). */
export interface SaveMod {
  modName: string;
  title: string;
  version: string;
  required: boolean;
  /** Size of the zip the save was played with; null for a DLC or a folder mod (no file hash). */
  fileSize: number | null;
  /** The save's hash is of a file that is no longer the one in the folders. */
  staleHash: boolean;
}

export interface SaveDef {
  savegame: Savegame;
  mods: SaveMod[];
  /** The slot cannot be read: listing shows it, reading it fails with savegameUnreadable. */
  unreadable?: boolean;
}

type Folders = ReadonlyMap<string, readonly ModEntry[]>;

const MAP_NOWHERE = 'FS25_CreekValleyMap';

/** "1.2.0.1" to "1.2.0.2". */
const newerVersion = (version: string): string =>
  version.replace(/(\d+)$/, (digits) => String(Number(digits) + 1));

const asSaveMod = (mod: ModEntry, extra: Partial<SaveMod> = {}): SaveMod => ({
  modName: mod.modName,
  title: mod.title,
  version: mod.version,
  required: false,
  fileSize: mod.format === 'zip' ? mod.sizeBytes : null,
  staleHash: false,
  ...extra,
});

const dlc = (modName: string, title: string): SaveMod => ({
  modName,
  title,
  version: '1.0.0.0',
  required: false,
  fileSize: null,
  staleHash: false,
});

/**
 * Three saves in slots 1, 4 and 7 (the other 17 are empty), and with `broken` a fourth in slot 9
 * that cannot be read.
 * - Riverside Farm (a mod map, in Main Farm only): needs everything the Main Farm has, a few
 *   mods that live only in other folders, a few that are nowhere, version changes, a stale hash.
 * - Coastal Ranch (the game's own map): a small save, no required mod.
 * - Creek Valley (a mod map that is in no folder): the required map is missing everywhere.
 */
export function buildSaves(folders: Folders, broken: boolean): SaveDef[] {
  const rng = createRng(0x25a0f1);
  const main = folders.get('main') ?? [];
  const testing = folders.get('testing') ?? [];
  const offline = folders.get('offline') ?? [];
  const plain = (mod: ModEntry) =>
    mod.kind !== 'map' &&
    mod.issues.every((issue) => issue.kind !== 'notAMod' && issue.kind !== 'conflictCopy');
  const mainNames = new Set(main.map((mod) => nameKey(mod.modName)));
  const pool = main.filter(plain);
  const mapMod = main.find(
    (mod) => mod.kind === 'map' && mod.format === 'zip' && !mod.issues.length,
  );

  // Riverside Farm
  const sampled = rng.sample(pool, 120);
  const bumped = new Set(sampled.slice(0, 6));
  const older = new Set(sampled.slice(6, 9));
  const stale = new Set(
    sampled
      .filter((mod) => mod.format === 'zip' && !bumped.has(mod) && !older.has(mod))
      .slice(0, 2),
  );
  // The mods the other folders hold copies of, as Main Farm has them: against Testing the older
  // ones differ in version and the re-zipped ones in file.
  const shared = [...testing, ...offline]
    .map((mod) => findByName(main, mod.modName))
    .filter((mod): mod is ModEntry => mod !== undefined && plain(mod))
    .map((mod) => asSaveMod(mod));
  const riverside: SaveMod[] = [
    ...(mapMod ? [asSaveMod(mapMod, { required: true })] : []),
    ...shared,
    ...sampled.map((mod) =>
      asSaveMod(mod, {
        version: bumped.has(mod)
          ? newerVersion(mod.version)
          : older.has(mod)
            ? olderVersion(mod.version)
            : mod.version,
        staleHash: stale.has(mod),
      }),
    ),
    ...testing
      .filter((mod) => !mainNames.has(nameKey(mod.modName)))
      .slice(0, 3)
      .map((m) => asSaveMod(m)),
    ...offline
      .filter((mod) => !mainNames.has(nameKey(mod.modName)))
      .slice(0, 2)
      .map((m) => asSaveMod(m)),
    {
      modName: 'FS25_retiredBaleMover',
      title: 'Bale Mover (retired)',
      version: '1.3.0.0',
      required: false,
      fileSize: 6_400_000,
      staleHash: false,
    },
    {
      modName: 'FS25_oldSilageHelper',
      title: 'Silage Helper',
      version: '0.9.2.0',
      required: false,
      fileSize: 1_200_000,
      staleHash: false,
    },
    {
      modName: 'FS25_seasonsLegacy',
      title: 'Seasons Legacy',
      version: '2.1.0.0',
      required: false,
      fileSize: 22_000_000,
      staleHash: false,
    },
    dlc('pdlc_strawHarvestPack', 'Straw Harvest Pack'),
    dlc('pdlc_antonioCarraroPack', 'Antonio Carraro Pack'),
  ];

  // Coastal Ranch
  const coastal: SaveMod[] = [
    ...rng.sample(pool, 26).map((mod) => asSaveMod(mod)),
    ...testing
      .filter((mod) => mainNames.has(nameKey(mod.modName)))
      .slice(0, 4)
      .map((m) => asSaveMod(m)),
    dlc('pdlc_strawHarvestPack', 'Straw Harvest Pack'),
  ];

  // Creek Valley: made with the Testing and Offline folders
  const creek: SaveMod[] = [
    {
      modName: MAP_NOWHERE,
      title: 'Creek Valley',
      version: '1.1.0.0',
      required: true,
      fileSize: 412_000_000,
      staleHash: false,
    },
    ...testing.filter(plain).map((mod) => asSaveMod(mod)),
    ...offline.filter(plain).map((mod) => asSaveMod(mod)),
    ...rng.sample(pool, 5).map((mod) => asSaveMod(mod)),
  ];

  const unique = (mods: SaveMod[]): SaveMod[] => {
    const seen = new Set<string>();
    return mods.filter(
      (mod) => !seen.has(nameKey(mod.modName)) && !!seen.add(nameKey(mod.modName)),
    );
  };
  const make = (
    slot: number,
    name: string,
    mapTitle: string,
    mapMod: string | null,
    saveDate: string,
    playMinutes: number,
    money: number,
    mods: SaveMod[],
  ): SaveDef => {
    const list = unique(mods);
    return {
      savegame: {
        id: `savegame${slot}`,
        slot,
        name,
        mapTitle,
        mapMod,
        saveDate,
        playMinutes,
        money,
        modCount: list.length,
      },
      mods: list,
    };
  };

  const saves = [
    make(
      1,
      'Riverside Farm',
      mapMod?.title ?? 'Riverside',
      mapMod?.modName ?? null,
      '2026-11-17',
      14_872,
      48_215_730,
      riverside,
    ),
    make(4, 'Coastal Ranch', 'Riverbend Springs', null, '2026-10-02', 3_125, 1_254_300, coastal),
    make(7, 'Creek Valley', 'Creek Valley', MAP_NOWHERE, '2026-08-23', 612, 312_000, creek),
  ];
  if (broken) {
    saves.push({
      ...make(9, 'Winter Test', 'Riverbend Springs', null, '2026-11-18', 95, 80_000, []),
      unreadable: true,
    });
  }
  return saves;
}

/** What the file cache remembers about a zip: the same file (name, size, date) is never hashed twice. */
export const hashKey = (profileId: string, mod: ModEntry): string =>
  `${profileId}|${mod.id}|${mod.sizeBytes}|${mod.modified}`;

const side = (mod: ModEntry): CompareSide => ({ modId: mod.id, summary: toSummary(mod) });

export interface ReportInput {
  save: SaveDef;
  profileId: string;
  folders: Folders;
  /** Profile ids in list order, for `foundIn`. */
  profileOrder: readonly Profile['id'][];
  /** Zips whose hash has been computed (see `hashKey`). */
  hashed: ReadonlySet<string>;
}

/** The mods of the save that would be hashed: same version, a file hash to compare. */
export function pendingMods(input: ReportInput): ModEntry[] {
  return reportRows(input).pending;
}

function reportRows({ save, profileId, folders, profileOrder, hashed }: ReportInput) {
  const mods = folders.get(profileId) ?? [];
  const pending: ModEntry[] = [];
  const rows = save.mods.map((entry): SaveModRow => {
    const base = {
      modName: entry.modName,
      title: entry.title,
      savedVersion: entry.version,
      required: entry.required,
    };
    if (entry.modName.toLowerCase().startsWith('pdlc_')) {
      const installed = INSTALLED_DLC.has(nameKey(entry.modName));
      return {
        ...base,
        state: installed ? 'dlcInstalled' : 'dlcMissing',
        installed: null,
        foundIn: [],
      };
    }
    const found = findByName(mods, entry.modName);
    if (!found) {
      const foundIn = profileOrder.filter(
        (id) => id !== profileId && findByName(folders.get(id) ?? [], entry.modName),
      );
      return { ...base, state: 'missing', installed: null, foundIn };
    }
    const installed = side(found);
    if (found.version !== entry.version) {
      return { ...base, state: 'versionDiffers', installed, foundIn: [] };
    }
    if (entry.fileSize === null || found.format !== 'zip') {
      return { ...base, state: 'present', installed, foundIn: [] };
    }
    if (!hashed.has(hashKey(profileId, found))) {
      pending.push(found);
      return { ...base, state: 'hashPending', installed, foundIn: [] };
    }
    const matches = found.sizeBytes === entry.fileSize && !entry.staleHash;
    return { ...base, state: matches ? 'present' : 'fileDiffers', installed, foundIn: [] };
  });
  return { rows, pending };
}

/** Required first, then by lower-cased mod name; extras are the profile's mods the save never lists. */
export function buildReport(input: ReportInput): SavegameReport {
  const { save, profileId, folders } = input;
  const { rows } = reportRows(input);
  rows.sort(
    (a, b) =>
      Number(b.required) - Number(a.required) ||
      (nameKey(a.modName) < nameKey(b.modName)
        ? -1
        : nameKey(a.modName) > nameKey(b.modName)
          ? 1
          : 0),
  );
  const used = new Set(save.mods.map((mod) => nameKey(mod.modName)));
  const extras = (folders.get(profileId) ?? [])
    .filter((mod) => !used.has(nameKey(mod.modName)))
    .map((mod) => mod.id);
  return { savegameId: save.savegame.id, profileId, rows, extras };
}
