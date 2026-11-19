import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ModEntry } from '../../api';
import { api } from '../../api/client';
import { describeError } from '../../lib/errors';
import { strings } from '../../strings';

export interface ModMetaFailure {
  title: string;
  message: string;
}

/** Tags and notes of the mods on screen: what the inspector edits. */
export interface ModMetaController {
  /** Every tag in use in any folder, sorted (for suggestions). */
  knownTags: readonly string[];
  /**
   * Gives each mod the tags `next` makes from its own. The list changes at once; a mod whose save
   * fails goes back to what it had, and `failure` says so. Resolves when all have settled.
   */
  setTags: (mods: readonly ModEntry[], next: (current: string[]) => string[]) => Promise<void>;
  /** Saves the note (an empty one clears it). Resolves true when it was saved. */
  setNote: (mod: ModEntry, note: string) => Promise<boolean>;
  failure: ModMetaFailure | null;
  dismissFailure: () => void;
}

const sameTags = (a: readonly string[], b: readonly string[]) =>
  a.length === b.length && a.every((tag, index) => tag === b[index]);

const keyOf = (mod: Pick<ModEntry, 'modName'>) => mod.modName.toLowerCase();

/**
 * Optimistic editing of tags and notes. `update` changes the loaded list in place (tags and notes
 * belong to a mod name, so every entry with that name changes). Failures roll the entry back and
 * are kept in `failure` until dismissed or the next edit starts.
 */
export function useModMeta(
  update: (change: (mods: ModEntry[]) => ModEntry[]) => void,
): ModMetaController {
  const [knownTags, setKnownTags] = useState<readonly string[]>([]);
  const [tagsVersion, setTagsVersion] = useState(0);
  const [failure, setFailure] = useState<ModMetaFailure | null>(null);

  useEffect(() => {
    let current = true;
    api.listTags().then(
      (tags) => {
        if (current) setKnownTags(tags);
      },
      () => undefined,
    );
    return () => {
      current = false;
    };
  }, [tagsVersion]);

  const patch = useCallback(
    (changes: ReadonlyMap<string, Partial<Pick<ModEntry, 'tags' | 'note'>>>) =>
      update((mods) =>
        mods.map((mod) => {
          const change = changes.get(keyOf(mod));
          return change ? { ...mod, ...change } : mod;
        }),
      ),
    [update],
  );

  const setTags = useCallback<ModMetaController['setTags']>(
    async (mods, next) => {
      const jobs = mods
        .map((mod) => ({ mod, tags: next(mod.tags) }))
        .filter((job) => !sameTags(job.tags, job.mod.tags));
      if (jobs.length === 0) return;
      setFailure(null);
      patch(new Map(jobs.map((job) => [keyOf(job.mod), { tags: job.tags }])));

      let message: string | null = null;
      await Promise.all(
        jobs.map(async ({ mod, tags }) => {
          try {
            const stored = await api.setTags(mod.modName, tags);
            patch(new Map([[keyOf(mod), { tags: stored }]]));
          } catch (error) {
            message = describeError(error);
            patch(new Map([[keyOf(mod), { tags: mod.tags }]]));
          }
        }),
      );
      setTagsVersion((count) => count + 1);
      if (message !== null) setFailure({ title: strings.actionFailed.saveTags, message });
    },
    [patch],
  );

  const setNote = useCallback<ModMetaController['setNote']>(
    async (mod, note) => {
      setFailure(null);
      const stored = note.trim() === '' ? null : note;
      patch(new Map([[keyOf(mod), { note: stored }]]));
      try {
        await api.setNote(mod.modName, note);
        return true;
      } catch (error) {
        patch(new Map([[keyOf(mod), { note: mod.note }]]));
        setFailure({ title: strings.actionFailed.saveNote, message: describeError(error) });
        return false;
      }
    },
    [patch],
  );

  const dismissFailure = useCallback(() => setFailure(null), []);
  return useMemo(
    () => ({ knownTags, setTags, setNote, failure, dismissFailure }),
    [knownTags, setTags, setNote, failure, dismissFailure],
  );
}
