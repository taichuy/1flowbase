import { useQueryClient } from '@tanstack/react-query';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { useAuthStore } from '../../../../state/auth-store';
import {
  buildFlowDebugRunInput,
  cancelFlowDebugRun,
  deleteDebugVariableCacheEntries,
  fetchApplicationRunDebugSnapshot,
  fetchDebugVariableSnapshot,
  startFlowDebugRun,
  startFlowDebugRunStream,
  upsertDebugVariableCacheEntry,
  type AgentFlowDebugMessage,
  type AgentFlowTraceItem,
  type AgentFlowVariableGroup,
  type FlowDebugRunDetail,
  type NodeDebugPreviewVariableCache
} from '../../api/runtime';
import {
  applyDebugStreamEventToAssistantMessage,
  applyDebugStreamEventToTrace
} from '../../lib/debug-console/stream-events';
import {
  buildStreamEventDedupKeys,
  clearRunContextQuery,
  createRunningAssistantMessage,
  createUserMessage,
  replaceAssistantMessage,
  replaceAssistantMessageWithError,
  resolvePrompt,
  shouldPollRun,
  updateRunContextQuery
} from './debug-session-messages';
import {
  applyVariableOverridesToGroups,
  buildDisplayVariableCache,
  buildInputVariableCacheFromRunDetail,
  buildNodeVariableDisplayMetadata,
  createDebugSessionState,
  hydrateRunDetailArtifacts,
  mergeVariableCache,
  mergeVariableGroupsByTitle,
  parseVariableCacheItemKey,
  projectVariableCache,
  removeCachedVariableItemsFromGroups,
  removeVariableCacheKeys
} from './debug-session-variable-cache';
import {
  mapRunDetailToConversation,
  mapRunDetailToTrace
} from '../../lib/debug-console/run-detail-mapper';
import {
  buildRunContextFromDocument,
  getRunContextValues,
  mapRunContextToVariableGroups,
  mapVariableCacheToVariableGroups
} from '../../lib/debug-console/variable-groups';
import type { AgentFlowEnvironmentVariable } from '../../lib/variables/application-environment-variables';
import { i18nText } from '../../../../shared/i18n/text';

const RUN_DETAIL_POLL_INTERVAL_MS = 200;
export type AgentFlowDebugSessionStatus =
  | 'idle'
  | 'running'
  | 'completed'
  | 'waiting_callback'
  | 'waiting_human'
  | 'cancelled'
  | 'failed';

