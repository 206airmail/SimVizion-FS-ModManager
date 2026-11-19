import type { LucideIcon } from 'lucide-react';
import type { ButtonHTMLAttributes } from 'react';
import { cx } from './cx';
import { Icon } from './Icon';
import { Tooltip } from './Tooltip';

type NativeButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'type'>;

export interface ButtonProps extends NativeButtonProps {
  variant?: 'secondary' | 'primary' | 'destructive';
  size?: 'standard' | 'compact';
  icon?: LucideIcon;
  /** Static lime halo for the one primary action in a functional context. */
  illuminated?: boolean;
}

export function Button({
  variant = 'secondary',
  size = 'standard',
  icon,
  illuminated = false,
  className,
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      type="button"
      className={cx(
        'sv-button',
        `sv-button--${variant}`,
        size === 'compact' && 'sv-button--compact',
        illuminated && 'sv-button--illuminated',
        className,
      )}
      {...rest}
    >
      {icon && <Icon icon={icon} size={size === 'compact' ? 14 : 16} />}
      {children}
    </button>
  );
}

export interface IconButtonProps extends Omit<NativeButtonProps, 'children' | 'aria-label'> {
  icon: LucideIcon;
  /** Accessible name, also shown as the tooltip. */
  label: string;
  size?: 'standard' | 'compact';
  /** Set for toggle buttons. */
  pressed?: boolean;
}

export function IconButton({
  icon,
  label,
  size = 'compact',
  pressed,
  className,
  ...rest
}: IconButtonProps) {
  return (
    <Tooltip label={label}>
      <button
        type="button"
        aria-label={label}
        aria-pressed={pressed}
        className={cx(
          'sv-button',
          'sv-button--secondary',
          'sv-button--icon',
          size === 'compact' && 'sv-button--compact',
          className,
        )}
        {...rest}
      >
        <Icon icon={icon} size={size === 'compact' ? 14 : 16} />
      </button>
    </Tooltip>
  );
}
