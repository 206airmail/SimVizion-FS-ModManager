import { ChevronRight, Copy } from 'lucide-react';
import { useId, useState } from 'react';
import type { Profile, SaveModRow } from '../../api';
import { strings } from '../../strings';
import { Icon } from '../../ui/Icon';
import { ReasonButton } from '../files/ReasonButton';
import type { CopyGroup } from './report';

const t = strings.savegames;

export interface MissingActionsProps {
  groups: readonly CopyGroup[];
  nowhere: readonly SaveModRow[];
  profiles: readonly Profile[];
  /** The folder the missing mods would be copied into. */
  targetName: string;
  /** Why copying is unavailable right now (game running, another operation), or null. */
  blockedReason: string | null;
  /** The folder a copy is being prepared for, if any. */
  preparing: string | null;
  onCopy: (group: CopyGroup) => void;
}

/**
 * What can be done about the missing mods: one "Copy N from <folder>" button per folder that has
 * some, and the list of those that no folder has. A single source makes its button the primary
 * action; with several there is no single main choice, so none is.
 */
export function MissingActions({
  groups,
  nowhere,
  profiles,
  targetName,
  blockedReason,
  preparing,
  onCopy,
}: MissingActionsProps) {
  const [showNames, setShowNames] = useState(false);
  const listId = useId();
  const nameOf = (id: string) => profiles.find((profile) => profile.id === id)?.name ?? id;

  return (
    <section className="mm-missing" aria-label={t.copyMissingTitle}>
      {groups.length > 0 && (
        <div className="mm-missing__block">
          <div className="mm-missing__text">
            <h3 className="mm-missing__title">{t.copyMissingTitle}</h3>
            <p className="mm-missing__note">{t.copyMissingText}</p>
          </div>
          <div className="mm-missing__buttons">
            {groups.map((group) => (
              <ReasonButton
                key={group.fromProfileId}
                size="compact"
                variant={groups.length === 1 ? 'primary' : 'secondary'}
                icon={Copy}
                hint={t.copyFromHint(
                  group.modNames.length,
                  nameOf(group.fromProfileId),
                  targetName,
                )}
                blockedReason={blockedReason}
                aria-busy={preparing === group.fromProfileId || undefined}
                onClick={() => preparing === null && onCopy(group)}
              >
                {preparing === group.fromProfileId
                  ? t.copyPreparing
                  : t.copyFrom(group.modNames.length, nameOf(group.fromProfileId))}
              </ReasonButton>
            ))}
          </div>
        </div>
      )}

      {nowhere.length > 0 && (
        <div className="mm-missing__block mm-missing__block--nowhere">
          <div className="mm-missing__text">
            <h3 className="mm-missing__title">{t.notAnywhereTitle}</h3>
            <p className="mm-missing__note">{t.notAnywhereCount(nowhere.length)}</p>
          </div>
          <button
            type="button"
            className="sv-link-button mm-missing__toggle"
            aria-expanded={showNames}
            aria-controls={listId}
            onClick={() => setShowNames(!showNames)}
          >
            <Icon
              icon={ChevronRight}
              size={16}
              className={showNames ? 'mm-chevron mm-chevron--open' : 'mm-chevron'}
            />
            {showNames ? t.notAnywhereHide : t.notAnywhereShow}
          </button>
          {showNames && (
            <ul id={listId} className="mm-namelist" aria-label={t.notAnywhereLabel}>
              {nowhere.map((row) => (
                <li key={row.modName} className="sv-truncate sv-selectable" title={row.title}>
                  {row.modName}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </section>
  );
}
