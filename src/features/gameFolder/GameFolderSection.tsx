import { useId } from 'react';
import type { GameStatus } from '../../api';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Banner } from '../../ui/Feedback';
import type { GameFolderController } from './useGameFolder';

const t = strings.gameFolder;

/**
 * Settings → Game folder: where the game's user folder is, how it was found, and the buttons to
 * choose another or go back to the detected one. A failed choice shows here as a banner.
 */
export function GameFolderSection({
  game,
  gameFolder,
}: {
  game: GameStatus | null;
  gameFolder: GameFolderController;
}) {
  const headingId = useId();
  const chosen = game?.userDirSource === 'chosen';
  const none = game?.userDirSource === 'none';

  return (
    <section className="mm-settings__section" aria-labelledby={headingId}>
      <h3 id={headingId} className="mm-settings__heading">
        {t.title}
      </h3>
      <p className="mm-settings__text">{t.lead}</p>

      {gameFolder.failure !== null && (
        <Banner
          tone="error"
          title={gameFolder.failure.title}
          actions={
            <Button size="compact" onClick={gameFolder.dismiss}>
              {strings.actionFailed.dismiss}
            </Button>
          }
        >
          {gameFolder.failure.message}
        </Banner>
      )}

      {game !== null && (
        <dl className="mm-settings__facts">
          <div className="mm-settings__fact">
            <dt>{t.pathLabel}</dt>
            <dd className="mm-settings__path sv-selectable">
              {game.userDir ?? <span className="mm-settings__quiet">{t.sources.none}</span>}
            </dd>
          </div>
          <div className="mm-settings__fact">
            <dt>{t.sourceLabel}</dt>
            <dd>{t.sources[game.userDirSource]}</dd>
          </div>
          <div className="mm-settings__fact">
            <dt>{t.storeLabel}</dt>
            <dd>{game.store !== null ? t.stores[game.store] : t.noStore}</dd>
          </div>
        </dl>
      )}
      {none && <p className="mm-settings__text">{t.notFound}</p>}
      {chosen && <p className="mm-settings__text mm-settings__quiet">{t.chosenNote}</p>}

      <div className="mm-settings__actions">
        <Button disabled={gameFolder.busy} onClick={gameFolder.choose}>
          {t.choose}
        </Button>
        {chosen && (
          <Button disabled={gameFolder.busy} onClick={gameFolder.useDetected}>
            {t.useDetected}
          </Button>
        )}
      </div>
    </section>
  );
}
