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

/** Refusals about one mod (its match, its file, its folder): with several mods, only that mod fails. */
const ABOUT_ONE_MOD: ReadonlySet<ErrorCode> = new Set<ErrorCode>([
  'notInstallable',
  'folderMissing',
  'folderUnreadable',
  'profileNotFound',
  'notEnoughSpace',
  'verifyFailed',
  'targetChanged',
]);

/**
 * Runs the jobs one after another through `install` and joins their results, naming each entry
 * by folder (one job; `folderNames` maps a profile id to its display name) or by mod (several
 * jobs, each in one folder). With several mods, a refusal about one mod (`ABOUT_ONE_MOD`) fails
 * only that mod. Any other refusal ends the run: before anything was done it is the failure
 * itself; after that, every job still waiting fails with its code. After
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
  const failRest = (from: number, code: ErrorCode, to = jobs.length) => {
    for (const job of jobs.slice(from, to)) {
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
      if (!(error instanceof ApiFailure)) throw error;
      if (!single && ABOUT_ONE_MOD.has(error.code)) {
        failRest(index, error.code, index + 1);
        continue;
      }
      // Nothing happened yet: the refusal is the answer, as for copy and move.
      if (index === 0) throw error;
      failRest(index, error.code);
      break;
    }
  }
  return joined;
}
