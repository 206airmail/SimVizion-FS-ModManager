/** Placeholder product emblem; matches src-tauri/app-icon.svg until real artwork is supplied. */
export function Emblem() {
  return (
    <svg viewBox="0 0 48 48" width="28" height="28" aria-hidden="true" focusable="false">
      <defs>
        <linearGradient id="mm-emblem-mark" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--sv-lime)" />
          <stop offset="1" stopColor="var(--sv-teal)" />
        </linearGradient>
      </defs>
      <path
        d="M24 5 40.5 14.5v19L24 43 7.5 33.5v-19Z"
        fill="none"
        stroke="url(#mm-emblem-mark)"
        strokeWidth="3"
        strokeLinejoin="round"
      />
      <path
        d="M15.5 29.5v-11l8.5 6.5 8.5-6.5v11"
        fill="none"
        stroke="var(--sv-text)"
        strokeWidth="3"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
