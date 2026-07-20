import {
  validateBlockUiSchema,
  type BlockProtocolError,
  type BlockUiSchema
} from '@1flowbase/page-protocol';

import {
  transformJsBlockSource,
  type JsBlockSourceTransformSuccess
} from './js-block-source-transform';

export interface JsBlockRuntimeLimits {
  timeoutMs: number;
  maxRenderDepth?: number;
  maxRenderNodes?: number;
}

export interface JsBlockRunRequest {
  requestId: string;
  blockId: string;
  source: string;
  inputs: Record<string, unknown>;
  props: Record<string, unknown>;
  state: Record<string, unknown>;
  contextSnapshot: Record<string, unknown>;
  limits: JsBlockRuntimeLimits;
  allowedImports?: string[];
}

export type JsBlockRunErrorKind =
  | 'source_policy_failed'
  | 'worker_startup_timeout'
  | 'worker_crash'
  | 'compile_failed'
  | 'main_failed'
  | 'effect_failed'
  | 'schema_invalid'
  | 'runtime_timeout'
  | 'runtime_error';

export type JsBlockRunPhase =
  | 'queued'
  | 'starting'
  | 'compiling'
  | 'executing'
  | 'waiting_effect'
  | 'validating_schema'
  | 'ready'
  | 'failed'
  | 'timed_out'
  | 'disposed';

export interface JsBlockRunError {
  kind: JsBlockRunErrorKind;
  message: string;
  errors: BlockProtocolError[];
}

export type JsBlockRunResult =
  | {
      ok: true;
      requestId: string;
      blockId: string;
      view: BlockUiSchema;
      outputs: Record<string, unknown>;
    }
  | {
      ok: false;
      requestId: string;
      blockId: string;
      error: JsBlockRunError;
    };

export type JsBlockWorkerRuntimeStatus =
  | 'idle'
  | 'initializing'
  | 'ready'
  | 'disposed';

export type JsBlockRunStatus =
  | 'pending'
  | 'ready'
  | 'failed'
  | 'timed_out'
  | 'disposed';

export type JsBlockWorkerLogLevel = 'debug' | 'info' | 'warn' | 'error';

export interface JsBlockWorkerLogEntry {
  requestId: string;
  level: JsBlockWorkerLogLevel;
  message: string;
  data?: unknown;
}

export type JsBlockWorkerEffect =
  | {
      type: 'event';
      requestId: string;
      name: string;
      payload?: unknown;
    }
  | {
      type: 'interface';
      requestId: string;
      effectId?: string;
      bindingAlias: string;
      request?: unknown;
    };

export interface JsBlockRuntimeRequestState {
  requestId: string;
  blockId: string;
  status: JsBlockRunStatus;
  phase: JsBlockRunPhase;
  request: JsBlockRunRequest;
  compiledSource?: JsBlockSourceTransformSuccess;
  result?: JsBlockRunResult;
  logs: JsBlockWorkerLogEntry[];
  effects: JsBlockWorkerEffect[];
}

export type JsBlockRuntimeRejectionCode =
  | 'invalid_message'
  | 'unknown_request_id'
  | 'stale_request_id'
  | 'request_not_pending';

export interface JsBlockRuntimeRejection {
  code: JsBlockRuntimeRejectionCode;
  path: string;
  message: string;
  requestId?: string;
}

export interface JsBlockRuntimeSessionState {
  workerStatus: JsBlockWorkerRuntimeStatus;
  currentRequestId?: string;
  requests: Record<string, JsBlockRuntimeRequestState>;
  rejections: JsBlockRuntimeRejection[];
}

export interface JsBlockWorkerInitMessage {
  direction: 'host_to_worker';
  type: 'init';
  requestId?: string;
}

export interface JsBlockWorkerRunMessage {
  direction: 'host_to_worker';
  type: 'run';
  request: JsBlockRunRequest;
}

export interface JsBlockWorkerDisposeMessage {
  direction: 'host_to_worker';
  type: 'dispose';
  requestId?: string;
}

export interface JsBlockWorkerTimeoutMessage {
  direction: 'host_to_worker';
  type: 'timeout';
  requestId: string;
}

