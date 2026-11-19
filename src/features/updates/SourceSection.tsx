import { Link2 } from 'lucide-react';
import { useState } from 'react';
import type { ModEntry } from '../../api';
import { strings } from '../../strings';
import { Icon } from '../../ui/Icon';
import { cx } from '../../ui/cx';
import { describeSource } from '../library/labels';
import { LinkDialog } from './LinkDialog';
import { PageLinks } from './PageLinks';

const t = strings.inspector;

/**
 * The Inspector's Source section: where the mod comes from, a button for each page it can be
 * opened on, and the link dialog ("Link source…", "Edit link…"). Links belong to the mod name,
 * so editing one changes every folder's copy; `onChanged` reads the list again.
 */
export function SourceSection({ mod, onChanged }: { mod: ModEntry; onChanged: () => void }) {
  const [editing, setEditing] = useState(false);
  const manual = mod.links.find((link) => link.kind === 'manual');
  const known = mod.source.type !== 'unknown';

  return (
    <>
      <p className={cx('mm-source__text sv-selectable', !known && 'mm-source__text--quiet')}>
        {known ? describeSource(mod.source) : t.sourceNone}
      </p>
      <PageLinks mod={mod} />
      <button
        type="button"
        className="sv-link-button mm-source__edit"
        onClick={() => setEditing(true)}
      >
        <Icon icon={Link2} size={16} />
        {manual ? t.editLink : t.linkSource}
      </button>
      {editing && (
        <LinkDialog
          mod={mod}
          currentHost={manual?.label ?? null}
          onClose={() => setEditing(false)}
          onChanged={onChanged}
        />
      )}
    </>
  );
}
