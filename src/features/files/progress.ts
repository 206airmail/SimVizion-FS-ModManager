import type { OpProgress } from '../../api';

/** How far a running operation is, 0 to 1: by bytes when it writes any, else by items. */
export function progressFraction(progress: OpProgress | null): number {
  if (progress === null) return 0;
  const fraction =
    progress.totalBytes > 0
      ? progress.doneBytes / progress.totalBytes
      : progress.totalItems > 0
        ? progress.doneItems / progress.totalItems
        : 0;
  return Math.min(1, Math.max(0, fraction));
}

/**
 * Where the latest progress of the running operation lives. Progress arrives many times a second;
 * keeping it outside React state means only the banner that shows it re-renders, not the screens.
 */
export interface ProgressStore {
  get: () => OpProgress | null;
  set: (progress: OpProgress | null) => void;
  subscribe: (listener: () => void) => () => void;
}

export function createProgressStore(): ProgressStore {
  let current: OpProgress | null = null;
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