export interface JsBlockWorkerEffectResultSuccessMessage {
  direction: 'host_to_worker';
  type: 'effect_result';
  requestId: string;
  effectId: string;
  ok: true;
  value?: unknown;
}

export interface JsBlockWorkerEffectResultFailureMessage {
  direction: 'host_to_worker';
  type: 'effect_result';
  requestId: string;
  effectId: string;
  ok: false;
  error: JsBlockRunError;
}

export type JsBlockWorkerEffectResultMessage =
  | JsBlockWorkerEffectResultSuccessMessage
  | JsBlockWorkerEffectResultFailureMessage;

export type JsBlockHostToWorkerMessage =
  | JsBlockWorkerInitMessage
  | JsBlockWorkerRunMessage
  | JsBlockWorkerDisposeMessage
  | JsBlockWorkerTimeoutMessage
  | JsBlockWorkerEffectResultMessage;

export interface JsBlockWorkerReadyMessage {
  direction: 'worker_to_host';
  type: 'ready';
  requestId?: string;
}

export interface JsBlockWorkerPhaseMessage {
  direction: 'worker_to_host';
  type: 'phase';
  requestId: string;
  phase: Extract<JsBlockRunPhase, 'compiling' | 'executing'>;
}

export interface JsBlockWorkerCompletedMessage {
  direction: 'worker_to_host';
  type: 'completed';
  requestId: string;
  view: unknown;
  outputs: unknown;
}

export interface JsBlockWorkerErrorMessage {
  direction: 'worker_to_host';
  type: 'error';
  requestId: string;
  kind?: JsBlockRunErrorKind;
  message?: string;
  errors?: BlockProtocolError[];
}

export interface JsBlockWorkerLogMessage {
  direction: 'worker_to_host';
  type: 'log';
  requestId: string;
  level: JsBlockWorkerLogLevel;
  message: string;
  data?: unknown;
}

export interface JsBlockWorkerEventRequestMessage {
  direction: 'worker_to_host';
  type: 'event';
  requestId: string;
  name: string;
  payload?: unknown;
}

export interface JsBlockWorkerInterfaceRequestMessage {
  direction: 'worker_to_host';
  type: 'interface';
  requestId: string;
  effectId?: string;
  bindingAlias: string;
  request?: unknown;
}

export type JsBlockWorkerToHostMessage =
  | JsBlockWorkerReadyMessage
  | JsBlockWorkerPhaseMessage
  | JsBlockWorkerCompletedMessage
  | JsBlockWorkerErrorMessage
  | JsBlockWorkerLogMessage
  | JsBlockWorkerEventRequestMessage
  | JsBlockWorkerInterfaceRequestMessage;

export type JsBlockWorkerRuntimeMessage =
  | JsBlockHostToWorkerMessage
  | JsBlockWorkerToHostMessage;

type RecordValue = Record<string, unknown>;

export function createJsBlockRuntimeSession(): JsBlockRuntimeSessionState {
  return {
    workerStatus: 'idle',
    requests: {},
    rejections: []
  };
}

export function reduceJsBlockRuntimeSession(
  state: JsBlockRuntimeSessionState,
  message: unknown
): JsBlockRuntimeSessionState {
  if (!isRecord(message)) {
    return reject(state, {
      code: 'invalid_message',
      path: 'message',
      message: 'Runtime message must be an object.'
    });
  }

  const direction = message.direction;
  if (direction !== 'host_to_worker' && direction !== 'worker_to_host') {
    return reject(state, {
      code: 'invalid_message',
      path: 'message.direction',
      message: 'Runtime message direction is invalid.'
    });
  }

  const type = message.type;
  if (typeof type !== 'string') {
    return reject(state, {
      code: 'invalid_message',
      path: 'message.type',
      message: 'Runtime message type is required.'
    });
  }

  if (direction === 'host_to_worker') {
    return reduceHostToWorkerMessage(state, message, type);
  }

  return reduceWorkerToHostMessage(state, message, type);
}

