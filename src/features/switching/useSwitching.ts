import { useCallback, useRef, useState } from 'react';
import type { ErrorCode, Profile, SwitchResult } from '../../api';
import { api } from '../../api/client';
import { ApiFailure, describeError } from '../../lib/errors';
import { strings } from '../../strings';

/** What a finished switch or undo left on screen. `scope` is the profile it happened in. */
export type SwitchDone =
  | { kind: 'switched'; scope: string; result: SwitchResult; name: string }
  | { kind: 'undone'; scope: string; name: string | null };

export interface SwitchFailure {
  scope: string;
  title: string;
  message: string;
}

export interface SwitchingController {
  /** Which write is in flight, if any. Only one runs at a time. */
  busy: 'switch' | 'undo' | null;
  done: SwitchDone | null;
  failure: SwitchFailure | null;
  setActive: (profile: Profile) => Promise<void>;
  undo: () => Promise<void>;
  /** Hides the banners, e.g. when the user moves to another profile. */
  clear: () => void;
  dismissFailure: () => void;
}

/** After an undo with these codes, the switch can never be undone, so Undo is no longer offered. */
const NOT_UNDOABLE: readonly ErrorCode[] = ['nothingToUndo', 'settingsChanged'];

/**
 * "Set as active" and its Undo. `reload` re-reads profiles and the game status after a write and
 * returns the fresh profiles (null if they could not be read); it never rejects, so a write that
 * succeeded is never reported as failed because the refresh did not.
 */
export function useSwitching(reload: () => Promise<Profile[] | null>): SwitchingController {
  const [busy, setBusy] = useState<SwitchingController['busy']>(null);
  const [done, setDone] = useState<SwitchDone | null>(null);
  const [failure, setFailure] = useState<SwitchFailure | null>(null);
  // State lags a render behind; the ref is what stops a second press getting through.
  const running = useRef(false);

  const run = useCallback(
    async (
      kind: 'switch' | 'undo',
      action: () => Promise<void>,
      fail: (error: unknown) => void,
    ) => {
      if (running.current) return;
      running.current = true;
      setBusy(kind);
      setFailure(null);
      try {
        await action();
      } catch (error) {
        fail(error);
      } finally {
        running.current = false;
        setBusy(null);
      }
    },
    [],
  );

  const setActive = useCallback(
    (profile: Profile) =>
      run(
        'switch',
        async () => {
          setDone(null);
          const result = await api.setActiveProfile(profile.id);
          await reload();
          setDone({ kind: 'switched', scope: profile.id, result, name: profile.name });
        },
        (error) =>
          setFailure({
            scope: profile.id,
            title: strings.actionFailed.setActive,
            message: describeError(error),
          }),
      ),
    [run, reload],
  );

  const undo = useCallback(() => {
    if (done?.kind !== 'switched') return Promise.resolve();
    const { scope, result } = done;
    return run(
      'undo',
      async () => {
        const undone = await api.undoSwitch(result.opId);
        const profiles = await reload();
        const name = profiles?.find((p) => p.id === undone.toProfileId)?.name ?? null;
        setDone({ kind: 'undone', scope, name });
      },
      (error) => {
        setFailure({
          scope,
          title: strings.actionFailed.undoSwitch,
          message: describeError(error),
        });
        if (error instanceof ApiFailure && NOT_UNDOABLE.includes(error.code)) setDone(null);
      },
    );
  }, [done, run, reload]);

  const clear = useCallback(() => {
    setDone(null);
    setFailure(null);
  }, []);
  const dismissFailure = useCallback(() => setFailure(null), []);

  return { busy, done, failure, setActive, undo, clear, dismissFailure };
}
