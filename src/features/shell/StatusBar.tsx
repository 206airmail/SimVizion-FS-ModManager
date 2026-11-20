import type { GameStatus } from '../../api';
import { tailPath } from '../../lib/format';
import { strings } from '../../strings';
import { cx } from '../../ui/cx';
import { Pill } from '../../ui/Pill';
import { Tooltip } from '../../ui/Tooltip';
import { statusDot, statusText, type UpdateStatus } from '../updates/status';

export interface StatusBarProps {
  game: GameStatus | null;
  /** True when the data on screen is generated sample data, not the user's folders. */
  sampleData: boolean;
  /** What the update checks are doing; a click on its item opens Settings. */
  updateStatus: UpdateStatus | null;
  onOpenSettings: () => void;
}

/** Quiet operational state: what the app knows about the game and the update checks right now. */
export function StatusBar({ game, sampleData, updateStatus, onOpenSettings }: StatusBarProps) {
  const t = strings.status;
  const updatesHint = strings.updates.status.hint;
  const installText =
    game === null
      ? null
      : game.userDirSource === 'none'
        ? t.noGameFolder
        : game.store !== null
          ? t.store(strings.gameFolder.stores[game.store])
          : null;
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
          {/* Which install the app found; "not found" when there is no user folder at all. */}
          {installText !== null && (
            <>
              <span className="mm-statusbar__divider mm-statusbar__optional" />
              <span className="mm-statusbar__item mm-statusbar__optional sv-truncate">
                {installText}
              </span>
            </>
          )}
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
      {updateStatus !== null && (
        <>
          <Tooltip label={updatesHint}>
            <button
              type="button"
              className="mm-statusbar__button"
              aria-label={`${statusText(updateStatus)}. ${updatesHint}`}
              onClick={onOpenSettings}
            >
              <span
                className={cx(
                  'sv-status-dot',
                  statusDot(updateStatus) !== 'none' && `sv-status-dot--${statusDot(updateStatus)}`,
                )}
              />
              {statusText(updateStatus)}
            </button>
          </Tooltip>
          <span className="mm-statusbar__divider" />
        </>
      )}
      {sampleData && (
        <Tooltip label={strings.app.sampleDataHint}>
          <Pill size="compact">{strings.app.sampleData}</Pill>
        </Tooltip>
      )}
      <span className="mm-statusbar__version">{strings.app.version(__APP_VERSION__)}</span>
    </footer>
  );
}
