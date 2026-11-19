import { useEffect, useId, useRef, type KeyboardEvent } from 'react';
import type { Savegame } from '../../api';
import { formatDate } from '../../lib/format';
import { strings } from '../../strings';
import { cx } from '../../ui/cx';

const t = strings.savegames;

export interface SaveListProps {
  saves: readonly Savegame[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}

/**
 * The saves in the game's slots: name, map, date, play time, money and mod count. One is selected
 * at a time; the list takes the arrow keys, Home and End, and selecting follows the cursor.
 */
export function SaveList({ saves, selectedId, onSelect }: SaveListProps) {
  const baseId = useId();
  const selectedIndex = saves.findIndex((save) => save.id === selectedId);
  const listRef = useRef<HTMLDivElement>(null);

  // The list shows a few rows and scrolls: keep the selected one in view (below the sticky header).
  useEffect(() => {
    listRef.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' });
  }, [selectedId]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return;
    const last = saves.length - 1;
    const target =
      event.key === 'ArrowDown'
        ? Math.min(selectedIndex + 1, last)
        : event.key === 'ArrowUp'
          ? Math.max(selectedIndex - 1, 0)
          : event.key === 'Home'
            ? 0
            : event.key === 'End'
              ? last
              : null;
    const next = target === null ? undefined : saves[target];
    if (!next) return;
    event.preventDefault();
    onSelect(next.id);
  };

  return (
    <div
      ref={listRef}
      className="mm-savelist"
      role="grid"
      tabIndex={0}
      aria-label={t.listLabel}
      aria-rowcount={saves.length + 1}
      aria-activedescendant={selectedIndex >= 0 ? `${baseId}-${selectedIndex}` : undefined}
      onKeyDown={onKeyDown}
    >
      <div
        className="sv-table__head mm-savelist__cols mm-savelist__head"
        role="row"
        aria-rowindex={1}
      >
        <div role="columnheader" className="sv-table__head-cell">
          {t.listColumns.name}
        </div>
        <div role="columnheader" className="sv-table__head-cell mm-savelist__c-map">
          {t.listColumns.map}
        </div>
        <div role="columnheader" className="sv-table__head-cell">
          {t.listColumns.saved}
        </div>
        <div role="columnheader" className="sv-table__head-cell mm-savelist__end">
          {t.listColumns.played}
        </div>
        <div role="columnheader" className="sv-table__head-cell mm-savelist__end">
          {t.listColumns.money}
        </div>
        <div role="columnheader" className="sv-table__head-cell mm-savelist__end">
          {t.listColumns.mods}
        </div>
      </div>
      {saves.map((save, index) => (
        <div
          key={save.id}
          id={`${baseId}-${index}`}
          role="row"
          aria-rowindex={index + 2}
          aria-selected={save.id === selectedId}
          className={cx('sv-row', 'mm-savelist__cols', 'mm-savelist__row')}
          onClick={() => onSelect(save.id)}
        >
          <div role="gridcell" className="sv-cell mm-savelist__name">
            <span className="sv-truncate">{save.name}</span>
            <span className="mm-savelist__slot">{t.slot(save.slot)}</span>
          </div>
          <div
            role="gridcell"
            className="sv-cell sv-cell--secondary sv-truncate mm-savelist__c-map"
            title={save.mapMod ?? undefined}
          >
            {save.mapTitle}
          </div>
          <div role="gridcell" className="sv-cell sv-cell--secondary sv-truncate sv-num">
            {save.saveDate ? formatDate(save.saveDate) : t.unknownDate}
          </div>
          <div role="gridcell" className="sv-cell sv-cell--end sv-num sv-truncate">
            {t.playTime(save.playMinutes)}
          </div>
          <div role="gridcell" className="sv-cell sv-cell--end sv-num sv-truncate">
            {t.money(save.money)}
          </div>
          <div role="gridcell" className="sv-cell sv-cell--end sv-num">
            {save.modCount}
          </div>
        </div>
      ))}
    </div>
  );
}
