import { ChevronRight } from 'lucide-react';
import { useId, useState } from 'react';
import { formatCount } from '../../lib/format';
import { strings } from '../../strings';
import { Icon } from '../../ui/Icon';

const t = strings.savegames;

/** A mod's id is its file or folder name; the game's mod name is that without `.zip`. */
const nameOfId = (id: string) => id.replace(/\.zip$/i, '');

/** The mods in the folder that the savegame does not use, collapsed by default. */
export function ExtrasDisclosure({ ids }: { ids: readonly string[] }) {
  const [open, setOpen] = useState(false);
  const listId = useId();

  return (
    <section className="mm-extras">
      <button
        type="button"
        className="mm-extras__toggle"
        aria-expanded={open}
        aria-controls={listId}
        onClick={() => setOpen(!open)}
      >
        <Icon
          icon={ChevronRight}
          size={16}
          className={open ? 'mm-chevron mm-chevron--open' : 'mm-chevron'}
        />
        <span className="mm-extras__title">{t.extrasTitle}</span>
        <span className="sv-count-chip sv-num">{formatCount(ids.length)}</span>
      </button>
      {open && (
        <div id={listId} className="mm-extras__body">
          <p className="mm-extras__note">{ids.length === 0 ? t.extrasNone : t.extrasText}</p>
          {ids.length > 0 && (
            <ul className="mm-namelist" aria-label={t.extrasLabel}>
              {ids.map((id) => (
                <li key={id} className="sv-truncate sv-selectable">
                  {nameOfId(id)}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </section>
  );
}
