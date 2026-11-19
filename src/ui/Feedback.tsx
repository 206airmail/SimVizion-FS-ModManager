import {
  CircleAlert,
  CircleCheck,
  CircleX,
  Info,
  TriangleAlert,
  type LucideIcon,
} from 'lucide-react';
import type { ReactNode } from 'react';
import { cx } from './cx';
import { Icon } from './Icon';
import type { Tone } from './Pill';

const toneIcons: Record<Exclude<Tone, 'neutral'>, LucideIcon> = {
  info: Info,
  warning: TriangleAlert,
  error: CircleX,
  success: CircleCheck,
};

export interface BannerProps {
  tone: Exclude<Tone, 'neutral'>;
  title: string;
  children?: ReactNode;
  /** Buttons shown at the right. */
  actions?: ReactNode;
  className?: string;
}

/** Persistent in-page message. Long text stays near-white; the icon carries the severity. */
export function Banner({ tone, title, children, actions, className }: BannerProps) {
  return (
    <div
      className={cx('sv-banner', `sv-banner--${tone}`, className)}
      role={tone === 'error' ? 'alert' : 'status'}
    >
      <Icon icon={toneIcons[tone]} size={20} className="sv-banner__icon" />
      <div className="sv-banner__body">
        <div className="sv-banner__title">{title}</div>
        {children && <div className="sv-banner__text">{children}</div>}
      </div>
      {actions && <div className="sv-banner__actions">{actions}</div>}
    </div>
  );
}

export interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  description?: string;
  /** Use for a failed load: the icon turns red and the region is announced. */
  tone?: 'neutral' | 'error';
  action?: ReactNode;
}

export function EmptyState({
  icon,
  title,
  description,
  tone = 'neutral',
  action,
}: EmptyStateProps) {
  return (
    <div
      className={cx('sv-empty', tone === 'error' && 'sv-empty--error')}
      role={tone === 'error' ? 'alert' : undefined}
    >
      <Icon icon={tone === 'error' ? CircleAlert : icon} size={32} className="sv-empty__icon" />
      <h2 className="sv-empty__title">{title}</h2>
      {description && <p className="sv-empty__text">{description}</p>}
      {action && <div className="sv-empty__action">{action}</div>}
    </div>
  );
}
