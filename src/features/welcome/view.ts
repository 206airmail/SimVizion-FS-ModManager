import type { GameStatus } from '../../api';

/** What the library area shows. `ready` means there is at least one folder to open. */
export type WorkspaceView = 'loading' | 'noGameFolder' | 'noFolders' | 'ready';

/**
 * Decides the library area before a profile is chosen. With no folder registered, the screen
 * depends on whether the game's user folder was found: without it the first-run screen asks for
 * it; with it the screen asks for a mod folder. The game status loads on its own schedule, so an
 * empty list waits for it rather than guessing.
 */
export function workspaceView(
  profilesLoaded: boolean,
  profileCount: number,
  game: GameStatus | null,
): WorkspaceView {
  if (!profilesLoaded) return 'loading';
  if (profileCount > 0) return 'ready';
  if (game === null) return 'loading';
  return game.userDirSource === 'none' ? 'noGameFolder' : 'noFolders';
}
