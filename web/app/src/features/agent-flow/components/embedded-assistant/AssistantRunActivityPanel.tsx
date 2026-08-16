import { ThoughtChain } from '@ant-design/x';
import type { ThoughtChainItemType } from '@ant-design/x';
import { ToolOutlined } from '@ant-design/icons';
import { Alert, Divider, Empty, Spin } from 'antd';
import { useEffect, useMemo, useState } from 'react';
import {
  getConsoleAssistantRunActivity,
  normalizeConsoleRuntimeEvent,
  type ConsoleAssistantRunActivityItem,
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
  text?: string;
  segmentIndex?: number | null;
  toolCallId?: string;
  toolName?: string;
  input?: unknown;
  output?: unknown;
  durationMs?: number | null;
  status: ThoughtChainItemType['status'];
  loading?: boolean;
}

interface LoadedRunActivity {
  status: string;
  startedAt: string;
  finishedAt: string | null;
  durationMs: number | null;
  items: ConsoleAssistantRunActivityItem[];
  traceEvents: ConsoleFlowDebugStreamEvent[];
}

function eventSequence(event: ConsoleFlowDebugStreamEvent, fallback: number) {
  return event.sequence ?? fallback;
}

function liveActivityItem(
  event: ConsoleFlowDebugStreamEvent,
  fallbackSequence: number
): ConsoleAssistantRunActivityItem | null {
  const sequence = eventSequence(event, fallbackSequence);
  const common = {
    event_id: event.event_id ?? `${event.type}:${sequence}`,
    sequence,
    created_at: event.created_at ?? ''
  };
  if (event.type === 'reasoning_delta' && event.text) {
    return { ...common, kind: 'reasoning', text: event.text };
  }
  if (
    event.type === 'text_delta' &&
    event.text &&
    event.presentation?.kind === 'answer'
  ) {
    return {
      ...common,
      kind: 'output',
      text: event.text,
      segment_index: event.presentation.segment_index
    };
  }
  if (event.type === 'assistant_tool_call_started') {
    return {
      ...common,
      kind: 'tool',
      tool_call_id: event.tool_call.id,
      tool_name: event.tool_call.name,
      input: event.tool_call.arguments,
      output: null,
      duration_ms: null,
      is_error: false,
      status: 'running'
    };
  }
  if (event.type === 'assistant_tool_call_finished') {
    const isError =
      typeof event.tool_result === 'object' &&
      event.tool_result !== null &&
      'is_error' in event.tool_result &&
      event.tool_result.is_error === true;
    return {
      ...common,
      kind: 'tool',
      tool_call_id: event.tool_call.id,
      tool_name: event.tool_call.name,
      input: event.tool_call.arguments,
      output: event.tool_result,
      duration_ms: event.duration_ms,
      is_error: isError,
      status: isError ? 'failed' : 'succeeded'
    };
  }
  if (event.type === 'flow_failed') {
    return { ...common, kind: 'error', error: event.error };
  }
  return null;
}

function projectActivity(items: ConsoleAssistantRunActivityItem[]) {
  const entries: ActivityEntry[] = [];
  [...items]
    .sort((left, right) => left.sequence - right.sequence)
    .forEach((item) => {
      const previous = entries.at(-1);
      if (item.kind === 'reasoning') {
        if (previous?.kind === 'reasoning') {
          previous.text = `${previous.text ?? ''}${item.text}`;
          return;
        }
        entries.push({
          key: item.event_id,
          kind: 'reasoning',
          sequence: item.sequence,
          text: item.text,
          status: 'success'
        });
        return;
      }
      if (item.kind === 'output') {
        if (
          previous?.kind === 'output' &&
          previous.segmentIndex === item.segment_index
        ) {
          previous.text = `${previous.text ?? ''}${item.text}`;
          return;
        }
        entries.push({
          key: item.event_id,
          kind: 'output',
          sequence: item.sequence,
          text: item.text,
          segmentIndex: item.segment_index,
          status: 'success'
        });
        return;
      }
      if (item.kind === 'tool') {
        const started = [...entries]
          .reverse()
          .find(
            (entry) =>
              entry.kind === 'tool' && entry.toolCallId === item.tool_call_id
          );
        if (started) {
          started.toolName = item.tool_name;
          started.input = item.input;
          started.output = item.output;
          started.durationMs = item.duration_ms;
          started.loading = item.status === 'running';
          started.status =
            item.status === 'running'
              ? 'loading'
              : item.status === 'failed'
                ? 'error'
                : 'success';
          return;
        }
        entries.push({
          key: item.event_id,
          kind: 'tool',
          sequence: item.sequence,
          toolCallId: item.tool_call_id,
          toolName: item.tool_name,
          input: item.input,
          output: item.output,
          durationMs: item.duration_ms,
          loading: item.status === 'running',
          status:
            item.status === 'running'
              ? 'loading'
              : item.status === 'failed'
                ? 'error'
                : 'success'
        });
        return;
      }
      entries.push({
        key: item.event_id,
        kind: 'error',
        sequence: item.sequence,
        text: item.error,
        status: 'error'
      });
    });
  return entries;
}