function reduceHostToWorkerMessage(
  state: JsBlockRuntimeSessionState,
  message: RecordValue,
  type: string
): JsBlockRuntimeSessionState {
  switch (type) {
    case 'init':
      return updateCurrentRequestPhase({
        ...state,
        workerStatus: 'initializing'
      }, 'starting');
    case 'run':
      return reduceRunMessage(state, message);
    case 'timeout':
      return reduceTimeoutMessage(state, message);
    case 'effect_result':
      return reduceEffectResultMessage(state, message);
    case 'dispose':
      return reduceDisposeMessage(state, message);
    default:
      return reject(state, {
        code: 'invalid_message',
        path: 'message.type',
        message: `Unsupported host to worker message type: ${type}.`
      });
  }
}

function reduceWorkerToHostMessage(
  state: JsBlockRuntimeSessionState,
  message: RecordValue,
  type: string
): JsBlockRuntimeSessionState {
  switch (type) {
    case 'ready':
      return {
        ...state,
        workerStatus: 'ready'
      };
    case 'phase':
      return reducePhaseMessage(state, message);
    case 'completed':
      return reduceCompletedMessage(state, message);
    case 'error':
      return reduceRuntimeErrorMessage(state, message);
    case 'log':
      return reduceLogMessage(state, message);
    case 'event':
      return reduceEffectMessage(state, message, 'event');
    case 'interface':
      return reduceEffectMessage(state, message, 'interface');
    default:
      return reject(state, {
        code: 'invalid_message',
        path: 'message.type',
        message: `Unsupported worker to host message type: ${type}.`
      });
  }
}

function reduceRunMessage(
  state: JsBlockRuntimeSessionState,
  message: RecordValue
): JsBlockRuntimeSessionState {
  const requestResult = readRunRequest(message.request);
  if (!requestResult.ok) {
    return reject(state, requestResult.rejection);
  }

  const request = requestResult.request;
  const sourceResult = transformJsBlockSource(request.source, {
    allowedImports: request.allowedImports
  });
  const requestState: JsBlockRuntimeRequestState = {
    requestId: request.requestId,
    blockId: request.blockId,
    status: 'pending',
    phase: 'queued',
    request,
    logs: [],
    effects: []
  };

  if (!sourceResult.ok) {
    const failedRequest = withRunFailure(
      requestState,
      'source_policy_failed',
      'JS block source policy validation failed.',
      sourceResult.errors
    );

    return withRequest(state, failedRequest);
  }

  return withRequest(
    state,
    {
      ...requestState,
      compiledSource: sourceResult
    },
    request.requestId
  );
}

function reduceCompletedMessage(
  state: JsBlockRuntimeSessionState,
  message: RecordValue
): JsBlockRuntimeSessionState {
  const requestIdResult = readString(message, 'requestId', 'message.requestId');
  if (!requestIdResult.ok) {
    return reject(state, requestIdResult.rejection);
  }

  if (!hasOwn(message, 'view')) {
    return reject(state, {
      code: 'invalid_message',
      path: 'message.view',
      message: 'Completed message view is required.',
      requestId: requestIdResult.value
    });
  }
  const outputsResult = readRecord(message, 'outputs', 'message.outputs');
  if (!outputsResult.ok) {
    return reject(state, outputsResult.rejection);
  }

  const requestResult = readCurrentRequest(state, requestIdResult.value);
  if (!requestResult.ok) {
    return reject(state, requestResult.rejection);
  }

  const validatingState = updateRequest(state, {
    ...requestResult.request,
    phase: 'validating_schema'
  });
  const validation = validateBlockUiSchema(message.view, {
    maxDepth: requestResult.request.request.limits.maxRenderDepth,
    maxNodes: requestResult.request.request.limits.maxRenderNodes
  });

  if (!validation.ok) {
    return completeRequest(
      validatingState,
      withRunFailure(
        requestResult.request,
        'schema_invalid',
        'BlockResult view validation failed.',
        validation.errors
      )
    );
  }

  return completeRequest(validatingState, {
    ...requestResult.request,
    status: 'ready',
    phase: 'ready',
    result: {
      ok: true,
      requestId: requestResult.request.requestId,
      blockId: requestResult.request.blockId,
      view: validation.schema,
      outputs: outputsResult.value
    }
  });
}

