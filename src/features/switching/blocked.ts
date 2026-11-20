import { strings } from '../../strings';
import type { GuardedGameStatus } from '../shell/launch';

/**
 * Why the active folder cannot be changed right now, or null when it can. Reads the polled game
 * status (the core checks again before it writes), plus `launching`: the game was just launched
 * and has not shown up yet. The game takes precedence when both run.
 */
export function switchBlockedReason(game: GuardedGameStatus | null): string | null {
  if (game?.gameRunning) return strings.switching.blockedGame;
  if (game?.launching) return strings.switching.blockedStarting;
  if (game?.serverRunning) return strings.switching.blockedServer;
  return null;
}
