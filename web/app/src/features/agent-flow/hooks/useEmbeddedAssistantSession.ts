import {
  cancelConsoleFlowRun,
  startConsoleAssistantRunStream,
  type ConsoleFlowDebugStreamEvent
} from '@1flowbase/api-client';
import { useCallback, useEffect, useRef, useState } from 'react';

import {
  fetchApplicationRunDebugSnapshot,
  type AgentFlowDebugMessage,
  type AgentFlowRunContext,
  type AgentFlowTraceItem
} from '../api/runtime';
import {
  applyDebugStreamEventToAssistantMessage,
  applyDebugStreamEventToTrace,
  reconcileSnapshotTraceWithLiveEvents
} from '../lib/debug-console/stream-events';
import { mapRunDetailToTrace } from '../lib/debug-console/run-detail-mapper';
import { i18nText } from '../../../shared/i18n/text';
import { useAuthStore } from '../../../state/auth-store';
import type { AgentFlowDebugSessionStatus } from './runtime/useAgentFlowDebugSession';
import {
  buildStreamEventDedupKeys,
  createRunningAssistantMessage,
  createUserMessage,
  replaceAssistantMessage,
  replaceAssistantMessageWithError
} from './runtime/debug-session-messages';

function createRunContext(applicationId: string | null): AgentFlowRunContext {
  return {
    environmentLabel: 'published',
    remembered: false,
    fields: applicationId
      ? [
          {
            nodeId: 'embedded-assistant',
            nodeLabel: 'Assistant',
            key: 'query',
            title: 'Message',
            valueType: 'string',
            value: ''
          }
        ]
      : []
  };
}

function isTraceEvent(event: ConsoleFlowDebugStreamEvent) {
  return (
    event.type === 'node_started' ||
    event.type === 'node_finished' ||
    event.type === 'text_delta' ||
    event.type === 'reasoning_delta' ||
    event.type === 'usage_snapshot' ||
    event.type === 'assistant_tool_call_started' ||
    event.type === 'assistant_tool_call_finished'
  );
}

function isTerminalEvent(event: ConsoleFlowDebugStreamEvent) {
  return (
    event.type === 'flow_finished' ||
    event.type === 'flow_incomplete' ||
    event.type === 'flow_failed' ||
    event.type === 'flow_cancelled' ||
    event.type === 'waiting_human' ||
    event.type === 'replay_expired'
  );
}

function contextTokenUsageFromSnapshot(usage: unknown) {
  if (!usage || typeof usage !== 'object' || Array.isArray(usage)) {
    return null;
  }

  const usageRecord = usage as Record<string, unknown>;
  const inputTokens = usageRecord.input_tokens ?? usageRecord.total_tokens;
  const tokenCount =
    typeof inputTokens === 'number' && Number.isFinite(inputTokens)
      ? inputTokens
      : null;

  return tokenCount === null ? null : Math.max(0, Math.round(tokenCount));
}

function contextTokenUsageFromEvent(event: ConsoleFlowDebugStreamEvent) {
  if (event.type === 'usage_snapshot') {
    return contextTokenUsageFromSnapshot(event.usage);
  }

  if (event.type === 'node_finished') {
    return (
      contextTokenUsageFromSnapshot(event.metrics_payload) ??
      contextTokenUsageFromSnapshot(event.output_payload)
    );
  }

  return null;
}

function contextTokenUsageFromTraceItems(items: AgentFlowTraceItem[]) {
  for (const item of [...items].reverse()) {
    const contextTokenUsage =
      contextTokenUsageFromSnapshot(item.metricsPayload) ??
      contextTokenUsageFromSnapshot(item.outputPayload);
    if (contextTokenUsage !== null) {
      return contextTokenUsage;
    }
  }

  return null;
}

const ASSISTANT_RUN_SNAPSHOT_POLL_INTERVAL_MS = 750;

function isTerminalFlowRunStatus(status: string) {
  return ['succeeded', 'completed', 'incomplete', 'failed', 'cancelled'].includes(
    status
  );
}

function sessionStatusFromTerminalFlowRun(
  status: string
): AgentFlowDebugSessionStatus {
  switch (status) {
    case 'succeeded':
    case 'completed':
    case 'incomplete':
      return 'completed';
    case 'failed':
      return 'failed';
    case 'cancelled':
      return 'cancelled';
    default:
      return 'running';
  }
}

