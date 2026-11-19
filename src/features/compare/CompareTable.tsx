import { useVirtualizer } from '@tanstack/react-virtual';
import { memo, useEffect, useId, useRef, type KeyboardEvent } from 'react';
import type { CompareRow, CompareSide } from '../../api';
import { formatBytes, formatDate } from '../../lib/format';
import { strings } from '../../strings';
import { cx } from '../../ui/cx';
import { Pill } from '../../ui/Pill';
import { Tooltip } from '../../ui/Tooltip';
import { handleCollectionKey, pressModifiers, type CollectionProps } from '../library/collection';
import type { Selection } from '../library/selection';
import { compareStateView } from './labels';

const ROW_HEIGHT = 36;
const GROUP_HEIGHT = 28;
const HEAD_HEIGHT = 32;
const STICKY_HEIGHT = GROUP_HEIGHT + HEAD_HEIGHT;

const t = strings.compare;

export interface CompareTableProps extends Pick<
  CollectionProps,
  'onPress' | 'onOpen' | 'onNavigate' | 'onSelectAll' | 'onClear' | 'label'
> {
  rows: readonly CompareRow[];
  /** Selection by mod name. */
  selection: Selection;
  leftName: string;
  rightName: string;
}

/**
 * The merged table: one row per mod name, the left folder's version, size and date, the right
 * folder's, and the state. It selects like the library list (click, Ctrl and Shift, the keyboard);
 * Space toggles the row under the cursor, and Enter does nothing because there is no detail view.
 */
export function CompareTable(props: CompareTableProps) {
  const { rows, selection, label, leftName, rightName, onPress } = props;
  const scrollRef = useRef<HTMLDivElement>(null);
  const baseId = useId();

  // TanStack Virtual hands back functions that change between renders by design.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
    scrollMargin: STICKY_HEIGHT,
    scrollPaddingStart: STICKY_HEIGHT,
  });

  const cursorIndex =
    selection.cursor === null ? -1 : rows.findIndex((row) => row.modName === selection.cursor);

  useEffect(() => {
    if (cursorIndex >= 0) virtualizer.scrollToIndex(cursorIndex);
  }, [cursorIndex, virtualizer]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target === event.currentTarget && event.key === ' ') {
      event.preventDefault();
      if (selection.cursor !== null) onPress(selection.cursor, { toggle: true, extend: false });
      return;
    }
    handleCollectionKey(event, props, {
      columns: 1,
      pageRows: Math.max(
        1,
        Math.floor((event.currentTarget.clientHeight - STICKY_HEIGHT) / ROW_HEIGHT) - 1,
      ),
    });
  };

  return (
    <div
      ref={scrollRef}
      className="mm-collection mm-cmp"
      role="grid"
      tabIndex={0}
      aria-label={label}
      aria-multiselectable="true"
      aria-rowcount={rows.length + 2}
      aria-activedescendant={cursorIndex >= 0 ? `${baseId}-${cursorIndex}` : undefined}
      onKeyDown={onKeyDown}
    >
      <div className="mm-cmp__sticky">
        <div className="mm-cmp__cols mm-cmp__groups" role="row" aria-rowindex={1}>
          <div role="columnheader" className="mm-cmp__group" aria-label={t.columns.mod} />
          <div role="columnheader" className="mm-cmp__group mm-cmp__group--left">
            <span className="mm-cmp__side-tag">{t.leftPrefix}</span>
            <span className="sv-truncate">{leftName}</span>
          </div>
          <div role="columnheader" className="mm-cmp__group mm-cmp__group--right">
            <span className="mm-cmp__side-tag">{t.rightPrefix}</span>
            <span className="sv-truncate">{rightName}</span>
          </div>
          <div role="columnheader" className="mm-cmp__group" aria-label={t.columns.state} />
        </div>
        <div className="sv-table__head mm-cmp__cols mm-cmp__head" role="row" aria-rowindex={2}>
          <div role="columnheader" className="sv-table__head-cell mm-cmp__c-title">
            {t.columns.mod}
          </div>
          <div role="columnheader" className="sv-table__head-cell mm-cmp__c-l-version">
            {t.columns.version}
          </div>
          <div role="columnheader" className="sv-table__head-cell mm-cmp__c-l-size mm-cmp__end">
            {t.columns.size}
          </div>
          <div role="columnheader" className="sv-table__head-cell mm-cmp__c-l-date">
            {t.columns.modified}
          </div>
          <div role="columnheader" className="sv-table__head-cell mm-cmp__c-r-version">
            {t.columns.version}
          </div>
          <div role="columnheader" className="sv-table__head-cell mm-cmp__c-r-size mm-cmp__end">
            {t.columns.size}
          </div>
          <div role="columnheader" className="sv-table__head-cell mm-cmp__c-r-date">
            {t.columns.modified}
          </div>
          <div role="columnheader" className="sv-table__head-cell mm-cmp__c-state">
            {t.columns.state}
          </div>
        </div>
      </div>

      <div className="mm-cmp__body" role="rowgroup" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((item) => {
          const row = rows[item.index];
          if (!row) return null;
          return (
            <CompareRowView
              key={row.modName}
              domId={`${baseId}-${item.index}`}
              row={row}
              index={item.index}
              top={item.start - STICKY_HEIGHT}
              selected={selection.ids.has(row.modName)}
              cursor={item.index === cursorIndex}
              onPress={onPress}
              onOpen={props.onOpen}
            />
          );
        })}
      </div>
    </div>
  );
}

