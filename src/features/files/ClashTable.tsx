import type { ModSummary, TransferClash } from '../../api';
import { formatBytes, formatDate } from '../../lib/format';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { SegmentedControl } from '../../ui/SegmentedControl';
import { Tooltip } from '../../ui/Tooltip';
import { setAll, setOne, type ClashDecisions } from './choices';

export interface ClashTableProps {
  clashes: readonly TransferClash[];
  decisions: ClashDecisions;
  onChange: (decisions: ClashDecisions) => void;
}

type Choice = 'replace' | 'skip';

const t = strings.fileOps.transfer;

/**
 * Both versions of every mod that already exists in the target, side by side, with Replace or Skip
 * per mod and an "apply to all" pair above. Skip is where every row starts.
 */
export function ClashTable({ clashes, decisions, onChange }: ClashTableProps) {
  return (
    <div className="mm-clash">
      <div className="mm-clash__tools">
        <span className="mm-clash__tools-label">{t.applyAll}</span>
        <Button size="compact" onClick={() => onChange(setAll(clashes, true))}>
          {t.replaceAll}
        </Button>
        <Button size="compact" onClick={() => onChange(setAll(clashes, false))}>
          {t.skipAll}
        </Button>
      </div>

      <div className="mm-clash__scroll">
        <div className="mm-clash__table" role="table" aria-label={t.clashTableLabel}>
          <div className="sv-table__head mm-clash__cols" role="row">
            <div className="sv-table__head-cell" role="columnheader">
              {t.colMod}
            </div>
            <div className="sv-table__head-cell" role="columnheader">
              {t.colIncoming}
            </div>
            <div className="sv-table__head-cell" role="columnheader">
              {t.colExisting}
            </div>
            <div className="sv-table__head-cell" role="columnheader">
              {t.colChoice}
            </div>
          </div>
          {clashes.map((clash) => (
            <div key={clash.modName} className="mm-clash__row mm-clash__cols" role="row">
              <div className="sv-cell" role="cell">
                <Tooltip label={clash.modName}>
                  <span className="mm-clash__name sv-truncate sv-selectable">{clash.modName}</span>
                </Tooltip>
              </div>
              <div className="sv-cell" role="cell">
                <Version summary={clash.incoming} />
              </div>
              <div className="sv-cell" role="cell">
                <Version summary={clash.existing} />
              </div>
              <div className="sv-cell" role="cell">
                <SegmentedControl<Choice>
                  label={t.choiceLabel(clash.modName)}
                  value={decisions.get(clash.modName) === true ? 'replace' : 'skip'}
                  onChange={(choice) =>
                    onChange(setOne(decisions, clash.modName, choice === 'replace'))
                  }
                  segments={[
                    { value: 'replace', label: t.choiceReplace },
                    { value: 'skip', label: t.choiceSkip },
                  ]}
                />
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function Version({ summary }: { summary: ModSummary }) {
  return (
    <span className="mm-clash__version">
      <span className="mm-clash__version-name sv-num">{summary.version || t.noVersion}</span>
      <span className="mm-clash__version-meta sv-num">
        {formatBytes(summary.sizeBytes)} · {formatDate(summary.modified)}
      </span>
    </span>
  );
}
