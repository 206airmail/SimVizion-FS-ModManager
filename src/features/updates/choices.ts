// Pure rules for the Update dialog and the "Update N" selection and for the link field.

import type { ModEntry, UpdateFolder, UpdatePlan } from '../../api';
import { isOlder } from '../../lib/version';

/**
 * The folders ticked when the dialog opens: the one the user is looking at, and every other
 * folder whose copy is older than the version being installed.
 */
export function defaultFolders(plan: UpdatePlan, selectedProfileId: string): string[] {
  return plan.folders
    .filter(
      (folder) => folder.profileId === selectedProfileId || isOlder(folder.version, plan.toVersion),
    )
    .map((folder) => folder.profileId);
}

/** A folder already at (or past) the new version has nothing to gain from the update. */
export const isCurrent = (folder: UpdateFolder, toVersion: string): boolean =>
  !isOlder(folder.version, toVersion);

/** The selected mods the app can update itself, each in the folder it is shown in. */
export function installableUpdates(mods: readonly ModEntry[]): ModEntry[] {
  return mods.filter((mod) => mod.update.type === 'available' && mod.update.installable);
}

/**
 * The host of what the user has typed, for the live preview, or null while it is not an http(s)
 * address. The core decides what is valid; this only says where the link would point.
 */
export function previewHost(text: string): string | null {
  try {
    const url = new URL(text.trim());
    return (url.protocol === 'http:' || url.protocol === 'https:') && url.hostname !== ''
      ? url.hostname
      : null;
  } catch {
    return null;
  }
}
