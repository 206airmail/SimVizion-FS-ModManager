import type { GameStatus } from '../../api';
import { strings } from '../../strings';

export interface LaunchView {
  /** The button says "Starting…" instead of "Launch game". */
  starting: boolean;
  /** Why the button cannot be used right now, or null when it can. */
  blockedReason: string | null;
}

/**
 * What the header's Launch game button shows. The running game comes first (it also ends the
 * "starting" state), then a click that is still being carried out, then an install the app
 * cannot start. Before the first status read nothing is known, so the button waits.
 */
export function launchView(game: GameStatus | null, starting: boolean): LaunchView {
  const t = strings.header;
  if (game === null) return { starting: false, blockedReason: t.launchChecking };
  if (game.gameRunning) return { starting: false, blockedReason: t.launchBlockedRunning };
  if (starting) return { starting: true, blockedReason: t.launchStartingReason };
  if (!game.launchable) return { starting: false, blockedReason: t.launchBlockedNone };
  return { starting: false, blockedReason: null };
}
