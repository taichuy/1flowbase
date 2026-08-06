import type {
  AgentFlowAnswerSnapshot,
  AgentFlowDebugMessage,
  AgentFlowDebugMessageStatus,
  AgentFlowTraceItem,
  FlowDebugRunStreamEvent,
  FlowDebugRunDetail
} from '../../api/runtime';
import {
  appendReasoningDeltaToAssistantContent,
  appendTextDeltaToAssistantContent,
  closeOpenThinkBlock,
  parseAssistantContent
} from './assistant-content';
import { applyDebugStreamEventToTrace } from './stream-events';

function summarizePayload(payload: Record<string, unknown> | null | undefined) {
  if (!payload || Object.keys(payload).length === 0) {
    return '';
  }

  return JSON.stringify(payload, null, 2);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function isRuntimeDebugArtifactPreview(value: unknown): value is {
  __runtime_debug_artifact: true;
  preview: string;
} {
  return (
    isRecord(value) &&
    value.__runtime_debug_artifact === true &&
    typeof value.preview === 'string'
  );
}

function extractDeltaText(payload: unknown): string {
  if (!isRecord(payload)) {
    return '';
  }

  for (const key of ['text', 'delta']) {
    const value = payload[key];

    if (typeof value === 'string') {
      return value;
    }
  }

  return '';
}

function collectDeltaEvents(
  detail: FlowDebugRunDetail,
  eventType: 'text_delta' | 'reasoning_delta'
): string | null {
  const text = detail.events
    .filter((event) => event.event_type === eventType)
    .sort((left, right) => left.sequence - right.sequence)
    .map((event) => extractDeltaText(event.payload))
    .join('');

  return text.trim().length > 0 ? text : null;
}

function collectTextDeltaEvents(detail: FlowDebugRunDetail): string | null {
  return collectDeltaEvents(detail, 'text_delta');
}

function collectOrderedAssistantContentEvents(
  detail: FlowDebugRunDetail
): string | null {
  const content = detail.events
    .filter(
      (event) =>
        event.event_type === 'text_delta' ||
        event.event_type === 'reasoning_delta'
    )
    .sort((left, right) => left.sequence - right.sequence)
    .reduce((currentContent, event) => {
      const text = extractDeltaText(event.payload);

      return event.event_type === 'reasoning_delta'
        ? appendReasoningDeltaToAssistantContent(currentContent, text)
        : appendTextDeltaToAssistantContent(currentContent, text);
    }, '');

  const closedContent = closeOpenThinkBlock(content);

  return closedContent.trim().length > 0 ? closedContent : null;
}

function findPreferredOutputText(payload: unknown): string | null {
  if (!isRecord(payload)) {
    return null;
  }

  for (const key of ['answer', 'text', 'content', 'message']) {
    const value = payload[key];

    if (typeof value === 'string' && value.trim().length > 0) {
      return value;
    }

    if (isRuntimeDebugArtifactPreview(value)) {
      return value.preview.trim().length > 0 ? value.preview : null;
    }
  }

  if (isRecord(payload.error)) {
    const message = payload.error.message;

    if (typeof message === 'string' && message.trim().length > 0) {
      return message;
    }
  }

  return null;
}

function findAnswerNodeOutputText(detail: FlowDebugRunDetail): string | null {
  for (const nodeRun of [...detail.node_runs].reverse()) {
    if (nodeRun.node_type !== 'answer') {
      continue;
    }

    const outputText = findPreferredOutputText(nodeRun.output_payload);

    if (outputText) {
      return outputText;
    }
  }

  return null;
}

function mapAnswerSnapshot(
  detail: FlowDebugRunDetail
): AgentFlowAnswerSnapshot | null {
  const snapshot =
    detail.answer_snapshot ?? detail.detail?.answer_snapshot ?? null;

  if (!snapshot) {
    return null;
  }

  return {
    kind: snapshot.kind,
    text: snapshot.text,
    outputPayload: snapshot.output_payload,
    complete: snapshot.complete,
    materializedFrom: snapshot.materialized_from,
    answerNodeId: snapshot.answer_node_id,
    answerNodeRunId: snapshot.answer_node_run_id,
    waitingNodeId: snapshot.waiting_node_id ?? null,
    waitingNodeRunId: snapshot.waiting_node_run_id ?? null
  };
}

function answerSnapshotBelongsToNodeRun(
  nodeRun: FlowDebugRunDetail['node_runs'][number],
  answerSnapshot: AgentFlowAnswerSnapshot | null
) {
  if (!answerSnapshot) {
    return false;
  }

  if (
    answerSnapshot.waitingNodeRunId &&
    nodeRun.id === answerSnapshot.waitingNodeRunId
  ) {
    return true;
  }

  return Boolean(
    answerSnapshot.waitingNodeId &&
    nodeRun.node_id === answerSnapshot.waitingNodeId
  );
}

function mapMessageStatus(status: string): AgentFlowDebugMessageStatus {
  switch (status) {
    case 'succeeded':
      return 'completed';
    case 'waiting_callback':
      return 'waiting_callback';
    case 'waiting_human':
      return 'waiting_human';
    case 'cancelled':
      return 'cancelled';
    case 'failed':
      return 'failed';
    default:
      return 'running';
  }
}

function traceItemDurationMs({
  finished_at,
  started_at
}: {
  finished_at: string | null;
  started_at: string;
}) {
  return finished_at
    ? Math.max(
        new Date(finished_at).getTime() - new Date(started_at).getTime(),
        0
      )
    : null;
}

function nodeRunDebugPayload(nodeRun: FlowDebugRunDetail['node_runs'][number]) {
  return isRecord(nodeRun.debug_payload) ? nodeRun.debug_payload : {};
}

function optionalStringField(record: Record<string, unknown>, key: string) {
  const value = record[key];
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}

function assistantToolEventFromRunEvent(
  event: FlowDebugRunDetail['events'][number]
): Extract<
  FlowDebugRunStreamEvent,
  | { type: 'assistant_tool_call_started' }
  | { type: 'assistant_tool_call_finished' }
> | null {
  if (
    event.event_type !== 'assistant_tool_call_started' &&
    event.event_type !== 'assistant_tool_call_finished'
  ) {
    return null;
  }

  const payload = event.payload;
  const nodeRunId =
    event.node_run_id ?? optionalStringField(payload, 'node_run_id');
  const nodeId = optionalStringField(payload, 'node_id');
  const toolCall = isRecord(payload.tool_call) ? payload.tool_call : null;

  if (!nodeRunId || !nodeId || !toolCall) {
    return null;
  }

  const base = {
    event_id: `persisted:${event.sequence}`,
    run_id: event.flow_run_id,
    sequence: event.sequence,
    created_at: event.created_at,
    node_run_id: nodeRunId,
    node_id: nodeId,
    tool_call: toolCall
  };

  if (event.event_type === 'assistant_tool_call_started') {
    return { ...base, type: 'assistant_tool_call_started' };
  }

  const toolResult = isRecord(payload.tool_result) ? payload.tool_result : null;
  if (!toolResult) {
    return null;
  }

  return {
    ...base,
    type: 'assistant_tool_call_finished',
    tool_result: toolResult,
    duration_ms:
      typeof payload.duration_ms === 'number' &&
      Number.isFinite(payload.duration_ms)
        ? payload.duration_ms
        : 0
  };
}

function mapNodeRunToTraceItem({
  answerSnapshot,
  debugPayload,
  nodeRun
}: {
  answerSnapshot?: AgentFlowAnswerSnapshot;
  debugPayload?: Record<string, unknown>;
  nodeRun: FlowDebugRunDetail['node_runs'][number];
}): AgentFlowTraceItem {
  return {
    nodeRunId: nodeRun.id,
    nodeId: nodeRun.node_id,
    nodeAlias: nodeRun.node_alias,
    nodeType: nodeRun.node_type,
    status: nodeRun.status,
    startedAt: nodeRun.started_at,
    finishedAt: nodeRun.finished_at,
    durationMs: traceItemDurationMs(nodeRun),
    inputPayload: nodeRun.input_payload,
    outputPayload: nodeRun.output_payload,
    errorPayload: nodeRun.error_payload,
    metricsPayload: nodeRun.metrics_payload,
    debugPayload: debugPayload ?? nodeRunDebugPayload(nodeRun),
    answerSnapshot
  };
}

function stitchedTraceDebugPayload({
  nodeRun,
  trace
}: {
  nodeRun: FlowDebugRunDetail['node_runs'][number];
  trace: NonNullable<FlowDebugRunDetail['stitched_trace']>[number];
}) {
  const callbackTaskIds = trace.callback_tasks
    .filter((task) => task.node_run_id === nodeRun.id)
    .map((task) => task.id);

  return {
    ...nodeRunDebugPayload(nodeRun),
    stitched_trace_source: {
      source_flow_run_id: trace.source_flow_run.id,
      source_node_run_id: nodeRun.id,
      callback_task_ids: callbackTaskIds
    }
  };
}

function mapStitchedTraceToTraceItems(
  detail: FlowDebugRunDetail
): AgentFlowTraceItem[] {
  const stitchedTrace =
    detail.stitched_trace ?? detail.detail?.stitched_trace ?? [];

  return stitchedTrace.flatMap((trace) =>
    trace.node_runs.map((nodeRun) =>
      mapNodeRunToTraceItem({
        debugPayload: stitchedTraceDebugPayload({ nodeRun, trace }),
        nodeRun
      })
    )
  );
}

export function mapRunDetailToTrace(
  detail: FlowDebugRunDetail
): AgentFlowTraceItem[] {
  const answerSnapshot = mapAnswerSnapshot(detail);

  const currentTraceItems = detail.node_runs.map((nodeRun) => {
    const nodeAnswerSnapshot =
      answerSnapshot && answerSnapshotBelongsToNodeRun(nodeRun, answerSnapshot)
        ? answerSnapshot
        : undefined;

    return mapNodeRunToTraceItem({
      answerSnapshot: nodeAnswerSnapshot,
      nodeRun
    });
  });

  const traceWithToolEvents = detail.events
    .slice()
    .sort((left, right) => left.sequence - right.sequence)
    .reduce((items, event) => {
      const toolEvent = assistantToolEventFromRunEvent(event);
      return toolEvent ? applyDebugStreamEventToTrace(items, toolEvent) : items;
    }, currentTraceItems);

  return [...traceWithToolEvents, ...mapStitchedTraceToTraceItems(detail)];
}

export function extractAssistantOutputText(detail: FlowDebugRunDetail): string {
  if (
    detail.flow_run.status === 'waiting_human' ||
    detail.flow_run.status === 'waiting_callback' ||
    detail.flow_run.status === 'cancelled'
  ) {
    return '';
  }

  const streamingText = collectTextDeltaEvents(detail);

  if (streamingText && detail.flow_run.status === 'running') {
    return streamingText;
  }

  const outputPayloadText = findPreferredOutputText(
    detail.flow_run.output_payload
  );

  if (outputPayloadText) {
    return outputPayloadText;
  }

  const answerNodeOutputText = findAnswerNodeOutputText(detail);
  if (answerNodeOutputText) {
    return answerNodeOutputText;
  }

  if (detail.flow_run.error_payload) {
    return summarizePayload(detail.flow_run.error_payload);
  }

  return '';
}

function extractAssistantContent(detail: FlowDebugRunDetail): string {
  const orderedStreamContent = collectOrderedAssistantContentEvents(detail);

  if (orderedStreamContent) {
    const orderedParsedContent = parseAssistantContent(orderedStreamContent);

    if (orderedParsedContent.answerText.trim().length > 0) {
      return orderedStreamContent;
    }

    const outputText = extractAssistantOutputText(detail);

    if (!outputText) {
      return orderedStreamContent;
    }

    const outputParsedContent = parseAssistantContent(outputText);

    if (outputParsedContent.answerText.trim().length > 0) {
      return outputText;
    }

    if (outputParsedContent.reasoningText.trim().length === 0) {
      return appendTextDeltaToAssistantContent(
        orderedStreamContent,
        outputText
      );
    }

    return orderedStreamContent;
  }

  return extractAssistantOutputText(detail);
}

export function mapRunDetailToConversation(
  detail: FlowDebugRunDetail
): AgentFlowDebugMessage {
  const traceItems = mapRunDetailToTrace(detail);
  const rawOutput =
    Object.keys(detail.flow_run.output_payload).length > 0
      ? detail.flow_run.output_payload
      : null;

  return {
    id: `assistant-${detail.flow_run.id}`,
    role: 'assistant',
    content: extractAssistantContent(detail),
    status: mapMessageStatus(detail.flow_run.status),
    runId: detail.flow_run.id,
    rawOutput,
    statistics: detail.statistics,
    traceSummary: traceItems.slice(0, 3)
  };
}
