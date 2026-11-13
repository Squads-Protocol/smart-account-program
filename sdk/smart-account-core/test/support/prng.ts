// Small seeded PRNG (mulberry32) and a property loop that prints the seed on failure.
// Reproduce a failure with SDK_TEST_SEED=<seed> npm test.
import { type Address, getAddressDecoder } from '@solana/kit';

export interface Rng {
  readonly seed: number;
  u32(): number;
  int(maxExclusive: number): number;
  range(min: number, maxInclusive: number): number;
  bool(pTrue?: number): boolean;
  pick<T>(xs: readonly T[]): T;
  shuffle<T>(xs: readonly T[]): T[];
  bytes(n: number): Uint8Array;
  bigBits(bits: number): bigint;
  address(): Address;
}

export function rng(seed: number): Rng {
  let s = seed >>> 0;
  const u32 = () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return (t ^ (t >>> 14)) >>> 0;
  };
  const int = (n: number) => (n <= 1 ? 0 : u32() % n);
  const bytes = (n: number) => Uint8Array.from({ length: n }, () => u32() & 0xff);
  const dec = getAddressDecoder();
  const self: Rng = {
    seed,
    u32,
    int,
    range: (min, max) => min + int(max - min + 1),
    bool: (p = 0.5) => u32() / 2 ** 32 < p,
    pick: (xs) => {
      if (xs.length === 0) throw new Error('pick from empty');
      return xs[int(xs.length)]!;
    },
    shuffle: (xs) => {
      const a = [...xs];
      for (let i = a.length - 1; i > 0; i--) {
        const j = int(i + 1);
        [a[i], a[j]] = [a[j]!, a[i]!];
      }
      return a;
    },
    bytes,
    bigBits: (bits) => {
      let v = 0n;
      for (let i = 0; i < bits; i += 32) v = (v << 32n) | BigInt(u32());
      return v & ((1n << BigInt(bits)) - 1n);
    },
    address: () => dec.decode(bytes(32)),
  };
  return self;
}

export const baseSeed: number = process.env.SDK_TEST_SEED !== undefined
  ? Number(process.env.SDK_TEST_SEED) >>> 0
  : (Math.random() * 2 ** 32) >>> 0;

/** Derives a per-property seed so properties stay independent of each other's draw counts. */
export const seedFor = (name: string): number => {
  let h = baseSeed ^ 0x9e3779b9;
  for (const c of name) h = Math.imul(h ^ c.charCodeAt(0), 0x01000193) >>> 0;
  return h >>> 0;
};

/**
 * Runs `prop` `runs` times with a fresh case from `gen`. On the first failure it throws with the
 * property's seed, the run index and the case, so the failure reproduces.
 */
export async function forAll<C>(
  name: string,
  runs: number,
  gen: (r: Rng) => C | Promise<C>,
  prop: (c: C, r: Rng) => void | Promise<void>,
  describeCase: (c: C) => string = () => '',
): Promise<void> {
  const seed = seedFor(name);
  const r = rng(seed);
  for (let i = 0; i < runs; i++) {
    const c = await gen(r);
    try {
      await prop(c, r);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      throw new Error(
        `property "${name}" failed on run ${i}/${runs}; SDK_TEST_SEED=${baseSeed} (property seed ${seed})\n` +
          `case: ${describeCase(c)}\n${msg}`,
        { cause: e },
      );
    }
  }
}
