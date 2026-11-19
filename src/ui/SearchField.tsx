import { Search, X } from 'lucide-react';
import { useRef } from 'react';
import { cx } from './cx';
import { Icon } from './Icon';

export interface SearchFieldProps {
  value: string;
  onChange: (value: string) => void;
  /** Accessible name; the placeholder is only a hint. */
  label: string;
  placeholder?: string;
  clearLabel: string;
  size?: 'standard' | 'compact';
  className?: string;
}

export function SearchField({
  value,
  onChange,
  label,
  placeholder,
  clearLabel,
  size = 'standard',
  className,
}: SearchFieldProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  return (
    <div className={cx('sv-search', size === 'compact' && 'sv-search--compact', className)}>
      <Icon icon={Search} size={16} className="sv-search__icon" />
      <input
        ref={inputRef}
        className="sv-search__input"
        type="text"
        role="searchbox"
        spellCheck={false}
        autoComplete="off"
        aria-label={label}
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Escape' && value !== '') {
            event.stopPropagation();
            onChange('');
          }
        }}
      />
      {value !== '' && (
        <button
          type="button"
          className="sv-search__clear"
          aria-label={clearLabel}
          onClick={() => {
            onChange('');
            inputRef.current?.focus();
          }}
        >
          <Icon icon={X} size={16} />
        </button>
      )}
    </div>
  );
}
