/** Deterministic PRNG (mulberry32). Returns a function yielding floats in [0, 1). */
export function makeRng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Pick a deterministic element from a non-empty array. */
export function pick<T>(rng: () => number, xs: readonly T[]): T {
  return xs[Math.floor(rng() * xs.length)];
}

/** Build a string of `n` random decimal digits. */
export function digits(rng: () => number, n: number): string {
  let out = '';
  for (let i = 0; i < n; i++) {
    out += Math.floor(rng() * 10).toString();
  }
  return out;
}

/** Derive a stable sub-seed from a base seed and an index. */
export function subSeed(seed: number, index: number): number {
  return (Math.imul(seed ^ 0x9e3779b9, 0x85ebca6b) + index * 0x27d4eb2f) >>> 0;
}
