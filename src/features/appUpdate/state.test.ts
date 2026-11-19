import { describe, expect, it } from 'vitest';
import {
  bannerVisible,
  installButtonKey,
  notesAreLong,
  notesPreview,
  notesText,
  NOTES_COLLAPSED_LINES,
} from './state';

const update = { version: '0.9.0-beta.2', notes: null, date: null };

describe('bannerVisible', () => {
  it('shows a found update while app updates are on', () => {
    expect(bannerVisible(update, true, null)).toBe(true);
  });

  it('shows nothing when nothing was found or the switch is off', () => {
    expect(bannerVisible(null, true, null)).toBe(false);
    expect(bannerVisible(update, false, null)).toBe(false);
  });

  it('hides the version the user put off, but shows a newer one', () => {
    expect(bannerVisible(update, true, '0.9.0-beta.2')).toBe(false);
    expect(bannerVisible({ ...update, version: '0.9.0-beta.3' }, true, '0.9.0-beta.2')).toBe(true);
  });
});

describe('notesText', () => {
  it('has nothing to show for missing or blank notes', () => {
    expect(notesText(null)).toBeNull();
    expect(notesText('')).toBeNull();
    expect(notesText('  \n\r\n  ')).toBeNull();
  });

  it('unifies line endings, trims, and folds runs of blank lines', () => {
    expect(notesText('  First  \r\n\r\n\r\n\r\nSecond\r\n')).toBe('First\n\nSecond');
  });

  it('keeps the text as plain text, markup included', () => {
    expect(notesText('## New\n- fixed *things*')).toBe('## New\n- fixed *things*');
  });
});

describe('notesPreview', () => {
  it('drops the blank lines between paragraphs and nothing else', () => {
    expect(notesPreview('One\n\nTwo\nThree\n\n\nFour')).toBe('One\nTwo\nThree\nFour');
    expect(notesPreview('Single line')).toBe('Single line');
  });
});

describe('notesAreLong', () => {
  it('is false for nothing and for a short note', () => {
    expect(notesAreLong(null)).toBe(false);
    expect(notesAreLong('Fixes a crash.')).toBe(false);
  });

  it('is true past the collapsed line count, or for one very long line', () => {
    const lines = Array.from({ length: NOTES_COLLAPSED_LINES + 1 }, (_, i) => `Line ${i}`);
    expect(notesAreLong(lines.join('\n'))).toBe(true);
    expect(notesAreLong(lines.slice(0, NOTES_COLLAPSED_LINES).join('\n'))).toBe(false);
    expect(notesAreLong('x'.repeat(400))).toBe(true);
  });
});

describe('installButtonKey', () => {
  it('names the button for each state', () => {
    expect(installButtonKey('idle')).toBe('install');
    expect(installButtonKey('installing')).toBe('installing');
    expect(installButtonKey('failed')).toBe('retry');
  });
});
