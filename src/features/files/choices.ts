// The user's answers in the clash list, as pure functions so the rules can be tested without
// rendering anything. A decision is keyed by mod name and means "replace the existing mod";
// anything else is Skip, which is also the default for every clash.

import type { ClashChoice, TransferClash, TransferPlan } from '../../api';

export type ClashDecisions = ReadonlyMap<string, boolean>;

/** Every clash starts as Skip: nothing is replaced unless the user says so. */
export function defaultDecisions(clashes: readonly TransferClash[]): ClashDecisions {
  return new Map(clashes.map((clash) => [clash.modName, false]));
}

/** "Apply to all": sets every clash to Replace or to Skip. */
export function setAll(clashes: readonly TransferClash[], replace: boolean): ClashDecisions {
  return new Map(clashes.map((clash) => [clash.modName, replace]));
}

export function setOne(
  decisions: ClashDecisions,
  modName: string,
  replace: boolean,
): ClashDecisions {
  const next = new Map(decisions);
  next.set(modName, replace);
  return next;
}

/** What the Api takes: one choice per clash, in the order of the plan. */
export function toChoices(
  clashes: readonly TransferClash[],
  decisions: ClashDecisions,
): ClashChoice[] {
  return clashes.map((clash) => ({
    modName: clash.modName,
    replace: decisions.get(clash.modName) === true,
  }));
}

export function countReplaced(clashes: readonly TransferClash[], decisions: ClashDecisions) {
  return clashes.filter((clash) => decisions.get(clash.modName) === true).length;
}

/** How many mods the plan will actually carry over: every item except the clashes set to Skip. */
export function transferCount(plan: TransferPlan, decisions: ClashDecisions): number {
  const skipped = plan.clashes.length - countReplaced(plan.clashes, decisions);
  return Math.max(0, plan.items.length - skipped);
}

/** A copy needs room on the target drive; the plan says how many bytes it would write. */
export function lacksSpace(plan: TransferPlan): boolean {
  return plan.totalBytes > plan.freeBytes;
}

/** A move that writes nothing stays on one drive and is an instant rename. */
export function isSameDriveMove(plan: TransferPlan): boolean {
  return plan.mode === 'move' && plan.totalBytes === 0;
}

/** Names of the mods whose existing version goes to the Recycle Bin. */
export function replacedNames(
  clashes: readonly TransferClash[],
  decisions: ClashDecisions,
): string[] {
  return clashes.filter((clash) => decisions.get(clash.modName) === true).map((c) => c.modName);
}
