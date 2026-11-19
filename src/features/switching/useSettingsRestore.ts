import { useCallback, useRef, useState } from 'react';
import type { GameStatus } from '../../api';
import { api } from '../../api/client';
import { useActionError } from '../../lib/useActionError';
import { useAsync } from '../../lib/useAsync';

export interface SettingsRestore {
  /** ISO time of the newest backup; set only while gameSettings.xml is unreadable and one exists. */
  offerFrom: string | null;
  busy: boolean;
  /** True after a restore succeeded, until dismissed. */
  restored: boolean;
  /** The worded reason the last restore failed, or null. */
  error: string | null;
  restore: () => Promise<void>;
  dismissError: () => void;
  dismissRestored: () => void;
}

const noBackup = () => Promise.resolve(null);

/**
 * The emergency restore offer: when the polled status says gameSettings.xml cannot be read, ask
 * the core for its newest backup. With no backup (or a failed lookup) nothing extra is offered;
 * the status bar already says the file is not readable. `reload` re-reads profiles and status.
 */
export function useSettingsRestore(
  game: GameStatus | null,
  reload: () => Promise<unknown>,
): SettingsRestore {
  const unreadable = game !== null && !game.settingsReadable;
  const lookup = useCallback(
    () => (unreadable ? api.latestSettingsBackup() : noBackup()),
    [unreadable],
  );
  const { state } = useAsync(lookup);
  const [busy, setBusy] = useState(false);
  const [restored, setRestored] = useState(false);
  const running = useRef(false);
  const failure = useActionError();
  const runAction = failure.run;

  const restore = useCallback(async () => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setRestored(false);
    try {
      const done = await runAction(async () => {
        await api.restoreSettingsBackup();
        return true;
      });
      if (done) {
        await reload();
        setRestored(true);
      }
    } finally {
      running.current = false;
      setBusy(false);
    }
  }, [runAction, reload]);

  return {
    offerFrom: unreadable && state.status === 'ready' ? state.data : null,
    busy,
    restored,
    error: failure.message,
    restore,
    dismissError: failure.dismiss,
    dismissRestored: useCallback(() => setRestored(false), []),
  };
}
