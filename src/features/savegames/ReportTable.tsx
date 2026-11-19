import { memo } from 'react';
import type { Profile, SaveModRow } from '../../api';
import { strings } from '../../strings';
import { cx } from '../../ui/cx';
import { Pill } from '../../ui/Pill';
import { Tooltip } from '../../ui/Tooltip';
import { saveStateView } from './labels';

const t = strings.savegames;

export interface ReportTableProps {
  rows: readonly SaveModRow[];
  label: string;
  profiles: readonly Profile[];
  /** The file check is running; when it is not, a row it never reached reads "Not checked". */
  checking: boolean;
}

/**
 * A savegame's mods against one folder: the title, the version the save was made with, the version
 * in the folder, the state, and the other folders that have the mod. Read-only; it scrolls with
 * the rest of the report, and the header stays at the top of the scroll area.
 */
export function ReportTable({ rows, label, profiles, checking }: ReportTableProps) {
  const names = new Map(profiles.map((profile) => [profile.id, profile.name]));
  return (
    <div className="mm-rpt" role="table" aria-label={label} aria-rowcount={rows.length + 1}>
      <div className="sv-table__head mm-rpt__cols mm-rpt__head" role="row" aria-rowindex={1}>
        <div role="columnheader" className="sv-table__head-cell">
          {t.columns.mod}
        </div>
        <div role="columnheader" className="sv-table__head-cell">
          {t.columns.saved}
        </div>
        <div role="columnheader" className="sv-table__head-cell">
          {t.columns.installed}
        </div>
        <div role="columnheader" className="sv-table__head-cell">
          {t.columns.state}
        </div>
        <div role="columnheader" className="sv-table__head-cell mm-rpt__c-found">
          {t.columns.foundIn}
        </div>
      </div>
      <div role="rowgroup">
        {rows.map((row, index) => (
          <ReportRow key={row.modName} row={row} index={index} names={names} checking={checking} />
        ))}
      </div>
    </div>
  );
}

const ReportRow = memo(function ReportRow({
  row,
  index,
  names,
  checking,
}: {
  row: SaveModRow;
  index: number;
  names: ReadonlyMap<string, string>;
  checking: boolean;
}) {
  const unchecked = row.state === 'hashPending' && !checking;
  const view = saveStateView[row.state];
  const foundIn = row.foundIn.map((id) => names.get(id) ?? id).join(', ');
  return (
    <div role="row" aria-rowindex={index + 2} className="sv-row mm-rpt__cols">
      <div role="cell" className="sv-cell mm-rpt__title">
        <span className="sv-truncate" title={row.modName}>
          {row.title}
        </span>
        {row.required && (
          <Pill size="compact" className="mm-rpt__required">
            {t.required}
          </Pill>
        )}
      </div>
      <div role="cell" className="sv-cell sv-cell--secondary sv-truncate sv-num">
        {row.savedVersion}
      </div>
      <div
        role="cell"
        className={cx('sv-cell sv-truncate sv-num', row.installed === null && 'mm-rpt__none')}
        title={row.installed?.summary.fileName}
      >
        {row.installed?.summary.version ?? '–'}
      </div>
      <div role="cell" className="sv-cell mm-rpt__state">
        <Tooltip label={unchecked ? t.stateHints.notChecked : t.stateHints[row.state]}>
          <Pill
            tone={view.tone}
            icon={view.icon}
            size="compact"
            className={row.state === 'hashPending' && checking ? 'mm-pill--busy' : undefined}
          >
            {unchecked ? t.states.notChecked : t.states[row.state]}
          </Pill>
        </Tooltip>
      </div>
      <div
        role="cell"
        className="sv-cell sv-cell--secondary sv-truncate mm-rpt__c-found"
        title={foundIn || undefined}
      >
        {foundIn}
      </div>
    </div>
  );
});
