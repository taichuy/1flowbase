import {
  attachConsoleAssistantRunWebSocket,
  cancelConsoleFlowRun,
  createConsoleAssistantConversation,
  getConsoleAssistantConversationMessages,
  getConsoleAssistantLegacySnapshotMessages,
  startConsoleAssistantRunWebSocket,
  startConsoleAssistantRunStream,
  type ConsoleAssistantConversationMessage,
  type ConsoleAssistantClientTools,
  type ConsoleAssistantPageReference,
  type ConsoleAssistantWebSocketControl,
  type ConsoleContextSnapshot,
  type ConsoleFlowDebugStreamEvent,
  type ConsoleFlowDebugStreamHandlers
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
import {
  mapRunDetailToConversation,
  mapRunDetailToTrace
} from '../lib/debug-console/run-detail-mapper';
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
    event.type === 'replay_expired' ||
    event.type === 'replay_gap'
  );
}

function isTerminalFlowRunStatus(status: string) {
  return [
    'succeeded',
    'completed',
    'incomplete',
    'failed',
    'cancelled'
  ].includes(status);
}

function sessionStatusFromTerminalFlowRun(
  status: string
): AgentFlowDebugMessage['status'] {
  switch (status) {
    case 'waiting_callback':
      return 'waiting_callback';
    case 'waiting_human':
      return 'waiting_human';
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

function hasActiveAssistantRun(status: AgentFlowDebugSessionStatus) {
  return ['running', 'waiting_callback', 'waiting_human'].includes(status);
}

function sessionStatusFromFlowRun(
  status: string | null | undefined
): AgentFlowDebugSessionStatus {
  switch (status) {
    case 'waiting_callback':
      return 'waiting_callback';
    case 'waiting_human':
      return 'waiting_human';
    case 'succeeded':
    case 'completed':
    case 'incomplete':
      return 'completed';
    case 'failed':
      return 'failed';
    case 'cancelled':
      return 'cancelled';
    case 'queued':
    case 'running':
    case 'paused':
      return 'running';
    default:
      return 'idle';
  }
}

function restoredMessages(
  items: ConsoleAssistantConversationMessage[]
): AgentFlowDebugMessage[] {
  return items.map((item) => ({
    id: item.id,
    role: item.role,
    content: item.content,
    pageReferences: item.page_references ?? [],
    status:
      item.role === 'assistant'
        ? sessionStatusFromTerminalFlowRun(item.status)
        : 'completed',
    runId: item.flow_run_id,
    detailRunId: item.flow_run_id,
    canOpenDetail: true,
    rawOutput: null,
    traceSummary: [],
    presentation: 'answer'
  }));
}

function withAssistantActivityEvent(
  message: AgentFlowDebugMessage,
  event: ConsoleFlowDebugStreamEvent
): AgentFlowDebugMessage {
  return {
    ...message,
    presentation: 'answer',
    activityEvents: [...(message.activityEvents ?? []), event]
  };
}

export function useEmbeddedAssistantSession(
  applicationId: string | null,
  clientTools?: ConsoleAssistantClientTools
) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [status, setStatus] = useState<AgentFlowDebugSessionStatus>('idle');
  const [stopping, setStopping] = useState(false);
  const [messages, setMessages] = useState<AgentFlowDebugMessage[]>([]);
  const [traceItems, setTraceItems] = useState<AgentFlowTraceItem[]>([]);
  const [contextSnapshot, setContextSnapshot] =
    useState<ConsoleContextSnapshot | null>(null);
  const [runContext, setRunContext] = useState(() =>
    createRunContext(applicationId)
  );
  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [legacyFlowRunId, setLegacyFlowRunId] = useState<string | null>(null);
  const [restoringHistory, setRestoringHistory] = useState(false);
  const activeRunIdRef = useRef<string | null>(null);
  const activeApplicationIdRef = useRef<string | null>(null);
  const conversationIdRef = useRef<string | null>(null);
  const legacyFlowRunIdRef = useRef<string | null>(null);
  const streamAbortControllerRef = useRef<AbortController | null>(null);
  const websocketControlRef = useRef<ConsoleAssistantWebSocketControl | null>(
    null
  );
  const streamGenerationRef = useRef(0);
  const liveTraceItemsRef = useRef<AgentFlowTraceItem[]>([]);

  const cancelActiveStream = useCallback(() => {
    streamGenerationRef.current += 1;
    streamAbortControllerRef.current?.abort();
    streamAbortControllerRef.current = null;
    websocketControlRef.current = null;
  }, []);

  const setCurrentConversation = useCallback(
    (nextConversationId: string | null) => {
      conversationIdRef.current = nextConversationId;
      setConversationId(nextConversationId);
    },
    []
  );

  const setCurrentLegacySnapshot = useCallback(
    (nextFlowRunId: string | null) => {
      legacyFlowRunIdRef.current = nextFlowRunId;
      setLegacyFlowRunId(nextFlowRunId);
    },
    []
  );

  const resetSession = useCallback(() => {
    cancelActiveStream();
    activeRunIdRef.current = null;
    activeApplicationIdRef.current = null;
    setCurrentConversation(null);
    setCurrentLegacySnapshot(null);
    setActiveRunId(null);
    setStatus('idle');
    setStopping(false);
    setMessages([]);
    setTraceItems([]);
    liveTraceItemsRef.current = [];
    setContextSnapshot(null);
    setRunContext(createRunContext(applicationId));
  }, [
    applicationId,
    cancelActiveStream,
    setCurrentConversation,
    setCurrentLegacySnapshot
  ]);

  useEffect(() => {
    resetSession();
  }, [resetSession]);

  useEffect(() => cancelActiveStream, [cancelActiveStream]);

  const clearSession = useCallback(() => {
    if (hasActiveAssistantRun(status)) {
      return;
    }
    resetSession();
  }, [resetSession, status]);

  const startNewConversation = useCallback(
    async (seedLegacyFlowRunId?: string) => {
      if (!applicationId || !csrfToken || restoringHistory) {
        return false;
      }

      cancelActiveStream();
      const generation = streamGenerationRef.current;
      setRestoringHistory(true);
      try {
        const conversation = await createConsoleAssistantConversation(
          {
            application_id: applicationId,
            ...(seedLegacyFlowRunId
              ? { seed_legacy_flow_run_id: seedLegacyFlowRunId }
              : {})
          },
          csrfToken
        );
        if (streamGenerationRef.current !== generation) {
          return false;
        }
        activeRunIdRef.current = null;
        activeApplicationIdRef.current = null;
        setActiveRunId(null);
        setCurrentConversation(conversation.conversation_id);
        setCurrentLegacySnapshot(null);
        setStatus('idle');
        setStopping(false);
        if (!seedLegacyFlowRunId) {
          setMessages([]);
        }
        setTraceItems([]);
        liveTraceItemsRef.current = [];
        setContextSnapshot(null);
        setRunContext(createRunContext(applicationId));
        return true;
      } finally {
        if (streamGenerationRef.current === generation) {
          setRestoringHistory(false);
        }
      }
    },
    [
      applicationId,
      cancelActiveStream,
      csrfToken,
      setCurrentConversation,
      setCurrentLegacySnapshot,
      restoringHistory
    ]
  );

  const reconcileRunSnapshot = useCallback(
    async (
      runApplicationId: string,
      runId: string,
      streamGeneration: number
    ) => {
      try {
        const detail = await fetchApplicationRunDebugSnapshot(
          runApplicationId,
          runId
        );
        if (streamGenerationRef.current !== streamGeneration) {
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
            message.role === 'assistant' && message.runId === runId
              ? { ...message, traceSummary: traceItems }
              : message
          )
        );
        const nextContextSnapshot = detail.context_snapshot ?? null;
        if (nextContextSnapshot !== null) {
          setContextSnapshot(nextContextSnapshot);
        }

        if (isTerminalFlowRunStatus(detail.flow_run.status)) {
          const nextStatus = sessionStatusFromTerminalFlowRun(
            detail.flow_run.status
          );
          const snapshotMessage =
            nextStatus === 'cancelled'
              ? mapRunDetailToConversation(detail)
              : null;
          setStatus(nextStatus);
          setMessages((current) =>
            current.map((message) =>
              message.role === 'assistant' && message.runId === runId
                ? {
                    ...message,
                    status: nextStatus,
                    content: snapshotMessage?.content || message.content,
                    rawOutput: snapshotMessage?.rawOutput ?? message.rawOutput,
                    traceSummary: traceItems
                  }
                : message
            )
          );
        }
      } catch {
        // The live stream remains authoritative when optional recovery cannot load.
      }
    },
    []
  );

  const attachActiveRun = useCallback(
    (runApplicationId: string, runId: string, runningMessageId: string) => {
      if (!csrfToken) {
        return;
      }
      cancelActiveStream();
      const streamGeneration = streamGenerationRef.current;
      let streamAssistantMessage: AgentFlowDebugMessage = {
        ...createRunningAssistantMessage(),
        id: runningMessageId,
        runId,
        detailRunId: runId,
        canOpenDetail: true
      };
      let receivedTerminal = false;
      const seenEventKeys = new Set<string>();

      const handlers: ConsoleFlowDebugStreamHandlers = {
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
          if (isTraceEvent(event)) {
            liveTraceItemsRef.current = applyDebugStreamEventToTrace(
              liveTraceItemsRef.current,
              event
            );
            setTraceItems(liveTraceItemsRef.current);
          }
          if (event.type === 'context_snapshot') {
            setContextSnapshot(event);
          }
          streamAssistantMessage = withAssistantActivityEvent(
            applyDebugStreamEventToAssistantMessage(
              streamAssistantMessage,
              event,
              liveTraceItemsRef.current
            ),
            event
          );
          if (isTerminalEvent(event)) {
            receivedTerminal = true;
          }
          setStatus(streamAssistantMessage.status);
          setMessages((current) =>
            replaceAssistantMessage(
              current,
              streamAssistantMessage,
              runningMessageId
            )
          );
          if (isTerminalEvent(event) && event.run_id) {
            void reconcileRunSnapshot(
              runApplicationId,
              event.run_id,
              streamGeneration
            );
          }
        }
      };

      void attachConsoleAssistantRunWebSocket(
        runApplicationId,
        runId,
        csrfToken,
        handlers,
        {
          onControl: (control) => {
            websocketControlRef.current = control;
          }
        }
      )
        .then(() => {
          if (
            streamGenerationRef.current === streamGeneration &&
            !receivedTerminal
          ) {
            return reconcileRunSnapshot(
              runApplicationId,
              runId,
              streamGeneration
            );
          }
        })
        .catch(() => {
          if (streamGenerationRef.current === streamGeneration) {
            return reconcileRunSnapshot(
              runApplicationId,
              runId,
              streamGeneration
            );
          }
        })
        .finally(() => {
          if (streamGenerationRef.current === streamGeneration) {
            streamAbortControllerRef.current = null;
            websocketControlRef.current = null;
          }
        });
    },
    [cancelActiveStream, csrfToken, reconcileRunSnapshot]
  );

  const restoreConversation = useCallback(
    async (target: {
      conversationId?: string | null;
      legacyFlowRunId?: string | null;
      latestFlowRunId?: string | null;
      latestFlowRunStatus?: string | null;
    }) => {
      if (
        !applicationId ||
        restoringHistory ||
        (!target.conversationId && !target.legacyFlowRunId)
      ) {
        return false;
      }

      cancelActiveStream();
      const generation = streamGenerationRef.current;
      setRestoringHistory(true);
      try {
        const history = target.conversationId
          ? await getConsoleAssistantConversationMessages(
              applicationId,
              target.conversationId
            )
          : await getConsoleAssistantLegacySnapshotMessages(
              applicationId,
              target.legacyFlowRunId as string
            );
        if (streamGenerationRef.current !== generation) {
          return false;
        }
        const nextStatus = sessionStatusFromFlowRun(target.latestFlowRunStatus);
        const activeRunId = hasActiveAssistantRun(nextStatus)
          ? (target.latestFlowRunId ?? null)
          : null;
        const nextMessages = restoredMessages(history);
        let runningMessageId: string | null = null;
        if (activeRunId) {
          const restoredAssistant = nextMessages.find(
            (message) =>
              message.role === 'assistant' && message.runId === activeRunId
          );
          if (restoredAssistant) {
            restoredAssistant.status = 'running';
            runningMessageId = restoredAssistant.id;
          } else {
            const runningMessage = {
              ...createRunningAssistantMessage(),
              runId: activeRunId,
              detailRunId: activeRunId,
              canOpenDetail: true,
              presentation: 'answer' as const
            };
            runningMessageId = runningMessage.id;
            nextMessages.push(runningMessage);
          }
        } else if (
          nextStatus === 'cancelled' &&
          target.latestFlowRunId &&
          !nextMessages.some(
            (message) =>
              message.role === 'assistant' &&
              message.runId === target.latestFlowRunId
          )
        ) {
          nextMessages.push({
            ...createRunningAssistantMessage(),
            runId: target.latestFlowRunId,
            detailRunId: target.latestFlowRunId,
            canOpenDetail: true,
            status: 'cancelled'
          });
        }
        activeRunIdRef.current = activeRunId;
        activeApplicationIdRef.current = activeRunId ? applicationId : null;
        setActiveRunId(activeRunId);
        setCurrentConversation(target.conversationId ?? null);
        setCurrentLegacySnapshot(target.legacyFlowRunId ?? null);
        setStatus(nextStatus);
        setStopping(false);
        setMessages(nextMessages);
        setTraceItems([]);
        liveTraceItemsRef.current = [];
        setContextSnapshot(null);
        setRunContext(createRunContext(applicationId));
        if (activeRunId && runningMessageId) {
          queueMicrotask(() => {
            if (streamGenerationRef.current === generation) {
              attachActiveRun(applicationId, activeRunId, runningMessageId);
            }
          });
        }
        return true;
      } finally {
        if (streamGenerationRef.current === generation) {
          setRestoringHistory(false);
        }
      }
    },
    [
      applicationId,
      attachActiveRun,
      cancelActiveStream,
      restoringHistory,
      setCurrentConversation,
      setCurrentLegacySnapshot
    ]
  );

  const disconnectSession = useCallback(() => {
    cancelActiveStream();
  }, [cancelActiveStream]);

  const resumeActiveRun = useCallback(() => {
    const runId = activeRunIdRef.current;
    const runApplicationId = activeApplicationIdRef.current;
    if (!runId || !runApplicationId || !hasActiveAssistantRun(status)) {
      return;
    }
    const runningMessage = messages.find(
      (message) => message.role === 'assistant' && message.runId === runId
    );
    if (runningMessage) {
      attachActiveRun(runApplicationId, runId, runningMessage.id);
    }
  }, [attachActiveRun, messages, status]);

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
    async (
      prompt: string,
      pageReferences: ConsoleAssistantPageReference[] = []
    ) => {
      const query = prompt.trim();
      if (
        !applicationId ||
        !csrfToken ||
        !query ||
        ['running', 'waiting_callback', 'waiting_human'].includes(status)
      ) {
        return;
      }

      cancelActiveStream();
      const streamGeneration = streamGenerationRef.current;
      let targetConversationId = conversationIdRef.current;
      if (!targetConversationId) {
        const seedLegacyFlowRunId = legacyFlowRunIdRef.current;
        const conversation = await createConsoleAssistantConversation(
          {
            application_id: applicationId,
            ...(seedLegacyFlowRunId
              ? { seed_legacy_flow_run_id: seedLegacyFlowRunId }
              : {})
          },
          csrfToken
        );
        if (streamGenerationRef.current !== streamGeneration) {
          return;
        }
        targetConversationId = conversation.conversation_id;
        setCurrentConversation(targetConversationId);
        setCurrentLegacySnapshot(null);
      }

      const runningMessage: AgentFlowDebugMessage = {
        ...createRunningAssistantMessage(),
        presentation: 'answer'
      };
      let streamAssistantMessage = runningMessage;
      let receivedTerminal = false;
      let receivedAnyEvent = false;
      const seenEventKeys = new Set<string>();

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
        createUserMessage(query, pageReferences),
        runningMessage
      ]);

      const handlers: ConsoleFlowDebugStreamHandlers = {
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
          receivedAnyEvent = true;
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

          if (event.type === 'context_snapshot') {
            setContextSnapshot(event);
          }

          streamAssistantMessage = withAssistantActivityEvent(
            applyDebugStreamEventToAssistantMessage(
              streamAssistantMessage,
              event,
              liveTraceItemsRef.current
            ),
            event
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
          if (isTerminalEvent(event) && event.run_id) {
            void reconcileRunSnapshot(
              applicationId,
              event.run_id,
              streamGeneration
            );
          }
        }
      };

      try {
        try {
          await startConsoleAssistantRunWebSocket(
            {
              application_id: applicationId,
              conversation_id: targetConversationId,
              query,
              ...(pageReferences.length > 0
                ? { page_references: pageReferences }
                : {})
            },
            csrfToken,
            handlers,
            {
              clientTools,
              onControl: (control) => {
                websocketControlRef.current = control;
              }
            }
          );
        } catch (error) {
          if (
            receivedAnyEvent ||
            streamGenerationRef.current !== streamGeneration
          ) {
            throw error;
          }
          websocketControlRef.current = null;
          await startConsoleAssistantRunStream(
            {
              application_id: applicationId,
              conversation_id: targetConversationId,
              query,
              ...(pageReferences.length > 0
                ? { page_references: pageReferences }
                : {})
            },
            csrfToken,
            handlers
          );
        }

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
        if (activeRunIdRef.current) {
          await reconcileRunSnapshot(
            applicationId,
            activeRunIdRef.current,
            streamGeneration
          );
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
          websocketControlRef.current = null;
        }
      }
    },
    [
      applicationId,
      cancelActiveStream,
      csrfToken,
      messages,
      reconcileRunSnapshot,
      setCurrentConversation,
      setCurrentLegacySnapshot,
      status,
      clientTools
    ]
  );

  const stopRun = useCallback(async () => {
    const runId = activeRunIdRef.current;
    const runApplicationId = activeApplicationIdRef.current;
    if (
      !csrfToken ||
      !runApplicationId ||
      !['running', 'waiting_callback', 'waiting_human'].includes(status)
    ) {
      return;
    }

    setStopping(true);
    try {
      if (!runId) {
        cancelActiveStream();
        setStatus('cancelled');
        setMessages((current) =>
          current.map((message) =>
            message.role === 'assistant' && message.status === 'running'
              ? { ...message, status: 'cancelled' }
              : message
          )
        );
        return;
      }
      const websocketControl = websocketControlRef.current;
      if (websocketControl) {
        websocketControl.cancel(runId);
        return;
      }
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
    contextSnapshot,
    runContext,
    activeRunId,
    conversationId,
    legacyFlowRunId,
    restoringHistory,
    canChangeConversation: !restoringHistory,
    canEditCurrentConversation: !hasActiveAssistantRun(status),
    clearSession,
    disconnectSession,
    restoreConversation,
    resumeActiveRun,
    setRunContextValue,
    startNewConversation,
    submitPrompt,
    stopRun
  };
}
