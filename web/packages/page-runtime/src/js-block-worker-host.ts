import {
  createBlockContextMediator,
  type BlockContextMediatorContext,
  type BlockContextMediatorPolicy,
  type BlockContextMediatorState
} from './block-context-mediator';
import {
  createJsBlockHostEffectBridge,
  type JsBlockHostEffectBridge,
  type JsBlockHostEffectHandlers
} from './js-block-host-effect-bridge';
import {
  createJsBlockRuntimeSession,
  reduceJsBlockRuntimeSession,
  type JsBlockRunRequest,
  type JsBlockRuntimeSessionState,
  type JsBlockWorkerEffectResultMessage,
  type JsBlockWorkerRuntimeMessage
} from './js-block-worker-runtime';

export interface JsBlockWorkerLike {
  onmessage?: ((event: { data: unknown }) => void) | null;
  onerror?: ((event: { message?: string }) => void) | null;
  onmessageerror?: ((event: { message?: string }) => void) | null;
  postMessage(message: JsBlockWorkerRuntimeMessage): void;
  terminate(): void;
}

export type JsBlockWorkerFactory = () => JsBlockWorkerLike;
export type JsBlockWorkerTimeoutHandle = unknown;
export type JsBlockWorkerScheduleTimeout = (
  callback: () => void,
  timeoutMs: number
) => JsBlockWorkerTimeoutHandle;
export type JsBlockWorkerClearTimeout = (
  handle: JsBlockWorkerTimeoutHandle
) => void;

export interface JsBlockWorkerHostOptions {
  workerFactory: JsBlockWorkerFactory;
  startupTimeoutMs?: number;
  scheduleTimeout?: JsBlockWorkerScheduleTimeout;
  clearScheduledTimeout?: JsBlockWorkerClearTimeout;
  effectBridge?: JsBlockWorkerHostEffectBridgeOptions;
}

export interface JsBlockWorkerHostEffectBridgeOptions {
  policy: BlockContextMediatorPolicy;
  initialState?: BlockContextMediatorState;
  handlers?: JsBlockHostEffectHandlers;
  onInterfaceCall?: import('./js-block-host-effect-bridge').JsBlockHostEffectBridgeOptions['onInterfaceCall'];
  getContext?: (message: unknown) => BlockContextMediatorContext;
}

export interface JsBlockWorkerHost {
  getState(): JsBlockRuntimeSessionState;
  getEffectMediatorState(): BlockContextMediatorState | undefined;
  init(): JsBlockRuntimeSessionState;
  run(request: JsBlockRunRequest): JsBlockRuntimeSessionState;
  resolveEffect(
    message: JsBlockWorkerEffectResultMessage
  ): JsBlockRuntimeSessionState;
  dispose(requestId?: string): JsBlockRuntimeSessionState;
}

