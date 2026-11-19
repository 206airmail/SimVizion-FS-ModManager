import type { ModKind } from '..';

// Original placeholder artwork: a dark two-stop gradient and a simple light glyph per kind.
// Each icon is a small 128x128 SVG returned as a data URI.

const FOREGROUND = '#e4f0ec';

function hslToHex(h: number, s: number, l: number): string {
  const a = s * Math.min(l, 1 - l);
  const channel = (n: number): string => {
    const k = (n + h / 30) % 12;
    const value = l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    return Math.round(255 * value)
      .toString(16)
      .padStart(2, '0');
  };
  return `#${channel(0)}${channel(8)}${channel(4)}`;
}

/** `bg` is the darker gradient colour, used to cut small holes out of the glyph. */
function glyph(kind: ModKind, variant: number, bg: string): string {
  switch (kind) {
    case 'vehicle':
      return (
        `<rect x='18' y='62' width='50' height='22' rx='4'/>` +
        `<rect x='62' y='40' width='32' height='44' rx='5'/>` +
        `<rect x='32' y='44' width='5' height='20'/>` +
        `<circle cx='84' cy='88' r='18'/><circle cx='36' cy='92' r='10'/>` +
        `<g fill='${bg}'><circle cx='84' cy='88' r='7'/><circle cx='36' cy='92' r='4'/></g>`
      );
    case 'placeable':
      return (
        `<path d='M20 62 64 32l44 30v42H20z'/>` +
        `<rect x='50' y='70' width='28' height='34' fill='${bg}'/>`
      );
    case 'script':
      return variant % 2 === 0
        ? `<path d='M50 44 26 64l24 20M78 44l24 20-24 20M70 38 58 90' fill='none' stroke='${FOREGROUND}' stroke-width='8' stroke-linecap='round' stroke-linejoin='round'/>`
        : `<circle cx='64' cy='64' r='30' fill='none' stroke='${FOREGROUND}' stroke-width='10' stroke-dasharray='11.8'/>` +
            `<circle cx='64' cy='64' r='20' fill='none' stroke='${FOREGROUND}' stroke-width='10'/>`;
    case 'map':
      return `<circle cx='92' cy='42' r='11'/><path d='M12 100 48 54l22 28 14-16 32 34z'/>`;
    case 'other':
      return (
        `<rect x='32' y='44' width='64' height='48' rx='4'/>` +
        `<path d='M32 62h64M64 44v18' fill='none' stroke='${bg}' stroke-width='5'/>`
      );
  }
}

function toDataUri(svg: string): string {
  const body = encodeURIComponent(svg)
    .replace(/%3D/g, '=')
    .replace(/%3A/g, ':')
    .replace(/%2F/g, '/')
    .replace(/%2C/g, ',')
    .replace(/\(/g, '%28')
    .replace(/\)/g, '%29');
  return `data:image/svg+xml,${body}`;
}

/** Deterministic for a given hash and kind. */
export function placeholderIcon(hash: number, kind: ModKind): string {
  const hue = hash % 360;
  const light = 0.15 + ((hash >>> 9) % 8) / 100;
  const top = hslToHex(hue, 0.5, light + 0.07);
  const bottom = hslToHex((hue + 28) % 360, 0.55, light - 0.04);
  return toDataUri(
    `<svg xmlns='http://www.w3.org/2000/svg' width='128' height='128' viewBox='0 0 128 128'>` +
      `<defs><linearGradient id='g' x2='0' y2='1'><stop stop-color='${top}'/>` +
      `<stop offset='1' stop-color='${bottom}'/></linearGradient></defs>` +
      `<rect width='128' height='128' fill='url(#g)'/>` +
      `<g fill='${FOREGROUND}' fill-opacity='.88'>${glyph(kind, hash >>> 17, bottom)}</g></svg>`,
  );
}
