// The mock's in-memory folders and the pure rules for copy, move, remove and profile management.
// index.ts adds the timing (delays, progress, cancel) around these.

import type { ModEntry, ModSummary, Profile, TransferClash } from '..';

/** One mock instance's folders. Cloned from the shared dataset so the dataset is never mutated. */
export interface MockState {
  profiles: Profile[];
  mods: Map<string, ModEntry[]>;
}

export function cloneState(source: MockState): MockState {
  return {
    profiles: source.profiles.map((profile) => ({ ...profile })),
    mods: new Map(
      [...source.mods].map(([id, list]) => [
        id,
        list.map((mod) => ({ ...mod, alsoIn: [...mod.alsoIn] })),
      ]),
    ),
  };
}

/** A mod's identity is its name, and the game compares names without regard to case. */
export const nameKey = (modName: string): string => modName.toLowerCase();

export function findByName(mods: readonly ModEntry[], modName: string): ModEntry | undefined {
  const key = nameKey(modName);
  return mods.find((mod) => nameKey(mod.modName) === key);
}

export function toSummary(mod: ModEntry): ModSummary {
  return {
    fileName: mod.fileName,
    version: mod.version,
    sizeBytes: mod.sizeBytes,
    modified: mod.modified,
  };
}

/** Counts and totals follow the folder's mods, the way a fresh scan would report them. */
export function refreshCounts(state: MockState): void {
  state.profiles = state.profiles.map((profile) => {
    const mods = state.mods.get(profile.id) ?? [];
    return {
      ...profile,
      modCount: mods.length,
      totalBytes: mods.reduce((sum, mod) => sum + mod.sizeBytes, 0),
      issueCount: mods.filter((mod) => mod.issues.length > 0).length,
      updateCount: mods.filter((mod) => mod.update.type === 'available').length,
    };
  });
}

/** "Also in" lists the other folders holding a mod of the same name, in the list's order. */
export function recomputeAlsoIn(state: MockState): void {
  const holders = new Map<string, Set<string>>();
  for (const [profileId, mods] of state.mods) {
    for (const mod of mods) {
      const key = nameKey(mod.modName);
      const ids = holders.get(key) ?? new Set<string>();
      ids.add(profileId);
      holders.set(key, ids);
    }
  }
  const order = state.profiles.map((profile) => profile.id);
  for (const [profileId, mods] of state.mods) {
    for (const mod of mods) {
      const ids = holders.get(nameKey(mod.modName));
      mod.alsoIn = order.filter((other) => other !== profileId && ids?.has(other));
    }
  }
}

/** Both are done after any change to the folders. */
export function settle(state: MockState): void {
  recomputeAlsoIn(state);
  refreshCounts(state);
}

/** The same mod as an older, slightly smaller version: what a clash looks like. */
export function olderTwin(mod: ModEntry, profileId: string): ModEntry {
  const bumped = mod.version.replace(/(\d+)$/, (digits) => String(Math.max(0, Number(digits) - 1)));
  const modified = new Date(Date.parse(mod.modified) - 40 * 24 * 3600 * 1000).toISOString();
  return {
    ...mod,
    profileId,
    version: bumped === mod.version ? '0.9.0.0' : bumped,
    sizeBytes: Math.round(mod.sizeBytes * 0.92),
    modified,
    alsoIn: [],
    tags: [],
    note: null,
    links: [],
  };
}

/**
 * The clashes of moving `incoming` into `toId`: same name, compared without case. With
 * `guarantee`, every other mod (at least one) first gets an older twin in the target, so a plan
 * always has something to decide.
 */
export function findClashes(
  state: MockState,
  incoming: readonly ModEntry[],
  toId: string,
  guarantee: boolean,
): TransferClash[] {
  const target = state.mods.get(toId) ?? [];
  if (guarantee) {
    incoming.forEach((mod, index) => {
      if (index % 2 === 0 && !findByName(target, mod.modName)) target.push(olderTwin(mod, toId));
    });
    state.mods.set(toId, target);
  }
  const clashes: TransferClash[] = [];
  for (const mod of incoming) {
    const existing = findByName(target, mod.modName);
    if (existing) {
      clashes.push({
        modName: mod.modName,
        incoming: toSummary(mod),
        existing: toSummary(existing),
      });
    }
  }
  return clashes;
}

/**
 * Puts one mod into the target (replacing the mod of the same name there) and, for a move,
 * takes it out of the source.
 */
export function transferMod(
  state: MockState,
  mod: ModEntry,
  fromId: string,
  toId: string,
  mode: 'copy' | 'move',
): void {
  const target = (state.mods.get(toId) ?? []).filter(
    (existing) => nameKey(existing.modName) !== nameKey(mod.modName),
  );
  target.push({ ...mod, profileId: toId, alsoIn: [] });
  state.mods.set(toId, target);
  if (mode === 'move') removeMod(state, fromId, mod.id);
}

export function removeMod(state: MockState, profileId: string, modId: string): void {
  state.mods.set(
    profileId,
    (state.mods.get(profileId) ?? []).filter((mod) => mod.id !== modId),
  );
}