export function createJsBlockWorkerHost(
  options: JsBlockWorkerHostOptions
): JsBlockWorkerHost {
  let state = createJsBlockRuntimeSession();
  const worker = options.workerFactory();
  const scheduleTimeout = options.scheduleTimeout ?? defaultScheduleTimeout;
  const clearScheduledTimeout =
    options.clearScheduledTimeout ?? defaultClearTimeout;
  const timeoutHandles = new Map<string, JsBlockWorkerTimeoutHandle>();
  const disposedEffectRequests = new Set<string>();
  let startupTimeoutHandle: JsBlockWorkerTimeoutHandle | undefined;
  let queuedRequest: JsBlockRunRequest | undefined;
  let didTerminate = false;
  let didDispose = false;
  let effectBridge: JsBlockHostEffectBridge | undefined;

  const clearRequestTimeout = (requestId: string) => {
    const handle = timeoutHandles.get(requestId);
    if (handle === undefined) {
      return;
    }

    clearScheduledTimeout(handle);
    timeoutHandles.delete(requestId);
  };

  const clearStartupTimeout = () => {
    if (startupTimeoutHandle === undefined) {
      return;
    }
    clearScheduledTimeout(startupTimeoutHandle);
    startupTimeoutHandle = undefined;
  };

  const terminateOnce = () => {
    if (didTerminate) {
      return;
    }

    didTerminate = true;
    worker.terminate();
  };

  const failCurrentRequest = (
    kind: 'worker_startup_timeout' | 'worker_crash',
    message: string
  ) => {
    const requestId = state.currentRequestId;
    if (!requestId) {
      return;
    }
    applyMessage({
      direction: 'worker_to_host',
      type: 'error',
      requestId,
      kind,
      message,
      errors: [{ code: 'runtime_error', path: 'worker', message }]
    });
    clearStartupTimeout();
    queuedRequest = undefined;
    terminateOnce();
  };

  const reconcileTimeouts = () => {
    for (const [requestId] of timeoutHandles) {
      const request = state.requests[requestId];
      if (!request || request.status !== 'pending') {
        clearRequestTimeout(requestId);
      }
    }
  };

  const applyMessage = (message: unknown): JsBlockRuntimeSessionState => {
    state = reduceJsBlockRuntimeSession(state, message);
    reconcileTimeouts();
    if (
      typeof message === 'object' &&
      message !== null &&
      typeof (message as { requestId?: unknown }).requestId === 'string'
    ) {
      const requestId = (message as { requestId: string }).requestId;
      const request = state.requests[requestId];
      if (
        request &&
        request.status !== 'pending' &&
        !disposedEffectRequests.has(requestId)
      ) {
        disposedEffectRequests.add(requestId);
        options.effectBridge?.handlers?.disposeRequest?.(requestId);
      }
    }
    return state;
  };

  const resolveEffectMessage = (
    message: JsBlockWorkerEffectResultMessage
  ): JsBlockRuntimeSessionState => {
    if (didDispose) {
      return state;
    }

    const rejectionCount = state.rejections.length;
    state = reduceJsBlockRuntimeSession(state, message);
    if (state.rejections.length === rejectionCount) {
      worker.postMessage(message);
    }
    return state;
  };

  if (options.effectBridge) {
    effectBridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator(
        options.effectBridge.policy,
        options.effectBridge.initialState
      ),
      resolveEffect: resolveEffectMessage,
      handlers: options.effectBridge.handlers,
      onInterfaceCall: options.effectBridge.onInterfaceCall
    });
  }

  const handleTimeout = (requestId: string) => {
    if (didDispose) {
      return;
    }

    clearRequestTimeout(requestId);
    applyMessage({
      direction: 'host_to_worker',
      type: 'timeout',
      requestId
    });
    terminateOnce();
  };

  const scheduleRequestTimeout = (request: JsBlockRunRequest) => {
    clearRequestTimeout(request.requestId);
    const handle = scheduleTimeout(
      () => handleTimeout(request.requestId),
      request.limits.timeoutMs
    );
    timeoutHandles.set(request.requestId, handle);
  };

  const scheduleStartupTimeout = () => {
    clearStartupTimeout();
    startupTimeoutHandle = scheduleTimeout(
      () =>
        failCurrentRequest(
          'worker_startup_timeout',
          'JS block worker did not become ready in time.'
        ),
      options.startupTimeoutMs ?? 5000
    );
  };

  const postQueuedRun = () => {
    if (!queuedRequest || didDispose) {
      return;
    }
    const request = queuedRequest;
    queuedRequest = undefined;
    clearStartupTimeout();
    applyMessage({
      direction: 'worker_to_host',
      type: 'phase',
      requestId: request.requestId,
      phase: 'compiling'
    });
    scheduleRequestTimeout(request);
    worker.postMessage({ direction: 'host_to_worker', type: 'run', request });
  };

  const detachWorker = () => {
    worker.onmessage = null;
    worker.onerror = null;
    worker.onmessageerror = null;
  };

  worker.onmessage = (event) => {
    if (didDispose) {
      return;
    }

    const rejectionCount = state.rejections.length;
    applyMessage(event.data);
    if (
      typeof event.data === 'object' &&
      event.data !== null &&
      (event.data as { type?: unknown }).type === 'ready' &&
      state.workerStatus === 'ready'
    ) {
      postQueuedRun();
    }
    if (state.rejections.length === rejectionCount) {
      effectBridge?.handle(
        event.data,
        options.effectBridge?.getContext?.(event.data)
      );
    }
  };
  worker.onerror = (event) => {
    if (didDispose || !state.currentRequestId) {
      return;
    }

    failCurrentRequest(
      'worker_crash',
      event.message ?? 'JS block worker failed.'
    );
  };
  worker.onmessageerror = (event) => {
    if (didDispose || !state.currentRequestId) {
      return;
    }

    failCurrentRequest(
      'worker_crash',
      event.message ?? 'JS block worker message failed.'
    );
  };

  return {
    getState() {
      return state;
    },
    getEffectMediatorState() {
      return effectBridge?.getMediatorState();
    },
    init() {
      if (didDispose) {
        return state;
      }

      if (
        state.workerStatus === 'initializing' ||
        state.workerStatus === 'ready'
      ) {
        return state;
      }
      const message = {
        direction: 'host_to_worker',
        type: 'init'
      } as const;
      state = reduceJsBlockRuntimeSession(state, message);
      worker.postMessage(message);
      return state;
    },
    run(request) {
      if (didDispose) {
        return state;
      }

      const message = {
        direction: 'host_to_worker',
        type: 'run',
        request
      } as const;
      state = reduceJsBlockRuntimeSession(state, message);

      const requestState = state.requests[request.requestId];
      if (requestState?.status !== 'pending') {
        return state;
      }

      if (state.workerStatus === 'ready') {
        applyMessage({
          direction: 'worker_to_host',
          type: 'phase',
          requestId: request.requestId,
          phase: 'compiling'
        });
        scheduleRequestTimeout(request);
        worker.postMessage(message);
        return state;
      }

      queuedRequest = request;
      scheduleStartupTimeout();
      if (state.workerStatus === 'idle') {
        const initMessage = {
          direction: 'host_to_worker',
          type: 'init'
        } as const;
        state = reduceJsBlockRuntimeSession(state, initMessage);
        worker.postMessage(initMessage);
      }
      return state;
    },
    resolveEffect(message) {
      return resolveEffectMessage(message);
    },
    dispose(requestId) {
      if (didDispose) {
        return state;
      }

      didDispose = true;
      options.effectBridge?.handlers?.disposeRequest?.(requestId);
      const message =
        requestId === undefined
          ? ({
              direction: 'host_to_worker',
              type: 'dispose'
            } as const)
          : ({
              direction: 'host_to_worker',
              type: 'dispose',
              requestId
            } as const);

      state = reduceJsBlockRuntimeSession(state, message);
      clearStartupTimeout();
      queuedRequest = undefined;
      for (const [pendingRequestId] of timeoutHandles) {
        clearRequestTimeout(pendingRequestId);
      }
      worker.postMessage(message);
      detachWorker();
      terminateOnce();
      return state;
    }
  };
}

function defaultScheduleTimeout(
  callback: () => void,
  timeoutMs: number
): ReturnType<typeof setTimeout> {
  return setTimeout(callback, timeoutMs);
}

function defaultClearTimeout(handle: JsBlockWorkerTimeoutHandle): void {
  clearTimeout(handle as ReturnType<typeof setTimeout>);
}
