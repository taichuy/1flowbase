import { Think, ThoughtChain } from '@ant-design/x';
import type { ThoughtChainItemType } from '@ant-design/x';
import { ToolOutlined } from '@ant-design/icons';
import { Alert, Empty, Spin } from 'antd';
import { useEffect, useMemo, useState } from 'react';
import {
  getConsoleAssistantRunActivity,
  normalizeConsoleRuntimeEvent,
  type ConsoleFlowDebugStreamEvent
} from '@1flowbase/api-client';

import type {
  AgentFlowDebugMessage,
  AgentFlowTraceItem
} from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';
import { parseAssistantContent } from '../../lib/debug-console/assistant-content';
import {
  applyDebugStreamEventToTrace,
  reconcileSnapshotTraceWithLiveEvents
} from '../../lib/debug-console/stream-events';
import { DebugMarkdownContent } from '../debug-console/conversation/DebugMarkdownContent';
import { DebugWorkflowProcess } from '../debug-console/conversation/DebugWorkflowProcess';

interface ActivityEntry {
  key: string;
  kind: 'reasoning' | 'tool' | 'output' | 'error';
  sequence: number;
  title: string;
  text?: string;
  detail?: unknown;
  status: ThoughtChainItemType['status'];
  loading?: boolean;
  toolCallId?: string | null;
}

function eventSequence(event: ConsoleFlowDebugStreamEvent, fallback: number) {
  return event.sequence ?? fallback;
}

function toolCallField(toolCall: Record<string, unknown>, key: string) {
  const value = toolCall[key];
  return typeof value === 'string' && value.trim() ? value : null;
}

function toolCallId(toolCall: Record<string, unknown>) {
  return (
    toolCallField(toolCall, 'id') ??
    toolCallField(toolCall, 'tool_call_id') ??
    toolCallField(toolCall, 'call_id')
  );
}

function toolCallName(toolCall: Record<string, unknown>) {
  const direct = toolCallField(toolCall, 'name');
  const callable = toolCall.function;
  const nested =
    callable && typeof callable === 'object' && !Array.isArray(callable)
      ? toolCallField(callable as Record<string, unknown>, 'name')
      : null;
  return direct ?? nested ?? i18nText('agentFlow', 'auto.tool_call');
}

function projectActivity(
  events: ConsoleFlowDebugStreamEvent[],
  fallbackAnswer: string
) {
  const ordered = events
    .map((event, index) => ({ event, sequence: eventSequence(event, index) }))
    .sort((left, right) => left.sequence - right.sequence);
  const entries: ActivityEntry[] = [];

  ordered.forEach(({ event, sequence }, index) => {
    if (event.type === 'reasoning_delta' && event.text) {
      const previous = entries.at(-1);
      if (previous?.kind === 'reasoning') {
        previous.text = `${previous.text ?? ''}${event.text}`;
        previous.sequence = sequence;
        return;
      }
      entries.push({
        key: event.event_id ?? `reasoning-${sequence}-${index}`,
        kind: 'reasoning',
        sequence,
        title: i18nText('agentFlow', 'auto.think'),
        text: event.text,
        status: 'success'
      });
      return;
    }
    if (
      event.type === 'text_delta' &&
      event.text &&
      event.presentation?.kind === 'answer'
    ) {
      const previous = entries.at(-1);
      if (previous?.kind === 'output') {
        previous.text = `${previous.text ?? ''}${event.text}`;
        previous.sequence = sequence;
        return;
      }
      entries.push({
        key: event.event_id ?? `output-${sequence}-${index}`,
        kind: 'output',
        sequence,
        title: i18nText('appShell', 'auto.assistant_activity_output'),
        text: event.text,
        status: 'success'
      });
      return;
    }
    if (event.type === 'assistant_tool_call_started') {
      const id = toolCallId(event.tool_call);
      entries.push({
        key: event.event_id ?? `tool-${id ?? sequence}-${index}`,
        kind: 'tool',
        sequence,
        title: toolCallName(event.tool_call),
        detail: { tool_call: event.tool_call },
        status: 'loading',
        loading: true,
        toolCallId: id
      });
      return;
    }
    if (event.type === 'assistant_tool_call_finished') {
      const id = toolCallId(event.tool_call);
      const started = [...entries]
        .reverse()
        .find((entry) => entry.kind === 'tool' && entry.toolCallId === id);
      if (started) {
        started.detail = {
          tool_call: event.tool_call,
          tool_result: event.tool_result,
          duration_ms: event.duration_ms
        };
        started.status = 'success';
        started.loading = false;
        return;
      }
      entries.push({
        key: event.event_id ?? `tool-${id ?? sequence}-${index}`,
        kind: 'tool',
        sequence,
        title: toolCallName(event.tool_call),
        detail: {
          tool_call: event.tool_call,
          tool_result: event.tool_result,
          duration_ms: event.duration_ms
        },
        status: 'success',
        toolCallId: id
      });
      return;
    }
    if (event.type === 'flow_failed') {
      entries.push({
        key: event.event_id ?? `error-${sequence}-${index}`,
        kind: 'error',
        sequence,
        title: i18nText('appShell', 'auto.assistant_status_failed'),
        text: event.error,
        status: 'error'
      });
    }
  });

  const projectedOutput = entries
    .filter((entry) => entry.kind === 'output')
    .map((entry) => entry.text ?? '')
    .join('');
  const missingOutput = fallbackAnswer.startsWith(projectedOutput)
    ? fallbackAnswer.slice(projectedOutput.length)
    : projectedOutput
      ? ''
      : fallbackAnswer;
  if (missingOutput) {
    entries.push({
      key: `output-fallback-${ordered.at(-1)?.sequence ?? 0}`,
      kind: 'output',
      sequence: (ordered.at(-1)?.sequence ?? 0) + 1,
      title: i18nText('appShell', 'auto.assistant_activity_output'),
      text: missingOutput,
      status: 'success'
    });
  }

  return entries;
}

