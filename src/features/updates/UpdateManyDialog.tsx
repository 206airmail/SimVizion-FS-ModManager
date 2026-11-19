import { Download } from 'lucide-react';
import type { GameStatus, ModEntry } from '../../api';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Dialog } from '../../ui/Dialog';
import { Banner } from '../../ui/Feedback';
import { fileOpBlockedReason } from '../files/blocked';
import { ReasonButton } from '../files/ReasonButton';

/** How many mods the confirmation lists before "and N more". */
const SHOWN = 6;

const t = strings.updates.many;

export interface UpdateManyDialogProps {
  /** Mods with an installable update, all in one folder. */
  mods: readonly ModEntry[];
  game: GameStatus | null;
  operationRunning: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

/**
 * The confirmation for "Update N" on a selection: every listed mod is updated in the folder
 * it is shown in, one after another. It does not ask about other folders; a mod opened on its
 * own offers those.
 */
export function UpdateManyDialog({
  mods,
  game,
  operationRunning,
  onClose,
  onConfirm,
}: UpdateManyDialogProps) {
  const blocked = fileOpBlockedReason(game, operationRunning);
  const shown = mods.slice(0, SHOWN);
  return (
    <Dialog
      title={t.title(mods.length)}
      size="small"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{strings.updates.dialog.cancel}</Button>
          <ReasonButton
            variant="primary"
            icon={Download}
            hint={strings.inspector.updateManyHint}
            blockedReason={blocked}
            onClick={onConfirm}
          >
            {t.confirm(mods.length)}
          </ReasonButton>
        </>
      }
    >
      {blocked !== null && <Banner tone="warning" title={blocked} />}
      <p className="mm-confirm__lead">{t.lead}</p>
      <ul className="mm-update__list">
        {shown.map((mod) => (
          <li key={mod.id} className="mm-update__item">
            <span className="sv-truncate">{mod.title}</span>
            <span className="mm-update__item-versions sv-num">
              {mod.update.type === 'available' ? t.to(mod.version, mod.update.version) : ''}
            </span>
          </li>
        ))}
        {mods.length > shown.length && (
          <li className="mm-confirm__more">{t.more(mods.length - shown.length)}</li>
        )}
      </ul>
      <p>{t.note}</p>
    </Dialog>
  );
}
