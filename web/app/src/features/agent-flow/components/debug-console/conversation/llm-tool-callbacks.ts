export interface LlmToolCallback {
  key: string;
  id: string;
  name: string;
  callbackStatus: 'returned' | 'waiting_callback' | 'cancelled';
  executionStatus:
    | 'succeeded'
    | 'intercepted'
    | 'failed'
    | 'timed_out'
    | 'cancelled'
    | 'unknown';
  requestPayload: Record<string, unknown>;
  callbackPayload: Record<string, unknown> | null;
  parsedResult: Record<string, unknown> | null;
  requestRoundIndex: number | null;
  resultRoundIndex: number | null;
  call_usage: Record<string, unknown> | null;
  result_context_usage: Record<string, unknown> | null;
  duration_ms: number | null;
  routeTrace: LlmToolRouteTraceSummary | null;
  detailArtifactRef?: string | null;
  detailRef?: string | null;
}

export interface LlmToolRouteTraceSummary {
  detailArtifactRef: string | null;
  rawPayload: Record<string, unknown>;
  routeKind: string | null;
  status: string | null;
  routeModel: string | null;
  targetNodeId: string | null;
  routeNodeId: string | null;
  routeNodeAlias: string | null;
  returnedToMain: boolean;
  mainResume: boolean;
  routeOutputSummary: Record<string, unknown> | null;
  finalOutputSummary: Record<string, unknown> | null;
  branchCount: number | null;
  branchSummaries: LlmToolRouteBranchSummary[];
  branchTraces: LlmToolRouteBranchTrace[];
  fanIn: Record<string, unknown> | null;
}

export interface LlmToolRouteBranchSummary {
  nodeId: string | null;
  nodeAlias: string | null;
  nodeType: string | null;
  status: string | null;
  routeModel: string | null;
  outputSummary: Record<string, unknown> | null;
  rawPayload: Record<string, unknown>;
}

