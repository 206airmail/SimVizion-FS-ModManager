import { Info } from 'lucide-react';
import type { ModEntry } from '../../api';
import { formatBytes } from '../../lib/format';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Dialog } from '../../ui/Dialog';
import { Icon } from '../../ui/Icon';

/** How many mod names the confirmation lists before "and N more". */
const NAMES_SHOWN = 5;

export interface RemoveDialogProps {
  mods: readonly ModEntry[];
  profileName: string;
  onClose: () => void;
  onConfirm: () => void;
}

const t = strings.fileOps.remove;

/**
 * Confirmation before mods go to the Recycle Bin. It states count and size, where they go, what
 * happens to a mod Windows cannot recycle, and (calmly) that a synced folder syncs the removal.
 * Focus starts on Cancel and nothing is removed without pressing the destructive button.
 */
export function RemoveDialog({ mods, profileName, onClose, onConfirm }: RemoveDialogProps) {
  const size = formatBytes(mods.reduce((sum, mod) => sum + mod.sizeBytes, 0));
  const shown = mods.slice(0, NAMES_SHOWN);
  return (
    <Dialog
      title={t.title(mods.length)}
      size="small"
      onClose={onClose}
      footer={
        <>
          <Button data-autofocus onClick={onClose}>
            {strings.fileOps.cancel}
          </Button>
          <Button variant="destructive" onClick={onConfirm}>
            {t.confirm(mods.length)}
          </Button>
        </>
      }
    >
      <p className="mm-confirm__lead">{t.lead(mods.length, size, profileName)}</p>
      <ul className="mm-confirm__names">
        {shown.map((mod) => (
          <li key={mod.id} className="sv-truncate">
            {mod.title}
          </li>
        ))}
        {mods.length > shown.length && (
          <li className="mm-confirm__more">{t.more(mods.length - shown.length)}</li>
        )}
      </ul>
      <p>{t.recycle}</p>
      <p>{t.leftInPlace}</p>
      <p className="mm-confirm__note">
        <Icon icon={Info} size={16} className="sv-tone-info" />
        {t.synced}
      </p>
    </Dialog>
  );
}
