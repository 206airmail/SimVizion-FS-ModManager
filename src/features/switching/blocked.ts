import type { GameStatus } from '../../api';
import { strings } from '../../strings';

/**
 * Why the active folder cannot be changed right now, or null when it can. Reads the polled game
 * status (the core checks again before it writes). The game takes precedence when both run.
 */
export function switchBlockedReason(game: GameStatus | null): string | null {
  if (game?.gameRunning) return strings.switching.blockedGame;
  if (game?.serverRunning) return strings.switching.blockedServer;
  return null;
}
