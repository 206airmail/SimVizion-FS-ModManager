import type { GameStatus } from '../../api';
import { tailPath } from '../../lib/format';
import { strings } from '../../strings';
import { cx } from '../../ui/cx';
import { Pill } from '../../ui/Pill';
import { Tooltip } from '../../ui/Tooltip';

export interface StatusBarProps {
  game: GameStatus | null;
  /** True when the data on screen is generated sample data, not the user's folders. */
  sampleData: boolean;
}

/** Quiet operational state: what the app knows about the game right now. */
export function StatusBar({ game, sampleData }: StatusBarProps) {
  const t = strings.status;
  return (
    <footer className="mm-statusbar">
      {game && (
        <>
          <span className="mm-statusbar__item">
            <span className={cx('sv-status-dot', game.gameRunning && 'sv-status-dot--warning')} />
            {game.gameRunning ? t.gameRunning : t.gameNotRunning}
          </span>
          <span className="mm-statusbar__divider" />
          <span className="mm-statusbar__item">
            <span
              className={cx(
                'sv-status-dot',
                game.settingsReadable ? 'sv-status-dot--success' : 'sv-status-dot--warning',
              )}
            />
            {game.settingsReadable ? t.settingsOk : t.settingsMissing}
          </span>
          {/* An unreadable file says nothing about the override, so claim nothing. */}
          {game.settingsReadable && (
            <>
              <span className="mm-statusbar__divider mm-statusbar__optional" />
              <span className="mm-statusbar__item mm-statusbar__optional sv-truncate">
                {game.overrideActive ? t.overrideOn : t.overrideOff}
                {game.overrideActive && game.effectiveModsDir && (
                  <Tooltip label={game.effectiveModsDir}>
                    <span className="mm-statusbar__path sv-truncate">
                      {tailPath(game.effectiveModsDir)}
                    </span>
                  </Tooltip>
                )}
              </span>
            </>
          )}
        </>
      )}
      <span className="mm-statusbar__spacer" />
      {sampleData && (
        <Tooltip label={strings.app.sampleDataHint}>
          <Pill size="compact">{strings.app.sampleData}</Pill>
        </Tooltip>
      )}
      <span className="mm-statusbar__version">{strings.app.version}</span>
    </footer>
  );
}
