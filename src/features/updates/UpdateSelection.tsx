import { Download } from 'lucide-react';
import { useState } from 'react';
import type { GameStatus, ModEntry, Profile } from '../../api';
import { strings } from '../../strings';
import { fileOpBlockedReason } from '../files/blocked';
import { ReasonButton } from '../files/ReasonButton';
import type { FileOpsController } from '../files/useFileOps';
import { installableUpdates } from './choices';
import { UpdateManyDialog } from './UpdateManyDialog';

export interface UpdateSelectionProps {
  /** The selected mods, all in one folder. */
  mods: readonly ModEntry[];
  profiles: readonly Profile[];
  game: GameStatus | null;
  fileOps: FileOpsController;
}

/**
 * "Update N" for a selection: shown when some selected mods have an update the app can install.
 * One confirmation, then each mod is updated in its own folder, one after another, through the
 * same progress and result banners as a single update. Other folders' copies are not touched.
 */
export function UpdateSelection({ mods, profiles, game, fileOps }: UpdateSelectionProps) {
  // The list is fixed when the dialog opens: a reload underneath must not change what it does.
  const [open, setOpen] = useState<readonly ModEntry[] | null>(null);
  const updatable = installableUpdates(mods);
  if (updatable.length === 0) return null;

  const folderNames = new Map(profiles.map((profile) => [profile.id, profile.name]));
  return (
    <div className="mm-update-selection">
      <p className="mm-update-selection__text">
        {strings.inspector.multiUpdatesText(updatable.length, mods.length)}
      </p>
      <ReasonButton
        size="compact"
        variant="primary"
        icon={Download}
        hint={strings.inspector.updateManyHint}
        blockedReason={fileOpBlockedReason(game, fileOps.running !== null)}
        onClick={() => setOpen(updatable)}
      >
        {strings.inspector.updateMany(updatable.length)}
      </ReasonButton>
      {open !== null && (
        <UpdateManyDialog
          mods={open}
          game={game}
          operationRunning={fileOps.running !== null}
          onClose={() => setOpen(null)}
          onConfirm={() => {
            setOpen(null);
            fileOps.startUpdate(
              open.map((mod) => ({ modName: mod.modName, profileIds: [mod.profileId] })),
              folderNames,
            );
          }}
        />
      )}
    </div>
  );
}
