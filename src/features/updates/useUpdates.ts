import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { ErrorCode, Profile, UpdateCheck, UpdateSettings } from '../../api';
import { api } from '../../api/client';
import { ApiFailure, describeError } from '../../lib/errors';
import { strings } from '../../strings';
import { deriveStatus, type UpdateStatus } from './status';

/** Where the first-run prompt remembers "Not now". Read and written only inside try/catch. */
const DISMISSED_KEY = 'fsmm.updatesPromptDismissed';

function readDismissed(): boolean {
  try {
    return window.localStorage.getItem(DISMISSED_KEY) === '1';
  } catch {
    return false;
  }
}

function writeDismissed(): void {
  try {
    window.localStorage.setItem(DISMISSED_KEY, '1');
  } catch {
    // Without storage the prompt simply comes back next time.
  }
}

export interface UpdateFailure {
  title: string;
  code: ErrorCode;
  message: string;
}

export interface UpdatesController {
  /** Null until the settings are read. */
  settings: UpdateSettings | null;
  /** What the status bar says; null until the settings are read. */
  status: UpdateStatus | null;
  /** ISO time the catalog asked to wait until, while that is still ahead. */
  pausedUntil: string | null;
  /** A check is running (the automatic one, "Check now", or the one that follows switching on). */
  checking: boolean;
  /** The latest check that reached an answer this session, from the cache or the catalog. */
  result: UpdateCheck | null;
  /** Why the last switch or check failed, until the next check succeeds. */
  failure: UpdateFailure | null;
  /** The user said "Not now" to the first-run prompt (which also offers the app updates). */
  promptDismissed: boolean;
  /** Goes up when a check changes what the lists show, so screens read their mods again. */
  revision: number;
  /** Switches online checks on (and runs the first check) or off. */
  setEnabled: (enabled: boolean) => void;
  /** A forced check (the core still answers from the cache within 60 s). */
  checkNow: () => void;
  dismissPrompt: () => void;
  dismissFailure: () => void;
}

export interface UseUpdatesOptions {
  /** The profiles loaded once: the automatic check waits for this. */
  ready: boolean;
  /** Reads profiles and the game status again (never rejects). */
  reload: () => Promise<Profile[] | null>;
}

/**
 * Online-update state at app level: the settings, the one automatic check per app start (when
 * enabled, after the first profile load), "Check now", and the first-run prompt. After a check
 * that changed what is shown, `reload` refreshes the profiles (counts) and `revision` tells the
 * open library to read its mods again.
 */
export function useUpdates({ ready, reload }: UseUpdatesOptions): UpdatesController {
  const [settings, setSettingsOnly] = useState<UpdateSettings | null>(null);
  // The clock as of the last change here: a retry-after is compared against it, and a timer
  // moves it when the wait ends. (Reading the time while rendering would not be pure.)
  const [now, setNow] = useState(Date.now);
  const setSettings = useCallback((next: UpdateSettings) => {
    setSettingsOnly(next);
    setNow(Date.now());
  }, []);
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<UpdateCheck | null>(null);
  const [failure, setFailure] = useState<UpdateFailure | null>(null);
  const [revision, setRevision] = useState(0);
  const [dismissed, setDismissed] = useState(readDismissed);
  // State lags a render behind; the ref stops a second check starting at once.
  const busy = useRef(false);
  const autoChecked = useRef(false);

  useEffect(() => {
    let live = true;
    api.getUpdateSettings().then(
      (loaded) => {
        if (live) setSettings(loaded);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [setSettings]);

  // The status leaves "paused" by itself when the wait ends.
  const retryAfter = settings?.retryAfter ?? null;
  useEffect(() => {
    if (retryAfter === null) return;
    const wait = Date.parse(retryAfter) - Date.now();
    if (!(wait > 0)) return;
    const timer = setTimeout(() => setNow(Date.now()), wait + 50);
    return () => clearTimeout(timer);
  }, [retryAfter]);

  const refreshAll = useCallback(async () => {
    await reload();
    setRevision((count) => count + 1);
  }, [reload]);

  const check = useCallback(
    async (force: boolean) => {
      if (busy.current) return;
      busy.current = true;
      setChecking(true);
      setFailure(null);
      let changed = false;
      try {
        setResult(await api.checkForUpdates(force));
        changed = true;
      } catch (error) {
        const code = error instanceof ApiFailure ? error.code : 'internal';
        setFailure({
          title: strings.actionFailed.checkUpdates,
          code,
          message: describeError(error),
        });
      }
      // The core keeps the time of the last check and any retry-after; read them back.
      const fresh = await api.getUpdateSettings().catch(() => null);
      if (fresh) setSettings(fresh);
      if (changed) await refreshAll();
      busy.current = false;
      setChecking(false);
    },
    [refreshAll, setSettings],
  );

  // The one automatic check per app start: when enabled, after the first profile load. It starts
  // from a timer so that nothing is set while the effect itself runs (and a strict-mode re-run
  // cancels the first start instead of doubling it).
  useEffect(() => {
    if (!ready || settings === null || autoChecked.current) return;
    const timer = setTimeout(() => {
      autoChecked.current = true;
      if (settings.enabled) void check(false);
    }, 0);
    return () => clearTimeout(timer);
  }, [ready, settings, check]);

  const setEnabled = useCallback(
    (enabled: boolean) => {
      void (async () => {
        try {
          const next = await api.setUpdatesEnabled(enabled);
          setSettings(next);
          setFailure(null);
          if (enabled) {
            await check(false);
          } else {
            setResult(null);
            await refreshAll();
          }
        } catch (error) {
          setFailure({
            title: strings.actionFailed.setUpdates,
            code: error instanceof ApiFailure ? error.code : 'internal',
            message: describeError(error),
          });
        }
      })();
    },
    [check, refreshAll, setSettings],
  );

  const checkNow = useCallback(() => void check(true), [check]);
  const dismissPrompt = useCallback(() => {
    writeDismissed();
    setDismissed(true);
  }, []);
  const dismissFailure = useCallback(() => setFailure(null), []);

  const failureCode = failure?.code ?? null;
  const status = useMemo(
    () => deriveStatus({ settings, checking, failure: failureCode }, now),
    [settings, checking, failureCode, now],
  );
  const pausedUntil = status?.kind === 'paused' ? status.until : null;

  return useMemo(
    () => ({
      settings,
      status,
      pausedUntil,
      checking,
      result,
      failure,
      promptDismissed: dismissed,
      revision,
      setEnabled,
      checkNow,
      dismissPrompt,
      dismissFailure,
    }),
    [
      settings,
      status,
      pausedUntil,
      checking,
      result,
      failure,
      dismissed,
      revision,
      setEnabled,
      checkNow,
      dismissPrompt,
      dismissFailure,
    ],
  );
}
