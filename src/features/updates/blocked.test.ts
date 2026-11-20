import { describe, expect, it } from 'vitest';
import type { GameStatus } from '../../api';
import { strings } from '../../strings';
import { updateBlockedReason } from './blocked';

const game: GameStatus = {
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
};

describe('updateBlockedReason', () => {
  it('is null when nothing stands in the way', () => {
    expect(updateBlockedReason(game, false, false)).toBeNull();
  });

  it('names a running update check, with its own wording', () => {
    expect(updateBlockedReason(game, false, true)).toBe(strings.updates.blockedChecking);
    expect(strings.updates.blockedChecking).toBe('An update check is running.');
  });

  it('keeps the file-operation reasons first', () => {
    expect(updateBlockedReason({ ...game, gameRunning: true }, false, true)).toBe(
      strings.fileOps.blockedGame,
    );
    expect(updateBlockedReason(game, true, true)).toBe(strings.fileOps.blockedRunning);
  });
});