async function loadDurableActivity(applicationId: string, runId: string) {
  const events: ConsoleFlowDebugStreamEvent[] = [];
  let afterSequence: number | undefined;

  for (;;) {
    const page = await getConsoleAssistantRunActivity(applicationId, runId, {
      ...(afterSequence === undefined ? {} : { afterSequence }),
      pageSize: 500
    });
    page.items.forEach((item) => {
      const event = normalizeConsoleRuntimeEvent(item);
      if (event) {
        events.push(event);
      }
    });
    if (!page.has_more || page.next_sequence === null) {
      return events;
    }
    afterSequence = page.next_sequence;
  }
}

function useAssistantRunEvents({
  applicationId,
  message
}: {
  applicationId: string;
  message: AgentFlowDebugMessage;
}) {
  const [durableEvents, setDurableEvents] = useState<
    ConsoleFlowDebugStreamEvent[]
  >([]);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const runId = message.detailRunId ?? message.runId;

  useEffect(() => {
    if (!runId) {
      setDurableEvents([]);
      return;
    }
    let disposed = false;
    setLoading(true);
    setFailed(false);
    void loadDurableActivity(applicationId, runId)
      .then((events) => {
        if (!disposed) {
          setDurableEvents(events);
        }
      })
      .catch(() => {
        if (!disposed) {
          setFailed(true);
        }
      })
      .finally(() => {
        if (!disposed) {
          setLoading(false);
        }
      });
    return () => {
      disposed = true;
    };
  }, [applicationId, runId]);

  const events = useMemo(() => {
    const byKey = new Map<string, ConsoleFlowDebugStreamEvent>();
    const durableWatermark = durableEvents.reduce(
      (watermark, event, index) =>
        Math.max(watermark, eventSequence(event, index)),
      -1
    );
    const liveTail = (message.activityEvents ?? []).filter(
      (event, index) => eventSequence(event, index) > durableWatermark
    );
    [...durableEvents, ...liveTail].forEach((event, index) => {
      const key =
        event.event_id ??
        `${'run_id' in event ? event.run_id : runId}:${eventSequence(event, index)}:${event.type}`;
      byKey.set(key, event);
    });
    return [...byKey.values()];
  }, [durableEvents, message.activityEvents, runId]);

  return { events, failed, loading };
}

export function AssistantRunTimeline({
  applicationId,
  message
}: {
  applicationId: string;
  message: AgentFlowDebugMessage;
}) {
  const { events, failed, loading } = useAssistantRunEvents({
    applicationId,
    message
  });
  const answer = parseAssistantContent(message.content).answerText;
  const activity = useMemo(
    () => projectActivity(events, answer),
    [answer, events]
  );
  const thoughtItems = useMemo<ThoughtChainItemType[]>(
    () =>
      activity.map((entry) => ({
        key: entry.key,
        icon: entry.kind === 'tool' ? <ToolOutlined /> : undefined,
        title: entry.title,
        status: entry.status,
        blink: entry.loading,
        // Reasoning and final output form the default narrative. Only the raw
        // tool payload is secondary detail that should start collapsed.
        collapsible: entry.kind === 'tool',
        content:
          entry.kind === 'reasoning' ? (
            <Think title={entry.title} loading={entry.loading} defaultExpanded>
              <DebugMarkdownContent content={entry.text ?? ''} />
            </Think>
          ) : entry.kind === 'tool' ? (
            <pre className="embedded-agent-assistant-activity__payload">
              {JSON.stringify(entry.detail, null, 2)}
            </pre>
          ) : (
            <DebugMarkdownContent content={entry.text ?? ''} />
          )
      })),
    [activity]
  );

  return (
    <div className="embedded-agent-assistant-activity">
      {failed ? (
        <Alert
          showIcon
          type="error"
          title={i18nText('appShell', 'auto.assistant_activity_load_failed')}
        />
      ) : null}
      {loading && events.length === 0 ? <Spin /> : null}
      {!loading && thoughtItems.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('appShell', 'auto.assistant_activity_empty')}
        />
      ) : (
        <ThoughtChain
          items={thoughtItems}
          line
          rootClassName="embedded-agent-assistant-activity__timeline"
        />
      )}
    </div>
  );
}

function projectNodeTrace(events: ConsoleFlowDebugStreamEvent[]) {
  return events.reduce<AgentFlowTraceItem[]>(
    (items, event) => applyDebugStreamEventToTrace(items, event),
    []
  );
}

export function AssistantRunNodePanel({
  applicationId,
  message
}: {
  applicationId: string;
  message: AgentFlowDebugMessage;
}) {
  const { events, failed, loading } = useAssistantRunEvents({
    applicationId,
    message
  });
  const durableTrace = useMemo(() => projectNodeTrace(events), [events]);
  const traceItems = useMemo(
    () =>
      reconcileSnapshotTraceWithLiveEvents(durableTrace, message.traceSummary),
    [durableTrace, message.traceSummary]
  );

  return (
    <div className="embedded-agent-assistant-node-panel">
      {failed ? (
        <Alert
          showIcon
          type="error"
          title={i18nText('appShell', 'auto.assistant_activity_load_failed')}
        />
      ) : null}
      {loading && traceItems.length === 0 ? <Spin /> : null}
      {!loading && traceItems.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('appShell', 'auto.assistant_activity_empty')}
        />
      ) : (
        <DebugWorkflowProcess items={traceItems} />
      )}
    </div>
  );
}
