import type {
  Api,
  ClashChoice,
  Comparison,
  GameStatus,
  HashProgress,
  ModEntry,
  OpProgress,
  OpResult,
  Profile,
  Savegame,
  SavegameReport,
  ScanProgress,
  SwitchResult,
  TransferMode,
  TransferPlan,
  UpdateCheck,
  UpdatePlan,
  UpdateSettings,
} from '..';
import { validateProfileName } from '../../lib/profileName';
import { ApiFailure } from '../../lib/errors';
import { compareMods } from './compare';
import { GAME_DESC_VERSION } from './entry';
import {
  cloneState,
  findClashes,
  nameKey,
  removeMod,
  settle,
  transferMod,
  type MockState,
} from './files';
import { allTags, normalizeNote, normalizeTags, seedMeta, withMeta } from './meta';
import { createRng } from './prng';
import { buildProfileMods, emptyRecipe, pickCopies, scaleRecipe, type Recipe } from './profile';
import { buildReport, buildSaves, hashKey, pendingMods, type SaveDef } from './savegames';

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
  /** Every transfer plan has clashes: the target gets an older twin of every other mod. Default false. */
  clashes?: boolean;
  /** Remove and Replace fail with recycleUnavailable for some mods, and the rest succeed. Default false. */
  recycleFails?: boolean;
  /** Plans show too little free space on the target drive, and moves count as cross-drive. Default false. */
  lowSpace?: boolean;
  /** Hashing a save's files takes about 5 s instead of 1.5 s, so progress and Cancel can be tried. Default false. */
  slowHash?: boolean;
  /** Adds a fourth save whose file cannot be read: its report fails with savegameUnreadable. Default false. */
  brokenSave?: boolean;
  /** The game has no saves: listSavegames returns an empty list. Default false. */
  noSaves?: boolean;
}

/** The mock's newest settings backup, offered while `brokenSettings` is on. */
export const MOCK_BACKUP_AT = '2026-11-17T18:42:00Z';

/** The profile that is active at start, and the one whose folder is the game's default. */
const INITIAL_ACTIVE_ID = 'testing';
const DEFAULT_PROFILE_ID = 'main';

/** A plan made by planTransfer and waiting for executeTransfer. */
interface StoredPlan {
  plan: TransferPlan;
}

interface MockOp {
  id: string;
  kind: 'switch' | 'undo';
  fromId: string | null;
  toId: string | null;
}

/** How many progress events a mock listMods emits across its delay. */
const PROGRESS_STEPS = 5;

/**
 * Copy speed of the mock: 2 MiB every 50 ms (about 40 MB/s), in 5 to 200 steps. A big mod takes
 * several seconds, so Cancel can be tried; a small one still shows movement.
 */
const TICK_MS = 50;
const BYTES_PER_TICK = 2 * 1024 * 1024;
const MIN_TICKS = 5;
const MAX_TICKS = 200;
/** Free space the mock reports for the target drive. */
const FREE_BYTES = 48 * 1024 ** 3;

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

