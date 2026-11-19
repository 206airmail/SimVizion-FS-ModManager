import { Check, ChevronDown } from 'lucide-react';
import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { cx } from './cx';
import { Icon } from './Icon';

export interface SelectOption<T extends string> {
  value: T;
  label: string;
  /** Quiet trailing text, such as a count. */
  hint?: string;
}

export interface SelectProps<T extends string> {
  value: T;
  options: ReadonlyArray<SelectOption<T>>;
  onChange: (value: T) => void;
  /** Accessible name. */
  label: string;
  /** Quiet text shown before the value inside the field, e.g. "Type". */
  prefix?: string;
  size?: 'standard' | 'compact';
  className?: string;
}

/** Dropdown whose field and menu form one continuous shape. */
export function Select<T extends string>({
  value,
  options,
  onChange,
  label,
  prefix,
  size = 'standard',
  className,
}: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const [highlighted, setHighlighted] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  const selectedIndex = Math.max(
    0,
    options.findIndex((option) => option.value === value),
  );
  const selected = options[selectedIndex];

  useEffect(() => {
    if (!open) return;
    const closeOnOutsidePress = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', closeOnOutsidePress);
    return () => document.removeEventListener('pointerdown', closeOnOutsidePress);
  }, [open]);

  const openMenu = () => {
    setHighlighted(selectedIndex);
    setOpen(true);
  };

  const commit = (index: number) => {
    const option = options[index];
    if (option) onChange(option.value);
    setOpen(false);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!open) {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)) {
        event.preventDefault();
        openMenu();
      }
      return;
    }
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        setHighlighted((index) => Math.min(index + 1, options.length - 1));
        break;
      case 'ArrowUp':
        event.preventDefault();
        setHighlighted((index) => Math.max(index - 1, 0));
        break;
      case 'Home':
        event.preventDefault();
        setHighlighted(0);
        break;
      case 'End':
        event.preventDefault();
        setHighlighted(options.length - 1);
        break;
      case 'Enter':
      case ' ':
        event.preventDefault();
        commit(highlighted);
        break;
      case 'Escape':
        event.preventDefault();
        event.stopPropagation();
        setOpen(false);
        break;
      case 'Tab':
        setOpen(false);
        break;
    }
  };

  return (
    <div
      ref={rootRef}
      className={cx(
        'sv-select',
        size === 'compact' && 'sv-select--compact',
        open && 'sv-select--open',
        className,
      )}
    >
      <button
        type="button"
        className="sv-select__field"
        role="combobox"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={open ? `${listId}-${highlighted}` : undefined}
        onClick={() => (open ? setOpen(false) : openMenu())}
        onKeyDown={onKeyDown}
      >
        {prefix && <span className="sv-select__prefix">{prefix}</span>}
        <span className="sv-select__value sv-truncate">{selected?.label}</span>
        <Icon icon={ChevronDown} size={16} className="sv-select__chevron" />
      </button>
      {open && (
        <ul id={listId} className="sv-select__menu" role="listbox" aria-label={label}>
          {options.map((option, index) => (
            <li
              key={option.value}
              id={`${listId}-${index}`}
              role="option"
              aria-selected={option.value === value}
              data-highlighted={index === highlighted}
              className="sv-select__option"
              onPointerMove={() => setHighlighted(index)}
              onClick={() => commit(index)}
            >
              <span className="sv-select__check">
                {option.value === value && <Icon icon={Check} size={16} />}
              </span>
              <span className="sv-select__option-label sv-truncate">{option.label}</span>
              {option.hint && <span className="sv-select__option-hint sv-num">{option.hint}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
