import { CircleArrowUp } from 'lucide-react';
import type { ModEntry } from '../../api';
import { strings } from '../../strings';
import { Icon } from '../../ui/Icon';
import { Pill } from '../../ui/Pill';
import { Tooltip } from '../../ui/Tooltip';
import { severityIcons } from './labels';
import { worstSeverity } from './query';

/** Compact signs that a mod needs attention: its worst issue and an available update. */
export function StatusMarks({ mod }: { mod: ModEntry }) {
  const t = strings.library;
  const severity = worstSeverity(mod);
  const updateVersion = mod.update.type === 'available' ? mod.update.version : null;
  if (severity === null && updateVersion === null) return null;

  return (
    <span className="mm-marks">
      {severity !== null && (
        <Tooltip label={mod.issues.map((issue) => strings.issues[issue.kind]).join(' · ')}>
          <span className="mm-mark" aria-label={t.issueCount(mod.issues.length)}>
            <Icon icon={severityIcons[severity]} size={16} className={`sv-tone-${severity}`} />
            {mod.issues.length > 1 && <span className="sv-num">{mod.issues.length}</span>}
          </span>
        </Tooltip>
      )}
      {updateVersion !== null && (
        <Tooltip label={t.updateTo(updateVersion)}>
          <Pill tone="info" icon={CircleArrowUp} size="compact">
            {t.update}
          </Pill>
        </Tooltip>
      )}
    </span>
  );
}
