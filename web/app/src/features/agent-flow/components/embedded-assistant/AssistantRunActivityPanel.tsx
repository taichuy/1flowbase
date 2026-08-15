import { Think, ThoughtChain } from '@ant-design/x';
import type { ThoughtChainItemType } from '@ant-design/x';
import {
  CheckCircleFilled,
  CloseCircleFilled,
  LoadingOutlined,
  ToolOutlined
} from '@ant-design/icons';
import { Alert, Empty, Spin, Typography } from 'antd';
import { useEffect, useMemo, useState } from 'react';
import {
  getConsoleAssistantRunActivity,
  normalizeConsoleRuntimeEvent,
  type ConsoleFlowDebugStreamEvent
} from '@1flowbase/api-client';

import type { AgentFlowDebugMessage } from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';
import { DebugMarkdownContent } from '../debug-console/conversation/DebugMarkdownContent';

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

interface NodeProgress {
  key: string;
  title: string;
  status: string;
  sequence: number;
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

function projectActivity(events: ConsoleFlowDebugStreamEvent[]) {
  const ordered = events
    .map((event, index) => ({ event, sequence: eventSequence(event, index) }))
    .sort((left, right) => left.sequence - right.sequence);
  const entries: ActivityEntry[] = [];
  const nodes = new Map<string, NodeProgress>();

  ordered.forEach(({ event, sequence }, index) => {
    if (event.type === 'node_started') {
      nodes.set(event.node_run_id || event.node_id, {
        key: event.node_run_id || event.node_id,
        title: event.title || event.node_id,
        status: 'running',
        sequence
      });
      return;
    }
    if (event.type === 'node_finished') {
      const key = event.node_run_id || event.node_id;
      const current = nodes.get(key);
      nodes.set(key, {
        key,
        title: current?.title ?? event.node_id,
        status: event.status,
        sequence
      });
      return;
    }
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

  const nodeProgress = [...nodes.values()].sort(
    (left, right) => left.sequence - right.sequence
  );
  const currentNode =
    [...nodeProgress].reverse().find((node) => node.status === 'running') ??
    nodeProgress.at(-1) ??
    null;
  return { currentNode, entries };
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

export function AssistantRunActivityPanel({
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
  const activity = useMemo(() => projectActivity(events), [events]);
  const thoughtItems = useMemo<ThoughtChainItemType[]>(
    () =>
      activity.entries.map((entry) => ({
        key: entry.key,
        icon: entry.kind === 'tool' ? <ToolOutlined /> : undefined,
        title: entry.title,
        status: entry.status,
        blink: entry.loading,
        collapsible: true,
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
    [activity.entries]
  );

  return (
    <div className="embedded-agent-assistant-activity">
      {activity.currentNode ? (
        <section className="embedded-agent-assistant-activity__current-node">
          <span className="embedded-agent-assistant-activity__node-icon">
            {activity.currentNode.status === 'running' ? (
              <LoadingOutlined spin />
            ) : activity.currentNode.status === 'failed' ? (
              <CloseCircleFilled />
            ) : (
              <CheckCircleFilled />
            )}
          </span>
          <span>
            <Typography.Text type="secondary">
              {activity.currentNode.status === 'running'
                ? i18nText('appShell', 'auto.assistant_activity_current_node')
                : i18nText('appShell', 'auto.assistant_activity_last_node')}
            </Typography.Text>
            <Typography.Text strong>
              {activity.currentNode.title}
            </Typography.Text>
          </span>
        </section>
      ) : null}
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
