import { Copy, FolderInput, Trash2, type LucideIcon } from 'lucide-react';
import { useState, useSyncExternalStore } from 'react';
import { formatBytes } from '../../lib/format';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Banner } from '../../ui/Feedback';
import { Icon } from '../../ui/Icon';
import { ActionErrorBanner } from '../ActionErrorBanner';
import { progressFraction, type ProgressStore } from './progress';
import { describeOutcome, type OpKind } from './summary';
import type { FileOpsController, RunningOp } from './useFileOps';

const kindIcons: Record<OpKind, LucideIcon> = {
  copy: Copy,
  move: FolderInput,
  remove: Trash2,
};

/**
 * App-level notices for copy, move and remove: the progress (with Cancel) while one runs, the
 * reason one failed as a whole, and the summary of how the last one ended. They sit above the
 * screens, so the user can keep browsing while a long copy runs.
 */
export function FileOpsNotices({ fileOps }: { fileOps: FileOpsController }) {
  const { running, failure, outcome } = fileOps;
  return (
    <>
      {running !== null && (
        <ProgressBanner running={running} store={fileOps.progress} onCancel={fileOps.cancel} />
      )}
      {failure !== null && (
        <ActionErrorBanner
          title={failure.title}
          message={failure.message}
          onDismiss={fileOps.dismiss}
        />
      )}
      {outcome !== null && (
        <OutcomeBanner
          key={outcome.id}
          view={describeOutcome(outcome)}
          onDismiss={fileOps.dismiss}
        />
      )}
    </>
  );
}

interface ProgressBannerProps {
  running: RunningOp;
  store: ProgressStore;
  onCancel: () => void;
}

function ProgressBanner({ running, store, onCancel }: ProgressBannerProps) {
  const p = strings.fileOps.progress;
  const progress = useSyncExternalStore(store.subscribe, store.get);
  const fraction = progressFraction(progress);
  const title = p.title(running.kind, running.target);

  return (
    <div className="sv-banner sv-banner--info mm-op" role="group" aria-label={p.label}>
      <Icon icon={kindIcons[running.kind]} size={20} className="sv-banner__icon" />
      <div className="sv-banner__body">
        <div className="sv-banner__title">{title}</div>
        {/* Announced once; the numbers below change too often to be read out. */}
        <span className="sv-visually-hidden" role="status">
          {title}
        </span>
        <div className="mm-op__current sv-truncate sv-selectable">
          {progress?.current ?? p.preparing}
        </div>
        <div
          className="mm-progress__track"
          role="progressbar"
          aria-label={title}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.floor(fraction * 100)}
        >
          <div className="mm-progress__fill" style={{ width: `${fraction * 100}%` }} />
        </div>
        <div className="mm-op__stats sv-num" aria-hidden="true">
          <span>
            {progress !== null && progress.totalBytes > 0
              ? p.bytes(formatBytes(progress.doneBytes), formatBytes(progress.totalBytes))
              : ''}
          </span>
          <span>{progress !== null ? p.items(progress.doneItems, progress.totalItems) : ''}</span>
          <span>{p.percent(fraction)}</span>
        </div>
      </div>
      <div className="sv-banner__actions">
        <Button size="compact" disabled={running.cancelling} onClick={onCancel}>
          {running.cancelling ? p.cancelling : p.cancel}
        </Button>
      </div>
    </div>
  );
}

function OutcomeBanner({
  view,
  onDismiss,
}: {
  view: ReturnType<typeof describeOutcome>;
  onDismiss: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const r = strings.fileOps.result;
  const hasProblems = view.problems.length > 0;

  return (
    <Banner
      tone={view.tone}
      title={view.title}
      actions={
        <>
          {hasProblems && (
            <Button size="compact" aria-expanded={expanded} onClick={() => setExpanded(!expanded)}>
              {expanded ? r.hideDetails : r.showDetails(view.problems.length)}
            </Button>
          )}
          <Button size="compact" onClick={onDismiss}>
            {strings.actionFailed.dismiss}
          </Button>
        </>
      }
    >
      {view.text}
      {hasProblems && expanded && (
        <ul className="mm-op__problems" aria-label={r.detailsLabel}>
          {view.problems.map((problem) => (
            <li key={problem.modName} className="mm-op__problem">
              <span className="mm-op__problem-name sv-truncate sv-selectable">
                {problem.modName}
              </span>
              <span className="mm-op__problem-reason">{problem.reason}</span>
            </li>
          ))}
        </ul>
      )}
    </Banner>
  );
}
