import type { LucideIcon } from 'lucide-react';

export interface IconProps {
  icon: LucideIcon;
  /** 14 compact, 16 standard, 20 status/prominent, 32 empty states. */
  size?: 12 | 14 | 16 | 20 | 32;
  className?: string;
}

/** Outline icon at the design system's stroke weight. Decorative: name the control, not the icon. */
export function Icon({ icon: Glyph, size = 16, className }: IconProps) {
  return (
    <Glyph
      size={size}
      strokeWidth={1.75}
      className={className}
      aria-hidden="true"
      focusable="false"
    />
  );
}
