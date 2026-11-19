import { Play } from 'lucide-react';
import type { GameStatus, Profile } from '../../api';
import { strings } from '../../strings';
import { cx } from '../../ui/cx';
import { ReasonButton } from '../files/ReasonButton';
import { Emblem } from './Emblem';
import type { LaunchController } from './useLaunch';

export interface AppHeaderProps {
  activeProfile: Profile | null;
  game: GameStatus | null;
  launch: LaunchController;
}

/** Identity region: product mark, where the game is pointed, and the app-level primary action. */
export function AppHeader({ activeProfile, game, launch }: AppHeaderProps) {
  const t = strings.header;
  return (
    <header className="mm-header">
      <div className="mm-brand">
        <span className="mm-brand__emblem">
          <Emblem />
        </span>
        <div className="mm-brand__text">
          <h1 className="mm-brand__wordmark">
            <span className="mm-brand__name">{strings.app.name}</span> {strings.app.product}
          </h1>
          <p className="mm-brand__subtitle">{strings.app.subtitle}</p>
        </div>
      </div>

      <dl className="mm-context">
        <div className="mm-context__cell">
          <dt className="sv-panel__eyebrow">{t.activeProfile}</dt>
          <dd className="mm-context__value mm-context__value--profile sv-truncate">
            {activeProfile?.name ?? t.noActiveProfile}
          </dd>
        </div>
        <div className="mm-context__cell mm-context__cell--game">
          <dt className="sv-panel__eyebrow">{t.game}</dt>
          <dd className="mm-context__value">
            <span className={cx('sv-status-dot', game?.gameRunning && 'sv-status-dot--warning')} />
            {game?.gameRunning ? t.gameRunning : t.gameNotRunning}
            {game?.gameVersion && (
              <span className="mm-context__quiet sv-num">{t.gameVersion(game.gameVersion)}</span>
            )}
          </dd>
        </div>
      </dl>

      <ReasonButton
        variant="primary"
        icon={Play}
        illuminated={launch.view.blockedReason === null}
        hint={t.launchHint}
        blockedReason={launch.view.blockedReason}
        onClick={launch.launch}
      >
        {launch.view.starting ? t.launchStarting : t.launch}
      </ReasonButton>
    </header>
  );
}
