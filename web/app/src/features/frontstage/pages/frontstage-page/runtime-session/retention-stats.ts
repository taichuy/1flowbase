const DEFAULT_BYTE_BUDGET = 64 * 1024;
const ROWS = 4;
const MAX_COUNT = 15;
const VERSION = 1;

function hash(value: string, seed: number): number {
  let result = seed >>> 0;
  for (let index = 0; index < value.length; index += 1) {
    result = Math.imul(result ^ value.charCodeAt(index), 16777619);
  }
  result ^= result >>> 16;
  return result >>> 0;
}

function browserStorage(): Pick<Storage, 'getItem' | 'setItem'> | null {
  try {
    return typeof window === 'undefined' ? null : window.localStorage;
  } catch {
    return null;
  }
}

/**
 * TinyLFU frequency estimator: bounded Count-Min sketch, aged by accesses.
 * Only numeric counters persist; page keys, content and session secrets do not.
 * Frequencies are approximate (hash collisions can overestimate a cold key).
 */
export class RetentionStats {
  private readonly storage: Pick<Storage, 'getItem' | 'setItem'> | null;
  private readonly storageKey: string;
  private readonly byteBudget: number;
  private readonly width: number;
  private readonly counters: Uint8Array;
  private readonly seen: Uint8Array;
  private distinctSeen = 0;
  private accesses = 0;
  private dirty = false;

  constructor({
    scope,
    storage = browserStorage(),
    byteBudget = DEFAULT_BYTE_BUDGET
  }: {
    scope: string;
    storage?: Pick<Storage, 'getItem' | 'setItem'> | null;
    byteBudget?: number;
  }) {
    this.storage = storage;
    this.byteBudget = Number.isFinite(byteBudget)
      ? Math.max(0, Math.floor(byteBudget))
      : DEFAULT_BYTE_BUDGET;
    // JSON counters require at most three ASCII bytes each, including commas.
    // Reserve header space and use a power of two for the sketch index mask.
    const seenBytes = Math.max(
      1,
      Math.min(128, Math.floor(this.byteBudget / 16))
    );
    this.seen = new Uint8Array(seenBytes);
    const maxWidth = Math.max(
      1,
      Math.floor((this.byteBudget - 256 - seenBytes * 4) / (ROWS * 3))
    );
    this.width = 2 ** Math.floor(Math.log2(Math.min(4096, maxWidth)));
    this.counters = new Uint8Array(this.width * ROWS);
    this.storageKey = `frontstage:retention-stats:v${VERSION}:${hash(scope, 2166136261).toString(16)}:${hash(scope, 3335557771).toString(16)}`;
    this.restore();
  }

  record(key: string): void {
    // A bounded bitmap estimates the working set in this access era. Aging
    // scales with that footprint, not the maximum allocated sketch dimensions.
    const seenIndex = hash(key, 2246822519) % (this.seen.length * 8);
    const byte = seenIndex >>> 3;
    const bit = 1 << (seenIndex & 7);
    if (!(this.seen[byte] & bit)) {
      this.seen[byte] |= bit;
      this.distinctSeen += 1;
    }
    for (const index of this.indices(key)) {
      if (this.counters[index] < MAX_COUNT) this.counters[index] += 1;
    }
    this.accesses += 1;
    if (this.accesses >= this.agingThreshold()) {
      for (let index = 0; index < this.counters.length; index += 1) {
        this.counters[index] >>= 1;
      }
      this.accesses = 0;
      this.seen.fill(0);
      this.distinctSeen = 0;
    }
    this.dirty = true;
  }

  frequency(key: string): number {
    return Math.min(...this.indices(key).map((index) => this.counters[index]));
  }

  flush(): void {
    if (!this.storage || !this.dirty) return;
    const serialized = JSON.stringify({
      version: VERSION,
      width: this.width,
      accesses: this.accesses,
      counters: Array.from(this.counters),
      seen: Array.from(this.seen)
    });
    // All serialized characters are ASCII, so length equals UTF-8 byte size.
    if (serialized.length > this.byteBudget) return;
    try {
      this.storage.setItem(this.storageKey, serialized);
      this.dirty = false;
    } catch {
      // Storage may be denied or full. In-memory admission remains available.
    }
  }

  private agingThreshold(): number {
    return Math.max(20, this.distinctSeen * 10);
  }

  private indices(key: string): number[] {
    return Array.from(
      { length: ROWS },
      (_, row) =>
        row * this.width +
        (hash(key, 2166136261 + row * 374761393) & (this.width - 1))
    );
  }

  private restore(): void {
    if (!this.storage) return;
    try {
      const serialized = this.storage.getItem(this.storageKey);
      if (!serialized || serialized.length > this.byteBudget) return;
      const data: unknown = JSON.parse(serialized);
      if (!data || typeof data !== 'object') return;
      const saved = data as Record<string, unknown>;
      if (
        saved.version !== VERSION ||
        saved.width !== this.width ||
        typeof saved.accesses !== 'number' ||
        !Number.isInteger(saved.accesses) ||
        saved.accesses < 0 ||
        !Array.isArray(saved.seen) ||
        saved.seen.length !== this.seen.length ||
        !saved.seen.every(
          (value: unknown) =>
            typeof value === 'number' &&
            Number.isInteger(value) &&
            value >= 0 &&
            value <= 255
        ) ||
        !Array.isArray(saved.counters) ||
        saved.counters.length !== this.counters.length ||
        !saved.counters.every(
          (value: unknown) =>
            typeof value === 'number' &&
            Number.isInteger(value) &&
            value >= 0 &&
            value <= MAX_COUNT
        )
      )
        return;
      const distinctSeen = saved.seen.reduce((total: number, value: number) => {
        let bits = value;
        while (bits) {
          total += bits & 1;
          bits >>>= 1;
        }
        return total;
      }, 0);
      if (
        saved.accesses >= Math.max(20, distinctSeen * 10) ||
        distinctSeen > saved.accesses
      )
        return;
      this.counters.set(saved.counters);
      this.seen.set(saved.seen);
      this.distinctSeen = distinctSeen;
      this.accesses = saved.accesses;
    } catch {
      // A corrupt or unavailable metadata record is equivalent to a cold sketch.
    }
  }
}
