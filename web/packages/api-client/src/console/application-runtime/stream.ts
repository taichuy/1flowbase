import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import { ApiClientError } from '../../errors';
import type {
  ConsoleAnswerPresentation,
  ConsoleFlowDebugStreamCursor,
  ConsoleFlowDebugStreamEvent,
  ConsoleFlowDebugStreamHandlers
} from './types';

function dispatchSseEvent(
  eventBuffer: string,
  handlers: ConsoleFlowDebugStreamHandlers
) {
  let eventId: string | undefined;
  let eventType: string | undefined;
  const dataLines: string[] = [];

  for (const line of eventBuffer.split(/\r?\n/)) {
    if (line.startsWith('data:')) {
      dataLines.push(line.slice(5).trimStart());
      continue;
    }

    if (line.startsWith('id:')) {
      eventId = line.slice(3).trim();
      continue;
    }

    if (line.startsWith('event:')) {
      eventType = line.slice(6).trim();
    }
  }

  if (dataLines.length === 0) {
    return;
  }

  const rawPayload = JSON.parse(dataLines.join('\n'));
  const parsedPayload = normalizeStreamPayload(rawPayload, eventType, eventId);

  if (!parsedPayload) {
    return;
  }

  handlers.onEvent(parsedPayload);
}

function normalizeStreamPayload(
  raw: unknown,
  fallbackEventType?: string,
  fallbackEventId?: string
): ConsoleFlowDebugStreamEvent | null {
  if (!isRecord(raw)) {
    return null;
  }

  const explicitType = isNonEmptyString(raw.event_type)
    ? raw.event_type
    : isNonEmptyString(raw.type)
      ? raw.type
      : undefined;
  const payloadType = isRecord(raw.payload)
    ? isNonEmptyString(raw.payload.type)
      ? String(raw.payload.type)
      : undefined
    : undefined;
  const sseEventType = isNonEmptyString(fallbackEventType)
    ? fallbackEventType
    : undefined;
  const eventType = explicitType ?? payloadType ?? sseEventType;

  if (!eventType || !isKnownStreamEventType(eventType)) {
    return null;
  }

  if (isRecord(raw.payload) && isConsoleFlowDebugEventEnvelope(raw)) {
    return normalizeFromEnvelope(raw, eventType, fallbackEventId);
  }

  if (!isNonEmptyString(raw.type)) {
    return null;
  }

  const legacyEvent = raw as ConsoleFlowDebugStreamEvent;
  if (fallbackEventId && !isNonEmptyString(raw.event_id)) {
    return {
      ...legacyEvent,
      event_id: fallbackEventId
    };
  }

  return legacyEvent;
}

