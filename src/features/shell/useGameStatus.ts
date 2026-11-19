import { useEffect, useState } from 'react';
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
 */
export function useGameStatus(): GameStatus | null {
  const [game, setGame] = useState<GameStatus | null>(null);

  useEffect(() => {
    let active = true;
    let reading = false;
    let timer: ReturnType<typeof setInterval> | undefined;

    const refresh = async () => {
      if (reading) return;
      reading = true;
      try {
        const next = await api.getGameStatus();
        // Returning the previous object keeps the screens from re-rendering for nothing.
        if (active) setGame((previous) => (sameStatus(previous, next) ? previous : next));
      } catch {
        // Keep showing the last known status; the next tick tries again.
      } finally {
        reading = false;
      }
    };
    const startPolling = () => {
      timer ??= setInterval(() => void refresh(), GAME_STATUS_INTERVAL_MS);
    };
    const stopPolling = () => {
      clearInterval(timer);
      timer = undefined;
    };
    const onVisibilityChange = () => {
      if (document.hidden) {
        stopPolling();
      } else {
        void refresh();
        startPolling();
      }
    };
    const onFocus = () => {
      if (!document.hidden) void refresh();
    };

    void refresh();
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

  return game;
}
