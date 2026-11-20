import type { GameStatus } from '../../api';
import { strings } from '../../strings';

/**
 * The game status as the write blockers see it. `launching` is set between a press of Launch game
 * and the game process showing up in the status (or the wait running out): the game may already
 * be reading its settings, so nothing may write to them yet. It lives beside the status, not in
 * it, because the status is read from the core and `launching` is this window's own knowledge.
 */
export type GuardedGameStatus = GameStatus & { launching?: boolean };

/** The status with `launching` set while a launch is waiting and the game is not yet running. */
export function withLaunching(
  game: GameStatus | null,
  launching: boolean,
): GuardedGameStatus | null {
  if (game === null || !launching || game.gameRunning) return game;
  return { ...game, launching: true };
}

export interface LaunchView {
  /** The button says "Starting…" instead of "Launch game". */
  starting: boolean;
  /** Why the button cannot be used right now, or null when it can. */
  blockedReason: string | null;
}

/**
 * What the header's Launch game button shows. The running game comes first (it also ends the
 * "starting" state), then a click that is still being carried out, then a write the app is making
 * to the game's files (`writeReason`: a file operation or a profile switch), then an install the
 * app cannot start. Before the first status read nothing is known, so the button waits.
 */
export function launchView(
  game: GameStatus | null,
  starting: boolean,
  writeReason: string | null,
): LaunchView {
  const t = strings.header;
  if (game === null) return { starting: false, blockedReason: t.launchChecking };
  if (game.gameRunning) return { starting: false, blockedReason: t.launchBlockedRunning };
  if (starting) return { starting: true, blockedReason: t.launchStartingReason };
  if (writeReason !== null) return { starting: false, blockedReason: writeReason };
  if (!game.launchable) return { starting: false, blockedReason: t.launchBlockedNone };
  return { starting: false, blockedReason: null };
}
