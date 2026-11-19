import { Download } from 'lucide-react';
import { useCallback, useMemo, useState } from 'react';
import type { GameStatus, ModEntry, Profile, UpdatePlan } from '../../api';
import { api } from '../../api/client';
import { describeError } from '../../lib/errors';
import { useAsync } from '../../lib/useAsync';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Checkbox } from '../../ui/Checkbox';
import { Dialog } from '../../ui/Dialog';
import { Banner } from '../../ui/Feedback';
import { Pill } from '../../ui/Pill';
import { fileOpBlockedReason } from '../files/blocked';
import { ReasonButton } from '../files/ReasonButton';
import { defaultFolders, isCurrent } from './choices';
import { PageLinks } from './PageLinks';

const t = strings.updates.dialog;

export interface UpdateDialogProps {
  /** The mod the user is looking at: its folder is ticked, and its page buttons are the fallback. */
  mod: ModEntry;
  profiles: readonly Profile[];
  game: GameStatus | null;
  /** A file operation is already running in this window. */
  operationRunning: boolean;
  onClose: () => void;
  /** The folders to update (profile ids) and the names to word the result with. */
  onConfirm: (profileIds: string[], folderNames: ReadonlyMap<string, string>) => void;
}

/**
 * The Update dialog: asks the core what the update would do (no network), then lists the
 * folders that hold the mod, each with its version and a checkbox. Only the folder on screen
 * starts ticked. When the update cannot be installed, it says why and offers
 * the mod's page buttons instead.
 */
export function UpdateDialog({
  mod,
  profiles,
  game,
  operationRunning,
  onClose,
  onConfirm,
}: UpdateDialogProps) {
  const load = useCallback(() => api.planUpdate(mod.modName), [mod.modName]);
  const { state } = useAsync(load);

  const title = t.title(mod.title);
  const close = (
    <Button data-autofocus={state.status !== 'ready' || undefined} onClick={onClose}>
      {state.status === 'ready' ? t.cancel : strings.fileOps.cancel}
    </Button>
  );

  if (state.status !== 'ready') {
    const failed = state.status === 'error';
    return (
      <Dialog title={title} size="small" onClose={onClose} footer={close}>
        {failed ? (
          <>
            <Banner tone="warning" title={t.failedTitle}>
              {describeError(state.error)}
            </Banner>
            <PageLinks mod={mod} />
          </>
        ) : (
          <p role="status" className="mm-update__planning">
            {t.planning}
          </p>
        )}
      </Dialog>
    );
  }

  return (
    <PlanBody
      title={title}
      plan={state.data}
      mod={mod}
      profiles={profiles}
      blockedReason={fileOpBlockedReason(game, operationRunning)}
      onClose={onClose}
      onConfirm={onConfirm}
    />
  );
}

interface PlanBodyProps extends Pick<
  UpdateDialogProps,
  'mod' | 'profiles' | 'onClose' | 'onConfirm'
> {
  title: string;
  plan: UpdatePlan;
  blockedReason: string | null;
}

function PlanBody({
  title,
  plan,
  mod,
  profiles,
  blockedReason,
  onClose,
  onConfirm,
}: PlanBodyProps) {
  const [ticked, setTicked] = useState(() => new Set(defaultFolders(plan, mod.profileId)));
  const names = useMemo(
    () => new Map(profiles.map((profile) => [profile.id, profile.name])),
    [profiles],
  );
  const chosen = plan.folders.filter((folder) => ticked.has(folder.profileId));
  const reason = blockedReason ?? (chosen.length === 0 ? t.none : null);

  const toggle = (profileId: string, on: boolean) =>
    setTicked((current) => {
      const next = new Set(current);
      if (on) next.add(profileId);
      else next.delete(profileId);
      return next;
    });

  return (
    <Dialog
      title={title}
      size="small"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t.cancel}</Button>
          <ReasonButton
            variant="primary"
            icon={Download}
            hint={t.hint(plan.toVersion)}
            blockedReason={reason}
            onClick={() =>
              onConfirm(
                chosen.map((folder) => folder.profileId),
                names,
              )
            }
          >
            {t.confirm(chosen.length)}
          </ReasonButton>
        </>
      }
    >
      {blockedReason !== null && <Banner tone="warning" title={blockedReason} />}
      <div className="mm-update__versions">
        <span className="mm-update__version sv-num">{mod.version}</span>
        <span className="mm-update__arrow" aria-hidden="true">
          →
        </span>
        <span className="mm-update__version mm-update__version--new sv-num">{plan.toVersion}</span>
        <span className="mm-update__from">{t.from(plan.source)}</span>
      </div>

      <fieldset className="mm-update__folders">
        <legend className="mm-field-label">{t.foldersLabel}</legend>
        {plan.folders.map((folder) => {
          const name = names.get(folder.profileId) ?? folder.profileId;
          const current = isCurrent(folder, plan.toVersion);
          return (
            <Checkbox
              key={folder.profileId}
              className="mm-update__folder"
              checked={ticked.has(folder.profileId)}
              onChange={(on) => toggle(folder.profileId, on)}
            >
              <span className="mm-update__folder-name sv-truncate">{name}</span>
              {profiles.find((profile) => profile.id === folder.profileId)?.isActive && (
                <Pill size="compact" tone="success">
                  {t.active}
                </Pill>
              )}
              <span className="mm-update__folder-version sv-num">
                {current ? t.current : t.folderVersion(folder.version)}
              </span>
            </Checkbox>
          );
        })}
      </fieldset>
      {chosen.length === 0 && <p className="mm-field-hint">{t.none}</p>}
      <p className="mm-update__note">{t.note}</p>
    </Dialog>
  );
}
