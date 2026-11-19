import { Check, Folder } from 'lucide-react';
import type { KeyboardEvent } from 'react';
import type { Profile } from '../../api';
import { tailPath } from '../../lib/format';
import { strings } from '../../strings';
import { Icon } from '../../ui/Icon';

export interface TargetListProps {
  /** The folders that can be chosen: every profile except the one the mods are in. */
  profiles: readonly Profile[];
  value: string | null;
  onChange: (profileId: string) => void;
  label: string;
}

/**
 * One-of-many choice of a target folder, as a list of rows. Arrow keys move the choice (it
 * follows focus, like any radio group); Tab leaves the group.
 */
export function TargetList({ profiles, value, onChange, label }: TargetListProps) {
  const t = strings.fileOps.transfer;
  const stop = profiles.some((profile) => profile.id === value) ? value : profiles[0]?.id;

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step =
      event.key === 'ArrowDown' || event.key === 'ArrowRight'
        ? 1
        : event.key === 'ArrowUp' || event.key === 'ArrowLeft'
          ? -1
          : 0;
    if (step === 0 || profiles.length === 0) return;
    event.preventDefault();
    const current = profiles.findIndex((profile) => profile.id === value);
    const next = profiles[(current + step + profiles.length) % profiles.length];
    if (!next) return;
    onChange(next.id);
    const radios = event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="radio"]');
    radios[profiles.indexOf(next)]?.focus();
  };

  return (
    <div className="mm-targets" role="radiogroup" aria-label={label} onKeyDown={onKeyDown}>
      {profiles.map((profile) => {
        const checked = profile.id === value;
        return (
          <button
            key={profile.id}
            type="button"
            role="radio"
            aria-checked={checked}
            tabIndex={profile.id === stop ? 0 : -1}
            className="mm-target"
            onClick={() => onChange(profile.id)}
          >
            <Icon icon={Folder} />
            <span className="mm-target__body">
              <span className="mm-target__name sv-truncate">{profile.name}</span>
              <span className="mm-target__meta sv-truncate sv-num">
                {t.targetMods(profile.modCount)} · {tailPath(profile.path)}
              </span>
            </span>
            {checked && <Icon icon={Check} className="mm-target__check" />}
          </button>
        );
      })}
    </div>
  );
}
