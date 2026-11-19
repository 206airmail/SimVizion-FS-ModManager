import { useVirtualizer } from '@tanstack/react-virtual';
import { ArrowDown, ArrowUp } from 'lucide-react';
import { memo, useEffect, useId, useRef } from 'react';
import type { ModEntry } from '../../api';
import { formatBytes } from '../../lib/format';
import { strings } from '../../strings';
import { cx } from '../../ui/cx';
import { Icon } from '../../ui/Icon';
import { Pill } from '../../ui/Pill';
import { handleCollectionKey, pressModifiers, type CollectionProps } from './collection';
import { sourceIcons } from './labels';
import { ModIcon } from './ModIcon';
import type { SortDirection, SortKey } from './query';
import { StatusMarks } from './StatusMarks';

const ROW_HEIGHT = 36;
const HEAD_HEIGHT = 32;

interface Column {
  id: string;
  label: string;
  sortKey?: SortKey;
  end?: boolean;
}

const t = strings.library;
const columns: readonly Column[] = [
  { id: 'title', label: t.columns.title, sortKey: 'title' },
  { id: 'author', label: t.columns.author, sortKey: 'author' },
  { id: 'version', label: t.columns.version, sortKey: 'version' },
  { id: 'kind', label: t.columns.kind, sortKey: 'kind' },
  { id: 'size', label: t.columns.size, sortKey: 'size', end: true },
  { id: 'source', label: t.columns.source, sortKey: 'source' },
  { id: 'status', label: t.columns.status },
];

export interface ModTableProps extends CollectionProps {
  sortKey: SortKey;
  sortDirection: SortDirection;
  onSort: (key: SortKey) => void;
}

export function ModTable(props: ModTableProps) {
  const { mods, selection, label, sortKey, sortDirection, onSort, onPress, onOpen } = props;
  const scrollRef = useRef<HTMLDivElement>(null);
  const baseId = useId();

  // TanStack Virtual hands back functions that change between renders by design.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: mods.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
    // The sticky header sits inside the scroll area, above the rows.
    scrollMargin: HEAD_HEIGHT,
    scrollPaddingStart: HEAD_HEIGHT,
  });

  const cursorIndex =
    selection.cursor === null ? -1 : mods.findIndex((mod) => mod.id === selection.cursor);

  useEffect(() => {
    if (cursorIndex >= 0) virtualizer.scrollToIndex(cursorIndex);
  }, [cursorIndex, virtualizer]);

  return (
    <div
      ref={scrollRef}
      className="mm-collection mm-modtable"
      role="grid"
      tabIndex={0}
      aria-label={label}
      aria-multiselectable="true"
      aria-rowcount={mods.length + 1}
      aria-activedescendant={cursorIndex >= 0 ? `${baseId}-${cursorIndex}` : undefined}
      onKeyDown={(event) =>
        handleCollectionKey(event, props, {
          columns: 1,
          pageRows: Math.max(
            1,
            Math.floor((event.currentTarget.clientHeight - HEAD_HEIGHT) / ROW_HEIGHT) - 1,
          ),
        })
      }
    >
      <div
        className="sv-table__head mm-modtable__cols mm-modtable__head"
        role="row"
        aria-rowindex={1}
      >
        {columns.map((column) => {
          const active = column.sortKey === sortKey;
          return (
            <div
              key={column.id}
              role="columnheader"
              className={`mm-col--${column.id}`}
              aria-sort={!active ? undefined : sortDirection === 'asc' ? 'ascending' : 'descending'}
            >
              {column.sortKey ? (
                <button
                  type="button"
                  className={cx('sv-table__sort', column.end && 'sv-table__sort--end')}
                  data-active={active}
                  onClick={() => column.sortKey && onSort(column.sortKey)}
                >
                  {/* Right-aligned headers put the arrow first so the label keeps the numeric edge. */}
                  {column.end && <SortArrow descending={active && sortDirection === 'desc'} />}
                  <span className="sv-truncate">{column.label}</span>
                  {!column.end && <SortArrow descending={active && sortDirection === 'desc'} />}
                </button>
              ) : (
                <div className="sv-table__head-cell">{column.label}</div>
              )}
            </div>
          );
        })}
      </div>

      <div
        className="mm-modtable__body"
        role="rowgroup"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((item) => {
          const mod = mods[item.index];
          if (!mod) return null;
          return (
            <ModRow
              key={mod.id}
              domId={`${baseId}-${item.index}`}
              mod={mod}
              index={item.index}
              top={item.start - HEAD_HEIGHT}
              selected={selection.ids.has(mod.id)}
              cursor={item.index === cursorIndex}
              onPress={onPress}
              onOpen={onOpen}
            />
          );
        })}
      </div>
    </div>
  );
}

function SortArrow({ descending }: { descending: boolean }) {
  return (
    <Icon icon={descending ? ArrowDown : ArrowUp} size={16} className="sv-table__sort-arrow" />
  );
}

interface ModRowProps {
  domId: string;
  mod: ModEntry;
  index: number;
  top: number;
  selected: boolean;
  cursor: boolean;
  onPress: CollectionProps['onPress'];
  onOpen: CollectionProps['onOpen'];
}

const ModRow = memo(function ModRow({
  domId,
  mod,
  index,
  top,
  selected,
  cursor,
  onPress,
  onOpen,
}: ModRowProps) {
  const SourceIcon = sourceIcons[mod.source.type];
  return (
    <div
      id={domId}
      role="row"
      aria-rowindex={index + 2}
      aria-selected={selected}
      className={cx('sv-row', 'mm-modtable__cols', 'mm-modrow', cursor && 'is-cursor')}
      style={{ transform: `translateY(${top}px)` }}
      onClick={(event) => onPress(mod.id, pressModifiers(event))}
      onDoubleClick={() => onOpen(mod.id)}
    >
      <div role="gridcell" className="sv-cell mm-col--title mm-modrow__title">
        <ModIcon mod={mod} size="row" />
        <span className="sv-truncate" title={mod.fileName}>
          {mod.title}
        </span>
        {mod.format === 'folder' && <Pill size="compact">{t.folderFormat}</Pill>}
      </div>
      <div role="gridcell" className="sv-cell sv-cell--secondary sv-truncate mm-col--author">
        {mod.author}
      </div>
      <div role="gridcell" className="sv-cell sv-truncate sv-num mm-col--version">
        {mod.version}
      </div>
      <div role="gridcell" className="sv-cell sv-cell--secondary sv-truncate mm-col--kind">
        {strings.kinds[mod.kind]}
      </div>
      <div role="gridcell" className="sv-cell sv-cell--end sv-num mm-col--size">
        {formatBytes(mod.sizeBytes)}
      </div>
      <div role="gridcell" className="sv-cell sv-cell--secondary mm-col--source mm-modrow__source">
        {SourceIcon ? (
          <>
            <Icon icon={SourceIcon} size={16} />
            <span className="sv-truncate">{strings.sources[mod.source.type]}</span>
          </>
        ) : (
          <span className="mm-modrow__none" aria-label={strings.sources.unknown}>
            –
          </span>
        )}
      </div>
      <div role="gridcell" className="sv-cell mm-col--status">
        <StatusMarks mod={mod} />
      </div>
    </div>
  );
});
