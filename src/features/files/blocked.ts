import type { GameStatus } from '../../api';
import { strings } from '../../strings';

/**
 * Why copy, move and remove are unavailable right now, or null when they are. Reads the polled game
 * status (the core checks again before every item) and whether this window already has an
 * operation running; only one runs at a time. The game takes precedence over the server.
 */
export function fileOpBlockedReason(
  game: GameStatus | null,
  operationRunning: boolean,
): string | null {
  if (game?.gameRunning) return strings.fileOps.blockedGame;
  if (game?.serverRunning) return strings.fileOps.blockedServer;
  if (operationRunning) return strings.fileOps.blockedRunning;
  return null;
}
