// What the list and grid views share: the callbacks they raise and how they read the keyboard.

import type { KeyboardEvent } from 'react';
import type { ModEntry } from '../../api';
import type { NavigationLayout, Selection } from './selection';

export interface CollectionProps {
  mods: readonly ModEntry[];
  selection: Selection;
  /** Accessible name of the list. */
  label: string;
  /** A row was clicked. */
  onPress: (id: string, modifiers: { toggle: boolean; extend: boolean }) => void;
  /** A row was double-clicked, or Enter was pressed on it. */
  onOpen: (id: string) => void;
  /** A navigation key was pressed. Returns whether it was handled. */
  onNavigate: (key: string, extend: boolean, layout: NavigationLayout) => boolean;
  onSelectAll: () => void;
  onClear: () => void;
}

export function handleCollectionKey(
  event: KeyboardEvent<HTMLElement>,
  props: Pick<CollectionProps, 'selection' | 'onOpen' | 'onNavigate' | 'onSelectAll' | 'onClear'>,
  layout: NavigationLayout,
): void {
  // Keys pressed on a control inside the list, such as a column header, belong to that control.
  if (event.target !== event.currentTarget) return;

  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a') {
    event.preventDefault();
    props.onSelectAll();
  } else if (event.key === 'Enter') {
    if (props.selection.cursor !== null) props.onOpen(props.selection.cursor);
  } else if (event.key === 'Escape') {
    props.onClear();
  } else if (props.onNavigate(event.key, event.shiftKey, layout)) {
    event.preventDefault();
  }
}

export function pressModifiers(event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }) {
  return { toggle: event.ctrlKey || event.metaKey, extend: event.shiftKey };
}
