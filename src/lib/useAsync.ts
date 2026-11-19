import { useCallback, useEffect, useState } from 'react';

export type AsyncState<T> =
  { status: 'loading' } | { status: 'ready'; data: T } | { status: 'error'; error: unknown };

type Settled<T> = Exclude<AsyncState<T>, { status: 'loading' }>;

/** Called by a loader to report how far it has got; ignored once the request is superseded. */
export type Report<P> = (progress: P) => void;

type Loader<T, P> = (report: Report<P>) => Promise<T>;

/**
 * Runs `load` and reports its state. Pass a function with a stable identity (useCallback):
 * a new identity means a new request, and the previous result stops being shown at once.
 * `error` is whatever the loader threw; word it with `describeError`. A loader may stream
 * progress through `report`; `progress` is the latest value of the current request, else null.
 */
export function useAsync<T, P = never>(
  load: Loader<T, P>,
): {
  state: AsyncState<T>;
  progress: P | null;
  reload: () => void;
  /** Replaces the loaded data without showing the loading state. */
  replace: (data: T) => void;
  /** Changes the loaded data in place (no loading state); does nothing until it is loaded. */
  update: (change: (data: T) => T) => void;
} {
  const [attempt, setAttempt] = useState(0);
  const [settled, setSettled] = useState<{
    load: Loader<T, P>;
    attempt: number;
    result: Settled<T>;
  } | null>(null);
  const [reported, setReported] = useState<{
    load: Loader<T, P>;
    attempt: number;
    value: P;
  } | null>(null);

  useEffect(() => {
    let cancelled = false;
    const report: Report<P> = (value) => {
      if (!cancelled) setReported({ load, attempt, value });
    };
    load(report).then(
      (data) => {
        if (!cancelled) setSettled({ load, attempt, result: { status: 'ready', data } });
      },
      (error: unknown) => {
        if (!cancelled) setSettled({ load, attempt, result: { status: 'error', error } });
      },
    );
    return () => {
      cancelled = true;
    };
  }, [load, attempt]);

  const reload = useCallback(() => setAttempt((count) => count + 1), []);
  const replace = useCallback(
    (data: T) => setSettled({ load, attempt, result: { status: 'ready', data } }),
    [load, attempt],
  );
  const update = useCallback(
    (change: (data: T) => T) =>
      setSettled((previous) =>
        previous !== null &&
        previous.load === load &&
        previous.attempt === attempt &&
        previous.result.status === 'ready'
          ? { ...previous, result: { status: 'ready', data: change(previous.result.data) } }
          : previous,
      ),
    [load, attempt],
  );
  const isCurrent = settled !== null && settled.load === load && settled.attempt === attempt;
  const isReportCurrent =
    reported !== null && reported.load === load && reported.attempt === attempt;
  return {
    state: isCurrent ? settled.result : { status: 'loading' },
    progress: isReportCurrent ? reported.value : null,
    reload,
    replace,
    update,
  };
}
