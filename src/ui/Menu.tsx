import { Ellipsis, type LucideIcon } from 'lucide-react';
import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { cx } from './cx';
import { Icon } from './Icon';
import { Tooltip } from './Tooltip';

const ANCHOR_GAP = 6;
const VIEWPORT_INSET = 8;

export interface MenuItem {
  id: string;
  label: string;
  icon?: LucideIcon;
  /** Red text and icon, for a command that removes something. Hover stays neutral. */
  destructive?: boolean;
  /**
   * Makes the item unavailable and says why. It stays focusable so the reason can be read (it is
   * the item's tooltip and its description); choosing it does nothing.
   */
  disabledReason?: string;
  /** A divider is drawn above this item. */
  separatorBefore?: boolean;
  onSelect: () => void;
}

/** Where a menu opens: below an element (aligned to its right edge), or at a point. */
export type MenuAnchor = HTMLElement | { x: number; y: number };

const itemButtonsOf = (menu: HTMLElement | null) =>
  Array.from(menu?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []);

export interface MenuPopupProps {
  /** Set to let a trigger point at the menu with `aria-controls`. */
  id?: string;
  /** Accessible name of the menu. */
  label: string;
  items: ReadonlyArray<MenuItem>;
  anchor: MenuAnchor;
  /** `restoreFocus` is true when the menu ended by a choice or Escape, false after an outside click. */
  onClose: (restoreFocus: boolean) => void;
}

/**
 * The floating list of a menu, positioned against its anchor and rendered in a portal so no panel
 * clips it. Arrow keys, Home and End move between items; Enter and Space choose; Escape closes and
 * Tab closes too; a press outside, a resize or a scroll elsewhere closes without moving focus.
 */
export function MenuPopup({ id, label, items, anchor, onClose }: MenuPopupProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const baseId = useId();

  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!menu) return;
    const size = menu.getBoundingClientRect();
    let top: number;
    let left: number;
    if (anchor instanceof HTMLElement) {
      const a = anchor.getBoundingClientRect();
      const below = a.bottom + ANCHOR_GAP;
      const above = a.top - ANCHOR_GAP - size.height;
      top =
        below + size.height <= window.innerHeight - VIEWPORT_INSET || above < VIEWPORT_INSET
          ? below
          : above;
      left = a.right - size.width;
    } else {
      top = anchor.y;
      left = anchor.x;
    }
    menu.style.top = `${Math.round(Math.max(VIEWPORT_INSET, Math.min(top, window.innerHeight - size.height - VIEWPORT_INSET)))}px`;
    menu.style.left = `${Math.round(Math.max(VIEWPORT_INSET, Math.min(left, window.innerWidth - size.width - VIEWPORT_INSET)))}px`;
    menu.style.opacity = '1';
    const buttons = itemButtonsOf(menu);
    (
      buttons.find((button) => button.getAttribute('aria-disabled') !== 'true') ?? buttons[0]
    )?.focus();
  }, [anchor]);

  useEffect(() => {
    const closeOnOutsidePress = (event: PointerEvent) => {
      const target = event.target as Node;
      if (menuRef.current?.contains(target)) return;
      // The trigger toggles the menu itself; closing here as well would reopen it.
      if (anchor instanceof HTMLElement && anchor.contains(target)) return;
      onClose(false);
    };
    const closeOnScroll = (event: Event) => {
      if (!menuRef.current?.contains(event.target as Node)) onClose(false);
    };
    const closeQuietly = () => onClose(false);
    document.addEventListener('pointerdown', closeOnOutsidePress);
    window.addEventListener('resize', closeQuietly);
    window.addEventListener('blur', closeQuietly);
    document.addEventListener('scroll', closeOnScroll, true);
    return () => {
      document.removeEventListener('pointerdown', closeOnOutsidePress);
      window.removeEventListener('resize', closeQuietly);
      window.removeEventListener('blur', closeQuietly);
      document.removeEventListener('scroll', closeOnScroll, true);
    };
  }, [anchor, onClose]);

  const move = (to: 'next' | 'previous' | 'first' | 'last') => {
    const buttons = itemButtonsOf(menuRef.current);
    if (buttons.length === 0) return;
    const current = buttons.indexOf(document.activeElement as HTMLElement);
    const index =
      to === 'first'
        ? 0
        : to === 'last'
          ? buttons.length - 1
          : to === 'next'
            ? (current + 1) % buttons.length
            : (current - 1 + buttons.length) % buttons.length;
    buttons[index]?.focus();
  };

  const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const keys: Record<string, () => void> = {
      ArrowDown: () => move('next'),
      ArrowUp: () => move('previous'),
      Home: () => move('first'),
      End: () => move('last'),
      Escape: () => onClose(true),
      Tab: () => onClose(true),
    };
    const action = keys[event.key];
    if (!action) return;
    event.preventDefault();
    // Escape closes the menu only: a dialog or drawer behind it must not also react.
    event.stopPropagation();
    action();
  };

  const choose = (item: MenuItem) => {
    if (item.disabledReason !== undefined) return;
    // Focus goes back first, so whatever the item opens remembers the right element to return to.
    onClose(true);
    item.onSelect();
  };

  return createPortal(
    <div
      ref={menuRef}
      id={id}
      className="sv-app sv-menu"
      role="menu"
      aria-label={label}
      onKeyDown={onKeyDown}
    >
      {items.map((item) => {
        const reasonId = `${baseId}-${item.id}`;
        const disabled = item.disabledReason !== undefined;
        const row = (
          <button
            type="button"
            role="menuitem"
            tabIndex={-1}
            className={cx('sv-menu__item', item.destructive && 'sv-menu__item--destructive')}
            aria-disabled={disabled || undefined}
            aria-describedby={disabled ? reasonId : undefined}
            onClick={() => choose(item)}
            onPointerMove={(event) => event.currentTarget.focus({ preventScroll: true })}
          >
            {item.icon && <Icon icon={item.icon} size={16} />}
            <span className="sv-menu__label">{item.label}</span>
          </button>
        );
        return (
          <div key={item.id} className="sv-menu__row">
            {item.separatorBefore && <div className="sv-menu__divider" role="separator" />}
            {item.disabledReason !== undefined ? (
              <>
                <Tooltip label={item.disabledReason}>{row}</Tooltip>
                <span id={reasonId} className="sv-visually-hidden">
                  {item.disabledReason}
                </span>
              </>
            ) : (
              row
            )}
          </div>
        );
      })}
    </div>,
    document.body,
  );
}

