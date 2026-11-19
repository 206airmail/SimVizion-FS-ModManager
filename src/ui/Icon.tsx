import type { LucideIcon } from 'lucide-react';

export interface IconProps {
  icon: LucideIcon;
  /** 14 compact, 16 standard, 20 status/prominent, 32 empty states. */
  size?: 12 | 14 | 16 | 20 | 32;
  className?: string;
}

/**
 * Stroke in screen pixels per size, as the design system specifies them (16px: 1.75px; compact:
 * 1.5px; status and larger: 2px). Lucide would otherwise scale the stroke with the icon, so a 16px
 * icon drew a 1.17px line that smears across two pixels and looks soft.
 */
const STROKE: Record<NonNullable<IconProps['size']>, number> = {
  12: 1.5,
  14: 1.5,
  16: 1.75,
  20: 2,
  32: 2,
};

/** Outline icon at the design system's stroke weight. Decorative: name the control, not the icon. */
export function Icon({ icon: Glyph, size = 16, className }: IconProps) {
  return (
    <Glyph
      size={size}
      strokeWidth={STROKE[size]}
      absoluteStrokeWidth
      className={className}
      aria-hidden="true"
      focusable="false"
    />
  );
}