export function useAgentFlowDebugSession({
  applicationId,
  draftId,
  document,
  environmentVariables = []
}: {
  applicationId: string;
  draftId: string;
  document: FlowAuthoringDocument;
  environmentVariables?: AgentFlowEnvironmentVariable[];
}) {
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const actorUserId = useAuthStore(
    (state) => state.actor?.id ?? state.me?.id ?? null
  );
  const [status, setStatus] = useState<AgentFlowDebugSessionStatus>('idle');
  const [stopping, setStopping] = useState(false);
  const [messages, setMessages] = useState<AgentFlowDebugMessage[]>([]);
  const [lastDetail, setLastDetail] = useState<FlowDebugRunDetail | null>(null);
  const [streamTraceItems, setStreamTraceItems] = useState<
    AgentFlowTraceItem[]
  >([]);
  const [nodePreviewInputCache, setNodePreviewInputCache] =
    useState<NodeDebugPreviewVariableCache>({});
  const [nodePreviewOutputCache, setNodePreviewOutputCache] =
    useState<NodeDebugPreviewVariableCache>({});
  const [variableOverrides, setVariableOverrides] =
    useState<NodeDebugPreviewVariableCache>({});
  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const [runContext, setRunContext] = useState(() =>
    buildRunContextFromDocument(document, null)
  );
  const previousDraftScopeRef = useRef(`${applicationId}:${draftId}`);
  const debugSessionScope = `${applicationId}:${draftId}`;
  const [debugSessionState, setDebugSessionState] = useState(() =>
    createDebugSessionState(applicationId, draftId)
  );
  const lastSubmittedPromptRef = useRef<string | null>(null);
  const activeRunIdRef = useRef<string | null>(null);
  const pollTimerRef = useRef<number | null>(null);
  const pendingAssistantMessageRef = useRef<AgentFlowDebugMessage | null>(null);
  const flushStreamMessageFrameRef = useRef<number | null>(null);
  const streamAbortControllerRef = useRef<AbortController | null>(null);
  const streamGenerationRef = useRef(0);
  const variableSnapshotRestoreGenerationRef = useRef(0);
  const stoppingRef = useRef(false);

  useEffect(() => {
    setDebugSessionState((current) =>
      current.scope === debugSessionScope
        ? current
        : createDebugSessionState(applicationId, draftId)
    );
    setActiveRunId(null);
    setNodePreviewInputCache({});
    setVariableOverrides({});
  }, [applicationId, debugSessionScope, draftId]);

  useEffect(() => {
    setRunContext((currentRunContext) => {
      const isSameDraft = previousDraftScopeRef.current === debugSessionScope;
      const nextValues = isSameDraft
        ? getRunContextValues(currentRunContext)
        : null;

      previousDraftScopeRef.current = debugSessionScope;

      const nextRunContext = buildRunContextFromDocument(document, nextValues);

      return isSameDraft
        ? { ...nextRunContext, remembered: currentRunContext.remembered }
        : nextRunContext;
    });
  }, [debugSessionScope, document]);

  useEffect(() => {
    if (debugSessionState.scope !== debugSessionScope) {
      return;
    }

    let disposed = false;
    const restoreGeneration =
      (variableSnapshotRestoreGenerationRef.current += 1);

    setNodePreviewOutputCache({});
    fetchDebugVariableSnapshot(applicationId)
      .then((snapshot) => {
        if (
          disposed ||
          restoreGeneration !== variableSnapshotRestoreGenerationRef.current
        ) {
          return;
        }

        setNodePreviewOutputCache(
          projectVariableCache(document, snapshot.variable_cache)
        );
      })
      .catch(() => {
        // Durable variable snapshots are a convenience cache; the editor still opens without them.
      });

    return () => {
      disposed = true;
    };
  }, [
    activeRunId,
    applicationId,
    debugSessionScope,
    debugSessionState,
    document,
    draftId
  ]);

  const rawTraceItems = useMemo(
    () =>
      streamTraceItems.length > 0
        ? streamTraceItems
        : lastDetail
          ? mapRunDetailToTrace(lastDetail)
          : [],
    [lastDetail, streamTraceItems]
  );
  const traceItems = rawTraceItems;
  const nodeVariableDisplayMetadata = useMemo(
    () => buildNodeVariableDisplayMetadata(document),
    [document]
  );
  const variableGroups = useMemo<AgentFlowVariableGroup[]>(() => {
    const groups = mapRunContextToVariableGroups(runContext, {
      applicationId,
      draftId,
      debugSessionId: debugSessionState.id,
      flowId: document.meta.flowId,
      actorUserId,
      environmentVariables
    });
    const cacheGroups = mapVariableCacheToVariableGroups(
      buildDisplayVariableCache(nodePreviewOutputCache),
      nodeVariableDisplayMetadata
    );
    const uncachedGroups = removeCachedVariableItemsFromGroups(
      groups,
      nodePreviewOutputCache
    );

    return applyVariableOverridesToGroups(
      cacheGroups.length > 0
        ? mergeVariableGroupsByTitle([...cacheGroups, ...uncachedGroups])
        : groups,
      mergeVariableCache(nodePreviewInputCache, variableOverrides)
    );
  }, [
    applicationId,
    actorUserId,
    debugSessionState.id,
    draftId,
    document.meta.flowId,
    environmentVariables,
    nodeVariableDisplayMetadata,
    nodePreviewInputCache,
    nodePreviewOutputCache,
    runContext,
    variableOverrides
  ]);

  const clearPollTimer = useCallback(() => {
    if (pollTimerRef.current !== null) {
      window.clearTimeout(pollTimerRef.current);
      pollTimerRef.current = null;
    }
  }, []);

  const stopPolling = useCallback(() => {
    clearPollTimer();
    activeRunIdRef.current = null;
  }, [clearPollTimer]);

  const abortActiveDebugStream = useCallback(() => {
    streamAbortControllerRef.current?.abort();
    streamAbortControllerRef.current = null;
  }, []);

  const startDebugStreamGeneration = useCallback(() => {
    abortActiveDebugStream();
    streamGenerationRef.current += 1;
    return streamGenerationRef.current;
  }, [abortActiveDebugStream]);

  const cancelActiveDebugStream = useCallback(() => {
    streamGenerationRef.current += 1;
    abortActiveDebugStream();
  }, [abortActiveDebugStream]);

  function isActiveDebugStreamGeneration(generation: number) {
    return streamGenerationRef.current === generation;
  }

  function scheduleAssistantMessageFlush(
    runningMessageId: string,
    nextMessage: AgentFlowDebugMessage
  ) {
    pendingAssistantMessageRef.current = nextMessage;

    if (flushStreamMessageFrameRef.current !== null) {
      return;
    }

    flushStreamMessageFrameRef.current = window.requestAnimationFrame(() => {
      flushStreamMessageFrameRef.current = null;
      const pending = pendingAssistantMessageRef.current;

      if (!pending) {
        return;
      }

      pendingAssistantMessageRef.current = null;
      setStatus(pending.status);
      setMessages((currentMessages) =>
        replaceAssistantMessage(currentMessages, pending, runningMessageId)
      );
    });
  }

  const clearScheduledAssistantMessageFlush = useCallback(() => {
    if (flushStreamMessageFrameRef.current !== null) {
      window.cancelAnimationFrame(flushStreamMessageFrameRef.current);
      flushStreamMessageFrameRef.current = null;
    }

    pendingAssistantMessageRef.current = null;
  }, []);

  function flushAssistantMessageImmediately(
    runningMessageId: string,
    nextMessage: AgentFlowDebugMessage
  ) {
    clearScheduledAssistantMessageFlush();
    setStatus(nextMessage.status);
    setMessages((currentMessages) =>
      replaceAssistantMessage(currentMessages, nextMessage, runningMessageId)
    );
  }

  async function applyRunDetail(
    detail: FlowDebugRunDetail,
    options?: {
      fallbackMessageId?: string | null;
      invalidateRuntime?: boolean;
    }
  ) {
    const hydratedDetail = await hydrateRunDetailArtifacts(
      applicationId,
      detail
    );
    const assistantMessage = mapRunDetailToConversation(hydratedDetail);
    const inputVariableCache =
      buildInputVariableCacheFromRunDetail(hydratedDetail);

    setActiveRunId(hydratedDetail.flow_run.id);
    setLastDetail(hydratedDetail);
    setVariableOverrides((currentOverrides) => {
      return removeVariableCacheKeys(currentOverrides, inputVariableCache);
    });
    setNodePreviewInputCache((currentCache) =>
      mergeVariableCache(currentCache, inputVariableCache)
    );
    setStatus(assistantMessage.status);
    setMessages((currentMessages) =>
      replaceAssistantMessage(
        currentMessages,
        assistantMessage,
        options?.fallbackMessageId
      )
    );

    if (options?.invalidateRuntime) {
      await queryClient.invalidateQueries({
        queryKey: ['applications', applicationId, 'runtime']
      });
    }

    return assistantMessage;
  }

  async function pollRunDetail(runId: string) {
    try {
      const detail = await fetchApplicationRunDebugSnapshot(
        applicationId,
        runId
      );

      if (activeRunIdRef.current !== runId) {
        return;
      }

      const assistantMessage = await applyRunDetail(detail);

      if (!shouldPollRun(detail)) {
        stopPolling();
        await queryClient.invalidateQueries({
          queryKey: ['applications', applicationId, 'runtime']
        });
        return;
      }

      setStatus(assistantMessage.status);
      pollTimerRef.current = window.setTimeout(() => {
        void pollRunDetail(runId);
      }, RUN_DETAIL_POLL_INTERVAL_MS);
    } catch (error) {
      if (activeRunIdRef.current !== runId) {
        return;
      }

      stopPolling();
      setStatus('failed');
      setMessages((currentMessages) =>
        replaceAssistantMessageWithError(
          currentMessages,
          error instanceof Error
            ? error.message
            : i18nText('agentFlow', 'auto.debug_run_failed'),
          { runId }
        )
      );
    }
  }

  function beginPolling(runId: string) {
    stopPolling();
    activeRunIdRef.current = runId;
    pollTimerRef.current = window.setTimeout(() => {
      void pollRunDetail(runId);
    }, RUN_DETAIL_POLL_INTERVAL_MS);
  }

  useEffect(
    () => () => {
      clearPollTimer();
      activeRunIdRef.current = null;
      clearScheduledAssistantMessageFlush();
      cancelActiveDebugStream();
    },
    [
      cancelActiveDebugStream,
      clearPollTimer,
      clearScheduledAssistantMessageFlush
    ]
  );

  async function submitPrompt(prompt?: string) {
    const resolvedPrompt = resolvePrompt(runContext, prompt);
    const nextRunContext = updateRunContextQuery(runContext, resolvedPrompt);
    const inputValues = getRunContextValues(nextRunContext);
    const runningMessage = createRunningAssistantMessage();

    stopPolling();
    const streamGeneration = startDebugStreamGeneration();
    clearScheduledAssistantMessageFlush();
    setRunContext(clearRunContextQuery(nextRunContext));
    setStatus('running');
    setLastDetail(null);
    setStreamTraceItems([]);
    setNodePreviewInputCache({});
    setMessages((currentMessages) => [
      ...currentMessages,
      createUserMessage(resolvedPrompt),
      runningMessage
    ]);

    if (!csrfToken) {
      setStatus('failed');
      setMessages((currentMessages) =>
        replaceAssistantMessageWithError(
          currentMessages,
          i18nText('agentFlow', 'auto.debug_run_csrf_missing'),
          { fallbackMessageId: runningMessage.id }
        )
      );
      return null;
    }

    const runInput = {
      ...buildFlowDebugRunInput(document, inputValues, environmentVariables),
      document,
      debug_session_id: debugSessionState.id
    };

    try {
      let streamAssistantMessage = runningMessage;
      let streamTraceItemsSnapshot: AgentFlowTraceItem[] = [];
      const seenStreamEventKeys = new Set<string>();

      await startFlowDebugRunStream(applicationId, runInput, csrfToken, {
        getAbortController: (abortController) => {
          if (!isActiveDebugStreamGeneration(streamGeneration)) {
            abortController.abort();
            return;
          }

          streamAbortControllerRef.current = abortController;
        },
        onEvent: (event) => {
          const dedupKeys = buildStreamEventDedupKeys(event);
          const isRepeatedStreamEvent = dedupKeys.some((key) =>
            seenStreamEventKeys.has(key)
          );

          if (isRepeatedStreamEvent) {
            return;
          }

          if (dedupKeys.length > 0) {
            dedupKeys.forEach((key) => {
              seenStreamEventKeys.add(key);
            });
          }

          if (!isActiveDebugStreamGeneration(streamGeneration)) {
            return;
          }

          const isTraceEvent =
            event.type === 'node_started' ||
            event.type === 'node_finished' ||
            event.type === 'text_delta' ||
            event.type === 'reasoning_delta' ||
            event.type === 'usage_snapshot';
          const isTerminalEvent =
            event.type === 'flow_finished' ||
            event.type === 'flow_failed' ||
            event.type === 'waiting_human' ||
            event.type === 'waiting_callback' ||
            event.type === 'replay_expired' ||
            event.type === 'replay_gap';

          if (isTraceEvent) {
            streamTraceItemsSnapshot = applyDebugStreamEventToTrace(
              streamTraceItemsSnapshot,
              event
            );
          }

          streamAssistantMessage = applyDebugStreamEventToAssistantMessage(
            streamAssistantMessage,
            event,
            streamTraceItemsSnapshot
          );

          if (
            event.type === 'flow_accepted' ||
            event.type === 'flow_started' ||
            event.type === 'flow_cancelled' ||
            event.type === 'waiting_human' ||
            event.type === 'waiting_callback'
          ) {
            activeRunIdRef.current = event.run_id;
          }

          if (isTraceEvent) {
            setStreamTraceItems(streamTraceItemsSnapshot);
          }

          if (
            (event.type === 'text_delta' || event.type === 'reasoning_delta') &&
            event.presentation?.kind === 'answer'
          ) {
            scheduleAssistantMessageFlush(
              runningMessage.id,
              streamAssistantMessage
            );
            return;
          }

          if (event.type === 'text_delta' || event.type === 'reasoning_delta') {
            return;
          }

          if (isTerminalEvent || event.type === 'flow_cancelled') {
            flushAssistantMessageImmediately(
              runningMessage.id,
              streamAssistantMessage
            );
            return;
          }

          clearScheduledAssistantMessageFlush();
          setStatus(streamAssistantMessage.status);
          setMessages((currentMessages) =>
            replaceAssistantMessage(
              currentMessages,
              streamAssistantMessage,
              runningMessage.id
            )
          );
        }
      });

      if (!isActiveDebugStreamGeneration(streamGeneration)) {
        return null;
      }

      streamAbortControllerRef.current = null;

      lastSubmittedPromptRef.current = resolvedPrompt;
      const completedRunId =
        activeRunIdRef.current ?? streamAssistantMessage.runId;
      if (completedRunId) {
        try {
          const detail = await fetchApplicationRunDebugSnapshot(
            applicationId,
            completedRunId
          );
          await applyRunDetail(detail, {
            fallbackMessageId: runningMessage.id,
            invalidateRuntime: true
          });
        } catch {
          setActiveRunId(completedRunId);
        }
      }
      stopPolling();
      await queryClient.invalidateQueries({
        queryKey: ['applications', applicationId, 'runtime']
      });

      return null;
    } catch (error) {
      if (!isActiveDebugStreamGeneration(streamGeneration)) {
        return null;
      }

      streamAbortControllerRef.current = null;
      if (activeRunIdRef.current) {
        const errorMessage =
          error instanceof Error
            ? error.message
            : i18nText('agentFlow', 'auto.debug_stream_connection_interrupted');
        clearScheduledAssistantMessageFlush();
        setStatus('failed');
        setMessages((currentMessages) =>
          replaceAssistantMessageWithError(currentMessages, errorMessage, {
            fallbackMessageId: runningMessage.id,
            runId: activeRunIdRef.current
          })
        );
        return null;
      }

      stopPolling();
      setStreamTraceItems([]);
    }

    try {
      const detail = await startFlowDebugRun(
        applicationId,
        runInput,
        csrfToken
      );

      lastSubmittedPromptRef.current = resolvedPrompt;
      const assistantMessage = await applyRunDetail(detail, {
        fallbackMessageId: runningMessage.id,
        invalidateRuntime: !shouldPollRun(detail)
      });

      if (shouldPollRun(detail)) {
        beginPolling(detail.flow_run.id);
      } else {
        stopPolling();
      }

      setStatus(assistantMessage.status);
      return detail;
    } catch (error) {
      const errorMessage =
        error instanceof Error
          ? error.message
          : i18nText('agentFlow', 'auto.debug_run_failed');

      setStatus('failed');
      setMessages((currentMessages) =>
        replaceAssistantMessageWithError(currentMessages, errorMessage, {
          fallbackMessageId: runningMessage.id
        })
      );
      return null;
    }
  }

  async function rerunLast() {
    const prompt = lastSubmittedPromptRef.current ?? undefined;
    return submitPrompt(prompt);
  }

  async function stopRun() {
    const runId = lastDetail?.flow_run.id ?? activeRunIdRef.current;

    if (
      stoppingRef.current ||
      !csrfToken ||
      !runId ||
      !['running', 'waiting_human', 'waiting_callback'].includes(status)
    ) {
      return null;
    }

    stoppingRef.current = true;
    setStopping(true);
    try {
      const detail = await cancelFlowDebugRun(applicationId, runId, csrfToken);
      cancelActiveDebugStream();
      stopPolling();
      clearScheduledAssistantMessageFlush();
      await applyRunDetail(detail, { invalidateRuntime: true });
      return detail;
    } catch {
      return null;
    } finally {
      stoppingRef.current = false;
      setStopping(false);
    }
  }

  function clearSession() {
    stoppingRef.current = false;
    setStopping(false);
    cancelActiveDebugStream();
    stopPolling();
    clearScheduledAssistantMessageFlush();
    setStatus('idle');
    setActiveRunId(null);
    setMessages([]);
    setLastDetail(null);
    setStreamTraceItems([]);
  }

  function setRunContextValue(nodeId: string, key: string, value: unknown) {
    setRunContext((currentRunContext) => ({
      ...currentRunContext,
      remembered: false,
      fields: currentRunContext.fields.map((field) =>
        field.nodeId === nodeId && field.key === key
          ? { ...field, value }
          : field
      )
    }));
  }

  function getNodePreviewVariableCache(): NodeDebugPreviewVariableCache {
    const cache: NodeDebugPreviewVariableCache = {};
    const startNodeId =
      document.graph.nodes.find((node) => node.type === 'start')?.id ??
      'node-start';

    for (const field of runContext.fields) {
      cache[field.nodeId] ??= {};
      cache[field.nodeId][field.key] = field.value;
    }

    for (const [nodeId, payload] of Object.entries(nodePreviewOutputCache)) {
      cache[nodeId] = {
        ...(cache[nodeId] ?? {}),
        ...payload
      };
    }

    for (const [nodeId, payload] of Object.entries(nodePreviewInputCache)) {
      if (nodeId !== startNodeId) {
        continue;
      }

      cache[nodeId] = {
        ...(cache[nodeId] ?? {}),
        ...payload
      };
    }

    return mergeVariableCache(cache, variableOverrides);
  }

  function setVariableCacheValue(key: string, value: unknown) {
    const parsed = parseVariableCacheItemKey(key);
    if (!parsed) {
      return;
    }

    const nextOverridePayload = {
      [parsed.nodeId]: {
        [parsed.variableKey]: value
      }
    };

    setVariableOverrides((currentOverrides) => {
      return mergeVariableCache(currentOverrides, nextOverridePayload);
    });
    if (csrfToken) {
      void upsertDebugVariableCacheEntry(
        applicationId,
        {
          node_id: parsed.nodeId,
          variable_key: parsed.variableKey,
          value
        },
        csrfToken
      ).catch(() => {});
    }
    setNodePreviewOutputCache((currentCache) =>
      mergeVariableCache(currentCache, nextOverridePayload)
    );
    setRunContext((currentRunContext) => {
      let changed = false;
      const nextFields = currentRunContext.fields.map((field) => {
        if (
          field.nodeId !== parsed.nodeId ||
          field.key !== parsed.variableKey
        ) {
          return field;
        }
        changed = true;
        return { ...field, value };
      });

      if (!changed) {
        return currentRunContext;
      }

      const nextRunContext = {
        ...currentRunContext,
        remembered: false,
        fields: nextFields
      };
      return nextRunContext;
    });
  }

  function rememberNodePreviewInputs(
    inputPayload: NodeDebugPreviewVariableCache
  ) {
    setNodePreviewInputCache((currentCache) => {
      return mergeVariableCache(currentCache, inputPayload);
    });
  }

  function rememberExternalRunDetail(detail: FlowDebugRunDetail) {
    const inputVariableCache = buildInputVariableCacheFromRunDetail(detail);

    setActiveRunId(detail.flow_run.id);
    setLastDetail(detail);
    setVariableOverrides((currentOverrides) => {
      return removeVariableCacheKeys(currentOverrides, inputVariableCache);
    });
    setNodePreviewInputCache((currentCache) =>
      mergeVariableCache(currentCache, inputVariableCache)
    );
  }

  function selectRunScope(runId: string | null) {
    setActiveRunId((current) => (current === runId ? current : runId));
    setLastDetail((current) =>
      current && current.flow_run.id !== runId ? null : current
    );
  }

  function resetVariableCache() {
    variableSnapshotRestoreGenerationRef.current += 1;
    const nextDebugSessionState = createDebugSessionState(
      applicationId,
      draftId
    );
    stoppingRef.current = false;
    setStopping(false);
    setDebugSessionState(nextDebugSessionState);
    cancelActiveDebugStream();
    stopPolling();
    clearScheduledAssistantMessageFlush();
    setStatus('idle');
    setActiveRunId(null);
    setLastDetail(null);
    setStreamTraceItems([]);
    setNodePreviewInputCache({});
    setNodePreviewOutputCache({});
    setVariableOverrides({});
    if (csrfToken) {
      void deleteDebugVariableCacheEntries(applicationId, {}, csrfToken).catch(
        () => {}
      );
    }
    setRunContext(buildRunContextFromDocument(document, null));
  }

  return {
    status,
    stopping,
    debugSessionId: debugSessionState.id,
    activeRunId,
    runContext,
    messages,
    traceItems,
    variableGroups,
    submitPrompt,
    rerunLast,
    stopRun,
    clearSession,
    setRunContextValue,
    getNodePreviewVariableCache,
    setVariableCacheValue,
    rememberNodePreviewInputs,
    rememberExternalRunDetail,
    selectRunScope,
    resetVariableCache
  };
}
