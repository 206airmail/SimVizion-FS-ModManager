import { useCallback, useMemo, useRef, useState } from 'react';
import { api } from '../../api/client';
import { describeError } from '../../lib/errors';
import { strings } from '../../strings';

export interface GameFolderFailure {
  title: string;
  message: string;
}

export interface GameFolderController {
  /** The picker is open or the folder is being changed. */
  busy: boolean;
  failure: GameFolderFailure | null;
  /** Opens the picker; a cancel changes nothing. Reads profiles and the status again after a change. */
  choose: () => void;
  /** Forgets the chosen folder so detection decides again. */
  useDetected: () => void;
  dismiss: () => void;
}

/**
 * Choosing the game's user folder, and going back to the detected one. `afterChange` reads the
 * profiles and the game status again (and never rejects), since both depend on that folder.
 */
export function useGameFolder(afterChange: () => Promise<unknown>): GameFolderController {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<GameFolderFailure | null>(null);
  // State lags a render behind; the ref stops a second picker opening at once.
  const running = useRef(false);

  const run = useCallback(
    async (title: string, action: () => Promise<boolean>) => {
      if (running.current) return;
      running.current = true;
      setBusy(true);
      setFailure(null);
      try {
        if (await action()) await afterChange();
      } catch (error) {
        setFailure({ title, message: describeError(error) });
      } finally {
        running.current = false;
        setBusy(false);
      }
    },
    [afterChange],
  );

  const choose = useCallback(
    () =>
      void run(strings.actionFailed.chooseGameFolder, async () => {
        // Null means the user cancelled the picker: nothing changed, nothing to reload.
        return (await api.chooseGameFolder()) !== null;
      }),
    [run],
  );
  const useDetected = useCallback(
    () =>
      void run(strings.actionFailed.clearGameFolder, async () => {
        await api.clearGameFolder();
        return true;
      }),
    [run],
  );
  const dismiss = useCallback(() => setFailure(null), []);

  return useMemo(
    () => ({ busy, failure, choose, useDetected, dismiss }),
    [busy, failure, choose, useDetected, dismiss],
  );
}
