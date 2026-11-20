import { Download } from 'lucide-react';
import { useState } from 'react';
import type { GameStatus, ModEntry, Profile } from '../../api';
import { strings } from '../../strings';
import { Banner } from '../../ui/Feedback';
import { ReasonButton } from '../files/ReasonButton';
import type { FileOpsController } from '../files/useFileOps';
import { updateBlockedReason } from './blocked';
import { PageLinks } from './PageLinks';
import { UpdateDialog } from './UpdateDialog';

const t = strings.inspector;

export interface UpdateBannerProps {
  mod: ModEntry;
  profiles: readonly Profile[];
  game: GameStatus | null;
  fileOps: FileOpsController;
  /** A check for mod updates is running: Update waits for it. */
  updateChecking: boolean;
}

/**
 * The Inspector's update notice. When the app can install the update, an Update button opens the
 * dialog; otherwise one line says why and the mod's page buttons take its place. Renders nothing
 * for a mod with no update.
 */
export function UpdateBanner({ mod, profiles, game, fileOps, updateChecking }: UpdateBannerProps) {
  const [open, setOpen] = useState(false);
  if (mod.update.type !== 'available') return null;
  const { version, installable } = mod.update;

  return (
    <>
      <Banner
        tone="info"
        title={t.updateTitle(version)}
        actions={
          installable ? (
            <ReasonButton
              size="compact"
              variant="primary"
              icon={Download}
              hint={t.updateHint(version)}
              blockedReason={updateBlockedReason(game, fileOps.running !== null, updateChecking)}
              onClick={() => setOpen(true)}
            >
              {t.updateAction}
            </ReasonButton>
          ) : undefined
        }
      >
        {installable ? (
          t.updateText(mod.version)
        ) : (
          <>
            <span className="mm-update__reason">
              {mod.links.length > 0 ? t.updateNotInstallable : t.updateNotInstallableNoPage}
            </span>
            <PageLinks mod={mod} />
          </>
        )}
      </Banner>
      {open && (
        <UpdateDialog
          mod={mod}
          profiles={profiles}
          game={game}
          operationRunning={fileOps.running !== null}
          updateChecking={updateChecking}
          onClose={() => setOpen(false)}
          onConfirm={(profileIds, names) => {
            setOpen(false);
            fileOps.startUpdate([{ modName: mod.modName, profileIds }], names);
          }}
        />
      )}
    </>
  );
}