/** Gives the first `count` same-version copies in `copies` a different size and a later date. */
function rezip(copies: ModEntry[], original: readonly ModEntry[], count: number): void {
  const byName = new Map(original.map((mod) => [nameKey(mod.modName), mod]));
  let changed = 0;
  for (const mod of copies) {
    const first = byName.get(nameKey(mod.modName));
    if (changed >= count) return;
    if (!first || first.version !== mod.version || first.format !== 'zip') continue;
    mod.sizeBytes = Math.round(mod.sizeBytes * 0.97);
    mod.modified = new Date(Date.parse(mod.modified) + 9 * 24 * 3600 * 1000).toISOString();
    changed += 1;
  }
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
  // Re-zipped copies: the same version as Main Farm's, but a different file (see compare.ts).
  rezip(testing, main, 2);
  rezip(offline, main, 1);

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
    clashes: guaranteeClashes = false,
    recycleFails = false,
    lowSpace = false,
    slowHash = false,
    brokenSave = false,
    noSaves = false,
  } = options;
  const wait = (): Promise<void> =>
    delayMs > 0 ? new Promise((resolve) => setTimeout(resolve, delayMs)) : Promise.resolve();

  // Switching state and the folders live in this instance; the shared dataset is never mutated.
  let activeId: string | null = INITIAL_ACTIVE_ID;
  let settingsBroken = brokenSettings;
  const ops: MockOp[] = [];
  let folders: MockState | null = null;
  const state = (): MockState => (folders ??= cloneState(dataset(stress)));
  const pathOf = (id: string | null) => state().profiles.find((p) => p.id === id)?.path ?? null;

  // File operations: plans wait here for execute, and one operation runs at a time.
  const plans = new Map<string, StoredPlan>();
  let planCount = 0;
  let running = false;
  let cancelRequested = false;
  const pause = (ms: number): Promise<void> =>
    delayMs > 0 ? new Promise((resolve) => setTimeout(resolve, ms)) : Promise.resolve();
  // Tags and notes (shared by every profile), the saves, and the hashes computed so far.
  const meta = seedMeta(dataset(stress).mods.get('main') ?? []);
  let saveDefs: SaveDef[] | null = null;
  const saves = (): SaveDef[] =>
    noSaves ? [] : (saveDefs ??= buildSaves(dataset(stress).mods, brokenSave));
  const hashed = new Set<string>();
  let hashing = false;
  let hashCancelRequested = false;
  const requireSave = (id: string): SaveDef => {
    const save = saves().find((candidate) => candidate.savegame.id === id);
    if (!save) throw new ApiFailure('savegameMissing');
    if (save.unreadable) throw new ApiFailure('savegameUnreadable');
    return save;
  };
  const reportOf = (save: SaveDef, profileId: string) =>
    buildReport({
      save,
      profileId,
      folders: state().mods,
      profileOrder: state().profiles.map((p) => p.id),
      hashed,
    });
  const requireProfile = (id: string): Profile => {
    const found = state().profiles.find((p) => p.id === id);
    if (!found) throw new ApiFailure('profileNotFound');
    return found;
  };
  const assertFileOpsAllowed = () => {
    if (gameRunning) throw new ApiFailure('gameRunning');
    if (serverRunning) throw new ApiFailure('serverRunning');
  };
  /** Which mods the mock refuses to recycle: every second one, or a lone one. */
  const recycleRefused = (index: number, total: number) =>
    recycleFails && (total === 1 || index % 2 === 1);
  /**
   * Streams `size` bytes as progress steps; false when the user cancelled. `report` gets the
   * bytes done so far in this mod.
   */
  const stream = async (
    size: number,
    report: (bytes: number) => void,
    steps?: number,
  ): Promise<boolean> => {
    const ticks =
      steps ?? Math.min(MAX_TICKS, Math.max(MIN_TICKS, Math.ceil(size / BYTES_PER_TICK)));
    for (let tick = 1; tick <= ticks; tick++) {
      await pause(TICK_MS);
      if (cancelRequested) return false;
      report(Math.round((size * tick) / ticks));
    }
    return true;
  };
  /** Runs one file operation: busy and game guards first, the busy flag held until it ends. */
  const exclusive = async <T>(body: () => Promise<T>): Promise<T> => {
    if (running) throw new ApiFailure('busy');
    assertFileOpsAllowed();
    running = true;
    cancelRequested = false;
    try {
      return await body();
    } finally {
      running = false;
      cancelRequested = false;
    }
  };

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
      requireProfile(profileId);
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
    async planTransfer(
      fromProfileId: string,
      modIds: string[],
      toProfileId: string,
      mode: TransferMode,
    ): Promise<TransferPlan> {
      await wait();
      requireProfile(fromProfileId);
      requireProfile(toProfileId);
      if (fromProfileId === toProfileId) throw new ApiFailure('internal', 'sameProfile');
      const source = state().mods.get(fromProfileId) ?? [];
      const picked = modIds.map((id) => source.find((mod) => mod.id === id));
      if (picked.some((mod) => mod === undefined)) throw new ApiFailure('targetChanged');
      const incoming = picked.filter((mod): mod is ModEntry => mod !== undefined);

      const clashes = findClashes(state(), incoming, toProfileId, guaranteeClashes);
      settle(state());
      const sum = incoming.reduce((total, mod) => total + mod.sizeBytes, 0);
      // Moves inside one drive are renames and write nothing; low space makes them cross-drive.
      const totalBytes = mode === 'copy' || lowSpace ? sum : 0;
      const plan: TransferPlan = {
        planId: `plan-${++planCount}`,
        fromProfileId,
        toProfileId,
        mode,
        items: incoming.map((mod) => ({
          modId: mod.id,
          modName: mod.modName,
          sizeBytes: mod.sizeBytes,
        })),
        clashes,
        totalBytes,
        freeBytes: lowSpace ? Math.floor(totalBytes / 2) : FREE_BYTES,
      };
      plans.set(plan.planId, { plan });
      return plan;
    },
    async executeTransfer(
      planId: string,
      choices: ClashChoice[],
      onProgress?: (progress: OpProgress) => void,
    ): Promise<OpResult> {
      return exclusive(async () => {
        const stored = plans.get(planId);
        if (!stored) throw new ApiFailure('targetChanged');
        const { plan } = stored;
        if (plan.totalBytes > plan.freeBytes) throw new ApiFailure('notEnoughSpace');
        plans.delete(planId);

        const replace = new Map(choices.map((c) => [nameKey(c.modName), c.replace]));
        const clashing = new Set(plan.clashes.map((c) => nameKey(c.modName)));
        const replaces = (name: string) =>
          clashing.has(nameKey(name)) && !!replace.get(nameKey(name));
        const result: OpResult = { done: [], skipped: [], failed: [] };
        const work = plan.items.filter((item) => {
          if (!clashing.has(nameKey(item.modName)) || replaces(item.modName)) return true;
          result.skipped.push(item.modName);
          return false;
        });
        const totalBytes =
          plan.totalBytes > 0 ? work.reduce((sum, item) => sum + item.sizeBytes, 0) : 0;
        const emit = (doneBytes: number, doneItems: number, current: string | null) =>
          onProgress?.({ doneBytes, totalBytes, current, doneItems, totalItems: work.length });

        let doneBytes = 0;
        let replaceIndex = 0;
        const replaceTotal = work.filter((item) => replaces(item.modName)).length;
        emit(0, 0, null);
        for (const [index, item] of work.entries()) {
          const mod = state()
            .mods.get(plan.fromProfileId)
            ?.find((m) => m.id === item.modId);
          if (cancelRequested) {
            result.failed.push({ modName: item.modName, code: 'cancelled' });
            continue;
          }
          if (!mod) {
            result.failed.push({ modName: item.modName, code: 'targetChanged' });
            continue;
          }
          emit(doneBytes, index, mod.fileName);
          // Without bytes to write (a same-drive move) an item is one short step.
          const finished = await stream(
            totalBytes > 0 ? mod.sizeBytes : 0,
            (bytes) => emit(doneBytes + bytes, index, mod.fileName),
            totalBytes > 0 ? undefined : 1,
          );
          if (!finished) {
            result.failed.push({ modName: item.modName, code: 'cancelled' });
            continue;
          }
          if (replaces(item.modName) && recycleRefused(replaceIndex++, replaceTotal)) {
            result.failed.push({ modName: item.modName, code: 'recycleUnavailable' });
            continue;
          }
          transferMod(state(), mod, plan.fromProfileId, plan.toProfileId, plan.mode);
          doneBytes += totalBytes > 0 ? mod.sizeBytes : 0;
          result.done.push(item.modName);
        }
        settle(state());
        emit(doneBytes, result.done.length, null);
        return result;
      });
    },
    async removeMods(
      profileId: string,
      modIds: string[],
      onProgress?: (progress: OpProgress) => void,
    ): Promise<OpResult> {
      return exclusive(async () => {
        requireProfile(profileId);
        const mods = state().mods.get(profileId) ?? [];
        const result: OpResult = { done: [], skipped: [], failed: [] };
        const picked: ModEntry[] = [];
        for (const id of modIds) {
          const mod = mods.find((m) => m.id === id);
          if (mod) picked.push(mod);
          else result.failed.push({ modName: id, code: 'targetChanged' });
        }
        const totalBytes = picked.reduce((sum, mod) => sum + mod.sizeBytes, 0);
        const emit = (doneBytes: number, doneItems: number, current: string | null) =>
          onProgress?.({ doneBytes, totalBytes, current, doneItems, totalItems: picked.length });

        let doneBytes = 0;
        emit(0, 0, null);
        for (const [index, mod] of picked.entries()) {
          if (cancelRequested) {
            result.failed.push({ modName: mod.modName, code: 'cancelled' });
            continue;
          }
          emit(doneBytes, index, mod.fileName);
          const finished = await stream(
            mod.sizeBytes,
            (bytes) => emit(doneBytes + bytes, index, mod.fileName),
            6,
          );
          if (!finished) {
            result.failed.push({ modName: mod.modName, code: 'cancelled' });
            continue;
          }
          if (recycleRefused(index, picked.length)) {
            result.failed.push({ modName: mod.modName, code: 'recycleUnavailable' });
            continue;
          }
          removeMod(state(), profileId, mod.id);
          doneBytes += mod.sizeBytes;
          result.done.push(mod.modName);
        }
        settle(state());
        emit(doneBytes, result.done.length, null);
        return result;
      });
    },
    async cancelOperation(): Promise<void> {
      if (running) cancelRequested = true;
    },
    /** Same rules as the core: a trimmed name of 1 to 100 characters; the folder is untouched. */
    async renameProfile(profileId: string, name: string): Promise<void> {
      await wait();
      requireProfile(profileId);
      if (validateProfileName(name) !== null) throw new ApiFailure('internal', 'invalidName');
      state().profiles = state().profiles.map((p) =>
        p.id === profileId ? { ...p, name: name.trim() } : p,
      );
    },
    /** The active folder and the game default stay in the list; the folder itself is untouched. */
    async removeProfile(profileId: string): Promise<void> {
      await wait();
      const profile = requireProfile(profileId);
      if (profile.id === activeId || profile.isGameDefault) {
        throw new ApiFailure('cannotRemoveProfile');
      }
      state().profiles = state().profiles.filter((p) => p.id !== profileId);
      state().mods.delete(profileId);
      settle(state());
    },
    async compareProfiles(leftProfileId: string, rightProfileId: string): Promise<Comparison> {
      await wait();
      requireProfile(leftProfileId);
      requireProfile(rightProfileId);
      return compareMods(
        leftProfileId,
        state().mods.get(leftProfileId) ?? [],
        rightProfileId,
        state().mods.get(rightProfileId) ?? [],
      );
    },
    async listSavegames(): Promise<Savegame[]> {
      await wait();
      return saves().map((save) => ({ ...save.savegame }));
    },
    async savegameReport(savegameId: string, profileId: string): Promise<SavegameReport> {
      await wait();
      const save = requireSave(savegameId);
      requireProfile(profileId);
      return reportOf(save, profileId);
    },
    /**
     * Hashes the pending zips at about 40 ms per progress step (1.5 s in all, 5 s with slowHash),
     * and returns the final report. Files finished before a cancel stay hashed.
     */
    async checkSavegameFiles(
      savegameId: string,
      profileId: string,
      onProgress?: (progress: HashProgress) => void,
    ): Promise<SavegameReport> {
      if (hashing) throw new ApiFailure('busy');
      hashing = true;
      hashCancelRequested = false;
      try {
        await wait();
        const save = requireSave(savegameId);
        requireProfile(profileId);
        const input = {
          save,
          profileId,
          folders: state().mods,
          profileOrder: state().profiles.map((p) => p.id),
          hashed,
        };
        const files = pendingMods(input);
        const totalBytes = files.reduce((sum, mod) => sum + mod.sizeBytes, 0);
        const ticks = slowHash ? 100 : 30;
        let doneFiles = 0;
        let doneBytes = 0;
        const emit = () =>
          onProgress?.({ doneFiles, totalFiles: files.length, doneBytes, totalBytes });
        emit();
        for (let tick = 1; tick <= ticks && files.length > 0; tick++) {
          await pause(TICK_MS);
          if (hashCancelRequested) throw new ApiFailure('cancelled');
          const target = Math.round((files.length * tick) / ticks);
          while (doneFiles < target) {
            const file = files[doneFiles];
            if (file) {
              hashed.add(hashKey(profileId, file));
              doneBytes += file.sizeBytes;
            }
            doneFiles += 1;
          }
          emit();
        }
        return buildReport({ ...input, hashed });
      } finally {
        hashing = false;
        hashCancelRequested = false;
      }
    },
    async cancelHashing(): Promise<void> {
      if (hashing) hashCancelRequested = true;
    },
    async setTags(modName: string, tags: string[]): Promise<string[]> {
      await wait();
      const stored = normalizeTags(tags);
      if (stored.length > 0) meta.tags.set(nameKey(modName), stored);
      else meta.tags.delete(nameKey(modName));
      return [...stored];
    },
    async setNote(modName: string, note: string): Promise<void> {
      await wait();
      const stored = normalizeNote(note);
      if (stored === null) meta.notes.delete(nameKey(modName));
      else meta.notes.set(nameKey(modName), stored);
    },
    async listTags(): Promise<string[]> {
      await wait();
      return allTags(meta);
    },
    // Plan M7: implemented by WS-C.
    async getUpdateSettings(): Promise<UpdateSettings> {
      return { enabled: false, lastCheck: null, retryAfter: null };
    },
    async setUpdatesEnabled(): Promise<UpdateSettings> {
      throw new ApiFailure('internal', 'notImplemented');
    },
    async checkForUpdates(): Promise<UpdateCheck> {
      throw new ApiFailure('updatesDisabled');
    },
    async planUpdate(): Promise<UpdatePlan> {
      throw new ApiFailure('internal', 'notImplemented');
    },
    async installUpdate(): Promise<OpResult> {
      throw new ApiFailure('internal', 'notImplemented');
    },
    async openModLink(): Promise<void> {
      throw new ApiFailure('internal', 'notImplemented');
    },
    async setSourceLink(): Promise<string | null> {
      throw new ApiFailure('internal', 'notImplemented');
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
      return state().profiles.map((p) => ({
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
      const mods = (state().mods.get(profileId) ?? []).map((mod) => withMeta(mod, meta));
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
