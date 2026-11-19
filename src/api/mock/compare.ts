// The mock's Compare: the same rules as the core's `compare_profiles`. The core's content check is
// a fingerprint of a zip's central directory; in the mock two copies have the same content when
// their size is the same, so a re-zipped copy (same version, other size) shows as content-differs.

import type { CompareRow, CompareSide, CompareState, Comparison, ModEntry } from '..';
import { nameKey, toSummary } from './files';

const side = (mod: ModEntry | undefined): CompareSide | null =>
  mod ? { modId: mod.id, summary: toSummary(mod) } : null;

function stateOf(left: ModEntry | undefined, right: ModEntry | undefined): CompareState {
  if (!left) return 'onlyRight';
  if (!right) return 'onlyLeft';
  if (left.version !== right.version) return 'versionDiffers';
  if (left.sizeBytes !== right.sizeBytes) return 'contentDiffers';
  return 'identical';
}

/** One row per mod name found in either list, sorted by lower-cased name. */
export function compareMods(
  leftProfileId: string,
  left: readonly ModEntry[],
  rightProfileId: string,
  right: readonly ModEntry[],
): Comparison {
  const lefts = new Map(left.map((mod) => [nameKey(mod.modName), mod]));
  const rights = new Map(right.map((mod) => [nameKey(mod.modName), mod]));
  const keys = [...new Set([...lefts.keys(), ...rights.keys()])].sort();
  const rows = keys.map((key): CompareRow => {
    const l = lefts.get(key);
    const r = rights.get(key);
    const named = l ?? r;
    return {
      modName: named?.modName ?? key,
      title: l?.title ?? r?.title ?? key,
      left: side(l),
      right: side(r),
      state: stateOf(l, r),
    };
  });
  return { leftProfileId, rightProfileId, rows };
}
