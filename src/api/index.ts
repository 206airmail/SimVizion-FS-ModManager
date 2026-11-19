// The frontend's only door to data. Screens import from here, never from an implementation.
// Types are generated from crates/fsmm-core/src/model.rs by `npm run gen:types`.

import type { GameStatus } from './types/GameStatus';
import type { ModEntry } from './types/ModEntry';
import type { Profile } from './types/Profile';
import type { ScanProgress } from './types/ScanProgress';
import type { ClashChoice } from './types/ClashChoice';
import type { Comparison } from './types/Comparison';
import type { HashProgress } from './types/HashProgress';
import type { LinkKind } from './types/LinkKind';
import type { Savegame } from './types/Savegame';
import type { SavegameReport } from './types/SavegameReport';
import type { OpProgress } from './types/OpProgress';
import type { OpResult } from './types/OpResult';
import type { SwitchResult } from './types/SwitchResult';
import type { TransferMode } from './types/TransferMode';
import type { TransferPlan } from './types/TransferPlan';
import type { UpdateCheck } from './types/UpdateCheck';
import type { UpdatePlan } from './types/UpdatePlan';
import type { UpdateSettings } from './types/UpdateSettings';
import type { AppUpdate } from './types/AppUpdate';
import type { AppUpdateSettings } from './types/AppUpdateSettings';
import type { HealthReport } from './types/HealthReport';

