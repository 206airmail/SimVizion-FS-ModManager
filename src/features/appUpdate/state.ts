// Pure rules for the app-update banner and the first-run prompt's choices.

import type { AppUpdate } from '../../api';

/**
 * The banner is shown while an update was found, app updates are on, and the user has not said
 * "Later" for it. "Later" hides one version for this session; a newer version found afterwards
 * shows again.
 */
export function bannerVisible(
  found: AppUpdate | null,
  enabled: boolean,
  hiddenVersion: string | null,
): boolean {
  return enabled && found !== null && found.version !== hiddenVersion;
}

/** Notes longer than this many lines (or characters) start collapsed behind "Show notes". */
export const NOTES_COLLAPSED_LINES = 3;
const NOTES_LONG_CHARS = 240;

/**
 * The release notes as the plain text to show: line endings unified, surrounding blank space
 * removed, runs of blank lines folded to one. Null or empty notes give null (nothing to show).
 */
export function notesText(notes: string | null): string | null {
  if (notes === null) return null;
  const text = notes
    .replace(/\r\n?/g, '\n')
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
  return text === '' ? null : text;
}

/** The collapsed notes: blank lines dropped, so the few lines shown are all text. */
export function notesPreview(text: string): string {
  return text.replace(/\n{2,}/g, '\n');
}

/** Whether the notes need the collapse and its "Show notes" toggle. */
export function notesAreLong(text: string | null): boolean {
  if (text === null) return false;
  return text.split('\n').length > NOTES_COLLAPSED_LINES || text.length > NOTES_LONG_CHARS;
}

/** What the install button is doing; `failed` offers a retry. */
export type InstallState = 'idle' | 'installing' | 'failed';

export function installButtonKey(state: InstallState): 'install' | 'installing' | 'retry' {
  return state === 'installing' ? 'installing' : state === 'failed' ? 'retry' : 'install';
}