function reducePhaseMessage(
  state: JsBlockRuntimeSessionState,
  message: RecordValue
): JsBlockRuntimeSessionState {
  const requestIdResult = readString(message, 'requestId', 'message.requestId');
  if (!requestIdResult.ok) {
    return reject(state, requestIdResult.rejection);
  }
  if (message.phase !== 'compiling' && message.phase !== 'executing') {
    return reject(state, {
      code: 'invalid_message',
      path: 'message.phase',
      message: 'Runtime phase message is invalid.',
      requestId: requestIdResult.value
    });
  }
  const requestResult = readCurrentRequest(state, requestIdResult.value);
  if (!requestResult.ok) {
    return reject(state, requestResult.rejection);
  }
  return updateRequest(state, {
    ...requestResult.request,
    phase: message.phase
  });
}

function reduceRuntimeErrorMessage(
  state: JsBlockRuntimeSessionState,
  message: RecordValue
): JsBlockRuntimeSessionState {
  const requestIdResult = readString(message, 'requestId', 'message.requestId');
  if (!requestIdResult.ok) {
    return reject(state, requestIdResult.rejection);
  }

  const requestResult = readCurrentRequest(state, requestIdResult.value);
  if (!requestResult.ok) {
    return reject(state, requestResult.rejection);
  }

  const runtimeMessage =
    typeof message.message === 'string'
      ? message.message
      : 'JS block runtime failed.';
  const errors = readProtocolErrors(message.errors) ?? [
    createProtocolError('runtime_error', 'runtime', runtimeMessage)
  ];
  const errorKind = isRunErrorKind(message.kind)
    ? message.kind
    : 'runtime_error';

  return completeRequest(
    state,
    withRunFailure(requestResult.request, errorKind, runtimeMessage, errors)
  );
}

function reduceTimeoutMessage(
  state: JsBlockRuntimeSessionState,
  message: RecordValue
): JsBlockRuntimeSessionState {
  const requestIdResult = readString(message, 'requestId', 'message.requestId');
  if (!requestIdResult.ok) {
    return reject(state, requestIdResult.rejection);
  }

  const requestResult = readCurrentRequest(state, requestIdResult.value);
  if (!requestResult.ok) {
    return reject(state, requestResult.rejection);
  }

  const error = createProtocolError(
    'runtime_timeout',
    'runtime',
    'JS block runtime timed out.'
  );

  return completeRequest(state, {
    ...withRunFailure(
      requestResult.request,
      'runtime_timeout',
      'JS block runtime timed out.',
      [error]
    ),
    status: 'timed_out',
    phase: 'timed_out'
  });
}

function reduceEffectResultMessage(
  state: JsBlockRuntimeSessionState,
  message: RecordValue
): JsBlockRuntimeSessionState {
  const requestIdResult = readString(message, 'requestId', 'message.requestId');
  if (!requestIdResult.ok) {
    return reject(state, requestIdResult.rejection);
  }

  const effectIdResult = readString(message, 'effectId', 'message.effectId');
  if (!effectIdResult.ok) {
    return reject(state, effectIdResult.rejection);
  }

  const requestResult = readCurrentRequest(state, requestIdResult.value);
  if (!requestResult.ok) {
    return reject(state, requestResult.rejection);
  }

  return updateRequest(state, {
    ...requestResult.request,
    phase: 'executing'
  });
}

function reduceDisposeMessage(
  state: JsBlockRuntimeSessionState,
  message: RecordValue
): JsBlockRuntimeSessionState {
  if (!hasOwn(message, 'requestId')) {
    return {
      ...state,
      workerStatus: 'disposed',
      currentRequestId: undefined
    };
  }

  const requestIdResult = readString(message, 'requestId', 'message.requestId');
  if (!requestIdResult.ok) {
    return reject(state, requestIdResult.rejection);
  }

  const requestResult = readCurrentRequest(state, requestIdResult.value);
  if (!requestResult.ok) {
    return reject(state, requestResult.rejection);
  }

  return {
    ...completeRequest(state, {
      ...requestResult.request,
      status: 'disposed',
      phase: 'disposed'
    }),
    currentRequestId: undefined
  };
}

