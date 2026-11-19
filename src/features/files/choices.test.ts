import { describe, expect, it } from 'vitest';
import type { TransferClash, TransferPlan } from '../../api';
import {
  countReplaced,
  defaultDecisions,
  isSameDriveMove,
  lacksSpace,
  replacedNames,
  setAll,
  setOne,
  toChoices,
  transferCount,
} from './choices';

const summary = { fileName: 'a.zip', version: '1.0.0.0', sizeBytes: 10, modified: '2026-11-01' };
const clash = (modName: string): TransferClash => ({
  modName,
  incoming: summary,
  existing: summary,
});
const clashes = [clash('FS25_A'), clash('FS25_B'), clash('FS25_C')];

const plan = (overrides: Partial<TransferPlan> = {}): TransferPlan => ({
  planId: 'plan-1',
  fromProfileId: 'main',
  toProfileId: 'testing',
  mode: 'copy',
  items: ['FS25_A', 'FS25_B', 'FS25_C', 'FS25_D', 'FS25_E'].map((modName) => ({
    modId: `${modName}.zip`,
    modName,
    sizeBytes: 10,
  })),
  clashes,
  totalBytes: 50,
  freeBytes: 1000,
  ...overrides,
});

describe('clash decisions', () => {
  it('start as Skip for every clash', () => {
    const decisions = defaultDecisions(clashes);
    expect([...decisions.values()]).toEqual([false, false, false]);
    expect(toChoices(clashes, decisions).every((choice) => !choice.replace)).toBe(true);
  });

  it('apply to all sets every clash, then one row can still differ', () => {
    const all = setAll(clashes, true);
    expect(toChoices(clashes, all).map((c) => c.replace)).toEqual([true, true, true]);
    const one = setOne(all, 'FS25_B', false);
    expect(toChoices(clashes, one).map((c) => c.replace)).toEqual([true, false, true]);
    expect(toChoices(clashes, setAll(clashes, false)).every((c) => !c.replace)).toBe(true);
  });

  it('never mutate the decisions they are given', () => {
    const start = defaultDecisions(clashes);
    setOne(start, 'FS25_A', true);
    setAll(clashes, true);
    expect(start.get('FS25_A')).toBe(false);
  });

  it('give the Api one choice per clash in plan order, and treat an unknown name as Skip', () => {
    const decisions = setOne(defaultDecisions(clashes), 'FS25_C', true);
    expect(toChoices(clashes, decisions)).toEqual([
      { modName: 'FS25_A', replace: false },
      { modName: 'FS25_B', replace: false },
      { modName: 'FS25_C', replace: true },
    ]);
    expect(toChoices(clashes, new Map())).toEqual([
      { modName: 'FS25_A', replace: false },
      { modName: 'FS25_B', replace: false },
      { modName: 'FS25_C', replace: false },
    ]);
  });

  it('count what will be replaced and name it', () => {
    const decisions = setOne(setOne(defaultDecisions(clashes), 'FS25_A', true), 'FS25_C', true);
    expect(countReplaced(clashes, decisions)).toBe(2);
    expect(replacedNames(clashes, decisions)).toEqual(['FS25_A', 'FS25_C']);
  });
});

describe('transfer count', () => {
  it('leaves out the clashes that are set to Skip', () => {
    expect(transferCount(plan(), defaultDecisions(clashes))).toBe(2);
    expect(transferCount(plan(), setOne(defaultDecisions(clashes), 'FS25_A', true))).toBe(3);
    expect(transferCount(plan(), setAll(clashes, true))).toBe(5);
  });

  it('is zero when everything is skipped and never negative', () => {
    const all = plan({ items: plan().items.slice(0, 3) });
    expect(transferCount(all, defaultDecisions(clashes))).toBe(0);
    expect(transferCount(plan({ items: [] }), defaultDecisions(clashes))).toBe(0);
  });

  it('is every item when there are no clashes', () => {
    expect(transferCount(plan({ clashes: [] }), new Map())).toBe(5);
  });
});

describe('space and drive', () => {
  it('lacks space only when the bytes to write exceed the free space', () => {
    expect(lacksSpace(plan({ totalBytes: 1001, freeBytes: 1000 }))).toBe(true);
    expect(lacksSpace(plan({ totalBytes: 1000, freeBytes: 1000 }))).toBe(false);
    expect(lacksSpace(plan({ totalBytes: 0, freeBytes: 0 }))).toBe(false);
  });

  it('knows a move that writes nothing is on one drive', () => {
    expect(isSameDriveMove(plan({ mode: 'move', totalBytes: 0 }))).toBe(true);
    expect(isSameDriveMove(plan({ mode: 'move', totalBytes: 5 }))).toBe(false);
    expect(isSameDriveMove(plan({ mode: 'copy', totalBytes: 0 }))).toBe(false);
  });
});
