import { FolderX } from 'lucide-react';
import { lazy, Suspense, useCallback, useMemo, useRef, useState } from 'react';
import type { Profile } from './api';
import { api } from './api/client';
import { ActionErrorBanner } from './features/ActionErrorBanner';
import { AppUpdateBanner } from './features/appUpdate/AppUpdateBanner';
import { useAppUpdates } from './features/appUpdate/useAppUpdates';
import { CompareScreen } from './features/compare/CompareScreen';
import { FileOpsNotices } from './features/files/FileOpsNotices';
import { useFileOps } from './features/files/useFileOps';
import { useGameFolder } from './features/gameFolder/useGameFolder';
import { HealthScreen } from './features/health/HealthScreen';
import { LibraryScreen, type LibraryFocus } from './features/library/LibraryScreen';
import { SavegamesScreen } from './features/savegames/SavegamesScreen';
import { AppHeader } from './features/shell/AppHeader';
import { ProfileRail, type ProfileActions, type Section } from './features/shell/ProfileRail';
import { StatusBar } from './features/shell/StatusBar';
import { useGameStatus } from './features/shell/useGameStatus';
import { withLaunching } from './features/shell/launch';
import { useLaunch } from './features/shell/useLaunch';
import { SettingsRestoreBanner } from './features/switching/SettingsRestoreBanner';
import { useSettingsRestore } from './features/switching/useSettingsRestore';
import { useSwitching } from './features/switching/useSwitching';
import { SettingsScreen } from './features/updates/SettingsScreen';
import { useUpdates } from './features/updates/useUpdates';
import { workspaceView } from './features/welcome/view';
import { WelcomeScreen } from './features/welcome/WelcomeScreen';
import { describeError } from './lib/errors';
import { useActionError } from './lib/useActionError';
import { useAsync } from './lib/useAsync';
import { strings } from './strings';
import { Button } from './ui/Button';
import { EmptyState } from './ui/Feedback';

const loadProfiles = () => api.listProfiles();

/**
 * `#gallery` shows every primitive in every state, for design review. It is loaded on demand and
 * only where `__SAMPLE_BUILD__` is true, so the release bundle holds neither it nor its styles.
 */
const Gallery = __SAMPLE_BUILD__
  ? lazy(() => import('./features/gallery/Gallery').then((module) => ({ default: module.Gallery })))
  : null;
const showGallery = Gallery !== null && window.location.hash === '#gallery';

export function App() {
  return showGallery ? (
    <Suspense fallback={null}>
      <Gallery />
    </Suspense>
  ) : (
    <Workspace />
  );
}

