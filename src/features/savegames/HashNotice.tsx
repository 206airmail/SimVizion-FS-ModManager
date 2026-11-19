import { FileSearch } from 'lucide-react';
import { useSyncExternalStore } from 'react';
import { describeError } from '../../lib/errors';
import { formatBytes } from '../../lib/format';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Banner } from '../../ui/Feedback';
import { Icon } from '../../ui/Icon';
import type { SavegameReportController } from './useSavegameReport';

const t = strings.savegames.hash;

/**
 * The file check: progress with Cancel while it runs (the banner of the file operations), and a
 * notice when it was cancelled or failed, with the way to run it again. The report stays on
 * screen in every case.
 */
export function HashNotice({ controller }: { controller: SavegameReportController }) {
  const { hash } = controller;
  if (hash.phase === 'running') return <HashProgressBanner controller={controller} />;
  if (hash.phase === 'cancelled') {
    return (
      <Banner
        tone="info"
        title={t.cancelledTitle}
        actions={
          <Button size="compact" onClick={controller.recheck}>
            {t.checkAgain}
          </Button>
        }
      >
        {t.cancelledText}
      </Banner>
    );
  }
  if (hash.phase === 'failed') {
    return (
      <Banner
        tone="warning"
        title={t.failedTitle}
        actions={
          <Button size="compact" onClick={controller.recheck}>
            {t.checkAgain}
          </Button>
        }
      >
        {describeError(hash.error)}
      </Banner>
    );
  }
  return null;
}

function HashProgressBanner({ controller }: { controller: SavegameReportController }) {
  const progress = useSyncExternalStore(controller.progress.subscribe, controller.progress.get);
  const fraction =
    progress === null
      ? 0
      : progress.totalBytes > 0
        ? progress.doneBytes / progress.totalBytes
        : progress.totalFiles > 0
          ? progress.doneFiles / progress.totalFiles
          : 0;
  const clamped = Math.min(1, Math.max(0, fraction));

  return (
    <div className="sv-banner sv-banner--info mm-op" role="group" aria-label={t.label}>
      <Icon icon={FileSearch} size={22} className="sv-banner__icon" />
      <div className="sv-banner__body">
        <div className="sv-banner__title">{t.title}</div>
        {/* Announced once; the numbers below change too often to be read out. */}
        <span className="sv-visually-hidden" role="status">
          {t.title}
        </span>
        <div
          className="mm-progress__track"
          role="progressbar"
          aria-label={t.title}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.floor(clamped * 100)}
        >
          <div className="mm-progress__fill" style={{ width: `${clamped * 100}%` }} />
        </div>
        <div className="mm-op__stats sv-num" aria-hidden="true">
          <span>
            {progress === null
              ? t.preparing
              : `${t.files(progress.doneFiles, progress.totalFiles)} · ${formatBytes(progress.doneBytes)} / ${formatBytes(progress.totalBytes)}`}
          </span>
          <span>{t.percent(clamped)}</span>
        </div>
      </div>
      <div className="sv-banner__actions">
        <Button size="compact" disabled={controller.hash.cancelling} onClick={controller.cancel}>
          {controller.hash.cancelling ? t.cancelling : t.cancel}
        </Button>
      </div>
    </div>
  );
}
