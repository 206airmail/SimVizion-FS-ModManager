import { useEffect, useState } from 'react';
import type { ClashChoice, Profile, TransferMode, TransferPlan } from '../../api';
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

/**
 * What the dialog starts from. Library actions give the mods and the profile they are in; the
 * dialog asks for the target and the action. Compare and Savegames also give the target (and
 * usually the action), so the dialog skips that step and goes straight to the plan.
 */
export interface TransferPreset {
  fromProfileId: string;
  /** Given: the target step is skipped and the plan is made on open. */
  toProfileId?: string;
  /** The starting action (copy when absent); fixed when the target is given too. */
  mode?: TransferMode;
  /** The mods to send, all from the profile `fromProfileId`. */
  modIds: readonly string[];
}

export interface TransferDialogProps {
  preset: TransferPreset;
  /** Every profile; the one the mods are in is left out of the choice. */
  profiles: readonly Profile[];
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
export function TransferDialog({ preset, profiles, onClose, onConfirm }: TransferDialogProps) {
  // Read once: what the dialog acts on is fixed when it opens, whatever the caller does after.
  const [{ fromProfileId, toProfileId, modIds, mode: startMode }] = useState(preset);
  const targets = profiles.filter((profile) => profile.id !== fromProfileId);
  const [mode, setMode] = useState<TransferMode>(startMode ?? 'copy');
  const [targetId, setTargetId] = useState<string | null>(
    toProfileId ?? (targets.length === 1 ? (targets[0]?.id ?? null) : null),
  );
  const [plan, setPlan] = useState<TransferPlan | null>(null);
  const [decisions, setDecisions] = useState<ClashDecisions>(new Map());
  // With the target given the plan is made on open, so the dialog starts out busy.
  const [checking, setChecking] = useState(toProfileId !== undefined);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  // The preset path: plan on open, and again when "Try again" is pressed.
  const presetMode = startMode ?? 'copy';
  useEffect(() => {
    if (toProfileId === undefined) return;
    let current = true;
    api.planTransfer(fromProfileId, [...modIds], toProfileId, presetMode).then(
      (made) => {
        if (!current) return;
        setDecisions(defaultDecisions(made.clashes));
        setPlan(made);
        setChecking(false);
      },
      (failure: unknown) => {
        if (!current) return;
        setError(describeError(failure));
        setChecking(false);
      },
    );
    return () => {
      current = false;
    };
  }, [fromProfileId, toProfileId, modIds, presetMode, attempt]);

  const check = async () => {
    if (targetId === null || checking) return;
    setChecking(true);
    setError(null);
    try {
      const made = await api.planTransfer(fromProfileId, [...modIds], targetId, mode);
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

  const targetNameOf = (id: string | null) => profiles.find((p) => p.id === id)?.name ?? '';

  if (plan === null && toProfileId !== undefined) {
    return (
      <Dialog
        title={t.titleTo(presetMode, modIds.length, targetNameOf(toProfileId))}
        size="medium"
        busy={checking}
        focusKey="planning"
        onClose={onClose}
        footer={
          <>
            {cancel}
            {error !== null && (
              <Button
                variant="primary"
                onClick={() => {
                  setError(null);
                  setChecking(true);
                  setAttempt((count) => count + 1);
                }}
              >
                {strings.states.retry}
              </Button>
            )}
          </>
        }
      >
        {error !== null ? (
          <Banner tone="error" title={strings.actionFailed.planTransfer}>
            {error}
          </Banner>
        ) : (
          <p role="status" aria-busy="true">
            {t.planning}
          </p>
        )}
      </Dialog>
    );
  }

  if (plan === null) {
    return (
      <Dialog
        title={t.title(mode, modIds.length)}
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

  const targetName = targetNameOf(plan.toProfileId);
  const count = transferCount(plan, decisions);
  const needsSpace = lacksSpace(plan);
  const itemsSize = plan.items.reduce((sum, item) => sum + item.sizeBytes, 0);

  return (
    <Dialog
      title={
        toProfileId === undefined
          ? t.title(plan.mode, plan.items.length)
          : t.titleTo(plan.mode, plan.items.length, targetName)
      }
      size="large"
      focusKey="review"
      onClose={onClose}
      footer={
        <>
          {toProfileId === undefined && (
            <Button
              onClick={() => {
                setPlan(null);
                setError(null);
              }}
            >
              {t.back}
            </Button>
          )}
          {/* The first control in the body is "Replace all": Enter must not start a replacement. */}
          <Button data-autofocus onClick={onClose} disabled={checking}>
            {strings.fileOps.cancel}
          </Button>
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
