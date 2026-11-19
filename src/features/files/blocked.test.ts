import { describe, expect, it } from 'vitest';
import type { GameStatus } from '../../api';
import { strings } from '../../strings';
import { fileOpBlockedReason } from './blocked';
import { createProgressStore, progressFraction } from './progress';

const game = (overrides: Partial<GameStatus> = {}): GameStatus => ({
  gameVersion: '1.25.1.0',
  gameRunning: false,
  serverRunning: false,
  settingsReadable: true,
  overrideActive: true,
  effectiveModsDir: null,
  loggedDescVersion: null,
  store: 'steam',
  launchable: true,
  userDirSource: 'detected',
  userDir: null,
  ...overrides,
});

describe('fileOpBlockedReason', () => {
  it('is null when nothing stands in the way, including before the status is known', () => {
    expect(fileOpBlockedReason(game(), false)).toBeNull();
    expect(fileOpBlockedReason(null, false)).toBeNull();
  });

  it('names the game, then the server, then a running operation', () => {
    expect(fileOpBlockedReason(game({ gameRunning: true }), false)).toBe(
      strings.fileOps.blockedGame,
    );
    expect(fileOpBlockedReason(game({ serverRunning: true }), false)).toBe(
      strings.fileOps.blockedServer,
    );
    expect(fileOpBlockedReason(game(), true)).toBe(strings.fileOps.blockedRunning);
    expect(fileOpBlockedReason(game({ gameRunning: true, serverRunning: true }), true)).toBe(
      strings.fileOps.blockedGame,
    );
  });
});

describe('progressFraction', () => {
  const progress = (
    doneBytes: number,
    totalBytes: number,
    doneItems: number,
    totalItems: number,
  ) => ({
    doneBytes,
    totalBytes,
    current: null,
    doneItems,
    totalItems,
  });

  it('goes by bytes when there are bytes, else by items, else zero', () => {
    expect(progressFraction(progress(25, 100, 1, 1))).toBe(0.25);
    expect(progressFraction(progress(0, 0, 1, 4))).toBe(0.25);
    expect(progressFraction(progress(0, 0, 0, 0))).toBe(0);
    expect(progressFraction(null)).toBe(0);
  });

  it('stays between 0 and 1', () => {
    expect(progressFraction(progress(150, 100, 0, 0))).toBe(1);
    expect(progressFraction(progress(-5, 100, 0, 0))).toBe(0);
  });
});

describe('createProgressStore', () => {
  it('holds the latest value and tells listeners until they unsubscribe', () => {
    const store = createProgressStore();
    const seen: unknown[] = [];
    const stop = store.subscribe(() => seen.push(store.get()));
    const value = { doneBytes: 1, totalBytes: 2, current: 'a.zip', doneItems: 0, totalItems: 1 };
    store.set(value);
    expect(store.get()).toBe(value);
    stop();
    store.set(null);
    expect(seen).toEqual([value]);
    expect(store.get()).toBeNull();
  });
});
