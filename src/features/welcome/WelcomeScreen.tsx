import { FolderPlus, FolderSearch } from 'lucide-react';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Icon } from '../../ui/Icon';

const t = strings.welcome;

export interface WelcomeScreenProps {
  /** `noGameFolder`: the game's user folder was not found. `noFolders`: it was, but no mod folder is listed. */
  kind: 'noGameFolder' | 'noFolders';
  /** Opens the picker for the game's user folder. */
  onChooseGameFolder: () => void;
  /** Opens the picker for a mod folder. */
  onAddFolder: () => void;
  /** The game folder picker is open or being applied. */
  busy: boolean;
}

/**
 * What the library area shows before there is anything to browse. It says what is missing and
 * offers the way forward: the game folder when it was not found, a mod folder otherwise. Calm
 * and plain: nothing is wrong with the user's setup.
 */
export function WelcomeScreen({ kind, onChooseGameFolder, onAddFolder, busy }: WelcomeScreenProps) {
  const noGame = kind === 'noGameFolder';
  return (
    <main className="sv-panel mm-welcome" aria-label={noGame ? t.noGameTitle : t.noFoldersTitle}>
      <div className="mm-welcome__body">
        <span className="mm-welcome__mark" aria-hidden="true">
          <Icon icon={noGame ? FolderSearch : FolderPlus} size={32} />
        </span>
        <h2 className="mm-welcome__title">{noGame ? t.noGameTitle : t.noFoldersTitle}</h2>
        <p className="mm-welcome__text">{noGame ? t.noGameText : t.noFoldersText}</p>
        {noGame && (
          <>
            <ul className="mm-welcome__places">
              {t.searched.map((place) => (
                <li key={place}>{place}</li>
              ))}
            </ul>
            <p className="mm-welcome__note">{t.noGameNote}</p>
          </>
        )}
        <div className="mm-welcome__actions">
          {noGame ? (
            <>
              <Button variant="primary" disabled={busy} onClick={onChooseGameFolder}>
                {t.chooseGameFolder}
              </Button>
              <Button icon={FolderPlus} onClick={onAddFolder}>
                {t.addFolder}
              </Button>
            </>
          ) : (
            <Button variant="primary" icon={FolderPlus} onClick={onAddFolder}>
              {t.addFolder}
            </Button>
          )}
        </div>
      </div>
    </main>
  );
}
