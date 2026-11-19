import { Check } from 'lucide-react';
import type { InputHTMLAttributes, ReactNode } from 'react';
import { cx } from './cx';
import { Icon } from './Icon';

export interface CheckboxProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  'type' | 'onChange'
> {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** The label beside the box; it is part of the click target. */
  children: ReactNode;
}

/**
 * A native checkbox drawn as the design system's 18px lime box (controls.md): the input keeps
 * focus, Space and the accessible state; the box is its picture.
 */
export function Checkbox({ checked, onChange, children, className, ...rest }: CheckboxProps) {
  return (
    <label className={cx('sv-checkbox', className)}>
      <input
        {...rest}
        type="checkbox"
        className="sv-checkbox__input"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="sv-checkbox__box" aria-hidden="true">
        {checked && <Icon icon={Check} size={14} />}
      </span>
      <span className="sv-checkbox__label">{children}</span>
    </label>
  );
}
