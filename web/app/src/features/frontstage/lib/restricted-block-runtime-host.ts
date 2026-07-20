import type { BlockUiSchemaValidationOptions } from '@1flowbase/page-protocol';
import {
  createJsBlockWorkerHost,
  type BlockContextMediatorState,
  type JsBlockHostEffectHandlers,
  type JsBlockInterfaceCallTrace,
  type JsBlockRunError,
  type JsBlockRunPhase,
  type JsBlockRuntimeRejection,
  type JsBlockRuntimeSessionState,
  type JsBlockWorkerClearTimeout,
  type JsBlockWorkerEffect,
  type JsBlockWorkerFactory,
  type JsBlockWorkerLogEntry,
  type JsBlockWorkerScheduleTimeout
} from '@1flowbase/page-runtime';

import type { RestrictedBlockRunPlan } from './restricted-block-loader';

export type RestrictedBlockRuntimeHostSnapshotStatus =
  | 'idle'
  | 'running'
  | 'ready'
  | 'failed'
  | 'timed_out'
  | 'disposed';

export interface RestrictedBlockRuntimeHostSnapshot {
  status: RestrictedBlockRuntimeHostSnapshotStatus;
  phase?: JsBlockRunPhase;
  requestId: string;
  blockId: string;
  schemaValidationOptions: BlockUiSchemaValidationOptions;
  view?: unknown;
  outputs?: Record<string, unknown>;
  error?: JsBlockRunError;
  logs: JsBlockWorkerLogEntry[];
  effects: JsBlockWorkerEffect[];
  rejections: JsBlockRuntimeRejection[];
  mediatorState?: BlockContextMediatorState;
  interfaceCalls?: JsBlockInterfaceCallTrace[];
}

export interface RestrictedBlockRuntimeHostOptions {
  runPlan: RestrictedBlockRunPlan;
  workerFactory: JsBlockWorkerFactory;
  handlers?: JsBlockHostEffectHandlers;
  scheduleTimeout?: JsBlockWorkerScheduleTimeout;
  clearScheduledTimeout?: JsBlockWorkerClearTimeout;
}

export interface RestrictedBlockRuntimeHost {
  run(): RestrictedBlockRuntimeHostSnapshot;
  dispose(): RestrictedBlockRuntimeHostSnapshot;
  getSnapshot(): RestrictedBlockRuntimeHostSnapshot;
  getHostState(): JsBlockRuntimeSessionState;
}

export function createRestrictedBlockRuntimeHost(
  options: RestrictedBlockRuntimeHostOptions
): RestrictedBlockRuntimeHost {
  const runPlan = options.runPlan;
  const interfaceCalls: JsBlockInterfaceCallTrace[] = [];
  const workerHost = createJsBlockWorkerHost({
    workerFactory: options.workerFactory,
    scheduleTimeout: options.scheduleTimeout,
    clearScheduledTimeout: options.clearScheduledTimeout,
    effectBridge: {
      policy: runPlan.mediatorPolicy,
      handlers: options.handlers,
      getContext: () => ({ tickId: runPlan.request.requestId }),
      onInterfaceCall: (trace) => interfaceCalls.push(trace)
    }
  });
  let didDispose = false;

  const createSnapshot = (): RestrictedBlockRuntimeHostSnapshot => {
    const state = workerHost.getState();
    const requestState = state.requests[runPlan.request.requestId];
    const result = requestState?.result;

    return {
      status: didDispose ? 'disposed' : mapSnapshotStatus(requestState?.status),
      phase: didDispose ? 'disposed' : requestState?.phase,
      requestId: runPlan.request.requestId,
      blockId: runPlan.request.blockId,
      schemaValidationOptions: cloneSchemaValidationOptions(
        runPlan.schemaValidationOptions
      ),
      ...(result?.ok === true
        ? {
            view: cloneSnapshotValue(result.view),
            outputs: cloneSnapshotValue(result.outputs)
          }
        : {}),
      ...(result?.ok === false
        ? { error: cloneSnapshotValue(result.error) }
        : {}),
      logs: cloneSnapshotValue(requestState?.logs ?? []),
      effects: cloneSnapshotValue(requestState?.effects ?? []),
      rejections: cloneSnapshotValue(state.rejections),
      mediatorState: cloneSnapshotValue(workerHost.getEffectMediatorState()),
      interfaceCalls: cloneSnapshotValue(interfaceCalls)
    };
  };

  return {
    run() {
      if (!didDispose) {
        workerHost.run(runPlan.request);
      }

      return createSnapshot();
    },
    dispose() {
      if (!didDispose) {
        didDispose = true;
        workerHost.dispose(runPlan.request.requestId);
      }

      return createSnapshot();
    },
    getSnapshot() {
      return createSnapshot();
    },
    getHostState() {
      return cloneSnapshotValue(workerHost.getState());
    }
  };
}

function mapSnapshotStatus(
  requestStatus: RestrictedBlockRuntimeHostRequestStatus | undefined
): RestrictedBlockRuntimeHostSnapshotStatus {
  switch (requestStatus) {
    case 'pending':
      return 'running';
    case 'ready':
    case 'failed':
    case 'timed_out':
    case 'disposed':
      return requestStatus;
    case undefined:
      return 'idle';
  }
}

type RestrictedBlockRuntimeHostRequestStatus =
  JsBlockRuntimeSessionState['requests'][string]['status'];

function cloneSchemaValidationOptions(
  options: BlockUiSchemaValidationOptions
): BlockUiSchemaValidationOptions {
  const { allowedActions, allowedDataPermissions, allowedEvents, ...base } =
    options;
  return {
    ...base,
    ...(allowedDataPermissions
      ? { allowedDataPermissions: [...allowedDataPermissions] }
      : {}),
    ...(allowedActions ? { allowedActions: [...allowedActions] } : {}),
    ...(allowedEvents ? { allowedEvents: [...allowedEvents] } : {})
  };
}

function cloneSnapshotValue<T>(value: T): T {
  return cloneUnknown(value, new WeakMap<object, unknown>()) as T;
}

function cloneUnknown(value: unknown, seen: WeakMap<object, unknown>): unknown {
  if (value === null || typeof value !== 'object') {
    return value;
  }

  const cached = seen.get(value);
  if (cached) {
    return cached;
  }

  if (Array.isArray(value)) {
    const output: unknown[] = [];
    seen.set(value, output);
    for (const item of value) {
      output.push(cloneUnknown(item, seen));
    }
    return output;
  }

  const output: Record<string, unknown> = {};
  seen.set(value, output);
  for (const [key, item] of Object.entries(value)) {
    output[key] = cloneUnknown(item, seen);
  }
  return output;
}
