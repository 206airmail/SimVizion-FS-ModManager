import { StickyNote, Tag } from 'lucide-react';
import type { ModEntry } from '../../api';
import { formatCount } from '../../lib/format';
import { strings } from '../../strings';
import { Icon } from '../../ui/Icon';

/**
 * A quiet sign that a mod has tags (a count) and a note, with both in its tooltip. Muted on
 * purpose: tags are the user's own sorting aid, not something that needs attention.
 */
export function TagMark({ mod }: { mod: Pick<ModEntry, 'tags' | 'note'> }) {
  const hasNote = mod.note !== null && mod.note !== '';
  if (mod.tags.length === 0 && !hasNote) return null;
  const label = strings.library.tagMarks(mod.tags, hasNote);
  return (
    <span className="mm-tagmark" title={label} aria-label={label} role="img">
      {mod.tags.length > 0 && (
        <span className="mm-tagmark__item">
          <Icon icon={Tag} size={14} />
          <span className="sv-num">{formatCount(mod.tags.length)}</span>
        </span>
      )}
      {hasNote && <Icon icon={StickyNote} size={14} />}
    </span>
  );
}
