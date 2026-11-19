import type { ModEntry } from '../../api';
import { cx } from '../../ui/cx';
import { Icon } from '../../ui/Icon';
import { kindIcons } from './labels';

export interface ModIconProps {
  mod: Pick<ModEntry, 'icon' | 'kind'>;
  size: 'row' | 'card' | 'detail';
}

const fallbackSizes = { row: 14, card: 32, detail: 32 } as const;

/** A mod's artwork on a consistent stage, or a quiet type glyph when it has none. */
export function ModIcon({ mod, size }: ModIconProps) {
  return (
    <span className={cx('mm-modicon', `mm-modicon--${size}`)}>
      {mod.icon ? (
        <img src={mod.icon} alt="" loading="lazy" decoding="async" draggable={false} />
      ) : (
        <Icon icon={kindIcons[mod.kind]} size={fallbackSizes[size]} />
      )}
    </span>
  );
}