function Workspace() {
  // Profiles and the game status load separately: the status refreshes on its own schedule and
  // must never reset the profile list or the selection.
  const { state, reload, replace } = useAsync(loadProfiles);
  const { game, refresh: refreshGame } = useGameStatus();
  const addFolder = useActionError();
  const runAddFolder = addFolder.run;
  const [section, setSection] = useState<Section>('library');
  const [chosenProfileId, setChosenProfileId] = useState<string | null>(null);
  // The folder on the left when Compare opens ("Compare with…" on a profile, or the shown one).
  const [compareLeftId, setCompareLeftId] = useState<string | null>(null);

  // After a write to gameSettings.xml: fresh profiles (the ACTIVE marks) and game status. Neither
  // read may fail the write that already succeeded, so this never rejects.
  const reloadAfterWrite = useCallback(async (): Promise<Profile[] | null> => {
    const [fresh] = await Promise.all([api.listProfiles().catch(() => null), refreshGame()]);
    if (fresh) replace(fresh);
    return fresh;
  }, [replace, refreshGame]);
  const switching = useSwitching(reloadAfterWrite);
  const clearSwitching = switching.clear;
  const settingsRestore = useSettingsRestore(game, reloadAfterWrite);
  // Copy, move and remove live here, not in a screen: their progress banner must stay while the
  // user browses other profiles.
  const fileOps = useFileOps(reloadAfterWrite);
  // Online update checks: the automatic one runs once, after the profiles first load.
  const updates = useUpdates({ ready: state.status === 'ready', reload: reloadAfterWrite });
  // The app's own updates: the same once-per-start check, with its banner above the screens.
  const appUpdates = useAppUpdates({ ready: state.status === 'ready' });
  // Launch game, and the game's user folder (chosen on the first-run screen or in Settings).
  // Launch waits for a write in progress (a file operation or a switch) to finish.
  const launch = useLaunch(
    game,
    refreshGame,
    fileOps.running !== null
      ? strings.header.launchBlockedFileOp
      : switching.busy !== null
        ? strings.header.launchBlockedSwitch
        : null,
  );
  // What the screens' write blockers read: the game status, plus "the game is starting" between a
  // press of Launch game and the process showing up.
  const guardedGame = useMemo(
    () => withLaunching(game, launch.launching),
    [game, launch.launching],
  );
  const gameFolder = useGameFolder(reloadAfterWrite);
  // A mod the Health screen asked the library to bring up; `id` tells one request from the next.
  const [libraryFocus, setLibraryFocus] = useState<LibraryFocus | null>(null);
  const focusRequests = useRef(0);
  const clearLibraryFocus = useCallback(() => setLibraryFocus(null), []);

  const selectProfile = useCallback(
    (id: string) => {
      // A switch banner belongs to the profile it happened in; leaving that profile dismisses it.
      clearSwitching();
      setChosenProfileId(id);
      setSection('library');
    },
    [clearSwitching],
  );

  const openCompare = useCallback((leftId: string | null) => {
    setCompareLeftId(leftId);
    setSection('compare');
  }, []);

  const openMod = useCallback(
    (profileId: string, modId: string) => {
      clearSwitching();
      setChosenProfileId(profileId);
      setSection('library');
      setLibraryFocus({ id: ++focusRequests.current, profileId, modId });
    },
    [clearSwitching],
  );

  const addProfile = useCallback(
    () =>
      runAddFolder(async () => {
        const added = await api.addProfile();
        if (!added) return;
        // Swap the list in place so the rail and the open library never blank out.
        replace(await api.listProfiles());
        selectProfile(added.id);
      }),
    [runAddFolder, replace, selectProfile],
  );

  const profiles = state.status === 'ready' ? state.data : [];
  const view = workspaceView(state.status === 'ready', profiles.length, game);
  const activeProfile = profiles.find((profile) => profile.isActive) ?? null;
  // Until the user picks one, show the profile the game is using.
  const profile =
    profiles.find((candidate) => candidate.id === chosenProfileId) ??
    activeProfile ??
    profiles[0] ??
    null;

  // Profile management only edits the app's own list, so the game does not gate it; a running
  // file operation does gate removal, which might take away a folder it is working in.
  const operationRunning = fileOps.running !== null;
  const profileActions: ProfileActions = {
    compareWith: (target) => openCompare(target.id),
    rename: async (target, name) => {
      await api.renameProfile(target.id, name);
      await reloadAfterWrite();
    },
    remove: async (target) => {
      await api.removeProfile(target.id);
      const fresh = await reloadAfterWrite();
      // The folder on screen is gone: show the one the game uses instead.
      if (target.id === profile?.id) {
        setChosenProfileId(fresh?.find((candidate) => candidate.isActive)?.id ?? null);
      }
    },
    removeBlockedReason: (target) =>
      target.isActive
        ? strings.profileMenu.blockedActive
        : target.isGameDefault
          ? strings.profileMenu.blockedDefault
          : operationRunning
            ? strings.profileMenu.blockedRunning
            : null,
  };

  return (
    <div className="mm-app">
      <AppHeader activeProfile={activeProfile} game={game} launch={launch} />

      <div className="mm-workspace">
        <ProfileRail
          profiles={profiles}
          selectedProfileId={profile?.id ?? null}
          section={section}
          onSelectProfile={selectProfile}
          onSelectSection={(next) =>
            next === 'compare' ? openCompare(profile?.id ?? null) : setSection(next)
          }
          onAddFolder={() => void addProfile()}
          profileActions={profileActions}
        />

        <div className="mm-content">
          <AppUpdateBanner
            appUpdates={appUpdates}
            fileOperationRunning={fileOps.running !== null}
            modUpdateChecking={updates.checking}
          />
          <SettingsRestoreBanner restore={settingsRestore} />
          <FileOpsNotices fileOps={fileOps} />
          {addFolder.message !== null && (
            <ActionErrorBanner
              title={strings.actionFailed.addFolder}
              message={addFolder.message}
              onDismiss={addFolder.dismiss}
            />
          )}
          {launch.error !== null && (
            <ActionErrorBanner
              title={strings.actionFailed.launch}
              message={launch.error}
              onDismiss={launch.dismissError}
            />
          )}
          {gameFolder.failure !== null && section !== 'settings' && (
            <ActionErrorBanner
              title={gameFolder.failure.title}
              message={gameFolder.failure.message}
              onDismiss={gameFolder.dismiss}
            />
          )}
          {state.status === 'error' ? (
            <main className="sv-panel">
              <EmptyState
                tone="error"
                icon={FolderX}
                title={strings.states.appFailedTitle}
                description={describeError(state.error)}
                action={<Button onClick={reload}>{strings.states.retry}</Button>}
              />
            </main>
          ) : section === 'compare' ? (
            <main className="mm-main">
              <CompareScreen
                // Opening Compare from another profile starts it over with that profile on the left.
                key={compareLeftId ?? ''}
                profiles={profiles}
                initialLeftId={compareLeftId}
                game={guardedGame}
                fileOps={fileOps}
              />
            </main>
          ) : section === 'savegames' ? (
            <main className="mm-main">
              <SavegamesScreen
                profiles={profiles}
                defaultProfileId={activeProfile?.id ?? null}
                game={guardedGame}
                fileOps={fileOps}
                onOpenSettings={() => setSection('settings')}
              />
            </main>
          ) : section === 'settings' ? (
            <SettingsScreen
              updates={updates}
              appUpdates={appUpdates}
              game={game}
              gameFolder={gameFolder}
            />
          ) : section === 'health' ? (
            <main className="mm-main">
              <HealthScreen revision={fileOps.revision + updates.revision} onOpenMod={openMod} />
            </main>
          ) : view === 'ready' && profile ? (
            <main className="mm-main">
              <LibraryScreen
                profile={profile}
                profiles={profiles}
                game={guardedGame}
                switching={switching}
                fileOps={fileOps}
                updates={updates}
                appUpdates={appUpdates}
                focus={libraryFocus}
                onFocusApplied={clearLibraryFocus}
                onCompare={() => openCompare(profile.id)}
              />
            </main>
          ) : view === 'noGameFolder' || view === 'noFolders' ? (
            <WelcomeScreen
              kind={view}
              busy={gameFolder.busy}
              onChooseGameFolder={gameFolder.choose}
              onAddFolder={() => void addProfile()}
            />
          ) : (
            <main className="sv-panel" aria-busy="true">
              <div
                className="mm-loading"
                role="status"
                aria-label={strings.states.loadingProfiles}
              />
            </main>
          )}
        </div>
      </div>

      <StatusBar
        game={game}
        sampleData={api.sampleData}
        updateStatus={updates.status}
        onOpenSettings={() => setSection('settings')}
      />
    </div>
  );
}
