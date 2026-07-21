import type {
  BlockContext,
  BlockApiMethod,
  BlockApiRequest,
  BlockContextEntity,
  BlockContextIdentity,
  BlockContextPage,
  BlockContextRecord,
  BlockContextTheme,
  BlockContextUi
} from '@1flowbase/page-protocol';
import { isBlockResult } from '@1flowbase/block-sdk';

import {
  evaluateJsBlockSource,
  mapJsBlockRuntimeSourceLocation,
  type JsBlockInjectedModuleMap
} from './js-block-source-evaluator';
import type {
  JsBlockHostToWorkerMessage,
  JsBlockRunError,
  JsBlockRunRequest,
  JsBlockWorkerEventRequestMessage,
  JsBlockWorkerInterfaceRequestMessage,
  JsBlockWorkerEffectResultMessage,
  JsBlockWorkerToHostMessage
} from './js-block-worker-runtime';

export interface JsBlockWorkerExecutorOptions {
  modules: JsBlockInjectedModuleMap;
  postMessage?: (message: JsBlockWorkerToHostMessage) => void;
}

export interface JsBlockWorkerExecutor {
  handleMessage(message: unknown): Promise<JsBlockWorkerToHostMessage[]>;
  dispose(): void;
}

export interface JsBlockWorkerRuntimeScope {
  postMessage(message: JsBlockWorkerToHostMessage): void;
  addEventListener?: (
    type: 'message',
    listener: (event: { data: unknown }) => void
  ) => void;
  removeEventListener?: (
    type: 'message',
    listener: (event: { data: unknown }) => void
  ) => void;
  onmessage?: ((event: { data: unknown }) => void) | null;
}

export interface AttachedJsBlockWorkerRuntime {
  executor: JsBlockWorkerExecutor;
  flush(): Promise<void>;
  dispose(): void;
}

type MutableBlockContext = BlockContext & {
  state: BlockContextRecord;
};

interface PendingEffect {
  requestId: string;
  resolve(value: unknown): void;
  reject(error: Error): void;
}

class JsBlockWorkerEffectError extends Error {
  readonly error: JsBlockRunError;

  constructor(error: JsBlockRunError) {
    super(error.message);
    this.name = 'JsBlockWorkerEffectError';
    this.error = error;
  }
}

export function createJsBlockWorkerExecutor(
  options: JsBlockWorkerExecutorOptions
): JsBlockWorkerExecutor {
  let disposed = false;
  let nextEffectIndex = 1;
  const pendingEffects = new Map<string, PendingEffect>();

  const dispatch = (
    output: JsBlockWorkerToHostMessage[],
    message: JsBlockWorkerToHostMessage
  ) => {
    if (disposed) {
      return;
    }

    output.push(message);
    options.postMessage?.(message);
  };

  const disposeExecutor = () => {
    if (disposed) {
      return;
    }

    disposed = true;
    for (const pendingEffect of pendingEffects.values()) {
      pendingEffect.reject(new Error('JS block worker runtime disposed.'));
    }
    pendingEffects.clear();
  };

  const settleEffect = (message: JsBlockWorkerEffectResultMessage) => {
    const pendingEffect = pendingEffects.get(message.effectId);
    if (!pendingEffect || pendingEffect.requestId !== message.requestId) {
      return;
    }

    pendingEffects.delete(message.effectId);
    if (message.ok) {
      pendingEffect.resolve(message.value);
      return;
    }

    pendingEffect.reject(new JsBlockWorkerEffectError(message.error));
  };

  const createEffectId = (requestId: string): string => {
    const effectId = `${requestId}:effect-${nextEffectIndex}`;
    nextEffectIndex += 1;
    return effectId;
  };

  return {
    async handleMessage(message) {
      const output: JsBlockWorkerToHostMessage[] = [];
      const hostMessage = normalizeHostMessage(message);
      if (!hostMessage || disposed) {
        return output;
      }

      if (hostMessage.type === 'init') {
        dispatch(output, {
          direction: 'worker_to_host',
          type: 'ready'
        });
        return output;
      }

      if (hostMessage.type === 'dispose') {
        disposeExecutor();
        return output;
      }

      if (hostMessage.type === 'timeout') {
        return output;
      }

      if (hostMessage.type === 'effect_result') {
        settleEffect(hostMessage);
        return output;
      }

      await runRequest(
        hostMessage.request,
        options.modules,
        (nextMessage) => dispatch(output, nextMessage),
        createEffectId,
        pendingEffects
      );
      return output;
    },
    dispose() {
      disposeExecutor();
    }
  };
}

