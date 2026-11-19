import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { cx } from './cx';

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** How many dialogs are open: the page behind them is inert while any is. */
let openDialogs = 0;

function setPageInert(inert: boolean) {
  const root = document.getElementById('root');
  if (root) root.inert = inert;
}

function focusableIn(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (element) => element.getClientRects().length > 0,
  );
}

export interface DialogProps {
  /** Names the dialog (`aria-labelledby`) and is shown as its heading. */
  title: string;
  /** Called for Escape. Cancel buttons in the footer call it too. Not called while `busy`. */
  onClose: () => void;
  /** The body. It scrolls when the dialog is taller than the window. */
  children: ReactNode;
  /** The footer actions, Cancel first and the primary action last. */
  footer: ReactNode;
  /** About 420, 560 or 720 px wide, never wider than the window. */
  size?: 'small' | 'medium' | 'large';
  /** An action is running: Escape does nothing and the dialog is marked busy. */
  busy?: boolean;
  /** Change it when the body shows something new (a wizard step) to move focus in again. */
  focusKey?: string | number;
}

/**
 * Modal dialog. Render it only while it is open: it moves focus in on mount (to the element marked
 * `data-autofocus`, else the first control in the body, else the footer), traps Tab, makes the page
 * behind it inert, and hands focus back to whatever opened it on unmount. Escape follows Cancel.
 * There is no close on backdrop click: a stray click must never discard an entered choice.
 * For a destructive confirmation mark the Cancel button `data-autofocus`.
 */
export function Dialog({
  title,
  onClose,
  children,
  footer,
  size = 'medium',
  busy = false,
  focusKey,
}: DialogProps) {
  const titleId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const [opener] = useState(() =>
    document.activeElement instanceof HTMLElement ? document.activeElement : null,
  );
  const latest = useRef({ onClose, busy });
  useEffect(() => {
    latest.current = { onClose, busy };
  });

  const focusInitial = useCallback(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    const marked = dialog.querySelector<HTMLElement>('[data-autofocus]');
    const target =
      marked ??
      (bodyRef.current ? focusableIn(bodyRef.current)[0] : undefined) ??
      focusableIn(dialog)[0] ??
      dialog;
    target.focus();
  }, []);

  useEffect(() => {
    openDialogs += 1;
    setPageInert(true);
    return () => {
      openDialogs -= 1;
      if (openDialogs === 0) setPageInert(false);
      if (opener?.isConnected) opener.focus();
    };
  }, [opener]);

  // Focus moves in on mount and again whenever the body announces a new step.
  useEffect(() => {
    focusInitial();
  }, [focusKey, focusInitial]);

  // Focus must never rest outside the dialog: a disabled button or a removed control drops it to
  // the page, and the page is inert. The dialog itself takes it, so Tab starts from the top.
  useEffect(() => {
    const pullBack = () => {
      const dialog = dialogRef.current;
      if (dialog && !dialog.contains(document.activeElement)) dialog.focus();
    };
    document.addEventListener('focusin', pullBack);
    return () => document.removeEventListener('focusin', pullBack);
  }, []);
  const keepFocusInside = () => {
    // After the event, so a click that moves focus to another control is not mistaken for a loss.
    setTimeout(() => {
      const dialog = dialogRef.current;
      if (dialog && !dialog.contains(document.activeElement)) dialog.focus();
    }, 0);
  };

  const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      // The library's inspector drawer also listens for Escape on the document.
      event.stopPropagation();
      event.preventDefault();
      if (!latest.current.busy) latest.current.onClose();
      return;
    }
    if (event.key !== 'Tab') return;
    const dialog = dialogRef.current;
    if (!dialog) return;
    const stops = focusableIn(dialog);
    const first = stops[0];
    const last = stops[stops.length - 1];
    if (!first || !last) {
      event.preventDefault();
      dialog.focus();
    } else if (
      event.shiftKey &&
      (document.activeElement === first || document.activeElement === dialog)
    ) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  return createPortal(
    <div className="sv-app sv-dialog-layer">
      <div className="sv-dialog-backdrop" aria-hidden="true" />
      <div
        ref={dialogRef}
        className={cx('sv-dialog', `sv-dialog--${size}`)}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-busy={busy || undefined}
        tabIndex={-1}
        onKeyDown={onKeyDown}
        onBlur={keepFocusInside}
      >
        <div className="sv-dialog__header">
          <h2 id={titleId} className="sv-dialog__title">
            {title}
          </h2>
        </div>
        <div ref={bodyRef} className="sv-dialog__body">
          {children}
        </div>
        <div className="sv-dialog__footer">{footer}</div>
      </div>
    </div>,
    document.body,
  );
}
