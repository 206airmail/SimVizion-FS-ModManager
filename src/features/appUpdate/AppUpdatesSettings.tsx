import { RefreshCw } from 'lucide-react';
import { useId } from 'react';
import { formatDateTime } from '../../lib/format';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Banner } from '../../ui/Feedback';
import { Switch } from '../../ui/Switch';
import type { AppUpdatesController } from './useAppUpdates';

const t = strings.appUpdates.settings;

/**
 * The second part of Settings → Online: the switch for the app's own update checks, when it last
 * asked, and Check now. The last failure is shown here (the banner stays quiet about it).
 */
export function AppUpdatesSettings({ appUpdates }: { appUpdates: AppUpdatesController }) {
  const headingId = useId();
  const { settings, checking, found, upToDate, failure } = appUpdates;
  const enabled = settings?.enabled === true;

  return (
    <div className="mm-settings__group" role="group" aria-labelledby={headingId}>
      <h4 id={headingId} className="mm-settings__subheading">
        {t.groupTitle}
      </h4>
      <Switch
        checked={enabled}
        disabled={settings === null || checking}
        onChange={appUpdates.setEnabled}
      >
        {t.switchLabel}
      </Switch>
      <p className="mm-settings__text">{t.sent}</p>
      <p className="mm-settings__text">{t.downloads}</p>

      {failure !== null && (
        <Banner
          tone="error"
          title={failure.title}
          actions={
            <Button size="compact" onClick={appUpdates.dismissFailure}>
              {strings.actionFailed.dismiss}
            </Button>
          }
        >
          {failure.message}
        </Banner>
      )}

      {settings !== null &&
        (enabled ? (
          <>
            <dl className="mm-settings__facts">
              <div className="mm-settings__fact">
                <dt>{t.lastCheck}</dt>
                <dd className="sv-num">
                  {settings.lastCheck !== null
                    ? formatDateTime(settings.lastCheck)
                    : t.neverChecked}
                </dd>
              </div>
              {(found !== null || upToDate) && (
                <div className="mm-settings__fact">
                  <dt>{t.result}</dt>
                  <dd>{found !== null ? t.available(found.version) : t.upToDate}</dd>
                </div>
              )}
            </dl>
            <div className="mm-settings__actions">
              <Button icon={RefreshCw} disabled={checking} onClick={appUpdates.checkNow}>
                {checking ? t.checking : t.checkNow}
              </Button>
            </div>
          </>
        ) : (
          <p className="mm-settings__text mm-settings__quiet">{t.off}</p>
        ))}
    </div>
  );
}