export function useEmbeddedAssistantSession(applicationId: string | null) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [status, setStatus] = useState<AgentFlowDebugSessionStatus>('idle');
  const [stopping, setStopping] = useState(false);
  const [messages, setMessages] = useState<AgentFlowDebugMessage[]>([]);
  const [traceItems, setTraceItems] = useState<AgentFlowTraceItem[]>([]);
  const [contextTokenUsage, setContextTokenUsage] = useState<number | null>(
    null
  );
  const [runContext, setRunContext] = useState(() =>
    createRunContext(applicationId)
  );
  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const activeRunIdRef = useRef<string | null>(null);
  const activeApplicationIdRef = useRef<string | null>(null);
  const streamAbortControllerRef = useRef<AbortController | null>(null);
  const streamGenerationRef = useRef(0);
  const liveTraceItemsRef = useRef<AgentFlowTraceItem[]>([]);

  const cancelActiveStream = useCallback(() => {
    streamGenerationRef.current += 1;
    streamAbortControllerRef.current?.abort();
    streamAbortControllerRef.current = null;
  }, []);

  const clearSession = useCallback(() => {
    cancelActiveStream();
    activeRunIdRef.current = null;
    activeApplicationIdRef.current = null;
    setActiveRunId(null);
    setStatus('idle');
    setStopping(false);
    setMessages([]);
    setTraceItems([]);
    liveTraceItemsRef.current = [];
    setContextTokenUsage(null);
    setRunContext(createRunContext(applicationId));
  }, [applicationId, cancelActiveStream]);

  useEffect(() => {
    clearSession();
  }, [clearSession]);

  useEffect(() => cancelActiveStream, [cancelActiveStream]);

  useEffect(() => {
    if (!applicationId || !activeRunId) {
      return;
    }

    let disposed = false;
    let pollTimer: number | null = null;

    const refreshRunSnapshot = () => {
      void fetchApplicationRunDebugSnapshot(applicationId, activeRunId)
        .then((detail) => {
          if (disposed) {
            return;
          }

          const traceItems = reconcileSnapshotTraceWithLiveEvents(
            mapRunDetailToTrace(detail),
            liveTraceItemsRef.current
          );
          liveTraceItemsRef.current = traceItems;
          setTraceItems(traceItems);
          setMessages((current) =>
            current.map((message) =>
              message.role === 'assistant' && message.runId === activeRunId
                ? { ...message, traceSummary: traceItems }
                : message
            )
          );
          const contextTokenUsage = contextTokenUsageFromTraceItems(traceItems);
          if (contextTokenUsage !== null) {
            setContextTokenUsage(contextTokenUsage);
          }

          if (isTerminalFlowRunStatus(detail.flow_run.status)) {
            const nextStatus = sessionStatusFromTerminalFlowRun(
              detail.flow_run.status
            );
            setStatus(nextStatus);
            setMessages((current) =>
              current.map((message) =>
                message.role === 'assistant' && message.runId === activeRunId
                  ? { ...message, status: nextStatus, traceSummary: traceItems }
                  : message
              )
            );
            return;
          }

          pollTimer = window.setTimeout(
            refreshRunSnapshot,
            ASSISTANT_RUN_SNAPSHOT_POLL_INTERVAL_MS
          );
        })
        .catch(() => {
          if (!disposed) {
            pollTimer = window.setTimeout(
              refreshRunSnapshot,
              ASSISTANT_RUN_SNAPSHOT_POLL_INTERVAL_MS
            );
          }
        });
    };

    refreshRunSnapshot();

    return () => {
      disposed = true;
      if (pollTimer !== null) {
        window.clearTimeout(pollTimer);
      }
    };
  }, [activeRunId, applicationId]);

  const setRunContextValue = useCallback(
    (nodeId: string, key: string, value: unknown) => {
      setRunContext((current) => ({
        ...current,
        remembered: false,
        fields: current.fields.map((field) =>
          field.nodeId === nodeId && field.key === key
            ? { ...field, value }
            : field
        )
      }));
    },
    []
  );

  const submitPrompt = useCallback(
    async (prompt: string) => {
      const query = prompt.trim();
      if (
        !applicationId ||
        !csrfToken ||
        !query ||
        ['running', 'waiting_callback', 'waiting_human'].includes(status)
      ) {
        return;
      }

      const history = messages.reduce<
        Array<{ role: 'user' | 'assistant'; content: string }>
      >((entries, message) => {
        if (
          (message.role === 'user' || message.role === 'assistant') &&
          message.content.trim().length > 0
        ) {
          entries.push({ role: message.role, content: message.content });
        }
        return entries;
      }, []);
      const runningMessage = createRunningAssistantMessage();
      const streamGeneration = streamGenerationRef.current + 1;
      let streamAssistantMessage = runningMessage;
      let receivedTerminal = false;
      const seenEventKeys = new Set<string>();

      cancelActiveStream();
      streamGenerationRef.current = streamGeneration;
      activeApplicationIdRef.current = applicationId;
      setStatus('running');
      setStopping(false);
      setTraceItems([]);
      liveTraceItemsRef.current = [];
      setRunContext((current) => ({
        ...current,
        fields: current.fields.map((field) =>
          field.key === 'query' ? { ...field, value: '' } : field
        )
      }));
      setMessages((current) => [
        ...current,
        createUserMessage(query),
        runningMessage
      ]);

      try {
        await startConsoleAssistantRunStream({ query, history }, csrfToken, {
          getAbortController: (abortController) => {
            if (streamGenerationRef.current !== streamGeneration) {
              abortController.abort();
              return;
            }
            streamAbortControllerRef.current = abortController;
          },
          onEvent: (event) => {
            if (streamGenerationRef.current !== streamGeneration) {
              return;
            }
            const eventKeys = buildStreamEventDedupKeys(event);
            if (eventKeys.some((key) => seenEventKeys.has(key))) {
              return;
            }
            eventKeys.forEach((key) => seenEventKeys.add(key));

            if (
              event.type === 'flow_accepted' ||
              event.type === 'flow_started' ||
              event.type === 'flow_cancelled'
            ) {
              activeRunIdRef.current = event.run_id;
              setActiveRunId(event.run_id);
            }

            if (isTraceEvent(event)) {
              liveTraceItemsRef.current = applyDebugStreamEventToTrace(
                liveTraceItemsRef.current,
                event
              );
              setTraceItems(liveTraceItemsRef.current);
            }

            const contextTokenUsage = contextTokenUsageFromEvent(event);
            if (contextTokenUsage !== null) {
              setContextTokenUsage(contextTokenUsage);
            }

            streamAssistantMessage = applyDebugStreamEventToAssistantMessage(
              streamAssistantMessage,
              event,
              liveTraceItemsRef.current
            );
            if (isTerminalEvent(event)) {
              receivedTerminal = true;
            }
            setStatus(streamAssistantMessage.status);
            setMessages((current) =>
              replaceAssistantMessage(
                current,
                streamAssistantMessage,
                runningMessage.id
              )
            );
          }
        });

        if (
          streamGenerationRef.current === streamGeneration &&
          !receivedTerminal
        ) {
          throw new Error(
            i18nText('agentFlow', 'auto.debug_stream_connection_interrupted')
          );
        }
      } catch (error) {
        if (streamGenerationRef.current !== streamGeneration) {
          return;
        }
        const errorMessage =
          error instanceof Error
            ? error.message
            : i18nText('appShell', 'auto.assistant_run_failed');
        setStatus('failed');
        setMessages((current) =>
          replaceAssistantMessageWithError(current, errorMessage, {
            fallbackMessageId: runningMessage.id,
            runId: activeRunIdRef.current
          })
        );
      } finally {
        if (streamGenerationRef.current === streamGeneration) {
          streamAbortControllerRef.current = null;
        }
      }
    },
    [applicationId, cancelActiveStream, csrfToken, messages, status]
  );

  const stopRun = useCallback(async () => {
    const runId = activeRunIdRef.current;
    const runApplicationId = activeApplicationIdRef.current;
    if (
      !csrfToken ||
      !runId ||
      !runApplicationId ||
      !['running', 'waiting_callback', 'waiting_human'].includes(status)
    ) {
      return;
    }

    setStopping(true);
    try {
      await cancelConsoleFlowRun(runApplicationId, runId, csrfToken);
      cancelActiveStream();
      setStatus('cancelled');
      setMessages((current) =>
        current.map((message) =>
          message.role === 'assistant' &&
          (message.runId === runId || message.status === 'running')
            ? { ...message, status: 'cancelled' }
            : message
        )
      );
    } finally {
      setStopping(false);
    }
  }, [cancelActiveStream, csrfToken, status]);

  return {
    status,
    stopping,
    messages,
    traceItems,
    contextTokenUsage,
    runContext,
    activeRunId,
    clearSession,
    setRunContextValue,
    submitPrompt,
    stopRun
  };
}
