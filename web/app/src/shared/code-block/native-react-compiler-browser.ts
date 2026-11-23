import {
  canonicalizeNativeReactComponentArtifact,
  createNativeReactComponentArtifactIdentity,
  nativeReactComponentArtifactMatchesIdentity,
  sha256Text,
  type NativeReactCompileDiagnostic,
  type NativeReactCompilerRequest,
  type NativeReactCompilerResponse,
  type NativeReactComponentArtifact,
  type NativeReactModuleDefinition
} from '@1flowbase/page-runtime/browser';

import { resolveRuntimeConcurrencyProfile } from '../utils/runtime-concurrency-profile';
import nativeReactCompilerWorkerUrl from './native-react-compiler.worker?worker&url';

export const NATIVE_REACT_COMPILER_WORKER_NAME =
  'native-react-component-compiler';

export interface NativeReactBrowserCompilerWorker {
  onmessage: ((event: MessageEvent<unknown>) => void) | null;
  onerror: ((event: ErrorEvent) => void) | null;
  postMessage(message: NativeReactCompilerRequest): void;
  terminate(): void;
}

export type NativeReactBrowserCompilerWorkerConstructor = new (
  scriptUrl: string | URL,
  options?: WorkerOptions
) => NativeReactBrowserCompilerWorker;

export type NativeReactBrowserCompilerWorkerFactory =
  () => NativeReactBrowserCompilerWorker;

export type NativeReactBrowserCompileResult =
  | { ok: true; artifact: NativeReactComponentArtifact; diagnostics: [] }
  | { ok: false; diagnostics: NativeReactCompileDiagnostic[] };

export function getNativeReactCompilerWorkerUrl(): string {
  return nativeReactCompilerWorkerUrl;
}

export function createNativeReactBrowserCompilerWorkerFactory({
  workerConstructor = globalThis.Worker as NativeReactBrowserCompilerWorkerConstructor,
  workerUrl = getNativeReactCompilerWorkerUrl()
}: {
  workerConstructor?: NativeReactBrowserCompilerWorkerConstructor;
  workerUrl?: string | URL;
} = {}): NativeReactBrowserCompilerWorkerFactory {
  if (typeof workerConstructor !== 'function') {
    throw new Error('Native React compiler Worker is unavailable.');
  }
  return () =>
    new workerConstructor(workerUrl, {
      type: 'module',
      name: NATIVE_REACT_COMPILER_WORKER_NAME
    });
}

export function compileNativeReactComponentInBrowser({
  source,
  requestId,
  moduleDefinitions,
  signal,
  workerFactory,
  flightKey,
  priority,
  slotIndex
}: {
  source: string;
  requestId: string;
  moduleDefinitions: readonly NativeReactModuleDefinition[];
  workerFactory?: NativeReactBrowserCompilerWorkerFactory;
  signal?: AbortSignal;
  flightKey?: string;
  priority?: number;
  slotIndex?: number;
}): Promise<NativeReactBrowserCompileResult> {
  if (!workerFactory) {
    return ensureDefaultNativeReactBrowserCompilerPool().then((pool) =>
      pool.compile({
        source,
        requestId,
        moduleDefinitions,
        signal,
        flightKey,
        priority,
        slotIndex
      })
    );
  }
  return new Promise((resolve) => {
    if (signal?.aborted) {
      resolve(compilerFailure('Native React compilation cancelled.'));
      return;
    }
    let worker: NativeReactBrowserCompilerWorker;
    try {
      worker = workerFactory();
    } catch (error) {
      resolve(compilerFailure(errorMessage(error)));
      return;
    }

    let settled = false;
    const cancel = () =>
      finish(compilerFailure('Native React compilation cancelled.'));
    const finish = (result: NativeReactBrowserCompileResult) => {
      if (settled) return;
      settled = true;
      signal?.removeEventListener('abort', cancel);
      worker.onmessage = null;
      worker.onerror = null;
      worker.terminate();
      resolve(result);
    };
    signal?.addEventListener('abort', cancel, { once: true });
    worker.onmessage = (event) => {
      finish(readCompilerResponse(event.data, requestId, source));
    };
    worker.onerror = (event) => {
      finish(
        compilerFailure(event.message || 'Native React compiler Worker failed.')
      );
    };
    try {
      worker.postMessage({
        direction: 'host_to_worker',
        type: 'compile_native_react_component',
        requestId,
        source,
        moduleDefinitions: [...moduleDefinitions]
      });
    } catch (error) {
      finish(compilerFailure(errorMessage(error)));
    }
  });
}

