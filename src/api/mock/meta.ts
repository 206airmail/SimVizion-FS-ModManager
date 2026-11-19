// The mock's tags and notes: the validation rules of the core's `set_tags` and `set_note`, and the
// per-name store. Both are keyed by lower-cased mod name, so they are shared by every profile.

import type { ModEntry } from '..';
import { ApiFailure } from '../../lib/errors';
import { nameKey } from './files';

export const MAX_TAG_LENGTH = 40;
export const MAX_TAGS = 20;
export const MAX_NOTE_LENGTH = 4000;

const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });
const length = (text: string) => [...text].length;

/**
 * Trims every tag, drops empty ones and keeps a case-insensitive duplicate once, with its first
 * spelling. Rejects a tag over 40 characters or more than 20 tags. Returns the tags sorted.
 */
export function normalizeTags(tags: readonly string[]): string[] {
  const seen = new Map<string, string>();
  for (const raw of tags) {
    const tag = raw.trim();
    if (tag === '') continue;
    if (length(tag) > MAX_TAG_LENGTH) throw new ApiFailure('internal', 'invalidTag');
    const key = tag.toLowerCase();
    if (!seen.has(key)) seen.set(key, tag);
  }
  if (seen.size > MAX_TAGS) throw new ApiFailure('internal', 'invalidTag');
  return [...seen.values()].sort(collator.compare);
}

/** The stored note: whitespace-only means none. Rejects a note over 4,000 characters. */
export function normalizeNote(note: string): string | null {
  if (length(note) > MAX_NOTE_LENGTH) throw new ApiFailure('internal', 'invalidNote');
  return note.trim() === '' ? null : note;
}

/** One mock instance's tags and notes. */
export interface MetaStore {
  tags: Map<string, string[]>;
  notes: Map<string, string>;
}

export const emptyMeta = (): MetaStore => ({ tags: new Map(), notes: new Map() });

/** The mod with its tags and note filled in; the same object when it has neither. */
export function withMeta(mod: ModEntry, meta: MetaStore): ModEntry {
  const key = nameKey(mod.modName);
  const tags = meta.tags.get(key);
  const note = meta.notes.get(key);
  if (tags === undefined && note === undefined) return mod;
  return { ...mod, tags: tags ? [...tags] : [], note: note ?? null };
}

/** Every distinct tag in use, one spelling each (the first stored), sorted. */
export function allTags(meta: MetaStore): string[] {
  const distinct = new Map<string, string>();
  for (const tags of meta.tags.values()) {
    for (const tag of tags) {
      if (!distinct.has(tag.toLowerCase())) distinct.set(tag.toLowerCase(), tag);
    }
  }
  return [...distinct.values()].sort(collator.compare);
}

/**
 * Starting tags and notes, so the Inspector, the tag filter and the table have something to show:
 * the first few healthy mods of a profile (the same ones every run).
 */
export function seedMeta(mods: readonly ModEntry[]): MetaStore {
  const meta = emptyMeta();
  const picks = mods.filter((mod) => mod.issues.length === 0 && mod.kind !== 'map').slice(0, 12);
  const plan: [string[], string | null][] = [
    [['Favourite', 'Realistic'], 'Great with the baler pack. Keep it updated.'],
    [['Favourite'], null],
    [['Testing'], 'Seems to conflict with the snow mod. Retest after the next update.'],
    [['Realistic'], null],
    [['Keep', 'Multiplayer'], null],
    [['Multiplayer'], null],
    [['Favourite', 'Multiplayer'], null],
    [['Testing'], null],
  ];
  plan.forEach(([tags, note], index) => {
    const mod = picks[index];
    if (!mod) return;
    const key = nameKey(mod.modName);
    meta.tags.set(key, normalizeTags(tags));
    if (note) meta.notes.set(key, note);
  });
  return meta;
}
