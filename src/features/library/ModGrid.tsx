import { useVirtualizer } from '@tanstack/react-virtual';
import { memo, useEffect, useId } from 'react';
import type { ModEntry } from '../../api';
import { formatBytes } from '../../lib/format';
import { useElementWidth } from '../../lib/useElementWidth';
import { strings } from '../../strings';
import { cx } from '../../ui/cx';
import { Pill } from '../../ui/Pill';
import { handleCollectionKey, pressModifiers, type CollectionProps } from './collection';
import { ModIcon } from './ModIcon';
import { StatusMarks } from './StatusMarks';

const CARD_MIN_WIDTH = 150;
const CARD_HEIGHT = 204;
const GAP = 8;
const PADDING = 8;
const ROW_HEIGHT = CARD_HEIGHT + GAP;

export function ModGrid(props: CollectionProps) {
  const { mods, selection, label, onPress, onOpen } = props;
  const [scrollRef, width] = useElementWidth<HTMLDivElement>();
  const baseId = useId();

  const perRow = Math.max(1, Math.floor((width - PADDING * 2 + GAP) / (CARD_MIN_WIDTH + GAP)));
  const rowCount = Math.ceil(mods.length / perRow);

  // TanStack Virtual hands back functions that change between renders by design.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 3,
    paddingStart: PADDING,
  });

  const cursorIndex =
    selection.cursor === null ? -1 : mods.findIndex((mod) => mod.id === selection.cursor);
  const cursorRow = cursorIndex >= 0 ? Math.floor(cursorIndex / perRow) : -1;

  useEffect(() => {
    if (cursorRow >= 0) virtualizer.scrollToIndex(cursorRow);
  }, [cursorRow, virtualizer]);

  return (
    <div
      ref={scrollRef}
      className="mm-collection mm-modgrid"
      role="listbox"
      tabIndex={0}
      aria-label={label}
      aria-multiselectable="true"
      aria-activedescendant={cursorIndex >= 0 ? `${baseId}-${cursorIndex}` : undefined}
      onKeyDown={(event) =>
        handleCollectionKey(event, props, {
          columns: perRow,
          pageRows: Math.max(1, Math.floor(event.currentTarget.clientHeight / ROW_HEIGHT)),
        })
      }
    >
      {/* Nothing is laid out until the width is known, so cards never flash at the wrong size. */}
      {width > 0 && (
        <div className="mm-modgrid__body" style={{ height: virtualizer.getTotalSize() + PADDING }}>
          {virtualizer.getVirtualItems().map((row) => {
            const first = row.index * perRow;
            return (
              <div
                key={row.index}
                className="mm-modgrid__row"
                role="presentation"
                style={{
                  transform: `translateY(${row.start}px)`,
                  gridTemplateColumns: `repeat(${perRow}, minmax(0, 1fr))`,
                }}
              >
                {mods.slice(first, first + perRow).map((mod, offset) => (
                  <ModCard
                    key={mod.id}
                    domId={`${baseId}-${first + offset}`}
                    mod={mod}
                    selected={selection.ids.has(mod.id)}
                    cursor={first + offset === cursorIndex}
                    onPress={onPress}
                    onOpen={onOpen}
                  />
                ))}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

interface ModCardProps {
  domId: string;
  mod: ModEntry;
  selected: boolean;
  cursor: boolean;
  onPress: CollectionProps['onPress'];
  onOpen: CollectionProps['onOpen'];
}

const ModCard = memo(function ModCard({
  domId,
  mod,
  selected,
  cursor,
  onPress,
  onOpen,
}: ModCardProps) {
  return (
    <div
      id={domId}
      role="option"
      aria-selected={selected}
      className={cx('mm-card', cursor && 'is-cursor')}
      onClick={(event) => onPress(mod.id, pressModifiers(event))}
      onDoubleClick={() => onOpen(mod.id)}
    >
      <div className="mm-card__stage">
        <ModIcon mod={mod} size="card" />
        <div className="mm-card__marks">
          <StatusMarks mod={mod} />
        </div>
      </div>
      <div className="mm-card__body">
        <div className="mm-card__title sv-truncate" title={mod.fileName}>
          {mod.title}
        </div>
        <div className="mm-card__meta sv-truncate">
          {mod.author || strings.inspector.unknownAuthor}
        </div>
        <div className="mm-card__foot">
          <Pill size="compact">{strings.kinds[mod.kind]}</Pill>
          <span className="mm-card__numbers sv-num sv-truncate">
            {mod.version && `${mod.version} · `}
            {formatBytes(mod.sizeBytes)}
          </span>
        </div>
      </div>
    </div>
  );
});
