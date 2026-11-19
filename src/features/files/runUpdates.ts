import type { ErrorCode, OpProgress, OpResult } from '../../api';
import { ApiFailure } from '../../lib/errors';

/** One mod to update, in the folders (profile ids) the user chose for it. */
export interface UpdateJob {
  modName: string;
  profileIds: readonly string[];
}

/** `Api.installUpdate`, passed in so the sequencing can be tested without an Api. */
export type Install = (
  modName: string,
  profileIds: string[],
  onProgress: (progress: OpProgress) => void,
) => Promise<OpResult>;

/**
 * Runs the jobs one after another through `install` and joins their results, naming each entry
 * by folder (one job; `folderNames` maps a profile id to its display name) or by mod (several
 * jobs, each in one folder). A refusal of the whole call ends the run: before anything was done
 * it is the failure itself; after that, every job still waiting fails with its code. After
 * Cancel, the jobs not started are reported as cancelled. Progress of several mods counts mods
 * (the bar moves one step per mod).
 */
export async function runUpdates(
  install: Install,
  jobs: readonly UpdateJob[],
  folderNames: ReadonlyMap<string, string>,
  onProgress: (progress: OpProgress) => void,
  isCancelled: () => boolean,
): Promise<OpResult> {
  const single = jobs.length === 1;
  const joined: OpResult = { done: [], skipped: [], failed: [] };
  const labelFor = (job: UpdateJob, profileId: string) =>
    single ? (folderNames.get(profileId) ?? profileId) : job.modName;
  const failRest = (from: number, code: ErrorCode) => {
    for (const job of jobs.slice(from)) {
      for (const id of job.profileIds) {
        joined.failed.push({ modName: labelFor(job, id), code });
      }
    }
  };

  for (const [index, job] of jobs.entries()) {
    if (isCancelled()) {
      failRest(index, 'cancelled');
      break;
    }
    try {
      const result = await install(
        job.modName,
        [...job.profileIds],
        single
          ? onProgress
          : (p) =>
              onProgress({
                doneBytes: 0,
                totalBytes: 0,
                current: p.current,
                doneItems: index,
                totalItems: jobs.length,
              }),
      );
      for (const id of result.done) joined.done.push(labelFor(job, id));
      for (const id of result.skipped) joined.skipped.push(labelFor(job, id));
      for (const failure of result.failed) {
        joined.failed.push({ modName: labelFor(job, failure.modName), code: failure.code });
      }
    } catch (error) {
      // Nothing happened yet: the refusal is the answer, as for copy and move.
      if (index === 0 || !(error instanceof ApiFailure)) throw error;
      failRest(index, error.code);
      break;
    }
  }
  return joined;
}
