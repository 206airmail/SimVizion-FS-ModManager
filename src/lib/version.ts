// Mod version ordering as the update catalog does it (plan M7): drop a leading "v", split on
// ".", "-", "_" or "+", compare numerically, and treat missing trailing parts as 0.

function parts(version: string): string[] {
  return version
    .trim()
    .replace(/^[vV]\.?/, '')
    .split(/[.\-_+]/)
    .filter((part) => part !== '');
}

/** Negative when `a` is older than `b`, positive when newer, 0 when they are the same version. */
export function compareVersions(a: string, b: string): number {
  const left = parts(a);
  const right = parts(b);
  for (let i = 0; i < Math.max(left.length, right.length); i++) {
    const x = left[i] ?? '0';
    const y = right[i] ?? '0';
    if (x === y) continue;
    if (/^\d+$/.test(x) && /^\d+$/.test(y)) {
      const diff = Number(x) - Number(y);
      if (diff !== 0) return diff;
    } else {
      return x < y ? -1 : 1;
    }
  }
  return 0;
}

export const isOlder = (version: string, than: string): boolean =>
  compareVersions(version, than) < 0;