function normalizeFromEnvelope(
  raw: Record<string, unknown>,
  eventType: ConsoleFlowDebugStreamEvent['type'],
  fallbackEventId?: string
): ConsoleFlowDebugStreamEvent | null {
  const payload = isRecord(raw.payload) ? raw.payload : {};
  const contentType: 'text' | 'reasoning' | null | undefined =
    raw.content_type === 'text' || raw.content_type === 'reasoning'
      ? raw.content_type
      : raw.content_type === null
        ? null
        : undefined;
  const base: {
    event_id?: string;
    run_id?: string;
    sequence?: number;
    created_at?: string;
    delta_index?: number | null;
    content_type?: 'text' | 'reasoning' | null;
    node_run_id?: string | null;
  } = {
    event_id: toOptionalString(raw.event_id) ?? fallbackEventId,
    run_id: toOptionalString(raw.run_id),
    sequence: typeof raw.sequence === 'number' ? raw.sequence : undefined,
    created_at: toOptionalString(raw.created_at),
    delta_index:
      raw.delta_index === null || typeof raw.delta_index === 'number'
        ? raw.delta_index
        : undefined,
    content_type: contentType,
    node_run_id: normalizeNullableString(raw.node_run_id)
  };

  const nodeId = toOptionalString(payload.node_id) ?? '';
  const output = isRecord(payload.output) ? payload.output : {};
  const errorPayload = isRecord(payload.error_payload)
    ? payload.error_payload
    : payload.error_payload === null
      ? null
      : undefined;
  const answerPresentation = normalizeAnswerPresentation(payload.presentation);

  if (eventType === 'flow_accepted') {
    return {
      ...base,
      type: 'flow_accepted',
      run_id: base.run_id ?? '',
      status: isNonEmptyString(payload.status)
        ? String(payload.status)
        : 'running'
    };
  }

  if (eventType === 'flow_started') {
    return {
      ...base,
      type: 'flow_started',
      run_id: base.run_id ?? '',
      status: isNonEmptyString(payload.status)
        ? String(payload.status)
        : 'running'
    };
  }

  if (eventType === 'node_started') {
    return {
      ...base,
      type: 'node_started',
      run_id: base.run_id,
      node_run_id: base.node_run_id ?? '',
      node_id: nodeId,
      node_type: isNonEmptyString(payload.node_type)
        ? String(payload.node_type)
        : 'node',
      title: isNonEmptyString(payload.title) ? String(payload.title) : 'node',
      input_payload: isRecord(payload.input_payload)
        ? payload.input_payload
        : {},
      started_at: toOptionalString(payload.started_at)
    };
  }

  if (eventType === 'node_finished') {
    return {
      ...base,
      type: 'node_finished',
      run_id: base.run_id,
      node_run_id: base.node_run_id ?? '',
      node_id: nodeId,
      status: isNonEmptyString(payload.status)
        ? String(payload.status)
        : 'succeeded',
      output_payload: isRecord(payload.output_payload)
        ? payload.output_payload
        : {},
      error_payload: errorPayload,
      metrics_payload: isRecord(payload.metrics_payload)
        ? payload.metrics_payload
        : {},
      debug_payload: isRecord(payload.debug_payload)
        ? payload.debug_payload
        : {},
      started_at: toOptionalString(payload.started_at),
      finished_at:
        payload.finished_at === null
          ? null
          : toOptionalString(payload.finished_at)
    };
  }

  if (eventType === 'text_delta') {
    return {
      ...base,
      type: 'text_delta',
      run_id: base.run_id,
      node_run_id: base.node_run_id,
      node_id: nodeId,
      text:
        toOptionalString(raw.text) ??
        toOptionalString(payload.text) ??
        toOptionalString(payload.delta) ??
        '',
      ...(answerPresentation ? { presentation: answerPresentation } : {})
    };
  }

  if (eventType === 'reasoning_delta') {
    return {
      ...base,
      type: 'reasoning_delta',
      run_id: base.run_id,
      node_run_id: base.node_run_id,
      node_id: nodeId,
      text:
        toOptionalString(raw.text) ??
        toOptionalString(payload.text) ??
        toOptionalString(payload.delta) ??
        '',
      ...(answerPresentation ? { presentation: answerPresentation } : {})
    };
  }

  if (eventType === 'usage_snapshot') {
    return {
      ...base,
      type: 'usage_snapshot',
      run_id: base.run_id,
      node_run_id: base.node_run_id,
      node_id: nodeId,
      usage: payload.usage
    };
  }

  if (eventType === 'assistant_tool_call_started') {
    return {
      ...base,
      type: 'assistant_tool_call_started',
      run_id: base.run_id,
      node_run_id: base.node_run_id ?? '',
      node_id: nodeId,
      tool_call: isRecord(payload.tool_call) ? payload.tool_call : {}
    };
  }

  if (eventType === 'assistant_tool_call_finished') {
    return {
      ...base,
      type: 'assistant_tool_call_finished',
      run_id: base.run_id,
      node_run_id: base.node_run_id ?? '',
      node_id: nodeId,
      tool_call: isRecord(payload.tool_call) ? payload.tool_call : {},
      tool_result: isRecord(payload.tool_result) ? payload.tool_result : {},
      duration_ms:
        typeof payload.duration_ms === 'number' &&
        Number.isFinite(payload.duration_ms)
          ? payload.duration_ms
          : 0
    };
  }

  if (eventType === 'flow_finished') {
    return {
      ...base,
      type: 'flow_finished',
      run_id: base.run_id ?? '',
      status: isNonEmptyString(payload.status)
        ? String(payload.status)
        : 'completed',
      output
    };
  }

  if (eventType === 'flow_incomplete') {
    return {
      ...base,
      type: 'flow_incomplete',
      run_id: base.run_id ?? '',
      status: isNonEmptyString(payload.status)
        ? String(payload.status)
        : 'incomplete',
      reason: toOptionalString(payload.reason),
      output
    };
  }

  if (eventType === 'flow_failed') {
    return {
      ...base,
      type: 'flow_failed',
      run_id: base.run_id ?? '',
      error: isNonEmptyString(payload.error)
        ? String(payload.error)
        : 'stream error',
      error_payload: errorPayload
    };
  }

  if (eventType === 'flow_cancelled') {
    return {
      ...base,
      type: 'flow_cancelled',
      run_id: base.run_id ?? '',
      status: isNonEmptyString(payload.status)
        ? String(payload.status)
        : 'cancelled',
      reason: toOptionalString(payload.reason),
      manual_stop: payload.manual_stop === true
    };
  }

  if (eventType === 'waiting_human') {
    return {
      ...base,
      type: 'waiting_human',
      run_id: base.run_id ?? '',
      node_run_id: base.node_run_id,
      node_id: nodeId || undefined,
      status: 'waiting_human'
    };
  }

  if (eventType === 'waiting_callback') {
    return {
      ...base,
      type: 'waiting_callback',
      run_id: base.run_id ?? '',
      node_run_id: base.node_run_id,
      node_id: nodeId || undefined,
      status: 'waiting_callback'
    };
  }

  if (eventType === 'heartbeat') {
    return {
      ...base,
      type: 'heartbeat'
    };
  }

  if (eventType === 'replay_expired') {
    return {
      ...base,
      type: 'replay_expired',
      run_id: base.run_id ?? toOptionalString(payload.run_id) ?? '',
      from_sequence:
        typeof payload.from_sequence === 'number'
          ? payload.from_sequence
          : typeof raw.from_sequence === 'number'
            ? raw.from_sequence
            : undefined,
      reason:
        toOptionalString(payload.reason) ??
        toOptionalString(raw.reason) ??
        'cursor_expired'
    };
  }

  return null;
}