function reduceLogMessage(
  state: JsBlockRuntimeSessionState,
  message: RecordValue
): JsBlockRuntimeSessionState {
  const requestIdResult = readString(message, 'requestId', 'message.requestId');
  if (!requestIdResult.ok) {
    return reject(state, requestIdResult.rejection);
  }

  const level = message.level;
  if (!isLogLevel(level)) {
    return reject(state, {
      code: 'invalid_message',
      path: 'message.level',
      message: 'Log message level is invalid.',
      requestId: requestIdResult.value
    });
  }

  const logMessageResult = readString(message, 'message', 'message.message');
  if (!logMessageResult.ok) {
    return reject(state, logMessageResult.rejection);
  }

  const requestResult = readCurrentRequest(state, requestIdResult.value);
  if (!requestResult.ok) {
    return reject(state, requestResult.rejection);
  }

  const logEntry: JsBlockWorkerLogEntry = {
    requestId: requestIdResult.value,
    level,
    message: logMessageResult.value,
    data: message.data
  };

  return updateRequest(state, {
    ...requestResult.request,
    logs: [...requestResult.request.logs, logEntry]
  });
}

function reduceEffectMessage(
  state: JsBlockRuntimeSessionState,
  message: RecordValue,
  effectType: JsBlockWorkerEffect['type']
): JsBlockRuntimeSessionState {
  const requestIdResult = readString(message, 'requestId', 'message.requestId');
  if (!requestIdResult.ok) {
    return reject(state, requestIdResult.rejection);
  }

  const effectResult = readWorkerEffect(message, effectType, requestIdResult.value);
  if (!effectResult.ok) {
    return reject(state, effectResult.rejection);
  }

  const requestResult = readCurrentRequest(state, requestIdResult.value);
  if (!requestResult.ok) {
    return reject(state, requestResult.rejection);
  }

  return updateRequest(state, {
    ...requestResult.request,
    phase:
      effectType === 'event'
        ? requestResult.request.phase
        : 'waiting_effect',
    effects: [...requestResult.request.effects, effectResult.effect]
  });
}

function readRunRequest(
  value: unknown
):
  | { ok: true; request: JsBlockRunRequest }
  | { ok: false; rejection: JsBlockRuntimeRejection } {
  if (!isRecord(value)) {
    return invalid('message.request', 'Run message request must be an object.');
  }

  const requestId = readString(value, 'requestId', 'message.request.requestId');
  if (!requestId.ok) {
    return requestId;
  }

  const blockId = readString(value, 'blockId', 'message.request.blockId');
  if (!blockId.ok) {
    return blockId;
  }

  const source = readString(value, 'source', 'message.request.source');
  if (!source.ok) {
    return source;
  }

  const props = readRecord(value, 'props', 'message.request.props');
  if (!props.ok) {
    return props;
  }

  const inputs = readRecord(value, 'inputs', 'message.request.inputs');
  if (!inputs.ok) {
    return inputs;
  }

  const state = readRecord(value, 'state', 'message.request.state');
  if (!state.ok) {
    return state;
  }

  const contextSnapshot = readRecord(
    value,
    'contextSnapshot',
    'message.request.contextSnapshot'
  );
  if (!contextSnapshot.ok) {
    return contextSnapshot;
  }

  const limits = readRuntimeLimits(value.limits);
  if (!limits.ok) {
    return limits;
  }
  const allowedImports = readOptionalStringArray(
    value.allowedImports,
    'message.request.allowedImports'
  );
  if (!allowedImports.ok) {
    return allowedImports;
  }

  return {
    ok: true,
    request: {
      requestId: requestId.value,
      blockId: blockId.value,
      source: source.value,
      inputs: inputs.value,
      props: props.value,
      state: state.value,
      contextSnapshot: contextSnapshot.value,
      limits: limits.value,
      allowedImports: allowedImports.value
    }
  };
}

