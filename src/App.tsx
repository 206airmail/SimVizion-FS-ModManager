import { Construction, FolderX } from 'lucide-react';
import { useCallback, useState } from 'react';
import { api } from './api/client';
import { ActionErrorBanner } from './features/ActionErrorBanner';
import { Gallery } from './features/gallery/Gallery';
import { LibraryScreen } from './features/library/LibraryScreen';
import { AppHeader } from './features/shell/AppHeader';
import { ProfileRail, type Section } from './features/shell/ProfileRail';
import { StatusBar } from './features/shell/StatusBar';
import { useGameStatus } from './features/shell/useGameStatus';
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
  const game = useGameStatus();
  const addFolder = useActionError();
  const runAddFolder = addFolder.run;
  const [section, setSection] = useState<Section>('library');
  const [chosenProfileId, setChosenProfileId] = useState<string | null>(null);

  const selectProfile = useCallback((id: string) => {
    setChosenProfileId(id);
    setSection('library');
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

  return (
    <div className="mm-app">
      <AppHeader activeProfile={activeProfile} game={game} />

      <div className="mm-workspace">
        <ProfileRail
          profiles={profiles}
          selectedProfileId={profile?.id ?? null}
          section={section}
          onSelectProfile={selectProfile}
          onSelectSection={setSection}
          onAddFolder={() => void addProfile()}
        />

        <div className="mm-content">
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
              <LibraryScreen profile={profile} profiles={profiles} game={game} />
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
