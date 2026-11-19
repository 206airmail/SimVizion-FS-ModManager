// The frontend's only door to data. Screens import from here, never from an implementation.
// Types are generated from crates/fsmm-core/src/model.rs by `npm run gen:types`.

import type { GameStatus } from './types/GameStatus';
import type { ModEntry } from './types/ModEntry';
import type { Profile } from './types/Profile';

export type { Dependency } from './types/Dependency';
export type { GameStatus } from './types/GameStatus';
export type { IssueKind } from './types/IssueKind';
export type { ModEntry } from './types/ModEntry';
export type { ModFormat } from './types/ModFormat';
export type { ModIssue } from './types/ModIssue';
export type { ModKind } from './types/ModKind';
export type { ModSource } from './types/ModSource';
export type { Profile } from './types/Profile';
export type { ScanSummary } from './types/ScanSummary';
export type { Severity } from './types/Severity';
export type { UpdateState } from './types/UpdateState';

export interface Api {
  getGameStatus(): Promise<GameStatus>;
  listProfiles(): Promise<Profile[]>;
  listMods(profileId: string): Promise<ModEntry[]>;
}