function toolSummary(entry: ActivityEntry) {
  const input = entry.input;
  if (input && typeof input === 'object' && !Array.isArray(input)) {
    const inputRecord = input as Record<string, unknown>;
    const locator =
      typeof inputRecord.path === 'string'
        ? inputRecord.path
        : typeof inputRecord.group_id === 'string'
          ? inputRecord.group_id
          : null;
    if (locator) {
      return `${entry.toolName} (${locator})`;
    }
  }
  return entry.toolName ?? i18nText('agentFlow', 'auto.tool_call');
}

function toolDetail(entry: ActivityEntry) {
  return (
    <div className="embedded-agent-assistant-activity__tool-detail">
      <div className="embedded-agent-assistant-activity__detail-label">
        {i18nText('agentFlow', 'auto.input')}
      </div>
      <pre className="embedded-agent-assistant-activity__payload">
        {JSON.stringify(entry.input ?? {}, null, 2)}
      </pre>
      {entry.output !== null && entry.output !== undefined ? (
        <>
          <div className="embedded-agent-assistant-activity__detail-label">
            {i18nText('appShell', 'auto.assistant_activity_output')}
          </div>
          <pre className="embedded-agent-assistant-activity__payload">
            {JSON.stringify(entry.output, null, 2)}
          </pre>
        </>
      ) : null}
      {entry.durationMs !== null && entry.durationMs !== undefined ? (
        <div className="embedded-agent-assistant-activity__tool-duration">
          {i18nText('appShell', 'auto.assistant_activity_tool_duration', {
            value1: entry.durationMs
          })}
        </div>
      ) : null}
    </div>
  );
}

function activityThoughtItem(
  entry: ActivityEntry,
  reasoningCollapsible: boolean
): ThoughtChainItemType {
  return {
    key: entry.key,
    icon: entry.kind === 'tool' ? <ToolOutlined /> : undefined,
    title:
      entry.kind === 'reasoning'
        ? i18nText('agentFlow', 'auto.think')
        : entry.kind === 'tool'
          ? toolSummary(entry)
          : entry.kind === 'error'
            ? i18nText('appShell', 'auto.assistant_status_failed')
            : i18nText('appShell', 'auto.assistant_activity_output'),
    status: entry.status,
    blink: entry.loading,
    collapsible:
      entry.kind === 'tool' ||
      (entry.kind === 'reasoning' && reasoningCollapsible),
    content:
      entry.kind === 'tool' ? (
        toolDetail(entry)
      ) : (
        <DebugMarkdownContent content={entry.text ?? ''} />
      )
  };
}

async function loadDurableActivity(applicationId: string, runId: string) {
  const items: ConsoleAssistantRunActivityItem[] = [];
  const traceEvents: ConsoleFlowDebugStreamEvent[] = [];
  let afterSequence: number | undefined;

  for (;;) {
    const page = await getConsoleAssistantRunActivity(applicationId, runId, {
      ...(afterSequence === undefined ? {} : { afterSequence }),
      pageSize: 500
    });
    items.push(...page.items);
    page.trace_events.forEach((event) => {
      const normalized = normalizeConsoleRuntimeEvent(event);
      if (normalized) {
        traceEvents.push(normalized);
      }
    });
    if (!page.has_more || page.next_sequence === null) {
      return {
        status: page.status,
        startedAt: page.started_at,
        finishedAt: page.finished_at,
        durationMs: page.duration_ms,
        items,
        traceEvents
      } satisfies LoadedRunActivity;
    }
    afterSequence = page.next_sequence;
  }
}

