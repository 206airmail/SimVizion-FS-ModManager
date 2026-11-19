import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { cx } from './cx';

export interface SwitchProps extends Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  'type' | 'role' | 'onChange'
> {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** The label beside the track; it is part of the click target. */
  children: ReactNode;
}

/**
 * An on/off setting that takes effect at once (controls.md: 32x18 track, lime when on). A
 * `role="switch"` button, so Space and Enter toggle it and a screen reader says "on" or "off".
 */
export function Switch({ checked, onChange, children, className, ...rest }: SwitchProps) {
  return (
    <button
      {...rest}
      type="button"
      role="switch"
      aria-checked={checked}
      className={cx('sv-switch', className)}
      onClick={() => onChange(!checked)}
    >
      <span className="sv-switch__track" aria-hidden="true">
        <span className="sv-switch__thumb" />
      </span>
      <span className="sv-switch__label">{children}</span>
    </button>
  );
}
