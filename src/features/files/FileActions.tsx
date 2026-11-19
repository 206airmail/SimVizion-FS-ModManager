import { Copy, FolderInput, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { GameStatus, ModEntry, Profile, TransferMode } from '../../api';
import { strings } from '../../strings';
import { fileOpBlockedReason } from './blocked';
import { ReasonButton } from './ReasonButton';
import { RemoveDialog } from './RemoveDialog';
import { TransferDialog } from './TransferDialog';
import type { FileOpsController } from './useFileOps';

export interface FileActionsProps {
  /** The selected mods, all in one profile. */
  mods: readonly ModEntry[];
  profiles: readonly Profile[];
  game: GameStatus | null;
  fileOps: FileOpsController;
}

type Open =
  | { kind: 'transfer'; mode: TransferMode; mods: readonly ModEntry[] }
  | { kind: 'remove'; mods: readonly ModEntry[] };

const t = strings.inspector;

/**
 * Copy to, Move to and Remove for the selected mods, with the dialogs they open. They are
 * unavailable, with the reason, while the game or server runs or another operation is running.
 * Render it inside a footer: it adds the three buttons and a spacer before Remove.
 */
export function FileActions({ mods, profiles, game, fileOps }: FileActionsProps) {
  // The mods are fixed when a dialog opens: a rescan underneath must not change what it acts on.
  const [open, setOpen] = useState<Open | null>(null);
  const blocked = fileOpBlockedReason(game, fileOps.running !== null);
  const profileId = mods[0]?.profileId;
  const profileName = profiles.find((profile) => profile.id === profileId)?.name ?? '';
  const close = () => setOpen(null);

  return (
    <>
      <ReasonButton
        size="compact"
        icon={Copy}
        hint={t.copyHint}
        blockedReason={blocked}
        onClick={() => setOpen({ kind: 'transfer', mode: 'copy', mods })}
      >
        {t.copyTo}
      </ReasonButton>
      <ReasonButton
        size="compact"
        icon={FolderInput}
        hint={t.moveHint}
        blockedReason={blocked}
        onClick={() => setOpen({ kind: 'transfer', mode: 'move', mods })}
      >
        {t.moveTo}
      </ReasonButton>
      <span className="mm-inspector__footer-gap" />
      <ReasonButton
        size="compact"
        variant="destructive"
        icon={Trash2}
        hint={t.removeHint}
        blockedReason={blocked}
        onClick={() => setOpen({ kind: 'remove', mods })}
      >
        {t.remove}
      </ReasonButton>

      {open?.kind === 'transfer' && profileId !== undefined && (
        <TransferDialog
          mods={open.mods}
          fromProfileId={profileId}
          profiles={profiles}
          initialMode={open.mode}
          onClose={close}
          onConfirm={(plan, choices, targetName, replaced) => {
            close();
            fileOps.startTransfer(plan, choices, targetName, replaced);
          }}
        />
      )}
      {open?.kind === 'remove' && profileId !== undefined && (
        <RemoveDialog
          mods={open.mods}
          profileName={profileName}
          onClose={close}
          onConfirm={() => {
            close();
            fileOps.startRemove(
              profileId,
              open.mods.map((mod) => mod.id),
            );
          }}
        />
      )}
    </>
  );
}