function readOptionalStringArray(
  value: unknown,
  path: string
):
  | { ok: true; value: string[] | undefined }
  | { ok: false; rejection: JsBlockRuntimeRejection } {
  if (value === undefined) {
    return { ok: true, value: undefined };
  }
  if (!Array.isArray(value) || value.some((item) => typeof item !== 'string')) {
    return invalid(path, 'Run message allowedImports must be a string array.');
  }
  return { ok: true, value };
}

function readRuntimeLimits(
  value: unknown
):
  | { ok: true; value: JsBlockRuntimeLimits }
  | { ok: false; rejection: JsBlockRuntimeRejection } {
  if (!isRecord(value)) {
    return invalid(
      'message.request.limits',
      'Run message limits must be an object.'
    );
  }

  const timeoutMs = value.timeoutMs;
  if (!isPositiveNumber(timeoutMs)) {
    return invalid(
      'message.request.limits.timeoutMs',
      'Run message timeoutMs must be a positive number.'
    );
  }

  const maxRenderDepth = readOptionalPositiveNumber(
    value.maxRenderDepth,
    'message.request.limits.maxRenderDepth'
  );
  if (!maxRenderDepth.ok) {
    return maxRenderDepth;
  }

  const maxRenderNodes = readOptionalPositiveNumber(
    value.maxRenderNodes,
    'message.request.limits.maxRenderNodes'
  );
  if (!maxRenderNodes.ok) {
    return maxRenderNodes;
  }

  return {
    ok: true,
    value: {
      timeoutMs,
      maxRenderDepth: maxRenderDepth.value,
      maxRenderNodes: maxRenderNodes.value
    }
  };
}

function readOptionalPositiveNumber(
  value: unknown,
  path: string
):
  | { ok: true; value?: number }
  | { ok: false; rejection: JsBlockRuntimeRejection } {
  if (value === undefined) {
    return { ok: true };
  }

  if (!isPositiveNumber(value)) {
    return invalid(path, 'Runtime limit must be a positive number.');
  }

  return { ok: true, value };
}

function readWorkerEffect(
  message: RecordValue,
  effectType: JsBlockWorkerEffect['type'],
  requestId: string
):
  | { ok: true; effect: JsBlockWorkerEffect }
  | { ok: false; rejection: JsBlockRuntimeRejection } {
  if (effectType === 'event') {
    const name = readString(message, 'name', 'message.name');
    if (!name.ok) {
      return name;
    }
    return {
      ok: true,
      effect: {
        type: 'event',
        requestId,
        name: name.value,
        payload: message.payload
      }
    };
  }

  const bindingAlias = readString(
    message,
    'bindingAlias',
    'message.bindingAlias'
  );
  if (!bindingAlias.ok) {
    return bindingAlias;
  }
  const effectId =
    typeof message.effectId === 'string' && message.effectId.length > 0
      ? message.effectId
      : undefined;
  return {
    ok: true,
    effect: {
      type: 'interface',
      requestId,
      ...(effectId ? { effectId } : {}),
      bindingAlias: bindingAlias.value,
      request: message.request
    }
  };
}

function readCurrentRequest(
  state: JsBlockRuntimeSessionState,
  requestId: string
):
  | { ok: true; request: JsBlockRuntimeRequestState }
  | { ok: false; rejection: JsBlockRuntimeRejection } {
  const request = state.requests[requestId];
  if (!request) {
    return {
      ok: false,
      rejection: {
        code: 'unknown_request_id',
        path: 'message.requestId',
        message: `Runtime message references unknown requestId: ${requestId}.`,
        requestId
      }
    };
  }

  if (request.status !== 'pending') {
    return {
      ok: false,
      rejection: {
        code: 'request_not_pending',
        path: 'message.requestId',
        message: `Runtime message requestId is not pending: ${requestId}.`,
        requestId
      }
    };
  }

  if (state.currentRequestId !== requestId) {
    return {
      ok: false,
      rejection: {
        code: 'stale_request_id',
        path: 'message.requestId',
        message: `Runtime message requestId is not current: ${requestId}.`,
        requestId
      }
    };
  }

  return { ok: true, request };
}

