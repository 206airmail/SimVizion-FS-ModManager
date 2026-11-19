import { useCallback, useMemo, useRef, useState } from 'react';
import type { ClashChoice, OpProgress, OpResult, Profile, TransferPlan } from '../../api';
import { api } from '../../api/client';
import { ApiFailure, describeError } from '../../lib/errors';
import { strings } from '../../strings';
import { createProgressStore, type ProgressStore } from './progress';
import { emptyResult, type OpKind, type Outcome } from './summary';

export interface RunningOp {
  kind: OpKind;
  /** Name of the target folder for a copy or move; null for a remove. */
  target: string | null;
  /** Cancel was pressed; the operation ends after the current chunk. */
  cancelling: boolean;
}

export interface OpFailure {
  title: string;
  message: string;
}

export interface FileOpsController {
  /** The copy, move or remove in flight, if any. Only one runs at a time. */
  running: RunningOp | null;
  /** How the last one ended; kept until dismissed or the next one starts. */
  outcome: (Outcome & { id: number }) | null;
  /** The last one was refused or failed as a whole (the worded reason). */
  failure: OpFailure | null;
  /** Goes up whenever an operation ends, so screens know to read their lists again. */
  revision: number;
  /** The latest progress of the running operation; read it with `useSyncExternalStore`. */
  progress: ProgressStore;
  /** Carries out a plan the user confirmed. Returns at once; the banners show the rest. */
  startTransfer: (
    plan: TransferPlan,
    choices: ClashChoice[],
    targetName: string,
    replacedNames: string[],
  ) => void;
  startRemove: (profileId: string, modIds: string[]) => void;
  cancel: () => void;
  dismiss: () => void;
}

interface Run {
  kind: OpKind;
  target: string | null;
  replaced: string[];
  call: (onProgress: (progress: OpProgress) => void) => Promise<OpResult>;
}

/**
 * Copy, move and remove as the app sees them: one operation at a time, with its progress, its
 * outcome and its failure kept at app level so they survive browsing to another profile.
 * `reload` re-reads profiles and the game status afterwards and never rejects, so an operation
 * that finished is never reported as failed because the refresh did not (the M4 pattern).
 */
export function useFileOps(reload: () => Promise<Profile[] | null>): FileOpsController {
  const [running, setRunning] = useState<RunningOp | null>(null);
  const [outcome, setOutcome] = useState<FileOpsController['outcome']>(null);
  const [failure, setFailure] = useState<OpFailure | null>(null);
  const [revision, setRevision] = useState(0);
  const [progress] = useState(createProgressStore);
  // State lags a render behind; the ref is what stops a second start getting through.
  const busy = useRef(false);
  const outcomes = useRef(0);

  const run = useCallback(
    async ({ kind, target, replaced, call }: Run) => {
      if (busy.current) return;
      busy.current = true;
      progress.set(null);
      setOutcome(null);
      setFailure(null);
      setRunning({ kind, target, cancelling: false });
      // Messages can still arrive after the call settles; they must not revive the banner.
      let live = true;
      try {
        const result = await call((next) => {
          if (live) progress.set(next);
        });
        setOutcome({ id: ++outcomes.current, kind, target, result, replaced, cancelled: false });
      } catch (error) {
        if (error instanceof ApiFailure && error.code === 'cancelled') {
          setOutcome({
            id: ++outcomes.current,
            kind,
            target,
            result: emptyResult,
            replaced,
            cancelled: true,
          });
        } else {
          setFailure({ title: strings.actionFailed[kind], message: describeError(error) });
        }
      } finally {
        live = false;
        busy.current = false;
        progress.set(null);
        setRunning(null);
        // Files may have changed even when the call failed part-way: read everything again.
        setRevision((count) => count + 1);
        void reload();
      }
    },
    [progress, reload],
  );

  const startTransfer = useCallback<FileOpsController['startTransfer']>(
    (plan, choices, targetName, replaced) =>
      void run({
        kind: plan.mode,
        target: targetName,
        replaced,
        call: (onProgress) => api.executeTransfer(plan.planId, choices, onProgress),
      }),
    [run],
  );
  const startRemove = useCallback<FileOpsController['startRemove']>(
    (profileId, modIds) =>
      void run({
        kind: 'remove',
        target: null,
        replaced: [],
        call: (onProgress) => api.removeMods(profileId, modIds, onProgress),
      }),
    [run],
  );

  const cancel = useCallback(() => {
    setRunning((current) => current && { ...current, cancelling: true });
    api.cancelOperation().catch((error: unknown) =>
      setFailure({
        title: strings.actionFailed.cancelOperation,
        message: describeError(error),
      }),
    );
  }, []);
  const dismiss = useCallback(() => {
    setOutcome(null);
    setFailure(null);
  }, []);

  return useMemo(
    () => ({
      running,
      outcome,
      failure,
      revision,
      progress,
      startTransfer,
      startRemove,
      cancel,
      dismiss,
    }),
    [running, outcome, failure, revision, progress, startTransfer, startRemove, cancel, dismiss],
  );
}