export function attachJsBlockWorkerRuntime(
  scope: JsBlockWorkerRuntimeScope,
  options: Omit<JsBlockWorkerExecutorOptions, 'postMessage'>
): AttachedJsBlockWorkerRuntime {
  const executor = createJsBlockWorkerExecutor({
    ...options,
    postMessage: (message) => scope.postMessage(message)
  });
  const pendingTasks = new Set<Promise<unknown>>();
  const listener = (event: { data: unknown }) => {
    const task = executor.handleMessage(event.data);
    pendingTasks.add(task);
    task.finally(() => pendingTasks.delete(task));
  };

  if (scope.addEventListener) {
    scope.addEventListener('message', listener);
  } else {
    scope.onmessage = listener;
  }

  return {
    executor,
    flush() {
      return Promise.all([...pendingTasks]).then(() => undefined);
    },
    dispose() {
      executor.dispose();
      if (scope.removeEventListener) {
        scope.removeEventListener('message', listener);
      } else if (scope.onmessage === listener) {
        scope.onmessage = null;
      }
    }
  };
}

async function runRequest(
  request: JsBlockRunRequest,
  modules: JsBlockInjectedModuleMap,
  postMessage: (message: JsBlockWorkerToHostMessage) => void,
  createEffectId: (requestId: string) => string,
  pendingEffects: Map<string, PendingEffect>
): Promise<void> {
  const evaluation = evaluateJsBlockSource({
    source: request.source,
    modules: selectRequestModules(modules, request.allowedImports),
    console: createWorkerConsole(request.requestId, postMessage)
  });

  if (!evaluation.ok) {
    postError(request, compileError(evaluation.error), postMessage);
    return;
  }

  const context = createBlockContext(
    request,
    postMessage,
    createEffectId,
    pendingEffects
  );
  let blockResult: unknown;
  try {
    blockResult = await evaluation.block.main(context);
  } catch (error) {
    if (error instanceof JsBlockWorkerEffectError) {
      postError(request, effectRuntimeError(error), postMessage);
      return;
    }

    postError(
      request,
      runtimeError(
        'main_failed',
        'runtime.main',
        `JS block main failed: ${getErrorMessage(error)}`,
        mapJsBlockRuntimeSourceLocation(error, evaluation.compiledSource)
      ),
      postMessage
    );
    return;
  }

  if (!isBlockResult(blockResult)) {
    postError(
      request,
      runtimeError(
        'runtime_error',
        'runtime.result',
        'JS block main must return { view, outputs } with plain-object outputs.'
      ),
      postMessage
    );
    return;
  }

  postMessage({
    direction: 'worker_to_host',
    type: 'completed',
    requestId: request.requestId,
    view: blockResult.view,
    outputs: blockResult.outputs
  });
}

function createWorkerConsole(
  requestId: string,
  postMessage: (message: JsBlockWorkerToHostMessage) => void
) {
  const write = (
    level: 'debug' | 'info' | 'warn' | 'error',
    values: unknown[]
  ) => {
    postMessage({
      direction: 'worker_to_host',
      type: 'log',
      requestId,
      level,
      message: values
        .map((value) =>
          typeof value === 'string' ? value : safeStringify(value)
        )
        .join(' '),
      data: values.length > 1 ? values : values[0]
    });
  };
  return {
    debug: (...values: unknown[]) => write('debug', values),
    info: (...values: unknown[]) => write('info', values),
    log: (...values: unknown[]) => write('info', values),
    warn: (...values: unknown[]) => write('warn', values),
    error: (...values: unknown[]) => write('error', values)
  };
}

