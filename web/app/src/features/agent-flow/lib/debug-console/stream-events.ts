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
