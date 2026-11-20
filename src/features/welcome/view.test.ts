import { describe, expect, it } from 'vitest';
import type { GameStatus, UserDirSource } from '../../api';
import { workspaceView } from './view';

const game = (userDirSource: UserDirSource): GameStatus => ({
  gameVersion: null,
  gameRunning: false,
  serverRunning: false,
  settingsReadable: userDirSource !== 'none',
  overrideActive: false,
  effectiveModsDir: null,
  loggedDescVersion: null,
  store: null,
  launchable: false,
  userDirSource,
  userDir: null,
});

describe('workspaceView', () => {
  it('loads while the profiles load', () => {
    expect(workspaceView(false, 0, game('none'))).toBe('loading');
    expect(workspaceView(false, 3, game('detected'))).toBe('loading');
  });

  it('opens the library as soon as there is a folder, even without a game folder', () => {
    expect(workspaceView(true, 2, game('detected'))).toBe('ready');
    expect(workspaceView(true, 1, game('none'))).toBe('ready');
    expect(workspaceView(true, 1, null)).toBe('ready');
  });

  it('waits for the game status when the list is empty', () => {
    expect(workspaceView(true, 0, null)).toBe('loading');
  });

  it('asks for the game folder when none was found and there are no folders', () => {
    expect(workspaceView(true, 0, game('none'))).toBe('noGameFolder');
  });

  it('asks for a mod folder when the game folder is known but there are no folders', () => {
    expect(workspaceView(true, 0, game('detected'))).toBe('noFolders');
    expect(workspaceView(true, 0, game('chosen'))).toBe('noFolders');
  });
});