function useAssistantRunActivity({
  applicationId,
  message
}: {
  applicationId: string;
  message: AgentFlowDebugMessage;
}) {
  const [durable, setDurable] = useState<LoadedRunActivity | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const runId = message.detailRunId ?? message.runId;

  useEffect(() => {
    if (!runId) {
      setDurable(null);
      return;
    }
    let disposed = false;
    setLoading(true);
    setFailed(false);
    void loadDurableActivity(applicationId, runId)
      .then((activity) => {
        if (!disposed) {
          setDurable(activity);
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
  }, [applicationId, message.status, runId]);

  const durableWatermark = (durable?.items ?? []).reduce(
    (watermark, item) => Math.max(watermark, item.sequence),
    -1
  );
  const liveItems = (message.activityEvents ?? [])
    .map(liveActivityItem)
    .filter((item): item is ConsoleAssistantRunActivityItem => item !== null)
    .filter((item) => item.sequence > durableWatermark);
  const itemById = new Map<string, ConsoleAssistantRunActivityItem>();
  [...(durable?.items ?? []), ...liveItems].forEach((item) => {
    itemById.set(item.event_id, item);
  });

  const traceById = new Map<string, ConsoleFlowDebugStreamEvent>();
  [...(durable?.traceEvents ?? []), ...(message.activityEvents ?? [])].forEach(
    (event, index) => {
      const key =
        event.event_id ??
        `${runId}:${eventSequence(event, index)}:${event.type}`;
      traceById.set(key, event);
    }
  );

  return {
    activity: durable,
    items: [...itemById.values()],
    traceEvents: [...traceById.values()],
    failed,
    loading
  };
}

function terminalStatus(status: string) {
  return [
    'succeeded',
    'completed',
    'incomplete',
    'failed',
    'cancelled'
  ].includes(status);
}

function formatDuration(durationMs: number | null) {
  if (durationMs === null) {
    return i18nText('appShell', 'auto.assistant_activity_duration_unknown');
  }
  const totalSeconds = Math.max(0, Math.round(durationMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return minutes > 0
    ? i18nText('appShell', 'auto.assistant_activity_duration_minutes', {
        value1: minutes,
        value2: seconds
      })
    : i18nText('appShell', 'auto.assistant_activity_duration_seconds', {
        value1: seconds
      });
}

function processTitle(status: string, durationMs: number | null) {
  const duration = formatDuration(durationMs);
  if (status === 'failed') {
    return `${duration} · ${i18nText('appShell', 'auto.assistant_status_failed')}`;
  }
  if (status === 'cancelled') {
    return `${duration} · ${i18nText('appShell', 'auto.assistant_status_cancelled')}`;
  }
  return duration;
}

export function AssistantRunTimeline({
  applicationId,
  message
}: {
  applicationId: string;
  message: AgentFlowDebugMessage;
}) {
  const { activity, items, failed, loading } = useAssistantRunActivity({
    applicationId,
    message
  });
  const entries = useMemo(() => projectActivity(items), [items]);
  const answer = parseAssistantContent(message.content).answerText;
  const status = terminalStatus(message.status)
    ? activity && terminalStatus(activity.status)
      ? activity.status
      : message.status
    : (activity?.status ?? message.status);
  const terminal = terminalStatus(status);
  let lastOutputIndex = -1;
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    if (entries[index]?.kind === 'output') {
      lastOutputIndex = index;
      break;
    }
  }
  const finalOutput =
    lastOutputIndex >= 0 ? (entries[lastOutputIndex]?.text ?? '') : answer;
  const terminalError =
    status === 'failed' && !finalOutput
      ? ([...entries].reverse().find((entry) => entry.kind === 'error')?.text ??
        i18nText('appShell', 'auto.assistant_run_failed'))
      : '';
  const processEntries =
    terminal && lastOutputIndex >= 0
      ? entries.filter((_, index) => index !== lastOutputIndex)
      : entries;
  const liveItems = entries.map((entry) => activityThoughtItem(entry, true));
  const processItems = processEntries.map((entry) =>
    activityThoughtItem(entry, false)
  );
  const terminalItems: ThoughtChainItemType[] = processItems.length
    ? [
        {
          key: 'terminal-process',
          title: processTitle(status, activity?.durationMs ?? null),
          status: status === 'failed' ? 'error' : 'success',
          collapsible: true,
          content: (
            <ThoughtChain
              items={processItems}
              line
              rootClassName="embedded-agent-assistant-activity__timeline"
            />
          )
        }
      ]
    : [];

  return (
    <div className="embedded-agent-assistant-activity">
      {failed ? (
        <Alert
          showIcon
          type="error"
          title={i18nText('appShell', 'auto.assistant_activity_load_failed')}
        />
      ) : null}
      {loading && entries.length === 0 ? <Spin /> : null}
      {!loading && entries.length === 0 && !terminalError && !finalOutput ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('appShell', 'auto.assistant_activity_empty')}
        />
      ) : terminal ? (
        <>
          {terminalItems.length ? (
            <ThoughtChain
              items={terminalItems}
              line
              rootClassName="embedded-agent-assistant-activity__timeline"
            />
          ) : null}
          {terminalItems.length && (finalOutput || terminalError) ? (
            <Divider />
          ) : null}
          {finalOutput || terminalError ? (
            <div className="embedded-agent-assistant-activity__final-output">
              <DebugMarkdownContent content={finalOutput || terminalError} />
            </div>
          ) : null}
        </>
      ) : (
        <ThoughtChain
          items={liveItems}
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
  const { traceEvents, failed, loading } = useAssistantRunActivity({
    applicationId,
    message
  });
  const durableTrace = useMemo(
    () => projectNodeTrace(traceEvents),
    [traceEvents]
  );
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
