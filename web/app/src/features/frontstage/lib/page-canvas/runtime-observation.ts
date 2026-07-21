export type FrontstageRuntimeObservationStage =
  | 'source_fetch'
  | 'worker_boot'
  | 'compile'
  | 'api_wait'
  | 'main'
  | 'schema_validate'
  | 'present';

export type FrontstageRuntimeObservationCacheTier =
  | 'network'
  | 'runtime'
  | 'l1'
  | 'l2'
  | 'miss';

export interface FrontstageRuntimeObservation {
  sequence: number;
  count: number;
  stage: FrontstageRuntimeObservationStage;
  timestampMs: number;
  durationMs: number;
  cacheTier: FrontstageRuntimeObservationCacheTier;
  actorId: string;
  workspaceId: string;
  pageId: string;
  tabId: string | null;
  blockId: string;
}

export type FrontstageRuntimeObservationInput = Omit<
  FrontstageRuntimeObservation,
  'sequence' | 'count' | 'timestampMs' | 'durationMs'
> & {
  timestampMs?: number;
  durationMs?: number;
};

export type FrontstageRuntimeObservationSubscriber = (
  observations: readonly FrontstageRuntimeObservation[]
) => void;

const DEFAULT_MAX_RUNTIME_OBSERVATIONS = 256;

export class FrontstageRuntimeObservationBuffer {
  readonly maxEntries: number;
  private readonly entries: FrontstageRuntimeObservation[] = [];
  private readonly stageCounts = new Map<
    FrontstageRuntimeObservationStage,
    number
  >();
  private readonly subscribers = new Set<
    FrontstageRuntimeObservationSubscriber
  >();
  private sequence = 0;

  constructor(maxEntries = DEFAULT_MAX_RUNTIME_OBSERVATIONS) {
    if (!Number.isSafeInteger(maxEntries) || maxEntries < 0) {
      throw new Error(
        'runtime observation max entries must be a non-negative integer'
      );
    }
    this.maxEntries = maxEntries;
  }

  record(input: FrontstageRuntimeObservationInput): void {
    const count = (this.stageCounts.get(input.stage) ?? 0) + 1;
    this.stageCounts.set(input.stage, count);
    this.sequence += 1;
    const observation: FrontstageRuntimeObservation = {
      sequence: this.sequence,
      count,
      stage: input.stage,
      timestampMs: normalizeMetric(input.timestampMs, Date.now()),
      durationMs: normalizeMetric(input.durationMs, 0),
      cacheTier: input.cacheTier,
      actorId: input.actorId,
      workspaceId: input.workspaceId,
      pageId: input.pageId,
      tabId: input.tabId,
      blockId: input.blockId
    };

    if (this.maxEntries > 0) {
      this.entries.push(observation);
      if (this.entries.length > this.maxEntries) {
        this.entries.splice(0, this.entries.length - this.maxEntries);
      }
    }
    this.notify();
  }

  read(): FrontstageRuntimeObservation[] {
    return this.entries.map((entry) => ({ ...entry }));
  }

  reset(): void {
    this.entries.length = 0;
    this.stageCounts.clear();
    this.sequence = 0;
    this.notify();
  }

  subscribe(subscriber: FrontstageRuntimeObservationSubscriber): () => void {
    this.subscribers.add(subscriber);
    return () => this.subscribers.delete(subscriber);
  }

  private notify(): void {
    if (this.subscribers.size === 0) {
      return;
    }
    const observations = this.read();
    for (const subscriber of this.subscribers) {
      subscriber(observations);
    }
  }
}

const runtimeObservations = new FrontstageRuntimeObservationBuffer();

export function recordFrontstageRuntimeObservation(
  input: FrontstageRuntimeObservationInput
): void {
  runtimeObservations.record(input);
}

export function readFrontstageRuntimeObservations(): FrontstageRuntimeObservation[] {
  return runtimeObservations.read();
}

export function resetFrontstageRuntimeObservations(): void {
  runtimeObservations.reset();
}

export function subscribeFrontstageRuntimeObservations(
  subscriber: FrontstageRuntimeObservationSubscriber
): () => void {
  return runtimeObservations.subscribe(subscriber);
}

function normalizeMetric(value: number | undefined, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0
    ? value
    : fallback;
}
