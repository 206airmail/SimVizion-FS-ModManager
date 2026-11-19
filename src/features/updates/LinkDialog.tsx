import { CircleAlert, Link2 } from 'lucide-react';
import { useEffect, useId, useRef, useState } from 'react';
import type { ModEntry } from '../../api';
import { api } from '../../api/client';
import { describeError } from '../../lib/errors';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Dialog } from '../../ui/Dialog';
import { Icon } from '../../ui/Icon';
import { previewHost } from './choices';

const t = strings.updates.link;

export interface LinkDialogProps {
  mod: ModEntry;
  /** The host of the mod's current manual link, when it has one (the address itself stays in the core). */
  currentHost: string | null;
  onClose: () => void;
  /** The link was saved or removed: lists should be read again. */
  onChanged: () => void;
}

/**
 * Pastes, replaces or removes the mod's manual source link. The core accepts only http(s)
 * addresses and says `invalidLink` otherwise; the host is shown as the user types. The link is
 * kept per mod name, so every folder's copy gets it.
 */
export function LinkDialog({ mod, currentHost, onClose, onChanged }: LinkDialogProps) {
  const fieldId = useId();
  const messageId = useId();
  const [text, setText] = useState('');
  const [busy, setBusy] = useState<'save' | 'remove' | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const host = previewHost(text);
  const input = useRef<HTMLInputElement>(null);
  // A disabled field drops focus; when a save fails and the field is back, put the caret in it.
  useEffect(() => {
    if (busy === null) input.current?.focus();
  }, [busy]);

  const run = async (kind: 'save' | 'remove') => {
    if (busy !== null || (kind === 'save' && text.trim() === '')) return;
    setBusy(kind);
    setProblem(null);
    try {
      await api.setSourceLink(mod.modName, kind === 'save' ? text.trim() : null);
      onChanged();
      onClose();
    } catch (error) {
      setProblem(describeError(error));
      setBusy(null);
    }
  };

  return (
    <Dialog
      title={currentHost === null ? t.title : t.editTitle}
      size="small"
      busy={busy !== null}
      onClose={onClose}
      footer={
        <>
          {currentHost !== null && (
            <Button
              variant="destructive"
              disabled={busy !== null}
              onClick={() => void run('remove')}
            >
              {busy === 'remove' ? t.removing : t.remove}
            </Button>
          )}
          <span className="mm-dialog-gap" />
          <Button disabled={busy !== null} onClick={onClose}>
            {t.cancel}
          </Button>
          <Button
            variant="primary"
            disabled={busy !== null || text.trim() === ''}
            aria-busy={busy === 'save' || undefined}
            onClick={() => void run('save')}
          >
            {busy === 'save' ? t.saving : t.save}
          </Button>
        </>
      }
    >
      <p className="mm-link__lead">{t.lead}</p>
      <div className="mm-field-group">
        <label className="mm-field-label" htmlFor={fieldId}>
          {t.label}
        </label>
        <input
          ref={input}
          id={fieldId}
          className="mm-input sv-selectable"
          type="text"
          inputMode="url"
          value={text}
          placeholder={t.placeholder}
          spellCheck={false}
          autoComplete="off"
          disabled={busy !== null}
          aria-invalid={problem !== null || undefined}
          aria-describedby={messageId}
          data-autofocus
          onChange={(event) => {
            setText(event.target.value);
            setProblem(null);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              void run('save');
            }
          }}
        />
        <span id={messageId} className={problem !== null ? 'mm-field-error' : 'mm-field-hint'}>
          {problem !== null ? (
            <>
              <Icon icon={CircleAlert} size={16} className="sv-tone-error" />
              {problem}
            </>
          ) : host !== null ? (
            <span className="mm-link__host">
              <Icon icon={Link2} size={16} />
              <span className="sv-truncate">{t.host(host)}</span>
            </span>
          ) : currentHost !== null ? (
            t.current(currentHost)
          ) : (
            t.invalid
          )}
        </span>
      </div>
    </Dialog>
  );
}
