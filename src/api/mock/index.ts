import type { Api, GameStatus, ModEntry, Profile, ScanProgress, SwitchResult } from '..';
import { ApiFailure } from '../../lib/errors';
import { GAME_DESC_VERSION } from './entry';
import { createRng } from './prng';
import { buildProfileMods, emptyRecipe, pickCopies, scaleRecipe, type Recipe } from './profile';

export interface MockOptions {
  /** Adds a fourth profile "Stress Test" with exactly 5,000 mods. Default false. */
  stress?: boolean;
  /** Artificial latency per call in ms. Default 250. */
  delayMs?: number;
  /** When true, listMods rejects with a folderUnreadable ApiFailure. Default false. */
  failMods?: boolean;
}

/** How many progress events a mock listMods emits across its delay. */
const PROGRESS_STEPS = 5;

const MODS_ROOT = 'C:\\Users\\Player\\Documents\\My Games\\FarmingSimulator2025';

const MAIN_RECIPE: Recipe = {
  notAMod: 3,
  invalidFileName: 5,
  nestedArchive: 5,
  twins: 2,
  conflicts: 4,
  mismatches: 9,
  missingDeps: 4,
  folders: 4,
  forcedUpdates: 0,
};

interface Dataset {
  profiles: Profile[];
  mods: Map<string, ModEntry[]>;
}

function summarise(
  id: string,
  name: string,
  path: string,
  flags: Pick<Profile, 'isActive' | 'isGameDefault'>,
  mods: readonly ModEntry[],
): Profile {
  return {
    id,
    name,
    path,
    ...flags,
    modCount: mods.length,
    totalBytes: mods.reduce((sum, m) => sum + m.sizeBytes, 0),
    issueCount: mods.filter((m) => m.issues.length > 0).length,
    updateCount: mods.filter((m) => m.update.type === 'available').length,
  };
}

function buildDataset(stress: boolean): Dataset {
  const main = buildProfileMods({ id: 'main', seed: 0x25a001, count: 590, recipe: MAIN_RECIPE });
  const names = new Set(main.map((m) => m.modName));

  const testingCopies = pickCopies(main, createRng(0x25a002), 10, 2, 'testing');
  const testing = buildProfileMods({
    id: 'testing',
    seed: 0x25a003,
    count: 16,
    copies: testingCopies,
    reservedNames: names,
    recipe: { ...emptyRecipe, conflicts: 1, mismatches: 1, missingDeps: 1, forcedUpdates: 1 },
  });
  for (const mod of testing) names.add(mod.modName);

  const offlineCopies = pickCopies(main, createRng(0x25a004), 4, 1, 'offline');
  const offline = buildProfileMods({
    id: 'offline',
    seed: 0x25a005,
    count: 6,
    copies: offlineCopies,
    reservedNames: names,
    recipe: { ...emptyRecipe, mismatches: 1, forcedUpdates: 1 },
  });
  for (const mod of offline) names.add(mod.modName);

  const lists: [string, string, string, Pick<Profile, 'isActive' | 'isGameDefault'>, ModEntry[]][] =
    [
      ['main', 'Main Farm', `${MODS_ROOT}\\mods`, { isActive: false, isGameDefault: true }, main],
      [
        'testing',
        'Testing',
        `${MODS_ROOT}\\$testingMods`,
        { isActive: true, isGameDefault: false },
        testing,
      ],
      [
        'offline',
        'Offline Testing',
        `${MODS_ROOT}\\$offlineTestingMods`,
        { isActive: false, isGameDefault: false },
        offline,
      ],
    ];
  if (stress) {
    const stressMods = buildProfileMods({
      id: 'stress',
      seed: 0x25a006,
      count: 5000,
      recipe: scaleRecipe(MAIN_RECIPE, 5000 / 590),
      reservedNames: names,
    });
    lists.push([
      'stress',
      'Stress Test',
      `${MODS_ROOT}\\$stressMods`,
      { isActive: false, isGameDefault: false },
      stressMods,
    ]);
  }

  // A mod's identity is its name, so alsoIn is derived from the finished lists and is symmetric.
  const profilesByName = new Map<string, Set<string>>();
  for (const [id, , , , mods] of lists) {
    for (const mod of mods) {
      const ids = profilesByName.get(mod.modName) ?? new Set<string>();
      ids.add(id);
      profilesByName.set(mod.modName, ids);
    }
  }
  const order = lists.map(([id]) => id);
  for (const [id, , , , mods] of lists) {
    for (const mod of mods) {
      const ids = profilesByName.get(mod.modName);
      mod.alsoIn = order.filter((other) => other !== id && ids?.has(other));
    }
  }

  return {
    profiles: lists.map(([id, name, path, flags, mods]) => summarise(id, name, path, flags, mods)),
    mods: new Map(lists.map(([id, , , , mods]) => [id, mods])),
  };
}

const datasets = new Map<boolean, Dataset>();

function dataset(stress: boolean): Dataset {
  let cached = datasets.get(stress);
  if (!cached) {
    cached = buildDataset(stress);
    datasets.set(stress, cached);
  }
  return cached;
}

/** A deterministic in-memory Api. The same options always return the same data. */
export function createMockApi(options: MockOptions = {}): Api {
  const { stress = false, delayMs = 250, failMods = false } = options;
  const wait = (): Promise<void> =>
    delayMs > 0 ? new Promise((resolve) => setTimeout(resolve, delayMs)) : Promise.resolve();

  return {
    sampleData: true,
    async addProfile(): Promise<Profile | null> {
      await wait();
      return null;
    },
    async openProfileFolder(): Promise<void> {},
    async showModInFolder(): Promise<void> {},
    // WS-C: switching, undo and restore in memory.
    async setActiveProfile(): Promise<SwitchResult> {
      throw new ApiFailure('writesDisabled');
    },
    async undoSwitch(): Promise<SwitchResult> {
      throw new ApiFailure('nothingToUndo');
    },
    async latestSettingsBackup(): Promise<string | null> {
      return null;
    },
    async restoreSettingsBackup(): Promise<void> {
      throw new ApiFailure('noBackup');
    },
    async getGameStatus(): Promise<GameStatus> {
      await wait();
      const testing = dataset(stress).profiles.find((p) => p.id === 'testing');
      return {
        gameVersion: '1.25.1.0',
        gameRunning: false,
        serverRunning: false,
        settingsReadable: true,
        overrideActive: true,
        effectiveModsDir: testing?.path ?? null,
        loggedDescVersion: GAME_DESC_VERSION,
      };
    },
    async listProfiles(): Promise<Profile[]> {
      await wait();
      return dataset(stress).profiles.map((p) => ({ ...p }));
    },
    async listMods(
      profileId: string,
      onProgress?: (progress: ScanProgress) => void,
    ): Promise<ModEntry[]> {
      if (failMods) {
        await wait();
        throw new ApiFailure('folderUnreadable');
      }
      const mods = dataset(stress).mods.get(profileId)?.slice() ?? [];
      // Spread the delay over a few steps, each reporting how much has been "read" so far.
      for (let step = 1; step <= PROGRESS_STEPS; step++) {
        await new Promise((resolve) => setTimeout(resolve, delayMs / PROGRESS_STEPS));
        onProgress?.({
          profileId,
          done: Math.round((mods.length * step) / PROGRESS_STEPS),
          total: mods.length,
        });
      }
      return mods;
    },
  };
}