function isKnownStreamEventType(
  type: string
): type is ConsoleFlowDebugStreamEvent['type'] {
  return (
    type === 'flow_accepted' ||
    type === 'flow_started' ||
    type === 'node_started' ||
    type === 'node_finished' ||
    type === 'text_delta' ||
    type === 'reasoning_delta' ||
    type === 'usage_snapshot' ||
    type === 'assistant_tool_call_started' ||
    type === 'assistant_tool_call_finished' ||
    type === 'flow_finished' ||
    type === 'flow_incomplete' ||
    type === 'flow_failed' ||
    type === 'flow_cancelled' ||
    type === 'waiting_human' ||
    type === 'waiting_callback' ||
    type === 'heartbeat' ||
    type === 'replay_expired'
  );
}

function isConsoleFlowDebugEventEnvelope(raw: Record<string, unknown>) {
  return typeof raw.event_type === 'string' && isRecord(raw.payload);
}

function normalizeNullableString(value: unknown): string | null | undefined {
  if (value === null) {
    return null;
  }

  return toOptionalString(value);
}

function normalizeAnswerPresentation(
  value: unknown
): ConsoleAnswerPresentation | undefined {
  if (!isRecord(value)) {
    return undefined;
  }

  const answerNodeId = toOptionalString(value.answer_node_id);
  const segmentIndex = value.segment_index;
  if (
    value.kind !== 'answer' ||
    !answerNodeId ||
    typeof segmentIndex !== 'number' ||
    !Number.isInteger(segmentIndex) ||
    segmentIndex < 0
  ) {
    return undefined;
  }

  return {
    kind: 'answer',
    answer_node_id: answerNodeId,
    segment_index: segmentIndex,
    source_node_id: toOptionalString(value.source_node_id),
    source_node_run_id: toOptionalString(value.source_node_run_id),
    source_output_key: toOptionalString(value.source_output_key)
  };
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function toOptionalString(value: unknown): string | undefined {
  return isNonEmptyString(value) ? value : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

export async function consumeConsoleRuntimeEventStream(
  response: Response,
  handlers: ConsoleFlowDebugStreamHandlers
) {
  const reader = response.body?.getReader();

  if (!reader) {
    handlers.onCompleted?.();
    return;
  }

  const decoder = new TextDecoder();
  let buffer = '';

  while (true) {
    const { value, done } = await reader.read();

    if (done) {
      if (buffer.trim().length > 0) {
        dispatchSseEvent(buffer, handlers);
      }
      handlers.onCompleted?.();
      return;
    }

    buffer += decoder.decode(value, { stream: true });
    const eventFrames = buffer.split(/\r?\n\r?\n/);
    buffer = eventFrames.pop() ?? '';

    for (const eventFrame of eventFrames) {
      dispatchSseEvent(eventFrame, handlers);
    }
  }
}

export async function startConsoleFlowDebugRunStream(
  applicationId: string,
  input: {
    input_payload: Record<string, unknown>;
    document?: FlowAuthoringDocument;
    debug_session_id?: string;
  },
  csrfToken: string,
  handlers: ConsoleFlowDebugStreamHandlers,
  options?: {
    cursor?: ConsoleFlowDebugStreamCursor;
    baseUrl?: string;
  }
) {
  const abortController = new AbortController();
  handlers.getAbortController?.(abortController);
  const baseUrl = options?.baseUrl;
  const query = buildStreamCursorQuery(options?.cursor);

  const response = await fetch(
    `${baseUrl ?? ''}/api/console/applications/${applicationId}/orchestration/debug-runs/stream${query}`,
    {
      method: 'POST',
      credentials: 'include',
      signal: abortController.signal,
      headers: {
        accept: 'text/event-stream',
        'content-type': 'application/json',
        'x-csrf-token': csrfToken
      },
      body: JSON.stringify(input)
    }
  );

  if (!response.ok) {
    throw await ApiClientError.fromResponse(response);
  }

  await consumeConsoleRuntimeEventStream(response, handlers);
}

export async function subscribeConsoleFlowDebugRunStream(
  applicationId: string,
  runId: string,
  csrfToken: string,
  handlers: ConsoleFlowDebugStreamHandlers,
  options?: {
    cursor?: ConsoleFlowDebugStreamCursor;
    baseUrl?: string;
  }
) {
  const abortController = new AbortController();
  handlers.getAbortController?.(abortController);
  const baseUrl = options?.baseUrl;
  const query = buildStreamCursorQuery(options?.cursor);

  const response = await fetch(
    `${baseUrl ?? ''}/api/console/applications/${applicationId}/orchestration/runs/${runId}/debug-stream${query}`,
    {
      method: 'GET',
      credentials: 'include',
      signal: abortController.signal,
      headers: {
        accept: 'text/event-stream',
        'x-csrf-token': csrfToken
      }
    }
  );

  if (!response.ok) {
    throw await ApiClientError.fromResponse(response);
  }

  await consumeConsoleRuntimeEventStream(response, handlers);
}

function buildStreamCursorQuery(cursor?: ConsoleFlowDebugStreamCursor) {
  if (!cursor) {
    return '';
  }

  const params = new URLSearchParams();
  if (typeof cursor.from_sequence === 'number') {
    params.set('from_sequence', String(cursor.from_sequence));
  }
  if (cursor.last_event_id) {
    params.set('last_event_id', cursor.last_event_id);
  }

  const query = params.toString();
  return query ? `?${query}` : '';
}
