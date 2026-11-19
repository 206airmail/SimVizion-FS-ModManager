import { HeartPulse, RefreshCw, SearchX } from 'lucide-react';
import { memo, useEffect, useMemo, useRef, useState } from 'react';
import type { HealthReport, HealthRow, IssueKind } from '../../api';
import { api } from '../../api/client';
import { describeError } from '../../lib/errors';
import { formatCount } from '../../lib/format';
import { useAsync } from '../../lib/useAsync';
import { strings } from '../../strings';
import { Button, IconButton } from '../../ui/Button';
import { cx } from '../../ui/cx';
import { EmptyState } from '../../ui/Feedback';
import { Pill } from '../../ui/Pill';
import { Select, type SelectOption } from '../../ui/Select';
import { Tooltip } from '../../ui/Tooltip';
import { severityIcons, severityTones } from '../library/labels';
import {
  countKinds,
  defaultFilter,
  effectiveFilter,
  filterRows,
  folderOptions,
  inFolder,
  presentKinds,
  type HealthFilter,
  type KindFilter,
} from './rows';

const t = strings.health;

/** How many issue pills a row shows before the rest fold into "+N" (the tooltip lists them all). */
const VISIBLE_ISSUES = 2;

export interface HealthScreenProps {
  /** Goes up when a file operation or an update check ended: the report is read again. */
  revision: number;
  /** Opens the folder's library with this mod selected. */
  onOpenMod: (profileId: string, modId: string) => void;
}

const loadReport = () => api.healthReport();

const SKELETON_WIDTHS = [58, 44, 66, 38, 52, 61, 47, 55];

/**
 * Every mod with issues in every folder: the counts per kind (they filter the table), a folder
 * filter, and the mods. Read-only; a click opens the mod in its folder's library.
 */
export function HealthScreen({ revision, onOpenMod }: HealthScreenProps) {
  const { state, reload, replace } = useAsync<HealthReport>(loadReport);
  const [filter, setFilter] = useState<HealthFilter>(defaultFilter);

  // After a copy, move, remove or update, read the report again and swap it in place.
  const seenRevision = useRef(revision);
  useEffect(() => {
    if (revision === seenRevision.current) return;
    seenRevision.current = revision;
    let current = true;
    api.healthReport().then(
      (fresh) => {
        if (current) replace(fresh);
      },
      () => undefined,
    );
    return () => {
      current = false;
    };
  }, [revision, replace]);

  const rows = useMemo(() => (state.status === 'ready' ? state.data.rows : []), [state]);
  const active = useMemo(() => effectiveFilter(rows, filter), [rows, filter]);
  const folders = useMemo(() => folderOptions(rows), [rows]);
  const kindCounts = useMemo(() => countKinds(inFolder(rows, active.folder)), [rows, active]);
  const kinds = useMemo(() => presentKinds(inFolder(rows, active.folder)), [rows, active]);
  const shown = useMemo(() => filterRows(rows, active), [rows, active]);

  const ready = state.status === 'ready';
  const folderChoices: SelectOption<string>[] = [
    { value: 'all', label: t.folderAll, hint: formatCount(rows.length) },
    ...folders.map((folder) => ({
      value: folder.profileId,
      label: folder.name,
      hint: formatCount(folder.count),
    })),
  ];

  return (
    <section className="sv-panel mm-health" aria-label={t.title}>
      <div className="mm-library__head">
        <div className="mm-library__title-row">
          <h2 className="mm-library__title">{t.title}</h2>
          {ready && (
            <span className="mm-health__summary sv-num sv-truncate" role="status">
              {rows.length === 0 ? t.summaryClean : t.summary(rows.length, folders.length)}
            </span>
          )}
          <span className="mm-library__gap" />
          {ready && folders.length > 1 && (
            <Select
              className="mm-health__folder"
              size="compact"
              label={t.folderLabel}
              prefix={t.folderPrefix}
              value={active.folder}
              options={folderChoices}
              onChange={(folder) => setFilter({ ...active, folder })}
            />
          )}
          <IconButton icon={RefreshCw} label={t.rescan} size="standard" onClick={reload} />
        </div>
        {ready && rows.length > 0 && (
          <KindChips
            active={active.kind}
            kinds={kinds}
            counts={kindCounts}
            onChange={(kind) => setFilter({ ...active, kind })}
          />
        )}
      </div>

      {state.status === 'loading' && (
        <div className="mm-loading" role="status" aria-label={t.loadingLabel} aria-busy="true">
          {SKELETON_WIDTHS.map((width, index) => (
            <div key={index} className="mm-loading__row">
              <span className="sv-skeleton" style={{ width: `${width}%` }} />
            </div>
          ))}
        </div>
      )}
      {state.status === 'error' && (
        <EmptyState
          tone="error"
          icon={HeartPulse}
          title={t.failedTitle}
          description={strings.states.loadFailedText(describeError(state.error))}
          action={<Button onClick={reload}>{strings.states.retry}</Button>}
        />
      )}
      {ready && rows.length === 0 && (
        <EmptyState icon={HeartPulse} title={t.emptyTitle} description={t.emptyText} />
      )}
      {ready && rows.length > 0 && shown.length === 0 && (
        <EmptyState
          icon={SearchX}
          title={t.noMatchesTitle}
          description={t.noMatchesText}
          action={<Button onClick={() => setFilter(defaultFilter)}>{t.clearFilters}</Button>}
        />
      )}
      {ready && shown.length > 0 && <HealthTable rows={shown} onOpenMod={onOpenMod} />}
    </section>
  );
}

