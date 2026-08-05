import {
  cancelConsoleFlowRun,
  startConsoleAssistantRunStream,
  type ConsoleFlowDebugStreamEvent
} from '@1flowbase/api-client';
import { useCallback, useEffect, useRef, useState } from 'react';

import type {
  AgentFlowDebugMessage,
  AgentFlowRunContext,
  AgentFlowTraceItem
} from '../api/runtime';
import {
  applyDebugStreamEventToAssistantMessage,
  applyDebugStreamEventToTrace
} from '../lib/debug-console/stream-events';
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
    event.type === 'usage_snapshot'
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

export function useEmbeddedAssistantSession(applicationId: string | null) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [status, setStatus] = useState<AgentFlowDebugSessionStatus>('idle');
  const [stopping, setStopping] = useState(false);
  const [messages, setMessages] = useState<AgentFlowDebugMessage[]>([]);
  const [traceItems, setTraceItems] = useState<AgentFlowTraceItem[]>([]);
  const [runContext, setRunContext] = useState(() =>
    createRunContext(applicationId)
  );
  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const activeRunIdRef = useRef<string | null>(null);
  const activeApplicationIdRef = useRef<string | null>(null);
  const streamAbortControllerRef = useRef<AbortController | null>(null);
  const streamGenerationRef = useRef(0);

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
    setRunContext(createRunContext(applicationId));
  }, [applicationId, cancelActiveStream]);

  useEffect(() => {
    clearSession();
  }, [clearSession]);

  useEffect(() => cancelActiveStream, [cancelActiveStream]);

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
      let streamTraceItems: AgentFlowTraceItem[] = [];
      let receivedTerminal = false;
      const seenEventKeys = new Set<string>();

      cancelActiveStream();
      streamGenerationRef.current = streamGeneration;
      activeApplicationIdRef.current = applicationId;
      setStatus('running');
      setStopping(false);
      setTraceItems([]);
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
              streamTraceItems = applyDebugStreamEventToTrace(
                streamTraceItems,
                event
              );
              setTraceItems(streamTraceItems);
            }

            streamAssistantMessage = applyDebugStreamEventToAssistantMessage(
              streamAssistantMessage,
              event,
              streamTraceItems
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
    runContext,
    activeRunId,
    clearSession,
    setRunContextValue,
    submitPrompt,
    stopRun
  };
}
