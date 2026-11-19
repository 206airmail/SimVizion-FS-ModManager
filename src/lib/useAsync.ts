import { useCallback, useEffect, useState } from 'react';

export type AsyncState<T> =
  { status: 'loading' } | { status: 'ready'; data: T } | { status: 'error'; message: string };

type Settled<T> = Exclude<AsyncState<T>, { status: 'loading' }>;

/**
 * Runs `load` and reports its state. Pass a function with a stable identity (useCallback):
 * a new identity means a new request, and the previous result stops being shown at once.
 */
export function useAsync<T>(load: () => Promise<T>): { state: AsyncState<T>; reload: () => void } {
  const [attempt, setAttempt] = useState(0);
  const [settled, setSettled] = useState<{
    load: () => Promise<T>;
    attempt: number;
    result: Settled<T>;
  } | null>(null);

  useEffect(() => {
    let cancelled = false;
    load().then(
      (data) => {
        if (!cancelled) setSettled({ load, attempt, result: { status: 'ready', data } });
      },
      (error: unknown) => {
        const message = error instanceof Error ? error.message : String(error);
        if (!cancelled) setSettled({ load, attempt, result: { status: 'error', message } });
      },
    );
    return () => {
      cancelled = true;
    };
  }, [load, attempt]);

  const reload = useCallback(() => setAttempt((count) => count + 1), []);
  const isCurrent = settled !== null && settled.load === load && settled.attempt === attempt;
  return { state: isCurrent ? settled.result : { status: 'loading' }, reload };
}
