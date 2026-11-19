import type { LucideIcon } from 'lucide-react';

export interface IconProps {
  icon: LucideIcon;
  /** 14 small marks, 16 compact, 18 standard, 22 status/prominent, 32 empty states. */
  size?: 14 | 16 | 18 | 22 | 32;
  className?: string;
}

/**
 * Stroke in screen pixels per size. Lucide would otherwise scale the stroke with the icon (a 16px
 * icon drew a 1.17px line that smeared across two pixels).
 *
 * The sizes are one step above the design system's (16px standard): at 100% display scaling a
 * 16px icon has too few pixels for smooth diagonals and curves, and the user chose 18px after
 * comparing them on their monitor (docs/dev/decisions.md, 2026-11-19).
 */
const STROKE: Record<NonNullable<IconProps['size']>, number> = {
  14: 1.5,
  16: 1.5,
  18: 1.75,
  22: 2,
  32: 2,
};

/** Outline icon at the design system's stroke weight. Decorative: name the control, not the icon. */
export function Icon({ icon: Glyph, size = 18, className }: IconProps) {
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