function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return '[Unserializable]';
  }
}

function selectRequestModules(
  modules: JsBlockInjectedModuleMap,
  allowedImports: string[] | undefined
): JsBlockInjectedModuleMap {
  if (!allowedImports) {
    return modules;
  }
  return Object.fromEntries(
    allowedImports
      .filter((source) => Object.hasOwn(modules, source))
      .map((source) => [
        source,
        modules[source as keyof JsBlockInjectedModuleMap]
      ])
  ) as JsBlockInjectedModuleMap;
}

function createBlockContext(
  request: JsBlockRunRequest,
  postMessage: (message: JsBlockWorkerToHostMessage) => void,
  createEffectId: (requestId: string) => string,
  pendingEffects: Map<string, PendingEffect>
): BlockContext {
  const snapshot = request.contextSnapshot;
  const state = { ...request.state };
  const postEvent = (message: JsBlockWorkerEventRequestMessage) =>
    postMessage(message);
  const requestHostEffect = (message: JsBlockWorkerInterfaceRequestMessage) => {
    const promise = new Promise<unknown>((resolve, reject) => {
      const effectId = createEffectId(request.requestId);
      pendingEffects.set(effectId, {
        requestId: request.requestId,
        resolve,
        reject
      });
      postMessage({ ...message, effectId });
    });
    promise.catch(() => undefined);
    return promise;
  };

  const context: MutableBlockContext = {
    currentUser: readIdentity(snapshot.currentUser),
    workspace: readEntity(snapshot.workspace, 'workspace'),
    application: readEntity(snapshot.application, 'application'),
    page: readPage(snapshot.page),
    inputs: { ...(request.inputs ?? {}) },
    params: readRecord(snapshot.params),
    props: { ...request.props },
    state,
    patch(patch) {
      if (isRecord(patch)) {
        Object.assign(state, patch);
      }
    },
    api: {
      async get<TResponse = unknown>(
        path: string,
        apiRequest?: BlockApiRequest
      ): Promise<TResponse> {
        return callApi<TResponse>('GET', path, apiRequest);
      },
      async post<TResponse = unknown>(
        path: string,
        apiRequest?: BlockApiRequest
      ) {
        return callApi<TResponse>('POST', path, apiRequest);
      },
      async put<TResponse = unknown>(
        path: string,
        apiRequest?: BlockApiRequest
      ) {
        return callApi<TResponse>('PUT', path, apiRequest);
      },
      async patch<TResponse = unknown>(
        path: string,
        apiRequest?: BlockApiRequest
      ) {
        return callApi<TResponse>('PATCH', path, apiRequest);
      },
      async delete<TResponse = unknown>(
        path: string,
        apiRequest?: BlockApiRequest
      ) {
        return callApi<TResponse>('DELETE', path, apiRequest);
      },
      async head<TResponse = unknown>(
        path: string,
        apiRequest?: BlockApiRequest
      ) {
        return callApi<TResponse>('HEAD', path, apiRequest);
      },
      async options<TResponse = unknown>(
        path: string,
        apiRequest?: BlockApiRequest
      ) {
        return callApi<TResponse>('OPTIONS', path, apiRequest);
      },
      stream<TEvent = unknown>(
        method: BlockApiMethod,
        path: string,
        apiRequest?: BlockApiRequest
      ): AsyncIterable<TEvent> {
        const route = requireBlockApiRoute(method, path);
        let streamId: string | undefined;
        let finished = false;
        const open = async (): Promise<string> => {
          if (streamId) return streamId;
          const opened = (await requestHostEffect({
            direction: 'worker_to_host',
            type: 'interface',
            requestId: request.requestId,
            method: route.method,
            path: route.path,
            operation: 'stream_open',
            ...(apiRequest === undefined ? {} : { request: apiRequest })
          })) as { stream_id?: unknown };
          if (typeof opened?.stream_id !== 'string' || !opened.stream_id) {
            throw new Error('API stream did not return a stream id.');
          }
          streamId = opened.stream_id;
          return streamId;
        };
        return {
          [Symbol.asyncIterator]() {
            return {
              async next(): Promise<IteratorResult<TEvent>> {
                if (finished) return { done: true, value: undefined };
                const currentStreamId = await open();
                const item = (await requestHostEffect({
                  direction: 'worker_to_host',
                  type: 'interface',
                  requestId: request.requestId,
                  method: route.method,
                  path: route.path,
                  operation: 'stream_next',
                  streamId: currentStreamId
                })) as IteratorResult<TEvent>;
                if (item.done) finished = true;
                return item;
              },
              async return(): Promise<IteratorResult<TEvent>> {
                finished = true;
                if (streamId) {
                  await requestHostEffect({
                    direction: 'worker_to_host',
                    type: 'interface',
                    requestId: request.requestId,
                    method: route.method,
                    path: route.path,
                    operation: 'stream_cancel',
                    streamId
                  });
                }
                return { done: true, value: undefined };
              }
            };
          }
        };
      }
    },
    events: {
      emit(name, payload) {
        postEvent({
          direction: 'worker_to_host',
          type: 'event',
          requestId: request.requestId,
          name,
          ...(isRecord(payload) ? { payload } : {})
        });
      }
    },
    theme: readTheme(snapshot.theme),
    ui: readUi(snapshot.ui)
  };

  async function callApi<TResponse>(
    method: BlockApiMethod,
    path: string,
    apiRequest?: BlockApiRequest
  ): Promise<TResponse> {
    const route = requireBlockApiRoute(method, path);
    return (await requestHostEffect({
      direction: 'worker_to_host',
      type: 'interface',
      requestId: request.requestId,
      method: route.method,
      path: route.path,
      ...(apiRequest === undefined ? {} : { request: apiRequest })
    })) as TResponse;
  }

  return context;
}

