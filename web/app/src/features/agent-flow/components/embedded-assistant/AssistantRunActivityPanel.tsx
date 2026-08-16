import { Think, ThoughtChain } from '@ant-design/x';
import type { ThoughtChainItemType } from '@ant-design/x';
import { ToolOutlined } from '@ant-design/icons';
import { Alert, Divider, Empty, Spin } from 'antd';
import { type ReactNode, useEffect, useMemo, useState } from 'react';
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
import { JsonPreviewBlock } from '../../../../shared/ui/json-preview/JsonPreviewBlock';
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

const activityTimelineClassNames = {
  item: 'embedded-agent-assistant-activity__timeline-item',
  itemHeader: 'embedded-agent-assistant-activity__timeline-item-header',
  itemContent: 'embedded-agent-assistant-activity__timeline-item-content'
};

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
  const inputTitle = i18nText('agentFlow', 'auto.input');
  const outputTitle = i18nText('appShell', 'auto.assistant_activity_output');

  return (
    <div className="embedded-agent-assistant-activity__tool-detail">
      <JsonPreviewBlock
        defaultCollapsed
        displayTitle=""
        height="160px"
        title={inputTitle}
        value={entry.input ?? {}}
      />
      {entry.output !== null && entry.output !== undefined ? (
        <JsonPreviewBlock
          defaultCollapsed
          displayTitle=""
          height="160px"
          title={outputTitle}
          value={entry.output}
        />
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

function activityThoughtItem(entry: ActivityEntry): ThoughtChainItemType {
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
    collapsible: entry.kind === 'tool',
    content:
      entry.kind === 'tool' ? (
        toolDetail(entry)
      ) : (
        <DebugMarkdownContent content={entry.text ?? ''} />
      )
  };
}

function ActivitySequence({
  entries,
  reasoningCollapsible
}: {
  entries: ActivityEntry[];
  reasoningCollapsible: boolean;
}) {
  const blocks: ReactNode[] = [];
  let chainEntries: ActivityEntry[] = [];

  const flushChain = () => {
    if (chainEntries.length === 0) {
      return;
    }
    const currentEntries = chainEntries;
    chainEntries = [];
    blocks.push(
      <ThoughtChain
        classNames={activityTimelineClassNames}
        key={`chain:${currentEntries[0]?.key}`}
        items={currentEntries.map(activityThoughtItem)}
        line
        rootClassName="embedded-agent-assistant-activity__timeline"
      />
    );
  };

  entries.forEach((entry) => {
    if (entry.kind !== 'reasoning') {
      chainEntries.push(entry);
      return;
    }
    flushChain();
    blocks.push(
      reasoningCollapsible ? (
        <Think
          className="embedded-agent-assistant-activity__think"
          defaultExpanded={false}
          key={entry.key}
          title={i18nText('agentFlow', 'auto.think')}
        >
          <DebugMarkdownContent content={entry.text ?? ''} />
        </Think>
      ) : (
        <div
          className="embedded-agent-assistant-activity__reasoning-content"
          key={entry.key}
        >
          <DebugMarkdownContent content={entry.text ?? ''} />
        </div>
      )
    );
  });
  flushChain();

  return (
    <div className="embedded-agent-assistant-activity__sequence">{blocks}</div>
  );
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
  const { activity, items, failed } = useAssistantRunActivity({
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
  const terminalItems: ThoughtChainItemType[] = processEntries.length
    ? [
        {
          key: 'terminal-process',
          title: processTitle(status, activity?.durationMs ?? null),
          status: status === 'failed' ? 'error' : 'success',
          collapsible: true,
          content: (
            <ActivitySequence
              entries={processEntries}
              reasoningCollapsible={false}
            />
          )
        }
      ]
    : [];

  if (entries.length === 0 && !terminalError && !finalOutput && !failed) {
    return null;
  }

  return (
    <div className="embedded-agent-assistant-activity">
      {failed ? (
        <Alert
          showIcon
          type="error"
          title={i18nText('appShell', 'auto.assistant_activity_load_failed')}
        />
      ) : null}
      {terminal ? (
        <>
          {terminalItems.length ? (
            <ThoughtChain
              classNames={activityTimelineClassNames}
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
        <ActivitySequence entries={entries} reasoningCollapsible />
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
