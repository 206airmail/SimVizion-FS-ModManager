import { Construction, FolderX } from 'lucide-react';
import { useCallback, useState } from 'react';
import { api } from './api/client';
import { Gallery } from './features/gallery/Gallery';
import { LibraryScreen } from './features/library/LibraryScreen';
import { AppHeader } from './features/shell/AppHeader';
import { ProfileRail, type Section } from './features/shell/ProfileRail';
import { StatusBar } from './features/shell/StatusBar';
import { useAsync } from './lib/useAsync';
import { strings } from './strings';
import { Button } from './ui/Button';
import { EmptyState } from './ui/Feedback';

const loadShell = () => Promise.all([api.listProfiles(), api.getGameStatus()]);

/** `#gallery` shows every primitive in every state, for design review. */
const showGallery = window.location.hash === '#gallery';

export function App() {
  return showGallery ? <Gallery /> : <Workspace />;
}

function Workspace() {
  const { state, reload } = useAsync(loadShell);
  const [section, setSection] = useState<Section>('library');
  const [chosenProfileId, setChosenProfileId] = useState<string | null>(null);

  const selectProfile = useCallback((id: string) => {
    setChosenProfileId(id);
    setSection('library');
  }, []);

  const profiles = state.status === 'ready' ? state.data[0] : [];
  const game = state.status === 'ready' ? state.data[1] : null;
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
        />

        {state.status === 'error' ? (
          <main className="sv-panel">
            <EmptyState
              tone="error"
              icon={FolderX}
              title={strings.states.appFailedTitle}
              description={state.message}
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

      <StatusBar game={game} />
    </div>
  );
}
