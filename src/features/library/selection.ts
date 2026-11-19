// Desktop-style multi-selection for the mod list: click, Ctrl+click, Shift+click and keyboard.
// Pure functions so the rules can be tested without rendering anything.

export interface Selection {
  ids: ReadonlySet<string>;
  /** Where a Shift range starts. */
  anchor: string | null;
  /** The row the keyboard is on. */
  cursor: string | null;
}

export const emptySelection: Selection = { ids: new Set(), anchor: null, cursor: null };

export function selectOnly(id: string): Selection {
  return { ids: new Set([id]), anchor: id, cursor: id };
}

/** Ctrl+click: adds or removes one row and moves the anchor to it. */
export function toggle(selection: Selection, id: string): Selection {
  const ids = new Set(selection.ids);
  if (!ids.delete(id)) ids.add(id);
  return { ids, anchor: id, cursor: id };
}

/** Shift+click: replaces the selection with the range from the anchor to `id`. */
export function extendTo(selection: Selection, order: readonly string[], id: string): Selection {
  const to = order.indexOf(id);
  if (to === -1) return selection;
  const anchorIndex = selection.anchor === null ? -1 : order.indexOf(selection.anchor);
  const from = anchorIndex === -1 ? to : anchorIndex;
  const ids = new Set(order.slice(Math.min(from, to), Math.max(from, to) + 1));
  return { ids, anchor: order[from] ?? id, cursor: id };
}

export function selectAll(selection: Selection, order: readonly string[]): Selection {
  return {
    ids: new Set(order),
    anchor: selection.anchor ?? order[0] ?? null,
    cursor: selection.cursor ?? order[0] ?? null,
  };
}

/** Drops rows that are no longer listed, for example after a filter change. */
export function restrictTo(selection: Selection, order: readonly string[]): Selection {
  if (selection.ids.size === 0) return selection;
  const listed = new Set(order);
  const ids = new Set([...selection.ids].filter((id) => listed.has(id)));
  if (ids.size === selection.ids.size) return selection;
  return {
    ids,
    anchor: selection.anchor !== null && listed.has(selection.anchor) ? selection.anchor : null,
    cursor: selection.cursor !== null && listed.has(selection.cursor) ? selection.cursor : null,
  };
}

export interface NavigationLayout {
  /** 1 for a list; the number of cards per row for a grid. */
  columns: number;
  /** Rows moved by Page Up and Page Down. */
  pageRows: number;
}

/**
 * Applies a navigation key. Returns the new selection, or null when the key is not one of ours
 * so the caller can leave the event alone.
 */
export function navigate(
  selection: Selection,
  order: readonly string[],
  key: string,
  extend: boolean,
  layout: NavigationLayout,
): Selection | null {
  if (order.length === 0) return null;
  const last = order.length - 1;
  const current = selection.cursor === null ? -1 : order.indexOf(selection.cursor);

  let target: number;
  switch (key) {
    case 'ArrowDown':
      target = current + layout.columns;
      break;
    case 'ArrowUp':
      target = current - layout.columns;
      break;
    case 'ArrowRight':
      if (layout.columns === 1) return null;
      target = current + 1;
      break;
    case 'ArrowLeft':
      if (layout.columns === 1) return null;
      target = current - 1;
      break;
    case 'PageDown':
      target = current + layout.columns * layout.pageRows;
      break;
    case 'PageUp':
      target = current - layout.columns * layout.pageRows;
      break;
    case 'Home':
      target = 0;
      break;
    case 'End':
      target = last;
      break;
    default:
      return null;
  }

  // With no cursor yet, any key lands on the first row (or the last, for End).
  if (current === -1) target = key === 'End' ? last : 0;
  const id = order[Math.min(Math.max(target, 0), last)];
  if (id === undefined) return null;
  return extend ? extendTo(selection, order, id) : selectOnly(id);
}