interface CompileInput {
  source: string;
  requestId: string;
  moduleDefinitions: readonly NativeReactModuleDefinition[];
  signal?: AbortSignal;
  /** Scoped source + compiler/module policy identity. Omit for a fresh debug compilation. */
  flightKey?: string;
  /** Viewport demand priority (`0` selected, `1` visible, `2` near-viewport preload). Lower runs first. */
  priority?: number;
  slotIndex?: number;
}
interface CompilerConsumer {
  finish(result: NativeReactBrowserCompileResult): void;
}
interface CompilerJob {
  input: CompileInput;
  priority: number;
  slotIndex: number;
  order: number;
  consumers: Set<CompilerConsumer>;
  worker?: NativeReactBrowserCompilerWorker;
}
let defaultCompilerPool: NativeReactBrowserCompilerPool | undefined;
let defaultCompilerPoolFlight: Promise<NativeReactBrowserCompilerPool> | undefined;
let resolvedWorkerBlobUrl: string | undefined;

export function promoteDefaultNativeReactCompilerFlight(
  flightKey: string,
  priority: number
): void {
  defaultCompilerPool?.promote(flightKey, priority);
}

async function resolveSingleFetchWorkerScriptUrl(): Promise<{
  url: string;
  inMemoryBlob: boolean;
}> {
  if (resolvedWorkerBlobUrl) {
    return { url: resolvedWorkerBlobUrl, inMemoryBlob: true };
  }
  const canonicalUrl = getNativeReactCompilerWorkerUrl();
  const profile = resolveRuntimeConcurrencyProfile();
  if (
    profile.isDev ||
    import.meta.env.VITEST ||
    typeof globalThis.fetch !== 'function' ||
    typeof URL?.createObjectURL !== 'function' ||
    typeof Blob !== 'function'
  ) {
    return { url: canonicalUrl, inMemoryBlob: false };
  }
  try {
    const response = await globalThis.fetch(canonicalUrl, {
      credentials: 'same-origin'
    });
    if (!response.ok) {
      return { url: canonicalUrl, inMemoryBlob: false };
    }
    const scriptText = await response.text();
    if (!scriptText) {
      return { url: canonicalUrl, inMemoryBlob: false };
    }
    resolvedWorkerBlobUrl = URL.createObjectURL(
      new Blob([scriptText], { type: 'text/javascript' })
    );
    return { url: resolvedWorkerBlobUrl, inMemoryBlob: true };
  } catch {
    return { url: canonicalUrl, inMemoryBlob: false };
  }
}

function ensureDefaultNativeReactBrowserCompilerPool(): Promise<NativeReactBrowserCompilerPool> {
  if (defaultCompilerPool) {
    return Promise.resolve(defaultCompilerPool);
  }
  defaultCompilerPoolFlight ??= resolveSingleFetchWorkerScriptUrl().then(
    ({ url, inMemoryBlob }) => {
      if (defaultCompilerPool) return defaultCompilerPool;
      const profile = resolveRuntimeConcurrencyProfile();
      defaultCompilerPool = new NativeReactBrowserCompilerPool(
        createNativeReactBrowserCompilerWorkerFactory({ workerUrl: url }),
        profile.compilerMaxWorkers,
        {
          cacheGateInitialWorker:
            !inMemoryBlob && profile.compilerCacheGateInitialWorker
        }
      );
      return defaultCompilerPool;
    }
  );
  return defaultCompilerPoolFlight;
}

export function warmDefaultNativeReactCompilerWorker(): void {
  if (typeof globalThis.Worker !== 'function') return;
  void ensureDefaultNativeReactBrowserCompilerPool().then((pool) => {
    pool.warm();
  });
}

export interface NativeReactBrowserCompilerPoolOptions {
  /**
   * When true and `maxWorkers > 1`, cold start spawns only 1 lead Worker until
   * either `markScriptCacheWarm()` is called or the lead Worker completes its
   * first compile, preventing duplicate cold-cache script downloads in Chromium.
   */
  cacheGateInitialWorker?: boolean;
  /**
   * Idle lifetime in ms for burst subordinate Workers (`workers.size > 1`).
   * Keeps at most 1 resident warm Worker so subsequent single-block edits compile
   * with zero Worker boot cost while reclaiming peak multi-core Workers when idle.
   */
  idleWorkerTtlMs?: number;
}

const DEFAULT_IDLE_SUBORDINATE_WORKER_TTL_MS = 15_000;

/** A bounded pool with one request per Worker and cancellation owned by each consumer. */
export class NativeReactBrowserCompilerPool {
  private readonly workers = new Set<NativeReactBrowserCompilerWorker>();
  private readonly idle: NativeReactBrowserCompilerWorker[] = [];
  private readonly idleTimers = new Map<
    NativeReactBrowserCompilerWorker,
    ReturnType<typeof setTimeout>
  >();
  private readonly queue: CompilerJob[] = [];
  private readonly flights = new Map<string, CompilerJob>();
  private readonly running = new Map<
    NativeReactBrowserCompilerWorker,
    CompilerJob
  >();
  private serial = 0;
  private disposed = false;
  private scriptCacheWarm: boolean;
  private readonly idleWorkerTtlMs: number;

