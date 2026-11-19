import { useId } from 'react';
import { Button, type ButtonProps } from '../../ui/Button';
import { cx } from '../../ui/cx';
import { Tooltip } from '../../ui/Tooltip';

export interface ReasonButtonProps extends Omit<ButtonProps, 'disabled'> {
  /** Why the action is unavailable right now, or null when it is available. */
  blockedReason: string | null;
  /** What the action does, shown as the tooltip while it is available. */
  hint: string;
}

/**
 * A button that stays focusable while it is unavailable, so keyboard and screen-reader users can
 * reach the reason: it is both the tooltip and a hidden description, and pressing does nothing.
 * (The same pattern as Set as active.) The tooltip is always there, saying what the action does
 * or why it cannot be used, so the button keeps its place in the tree when its state changes and
 * a dialog it opened can hand focus back to it.
 */
export function ReasonButton({
  blockedReason,
  hint,
  className,
  onClick,
  ...rest
}: ReasonButtonProps) {
  const reasonId = useId();
  return (
    <>
      <Tooltip label={blockedReason ?? hint}>
        <Button
          {...rest}
          className={cx('mm-reason', className)}
          aria-disabled={blockedReason !== null || undefined}
          aria-describedby={blockedReason !== null ? reasonId : undefined}
          onClick={(event) => {
            if (blockedReason === null) onClick?.(event);
          }}
        />
      </Tooltip>
      {blockedReason !== null && (
        <span id={reasonId} className="sv-visually-hidden">
          {blockedReason}
        </span>
      )}
    </>
  );
}
