import { useState } from 'react';
import type { ClashChoice, ModEntry, Profile, TransferMode, TransferPlan } from '../../api';
import { api } from '../../api/client';
import { describeError } from '../../lib/errors';
import { formatBytes } from '../../lib/format';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Dialog } from '../../ui/Dialog';
import { Banner } from '../../ui/Feedback';
import { SegmentedControl } from '../../ui/SegmentedControl';
import {
  defaultDecisions,
  isSameDriveMove,
  lacksSpace,
  replacedNames,
  toChoices,
  transferCount,
  type ClashDecisions,
} from './choices';
import { ClashTable } from './ClashTable';
import { TargetList } from './TargetList';

export interface TransferDialogProps {
  /** The mods to send, all from the profile `fromProfileId`. */
  mods: readonly ModEntry[];
  fromProfileId: string;
  /** Every profile; the one the mods are in is left out of the choice. */
  profiles: readonly Profile[];
  initialMode: TransferMode;
  onClose: () => void;
  /** The user confirmed the plan: carry it out. The dialog is closed by the caller. */
  onConfirm: (
    plan: TransferPlan,
    choices: ClashChoice[],
    targetName: string,
    replaced: string[],
  ) => void;
}

const t = strings.fileOps.transfer;

/**
 * Copy or Move in two steps inside one dialog. First the target and the action. Then the plan the
 * core worked out (`planTransfer` writes nothing): items, size, free space and any clashes to
 * decide. Confirming hands over to the progress banner.
 */
export function TransferDialog({
  mods,
  fromProfileId,
  profiles,
  initialMode,
  onClose,
  onConfirm,
}: TransferDialogProps) {
  const targets = profiles.filter((profile) => profile.id !== fromProfileId);
  const [mode, setMode] = useState<TransferMode>(initialMode);
  const [targetId, setTargetId] = useState<string | null>(
    targets.length === 1 ? (targets[0]?.id ?? null) : null,
  );
  const [plan, setPlan] = useState<TransferPlan | null>(null);
  const [decisions, setDecisions] = useState<ClashDecisions>(new Map());
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const check = async () => {
    if (targetId === null || checking) return;
    setChecking(true);
    setError(null);
    try {
      const made = await api.planTransfer(
        fromProfileId,
        mods.map((mod) => mod.id),
        targetId,
        mode,
      );
      setDecisions(defaultDecisions(made.clashes));
      setPlan(made);
    } catch (failure) {
      setError(describeError(failure));
    } finally {
      setChecking(false);
    }
  };

  const cancel = (
    <Button onClick={onClose} disabled={checking}>
      {strings.fileOps.cancel}
    </Button>
  );

  if (plan === null) {
    return (
      <Dialog
        title={t.title(mode, mods.length)}
        size="medium"
        busy={checking}
        focusKey="choose"
        onClose={onClose}
        footer={
          <>
            {cancel}
            <Button
              variant="primary"
              disabled={targetId === null || checking}
              aria-busy={checking || undefined}
              onClick={() => void check()}
            >
              {checking ? t.checking : t.next}
            </Button>
          </>
        }
      >
        {error !== null && (
          <Banner tone="error" title={strings.actionFailed.planTransfer}>
            {error}
          </Banner>
        )}
        <div className="mm-field-group">
          <span className="mm-field-label">{t.modeLabel}</span>
          <SegmentedControl<TransferMode>
            label={t.modeLabel}
            value={mode}
            onChange={setMode}
            segments={[
              { value: 'copy', label: t.modeCopy },
              { value: 'move', label: t.modeMove },
            ]}
          />
          <span className="mm-field-hint">{mode === 'copy' ? t.modeCopyHint : t.modeMoveHint}</span>
        </div>
        <div className="mm-field-group">
          <span className="mm-field-label">{t.targetLabel}</span>
          {targets.length === 0 ? (
            <p>{t.targetEmpty}</p>
          ) : (
            <TargetList
              label={t.targetLabel}
              profiles={targets}
              value={targetId}
              onChange={setTargetId}
            />
          )}
        </div>
      </Dialog>
    );
  }

  const targetName = profiles.find((profile) => profile.id === plan.toProfileId)?.name ?? '';
  const count = transferCount(plan, decisions);
  const needsSpace = lacksSpace(plan);
  const itemsSize = plan.items.reduce((sum, item) => sum + item.sizeBytes, 0);

  return (
    <Dialog
      title={t.title(plan.mode, plan.items.length)}
      size="large"
      focusKey="review"
      onClose={onClose}
      footer={
        <>
          <Button
            onClick={() => {
              setPlan(null);
              setError(null);
            }}
          >
            {t.back}
          </Button>
          {cancel}
          <Button
            variant="primary"
            disabled={needsSpace || count === 0}
            onClick={() =>
              onConfirm(
                plan,
                toChoices(plan.clashes, decisions),
                targetName,
                replacedNames(plan.clashes, decisions),
              )
            }
          >
            {t.confirm(plan.mode, count)}
          </Button>
        </>
      }
    >
      <dl className="mm-plan">
        <div className="mm-plan__row">
          <dt>{t.targetLabel}</dt>
          <dd>{targetName}</dd>
        </div>
        <div className="mm-plan__row">
          <dt>{t.planMods}</dt>
          <dd className="sv-num">{plan.items.length}</dd>
        </div>
        <div className="mm-plan__row">
          <dt>{t.planSize}</dt>
          <dd className="sv-num">{formatBytes(itemsSize)}</dd>
        </div>
        <div className="mm-plan__row">
          <dt>{t.planFree}</dt>
          {isSameDriveMove(plan) ? (
            <dd>{t.sameDrive}</dd>
          ) : (
            <dd className="sv-num">{formatBytes(plan.freeBytes)}</dd>
          )}
        </div>
      </dl>

      {needsSpace && (
        <Banner tone="error" title={t.noSpaceTitle}>
          {t.noSpaceText(formatBytes(plan.totalBytes), formatBytes(plan.freeBytes))}
        </Banner>
      )}

      {plan.clashes.length > 0 && (
        <section className="mm-clash-section">
          <h3 className="mm-clash-section__title">
            {t.clashTitle(plan.clashes.length, targetName)}
          </h3>
          <p>{t.clashText}</p>
          <ClashTable clashes={plan.clashes} decisions={decisions} onChange={setDecisions} />
        </section>
      )}

      {!needsSpace && (
        <p className="mm-plan__note" role="status">
          {t.confirmNote(count, plan.items.length - count)}
        </p>
      )}
    </Dialog>
  );
}
