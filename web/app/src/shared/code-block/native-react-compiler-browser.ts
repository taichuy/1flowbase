import {
  canonicalizeNativeReactComponentArtifact,
  sha256Text,
  type NativeReactCompileDiagnostic,
  type NativeReactCompilerRequest,
  type NativeReactCompilerResponse,
  type NativeReactComponentArtifact,
  type NativeReactCatalogDependencyLock
} from '@1flowbase/page-runtime';

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
  dependencyLock = [],
  workerFactory = createNativeReactBrowserCompilerWorkerFactory()
}: {
  source: string;
  requestId: string;
  dependencyLock?: NativeReactCatalogDependencyLock;
  workerFactory?: NativeReactBrowserCompilerWorkerFactory;
}): Promise<NativeReactBrowserCompileResult> {
  return new Promise((resolve) => {
    let worker: NativeReactBrowserCompilerWorker;
    try {
      worker = workerFactory();
    } catch (error) {
      resolve(compilerFailure(errorMessage(error)));
      return;
    }

    const finish = (result: NativeReactBrowserCompileResult) => {
      worker.onmessage = null;
      worker.onerror = null;
      worker.terminate();
      resolve(result);
    };
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
        dependencyLock
      });
    } catch (error) {
      finish(compilerFailure(errorMessage(error)));
    }
  });
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
  return artifact && artifact.sourceSha256 === sha256Text(source)
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
