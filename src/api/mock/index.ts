import type {
  Api,
  GameStatus,
  ModEntry,
  OpResult,
  Profile,
  ScanProgress,
  SwitchResult,
  TransferPlan,
} from '..';
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
  /** The game is running: the status says so and switching is refused. Default false. */
  gameRunning?: boolean;
  /** The dedicated server is running: the status says so and switching is refused. Default false. */
  serverRunning?: boolean;
  /** gameSettings.xml is unreadable until restoreSettingsBackup runs; a backup exists. Default false. */
  brokenSettings?: boolean;
  /** setActiveProfile rejects with a writeFailed ApiFailure. Default false. */
  writeFails?: boolean;
}

/** The mock's newest settings backup, offered while `brokenSettings` is on. */
export const MOCK_BACKUP_AT = '2026-11-17T18:42:00Z';

/** The profile that is active at start, and the one whose folder is the game's default. */
const INITIAL_ACTIVE_ID = 'testing';
const DEFAULT_PROFILE_ID = 'main';

interface MockOp {
  id: string;
  kind: 'switch' | 'undo';
  fromId: string | null;
  toId: string | null;
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
  const {
    stress = false,
    delayMs = 250,
    failMods = false,
    gameRunning = false,
    serverRunning = false,
    brokenSettings = false,
    writeFails = false,
  } = options;
  const wait = (): Promise<void> =>
    delayMs > 0 ? new Promise((resolve) => setTimeout(resolve, delayMs)) : Promise.resolve();

  // Switching state lives in this instance; the shared dataset is never mutated.
  let activeId: string | null = INITIAL_ACTIVE_ID;
  let settingsBroken = brokenSettings;
  const ops: MockOp[] = [];
  const pathOf = (id: string | null) =>
    dataset(stress).profiles.find((p) => p.id === id)?.path ?? null;

  /** Guards shared by a switch and an undo, in the order the core checks them. */
  const assertWritable = () => {
    if (gameRunning) throw new ApiFailure('gameRunning');
    if (serverRunning) throw new ApiFailure('serverRunning');
    if (settingsBroken) throw new ApiFailure('settingsUnreadable');
  };
  const record = (kind: MockOp['kind'], toId: string | null): SwitchResult => {
    const fromId = activeId;
    activeId = toId;
    const op: MockOp = { id: String(ops.length + 1), kind, fromId, toId };
    ops.push(op);
    return { opId: op.id, fromProfileId: fromId, toProfileId: toId };
  };

  return {
    sampleData: true,
    async addProfile(): Promise<Profile | null> {
      await wait();
      return null;
    },
    async openProfileFolder(): Promise<void> {},
    async showModInFolder(): Promise<void> {},
    async setActiveProfile(profileId: string): Promise<SwitchResult> {
      await wait();
      assertWritable();
      if (!dataset(stress).profiles.some((p) => p.id === profileId)) {
        throw new ApiFailure('profileNotFound');
      }
      if (writeFails) throw new ApiFailure('writeFailed');
      return record('switch', profileId);
    },
    /** Reverses the latest operation only, and only a switch (an undo is not undone). */
    async undoSwitch(opId: string): Promise<SwitchResult> {
      await wait();
      assertWritable();
      const latest = ops.at(-1);
      if (latest?.id !== opId || latest.kind !== 'switch') throw new ApiFailure('nothingToUndo');
      return record('undo', latest.fromId);
    },
    async latestSettingsBackup(): Promise<string | null> {
      await wait();
      return brokenSettings ? MOCK_BACKUP_AT : null;
    },
    async restoreSettingsBackup(): Promise<void> {
      await wait();
      if (!brokenSettings) throw new ApiFailure('noBackup');
      if (gameRunning) throw new ApiFailure('gameRunning');
      if (serverRunning) throw new ApiFailure('serverRunning');
      settingsBroken = false;
    },
    // WS-C: transfers, removal and profile management in memory.
    async planTransfer(): Promise<TransferPlan> {
      throw new ApiFailure('internal');
    },
    async executeTransfer(): Promise<OpResult> {
      throw new ApiFailure('internal');
    },
    async removeMods(): Promise<OpResult> {
      throw new ApiFailure('internal');
    },
    async cancelOperation(): Promise<void> {},
    async renameProfile(): Promise<void> {
      throw new ApiFailure('internal');
    },
    async removeProfile(): Promise<void> {
      throw new ApiFailure('internal');
    },
    async getGameStatus(): Promise<GameStatus> {
      await wait();
      // The game's default folder means the override is off; an unreadable file tells us nothing.
      const overrideActive =
        !settingsBroken && activeId !== null && activeId !== DEFAULT_PROFILE_ID;
      return {
        gameVersion: '1.25.1.0',
        gameRunning,
        serverRunning,
        settingsReadable: !settingsBroken,
        overrideActive,
        effectiveModsDir: settingsBroken ? null : pathOf(activeId),
        loggedDescVersion: GAME_DESC_VERSION,
      };
    },
    async listProfiles(): Promise<Profile[]> {
      await wait();
      return dataset(stress).profiles.map((p) => ({
        ...p,
        isActive: !settingsBroken && p.id === activeId,
      }));
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
