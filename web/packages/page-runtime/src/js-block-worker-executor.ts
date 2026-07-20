import type {
  BlockContext,
  BlockContextEntity,
  BlockContextIdentity,
  BlockContextPage,
  BlockContextRecord,
  BlockContextTheme,
  BlockContextUi
} from '@1flowbase/page-protocol';

import {
  evaluateJsBlockSource,
  type JsBlockInjectedModuleMap
} from './js-block-source-evaluator';
import type {
  JsBlockHostToWorkerMessage,
  JsBlockRunError,
  JsBlockRunRequest,
  JsBlockWorkerActionRequestMessage,
  JsBlockWorkerDataRequestMessage,
  JsBlockWorkerEventRequestMessage,
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
  postMessage({
    direction: 'worker_to_host',
    type: 'phase',
    requestId: request.requestId,
    phase: 'compiling'
  });
  const evaluation = evaluateJsBlockSource({
    source: request.source,
    modules: selectRequestModules(modules, request.allowedImports)
  });

  if (!evaluation.ok) {
    postError(request, compileError(evaluation.error), postMessage);
    return;
  }

  postMessage({
    direction: 'worker_to_host',
    type: 'phase',
    requestId: request.requestId,
    phase: 'executing'
  });

  const context = createBlockContext(
    request,
    postMessage,
    createEffectId,
    pendingEffects
  );
  let schema: unknown;
  try {
    schema = await evaluation.block.render(context);
  } catch (error) {
    if (error instanceof JsBlockWorkerEffectError) {
      postError(request, effectRuntimeError(error), postMessage);
      return;
    }

    postError(
      request,
      runtimeError(
        'render_failed',
        'runtime.render',
        `JS block render failed: ${getErrorMessage(error)}`
      ),
      postMessage
    );
    return;
  }

  postMessage({
    direction: 'worker_to_host',
    type: 'rendered',
    requestId: request.requestId,
    schema
  });
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
  const requestHostEffect = (
    message: JsBlockWorkerDataRequestMessage | JsBlockWorkerActionRequestMessage
  ) => {
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
    params: readRecord(snapshot.params),
    props: { ...request.props },
    state,
    patch(patch) {
      if (isRecord(patch)) {
        Object.assign(state, patch);
      }
    },
    data: {
      async query(queryId, params) {
        return requestHostEffect({
          direction: 'worker_to_host',
          type: 'data',
          requestId: request.requestId,
          queryId,
          ...(isRecord(params) ? { params } : {})
        });
      },
      async create() {
        throw new Error('Restricted block data creation is not supported.');
      },
      async update() {
        throw new Error('Restricted block data updates are not supported.');
      },
      async delete() {
        throw new Error('Restricted block data deletion is not supported.');
      }
    },
    actions: {
      async invoke(actionId, payload) {
        return requestHostEffect({
          direction: 'worker_to_host',
          type: 'action',
          requestId: request.requestId,
          actionId,
          ...(isRecord(payload) ? { payload } : {})
        });
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
  message: string
): JsBlockRunError {
  return {
    kind,
    message,
    errors: [{ code: kind, path, message }]
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
      code: 'compile_failed'
    }))
  };
}

function effectRuntimeError(error: JsBlockWorkerEffectError): JsBlockRunError {
  return runtimeError(
    'effect_failed',
    'runtime.render',
    `JS block render failed: ${error.message}`
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
