import { describe, expect, it } from 'vitest';
import type { GameStatus } from '../../api';
import { strings } from '../../strings';
import { launchView } from './launch';

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
    expect(launchView(game(), false)).toEqual({ starting: false, blockedReason: null });
  });

  it('waits for the first status read', () => {
    const view = launchView(null, false);
    expect(view.blockedReason).toBe(strings.header.launchChecking);
    expect(view.starting).toBe(false);
  });

  it('is refused with a reason while the game is running', () => {
    expect(launchView(game({ gameRunning: true }), false).blockedReason).toBe(
      strings.header.launchBlockedRunning,
    );
  });

  it('says why when no installation was found', () => {
    expect(launchView(game({ launchable: false, store: null }), false).blockedReason).toBe(
      strings.header.launchBlockedNone,
    );
  });

  it('shows Starting until the game is seen running, then says it is running', () => {
    const starting = launchView(game(), true);
    expect(starting.starting).toBe(true);
    expect(starting.blockedReason).toBe(strings.header.launchStartingReason);

    const running = launchView(game({ gameRunning: true }), true);
    expect(running.starting).toBe(false);
    expect(running.blockedReason).toBe(strings.header.launchBlockedRunning);
  });
});
