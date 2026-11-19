import { useCallback, useEffect, useMemo, useState } from 'react';
import type { HashProgress, SavegameReport } from '../../api';
import { api } from '../../api/client';
import { startReportRun, type HashPhase } from './reportRunner';

/** The latest hash progress, kept outside React state: it arrives many times a second. */
export interface HashProgressStore {
  get: () => HashProgress | null;
  set: (progress: HashProgress | null) => void;
  subscribe: (listener: () => void) => () => void;
}

function createHashProgressStore(): HashProgressStore {
  let current: HashProgress | null = null;
  const listeners = new Set<() => void>();
  return {
    get: () => current,
    set: (progress) => {
      current = progress;
      for (const listener of listeners) listener();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

export interface SavegameReportController {
  status: 'loading' | 'ready' | 'error';
  /** The report on screen: the first (with pending rows), then the final one. */
  report: SavegameReport | null;
  /** Why the report could not be read (status `error`). */
  error: unknown;
  /** How the file check stands. */
  hash: { phase: HashPhase; error: unknown; cancelling: boolean };
  progress: HashProgressStore;
  /** Stops the running file check; the first report stays. */
  cancel: () => void;
  /** Runs the whole check again (versions, then files). */
  recheck: () => void;
}

interface View {
  key: string;
  report: SavegameReport | null;
  error: unknown;
  phase: HashPhase;
  phaseError: unknown;
  cancelling: boolean;
}

const noView: View = {
  key: '',
  report: null,
  error: null,
  phase: 'idle',
  phaseError: null,
  cancelling: false,
};

/**
 * A savegame checked against a folder: the report at once, then the file check with progress.
 * It starts again whenever the save or the folder changes (and the old run's late results are
 * dropped, see `startReportRun`), and when `revision` goes up (a file operation ended), keeping
 * the report on screen until the new one arrives.
 */
export function useSavegameReport(
  savegameId: string | null,
  profileId: string | null,
  revision: number,
): SavegameReportController {
  const [view, setView] = useState<View>(noView);
  const [attempt, setAttempt] = useState(0);
  const [progress] = useState(createHashProgressStore);
  const key = savegameId !== null && profileId !== null ? `${savegameId}|${profileId}` : '';

  useEffect(() => {
    if (savegameId === null || profileId === null) return;
    const patch = (change: Partial<View>) =>
      setView((previous) => ({
        ...(previous.key === key ? previous : { ...noView, key }),
        ...change,
      }));
    const run = startReportRun(api, savegameId, profileId, {
      onReport: (report) => patch({ report, error: null }),
      // A failed re-read keeps the report that is on screen.
      onReportFailed: (error) => patch({ error }),
      onHash: (phase, error) => {
        if (phase !== 'running') progress.set(null);
        patch({ phase, phaseError: error ?? null, cancelling: false });
      },
      onProgress: (next) => progress.set(next),
    });
    return () => {
      run.stop();
      progress.set(null);
    };
    // `revision` and `attempt` are the triggers: a file operation ended, or "check again" was pressed.
  }, [savegameId, profileId, key, revision, attempt, progress]);

  const cancel = useCallback(() => {
    setView((previous) => ({ ...previous, cancelling: true }));
    api.cancelHashing().catch(() => undefined);
  }, []);
  const recheck = useCallback(() => setAttempt((count) => count + 1), []);

  // A view that belongs to another save or folder counts as nothing: never show it.
  const current = view.key === key ? view : noView;
  return useMemo(
    () => ({
      status: current.report ? 'ready' : current.error !== null ? 'error' : 'loading',
      report: current.report,
      error: current.error,
      hash: { phase: current.phase, error: current.phaseError, cancelling: current.cancelling },
      progress,
      cancel,
      recheck,
    }),
    [current, progress, cancel, recheck],
  );
}
