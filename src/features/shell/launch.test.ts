import { describe, expect, it } from 'vitest';
import type { GameStatus } from '../../api';
import { strings } from '../../strings';
import { launchView, withLaunching } from './launch';

const game = (changes: Partial<GameStatus> = {}): GameStatus => ({
  gameVersion: '1.25.1.0',
  gameRunning: false,
  serverRunning: false,
  settingsReadable: true,
  overrideActive: false,
  effectiveModsDir: null,
  loggedDescVersion: null,
  store: 'steam',
  launchable: true,
  userDirSource: 'detected',
  userDir: null,
  ...changes,
});

describe('launchView', () => {
  it('is available for a launchable game that is not running', () => {
    expect(launchView(game(), false, null)).toEqual({ starting: false, blockedReason: null });
  });

  it('waits for the first status read', () => {
    const view = launchView(null, false, null);
    expect(view.blockedReason).toBe(strings.header.launchChecking);
    expect(view.starting).toBe(false);
  });

  it('is refused with a reason while the game is running', () => {
    expect(launchView(game({ gameRunning: true }), false, null).blockedReason).toBe(
      strings.header.launchBlockedRunning,
    );
  });

  it('says why when no installation was found', () => {
    expect(launchView(game({ launchable: false, store: null }), false, null).blockedReason).toBe(
      strings.header.launchBlockedNone,
    );
  });

  it('shows Starting until the game is seen running, then says it is running', () => {
    const starting = launchView(game(), true, null);
    expect(starting.starting).toBe(true);
    expect(starting.blockedReason).toBe(strings.header.launchStartingReason);

    const running = launchView(game({ gameRunning: true }), true, null);
    expect(running.starting).toBe(false);
    expect(running.blockedReason).toBe(strings.header.launchBlockedRunning);
  });

  it('is refused with the given reason while the app is writing (a file operation or a switch)', () => {
    expect(launchView(game(), false, 'A file operation is running.')).toEqual({
      starting: false,
      blockedReason: 'A file operation is running.',
    });
  });

  it('names the running game before a running write', () => {
    expect(launchView(game({ gameRunning: true }), false, 'busy').blockedReason).toBe(
      strings.header.launchBlockedRunning,
    );
  });
});

describe('withLaunching', () => {
  it('marks the status as launching only while the game is not yet running', () => {
    expect(withLaunching(game(), true)).toEqual({ ...game(), launching: true });
    expect(withLaunching(game({ gameRunning: true }), true)).toEqual(game({ gameRunning: true }));
    expect(withLaunching(game(), false)).toEqual(game());
    expect(withLaunching(null, true)).toBeNull();
  });
});
