// Seeded pseudo-random numbers for the mock dataset. Never use Math.random() here: the data must
// be identical on every run so screenshots and tests are stable.

export interface Rng {
  /** Uniform in [0, 1). */
  next(): number;
  /** Uniform integer in [min, max], both inclusive. */
  int(min: number, max: number): number;
  chance(probability: number): boolean;
  pick<T>(items: readonly T[]): T;
  /** Standard normal deviate (Box-Muller). */
  gauss(): number;
  /** `count` distinct items, in random order. Fewer if the list is shorter. */
  sample<T>(items: readonly T[], count: number): T[];
}

/** mulberry32: a tiny 32-bit generator with good enough statistics for fake data. */
export function createRng(seed: number): Rng {
  let state = seed >>> 0;
  const next = (): number => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const int = (min: number, max: number): number => min + Math.floor(next() * (max - min + 1));
  const pick = <T>(items: readonly T[]): T => {
    const item = items[Math.floor(next() * items.length)];
    if (item === undefined) throw new Error('pick from an empty list');
    return item;
  };
  return {
    next,
    int,
    chance: (probability) => next() < probability,
    pick,
    gauss: () => Math.sqrt(-2 * Math.log(1 - next())) * Math.cos(2 * Math.PI * next()),
    sample: (items, count) => {
      const copy = items.slice();
      const taken = Math.min(count, copy.length);
      for (let i = 0; i < taken; i++) {
        const j = int(i, copy.length - 1);
        const a = copy[i] as (typeof copy)[number];
        copy[i] = copy[j] as (typeof copy)[number];
        copy[j] = a;
      }
      return copy.slice(0, taken);
    },
  };
}

/** FNV-1a, for stable per-name values such as icon hues. */
export function hashString(text: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}
