import { describe, expect, it } from 'vitest';
import type { GameStatus } from '../../api';
import { strings } from '../../strings';
import { switchBlockedReason } from './blocked';

const idle: GameStatus = {
  gameVersion: '1.25.1.0',
  gameRunning: false,
  serverRunning: false,
  settingsReadable: true,
  overrideActive: true,
  effectiveModsDir: null,
  loggedDescVersion: 106,
  store: 'steam',
  launchable: true,
  userDirSource: 'detected',
  userDir: null,
};

describe('switchBlockedReason', () => {
  it('allows switching while nothing runs and before the status is known', () => {
    expect(switchBlockedReason(idle)).toBeNull();
    expect(switchBlockedReason(null)).toBeNull();
  });

  it('words the game and the server separately', () => {
    expect(switchBlockedReason({ ...idle, gameRunning: true })).toBe(strings.switching.blockedGame);
    expect(switchBlockedReason({ ...idle, serverRunning: true })).toBe(
      strings.switching.blockedServer,
    );
    expect(strings.switching.blockedGame).not.toBe(strings.switching.blockedServer);
  });

  it('names the game first when both run', () => {
    expect(switchBlockedReason({ ...idle, gameRunning: true, serverRunning: true })).toBe(
      strings.switching.blockedGame,
    );
  });
});
