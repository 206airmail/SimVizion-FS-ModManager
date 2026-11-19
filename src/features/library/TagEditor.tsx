import { Plus, X } from 'lucide-react';
import { useId, useState, type KeyboardEvent } from 'react';
import { strings } from '../../strings';
import { Icon } from '../../ui/Icon';
import {
  MAX_TAG_LENGTH,
  MAX_TAGS,
  parseTagInput,
  suggestTags,
  type TagCount,
  type TagProblem,
} from './tags';

const t = strings.tags;

export interface TagEditorProps {
  /** The tags to show as chips, with how many of the `total` mods have each. */
  chips: readonly TagCount[];
  /** How many mods are being edited: 1 for a single mod. */
  total: number;
  /** Every tag in use, for suggestions. */
  known: readonly string[];
  /** Why this tag cannot be added to every mod being edited, or null. */
  problemFor: (tag: string) => TagProblem | null;
  onAdd: (tags: string[]) => void;
  onRemove: (tag: string) => void;
}

/**
 * Chips with a remove button, an input (Enter or a comma adds what is typed) and the existing tags
 * as one-press suggestions. With several mods, a chip shows how many have the tag ("2/5") and
 * removing it takes it off all of them; adding puts the tag on all of them.
 */
export function TagEditor({ chips, total, known, problemFor, onAdd, onRemove }: TagEditorProps) {
  const [text, setText] = useState('');
  const [problem, setProblem] = useState<TagProblem | null>(null);
  const inputId = useId();
  const many = total > 1;
  const offered = suggestTags(known, chips, total, text);

  /** Adds every tag in `raw`; a tag that is not allowed stays in the field with its message. */
  const commit = (raw: string) => {
    const tags = parseTagInput(raw);
    if (tags.length === 0) {
      setText('');
      setProblem(null);
      return;
    }
    const allowed: string[] = [];
    let refused: { tag: string; why: TagProblem } | null = null;
    for (const tag of tags) {
      const why = problemFor(tag);
      if (why === null) allowed.push(tag);
      else refused ??= { tag, why };
    }
    if (allowed.length > 0) onAdd(allowed);
    setText(refused?.tag ?? '');
    setProblem(refused?.why ?? null);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      commit(text);
    } else if (event.key === ',') {
      event.preventDefault();
      commit(text);
    }
  };

  return (
    <div className="mm-tags">
      {chips.length > 0 ? (
        <ul className="mm-tags__chips" aria-label={t.title}>
          {chips.map(({ tag, count }) => (
            <li
              key={tag.toLowerCase()}
              className="mm-tag"
              title={many ? t.partialHint(tag, count, total) : undefined}
            >
              <span className="mm-tag__name sv-truncate">{tag}</span>
              {many && count < total && (
                <span className="mm-tag__count sv-num">{t.partial(count, total)}</span>
              )}
              <button
                type="button"
                className="mm-tag__remove"
                aria-label={many ? t.removeFromAll(tag) : t.remove(tag)}
                onClick={() => onRemove(tag)}
              >
                <Icon icon={X} size={14} />
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="mm-tags__none">{many ? t.noneMany : t.none}</p>
      )}

      <div className="mm-tags__field">
        <input
          id={inputId}
          className="mm-input mm-tags__input"
          type="text"
          spellCheck={false}
          autoComplete="off"
          maxLength={MAX_TAG_LENGTH * 4}
          aria-label={many ? t.inputLabelMany(total) : t.inputLabel}
          aria-invalid={problem !== null || undefined}
          placeholder={t.inputPlaceholder}
          value={text}
          onChange={(event) => {
            setText(event.target.value);
            setProblem(null);
          }}
          onKeyDown={onKeyDown}
        />
        <button
          type="button"
          className="sv-button sv-button--secondary sv-button--compact sv-button--icon"
          aria-label={t.add}
          disabled={text.trim() === ''}
          onClick={() => commit(text)}
        >
          <Icon icon={Plus} size={16} />
        </button>
      </div>
      {problem !== null && (
        <p className="mm-field-error" role="alert">
          {problem === 'tooLong' ? t.tooLong(MAX_TAG_LENGTH) : t.tooMany(MAX_TAGS)}
        </p>
      )}

      {offered.length > 0 && (
        <ul className="mm-tags__suggest" aria-label={t.suggestions}>
          {offered.map((tag) => (
            <li key={tag.toLowerCase()}>
              <button
                type="button"
                className="mm-tag mm-tag--suggest"
                aria-label={t.suggestionLabel(tag)}
                onClick={() => commit(tag)}
              >
                <Icon icon={Plus} size={14} />
                <span className="sv-truncate">{tag}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
