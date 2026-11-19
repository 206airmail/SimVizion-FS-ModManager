import { CircleAlert } from 'lucide-react';
import { useId, useState } from 'react';
import type { Profile } from '../../api';
import { describeError } from '../../lib/errors';
import { PROFILE_NAME_MAX, validateProfileName } from '../../lib/profileName';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Dialog } from '../../ui/Dialog';
import { Banner } from '../../ui/Feedback';
import { Icon } from '../../ui/Icon';

const t = strings.profileMenu;

export interface ProfileDialogProps {
  profile: Profile;
  onClose: () => void;
}

/**
 * Rename: a text field with its validation (not empty, up to 100 characters). The name is the
 * label in the list only; the folder on disk keeps its name. A rejected call is shown in the
 * dialog, which stays open so the name can be corrected.
 */
export function RenameProfileDialog({
  profile,
  onClose,
  onRename,
}: ProfileDialogProps & { onRename: (profile: Profile, name: string) => Promise<void> }) {
  const [name, setName] = useState(profile.name);
  const [attempted, setAttempted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const fieldId = useId();
  const messageId = useId();

  const problem = validateProfileName(name);
  // An empty name is only reported once the user tries to confirm; a too-long one at once.
  const shown = problem === 'tooLong' || (problem === 'empty' && attempted) ? problem : null;

  const submit = async () => {
    if (busy) return;
    setAttempted(true);
    if (problem !== null) return;
    const trimmed = name.trim();
    if (trimmed === profile.name) return onClose();
    setBusy(true);
    setFailure(null);
    try {
      await onRename(profile, trimmed);
      onClose();
    } catch (error) {
      setFailure(describeError(error));
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t.renameTitle}
      size="small"
      busy={busy}
      onClose={onClose}
      footer={
        <>
          <Button disabled={busy} onClick={onClose}>
            {t.cancel}
          </Button>
          <Button
            variant="primary"
            disabled={busy}
            aria-busy={busy || undefined}
            onClick={() => void submit()}
          >
            {busy ? t.renaming : t.renameAction}
          </Button>
        </>
      }
    >
      {failure !== null && (
        <Banner tone="error" title={strings.actionFailed.renameProfile}>
          {failure}
        </Banner>
      )}
      <div className="mm-field-group">
        <label className="mm-field-label" htmlFor={fieldId}>
          {t.renameLabel}
        </label>
        <input
          id={fieldId}
          className="mm-input sv-selectable"
          type="text"
          value={name}
          spellCheck={false}
          autoComplete="off"
          disabled={busy}
          aria-invalid={shown !== null || undefined}
          aria-describedby={messageId}
          data-autofocus
          onFocus={(event) => event.currentTarget.select()}
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              void submit();
            }
          }}
        />
        <span id={messageId} className={shown !== null ? 'mm-field-error' : 'mm-field-hint'}>
          {shown !== null ? (
            <>
              <Icon icon={CircleAlert} size={14} className="sv-tone-error" />
              {shown === 'empty' ? t.renameEmpty : t.renameTooLong(PROFILE_NAME_MAX)}
            </>
          ) : (
            t.renameHelp
          )}
        </span>
      </div>
    </Dialog>
  );
}

/** Remove from list: says plainly that the folder and its mods are not touched. */
export function RemoveProfileDialog({
  profile,
  onClose,
  onRemove,
}: ProfileDialogProps & { onRemove: (profile: Profile) => Promise<void> }) {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  const submit = async () => {
    if (busy) return;
    setBusy(true);
    setFailure(null);
    try {
      await onRemove(profile);
      onClose();
    } catch (error) {
      setFailure(describeError(error));
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t.removeTitle(profile.name)}
      size="small"
      busy={busy}
      onClose={onClose}
      footer={
        <>
          <Button data-autofocus disabled={busy} onClick={onClose}>
            {t.cancel}
          </Button>
          <Button
            variant="primary"
            disabled={busy}
            aria-busy={busy || undefined}
            onClick={() => void submit()}
          >
            {busy ? t.removing : t.removeAction}
          </Button>
        </>
      }
    >
      {failure !== null && (
        <Banner tone="error" title={strings.actionFailed.removeProfile}>
          {failure}
        </Banner>
      )}
      <p>{t.removeText}</p>
    </Dialog>
  );
}