export interface LlmToolRouteBranchTrace extends LlmToolRouteBranchSummary {
  inputPayload: Record<string, unknown>;
  debugPayload: Record<string, unknown>;
  outputPayload: Record<string, unknown>;
  metricsPayload: Record<string, unknown>;
  debugPayloadRef: string | null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function firstRecordField(
  record: Record<string, unknown>,
  keys: string[]
): Record<string, unknown> {
  return optionalRecordField(record, keys) ?? {};
}

function optionalRecordField(
  record: Record<string, unknown>,
  keys: string[]
): Record<string, unknown> | null {
  for (const key of keys) {
    const value = record[key];

    if (isRecord(value)) {
      return value;
    }
  }

  return null;
}

function firstStringField(
  record: Record<string, unknown>,
  keys: string[]
): string | null {
  for (const key of keys) {
    const value = record[key];

    if (typeof value === 'string' && value.trim().length > 0) {
      return value;
    }
  }

  return null;
}

function optionalNumberField(
  record: Record<string, unknown>,
  keys: string[]
): number | null {
  for (const key of keys) {
    const value = record[key];

    if (typeof value === 'number' && Number.isFinite(value)) {
      return value;
    }
  }

  return null;
}

function optionalBooleanField(
  record: Record<string, unknown>,
  keys: string[]
): boolean {
  for (const key of keys) {
    const value = record[key];

    if (typeof value === 'boolean') {
      return value;
    }
  }

  return false;
}

function recordArray(value: unknown): Record<string, unknown>[] {
  return Array.isArray(value) ? value.filter(isRecord) : [];
}

function readRouteTraceSummary(value: unknown): {
  id: string;
  trace: LlmToolRouteTraceSummary;
} | null {
  if (!isRecord(value)) {
    return null;
  }
  const kind = firstStringField(value, ['kind', 'preview_kind']);
  const id = firstStringField(value, ['tool_call_id', 'id', 'call_id']);

  if (kind !== 'visible_internal_llm_tool_trace' || !id) {
    return null;
  }

  return {
    id,
    trace: {
      detailArtifactRef: firstStringField(value, [
        'artifact_ref',
        'detail_artifact_ref'
      ]),
      rawPayload: value,
      routeKind: firstStringField(value, ['route_kind']),
      status: firstStringField(value, ['status']),
      routeModel: firstStringField(value, ['route_model']),
      targetNodeId: firstStringField(value, ['target_node_id']),
      routeNodeId: firstStringField(value, ['route_node_id']),
      routeNodeAlias: firstStringField(value, ['route_node_alias']),
      returnedToMain: optionalBooleanField(value, ['returned_to_main']),
      mainResume: optionalBooleanField(value, ['main_resume']),
      routeOutputSummary: optionalRecordField(value, ['route_output_summary']),
      finalOutputSummary: optionalRecordField(value, ['final_output_summary']),
      branchCount: optionalNumberField(value, ['branch_count']),
      branchSummaries: recordArray(value.branch_summaries).map(
        readRouteBranchSummary
      ),
      branchTraces: recordArray(value.branch_traces).map(readRouteBranchTrace),
      fanIn: optionalRecordField(value, ['fan_in'])
    }
  };
}

function readRouteBranchSummary(
  value: Record<string, unknown>
): LlmToolRouteBranchSummary {
  return {
    nodeId: firstStringField(value, ['node_id']),
    nodeAlias: firstStringField(value, ['node_alias']),
    nodeType: firstStringField(value, ['node_type']),
    status: firstStringField(value, ['status']),
    routeModel: firstStringField(value, ['route_model']),
    outputSummary: optionalRecordField(value, ['output_summary']),
    rawPayload: value
  };
}

function readRouteBranchTrace(
  value: Record<string, unknown>
): LlmToolRouteBranchTrace {
  return {
    ...readRouteBranchSummary(value),
    inputPayload: firstRecordField(value, ['input_payload']),
    debugPayload: firstRecordField(value, ['debug_payload']),
    outputPayload: firstRecordField(value, ['output_payload']),
    metricsPayload: firstRecordField(value, ['metrics_payload']),
    debugPayloadRef: firstStringField(value, ['debug_payload_ref'])
  };
}

export function readLlmToolRouteTraceDetail(
  loadedPayload: unknown
): LlmToolRouteTraceSummary | null {
  return readRouteTraceSummary(loadedPayload)?.trace ?? null;
}

function readVisibleInternalRouteTraces(
  debugPayload: unknown
): Array<{ id: string; trace: LlmToolRouteTraceSummary }> {
  if (!isRecord(debugPayload)) {
    return [];
  }

  return recordArray(debugPayload.visible_internal_llm_tool_trace)
    .map(readRouteTraceSummary)
    .filter(
      (trace): trace is { id: string; trace: LlmToolRouteTraceSummary } =>
        trace !== null
    );
}

function roundIndex(round: Record<string, unknown>, fallbackIndex: number) {
  const value = round.round_index;

  if (typeof value === 'number' && Number.isFinite(value)) {
    return value;
  }

  return fallbackIndex;
}

function readLlmRoundsFromDebugPayload(
  debugPayload: unknown
): Record<string, unknown>[] {
  if (!isRecord(debugPayload)) {
    return [];
  }

  return recordArray(debugPayload.llm_rounds);
}

function callbackStatusValue(
  value: unknown
): LlmToolCallback['callbackStatus'] {
  return value === 'returned' || value === 'cancelled'
    ? value
    : 'waiting_callback';
}

function executionStatusValue(
  value: unknown
): LlmToolCallback['executionStatus'] {
  return value === 'succeeded' ||
    value === 'intercepted' ||
    value === 'failed' ||
    value === 'timed_out' ||
    value === 'cancelled' ||
    value === 'unknown'
    ? value
    : 'unknown';
}

function nullableRoundIndex(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function readIndexedToolCallback(
  toolCallback: Record<string, unknown>,
  index: number
): LlmToolCallback {
  const id =
    firstStringField(toolCallback, ['id', 'tool_call_id', 'call_id']) ??
    `tool-callback-${index + 1}`;

  return {
    key: `${id}-${index}`,
    id,
    name: firstStringField(toolCallback, ['name']) ?? 'Tool',
    callbackStatus: callbackStatusValue(toolCallback.callback_status),
    executionStatus: executionStatusValue(toolCallback.execution_status),
    requestPayload: {},
    callbackPayload: null,
    parsedResult: null,
    requestRoundIndex: nullableRoundIndex(toolCallback.request_round_index),
    resultRoundIndex: nullableRoundIndex(toolCallback.result_round_index),
    call_usage: optionalRecordField(toolCallback, ['call_usage']),
    result_context_usage: optionalRecordField(toolCallback, [
      'result_context_usage'
    ]),
    duration_ms: optionalNumberField(toolCallback, ['duration_ms']),
    routeTrace: readRouteTraceSummary(toolCallback.route_trace)?.trace ?? null,
    detailArtifactRef: firstStringField(toolCallback, [
      'artifact_ref',
      'detail_artifact_ref'
    ]),
    detailRef: firstStringField(toolCallback, ['detail_ref'])
  };
}

function readIndexedToolCallbacks(debugPayload: unknown): LlmToolCallback[] {
  if (!isRecord(debugPayload)) {
    return [];
  }

  const rootCallbacks = recordArray(debugPayload.tool_callbacks);
  const llmRoundsCallbacks = isRecord(debugPayload.llm_rounds)
    ? recordArray(debugPayload.llm_rounds.tool_callbacks)
    : [];

  return [...rootCallbacks, ...llmRoundsCallbacks].map(readIndexedToolCallback);
}

export function readLlmToolCallbackDetail(
  loadedPayload: unknown
): Omit<LlmToolCallback, 'key'> | null {
  if (!isRecord(loadedPayload)) {
    return null;
  }

  const id = firstStringField(loadedPayload, ['id', 'tool_call_id', 'call_id']);

  if (!id) {
    return null;
  }

  return {
    id,
    name: firstStringField(loadedPayload, ['name']) ?? 'Tool',
    callbackStatus: callbackStatusValue(loadedPayload.callback_status),
    executionStatus: executionStatusValue(loadedPayload.execution_status),
    requestPayload:
      optionalRecordField(loadedPayload, [
        'request_payload',
        'requestPayload'
      ]) ?? {},
    callbackPayload: optionalRecordField(loadedPayload, [
      'callback_payload',
      'callbackPayload'
    ]),
    parsedResult: optionalRecordField(loadedPayload, [
      'parsed_result',
      'parsedResult'
    ]),
    requestRoundIndex: nullableRoundIndex(loadedPayload.request_round_index),
    resultRoundIndex: nullableRoundIndex(loadedPayload.result_round_index),
    call_usage: optionalRecordField(loadedPayload, ['call_usage']),
    result_context_usage: optionalRecordField(loadedPayload, [
      'result_context_usage'
    ]),
    duration_ms: optionalNumberField(loadedPayload, ['duration_ms']),
    routeTrace: readRouteTraceSummary(loadedPayload.route_trace)?.trace ?? null,
    detailArtifactRef:
      firstStringField(loadedPayload, [
        'artifact_ref',
        'detail_artifact_ref'
      ]) ?? null,
    detailRef: firstStringField(loadedPayload, ['detail_ref'])
  };
}

export function stripLlmRoundsFromDebugPayload(debugPayload: unknown) {
  if (
    !isRecord(debugPayload) ||
    (!Object.prototype.hasOwnProperty.call(debugPayload, 'llm_rounds') &&
      !Object.prototype.hasOwnProperty.call(debugPayload, 'tool_callbacks'))
  ) {
    return debugPayload;
  }

  return Object.fromEntries(
    Object.entries(debugPayload).filter(
      ([key]) => key !== 'llm_rounds' && key !== 'tool_callbacks'
    )
  );
}

function readRoundToolCalls(round: Record<string, unknown>) {
  const assistant = firstRecordField(round, ['assistant', 'assistant_message']);
  const assistantToolCalls = recordArray(assistant.tool_calls);

  if (assistantToolCalls.length > 0) {
    return assistantToolCalls;
  }

  return recordArray(round.tool_calls);
}

function readRoundToolResults(round: Record<string, unknown>) {
  return recordArray(round.tool_results);
}

function toolCallId(
  toolCall: Record<string, unknown>,
  roundNumber: number,
  toolCallIndex: number
) {
  return (
    firstStringField(toolCall, ['id', 'tool_call_id', 'call_id']) ??
    `tool-${roundNumber + 1}-${toolCallIndex + 1}`
  );
}

function toolResultId(
  toolResult: Record<string, unknown>,
  roundNumber: number,
  toolResultIndex: number
) {
  return (
    firstStringField(toolResult, ['tool_call_id', 'id', 'call_id']) ??
    `tool-result-${roundNumber + 1}-${toolResultIndex + 1}`
  );
}

function callbackStatus(
  callbackPayload: Record<string, unknown> | null
): LlmToolCallback['callbackStatus'] {
  return callbackPayload ? 'returned' : 'waiting_callback';
}

function normalizedExecutionStatus(
  status: unknown
): LlmToolCallback['executionStatus'] | null {
  if (typeof status !== 'string') {
    return null;
  }

  if (status === 'canceled') {
    return 'cancelled';
  }

  return status === 'succeeded' ||
    status === 'intercepted' ||
    status === 'failed' ||
    status === 'timed_out' ||
    status === 'cancelled' ||
    status === 'unknown'
    ? status
    : null;
}

function executionStatusFromCallbackPayload(
  callbackPayload: Record<string, unknown> | null
): LlmToolCallback['executionStatus'] {
  if (!callbackPayload) {
    return 'unknown';
  }

  const execution = isRecord(callbackPayload.execution)
    ? callbackPayload.execution
    : null;
  const executionStatus =
    normalizedExecutionStatus(execution?.status) ??
    normalizedExecutionStatus(callbackPayload.execution_status);

  if (executionStatus) {
    return executionStatus;
  }
  if (callbackPayload.timed_out === true) {
    return 'timed_out';
  }
  if (callbackPayload.cancelled === true) {
    return 'cancelled';
  }
  if (typeof callbackPayload.exit_code === 'number') {
    return callbackPayload.exit_code === 0 ? 'succeeded' : 'failed';
  }
  if (typeof callbackPayload.http_status === 'number') {
    return callbackPayload.http_status >= 200 &&
      callbackPayload.http_status < 300
      ? 'succeeded'
      : 'failed';
  }
  if (
    callbackPayload.is_error === true ||
    (Object.prototype.hasOwnProperty.call(callbackPayload, 'error') &&
      callbackPayload.error !== null &&
      callbackPayload.error !== undefined)
  ) {
    return 'failed';
  }

  return 'unknown';
}

function parsedResultFromCallbackPayload(
  callbackPayload: Record<string, unknown> | null
): Record<string, unknown> | null {
  if (!callbackPayload) {
    return null;
  }

  const parsedEntries = [
    'tool_call_id',
    'id',
    'call_id',
    'name',
    'content',
    'stdout',
    'stderr',
    'error',
    'exit_code',
    'http_status',
    'is_error',
    'timed_out',
    'cancelled',
    'execution',
    'execution_status'
  ]
    .flatMap((key) => Object.prototype.hasOwnProperty.call(callbackPayload, key)
      ? [[key, callbackPayload[key]] as const]
      : []);

  return Object.fromEntries(parsedEntries);
}

export function collectLlmToolCallbacks(
  debugPayload: unknown
): LlmToolCallback[] {
  return collectLlmToolCallbacksFromDebugPayloads([debugPayload]);
}

export function collectLlmToolCallbacksFromDebugPayloads(
  debugPayloads: unknown[]
): LlmToolCallback[] {
  const routeTracesById = new Map<string, LlmToolRouteTraceSummary>();
  for (const payload of debugPayloads) {
    for (const { id, trace } of readVisibleInternalRouteTraces(payload)) {
      routeTracesById.set(id, trace);
    }
  }
  const callbacks = mergeLlmToolCallbacks([
    ...debugPayloads.flatMap(readIndexedToolCallbacks),
    ...collectLlmToolCallbacksFromRounds(
      debugPayloads.flatMap(readLlmRoundsFromDebugPayload)
    )
  ]);

  return callbacks.map((callback) => ({
    ...callback,
    routeTrace: routeTracesById.get(callback.id) ?? callback.routeTrace
  }));
}

function mergeLlmToolCallbacks(callbacks: LlmToolCallback[]) {
  const merged: LlmToolCallback[] = [];
  const callbackIndexById = new Map<string, number>();

  for (const callback of callbacks) {
    const callbackIndex = callbackIndexById.get(callback.id);

    if (callbackIndex === undefined) {
      callbackIndexById.set(callback.id, merged.length);
      merged.push(callback);
      continue;
    }

    const currentCallback = merged[callbackIndex];
    const requestPayload =
      Object.keys(callback.requestPayload).length > 0
        ? callback.requestPayload
        : currentCallback.requestPayload;
    const callbackPayload =
      callback.callbackPayload ?? currentCallback.callbackPayload;
    const parsedResult =
      callback.parsedResult ??
      currentCallback.parsedResult ??
      parsedResultFromCallbackPayload(callbackPayload);

    merged[callbackIndex] = {
      ...currentCallback,
      name:
        currentCallback.name === 'Tool' && callback.name !== 'Tool'
          ? callback.name
          : currentCallback.name,
      callbackStatus:
        callback.callbackStatus !== 'waiting_callback'
          ? callback.callbackStatus
          : currentCallback.callbackStatus !== 'waiting_callback'
            ? currentCallback.callbackStatus
            : callbackStatus(callbackPayload),
      executionStatus:
        callback.executionStatus !== 'unknown'
          ? callback.executionStatus
          : currentCallback.executionStatus !== 'unknown'
            ? currentCallback.executionStatus
            : executionStatusFromCallbackPayload(callbackPayload),
      requestPayload,
      callbackPayload,
      parsedResult,
      requestRoundIndex:
        callback.requestRoundIndex ?? currentCallback.requestRoundIndex,
      resultRoundIndex:
        callback.resultRoundIndex ?? currentCallback.resultRoundIndex,
      detailArtifactRef:
        callback.detailArtifactRef ?? currentCallback.detailArtifactRef,
      detailRef: callback.detailRef ?? currentCallback.detailRef,
      call_usage: callback.call_usage ?? currentCallback.call_usage,
      result_context_usage:
        callback.result_context_usage ?? currentCallback.result_context_usage,
      duration_ms: callback.duration_ms ?? currentCallback.duration_ms,
      routeTrace: callback.routeTrace ?? currentCallback.routeTrace
    };
  }

  return merged;
}

function collectLlmToolCallbacksFromRounds(
  rounds: Record<string, unknown>[]
): LlmToolCallback[] {
  const callbacks: LlmToolCallback[] = [];
  const callbackIndexById = new Map<string, number>();

  const upsertCallback = (
    id: string,
    nextCallback: Omit<
      LlmToolCallback,
      'key' | 'callbackStatus' | 'executionStatus' | 'parsedResult'
    >
  ) => {
    const callbackIndex = callbackIndexById.get(id);

    if (callbackIndex === undefined) {
      callbackIndexById.set(id, callbacks.length);
      callbacks.push({
        ...nextCallback,
        key: `${id}-${callbacks.length}`,
        callbackStatus: callbackStatus(nextCallback.callbackPayload),
        executionStatus: executionStatusFromCallbackPayload(
          nextCallback.callbackPayload
        ),
        parsedResult: parsedResultFromCallbackPayload(
          nextCallback.callbackPayload
        )
      });
      return;
    }

    const currentCallback = callbacks[callbackIndex];
    const requestPayload =
      Object.keys(nextCallback.requestPayload).length > 0
        ? nextCallback.requestPayload
        : currentCallback.requestPayload;
    const callbackPayload =
      nextCallback.callbackPayload ?? currentCallback.callbackPayload;
    const parsedResult =
      currentCallback.parsedResult ??
      parsedResultFromCallbackPayload(callbackPayload);

    callbacks[callbackIndex] = {
      ...currentCallback,
      name:
        currentCallback.name === 'Tool' && nextCallback.name !== 'Tool'
          ? nextCallback.name
          : currentCallback.name,
      requestPayload,
      callbackPayload,
      parsedResult,
      requestRoundIndex:
        nextCallback.requestRoundIndex ?? currentCallback.requestRoundIndex,
      resultRoundIndex:
        nextCallback.resultRoundIndex ?? currentCallback.resultRoundIndex,
      callbackStatus: callbackStatus(callbackPayload),
      executionStatus: executionStatusFromCallbackPayload(callbackPayload),
      call_usage: nextCallback.call_usage ?? currentCallback.call_usage,
      result_context_usage:
        nextCallback.result_context_usage ??
        currentCallback.result_context_usage,
      duration_ms: nextCallback.duration_ms ?? currentCallback.duration_ms,
      routeTrace: nextCallback.routeTrace ?? currentCallback.routeTrace
    };
  };

  rounds.forEach((round, fallbackRoundIndex) => {
    const currentRoundIndex = roundIndex(round, fallbackRoundIndex);
    const currentUsage = optionalRecordField(round, ['usage']);

    readRoundToolCalls(round).forEach((toolCall, toolCallIndex) => {
      const id = toolCallId(toolCall, currentRoundIndex, toolCallIndex);

      upsertCallback(id, {
        id,
        name: firstStringField(toolCall, ['name']) ?? 'Tool',
        requestPayload: toolCall,
        callbackPayload: null,
        requestRoundIndex: currentRoundIndex,
        resultRoundIndex: null,
        call_usage:
          optionalRecordField(toolCall, ['call_usage']) ?? currentUsage,
        result_context_usage: null,
        duration_ms: optionalNumberField(toolCall, ['duration_ms']),
        routeTrace: readRouteTraceSummary(toolCall.route_trace)?.trace ?? null
      });
    });

    readRoundToolResults(round).forEach((toolResult, toolResultIndex) => {
      const id = toolResultId(toolResult, currentRoundIndex, toolResultIndex);

      upsertCallback(id, {
        id,
        name: firstStringField(toolResult, ['name']) ?? 'Tool',
        requestPayload: {},
        callbackPayload: toolResult,
        requestRoundIndex: null,
        resultRoundIndex: currentRoundIndex,
        call_usage: optionalRecordField(toolResult, ['call_usage']),
        result_context_usage: optionalRecordField(toolResult, [
          'result_context_usage'
        ]),
        duration_ms: optionalNumberField(toolResult, ['duration_ms']),
        routeTrace: readRouteTraceSummary(toolResult.route_trace)?.trace ?? null
      });
    });
  });

  return callbacks;
}
