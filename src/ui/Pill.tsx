import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { cx } from './cx';
import { Icon } from './Icon';

export type Tone = 'neutral' | 'info' | 'warning' | 'error' | 'success';

export interface PillProps {
  /** Neutral for category tags; a semantic tone only when the pill reports a state. */
  tone?: Tone;
  icon?: LucideIcon;
  size?: 'standard' | 'compact';
  className?: string;
  children: ReactNode;
}

export function Pill({
  tone = 'neutral',
  icon,
  size = 'standard',
  className,
  children,
}: PillProps) {
  return (
    <span
      className={cx(
        'sv-pill',
        `sv-pill--${tone}`,
        size === 'compact' && 'sv-pill--compact',
        className,
      )}
    >
      {icon && <Icon icon={icon} size={14} className="sv-pill__icon" />}
      {children}
    </span>
  );
}
