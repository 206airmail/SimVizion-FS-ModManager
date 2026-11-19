// Display formatting shared by every screen.

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const;

const countFormat = new Intl.NumberFormat();

export function formatCount(value: number): string {
  return countFormat.format(value);
}

/** Whole numbers below 100 MB and from 100 up; one decimal in between, where it carries meaning. */
function digitsFor(value: number, unit: number): number {
  return unit >= 2 && value < 100 ? 1 : 0;
}

/** Binary units, the same as Windows Explorer: 1 KB = 1,024 bytes. */
export function formatBytes(bytes: number): string {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  // 1,023.97 MB would print as "1024 MB": step up to the next unit instead.
  if (Number(value.toFixed(digitsFor(value, unit))) >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(digitsFor(value, unit))} ${UNITS[unit]}`;
}

/** Calendar date of an ISO 8601 timestamp, in the user's locale unless one is given. */
export function formatDate(iso: string, locale?: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return new Intl.DateTimeFormat(locale, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    timeZone: 'UTC',
  }).format(date);
}

/** Shortens a path to its last segments, keeping the separator style it came with. */
export function tailPath(path: string, keep = 2): string {
  const separator = path.includes('\\') ? '\\' : '/';
  const segments = path.split(/[\\/]+/).filter(Boolean);
  if (segments.length <= keep + 1) return path;
  return `…${separator}${segments.slice(-keep).join(separator)}`;
}
