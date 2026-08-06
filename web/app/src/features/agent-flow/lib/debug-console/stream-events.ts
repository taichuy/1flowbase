import type {
  AgentFlowDebugMessage,
  AgentFlowDebugMessageStatus,
  AgentFlowTraceItem,
  FlowDebugRunStreamEvent
} from '../../api/runtime';
import {
  appendReasoningDeltaToAssistantContent,
  appendTextDeltaToAssistantContent,
  closeOpenThinkBlock,
  parseAssistantContent
} from './assistant-content';
import { i18nText } from '../../../../shared/i18n/text';

function nowIso() {
  return new Date().toISOString();
}

function mapFlowStatus(status: string): AgentFlowDebugMessageStatus {
  switch (status) {
    case 'succeeded':
    case 'completed':
    case 'incomplete':
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

function durationMs(startedAt: string, finishedAt: string | null) {
  if (!finishedAt) {
    return null;
  }

  const started = Date.parse(startedAt);
  const finished = Date.parse(finishedAt);

  if (Number.isNaN(started) || Number.isNaN(finished)) {
    return null;
  }

  return Math.max(0, finished - started);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function upsertTraceItem(
  items: AgentFlowTraceItem[],
  nextItem: AgentFlowTraceItem
) {
  const nextItemKey = getTraceItemKey(nextItem);
  const index = items.findIndex(
    (item) => getTraceItemKey(item) === nextItemKey
  );

  if (index === -1) {
    return [...items, nextItem];
  }

  return items.map((item, itemIndex) =>
    itemIndex === index ? { ...item, ...nextItem } : item
  );
}

function appendProcessEvent(
  item: AgentFlowTraceItem,
  processEvent: Record<string, unknown>
): AgentFlowTraceItem {
  const debugPayload = isRecord(item.debugPayload) ? item.debugPayload : {};
  const providerEvents = Array.isArray(debugPayload.provider_events)
    ? debugPayload.provider_events
    : [];

  return {
    ...item,
    debugPayload: {
      ...debugPayload,
      provider_events: [...providerEvents, processEvent]
    }
  };
}

function appendProcessEventToTrace(
  items: AgentFlowTraceItem[],
  event: {
    node_run_id?: string | null;
    node_id: string;
  },
  processEvent: Record<string, unknown>
) {
  const eventKey = event.node_run_id ?? event.node_id;

  return items.map((item) => {
    const itemKey = getTraceItemKey(item);
    const matchesByKey = itemKey === eventKey;
    const matchesByNodeId = !event.node_run_id && item.nodeId === event.node_id;

    return matchesByKey || matchesByNodeId
      ? appendProcessEvent(item, processEvent)
      : item;
  });
}

function appendUsageSnapshotToTrace(
  items: AgentFlowTraceItem[],
  event: {
    node_run_id?: string | null;
    node_id: string;
    usage: unknown;
  }
) {
  const eventKey = event.node_run_id ?? event.node_id;

  return items.map((item) => {
    const itemKey = getTraceItemKey(item);
    const matchesByKey = itemKey === eventKey;
    const matchesByNodeId = !event.node_run_id && item.nodeId === event.node_id;

    if (!matchesByKey && !matchesByNodeId) {
      return item;
    }

    const outputPayload = isRecord(item.outputPayload)
      ? item.outputPayload
      : {};

    return {
      ...item,
      outputPayload: {
        ...outputPayload,
        usage: isRecord(event.usage) ? event.usage : { value: event.usage }
      }
    };
  });
}

function toolCallId(toolCall: Record<string, unknown>) {
  for (const key of ['id', 'tool_call_id', 'call_id']) {
    const value = toolCall[key];
    if (typeof value === 'string' && value.trim()) {
      return value;
    }
  }

  return null;
}

function toolResultId(toolResult: Record<string, unknown>) {
  for (const key of ['tool_call_id', 'id', 'call_id']) {
    const value = toolResult[key];
    if (typeof value === 'string' && value.trim()) {
      return value;
    }
  }

  return null;
}

function toolRoundIndex(
  rounds: Record<string, unknown>[],
  toolCallIdValue: string
) {
  return rounds.findIndex((round) => {
    const assistant = isRecord(round.assistant) ? round.assistant : {};
    const toolCalls = Array.isArray(assistant.tool_calls)
      ? assistant.tool_calls.filter(isRecord)
      : [];
    const toolResults = Array.isArray(round.tool_results)
      ? round.tool_results.filter(isRecord)
      : [];

    return (
      toolCalls.some((toolCall) => toolCallId(toolCall) === toolCallIdValue) ||
      toolResults.some((toolResult) => toolResultId(toolResult) === toolCallIdValue)
    );
  });
}

function upsertToolPayload(
  items: Record<string, unknown>[],
  nextItem: Record<string, unknown>,
  idOf: (item: Record<string, unknown>) => string | null
) {
  const nextId = idOf(nextItem);
  const index = nextId
    ? items.findIndex((item) => idOf(item) === nextId)
    : -1;

  if (index === -1) {
    return [...items, nextItem];
  }

  return items.map((item, itemIndex) =>
    itemIndex === index ? { ...item, ...nextItem } : item
  );
}

function appendAssistantToolCallToTrace(
  items: AgentFlowTraceItem[],
  event: Extract<
    FlowDebugRunStreamEvent,
    | { type: 'assistant_tool_call_started' }
    | { type: 'assistant_tool_call_finished' }
  >
) {
  const eventKey = event.node_run_id ?? event.node_id;

  return items.map((item) => {
    const itemKey = getTraceItemKey(item);
    const matchesByKey = itemKey === eventKey;
    const matchesByNodeId = !event.node_run_id && item.nodeId === event.node_id;

    if (!matchesByKey && !matchesByNodeId) {
      return item;
    }

    const debugPayload = isRecord(item.debugPayload) ? item.debugPayload : {};
    const rounds = Array.isArray(debugPayload.llm_rounds)
      ? debugPayload.llm_rounds.filter(isRecord).map((round) => ({ ...round }))
      : [];
    const currentToolCall = event.tool_call;
    const currentToolCallId = toolCallId(currentToolCall);
    const currentRoundIndex = currentToolCallId
      ? toolRoundIndex(rounds, currentToolCallId)
      : -1;
    const roundIndex =
      currentRoundIndex === -1 ? rounds.length : currentRoundIndex;
    const round = rounds[roundIndex] ?? {
      round_index: roundIndex,
      assistant: { tool_calls: [] },
      tool_results: []
    };
    const assistant = isRecord(round.assistant) ? round.assistant : {};
    const toolCalls = Array.isArray(assistant.tool_calls)
      ? assistant.tool_calls.filter(isRecord)
      : [];
    const toolResults = Array.isArray(round.tool_results)
      ? round.tool_results.filter(isRecord)
      : [];
    const nextRound: Record<string, unknown> = {
      ...round,
      assistant: {
        ...assistant,
        tool_calls: upsertToolPayload(toolCalls, currentToolCall, toolCallId)
      },
      tool_results:
        event.type === 'assistant_tool_call_finished'
          ? upsertToolPayload(
              toolResults,
              {
                ...event.tool_result,
                duration_ms: event.duration_ms,
                execution_status:
                  event.tool_result.is_error === true ? 'failed' : 'succeeded'
              },
              toolResultId
            )
          : toolResults
    };
    rounds[roundIndex] = nextRound;

    return {
      ...item,
      debugPayload: {
        ...debugPayload,
        llm_rounds: rounds
      }
    };
  });
}

function extractOutputText(output: Record<string, unknown>) {
  for (const key of ['answer', 'text', 'content', 'message']) {
    const value = output[key];

    if (typeof value === 'string' && value.trim().length > 0) {
      return value;
    }
  }

  return '';
}

function chooseFinishedDebugPayload(
  existingDebugPayload: Record<string, unknown> | undefined,
  eventDebugPayload: Record<string, unknown> | undefined
) {
  if (!eventDebugPayload || Object.keys(eventDebugPayload).length === 0) {
    return existingDebugPayload ?? {};
  }

  const existingProviderEvents = Array.isArray(
    existingDebugPayload?.provider_events
  )
    ? existingDebugPayload.provider_events
    : [];
  const eventProviderEvents = Array.isArray(eventDebugPayload.provider_events)
    ? eventDebugPayload.provider_events
    : [];

  if (eventProviderEvents.length > 0 || existingProviderEvents.length > 0) {
    return {
      ...(existingDebugPayload ?? {}),
      ...eventDebugPayload,
      provider_events:
        eventProviderEvents.length > 0
          ? eventProviderEvents
          : existingProviderEvents
    };
  }

  return {
    ...(existingDebugPayload ?? {}),
    ...eventDebugPayload
  };
}

export function applyDebugStreamEventToTrace(
  items: AgentFlowTraceItem[],
  event: FlowDebugRunStreamEvent
): AgentFlowTraceItem[] {
  if (event.type === 'node_started') {
    const startedAt = event.started_at ?? nowIso();

    return upsertTraceItem(items, {
      nodeRunId: event.node_run_id,
      nodeId: event.node_id,
      nodeAlias: event.title,
      nodeType: event.node_type,
      status: 'running',
      startedAt,
      finishedAt: null,
      durationMs: null,
      inputPayload: event.input_payload ?? {},
      outputPayload: {},
      errorPayload: null,
      metricsPayload: {},
      debugPayload: {}
    });
  }

  if (event.type === 'node_finished') {
    const existing = items.find(
      (item) => getTraceItemKey(item) === getTraceItemKeyFromFinished(event)
    );
    const startedAt = event.started_at ?? existing?.startedAt ?? nowIso();
    const finishedAt = event.finished_at ?? nowIso();

    return upsertTraceItem(items, {
      nodeRunId: event.node_run_id,
      nodeId: event.node_id,
      nodeAlias: existing?.nodeAlias ?? event.node_id,
      nodeType: existing?.nodeType ?? 'node',
      status: event.status,
      startedAt,
      finishedAt,
      durationMs: durationMs(startedAt, finishedAt),
      inputPayload: existing?.inputPayload ?? {},
      outputPayload: event.output_payload ?? {},
      errorPayload: event.error_payload ?? null,
      metricsPayload: event.metrics_payload ?? {},
      debugPayload: chooseFinishedDebugPayload(
        existing?.debugPayload,
        event.debug_payload
      )
    });
  }

  if (event.type === 'text_delta' || event.type === 'reasoning_delta') {
    if (event.presentation?.kind === 'answer') {
      return items;
    }

    return appendProcessEventToTrace(items, event, {
      type: event.type,
      text: event.text
    });
  }

  if (event.type === 'usage_snapshot') {
    return appendUsageSnapshotToTrace(items, event);
  }

  if (
    event.type === 'assistant_tool_call_started' ||
    event.type === 'assistant_tool_call_finished'
  ) {
    return appendAssistantToolCallToTrace(items, event);
  }

  return items;
}

function getTraceItemKey(item: AgentFlowTraceItem) {
  return item.nodeRunId ?? item.nodeId;
}

function getTraceItemKeyFromFinished(event: {
  node_run_id?: string | null;
  node_id: string;
}) {
  return event.node_run_id ?? event.node_id;
}

function debugPayloadWithLiveEvents(
  snapshotPayload: Record<string, unknown> | undefined,
  livePayload: Record<string, unknown> | undefined
) {
  const snapshot = snapshotPayload ?? {};
  const live = livePayload ?? {};
  const liveLlmRounds = Array.isArray(live.llm_rounds) ? live.llm_rounds : [];
  const liveProviderEvents = Array.isArray(live.provider_events)
    ? live.provider_events
    : [];

  return {
    ...snapshot,
    ...(liveLlmRounds.length > 0 ? { llm_rounds: liveLlmRounds } : {}),
    ...(liveProviderEvents.length > 0
      ? { provider_events: liveProviderEvents }
      : {})
  };
}

/**
 * A run-detail request may observe storage before a just-delivered SSE event is
 * durable. Keep the live event projection until the snapshot catches up.
 */
export function reconcileSnapshotTraceWithLiveEvents(
  snapshotItems: AgentFlowTraceItem[],
  liveItems: AgentFlowTraceItem[]
): AgentFlowTraceItem[] {
  const liveByKey = new Map(
    liveItems.map((item) => [getTraceItemKey(item), item])
  );
  const snapshotKeys = new Set<string>();
  const reconciledSnapshotItems = snapshotItems.map((snapshotItem) => {
    const key = getTraceItemKey(snapshotItem);
    snapshotKeys.add(key);
    const liveItem = liveByKey.get(key);

    if (!liveItem) {
      return snapshotItem;
    }

    return {
      ...snapshotItem,
      debugPayload: debugPayloadWithLiveEvents(
        snapshotItem.debugPayload,
        liveItem.debugPayload
      )
    };
  });

  return [
    ...reconciledSnapshotItems,
    ...liveItems.filter((item) => !snapshotKeys.has(getTraceItemKey(item)))
  ];
}

export function applyDebugStreamEventToAssistantMessage(
  message: AgentFlowDebugMessage,
  event: FlowDebugRunStreamEvent,
  traceItems: AgentFlowTraceItem[]
): AgentFlowDebugMessage {
  switch (event.type) {
    case 'flow_accepted':
      return {
        ...message,
        runId: event.run_id,
        status: 'running',
        traceSummary: traceItems
      };
    case 'flow_started':
      return {
        ...message,
        runId: event.run_id,
        status: mapFlowStatus(event.status),
        traceSummary: traceItems
      };
    case 'text_delta':
      if (event.presentation?.kind !== 'answer') {
        return message;
      }
      return {
        ...message,
        content: appendTextDeltaToAssistantContent(message.content, event.text)
      };
    case 'reasoning_delta':
      if (event.presentation?.kind !== 'answer') {
        return message;
      }
      return {
        ...message,
        content: appendReasoningDeltaToAssistantContent(
          message.content,
          event.text
        )
      };
    case 'node_started':
    case 'node_finished':
    case 'assistant_tool_call_started':
    case 'assistant_tool_call_finished':
      return {
        ...message,
        traceSummary: traceItems
      };
    case 'flow_finished': {
      const closedContent = closeOpenThinkBlock(message.content);
      const outputText = extractOutputText(event.output);
      const nextContent =
        parseAssistantContent(closedContent).answerText || !outputText
          ? closedContent
          : appendTextDeltaToAssistantContent(closedContent, outputText);

      return {
        ...message,
        runId: event.run_id,
        status: mapFlowStatus(event.status),
        content: nextContent,
        rawOutput: event.output,
        traceSummary: traceItems
      };
    }
    case 'flow_incomplete': {
      const closedContent = closeOpenThinkBlock(message.content);
      const outputText = extractOutputText(event.output);
      const nextContent =
        parseAssistantContent(closedContent).answerText || !outputText
          ? closedContent
          : appendTextDeltaToAssistantContent(closedContent, outputText);

      return {
        ...message,
        runId: event.run_id,
        status: mapFlowStatus(event.status),
        content: nextContent,
        rawOutput: event.output,
        traceSummary: traceItems
      };
    }
    case 'flow_failed':
      return {
        ...message,
        runId: event.run_id,
        status: 'failed',
        content: event.error,
        rawOutput: event.error_payload ?? null,
        traceSummary: traceItems
      };
    case 'flow_cancelled':
      return {
        ...message,
        runId: event.run_id,
        status: 'cancelled',
        traceSummary: traceItems
      };
    case 'waiting_human':
    case 'waiting_callback':
      return {
        ...message,
        runId: event.run_id,
        status: mapFlowStatus(event.status),
        traceSummary: traceItems
      };
    case 'replay_expired':
      return {
        ...message,
        status: 'failed',
        content: i18nText('agentFlow', 'auto.debug_stream_replay_expired')
      };
    default:
      return message;
  }
}
