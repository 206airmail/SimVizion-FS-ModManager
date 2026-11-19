import { useCallback, useEffect, useRef, useState } from 'react';
import type { GameStatus } from '../../api';
import { api } from '../../api/client';
import { useActionError } from '../../lib/useActionError';
import { launchView, type LaunchView } from './launch';

/** The game takes a moment to show up as running; "Starting…" covers this long at most. */
export const STARTING_MS = 8000;
/** When the game status is read again after a launch (and at once). */
const RECHECK_MS = 2500;

export interface LaunchController {
  view: LaunchView;
  /** Starts the game; a failure is kept in `error` and the button returns to normal. */
  launch: () => void;
  /** The worded reason the last launch failed, until dismissed. */
  error: string | null;
  dismissError: () => void;
}

/**
 * The Launch game button's behaviour. A click asks the core to start the game, then reads the game
 * status at once and again shortly after, so the header sees it running; "Starting…" shows until
 * then (or until `STARTING_MS`, if the game never appears).
 */
export function useLaunch(
  game: GameStatus | null,
  refreshGame: () => Promise<void>,
): LaunchController {
  const [starting, setStarting] = useState(false);
  const failure = useActionError();
  const runLaunch = failure.run;
  // State lags a render behind; the ref stops a second click getting through.
  const busy = useRef(false);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);

  useEffect(() => {
    const pending = timers.current;
    return () => {
      for (const timer of pending) clearTimeout(timer);
    };
  }, []);

  const launch = useCallback(() => {
    if (busy.current) return;
    busy.current = true;
    setStarting(true);
    void (async () => {
      const started = await runLaunch(async () => {
        await api.launchGame();
        return true;
      });
      busy.current = false;
      if (!started) {
        setStarting(false);
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
    view: launchView(game, starting),
    launch,
    error: failure.message,
    dismissError: failure.dismiss,
  };
}
