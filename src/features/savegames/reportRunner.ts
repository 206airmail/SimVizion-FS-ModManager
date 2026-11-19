// One savegame check, in its two phases: the report at once, then (if some rows still need a file
// hash) `checkSavegameFiles` with progress. The screen starts a run whenever the save, the folder
// or the files change, and stops the old one; a stopped run never delivers anything, so a slow
// result for a previous save or folder cannot overwrite the current one.

import type { Api, HashProgress, SavegameReport } from '../../api';
import { ApiFailure } from '../../lib/errors';

/** How the file check stands. `cancelled` and `failed` keep the first report on screen. */
export type HashPhase = 'idle' | 'running' | 'cancelled' | 'failed';

export interface RunEvents {
  /** The report: the first one (maybe with pending rows), then the final one. */
  onReport: (report: SavegameReport) => void;
  /** Reading the report failed; there is nothing to show. */
  onReportFailed: (error: unknown) => void;
  onHash: (phase: HashPhase, error?: unknown) => void;
  onProgress: (progress: HashProgress) => void;
}

export interface RunHandle {
  /** Ends the run: nothing more is delivered, and a hash in flight is cancelled in the core. */
  stop: () => void;
  /** Resolves when the run has ended (for tests). */
  done: Promise<void>;
}

type RunApi = Pick<Api, 'savegameReport' | 'checkSavegameFiles' | 'cancelHashing'>;

/**
 * The core allows one hash at a time (`busy`), and a cancelled one takes a moment to unwind. A new
 * run waits for the last check to end before it starts its own.
 */
let lastCheck: Promise<unknown> = Promise.resolve();

export function startReportRun(
  api: RunApi,
  savegameId: string,
  profileId: string,
  events: RunEvents,
): RunHandle {
  let live = true;
  let hashing = false;

  const run = async (): Promise<void> => {
    let first: SavegameReport;
    try {
      first = await api.savegameReport(savegameId, profileId);
    } catch (error) {
      if (live) events.onReportFailed(error);
      return;
    }
    if (!live) return;
    events.onReport(first);
    if (!first.rows.some((row) => row.state === 'hashPending')) {
      events.onHash('idle');
      return;
    }
    events.onHash('running');
    await lastCheck.catch(() => undefined);
    if (!live) return;

    hashing = true;
    const call = api.checkSavegameFiles(savegameId, profileId, (progress) => {
      if (live) events.onProgress(progress);
    });
    lastCheck = call.catch(() => undefined);
    try {
      const final = await call;
      if (!live) return;
      events.onReport(final);
      events.onHash('idle');
    } catch (error) {
      if (!live) return;
      events.onHash(
        error instanceof ApiFailure && error.code === 'cancelled' ? 'cancelled' : 'failed',
        error,
      );
    } finally {
      hashing = false;
    }
  };

  return {
    stop: () => {
      live = false;
      if (hashing) api.cancelHashing().catch(() => undefined);
    },
    done: run(),
  };
}
