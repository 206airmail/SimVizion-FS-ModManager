import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { AppUpdate, AppUpdateSettings, ErrorCode } from '../../api';
import { api } from '../../api/client';
import { ApiFailure, describeError } from '../../lib/errors';
import { strings } from '../../strings';
import { createProgressStore, type ProgressStore } from '../files/progress';
import { bannerVisible, type InstallState } from './state';

export interface AppUpdateFailure {
  title: string;
  code: ErrorCode;
  message: string;
}

export interface AppUpdatesController {
  /** Null until the settings are read. */
  settings: AppUpdateSettings | null;
  /** A check is running (the startup one, "Check now", or the one that follows switching on). */
  checking: boolean;
  /** The update found this session, if any. */
  found: AppUpdate | null;
  /** A "Check now" ended with nothing newer; cleared by the next check. */
  upToDate: boolean;
  /** The banner is on screen: an update was found and the user has not put it off. */
  bannerVisible: boolean;
  install: InstallState;
  /** Download progress of the running install; read it with `useSyncExternalStore`. */
  progress: ProgressStore;
  /** Why the last check, switch or install failed. Quiet: Settings shows it, nothing blocks. */
  failure: AppUpdateFailure | null;
  setEnabled: (enabled: boolean) => void;
  /** A forced check. */
  checkNow: () => void;
  /** Downloads, verifies and installs; the app restarts by itself when it succeeds. */
  installNow: () => void;
  /** Hides the banner for this session. */
  later: () => void;
  dismissFailure: () => void;
}

export interface UseAppUpdatesOptions {
  /** The first load is done: the one startup check waits for this. */
  ready: boolean;
}

const codeOf = (error: unknown): ErrorCode =>
  error instanceof ApiFailure ? error.code : 'internal';

/**
 * The app's own update state: the settings, the one startup check per run (when enabled, after the
 * first load; the core answers null when it checked less than a day ago), "Check now", and the
 * install with its progress. Failures never interrupt: they are kept for Settings to show.
 */
export function useAppUpdates({ ready }: UseAppUpdatesOptions): AppUpdatesController {
  const [settings, setSettings] = useState<AppUpdateSettings | null>(null);
  const [checking, setChecking] = useState(false);
  const [found, setFound] = useState<AppUpdate | null>(null);
  const [upToDate, setUpToDate] = useState(false);
  const [hiddenVersion, setHiddenVersion] = useState<string | null>(null);
  const [install, setInstall] = useState<InstallState>('idle');
  const [failure, setFailure] = useState<AppUpdateFailure | null>(null);
  const [progress] = useState(createProgressStore);
  // State lags a render behind; the refs stop a second check or install starting at once.
  const checkBusy = useRef(false);
  const installBusy = useRef(false);
  const autoChecked = useRef(false);

  useEffect(() => {
    let live = true;
    api.getAppUpdateSettings().then(
      (loaded) => {
        if (live) setSettings(loaded);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);

  const check = useCallback(async (force: boolean) => {
    if (checkBusy.current) return;
    checkBusy.current = true;
    setChecking(true);
    setFailure(null);
    setUpToDate(false);
    try {
      const update = await api.checkAppUpdate(force);
      if (update !== null) {
        setFound(update);
        // A forced check that finds the update again brings back a banner put off earlier.
        if (force) setHiddenVersion(null);
      } else if (force) {
        // Without force, null can also mean "asked less than a day ago": say nothing then.
        setFound(null);
        setUpToDate(true);
      }
    } catch (error) {
      setFailure({
        title: strings.actionFailed.checkAppUpdate,
        code: codeOf(error),
        message: describeError(error),
      });
    }
    // The core keeps the time of the last check; read it back.
    const fresh = await api.getAppUpdateSettings().catch(() => null);
    if (fresh) setSettings(fresh);
    checkBusy.current = false;
    setChecking(false);
  }, []);

  // The one startup check: when enabled, after the first load. It starts from a timer so nothing
  // is set while the effect itself runs (and a strict-mode re-run cancels the first start).
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
          setSettings(await api.setAppUpdatesEnabled(enabled));
          setFailure(null);
          if (enabled) {
            await check(false);
          } else {
            setFound(null);
            setUpToDate(false);
          }
        } catch (error) {
          setFailure({
            title: strings.actionFailed.setAppUpdates,
            code: codeOf(error),
            message: describeError(error),
          });
        }
      })();
    },
    [check],
  );

  const checkNow = useCallback(() => void check(true), [check]);

  const installNow = useCallback(() => {
    if (installBusy.current) return;
    installBusy.current = true;
    setInstall('installing');
    setFailure(null);
    progress.set(null);
    void (async () => {
      try {
        await api.installAppUpdate((next) => progress.set(next));
        // The app restarts by itself; until it does the banner keeps saying so.
      } catch (error) {
        setInstall('failed');
        setFailure({
          title: strings.actionFailed.installAppUpdate,
          code: codeOf(error),
          message: describeError(error),
        });
        progress.set(null);
        installBusy.current = false;
      }
    })();
  }, [progress]);

  const later = useCallback(() => setHiddenVersion(found?.version ?? null), [found]);
  const dismissFailure = useCallback(() => setFailure(null), []);

  const enabled = settings?.enabled === true;
  const visible = bannerVisible(found, enabled, hiddenVersion);

  return useMemo(
    () => ({
      settings,
      checking,
      found,
      upToDate,
      bannerVisible: visible,
      install,
      progress,
      failure,
      setEnabled,
      checkNow,
      installNow,
      later,
      dismissFailure,
    }),
    [
      settings,
      checking,
      found,
      upToDate,
      visible,
      install,
      progress,
      failure,
      setEnabled,
      checkNow,
      installNow,
      later,
      dismissFailure,
    ],
  );
}
