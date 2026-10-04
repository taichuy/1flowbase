/** These are cache accounting bytes, never an estimate of renderer RSS. */
export const RETENTION_SETTINGS_KEY = 'frontstage.retention.settings.v1';
// Used only when no heap limit is exposed. This resource-policy fallback is
// configurable per browser, not a page-count limit or a device RAM assumption.
export const FALLBACK_RETENTION_COST_BYTES = 64 * 1024 * 1024;
export const RETENTION_SAMPLE_MS = 15_000;

export type HeapSample = { used: number; limit: number };
export function readHeapSample(
  source: unknown = performance
): HeapSample | null {
  try {
    const memory = (
      source as {
        memory?: { usedJSHeapSize?: number; jsHeapSizeLimit?: number };
      }
    ).memory;
    const used = memory?.usedJSHeapSize;
    const limit = memory?.jsHeapSizeLimit;
    return typeof used === 'number' &&
      Number.isFinite(used) &&
      used >= 0 &&
      typeof limit === 'number' &&
      Number.isFinite(limit) &&
      limit > 0 &&
      used <= limit
      ? { used, limit }
      : null;
  } catch {
    return null;
  }
}

export function browserRetentionStorage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function readRetentionBudget(
  storage: Pick<Storage, 'getItem'> | null = browserRetentionStorage(),
  heap = readHeapSample()
): number {
  try {
    const value = JSON.parse(
      storage?.getItem(RETENTION_SETTINGS_KEY) ?? 'null'
    )?.costBudgetBytes;
    if (typeof value === 'number' && Number.isFinite(value) && value >= 0)
      return value;
  } catch {
    /* Disabled or corrupt preferences use the automatic policy. */
  }
  return heap ? Math.floor(heap.limit / 8) : FALLBACK_RETENTION_COST_BYTES;
}

/** One pressure reduction per high-water excursion, not a GC polling loop. */
export class RetentionPressure {
  private highWater = 0;
  budget(
    sample: HeapSample | null,
    configured: number,
    inactiveCost: number
  ): number {
    if (!sample) return configured;
    if (sample.used < sample.limit * 0.6) this.highWater = 0;
    if (
      sample.used < sample.limit * 0.7 ||
      (this.highWater > 0 && sample.used <= this.highWater * 1.05)
    )
      return configured;
    this.highWater = sample.used;
    return Math.min(configured, inactiveCost / 2);
  }
}

const encoder = new TextEncoder();
export function serializedCost(value: unknown): number {
  try {
    return encoder.encode(JSON.stringify(value) ?? '').byteLength;
  } catch {
    return 0;
  }
}

/**
 * Observable serialized markup cost, including this entry's shadow trees.
 * It deliberately excludes unknowable React state / native backing allocations.
 * Heap pressure is a separate, tab-wide signal for those costs. No node->KB rule.
 * Only an entry-owned wrapper is walked; no DOM reference survives this call.
 */
export function retainedMarkupCost(root: HTMLElement | null): number {
  if (!root) return 0;
  let bytes = encoder.encode(root.innerHTML).byteLength;
  const visit = (container: Element | ShadowRoot) => {
    for (const element of container.querySelectorAll('*')) {
      if (element.shadowRoot) {
        bytes += encoder.encode(element.shadowRoot.innerHTML).byteLength;
        visit(element.shadowRoot);
      }
    }
  };
  visit(root);
  return bytes;
}
