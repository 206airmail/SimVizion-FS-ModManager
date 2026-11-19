import { useId } from 'react';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Tooltip } from '../../ui/Tooltip';

export interface SetActiveButtonProps {
  /** Why switching is unavailable right now (the game or server runs), or null. */
  blockedReason: string | null;
  /** A write is in flight: this switch (shows the busy label) or an undo. */
  busy: 'switch' | 'undo' | null;
  label: string;
  onPress: () => void;
}

/**
 * The primary "Set as active" action. While the game or server runs it stays focusable but
 * `aria-disabled`, so keyboard and screen-reader users can reach the reason, which is both a
 * tooltip and a hidden description. While a write is in flight it is truly disabled.
 */
export function SetActiveButton({ blockedReason, busy, label, onPress }: SetActiveButtonProps) {
  const reasonId = useId();
  const button = (
    <Button
      variant="primary"
      className="mm-set-active"
      disabled={busy !== null}
      aria-busy={busy === 'switch' || undefined}
      aria-disabled={blockedReason !== null || undefined}
      aria-describedby={blockedReason !== null ? reasonId : undefined}
      onClick={() => {
        if (blockedReason === null) onPress();
      }}
    >
      {busy === 'switch' ? strings.switching.busy : label}
    </Button>
  );
  if (blockedReason === null) return button;
  return (
    <>
      <Tooltip label={blockedReason}>{button}</Tooltip>
      <span id={reasonId} className="sv-visually-hidden">
        {blockedReason}
      </span>
    </>
  );
}