  constructor(
    private readonly factory: NativeReactBrowserCompilerWorkerFactory,
    private readonly maxWorkers = resolveRuntimeConcurrencyProfile()
      .compilerMaxWorkers,
    options: NativeReactBrowserCompilerPoolOptions = {}
  ) {
    if (!Number.isSafeInteger(maxWorkers) || maxWorkers < 1)
      throw new Error('Compiler concurrency must be a positive integer.');
    this.scriptCacheWarm = !(
      (options.cacheGateInitialWorker ?? false) && maxWorkers > 1
    );
    this.idleWorkerTtlMs =
      options.idleWorkerTtlMs ?? DEFAULT_IDLE_SUBORDINATE_WORKER_TTL_MS;
  }

  markScriptCacheWarm(): void {
    if (this.disposed || this.scriptCacheWarm) return;
    this.scriptCacheWarm = true;
    this.pump();
  }

  warm(): void {
    if (this.disposed || this.workers.size > 0) return;
    try {
      const worker = this.factory();
      this.workers.add(worker);
      this.idle.push(worker);
    } catch {
      // Ignore prewarm failures; compile() surfaces actionable diagnostics on demand.
    }
  }

  compile(input: CompileInput): Promise<NativeReactBrowserCompileResult> {
    if (input.signal?.aborted || this.disposed)
      return Promise.resolve(
        compilerFailure('Native React compilation cancelled.')
      );
    const nextPriority = input.priority ?? 1;
    const nextSlotIndex = input.slotIndex ?? 0;
    // The owner supplies a full scoped identity. Debug compilation never supplies a flight key.
    let job = input.flightKey ? this.flights.get(input.flightKey) : undefined;
    if (!job) {
      const order = ++this.serial;
      job = {
        input: {
          ...input,
          requestId: `${input.requestId}:worker:${order}`
        },
        priority: nextPriority,
        slotIndex: nextSlotIndex,
        order,
        consumers: new Set()
      };
      this.enqueueSorted(job);
      if (input.flightKey) this.flights.set(input.flightKey, job);
    } else if (!job.worker && nextPriority < job.priority) {
      job.priority = nextPriority;
      job.slotIndex = Math.min(job.slotIndex, nextSlotIndex);
      this.resortQueue();
    }
    const current = job;
    const promise = new Promise<NativeReactBrowserCompileResult>((resolve) => {
      const consumer: CompilerConsumer = {
        finish: (result) => {
          input.signal?.removeEventListener('abort', cancel);
          current.consumers.delete(consumer);
          resolve(result);
        }
      };
      const cancel = () => {
        consumer.finish(compilerFailure('Native React compilation cancelled.'));
        if (!current.consumers.size) {
          if (current.worker) this.retire(current.worker);
          this.forget(current);
          this.pump();
        }
      };
      current.consumers.add(consumer);
      input.signal?.addEventListener('abort', cancel, { once: true });
    });
    this.pump();
    return promise;
  }

  promote(flightKey: string, priority: number): void {
    if (this.disposed) return;
    const job = this.flights.get(flightKey);
    if (!job || job.worker || priority >= job.priority) return;
    job.priority = priority;
    this.resortQueue();
  }

  private enqueueSorted(job: CompilerJob): void {
    this.queue.push(job);
    this.resortQueue();
  }

  private resortQueue(): void {
    if (this.queue.length <= 1) return;
    this.queue.sort(
      (left, right) =>
        left.priority - right.priority ||
        left.slotIndex - right.slotIndex ||
        left.order - right.order
    );
  }

  dispose(): void {
    this.disposed = true;
    const jobs = new Set([...this.queue, ...this.running.values()]);
    for (const job of jobs)
      this.complete(
        job,
        compilerFailure('Native React compilation cancelled.'),
        false
      );
    for (const worker of this.workers) this.retire(worker);
  }

  private effectiveMaxWorkers(): number {
    return this.scriptCacheWarm ? this.maxWorkers : 1;
  }

