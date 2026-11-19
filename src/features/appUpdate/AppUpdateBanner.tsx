import { Download } from 'lucide-react';
import { useId, useState, useSyncExternalStore } from 'react';
import { formatBytes } from '../../lib/format';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Banner } from '../../ui/Feedback';
import { progressFraction } from '../files/progress';
import { installButtonKey, notesAreLong, notesPreview, notesText } from './state';
import type { AppUpdatesController } from './useAppUpdates';

const t = strings.appUpdates.banner;

/**
 * The offer to install a newer app version, above the screens: the version, the release notes as
 * plain text (a few lines, then "Show notes"), Install and restart, and Later. While it installs
 * it shows the download progress; the app restarts by itself when that is done. A failed install
 * leaves the banner in place with the reason and a retry, never a dialog.
 */
export function AppUpdateBanner({ appUpdates }: { appUpdates: AppUpdatesController }) {
  const { found, bannerVisible, install, failure } = appUpdates;
  // The notes open state belongs to one version: a newer update starts collapsed again.
  const [expanded, setExpanded] = useState<string | null>(null);
  const notesId = useId();
  if (!bannerVisible || found === null) return null;

  const text = notesText(found.notes);
  const long = notesAreLong(text);
  const open = expanded === found.version;
  const installing = install === 'installing';
  const key = installButtonKey(install);

  return (
    <Banner
      tone="info"
      title={t.title(found.version)}
      className="mm-appupdate"
      actions={
        <>
          <Button size="compact" disabled={installing} onClick={appUpdates.later}>
            {t.later}
          </Button>
          <Button
            size="compact"
            variant="primary"
            icon={Download}
            disabled={installing}
            onClick={appUpdates.installNow}
          >
            {t[key]}
          </Button>
        </>
      }
    >
      <span className="mm-appupdate__lead">{t.text}</span>
      {install === 'failed' && failure !== null && (
        <span className="mm-appupdate__failure" role="alert">
          {failure.message}
        </span>
      )}
      {text !== null && (
        <div className="mm-appupdate__notes-block">
          <div className="mm-appupdate__notes-head">
            <span className="sv-panel__eyebrow">{t.notesLabel}</span>
            {long && (
              <button
                type="button"
                className="sv-link-button"
                aria-expanded={open}
                aria-controls={notesId}
                onClick={() => setExpanded(open ? null : found.version)}
              >
                {open ? t.hideNotes : t.showNotes}
              </button>
            )}
          </div>
          <div id={notesId} className="mm-appupdate__notes sv-selectable">
            <div
              className="mm-appupdate__notes-text"
              data-collapsed={long && !open ? 'true' : undefined}
            >
              {long && !open ? notesPreview(text) : text}
            </div>
          </div>
        </div>
      )}
      {installing && <InstallProgress appUpdates={appUpdates} />}
    </Banner>
  );
}

function InstallProgress({ appUpdates }: { appUpdates: AppUpdatesController }) {
  const store = appUpdates.progress;
  const progress = useSyncExternalStore(store.subscribe, store.get);
  const fraction = progressFraction(progress);
  const done = fraction >= 1;
  return (
    <div className="mm-appupdate__progress">
      <div className="mm-appupdate__progress-head" aria-hidden="true">
        <span>{done ? t.restarting : (progress?.current ?? t.preparing)}</span>
        <span className="sv-num">
          {progress !== null && progress.totalBytes > 0
            ? t.progress(formatBytes(progress.doneBytes), formatBytes(progress.totalBytes))
            : ''}{' '}
          {t.percent(fraction)}
        </span>
      </div>
      <div
        className="mm-progress__track"
        role="progressbar"
        aria-label={t.progressLabel}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.floor(fraction * 100)}
      >
        <div className="mm-progress__fill" style={{ width: `${fraction * 100}%` }} />
      </div>
    </div>
  );
}
