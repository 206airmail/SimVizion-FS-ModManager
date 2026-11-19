import { Check } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { ModEntry } from '../../api';
import { strings } from '../../strings';
import { Icon } from '../../ui/Icon';
import type { ModMetaController } from './useModMeta';
import { MAX_NOTE_LENGTH } from './tags';

const t = strings.tags;
const SAVED_MS = 2500;

type Status = 'idle' | 'saving' | 'saved';

/**
 * The note of one mod. It saves when the field loses focus and on Ctrl+Enter, and says "Saved"
 * for a moment. The text stays in the field if saving fails (the failure is shown by the
 * inspector), so the user can try again. Mount it with a key per mod.
 */
export function NoteField({ mod, meta }: { mod: ModEntry; meta: ModMetaController }) {
  const [draft, setDraft] = useState(mod.note ?? '');
  const [status, setStatus] = useState<Status>('idle');
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const save = async () => {
    if (draft === (mod.note ?? '')) return;
    window.clearTimeout(timer.current);
    setStatus('saving');
    const saved = await meta.setNote(mod, draft);
    setStatus(saved ? 'saved' : 'idle');
    if (saved) timer.current = window.setTimeout(() => setStatus('idle'), SAVED_MS);
  };

  return (
    <div className="mm-note">
      <textarea
        className="mm-input mm-note__input"
        aria-label={t.noteLabel}
        placeholder={t.notePlaceholder}
        maxLength={MAX_NOTE_LENGTH}
        rows={4}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => void save()}
        onKeyDown={(event) => {
          if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
            event.preventDefault();
            void save();
          }
        }}
      />
      <div className="mm-note__status" role="status">
        {status === 'saving' ? (
          t.noteSaving
        ) : status === 'saved' ? (
          <span className="mm-note__saved">
            <Icon icon={Check} size={14} className="sv-tone-success" />
            {t.noteSaved}
          </span>
        ) : (
          <span className="mm-note__hint">{t.noteHint}</span>
        )}
      </div>
    </div>
  );
}