  private pump() {
    while (
      this.queue.length &&
      (this.idle.length || this.workers.size < this.effectiveMaxWorkers()) &&
      !this.disposed
    ) {
      const job = this.queue.shift()!;
      if (!job.consumers.size) continue;
      let worker = this.idle.pop();
      if (worker) this.clearIdleTimer(worker);
      try {
        worker ??= this.factory();
      } catch (error) {
        this.complete(job, compilerFailure(errorMessage(error)), false);
        continue;
      }
      this.workers.add(worker);
      this.running.set(worker, job);
      job.worker = worker;
      const activeWorker = worker;
      worker.onmessage = (event) => {
        // Ignore replies from a previous job/consumer; they cannot settle another generation.
        if (this.running.get(activeWorker) !== job) return;
        const result = readCompilerResponse(
          event.data,
          job.input.requestId,
          job.input.source
        );
        const validResponse =
          isCompilerResponse(event.data) &&
          event.data.requestId === job.input.requestId;
        if (validResponse) {
          this.scriptCacheWarm = true;
        }
        this.complete(job, result, validResponse);
        this.pump();
      };
      worker.onerror = (event) => {
        if (this.running.get(activeWorker) !== job) return;
        this.complete(
          job,
          compilerFailure(
            event.message || 'Native React compiler Worker failed.'
          ),
          false
        );
        this.pump();
      };
      try {
        worker.postMessage({
          direction: 'host_to_worker',
          type: 'compile_native_react_component',
          requestId: job.input.requestId,
          source: job.input.source,
          moduleDefinitions: [...job.input.moduleDefinitions]
        });
      } catch (error) {
        this.complete(job, compilerFailure(errorMessage(error)), false);
      }
    }
  }

  private complete(
    job: CompilerJob,
    result: NativeReactBrowserCompileResult,
    reusable: boolean
  ) {
    if (job.worker) {
      const worker = job.worker;
      worker.onmessage = worker.onerror = null;
      this.running.delete(worker);
      if (reusable && !this.disposed) {
        this.idle.push(worker);
        this.scheduleIdleReclamation(worker);
      } else {
        this.retire(worker);
      }
    }
    this.forget(job);
    for (const consumer of [...job.consumers]) consumer.finish(result);
  }

  private scheduleIdleReclamation(worker: NativeReactBrowserCompilerWorker) {
    this.clearIdleTimer(worker);
    if (this.maxWorkers <= 1 || this.idleWorkerTtlMs <= 0) return;
    const timer = setTimeout(() => {
      this.idleTimers.delete(worker);
      if (this.disposed || !this.idle.includes(worker)) return;
      // Retain at most 1 resident warm Worker; reclaim idle burst subordinate Workers.
      if (this.workers.size <= 1) return;
      this.retire(worker);
    }, this.idleWorkerTtlMs);
    (timer as { unref?: () => void }).unref?.();
    this.idleTimers.set(worker, timer);
  }

  private clearIdleTimer(worker: NativeReactBrowserCompilerWorker) {
    const timer = this.idleTimers.get(worker);
    if (timer !== undefined) {
      clearTimeout(timer);
      this.idleTimers.delete(worker);
    }
  }

  private forget(job: CompilerJob) {
    if (job.input.flightKey && this.flights.get(job.input.flightKey) === job)
      this.flights.delete(job.input.flightKey);
    const index = this.queue.indexOf(job);
    if (index >= 0) this.queue.splice(index, 1);
  }

  private retire(worker: NativeReactBrowserCompilerWorker) {
    this.clearIdleTimer(worker);
    worker.onmessage = worker.onerror = null;
    worker.terminate();
    this.workers.delete(worker);
    this.running.delete(worker);
    const index = this.idle.indexOf(worker);
    if (index >= 0) this.idle.splice(index, 1);
  }
}

function readCompilerResponse(
  value: unknown,
  requestId: string,
  source: string
): NativeReactBrowserCompileResult {
  if (!isCompilerResponse(value) || value.requestId !== requestId) {
    return compilerFailure('Native React compiler response is invalid.');
  }
  if (value.type === 'native_react_component_compile_failed') {
    return { ok: false, diagnostics: value.diagnostics };
  }
  const artifact = canonicalizeNativeReactComponentArtifact(value.artifact);
  const expectedIdentity = createNativeReactComponentArtifactIdentity({
    sourceSha256: sha256Text(source)
  });
  return artifact &&
    nativeReactComponentArtifactMatchesIdentity(artifact, expectedIdentity)
    ? { ok: true, artifact, diagnostics: [] }
    : compilerFailure('Native React compiler artifact is invalid.');
}

function isCompilerResponse(
  value: unknown
): value is NativeReactCompilerResponse {
  if (!isRecord(value) || value.direction !== 'worker_to_host') return false;
  if (
    value.type === 'native_react_component_compiled' &&
    typeof value.requestId === 'string'
  ) {
    return 'artifact' in value;
  }
  return (
    value.type === 'native_react_component_compile_failed' &&
    typeof value.requestId === 'string' &&
    Array.isArray(value.diagnostics)
  );
}

function compilerFailure(message: string): NativeReactBrowserCompileResult {
  return {
    ok: false,
    diagnostics: [
      {
        phase: 'compile',
        code: 'transform_failed',
        path: 'worker',
        message
      }
    ]
  };
}

function errorMessage(error: unknown): string {
  return error instanceof Error && error.message
    ? error.message
    : 'Native React compiler Worker failed.';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

if (import.meta.hot)
  import.meta.hot.dispose(() => defaultCompilerPool?.dispose());
