import {
  ArrowLeftRight,
  Copy,
  FolderInput,
  GitCompareArrows,
  RefreshCw,
  SearchX,
} from 'lucide-react';
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from 'react';
import type { CompareRow, GameStatus, Profile } from '../../api';
import { api } from '../../api/client';
import { describeError } from '../../lib/errors';
import { useAsync } from '../../lib/useAsync';
import { strings } from '../../strings';
import { Button, IconButton } from '../../ui/Button';
import { EmptyState } from '../../ui/Feedback';
import { SearchField } from '../../ui/SearchField';
import { SegmentedControl } from '../../ui/SegmentedControl';
import { Select, type SelectOption } from '../../ui/Select';
import { fileOpBlockedReason } from '../files/blocked';
import { ReasonButton } from '../files/ReasonButton';
import { TransferDialog, type TransferPreset } from '../files/TransferDialog';
import type { FileOpsController } from '../files/useFileOps';
import {
  emptySelection,
  extendTo,
  navigate,
  restrictTo,
  selectAll,
  selectOnly,
  toggle,
  type Selection,
} from '../library/selection';
import { CompareTable } from './CompareTable';
import {
  compareFilters,
  countRows,
  defaultCompareFilter,
  filterRows,
  sourceModIds,
  type CompareFilter,
  type Direction,
} from './rows';

const t = strings.compare;

export interface CompareScreenProps {
  profiles: readonly Profile[];
  /** The folder on the left when the screen opens. */
  initialLeftId: string | null;
  game: GameStatus | null;
  fileOps: FileOpsController;
}

/** The folders to start with: `leftId` on the left, the active folder (else the first other) on the right. */
export function initialPair(
  profiles: readonly Profile[],
  leftId: string | null,
): { left: string; right: string } | null {
  const left =
    profiles.find((p) => p.id === leftId) ?? profiles.find((p) => p.isActive) ?? profiles[0];
  if (!left) return null;
  const others = profiles.filter((p) => p.id !== left.id);
  const right = others.find((p) => p.isActive) ?? others[0];
  return right ? { left: left.id, right: right.id } : null;
}

const filterLabels: Record<CompareFilter, string> = {
  differences: t.filterDifferences,
  onlyLeft: t.filterOnlyLeft,
  onlyRight: t.filterOnlyRight,
  different: t.filterDifferent,
  all: t.filterAll,
};

const NO_ROWS: readonly CompareRow[] = [];

export function CompareScreen({ profiles, initialLeftId, game, fileOps }: CompareScreenProps) {
  const [pair, setPair] = useState(() => initialPair(profiles, initialLeftId));
  if (pair === null) {
    return (
      <section className="sv-panel mm-compare" aria-label={t.title}>
        <EmptyState icon={GitCompareArrows} title={t.needTwoTitle} description={t.needTwoText} />
      </section>
    );
  }
  return (
    <CompareView
      profiles={profiles}
      leftId={pair.left}
      rightId={pair.right}
      onPair={(left, right) => setPair({ left, right })}
      game={game}
      fileOps={fileOps}
    />
  );
}

interface CompareViewProps {
  profiles: readonly Profile[];
  leftId: string;
  rightId: string;
  onPair: (left: string, right: string) => void;
  game: GameStatus | null;
  fileOps: FileOpsController;
}

