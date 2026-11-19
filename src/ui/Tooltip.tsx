import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

const SHOW_DELAY_MS = 300;
const ANCHOR_GAP = 6;
const VIEWPORT_INSET = 8;

export interface TooltipProps {
  label: string;
  children: ReactNode;
}

/**
 * Supplemental hint shown after a short hover or on keyboard focus.
 * It never carries the control's only name: give icon-only controls an aria-label too.
 */
export function Tooltip({ label, children }: TooltipProps) {
  const [open, setOpen] = useState(false);
  const anchorRef = useRef<HTMLSpanElement>(null);
  const bubbleRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<number | undefined>(undefined);
  const id = useId();

  const show = (delay: number) => {
    window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => setOpen(true), delay);
  };
  const hide = () => {
    window.clearTimeout(timerRef.current);
    setOpen(false);
  };

  useEffect(() => () => window.clearTimeout(timerRef.current), []);

  // Place above the anchor, flipping below and clamping to the viewport when needed.
  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    const bubble = bubbleRef.current;
    if (!open || !anchor || !bubble) return;
    const a = anchor.getBoundingClientRect();
    const b = bubble.getBoundingClientRect();
    const above = a.top - ANCHOR_GAP - b.height;
    const top = above >= VIEWPORT_INSET ? above : a.bottom + ANCHOR_GAP;
    const centred = a.left + a.width / 2 - b.width / 2;
    const left = Math.min(
      Math.max(centred, VIEWPORT_INSET),
      window.innerWidth - b.width - VIEWPORT_INSET,
    );
    bubble.style.top = `${Math.round(top)}px`;
    bubble.style.left = `${Math.round(left)}px`;
    bubble.style.opacity = '1';
  }, [open]);

  return (
    <span
      ref={anchorRef}
      className="sv-tooltip-anchor"
      onPointerEnter={() => show(SHOW_DELAY_MS)}
      onPointerLeave={hide}
      onPointerDown={hide}
      onFocus={() => show(0)}
      onBlur={hide}
      aria-describedby={open ? id : undefined}
    >
      {children}
      {open &&
        createPortal(
          <div ref={bubbleRef} id={id} role="tooltip" className="sv-app sv-tooltip">
            {label}
          </div>,
          document.body,
        )}
    </span>
  );
}