function readString(
  record: RecordValue,
  key: string,
  path: string
):
  | { ok: true; value: string }
  | { ok: false; rejection: JsBlockRuntimeRejection } {
  const value = record[key];
  if (typeof value !== 'string' || value.length === 0) {
    return invalid(path, `${key} must be a non-empty string.`);
  }

  return { ok: true, value };
}

function readRecord(
  record: RecordValue,
  key: string,
  path: string
):
  | { ok: true; value: RecordValue }
  | { ok: false; rejection: JsBlockRuntimeRejection } {
  const value = record[key];
  if (!isRecord(value)) {
    return invalid(path, `${key} must be an object.`);
  }

  return { ok: true, value };
}

function readProtocolErrors(value: unknown): BlockProtocolError[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }

  const errors = value.filter(isProtocolError);
  return errors.length > 0 ? errors : undefined;
}

function isProtocolError(value: unknown): value is BlockProtocolError {
  return (
    isRecord(value) &&
    typeof value.code === 'string' &&
    typeof value.path === 'string' &&
    typeof value.message === 'string'
  );
}

function withRunFailure(
  request: JsBlockRuntimeRequestState,
  kind: JsBlockRunErrorKind,
  message: string,
  errors: BlockProtocolError[]
): JsBlockRuntimeRequestState {
  return {
    ...request,
    status: kind === 'runtime_timeout' ? 'timed_out' : 'failed',
    phase: kind === 'runtime_timeout' ? 'timed_out' : 'failed',
    result: {
      ok: false,
      requestId: request.requestId,
      blockId: request.blockId,
      error: {
        kind,
        message,
        errors
      }
    }
  };
}

function withRequest(
  state: JsBlockRuntimeSessionState,
  request: JsBlockRuntimeRequestState,
  currentRequestId?: string
): JsBlockRuntimeSessionState {
  return {
    ...state,
    currentRequestId,
    requests: {
      ...state.requests,
      [request.requestId]: request
    }
  };
}

function completeRequest(
  state: JsBlockRuntimeSessionState,
  request: JsBlockRuntimeRequestState
): JsBlockRuntimeSessionState {
  return {
    ...updateRequest(state, request),
    currentRequestId: undefined
  };
}

function updateRequest(
  state: JsBlockRuntimeSessionState,
  request: JsBlockRuntimeRequestState
): JsBlockRuntimeSessionState {
  return {
    ...state,
    requests: {
      ...state.requests,
      [request.requestId]: request
    }
  };
}

function updateCurrentRequestPhase(
  state: JsBlockRuntimeSessionState,
  phase: JsBlockRunPhase
): JsBlockRuntimeSessionState {
  const requestId = state.currentRequestId;
  if (!requestId || !state.requests[requestId]) {
    return state;
  }
  return updateRequest(state, {
    ...state.requests[requestId],
    phase
  });
}

function reject(
  state: JsBlockRuntimeSessionState,
  rejection: JsBlockRuntimeRejection
): JsBlockRuntimeSessionState {
  return {
    ...state,
    rejections: [...state.rejections, rejection]
  };
}

function invalid(
  path: string,
  message: string,
  requestId?: string
): { ok: false; rejection: JsBlockRuntimeRejection } {
  return {
    ok: false,
    rejection: {
      code: 'invalid_message',
      path,
      message,
      requestId
    }
  };
}

function createProtocolError(
  code: BlockProtocolError['code'],
  path: string,
  message: string
): BlockProtocolError {
  return { code, path, message };
}

function isRecord(value: unknown): value is RecordValue {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isPositiveNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0;
}

function isLogLevel(value: unknown): value is JsBlockWorkerLogLevel {
  return (
    value === 'debug' ||
    value === 'info' ||
    value === 'warn' ||
    value === 'error'
  );
}

function isRunErrorKind(value: unknown): value is JsBlockRunErrorKind {
  return (
    value === 'source_policy_failed' ||
    value === 'worker_startup_timeout' ||
    value === 'worker_crash' ||
    value === 'compile_failed' ||
    value === 'main_failed' ||
    value === 'effect_failed' ||
    value === 'schema_invalid' ||
    value === 'runtime_timeout' ||
    value === 'runtime_error'
  );
}

function hasOwn(record: RecordValue, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(record, key);
}