function CompareView({ profiles, leftId, rightId, onPair, game, fileOps }: CompareViewProps) {
  const [filter, setFilter] = useState<CompareFilter>(defaultCompareFilter);
  const [search, setSearch] = useState('');
  const [stored, setStored] = useState<{ pair: string; selection: Selection }>({
    pair: '',
    selection: emptySelection,
  });
  const [preset, setPreset] = useState<TransferPreset | null>(null);

  const left = profiles.find((p) => p.id === leftId);
  const right = profiles.find((p) => p.id === rightId);
  const leftName = left?.name ?? '';
  const rightName = right?.name ?? '';
  const pairKey = `${leftId}|${rightId}`;

  const load = useCallback(() => api.compareProfiles(leftId, rightId), [leftId, rightId]);
  const { state, reload, replace } = useAsync(load);

  // After a copy or move ends, read the comparison again and swap it in place, so the rows and the
  // scroll position stay. A failed read keeps what is on screen.
  const seenRevision = useRef(fileOps.revision);
  useEffect(() => {
    if (fileOps.revision === seenRevision.current) return;
    seenRevision.current = fileOps.revision;
    let current = true;
    api.compareProfiles(leftId, rightId).then(
      (fresh) => {
        if (current) replace(fresh);
      },
      () => undefined,
    );
    return () => {
      current = false;
    };
  }, [fileOps.revision, leftId, rightId, replace]);

  const allRows = state.status === 'ready' ? state.data.rows : NO_ROWS;
  const deferredSearch = useDeferredValue(search);
  const rows = useMemo(
    () => filterRows(allRows, filter, deferredSearch),
    [allRows, filter, deferredSearch],
  );
  const counts = useMemo(() => countRows(allRows), [allRows]);
  const order = useMemo(() => rows.map((row) => row.modName), [rows]);

  const selection = useMemo(
    () => restrictTo(stored.pair === pairKey ? stored.selection : emptySelection, order),
    [stored, pairKey, order],
  );
  const select = useCallback(
    (next: Selection) => setStored({ pair: pairKey, selection: next }),
    [pairKey],
  );
  const selectedRows = useMemo(
    () => rows.filter((row) => selection.ids.has(row.modName)),
    [rows, selection],
  );

  const chooseLeft = (id: string) => onPair(id, id === rightId ? leftId : rightId);
  const chooseRight = (id: string) => onPair(id === leftId ? rightId : leftId, id);

  const folderOptions: SelectOption<string>[] = profiles.map((profile) => ({
    value: profile.id,
    label: profile.name,
  }));

  const blocked = fileOpBlockedReason(game, fileOps.running !== null);
  const open = (direction: Direction, mode: 'copy' | 'move') => {
    const from = direction === 'toRight' ? leftId : rightId;
    const to = direction === 'toRight' ? rightId : leftId;
    setPreset({
      fromProfileId: from,
      toProfileId: to,
      mode,
      modIds: sourceModIds(selectedRows, direction),
    });
  };
  const reasonFor = (direction: Direction): string | null => {
    if (selectedRows.length === 0) return t.blockedNoSelection;
    if (sourceModIds(selectedRows, direction).length === 0) {
      return t.blockedNotOnSide(direction === 'toRight' ? leftName : rightName);
    }
    return blocked;
  };

  const ready = state.status === 'ready';
  // The default view is empty because there is nothing to show, not because of a filter.
  const matching = filter === defaultCompareFilter && search.trim() === '' && allRows.length > 0;

  return (
    <section className="sv-panel mm-compare" aria-label={t.title}>
      <div className="mm-library__head">
        <div className="mm-library__title-row">
          <h2 className="mm-library__title">{t.title}</h2>
          <span className="mm-library__gap" />
          <IconButton icon={RefreshCw} label={t.rescan} size="standard" onClick={reload} />
        </div>

        <div className="mm-compare__pickers">
          <Select
            className="mm-compare__picker"
            size="compact"
            label={t.leftLabel}
            prefix={t.leftPrefix}
            value={leftId}
            options={folderOptions}
            onChange={chooseLeft}
          />
          <IconButton
            icon={ArrowLeftRight}
            label={t.swap}
            onClick={() => onPair(rightId, leftId)}
          />
          <Select
            className="mm-compare__picker"
            size="compact"
            label={t.rightLabel}
            prefix={t.rightPrefix}
            value={rightId}
            options={folderOptions}
            onChange={chooseRight}
          />
        </div>

        <div className="mm-library__toolbar">
          <div className="mm-library__filters">
            <SegmentedControl<CompareFilter>
              label={t.filterLabel}
              value={filter}
              onChange={setFilter}
              segments={compareFilters.map((value) => ({
                value,
                label: filterLabels[value],
                count: ready ? counts[value] : undefined,
              }))}
            />
            <SearchField
              className="mm-library__search"
              size="compact"
              label={t.searchLabel}
              placeholder={t.searchPlaceholder}
              clearLabel={strings.library.clearSearch}
              value={search}
              onChange={setSearch}
            />
          </div>
        </div>
      </div>

      {state.status === 'loading' && <CompareLoading />}
      {state.status === 'error' && (
        <EmptyState
          tone="error"
          icon={GitCompareArrows}
          title={strings.actionFailed.loadComparison}
          description={strings.states.loadFailedText(describeError(state.error))}
          action={<Button onClick={reload}>{strings.states.retry}</Button>}
        />
      )}
      {ready && rows.length === 0 && (
        <EmptyState
          icon={matching ? GitCompareArrows : SearchX}
          title={matching ? t.identicalTitle : t.noRowsTitle}
          description={matching ? t.identicalText : t.noRowsText}
          action={
            allRows.length > 0 && (
              <Button
                onClick={() => {
                  setFilter('all');
                  setSearch('');
                }}
              >
                {t.showAll}
              </Button>
            )
          }
        />
      )}
      {ready && rows.length > 0 && (
        <CompareTable
          rows={rows}
          selection={selection}
          label={t.tableLabel(leftName, rightName)}
          leftName={leftName}
          rightName={rightName}
          onPress={(id, modifiers) => {
            if (modifiers.extend) select(extendTo(selection, order, id));
            else if (modifiers.toggle) select(toggle(selection, id));
            else select(selectOnly(id));
          }}
          onOpen={(id) => select(selectOnly(id))}
          onNavigate={(key, extend, layout) => {
            const next = navigate(selection, order, key, extend, layout);
            if (next) select(next);
            return next !== null;
          }}
          onSelectAll={() => select(selectAll(selection, order))}
          onClear={() => select(emptySelection)}
        />
      )}

      <div className="sv-panel__footer mm-compare__footer">
        <span className="mm-compare__status sv-num" role="status">
          {selectedRows.length > 0 ? (
            <strong>{t.selected(selectedRows.length)}</strong>
          ) : ready ? (
            t.count(rows.length, allRows.length)
          ) : null}
        </span>
        <span className="mm-compare__spacer" />
        <ReasonButton
          size="compact"
          icon={Copy}
          hint={t.transferHint('copy', leftName, rightName)}
          blockedReason={reasonFor('toRight')}
          onClick={() => open('toRight', 'copy')}
        >
          {t.copyRight}
        </ReasonButton>
        <ReasonButton
          size="compact"
          icon={Copy}
          hint={t.transferHint('copy', rightName, leftName)}
          blockedReason={reasonFor('toLeft')}
          onClick={() => open('toLeft', 'copy')}
        >
          {t.copyLeft}
        </ReasonButton>
        <ReasonButton
          size="compact"
          icon={FolderInput}
          hint={t.transferHint('move', leftName, rightName)}
          blockedReason={reasonFor('toRight')}
          onClick={() => open('toRight', 'move')}
        >
          {t.moveRight}
        </ReasonButton>
        <ReasonButton
          size="compact"
          icon={FolderInput}
          hint={t.transferHint('move', rightName, leftName)}
          blockedReason={reasonFor('toLeft')}
          onClick={() => open('toLeft', 'move')}
        >
          {t.moveLeft}
        </ReasonButton>
      </div>

      {preset !== null && (
        <TransferDialog
          preset={preset}
          profiles={profiles}
          onClose={() => setPreset(null)}
          onConfirm={(plan, choices, targetName, replaced) => {
            setPreset(null);
            fileOps.startTransfer(plan, choices, targetName, replaced);
          }}
        />
      )}
    </section>
  );
}

const SKELETON_WIDTHS = [58, 44, 66, 38, 52, 62, 46, 55, 40, 64];

function CompareLoading() {
  return (
    <div className="mm-loading" role="status" aria-label={t.loadingLabel}>
      {SKELETON_WIDTHS.map((width, index) => (
        <div key={index} className="mm-loading__row">
          <span className="sv-skeleton" style={{ width: `${width}%` }} />
        </div>
      ))}
    </div>
  );
}
