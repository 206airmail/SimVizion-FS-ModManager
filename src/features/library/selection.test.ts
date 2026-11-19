import { describe, expect, it } from 'vitest';
import {
  emptySelection,
  extendTo,
  navigate,
  restrictTo,
  selectAll,
  selectOnly,
  toggle,
  type Selection,
} from './selection';

const order = ['a', 'b', 'c', 'd', 'e', 'f'];
const list = { columns: 1, pageRows: 3 };
const grid = { columns: 3, pageRows: 1 };
const ids = (selection: Selection) => [...selection.ids].sort();

describe('click selection', () => {
  it('selects one row and makes it the anchor and cursor', () => {
    expect(selectOnly('c')).toEqual({ ids: new Set(['c']), anchor: 'c', cursor: 'c' });
  });

  it('toggles rows in and out', () => {
    const added = toggle(selectOnly('a'), 'c');
    expect(ids(added)).toEqual(['a', 'c']);
    expect(added.anchor).toBe('c');
    expect(ids(toggle(added, 'a'))).toEqual(['c']);
  });

  it('extends from the anchor in either direction, replacing the old range', () => {
    const down = extendTo(selectOnly('b'), order, 'd');
    expect(ids(down)).toEqual(['b', 'c', 'd']);
    expect(down.anchor).toBe('b');
    expect(down.cursor).toBe('d');

    const up = extendTo(down, order, 'a');
    expect(ids(up)).toEqual(['a', 'b']);
    expect(up.anchor).toBe('b');
  });

  it('treats a Shift+click with no anchor as a plain selection', () => {
    expect(ids(extendTo(emptySelection, order, 'c'))).toEqual(['c']);
  });

  it('ignores a row that is not listed', () => {
    const selection = selectOnly('a');
    expect(extendTo(selection, order, 'zz')).toBe(selection);
  });

  it('selects everything', () => {
    expect(ids(selectAll(emptySelection, order))).toEqual(order);
  });
});

describe('restrictTo', () => {
  it('drops rows that are no longer listed', () => {
    const selection = extendTo(selectOnly('a'), order, 'c');
    const restricted = restrictTo(selection, ['b', 'c', 'd']);
    expect(ids(restricted)).toEqual(['b', 'c']);
    expect(restricted.anchor).toBeNull();
    expect(restricted.cursor).toBe('c');
  });

  it('returns the same object when nothing changed', () => {
    const selection = selectOnly('b');
    expect(restrictTo(selection, order)).toBe(selection);
  });
});

describe('keyboard navigation', () => {
  it('moves down and up a list', () => {
    expect(navigate(selectOnly('b'), order, 'ArrowDown', false, list)?.cursor).toBe('c');
    expect(navigate(selectOnly('b'), order, 'ArrowUp', false, list)?.cursor).toBe('a');
  });

  it('stops at the ends', () => {
    expect(navigate(selectOnly('a'), order, 'ArrowUp', false, list)?.cursor).toBe('a');
    expect(navigate(selectOnly('f'), order, 'PageDown', false, list)?.cursor).toBe('f');
  });

  it('starts at the first row when nothing has the cursor', () => {
    expect(navigate(emptySelection, order, 'ArrowDown', false, list)?.cursor).toBe('a');
    expect(navigate(emptySelection, order, 'End', false, list)?.cursor).toBe('f');
  });

  it('pages, and jumps to the ends', () => {
    expect(navigate(selectOnly('a'), order, 'PageDown', false, list)?.cursor).toBe('d');
    expect(navigate(selectOnly('e'), order, 'Home', false, list)?.cursor).toBe('a');
    expect(navigate(selectOnly('b'), order, 'End', false, list)?.cursor).toBe('f');
  });

  it('extends the selection with Shift', () => {
    const extended = navigate(selectOnly('b'), order, 'ArrowDown', true, list);
    expect(extended && ids(extended)).toEqual(['b', 'c']);
  });

  it('moves by row and by card in a grid', () => {
    expect(navigate(selectOnly('b'), order, 'ArrowDown', false, grid)?.cursor).toBe('e');
    expect(navigate(selectOnly('b'), order, 'ArrowRight', false, grid)?.cursor).toBe('c');
    expect(navigate(selectOnly('b'), order, 'ArrowLeft', false, grid)?.cursor).toBe('a');
  });

  it('leaves sideways keys and unrelated keys alone in a list', () => {
    expect(navigate(selectOnly('b'), order, 'ArrowRight', false, list)).toBeNull();
    expect(navigate(selectOnly('b'), order, 'x', false, list)).toBeNull();
  });

  it('does nothing for an empty list', () => {
    expect(navigate(emptySelection, [], 'ArrowDown', false, list)).toBeNull();
  });
});
