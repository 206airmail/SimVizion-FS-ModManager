import type { LucideIcon } from 'lucide-react';
import type { KeyboardEvent } from 'react';
import { cx } from './cx';
import { Icon } from './Icon';
import { Tooltip } from './Tooltip';

export interface Segment<T extends string> {
  value: T;
  label: string;
  /** When set, the segment shows only the icon and the label becomes its name and tooltip. */
  icon?: LucideIcon;
  count?: number;
}

export interface SegmentedControlProps<T extends string> {
  value: T;
  segments: ReadonlyArray<Segment<T>>;
  onChange: (value: T) => void;
  /** Accessible name of the group. */
  label: string;
  className?: string;
}

/** One-of-many view choice. Selection is teal: lime is reserved for on/off and actions. */
export function SegmentedControl<T extends string>({
  value,
  segments,
  onChange,
  label,
  className,
}: SegmentedControlProps<T>) {
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
    if (step === 0) return;
    event.preventDefault();
    const current = segments.findIndex((segment) => segment.value === value);
    const next = segments[(current + step + segments.length) % segments.length];
    if (!next) return;
    onChange(next.value);
    const buttons = event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="radio"]');
    buttons[segments.indexOf(next)]?.focus();
  };

  return (
    <div
      className={cx('sv-segmented', className)}
      role="radiogroup"
      aria-label={label}
      onKeyDown={onKeyDown}
    >
      {segments.map((segment) => {
        const checked = segment.value === value;
        const button = (
          <button
            key={segment.value}
            type="button"
            role="radio"
            aria-checked={checked}
            aria-label={segment.icon ? segment.label : undefined}
            tabIndex={checked ? 0 : -1}
            className={cx('sv-segmented__option', segment.icon && 'sv-segmented__option--icon')}
            onClick={() => onChange(segment.value)}
          >
            {segment.icon ? <Icon icon={segment.icon} size={14} /> : segment.label}
            {segment.count !== undefined && (
              <span className="sv-count-chip sv-num">{segment.count}</span>
            )}
          </button>
        );
        return segment.icon ? (
          <Tooltip key={segment.value} label={segment.label}>
            {button}
          </Tooltip>
        ) : (
          button
        );
      })}
    </div>
  );
}
