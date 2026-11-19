import { useCallback, useEffect, useRef, useState } from 'react';
import type { GameStatus } from '../../api';
import { api } from '../../api/client';

/** How often the game status is re-read while the window is visible. */
export const GAME_STATUS_INTERVAL_MS = 5000;

const sameStatus = (a: GameStatus | null, b: GameStatus) =>
  a !== null && JSON.stringify(a) === JSON.stringify(b);

/**
 * The live game status: read at once, every few seconds, and when the window comes back into
 * view or regains focus. Polling stops while the document is hidden. A refresh never shows a
 * loading state: the last known status stays until a new one differs, and a failed read keeps it.
 * `refresh` reads it now (after a write to gameSettings.xml); its result wins over any poll that
 * started earlier and is still in flight.
 */
export function useGameStatus(): { game: GameStatus | null; refresh: () => Promise<void> } {
  const [game, setGame] = useState<GameStatus | null>(null);
  const readNow = useRef<() => Promise<void>>(() => Promise.resolve());
  const refresh = useCallback(() => readNow.current(), []);

  useEffect(() => {
    let active = true;
    let reading = false;
    let latest = 0;
    let timer: ReturnType<typeof setInterval> | undefined;

    // A forced read runs even while a poll is in flight; only the newest read is applied.
    const read = async (force: boolean) => {
      if (reading && !force) return;
      const mine = ++latest;
      reading = true;
      try {
        const next = await api.getGameStatus();
        // Returning the previous object keeps the screens from re-rendering for nothing.
        if (active && mine === latest) {
          setGame((previous) => (sameStatus(previous, next) ? previous : next));
        }
      } catch {
        // Keep showing the last known status; the next tick tries again.
      } finally {
        if (mine === latest) reading = false;
      }
    };
    readNow.current = () => read(true);

    const startPolling = () => {
      timer ??= setInterval(() => void read(false), GAME_STATUS_INTERVAL_MS);
    };
    const stopPolling = () => {
      clearInterval(timer);
      timer = undefined;
    };
    const onVisibilityChange = () => {
      if (document.hidden) {
        stopPolling();
      } else {
        void read(false);
        startPolling();
      }
    };
    const onFocus = () => {
      if (!document.hidden) void read(false);
    };

    void read(false);
    if (!document.hidden) startPolling();
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('focus', onFocus);
    return () => {
      active = false;
      stopPolling();
      document.removeEventListener('visibilitychange', onVisibilityChange);
      window.removeEventListener('focus', onFocus);
    };
  }, []);

  return { game, refresh };
}
