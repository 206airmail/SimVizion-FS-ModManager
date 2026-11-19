// The frontend's only door to data. Screens import from here, never from an implementation.
// Types are generated from crates/fsmm-core/src/model.rs by `npm run gen:types`.

import type { GameStatus } from './types/GameStatus';
import type { ModEntry } from './types/ModEntry';
import type { Profile } from './types/Profile';
import type { ScanProgress } from './types/ScanProgress';
import type { SwitchResult } from './types/SwitchResult';

export type { ApiError } from './types/ApiError';
export type { Dependency } from './types/Dependency';
export type { ErrorCode } from './types/ErrorCode';
export type { GameStatus } from './types/GameStatus';
export type { IssueKind } from './types/IssueKind';
export type { ModEntry } from './types/ModEntry';
export type { ModFormat } from './types/ModFormat';
export type { ModIssue } from './types/ModIssue';
export type { ModKind } from './types/ModKind';
export type { ModSource } from './types/ModSource';
export type { Profile } from './types/Profile';
export type { ScanProgress } from './types/ScanProgress';
export type { ScanSummary } from './types/ScanSummary';
export type { Severity } from './types/Severity';
export type { SwitchResult } from './types/SwitchResult';
export type { UpdateState } from './types/UpdateState';

/**
 * Every method rejects with an `ApiError` (`{ code, detail }`) when it fails. Profiles and mods
 * are always named by id; the frontend never sends a filesystem path.
 */
export interface Api {
  /** True for generated sample data (the browser build), false for the user's real folders. */
  readonly sampleData: boolean;
  getGameStatus(): Promise<GameStatus>;
  /** Every registered profile with fresh counts. */
  listProfiles(): Promise<Profile[]>;
  /** The profile's mods, read fresh (unchanged files come from the cache). */
  listMods(profileId: string, onProgress?: (progress: ScanProgress) => void): Promise<ModEntry[]>;
  /** Opens a folder picker and registers the chosen folder; null when the user cancels. */
  addProfile(): Promise<Profile | null>;
  /** Opens the profile's folder in Explorer. */
  openProfileFolder(profileId: string): Promise<void>;
  /** Opens Explorer with the mod's file or folder selected. */
  showModInFolder(profileId: string, modId: string): Promise<void>;
  /**
   * Makes the profile the folder the game loads (edits `modsDirectoryOverride` in
   * gameSettings.xml after a backup). Refused while the game or the dedicated server runs.
   */
  setActiveProfile(profileId: string): Promise<SwitchResult>;
  /** Reverses the switch `opId` if it is the latest write and the file still holds it. */
  undoSwitch(opId: string): Promise<SwitchResult>;
  /** ISO time of the newest gameSettings.xml backup, or null when there is none. */
  latestSettingsBackup(): Promise<string | null>;
  /** Replaces an unreadable gameSettings.xml with the newest backup. */
  restoreSettingsBackup(): Promise<void>;
}
