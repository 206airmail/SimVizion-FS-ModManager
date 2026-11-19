import { Construction, FolderX } from 'lucide-react';
import { useCallback, useState } from 'react';
import type { Profile } from './api';
import { api } from './api/client';
import { ActionErrorBanner } from './features/ActionErrorBanner';
import { CompareScreen } from './features/compare/CompareScreen';
import { FileOpsNotices } from './features/files/FileOpsNotices';
import { useFileOps } from './features/files/useFileOps';
import { Gallery } from './features/gallery/Gallery';
import { LibraryScreen } from './features/library/LibraryScreen';
import { SavegamesScreen } from './features/savegames/SavegamesScreen';
import { AppHeader } from './features/shell/AppHeader';
import { ProfileRail, type ProfileActions, type Section } from './features/shell/ProfileRail';
import { StatusBar } from './features/shell/StatusBar';
import { useGameStatus } from './features/shell/useGameStatus';
import { SettingsRestoreBanner } from './features/switching/SettingsRestoreBanner';
import { useSettingsRestore } from './features/switching/useSettingsRestore';
import { useSwitching } from './features/switching/useSwitching';
import { describeError } from './lib/errors';
import { useActionError } from './lib/useActionError';
import { useAsync } from './lib/useAsync';
import { strings } from './strings';
import { Button } from './ui/Button';
import { EmptyState } from './ui/Feedback';

const loadProfiles = () => api.listProfiles();

/** `#gallery` shows every primitive in every state, for design review. */
const showGallery = window.location.hash === '#gallery';

export function App() {
  return showGallery ? <Gallery /> : <Workspace />;
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
      <AppHeader activeProfile={activeProfile} game={game} />

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
          <SettingsRestoreBanner restore={settingsRestore} />
          <FileOpsNotices fileOps={fileOps} />
          {addFolder.message !== null && (
            <ActionErrorBanner
              title={strings.actionFailed.addFolder}
              message={addFolder.message}
              onDismiss={addFolder.dismiss}
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
                game={game}
                fileOps={fileOps}
              />
            </main>
          ) : section === 'savegames' ? (
            <main className="mm-main">
              <SavegamesScreen
                profiles={profiles}
                defaultProfileId={activeProfile?.id ?? null}
                game={game}
                fileOps={fileOps}
              />
            </main>
          ) : section !== 'library' ? (
            <main className="sv-panel">
              <EmptyState
                icon={Construction}
                title={strings.states.notBuiltTitle(strings.rail[section])}
                description={strings.states.notBuiltText}
                action={
                  <Button onClick={() => setSection('library')}>
                    {strings.states.backToLibrary}
                  </Button>
                }
              />
            </main>
          ) : profile ? (
            <main className="mm-main">
              <LibraryScreen
                profile={profile}
                profiles={profiles}
                game={game}
                switching={switching}
                fileOps={fileOps}
                onCompare={() => openCompare(profile.id)}
              />
            </main>
          ) : (
            <main className="sv-panel" aria-busy="true" />
          )}
        </div>
      </div>

      <StatusBar game={game} sampleData={api.sampleData} />
    </div>
  );
}
