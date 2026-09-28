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
  flightKey
}: {
  source: string;
  requestId: string;
  moduleDefinitions: readonly NativeReactModuleDefinition[];
  workerFactory?: NativeReactBrowserCompilerWorkerFactory;
  signal?: AbortSignal;
  flightKey?: string;
}): Promise<NativeReactBrowserCompileResult> {
  if (!workerFactory) {
    defaultCompilerPool ??= new NativeReactBrowserCompilerPool(
      createNativeReactBrowserCompilerWorkerFactory()
    );
    return defaultCompilerPool.compile({
      source,
      requestId,
      moduleDefinitions,
      signal,
      flightKey
    });
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
}
interface CompilerConsumer {
  finish(result: NativeReactBrowserCompileResult): void;
}
interface CompilerJob {
  input: CompileInput;
  consumers: Set<CompilerConsumer>;
  worker?: NativeReactBrowserCompilerWorker;
}
let defaultCompilerPool: NativeReactBrowserCompilerPool | undefined;

/** A bounded pool with one request per Worker and cancellation owned by each consumer. */
export class NativeReactBrowserCompilerPool {
  private readonly workers = new Set<NativeReactBrowserCompilerWorker>();
  private readonly idle: NativeReactBrowserCompilerWorker[] = [];
  private readonly queue: CompilerJob[] = [];
  private readonly flights = new Map<string, CompilerJob>();
  private readonly running = new Map<
    NativeReactBrowserCompilerWorker,
    CompilerJob
  >();
  private serial = 0;
  private disposed = false;

  constructor(
    private readonly factory: NativeReactBrowserCompilerWorkerFactory,
    private readonly maxWorkers = 2
  ) {
    if (!Number.isSafeInteger(maxWorkers) || maxWorkers < 1)
      throw new Error('Compiler concurrency must be a positive integer.');
  }

  compile(input: CompileInput): Promise<NativeReactBrowserCompileResult> {
    if (input.signal?.aborted || this.disposed)
      return Promise.resolve(
        compilerFailure('Native React compilation cancelled.')
      );
    // The owner supplies a full scoped identity. Debug compilation never supplies a flight key.
    let job = input.flightKey ? this.flights.get(input.flightKey) : undefined;
    if (!job) {
      job = {
        input: {
          ...input,
          requestId: `${input.requestId}:worker:${++this.serial}`
        },
        consumers: new Set()
      };
      this.queue.push(job);
      if (input.flightKey) this.flights.set(input.flightKey, job);
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

  private pump() {
    while (
      this.queue.length &&
      (this.idle.length || this.workers.size < this.maxWorkers) &&
      !this.disposed
    ) {
      const job = this.queue.shift()!;
      if (!job.consumers.size) continue;
      let worker = this.idle.pop();
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
      if (reusable && !this.disposed) this.idle.push(worker);
      else this.retire(worker);
    }
    this.forget(job);
    for (const consumer of [...job.consumers]) consumer.finish(result);
  }

  private forget(job: CompilerJob) {
    if (job.input.flightKey && this.flights.get(job.input.flightKey) === job)
      this.flights.delete(job.input.flightKey);
    const index = this.queue.indexOf(job);
    if (index >= 0) this.queue.splice(index, 1);
  }

  private retire(worker: NativeReactBrowserCompilerWorker) {
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
