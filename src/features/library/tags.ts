// The rules of the tag editor, as pure functions so they are tested without rendering. They mirror
// the core's: a tag is trimmed, matched without regard to case, at most 40 characters, and a mod
// has at most 20.

import type { ModEntry } from '../../api';

export const MAX_TAG_LENGTH = 40;
export const MAX_TAGS = 20;
export const MAX_NOTE_LENGTH = 4000;

const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });

const keyOf = (tag: string) => tag.toLowerCase();
const lengthOf = (text: string) => [...text].length;

/** A tag, and how many of the mods it was counted over have it. */
export interface TagCount {
  tag: string;
  count: number;
}

/** The tags the mods have, one entry per tag (case-insensitive, first spelling), sorted. */
export function tagCounts(mods: readonly Pick<ModEntry, 'tags'>[]): TagCount[] {
  const counts = new Map<string, TagCount>();
  for (const mod of mods) {
    const seen = new Set<string>();
    for (const tag of mod.tags) {
      const key = keyOf(tag);
      if (seen.has(key)) continue;
      seen.add(key);
      const entry = counts.get(key);
      if (entry) entry.count += 1;
      else counts.set(key, { tag, count: 1 });
    }
  }
  return [...counts.values()].sort((a, b) => collator.compare(a.tag, b.tag));
}

/** The tags typed into the field: split at commas and line breaks, trimmed, empty ones dropped. */
export function parseTagInput(text: string): string[] {
  return text
    .split(/[,\n]/)
    .map((part) => part.trim())
    .filter((part) => part !== '');
}

export const hasTag = (tags: readonly string[], tag: string): boolean =>
  tags.some((existing) => keyOf(existing) === keyOf(tag));

/** `tags` with `add` included once (an existing spelling wins), sorted. */
export function addTags(tags: readonly string[], add: readonly string[]): string[] {
  const result = [...tags];
  for (const tag of add) if (!hasTag(result, tag)) result.push(tag);
  return result.sort(collator.compare);
}

/** `tags` without `tag`, matched without regard to case. */
export function removeTag(tags: readonly string[], tag: string): string[] {
  return tags.filter((existing) => keyOf(existing) !== keyOf(tag));
}

export type TagProblem = 'tooLong' | 'tooMany';

/** Why `tag` cannot be added to all of these mods, or null. A mod that has it already is fine. */
export function tagProblem(
  mods: readonly Pick<ModEntry, 'tags'>[],
  tag: string,
): TagProblem | null {
  if (lengthOf(tag) > MAX_TAG_LENGTH) return 'tooLong';
  if (mods.some((mod) => !hasTag(mod.tags, tag) && mod.tags.length >= MAX_TAGS)) return 'tooMany';
  return null;
}

/**
 * Tags to offer while typing: the known ones that are not applied (to every mod being edited)
 * and that contain the text, those starting with it first. At most `limit`.
 */
export function suggestTags(
  known: readonly string[],
  applied: readonly TagCount[],
  total: number,
  text: string,
  limit = 6,
): string[] {
  const needle = text.trim().toLowerCase();
  const full = new Set(applied.filter((entry) => entry.count >= total).map((e) => keyOf(e.tag)));
  const fits = known.filter((tag) => !full.has(keyOf(tag)) && keyOf(tag).includes(needle));
  const starts = fits.filter((tag) => keyOf(tag).startsWith(needle));
  const rest = fits.filter((tag) => !keyOf(tag).startsWith(needle));
  return [...starts, ...rest].slice(0, limit);
}
