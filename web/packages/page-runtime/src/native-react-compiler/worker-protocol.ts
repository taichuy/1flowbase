import {
  compileNativeReactComponent,
  type NativeReactCompileDiagnostic,
  type NativeReactComponentArtifact
} from './artifact';
import type { NativeReactCatalogDependencyLock } from './module-registry/contracts';

export interface NativeReactCompilerRequest {
  direction: 'host_to_worker';
  type: 'compile_native_react_component';
  requestId: string;
  source: string;
  dependencyLock: NativeReactCatalogDependencyLock;
}

export type NativeReactCompilerResponse =
  | {
      direction: 'worker_to_host';
      type: 'native_react_component_compiled';
      requestId: string;
      artifact: NativeReactComponentArtifact;
      diagnostics: [];
    }
  | {
      direction: 'worker_to_host';
      type: 'native_react_component_compile_failed';
      requestId: string;
      diagnostics: NativeReactCompileDiagnostic[];
    };

export interface NativeReactCompilerWorkerScope {
  onmessage: ((event: { data: unknown }) => void) | null;
  postMessage(message: NativeReactCompilerResponse): void;
  close(): void;
}

export function handleNativeReactCompilerRequest(
  value: unknown
): NativeReactCompilerResponse {
  const request = readRequest(value);
  if (!request) {
    return {
      direction: 'worker_to_host',
      type: 'native_react_component_compile_failed',
      requestId: readRequestId(value),
      diagnostics: [
        {
          phase: 'compile',
          code: 'transform_failed',
          path: 'message',
          message: 'Native React compiler request is invalid.'
        }
      ]
    };
  }

  const result = compileNativeReactComponent(
    request.source,
    request.dependencyLock
  );
  return result.ok
    ? {
        direction: 'worker_to_host',
        type: 'native_react_component_compiled',
        requestId: request.requestId,
        artifact: result.artifact,
        diagnostics: []
      }
    : {
        direction: 'worker_to_host',
        type: 'native_react_component_compile_failed',
        requestId: request.requestId,
        diagnostics: result.diagnostics
      };
}

/** Attaches the one-shot compiler Worker. Component code is never evaluated here. */
export function attachNativeReactCompilerWorker(
  scope: NativeReactCompilerWorkerScope
): void {
  scope.onmessage = (event) => {
    scope.onmessage = null;
    try {
      scope.postMessage(handleNativeReactCompilerRequest(event.data));
    } finally {
      scope.close();
    }
  };
}

function readRequest(value: unknown): NativeReactCompilerRequest | null {
  if (!isRecord(value)) return null;
  return value.direction === 'host_to_worker' &&
    value.type === 'compile_native_react_component' &&
    isNonEmptyString(value.requestId) &&
    typeof value.source === 'string' &&
    (value.dependencyLock === undefined || Array.isArray(value.dependencyLock))
    ? {
        direction: value.direction,
        type: value.type,
        requestId: value.requestId,
        source: value.source,
        dependencyLock: (value.dependencyLock ??
          []) as NativeReactCatalogDependencyLock
      }
    : null;
}

function readRequestId(value: unknown): string {
  return isRecord(value) && isNonEmptyString(value.requestId)
    ? value.requestId
    : 'unknown';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}
