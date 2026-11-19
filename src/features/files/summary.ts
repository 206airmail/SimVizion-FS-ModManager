// What a finished copy, move or remove says, worded from the Api's `OpResult`.

import type { OpResult } from '../../api';
import { isErrorCode } from '../../lib/errors';
import { strings } from '../../strings';

export type OpKind = 'copy' | 'move' | 'remove';

/** A finished operation, as kept by `useFileOps` until the user dismisses it. */
export interface Outcome {
  kind: OpKind;
  /** Name of the target folder for a copy or move; null for a remove. */
  target: string | null;
  result: OpResult;
  /** Mods whose existing version the user chose to replace (it goes to the Recycle Bin). */
  replaced: readonly string[];
  /** The whole call ended with the `cancelled` code instead of a result. */
  cancelled: boolean;
}

export interface Problem {
  modName: string;
  reason: string;
}

export interface OutcomeView {
  tone: 'success' | 'warning' | 'error';
  title: string;
  text: string;
  /** Mods that failed for a reason other than the user cancelling, with the worded reason. */
  problems: Problem[];
}

const t = strings.fileOps.result;

export const emptyResult: OpResult = { done: [], skipped: [], failed: [] };

function title(kind: OpKind, doneCount: number, target: string | null): string {
  if (doneCount === 0) return t.nothing[kind];
  if (kind === 'remove') return t.removed(doneCount);
  const where = target ?? '';
  return kind === 'copy' ? t.copied(doneCount, where) : t.moved(doneCount, where);
}

export function describeOutcome(outcome: Outcome): OutcomeView {
  const { kind, target, result } = outcome;
  const doneCount = result.done.length;
  const notDone = result.failed.filter((failure) => failure.code === 'cancelled');
  const failed = result.failed.filter((failure) => failure.code !== 'cancelled');
  const nothingHappened =
    doneCount === 0 && result.skipped.length === 0 && result.failed.length === 0;

  if (outcome.cancelled && nothingHappened) {
    return { tone: 'warning', title: t.cancelled, text: strings.errors.cancelled, problems: [] };
  }

  const pieces: string[] = [];
  if (result.skipped.length > 0) pieces.push(t.skipped(result.skipped.length));
  if (failed.length > 0) pieces.push(t.failed(failed.length, kind));
  if (notDone.length > 0) pieces.push(t.notDone(notDone.length));
  const sentences: string[] = [];
  if (doneCount === 0 && failed.length === 0 && notDone.length === 0 && result.skipped.length > 0) {
    sentences.push(t.allSkipped);
  } else if (pieces.length > 0) {
    sentences.push(`${pieces.join(', ')}.`);
  }

  if (kind === 'remove' && doneCount > 0) sentences.push(t.recycleNote);
  const done = new Set(result.done);
  const replacedDone = outcome.replaced.filter((name) => done.has(name)).length;
  if (kind !== 'remove' && replacedDone > 0) sentences.push(t.replacedNote(replacedDone));

  const tone =
    failed.length > 0
      ? doneCount > 0
        ? 'warning'
        : 'error'
      : notDone.length > 0
        ? 'warning'
        : 'success';

  return {
    tone,
    title: title(kind, doneCount, target),
    text: sentences.join(' '),
    problems: failed.map((failure) => ({
      modName: failure.modName,
      reason: isErrorCode(failure.code) ? strings.errors[failure.code] : strings.errors.internal,
    })),
  };
}
