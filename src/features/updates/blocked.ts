import { strings } from '../../strings';
import { fileOpBlockedReason } from '../files/blocked';
import type { GuardedGameStatus } from '../shell/launch';

/**
 * Why the Update buttons are unavailable right now, or null when they are: everything that blocks
 * a file operation, and a running update check (it holds the update service, so an update started
 * meanwhile would be refused as busy).
 */
export function updateBlockedReason(
  game: GuardedGameStatus | null,
  operationRunning: boolean,
  checking: boolean,
): string | null {
  return (
    fileOpBlockedReason(game, operationRunning) ??
    (checking ? strings.updates.blockedChecking : null)
  );
}
