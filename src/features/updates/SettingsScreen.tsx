import { RefreshCw } from 'lucide-react';
import { useId } from 'react';
import { formatDateTime, formatTime } from '../../lib/format';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Banner } from '../../ui/Feedback';
import { Switch } from '../../ui/Switch';
import type { UpdatesController } from './useUpdates';

const t = strings.updates.settings;

/**
 * Settings. Today it holds one section, Online: the switch for update checks, what is sent and to
 * whom, and the state of the last check. Everything takes effect at once.
 */
export function SettingsScreen({ updates }: { updates: UpdatesController }) {
  const headingId = useId();
  const { settings, checking, result, failure, pausedUntil: paused } = updates;
  const enabled = settings?.enabled === true;

  return (
    <main className="sv-panel mm-settings" aria-label={t.title}>
      <div className="sv-panel__header">
        <h2 className="sv-panel__title">{t.title}</h2>
      </div>
      <div className="mm-settings__scroll">
        <section className="mm-settings__section" aria-labelledby={headingId}>
          <h3 id={headingId} className="mm-settings__heading">
            {t.onlineTitle}
          </h3>
          <Switch
            checked={enabled}
            disabled={settings === null || checking}
            onChange={updates.setEnabled}
          >
            {t.switchLabel}
          </Switch>
          <p className="mm-settings__text">{t.sent}</p>
          <p className="mm-settings__text">{t.downloads}</p>

          {failure !== null && (
            <Banner
              tone={
                failure.code === 'offline' || failure.code === 'rateLimited' ? 'warning' : 'error'
              }
              title={failure.title}
              actions={
                <Button size="compact" onClick={updates.dismissFailure}>
                  {strings.actionFailed.dismiss}
                </Button>
              }
            >
              {failure.code === 'rateLimited' && paused !== null
                ? t.pausedText(formatTime(paused))
                : failure.message}
            </Banner>
          )}

          {settings !== null && (
            <div className="mm-settings__state">
              {enabled ? (
                <dl className="mm-settings__facts">
                  <div className="mm-settings__fact">
                    <dt>{t.lastCheck}</dt>
                    <dd className="sv-num">
                      {settings.lastCheck !== null
                        ? formatDateTime(settings.lastCheck)
                        : t.neverChecked}
                    </dd>
                  </div>
                  {paused !== null && (
                    <div className="mm-settings__fact">
                      <dt>{t.retryAfter}</dt>
                      <dd className="sv-num">{formatDateTime(paused)}</dd>
                    </div>
                  )}
                  {result !== null && (
                    <div className="mm-settings__fact">
                      <dt>{t.result}</dt>
                      <dd>
                        {t.resultText(
                          result.checked,
                          result.matchedByName,
                          result.matchedByHash,
                          result.updates,
                        )}
                        {result.fromCache && (
                          <span className="mm-settings__quiet"> {t.fromCache}</span>
                        )}
                      </dd>
                    </div>
                  )}
                </dl>
              ) : (
                <p className="mm-settings__text mm-settings__quiet">{t.off}</p>
              )}
              {enabled && (
                <div className="mm-settings__actions">
                  <Button
                    icon={RefreshCw}
                    disabled={checking || paused !== null}
                    onClick={updates.checkNow}
                  >
                    {checking ? t.checking : t.checkNow}
                  </Button>
                </div>
              )}
            </div>
          )}
        </section>
      </div>
    </main>
  );
}