interface RowProps {
  domId: string;
  row: CompareRow;
  index: number;
  top: number;
  selected: boolean;
  cursor: boolean;
  onPress: CollectionProps['onPress'];
  onOpen: CollectionProps['onOpen'];
}

const CompareRowView = memo(function CompareRowView({
  domId,
  row,
  index,
  top,
  selected,
  cursor,
  onPress,
  onOpen,
}: RowProps) {
  const view = compareStateView[row.state];
  return (
    <div
      id={domId}
      role="row"
      aria-rowindex={index + 3}
      aria-selected={selected}
      className={cx('sv-row', 'mm-cmp__cols', 'mm-cmp__row', cursor && 'is-cursor')}
      style={{ transform: `translateY(${top}px)` }}
      onClick={(event) => onPress(row.modName, pressModifiers(event))}
      onDoubleClick={() => onOpen(row.modName)}
    >
      <div role="gridcell" className="sv-cell mm-cmp__c-title">
        <span className="sv-truncate" title={row.modName}>
          {row.title}
        </span>
      </div>
      <SideCells side={row.left} prefix="l" />
      <SideCells side={row.right} prefix="r" />
      <div role="gridcell" className="sv-cell mm-cmp__c-state">
        <Tooltip label={t.stateHints[row.state]}>
          <Pill tone={view.tone} icon={view.icon} size="compact">
            {t.states[row.state]}
          </Pill>
        </Tooltip>
      </div>
    </div>
  );
});

function SideCells({ side, prefix }: { side: CompareSide | null; prefix: 'l' | 'r' }) {
  if (!side) {
    return (
      <>
        <div
          role="gridcell"
          className={cx('sv-cell', 'mm-cmp__none', `mm-cmp__c-${prefix}-version`)}
          aria-label={t.absent}
        >
          –
        </div>
        <div role="gridcell" className={cx('sv-cell', `mm-cmp__c-${prefix}-size`)} />
        <div role="gridcell" className={cx('sv-cell', `mm-cmp__c-${prefix}-date`)} />
      </>
    );
  }
  const { summary } = side;
  return (
    <>
      <div
        role="gridcell"
        className={cx('sv-cell sv-truncate sv-num', `mm-cmp__c-${prefix}-version`)}
        title={summary.fileName}
      >
        {summary.version}
      </div>
      <div
        role="gridcell"
        className={cx('sv-cell sv-cell--end sv-cell--secondary sv-num', `mm-cmp__c-${prefix}-size`)}
      >
        {formatBytes(summary.sizeBytes)}
      </div>
      <div
        role="gridcell"
        className={cx('sv-cell sv-cell--secondary sv-truncate sv-num', `mm-cmp__c-${prefix}-date`)}
      >
        {formatDate(summary.modified)}
      </div>
    </>
  );
}