function postError(
  request: JsBlockRunRequest,
  error: JsBlockRunError,
  postMessage: (message: JsBlockWorkerToHostMessage) => void
): void {
  postMessage({
    direction: 'worker_to_host',
    type: 'error',
    requestId: request.requestId,
    kind: error.kind,
    message: error.message,
    errors: error.errors
  });
}

function runtimeError(
  kind: JsBlockRunError['kind'],
  path: string,
  message: string,
  sourceLocation?: import('@1flowbase/page-protocol').BlockSourceLocation
): JsBlockRunError {
  return {
    kind,
    message,
    errors: [
      {
        code: 'runtime_error',
        path,
        message,
        ...(sourceLocation ? { sourceLocation } : {})
      }
    ]
  };
}

function compileError(error: JsBlockRunError): JsBlockRunError {
  if (error.kind === 'source_policy_failed') {
    return error;
  }
  return {
    ...error,
    kind: 'compile_failed',
    errors: error.errors.map((item) => ({
      ...item,
      code: 'transform_failed'
    }))
  };
}

function effectRuntimeError(error: JsBlockWorkerEffectError): JsBlockRunError {
  return runtimeError(
    'effect_failed',
    'runtime.main',
    `JS block main failed: ${error.message}`
  );
}

function normalizeHostMessage(
  value: unknown
): JsBlockHostToWorkerMessage | null {
  if (!isRecord(value)) {
    return null;
  }

  if (value.direction !== 'host_to_worker') {
    return null;
  }

  if (value.type === 'init') {
    return {
      direction: 'host_to_worker',
      type: 'init',
      ...(typeof value.requestId === 'string'
        ? { requestId: value.requestId }
        : {})
    };
  }

  if (value.type === 'dispose') {
    return {
      direction: 'host_to_worker',
      type: 'dispose',
      ...(typeof value.requestId === 'string'
        ? { requestId: value.requestId }
        : {})
    };
  }

  if (value.type === 'timeout' && typeof value.requestId === 'string') {
    return {
      direction: 'host_to_worker',
      type: 'timeout',
      requestId: value.requestId
    };
  }

  if (
    value.type === 'effect_result' &&
    typeof value.requestId === 'string' &&
    typeof value.effectId === 'string'
  ) {
    if (value.ok === true) {
      return {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: value.requestId,
        effectId: value.effectId,
        ok: true,
        value: value.value
      };
    }

    if (value.ok === false && isRunError(value.error)) {
      return {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: value.requestId,
        effectId: value.effectId,
        ok: false,
        error: value.error
      };
    }
  }

  if (value.type === 'run' && isRecord(value.request)) {
    return {
      direction: 'host_to_worker',
      type: 'run',
      request: value.request as unknown as JsBlockRunRequest
    };
  }

  return null;
}

