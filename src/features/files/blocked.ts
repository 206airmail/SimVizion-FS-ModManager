import { strings } from '../../strings';
import type { GuardedGameStatus } from '../shell/launch';

/**
 * Why copy, move and remove are unavailable right now, or null when they are. Reads the polled game
 * status (the core checks again before every item), whether a launch is waiting for the game to
 * appear, and whether this window already has an operation running; only one runs at a time. The
 * game takes precedence over a launch in progress, and that over the server.
 */
export function fileOpBlockedReason(
  game: GuardedGameStatus | null,
  operationRunning: boolean,
): string | null {
  if (game?.gameRunning) return strings.fileOps.blockedGame;
  if (game?.launching) return strings.fileOps.blockedStarting;
  if (game?.serverRunning) return strings.fileOps.blockedServer;
  if (operationRunning) return strings.fileOps.blockedRunning;
  return null;
}