export type { ApiError } from './types/ApiError';
export type { AppUpdate } from './types/AppUpdate';
export type { AppUpdateSettings } from './types/AppUpdateSettings';
export type { GameStore } from './types/GameStore';
export type { HealthCount } from './types/HealthCount';
export type { HealthReport } from './types/HealthReport';
export type { HealthRow } from './types/HealthRow';
export type { UserDirSource } from './types/UserDirSource';
export type { ClashChoice } from './types/ClashChoice';
export type { CompareRow } from './types/CompareRow';
export type { CompareSide } from './types/CompareSide';
export type { CompareState } from './types/CompareState';
export type { Comparison } from './types/Comparison';
export type { Dependency } from './types/Dependency';
export type { ErrorCode } from './types/ErrorCode';
export type { GameStatus } from './types/GameStatus';
export type { HashProgress } from './types/HashProgress';
export type { ItemFailure } from './types/ItemFailure';
export type { IssueKind } from './types/IssueKind';
export type { LinkKind } from './types/LinkKind';
export type { ModEntry } from './types/ModEntry';
export type { ModFormat } from './types/ModFormat';
export type { ModIssue } from './types/ModIssue';
export type { ModKind } from './types/ModKind';
export type { ModSource } from './types/ModSource';
export type { ModSummary } from './types/ModSummary';
export type { OpProgress } from './types/OpProgress';
export type { OpResult } from './types/OpResult';
export type { Profile } from './types/Profile';
export type { ScanProgress } from './types/ScanProgress';
export type { Savegame } from './types/Savegame';
export type { SavegameReport } from './types/SavegameReport';
export type { SaveModRow } from './types/SaveModRow';
export type { SaveModState } from './types/SaveModState';
export type { ScanSummary } from './types/ScanSummary';
export type { Severity } from './types/Severity';
export type { SourceLink } from './types/SourceLink';
export type { SwitchResult } from './types/SwitchResult';
export type { TransferClash } from './types/TransferClash';
export type { TransferItem } from './types/TransferItem';
export type { TransferMode } from './types/TransferMode';
export type { TransferPlan } from './types/TransferPlan';
export type { UpdateCheck } from './types/UpdateCheck';
export type { UpdateFolder } from './types/UpdateFolder';
export type { UpdatePlan } from './types/UpdatePlan';
export type { UpdateSettings } from './types/UpdateSettings';
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
  /** Works out a copy or move (items, clashes, space) without writing anything. */
  planTransfer(
    fromProfileId: string,
    modIds: string[],
    toProfileId: string,
    mode: TransferMode,
  ): Promise<TransferPlan>;
  /** Carries out a plan; clashes without a choice are skipped. Refused while the game runs. */
  executeTransfer(
    planId: string,
    choices: ClashChoice[],
    onProgress?: (progress: OpProgress) => void,
  ): Promise<OpResult>;
  /** Sends mods to the Recycle Bin; never deletes permanently. */
  removeMods(
    profileId: string,
    modIds: string[],
    onProgress?: (progress: OpProgress) => void,
  ): Promise<OpResult>;
  /** Stops the running copy, move or remove after the current chunk. */
  cancelOperation(): Promise<void>;
  /** Changes a profile's display name; the folder is untouched. */
  renameProfile(profileId: string, name: string): Promise<void>;
  /** Removes a profile from the list; the folder is untouched. */
  removeProfile(profileId: string): Promise<void>;
  /** Every mod name of two profiles side by side, with how each pair compares. */
  compareProfiles(leftProfileId: string, rightProfileId: string): Promise<Comparison>;
  /** The saves in the game's savegame slots (read-only). */
  listSavegames(): Promise<Savegame[]>;
  /**
   * A save's mods against a profile, at once: rows needing a file hash that is not cached yet
   * are `hashPending`. Follow with `checkSavegameFiles`.
   */
  savegameReport(savegameId: string, profileId: string): Promise<SavegameReport>;
  /** Hashes the pending zips (parallel, cached) and returns the final report. */
  checkSavegameFiles(
    savegameId: string,
    profileId: string,
    onProgress?: (progress: HashProgress) => void,
  ): Promise<SavegameReport>;
  /** Stops a running `checkSavegameFiles`; it rejects with `cancelled`. */
  cancelHashing(): Promise<void>;
  /** Replaces a mod name's tags (shared by every profile); returns the stored, sorted tags. */
  setTags(modName: string, tags: string[]): Promise<string[]>;
  /** Sets a mod name's note; an empty note clears it. */
  setNote(modName: string, note: string): Promise<void>;
  /** Every tag in use, sorted. */
  listTags(): Promise<string[]>;
  /** The online-update settings; off by default, and nothing goes online while off. */
  getUpdateSettings(): Promise<UpdateSettings>;
  setUpdatesEnabled(enabled: boolean): Promise<UpdateSettings>;
  /**
   * Checks every folder's mods against the update catalog. Without `force`, results less than
   * 3 hours old come from the cache; with it, less than 60 s. Rejects with `updatesDisabled`,
   * `offline` or `rateLimited` (detail: ISO time to try again).
   */
  checkForUpdates(force: boolean): Promise<UpdateCheck>;
  /** Which folders hold the mod and where its update comes from; no network. */
  planUpdate(modName: string): Promise<UpdatePlan>;
  /** Downloads, verifies and installs the update into the given folders (old versions recycled). */
  installUpdate(
    modName: string,
    profileIds: string[],
    onProgress?: (progress: OpProgress) => void,
  ): Promise<OpResult>;
  /** Opens one of the mod's pages (see `ModEntry.links`) in the browser. */
  openModLink(modName: string, kind: LinkKind): Promise<void>;
  /** Sets (or clears with null) the user's source link; returns its host. */
  setSourceLink(modName: string, url: string | null): Promise<string | null>;
  /**
   * Starts the detected game install (Steam, Epic, GIANTS or Microsoft Store). Rejects with
   * `gameRunning` or `notLaunchable`.
   */
  launchGame(): Promise<void>;
  /** Every mod with issues in every folder, with per-kind counts (the Health screen). */
  healthReport(): Promise<HealthReport>;
  /**
   * Asks for the game's user folder with the native picker and stores it (it overrides
   * detection). Null when cancelled; rejects with `invalidGameFolder`. Returns the new status.
   */
  chooseGameFolder(): Promise<GameStatus | null>;
  /** Forgets the chosen game folder, so detection decides again. */
  clearGameFolder(): Promise<GameStatus>;
  /** The app's own update settings; off by default. */
  getAppUpdateSettings(): Promise<AppUpdateSettings>;
  setAppUpdatesEnabled(enabled: boolean): Promise<AppUpdateSettings>;
  /**
   * Asks the release feed for a newer app version (null when up to date). Without `force`,
   * nothing is asked within 24 hours of the last check (null). Rejects with `updatesDisabled`
   * when off, `appUpdateFailed` otherwise.
   */
  checkAppUpdate(force: boolean): Promise<AppUpdate | null>;
  /** Downloads and verifies the update found, runs its installer and restarts the app. */
  installAppUpdate(onProgress?: (progress: OpProgress) => void): Promise<void>;
}