function readIdentity(value: unknown): BlockContextIdentity | null {
  if (!isRecord(value) || typeof value.id !== 'string') {
    return null;
  }

  return {
    id: value.id,
    ...(typeof value.displayName === 'string'
      ? { displayName: value.displayName }
      : {})
  };
}

function requireBlockApiRoute(method: unknown, path: unknown) {
  const supportedMethods = new Set<BlockApiMethod>([
    'GET',
    'POST',
    'PUT',
    'PATCH',
    'DELETE',
    'HEAD',
    'OPTIONS'
  ]);
  if (!supportedMethods.has(method as BlockApiMethod)) {
    throw new Error('API method is not supported.');
  }
  if (
    typeof path !== 'string' ||
    path.length === 0 ||
    path !== path.trim() ||
    !path.startsWith('/') ||
    path.startsWith('//') ||
    path.includes('?') ||
    path.includes('#') ||
    path.split('/').some((segment) => segment === '.' || segment === '..')
  ) {
    throw new Error('API path must be a canonical relative path template.');
  }
  return {
    method: method as BlockApiMethod,
    path
  };
}

function readEntity(value: unknown, fallbackId: string): BlockContextEntity {
  if (!isRecord(value) || typeof value.id !== 'string') {
    return { id: fallbackId };
  }

  return {
    id: value.id,
    ...(typeof value.name === 'string' ? { name: value.name } : {})
  };
}

function readPage(value: unknown): BlockContextPage {
  if (!isRecord(value) || typeof value.id !== 'string') {
    return { id: 'page', route: '' };
  }

  return {
    id: value.id,
    route: typeof value.route === 'string' ? value.route : '',
    ...(typeof value.title === 'string' ? { title: value.title } : {})
  };
}

function readRecord(value: unknown): BlockContextRecord {
  return isRecord(value) ? { ...value } : {};
}

function readTheme(value: unknown): BlockContextTheme {
  if (!isRecord(value)) {
    return { mode: 'light', tokens: {} };
  }

  return {
    mode: value.mode === 'dark' ? 'dark' : 'light',
    tokens: readRecord(value.tokens)
  };
}

function readUi(value: unknown): BlockContextUi {
  if (!isRecord(value)) {
    return {};
  }

  return {
    ...(typeof value.locale === 'string' ? { locale: value.locale } : {}),
    ...(value.density === 'compact' || value.density === 'comfortable'
      ? { density: value.density }
      : {})
  };
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  return 'unknown error';
}

function isRunError(value: unknown): value is JsBlockRunError {
  return (
    isRecord(value) &&
    typeof value.kind === 'string' &&
    typeof value.message === 'string' &&
    Array.isArray(value.errors)
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