export interface MenuProps {
  /** Accessible name of the trigger and of the menu. */
  label: string;
  items: ReadonlyArray<MenuItem>;
  /** The trigger's glyph. Default: three dots. */
  icon?: LucideIcon;
  /** When set the trigger is a labelled button instead of an icon-only one. */
  text?: string;
  size?: 'standard' | 'compact';
  className?: string;
  /** Reports the open state, so a parent can keep a hover-only trigger visible while it is open. */
  onOpenChange?: (open: boolean) => void;
}

/** A button that opens a menu below it (above when there is no room). */
export function Menu({
  label,
  items,
  icon = Ellipsis,
  text,
  size = 'compact',
  className,
  onOpenChange,
}: MenuProps) {
  // The trigger element doubles as the anchor, so open is simply "an anchor is set".
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const open = anchor !== null;
  const menuId = useId();

  const setOpenAt = (next: HTMLButtonElement | null) => {
    setAnchor(next);
    onOpenChange?.(next !== null);
  };
  const close = (restoreFocus: boolean) => {
    if (restoreFocus) anchor?.focus();
    setOpenAt(null);
  };

  const trigger = (
    <button
      type="button"
      className={cx(
        'sv-button',
        'sv-button--secondary',
        text === undefined && 'sv-button--icon',
        size === 'compact' && 'sv-button--compact',
        className,
      )}
      aria-label={text === undefined ? label : undefined}
      aria-haspopup="menu"
      aria-expanded={open}
      aria-controls={open ? menuId : undefined}
      onClick={(event) => setOpenAt(open ? null : event.currentTarget)}
      onKeyDown={(event) => {
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
          event.preventDefault();
          setOpenAt(event.currentTarget);
        }
      }}
    >
      <Icon icon={icon} size={size === 'compact' ? 16 : 18} />
      {text}
    </button>
  );

  return (
    <>
      {text === undefined ? <Tooltip label={label}>{trigger}</Tooltip> : trigger}
      {anchor && (
        <MenuPopup id={menuId} label={label} items={items} anchor={anchor} onClose={close} />
      )}
    </>
  );
}