interface KindChipsProps {
  active: KindFilter;
  kinds: readonly IssueKind[];
  counts: Record<KindFilter, number>;
  onChange: (kind: KindFilter) => void;
}

/** The counts per kind of issue, as a one-of-many filter. Selection is teal, like the other filters. */
function KindChips({ active, kinds, counts, onChange }: KindChipsProps) {
  const chips: KindFilter[] = ['all', ...kinds];
  return (
    <div className="mm-health__chips" role="radiogroup" aria-label={t.kindLabel}>
      {chips.map((kind) => (
        <button
          key={kind}
          type="button"
          role="radio"
          aria-checked={kind === active}
          className="mm-chip"
          onClick={() => onChange(kind)}
        >
          <span className="sv-truncate">{kind === 'all' ? t.kindAll : strings.issues[kind]}</span>
          <span className="sv-count-chip sv-num">{formatCount(counts[kind])}</span>
        </button>
      ))}
    </div>
  );
}

function HealthTable({
  rows,
  onOpenMod,
}: {
  rows: readonly HealthRow[];
  onOpenMod: HealthScreenProps['onOpenMod'];
}) {
  return (
    <div className="mm-health__scroll">
      <div
        className="mm-health__table"
        role="table"
        aria-label={t.tableLabel}
        aria-rowcount={rows.length + 1}
      >
        <div
          className="sv-table__head mm-health__cols mm-health__head"
          role="row"
          aria-rowindex={1}
        >
          <div role="columnheader" className="sv-table__head-cell">
            {t.columns.title}
          </div>
          <div role="columnheader" className="sv-table__head-cell mm-health__c-file">
            {t.columns.modName}
          </div>
          <div role="columnheader" className="sv-table__head-cell">
            {t.columns.folder}
          </div>
          <div role="columnheader" className="sv-table__head-cell">
            {t.columns.issues}
          </div>
        </div>
        <div role="rowgroup">
          {rows.map((row, index) => (
            <HealthRowView
              key={`${row.profileId}/${row.modId}`}
              row={row}
              index={index}
              onOpenMod={onOpenMod}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

const HealthRowView = memo(function HealthRowView({
  row,
  index,
  onOpenMod,
}: {
  row: HealthRow;
  index: number;
  onOpenMod: HealthScreenProps['onOpenMod'];
}) {
  const open = () => onOpenMod(row.profileId, row.modId);
  const visible = row.issues.slice(0, VISIBLE_ISSUES);
  const hidden = row.issues.length - visible.length;
  return (
    <div
      role="row"
      aria-rowindex={index + 2}
      className="sv-row mm-health__cols mm-health__row"
      onClick={open}
    >
      <div role="cell" className="sv-cell mm-health__title">
        <button
          type="button"
          className="mm-health__open sv-truncate"
          aria-label={t.open(row.title, row.profileName)}
          onClick={(event) => {
            event.stopPropagation();
            open();
          }}
        >
          {row.title}
        </button>
      </div>
      <div
        role="cell"
        className="sv-cell sv-cell--secondary sv-truncate mm-health__file mm-health__c-file"
        title={row.modName}
      >
        {row.modName}
      </div>
      <div role="cell" className="sv-cell sv-cell--secondary sv-truncate" title={row.profileName}>
        {row.profileName}
      </div>
      <div role="cell" className="sv-cell mm-health__issues">
        <Tooltip label={row.issues.map(issueText).join(' · ')}>
          <span className="mm-health__pills">
            {visible.map((issue) => (
              <Pill
                key={issue.kind}
                tone={severityTones[issue.severity]}
                icon={severityIcons[issue.severity]}
                size="compact"
                className="mm-health__pill"
              >
                <span className="sv-truncate">{strings.issues[issue.kind]}</span>
              </Pill>
            ))}
            {hidden > 0 && (
              <span className={cx('mm-health__more', 'sv-num')}>{t.more(hidden)}</span>
            )}
          </span>
        </Tooltip>
      </div>
    </div>
  );
});

function issueText(issue: HealthRow['issues'][number]): string {
  const detail = strings.issueDetail[issue.kind](issue.detail);
  return detail === null ? strings.issues[issue.kind] : `${strings.issues[issue.kind]}: ${detail}`;
}
