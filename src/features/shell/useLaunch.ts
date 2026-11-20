import { useCallback, useEffect, useRef, useState } from 'react';
import type { GameStatus } from '../../api';
import { api } from '../../api/client';
import { useActionError } from '../../lib/useActionError';
import { launchView, type LaunchView } from './launch';

/** The game takes a moment to show up as running; "Starting…" covers this long at most. */
export const STARTING_MS = 8000;
/**
 * Steam can take far longer than that to start the game process. Until the process shows up in the
 * status, or this long has passed, the write blockers treat the game as starting.
 */
export const LAUNCH_GUARD_MS = 60_000;
/** When the game status is read again after a launch (and at once). */
const RECHECK_MS = 2500;

export interface LaunchController {
  view: LaunchView;
  /**
   * A launch was requested and the game is not yet seen running: file writes stay blocked (see
   * `withLaunching`). Ends when the game appears or after `LAUNCH_GUARD_MS`.
   */
  launching: boolean;
  /** Starts the game; a failure is kept in `error` and the button returns to normal. */
  launch: () => void;
  /** The worded reason the last launch failed, until dismissed. */
  error: string | null;
  dismissError: () => void;
}

/**
 * The Launch game button's behaviour. A click asks the core to start the game, then reads the game
 * status at once and again shortly after, so the header sees it running; "Starting…" shows until
 * then (or until `STARTING_MS`, if the game never appears). `writeReason` is why the app's own
 * writes (a file operation, a profile switch) forbid starting the game right now, or null. From
 * the click until the game is seen running (or `LAUNCH_GUARD_MS`), `launching` tells the write
 * blockers the game is on its way.
 */
export function useLaunch(
  game: GameStatus | null,
  refreshGame: () => Promise<void>,
  writeReason: string | null,
): LaunchController {
  const [starting, setStarting] = useState(false);
  const [launching, setLaunching] = useState(false);
  const failure = useActionError();
  const runLaunch = failure.run;
  // State lags a render behind; the ref stops a second click getting through.
  const busy = useRef(false);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const guardTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    const pending = timers.current;
    return () => {
      for (const timer of pending) clearTimeout(timer);
      clearTimeout(guardTimer.current);
    };
  }, []);

  // Seen running: the status itself blocks writes from here on, so the guard has done its job.
  if (launching && game?.gameRunning) setLaunching(false);

  const launch = useCallback(() => {
    if (busy.current) return;
    busy.current = true;
    setStarting(true);
    setLaunching(true);
    clearTimeout(guardTimer.current);
    guardTimer.current = setTimeout(() => setLaunching(false), LAUNCH_GUARD_MS);
    void (async () => {
      const started = await runLaunch(async () => {
        await api.launchGame();
        return true;
      });
      busy.current = false;
      if (!started) {
        setStarting(false);
        setLaunching(false);
        clearTimeout(guardTimer.current);
        return;
      }
      void refreshGame();
      timers.current.push(
        setTimeout(() => void refreshGame(), RECHECK_MS),
        setTimeout(() => setStarting(false), STARTING_MS),
      );
    })();
  }, [runLaunch, refreshGame]);

  return {
    view: launchView(game, starting, writeReason),
    launching,
    launch,
    error: failure.message,
    dismissError: failure.dismiss,
  };
}
