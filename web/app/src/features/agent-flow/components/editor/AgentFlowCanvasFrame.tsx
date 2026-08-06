import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { App, Button, Typography } from 'antd';
import {
  type KeyboardEvent as ReactKeyboardEvent,
  useEffect,
  useMemo,
  useRef,
  useState
} from 'react';

import { useContainerNavigation } from '../../hooks/interactions/use-container-navigation';
import { useDraftSync } from '../../hooks/interactions/use-draft-sync';
import { useEditorShortcuts } from '../../hooks/interactions/use-editor-shortcuts';
import { useNodeDetailActions } from '../../hooks/interactions/use-node-detail-actions';
import { useAgentFlowDebugSession } from '../../hooks/runtime/useAgentFlowDebugSession';
import {
  applicationRunNodeLastRunQueryKey,
  buildNodeDebugPreviewPlan,
  buildNodeDebugVariableConfirmationPlan,
  fetchRuntimeDebugArtifact,
  fetchRuntimeDebugArtifacts,
  nodeLastRunToFlowDebugRunDetail,
  nodeLastRunQueryKey,
  startNodeDebugPreview,
  type NodeDebugPreviewPlan,
  type NodeDebugPreviewVariableCache
} from '../../api/runtime';
import { orchestrationQueryKey, updateVersion } from '../../api/orchestration';
import {
  applicationDetailQueryKey,
  applicationEnvironmentVariablesQueryKey,
  exportApplicationArchive,
  replaceApplicationEnvironmentVariables
} from '../../../applications/api/applications';
import {
  applicationApiPublicationQueryKey,
  fetchApplicationApiMapping,
  publishApplicationApiVersion
} from '../../../applications/api/public-api';
import {
  environmentVariableNodeId,
  type AgentFlowEnvironmentVariable
} from '../../lib/variables/application-environment-variables';
import {
  listConversationVariables,
  replaceConversationVariables
} from '../../lib/variables/conversation-variables';
import { systemVariableNodeId } from '../../lib/variables/system-variables';
import {
  fetchModelProviderOptions,
  modelProviderOptionsQueryKey
} from '../../api/model-provider-options';
import { clampNodeDetailWidth } from '../../lib/detail-panel-width';
import {
  isVariableAggregatorCandidateTypeMismatchIssue,
  validateDocument
} from '../../lib/validate-document';
import { buildNodePickerOptions } from '../../lib/plugin-node-definitions';
import { useAuthStore } from '../../../../state/auth-store';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import {
  selectAutosaveStatus,
  selectLastSavedDocument,
  selectUserProtectionLimit,
  selectVersions,
  selectWorkingDocument
} from '../../store/editor/selectors';
import { AgentFlowDebugConsole } from '../debug-console/AgentFlowDebugConsole';
import { ConversationLogPanel } from '../debug-console/ConversationLogPanel';
import {
  AgentFlowVariableCachePanel,
  type SelectedVariableInfo
} from './AgentFlowVariableCachePanel';
import { NodeDetailPanel } from '../detail/NodeDetailPanel';
import { NodePreviewVariablesModal } from '../detail/NodePreviewVariablesModal';
import { VersionHistoryPanel } from '../history/VersionHistoryPanel';
import { IssuesDrawer } from '../issues/IssuesDrawer';
import { AgentFlowCanvas } from './AgentFlowCanvas';
import { AgentFlowOverlay } from './AgentFlowOverlay';
import { AgentFlowSideDock } from './AgentFlowSideDock';
import { ApplicationEnvironmentVariablesPanel } from './ApplicationEnvironmentVariablesPanel';
import { ConversationVariablesPanel } from './ConversationVariablesPanel';
import { SystemVariablesPanel } from './SystemVariablesPanel';
import { i18nText } from '../../../../shared/i18n/text';
import { downloadApplicationArchive } from '../../../applications/lib/template-download';
import {
  CONVERSATION_LOG_DEFAULT_WIDTH,
  DEBUG_CONSOLE_DEFAULT_WIDTH,
  DEBUG_CONSOLE_GAP,
  ENVIRONMENT_VARIABLES_DOCK_WIDTH,
  HISTORY_DOCK_WIDTH,
  SYSTEM_VARIABLES_DOCK_WIDTH,
  VARIABLE_CACHE_DEFAULT_HEIGHT,
  VARIABLE_CACHE_DEFAULT_SIDEBAR_WIDTH,
  VARIABLE_CACHE_MIN_SIDEBAR_WIDTH
} from './canvas-frame/layout';
import { deriveCanvasFrameLayout } from './canvas-frame/derived-layout';
import { createCanvasFrameResizeHandlers } from './canvas-frame/resize-handlers';
import {
  countIssuesByNodeId,
  getDocumentWithLatestViewport
} from './canvas-frame/document';
import { type AgentFlowCanvasFrameProps } from './canvas-frame/types';

type NodePreviewAction = 'run' | 'debug';

const EMPTY_ENVIRONMENT_VARIABLES: NonNullable<
  AgentFlowCanvasFrameProps['initialEnvironmentVariables']
> = [];

export function AgentFlowCanvasFrame({
  applicationId,
  applicationName,
  initialEnvironmentVariables = EMPTY_ENVIRONMENT_VARIABLES,
  nodeCatalog,
  saveDraftOverride,
  restoreVersionOverride
}: AgentFlowCanvasFrameProps) {
  const { message } = App.useApp();
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const actorUserId = useAuthStore(
    (state) => state.actor?.id ?? state.me?.id ?? null
  );
  const workingDocument = useAgentFlowEditorStore(selectWorkingDocument);
  const lastSavedDocument = useAgentFlowEditorStore(selectLastSavedDocument);
  const setWorkingDocument = useAgentFlowEditorStore(
    (state) => state.setWorkingDocument
  );
  const autosaveStatus = useAgentFlowEditorStore(selectAutosaveStatus);
  const versions = useAgentFlowEditorStore(selectVersions);
  const userProtectionLimit = useAgentFlowEditorStore(
    selectUserProtectionLimit
  );
  const draftMeta = useAgentFlowEditorStore((state) => state.draftMeta);
  const autosaveIntervalMs = useAgentFlowEditorStore(
    (state) => state.autosaveIntervalMs
  );
  const selectedNodeId = useAgentFlowEditorStore(
    (state) => state.selectedNodeId
  );
  const activeContainerPath = useAgentFlowEditorStore(
    (state) => state.activeContainerPath
  );
  const issuesOpen = useAgentFlowEditorStore((state) => state.issuesOpen);
  const historyOpen = useAgentFlowEditorStore((state) => state.historyOpen);
  const isRestoringVersion = useAgentFlowEditorStore(
    (state) => state.isRestoringVersion
  );
  const nodeDetailWidth = useAgentFlowEditorStore(
    (state) => state.nodeDetailWidth
  );
  const setPanelState = useAgentFlowEditorStore((state) => state.setPanelState);
  const syncSavedServerState = useAgentFlowEditorStore(
    (state) => state.syncSavedServerState
  );
  const documentRef = useRef(workingDocument);
  const lastSavedDocumentRef = useRef(lastSavedDocument);
  const viewportSnapshotRef = useRef(workingDocument.editor.viewport);
  const viewportGetterRef = useRef<
    (() => FlowAuthoringDocument['editor']['viewport']) | null
  >(null);
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const stopNodeDetailResizeRef = useRef<(() => void) | null>(null);
  const stopDebugConsoleResizeRef = useRef<(() => void) | null>(null);
  const stopConversationLogResizeRef = useRef<(() => void) | null>(null);
  const stopVariablesDockResizeRef = useRef<(() => void) | null>(null);
  const stopHistoryDockResizeRef = useRef<(() => void) | null>(null);
  const stopVariableCacheResizeRef = useRef<(() => void) | null>(null);
  const stopVariableCacheSidebarResizeRef = useRef<(() => void) | null>(null);
  const [bodyWidth, setBodyWidth] = useState(0);
  const [debugConsoleOpen, setDebugConsoleOpen] = useState(false);
  const [debugConsoleWidth, setDebugConsoleWidth] = useState(
    DEBUG_CONSOLE_DEFAULT_WIDTH
  );
  const [nodeDetailTab, setNodeDetailTab] = useState<'config' | 'lastRun'>(
    'config'
  );
  const [bodyHeight, setBodyHeight] = useState(0);
  const [isResizingNodeDetail, setIsResizingNodeDetail] = useState(false);
  const [isResizingDebugConsole, setIsResizingDebugConsole] = useState(false);
  const [conversationLogMessageId, setConversationLogMessageId] = useState<
    string | null
  >(null);
  const [conversationLogWidth, setConversationLogWidth] = useState(
    CONVERSATION_LOG_DEFAULT_WIDTH
  );
  const [isResizingConversationLog, setIsResizingConversationLog] =
    useState(false);
  const [isResizingVariablesDock, setIsResizingVariablesDock] = useState(false);
  const [isResizingHistoryDock, setIsResizingHistoryDock] = useState(false);
  const [pendingNodePreview, setPendingNodePreview] = useState<{
    action: NodePreviewAction;
    nodeId: string;
    inputPayload: NodeDebugPreviewPlan['input_payload'];
    fields: NodeDebugPreviewPlan['missing_fields'];
  } | null>(null);
  const [variableCacheOpen, setVariableCacheOpen] = useState(false);
  const [systemVariablesOpen, setSystemVariablesOpen] = useState(false);
  const [environmentVariablesOpen, setEnvironmentVariablesOpen] =
    useState(false);
  const [conversationVariablesOpen, setConversationVariablesOpen] =
    useState(false);
  const [systemVariablesDockWidth, setSystemVariablesDockWidth] = useState(
    SYSTEM_VARIABLES_DOCK_WIDTH
  );
  const [environmentVariablesDockWidth, setEnvironmentVariablesDockWidth] =
    useState(ENVIRONMENT_VARIABLES_DOCK_WIDTH);
  const [conversationVariablesDockWidth, setConversationVariablesDockWidth] =
    useState(ENVIRONMENT_VARIABLES_DOCK_WIDTH);
  const [historyDockWidth, setHistoryDockWidth] = useState(HISTORY_DOCK_WIDTH);
  const environmentVariablesSourceRef = useRef(initialEnvironmentVariables);
  const [environmentVariables, setEnvironmentVariables] = useState<
    AgentFlowEnvironmentVariable[]
  >(initialEnvironmentVariables);
  if (environmentVariablesSourceRef.current !== initialEnvironmentVariables) {
    environmentVariablesSourceRef.current = initialEnvironmentVariables;
    setEnvironmentVariables(initialEnvironmentVariables);
  }
  const conversationVariables = useMemo(
    () => listConversationVariables(workingDocument),
    [workingDocument]
  );
  const [selectedVariable, setSelectedVariable] =
    useState<SelectedVariableInfo | null>(null);
  const [variableCacheHeight, setVariableCacheHeight] = useState(
    VARIABLE_CACHE_DEFAULT_HEIGHT
  );
  const [isResizingVariableCache, setIsResizingVariableCache] = useState(false);
  const [variableCacheSidebarWidth, setVariableCacheSidebarWidth] = useState(
    VARIABLE_CACHE_DEFAULT_SIDEBAR_WIDTH
  );
  const [isResizingVariableCacheSidebar, setIsResizingVariableCacheSidebar] =
    useState(false);
  const modelProviderOptionsQuery = useQuery({
    queryKey: modelProviderOptionsQueryKey,
    queryFn: fetchModelProviderOptions
  });
  const environmentVariablesMutation = useMutation({
    mutationFn: (variables: AgentFlowEnvironmentVariable[]) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return replaceApplicationEnvironmentVariables(
        applicationId,
        variables,
        csrfToken
      );
    },
    onSuccess(nextVariables) {
      setEnvironmentVariables(nextVariables);
      queryClient.setQueryData(
        applicationEnvironmentVariablesQueryKey(applicationId),
        nextVariables
      );
      message.success(
        i18nText('agentFlow', 'auto.environment_variables_saved')
      );
    },
    onError() {
      message.error(
        i18nText('agentFlow', 'auto.failed_save_environment_variables')
      );
    }
  });
  const exportTemplateMutation = useMutation({
    mutationFn: () => exportApplicationArchive([applicationId]),
    onSuccess(archive) {
      downloadApplicationArchive(archive);
      message.success(i18nText('agentFlow', 'auto.template_exported'));
    },
    onError() {
      message.error(i18nText('agentFlow', 'auto.template_export_failed'));
    }
  });
  const publishMutation = useMutation({
    mutationFn: async () => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      const mapping = await fetchApplicationApiMapping(applicationId);
      return publishApplicationApiVersion(applicationId, mapping, csrfToken);
    },
    onSuccess() {
      void queryClient.invalidateQueries({
        queryKey: applicationApiPublicationQueryKey(applicationId)
      });
      void queryClient.invalidateQueries({
        queryKey: applicationDetailQueryKey(applicationId)
      });
      message.success(i18nText('agentFlow', 'auto.posted_successfully'));
    },
    onError() {
      message.error(i18nText('agentFlow', 'auto.publishing_failed'));
    }
  });
  const versionMetadataMutation = useMutation({
    mutationFn: ({
      versionId,
      input
    }: {
      versionId: string;
      input: Parameters<typeof updateVersion>[2];
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return updateVersion(applicationId, versionId, input, csrfToken);
    },
    onSuccess(nextState) {
      syncSavedServerState(nextState);
      queryClient.setQueryData(orchestrationQueryKey(applicationId), nextState);
      message.success(i18nText('agentFlow', 'auto.historical_version_updated'));
    },
    onError() {
      message.error(
        i18nText('agentFlow', 'auto.historical_version_update_failed')
      );
    }
  });

  const navigation = useContainerNavigation();
  const draftSync = useDraftSync({
    applicationId,
    saveDraftOverride,
    restoreVersionOverride,
    getCurrentDocument: () =>
      getDocumentWithLatestViewport(
        documentRef.current,
        viewportGetterRef.current?.() ?? viewportSnapshotRef.current
      ),
    getLastSavedDocument: () => lastSavedDocumentRef.current,
    environmentVariables
  });
  const debugSession = useAgentFlowDebugSession({
    applicationId,
    draftId: draftMeta.draftId,
    document: workingDocument,
    environmentVariables
  });
  const conversationLogMessage = useMemo(
    () =>
      debugSession.messages.find(
        (debugMessage) =>
          debugMessage.id === conversationLogMessageId &&
          debugMessage.role === 'assistant'
      ) ?? null,
    [conversationLogMessageId, debugSession.messages]
  );
  if (conversationLogMessageId && !conversationLogMessage) {
    setConversationLogMessageId(null);
  }
  const issues = useMemo(
    () =>
      validateDocument(
        workingDocument,
        modelProviderOptionsQuery.isSuccess
          ? modelProviderOptionsQuery.data
          : null,
        environmentVariables
      ),
    [
      environmentVariables,
      workingDocument,
      modelProviderOptionsQuery.data,
      modelProviderOptionsQuery.isSuccess
    ]
  );
  const variableAggregatorSaveBlocked = issues.some(
    isVariableAggregatorCandidateTypeMismatchIssue
  );
  const activeContainerId = activeContainerPath.at(-1) ?? null;
  const detailActions = useNodeDetailActions();
  const nodePreviewMutation = useMutation({
    mutationFn: async ({
      nodeId,
      inputPayload
    }: {
      action: NodePreviewAction;
      nodeId: string;
      inputPayload: Record<string, Record<string, unknown>>;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return startNodeDebugPreview(
        applicationId,
        nodeId,
        {
          input_payload: inputPayload,
          document: getDocumentWithLatestViewport(
            documentRef.current,
            viewportGetterRef.current?.() ?? viewportSnapshotRef.current
          ),
          debug_session_id: debugSession.debugSessionId
        },
        csrfToken
      );
    },
    onSuccess: async (lastRun, variables) => {
      queryClient.setQueryData(
        nodeLastRunQueryKey(applicationId, variables.nodeId),
        lastRun
      );
      queryClient.setQueryData(
        applicationRunNodeLastRunQueryKey(
          applicationId,
          lastRun.flow_run.id,
          variables.nodeId
        ),
        lastRun
      );
      debugSession.rememberExternalRunDetail(
        nodeLastRunToFlowDebugRunDetail(lastRun)
      );
      setNodeDetailTab('lastRun');
      await queryClient.invalidateQueries({
        queryKey: ['applications', applicationId, 'runtime']
      });
    },
    onError(error) {
      message.error(
        error instanceof Error && error.message
          ? error.message
          : i18nText('agentFlow', 'auto.debug_run_failed')
      );
    }
  });
  const issueCountByNodeId = useMemo(
    () => countIssuesByNodeId(issues),
    [issues]
  );
  const issueErrorCount = useMemo(
    () => issues.filter((issue) => issue.level === 'error').length,
    [issues]
  );
  const nodePickerOptions = useMemo(
    () => buildNodePickerOptions(nodeCatalog.nodes),
    [nodeCatalog.nodes]
  );

  useEffect(() => {
    documentRef.current = workingDocument;
  }, [workingDocument]);

  useEffect(() => {
    lastSavedDocumentRef.current = lastSavedDocument;
  }, [lastSavedDocument]);

  useEffect(() => {
    viewportSnapshotRef.current = workingDocument.editor.viewport;
  }, [workingDocument.editor.viewport]);

  useEffect(() => {
    const element = bodyRef.current;

    if (!element) {
      return;
    }

    const resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0];

      if (!entry) {
        return;
      }

      setBodyWidth(entry.contentRect.width);
      setBodyHeight(entry.contentRect.height);
    });

    resizeObserver.observe(element);
    const bodyRect = element.getBoundingClientRect();
    setBodyWidth(bodyRect.width);
    setBodyHeight(bodyRect.height);

    return () => resizeObserver.disconnect();
  }, []);

  useEffect(() => {
    const stopNodeDetailResize = stopNodeDetailResizeRef;
    const stopDebugConsoleResize = stopDebugConsoleResizeRef;
    const stopConversationLogResize = stopConversationLogResizeRef;
    const stopVariablesDockResize = stopVariablesDockResizeRef;
    const stopHistoryDockResize = stopHistoryDockResizeRef;
    const stopVariableCacheResize = stopVariableCacheResizeRef;
    const stopVariableCacheSidebarResize = stopVariableCacheSidebarResizeRef;

    return () => {
      stopNodeDetailResize.current?.();
      stopDebugConsoleResize.current?.();
      stopConversationLogResize.current?.();
      stopVariablesDockResize.current?.();
      stopHistoryDockResize.current?.();
      stopVariableCacheResize.current?.();
      stopVariableCacheSidebarResize.current?.();
    };
  }, []);

  useEffect(() => {
    if (selectedNodeId) {
      return;
    }

    stopNodeDetailResizeRef.current?.();
  }, [selectedNodeId]);

  useEffect(() => {
    if (conversationLogMessage) {
      return;
    }

    stopConversationLogResizeRef.current?.();
  }, [conversationLogMessage]);

  useEffect(() => {
    if (
      systemVariablesOpen ||
      environmentVariablesOpen ||
      conversationVariablesOpen
    ) {
      return;
    }

    stopVariablesDockResizeRef.current?.();
  }, [
    conversationVariablesOpen,
    environmentVariablesOpen,
    systemVariablesOpen
  ]);

  useEffect(() => {
    if (historyOpen) {
      return;
    }

    stopHistoryDockResizeRef.current?.();
  }, [historyOpen]);

  useEffect(() => {
    if (variableCacheOpen) {
      return;
    }

    stopVariableCacheResizeRef.current?.();
  }, [variableCacheOpen]);

  useEffect(() => {
    if (variableCacheOpen) {
      return;
    }

    stopVariableCacheSidebarResizeRef.current?.();
  }, [variableCacheOpen]);

  useEditorShortcuts();

  const {
    boundedConversationLogWidth,
    boundedDebugConsoleWidth,
    boundedHistoryDockWidth,
    boundedNodeDetailWidth,
    boundedVariableCacheHeight,
    boundedVariableCacheSidebarWidth,
    boundedVariablesDockWidth,
    canvasFrameWidth,
    conversationLogOpen,
    detailContainerWidth,
    nodeDetailLayout,
    sideDockOccupiedWidth,
    variableCacheCenterLeft,
    variableCacheMaxHeight,
    variableCacheRightOffset,
    variableCacheSidebarMaxWidth,
    variablesDockOpen
  } = deriveCanvasFrameLayout({
    bodyHeight,
    bodyWidth,
    conversationLogMessageOpen: conversationLogMessage !== null,
    conversationLogWidth,
    conversationVariablesDockWidth,
    conversationVariablesOpen,
    debugConsoleOpen,
    debugConsoleWidth,
    environmentVariablesDockWidth,
    environmentVariablesOpen,
    historyDockWidth,
    historyOpen,
    nodeDetailWidth,
    selectedNodeId,
    systemVariablesDockWidth,
    systemVariablesOpen,
    variableCacheHeight,
    variableCacheSidebarWidth
  });

  const {
    handleConversationLogResizeStart,
    handleDebugConsoleResizeStart,
    handleHistoryDockResizeStart,
    handleNodeDetailResizeStart,
    handleVariableCacheResizeStart,
    handleVariableCacheSidebarResizeStart,
    handleVariablesDockResizeStart
  } = createCanvasFrameResizeHandlers({
    boundedConversationLogWidth,
    boundedDebugConsoleWidth,
    boundedHistoryDockWidth,
    boundedNodeDetailWidth,
    boundedVariableCacheHeight,
    boundedVariableCacheSidebarWidth,
    boundedVariablesDockWidth,
    canvasFrameWidth,
    conversationVariablesOpen,
    detailContainerWidth,
    environmentVariablesOpen,
    selectedNodeId,
    variableCacheMaxHeight,
    variableCacheSidebarMaxWidth,
    setConversationLogWidth,
    setConversationVariablesDockWidth,
    setEnvironmentVariablesDockWidth,
    setHistoryDockWidth,
    setDebugConsoleWidth,
    setIsResizingConversationLog,
    setIsResizingDebugConsole,
    setIsResizingHistoryDock,
    setIsResizingNodeDetail,
    setIsResizingVariableCache,
    setIsResizingVariableCacheSidebar,
    setIsResizingVariablesDock,
    setPanelState,
    setSystemVariablesDockWidth,
    setVariableCacheHeight,
    setVariableCacheSidebarWidth,
    stopConversationLogResizeRef,
    stopDebugConsoleResizeRef,
    stopHistoryDockResizeRef,
    stopNodeDetailResizeRef,
    stopVariableCacheResizeRef,
    stopVariableCacheSidebarResizeRef,
    stopVariablesDockResizeRef
  });
  function handleResetVariableCache() {
    debugSession.resetVariableCache();
    setSelectedVariable(null);
    message.success(i18nText('agentFlow', 'auto.variable_cache_reset'));
  }

  function handleVariableCacheValueChange(key: string, value: unknown) {
    if (
      selectedVariable?.isReadOnly ||
      !selectedVariable ||
      selectedVariable.key !== key
    ) {
      return;
    }

    setSelectedVariable((current) =>
      current?.key === key
        ? {
            ...current,
            value
          }
        : current
    );
    debugSession.setVariableCacheValue(key, value);
  }

  function handleNodeDetailResizeKeyDown(
    event: ReactKeyboardEvent<HTMLDivElement>
  ) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') {
      return;
    }

    event.preventDefault();
    const direction = event.key === 'ArrowLeft' ? 1 : -1;
    setPanelState({
      nodeDetailWidth: clampNodeDetailWidth(
        boundedNodeDetailWidth + direction * 24,
        detailContainerWidth
      )
    });
  }

  function buildNodeDebugRuntimeVariableCache(): NodeDebugPreviewVariableCache {
    const cache = debugSession.getNodePreviewVariableCache();
    const environmentPayload = environmentVariables.reduce<
      Record<string, unknown>
    >((payload, variable) => {
      payload[variable.name] = variable.value;
      return payload;
    }, {});
    const systemPayload: Record<string, unknown> = {
      conversation_id: debugSession.debugSessionId,
      dialog_count: 0,
      user_id: actorUserId ?? '',
      application_id: applicationId,
      workflow_id: draftMeta.flowId,
      workflow_run_id: debugSession.activeRunId ?? '',
      model_parameters: {}
    };

    return {
      ...cache,
      [environmentVariableNodeId]: {
        ...environmentPayload,
        ...(cache[environmentVariableNodeId] ?? {})
      },
      [systemVariableNodeId]: {
        ...systemPayload,
        ...(cache[systemVariableNodeId] ?? {})
      }
    };
  }

  function runNodePreview(
    action: NodePreviewAction,
    nodeId: string,
    inputPayload: Record<string, Record<string, unknown>>
  ) {
    debugSession.rememberNodePreviewInputs(inputPayload);
    nodePreviewMutation.mutate({ action, nodeId, inputPayload });
  }

  function handleRunNode(nodeId: string) {
    const plan = buildNodeDebugPreviewPlan(
      documentRef.current,
      nodeId,
      buildNodeDebugRuntimeVariableCache()
    );

    if (plan.missing_fields.length > 0) {
      setPendingNodePreview({
        action: 'run',
        nodeId,
        inputPayload: plan.input_payload,
        fields: plan.missing_fields
      });
      return;
    }

    runNodePreview('run', nodeId, plan.input_payload);
  }

  function handleDebugNode(nodeId: string) {
    const plan = buildNodeDebugVariableConfirmationPlan(
      documentRef.current,
      nodeId,
      buildNodeDebugRuntimeVariableCache()
    );

    if (plan.fields.length > 0) {
      setPendingNodePreview({
        action: 'debug',
        nodeId,
        inputPayload: plan.input_payload,
        fields: plan.fields
      });
      return;
    }

    runNodePreview('debug', nodeId, plan.input_payload);
  }

  function handleSubmitNodePreviewVariables(
    inputPayload: Record<string, Record<string, unknown>>
  ) {
    if (!pendingNodePreview) {
      return;
    }

    const mergedInputPayload = { ...pendingNodePreview.inputPayload };

    for (const [nodeId, payload] of Object.entries(inputPayload)) {
      mergedInputPayload[nodeId] = {
        ...(mergedInputPayload[nodeId] ?? {}),
        ...payload
      };
    }

    const { action, nodeId } = pendingNodePreview;

    setPendingNodePreview(null);
    runNodePreview(action, nodeId, mergedInputPayload);
  }

  function handleRunSelectedNode() {
    if (!selectedNodeId) {
      return;
    }

    handleRunNode(selectedNodeId);
  }

  function handleDebugSelectedNode() {
    if (!selectedNodeId) {
      return;
    }

    handleDebugNode(selectedNodeId);
  }

  function openDebugConsole() {
    setEnvironmentVariablesOpen(false);
    setSystemVariablesOpen(false);
    setConversationVariablesOpen(false);
    setDebugConsoleOpen(true);
    setPanelState({ historyOpen: false });
  }

  function closeDebugConsole() {
    stopDebugConsoleResizeRef.current?.();
    stopConversationLogResizeRef.current?.();
    setConversationLogMessageId(null);
    setDebugConsoleOpen(false);
  }

  function openEnvironmentVariables() {
    closeDebugConsole();
    setPanelState({ historyOpen: false });
    setSystemVariablesOpen(false);
    setConversationVariablesOpen(false);
    setEnvironmentVariablesOpen(true);
  }

  function openConversationVariables() {
    closeDebugConsole();
    setPanelState({ historyOpen: false });
    setSystemVariablesOpen(false);
    setEnvironmentVariablesOpen(false);
    setConversationVariablesOpen(true);
  }

  function openSystemVariables() {
    closeDebugConsole();
    setPanelState({ historyOpen: false });
    setEnvironmentVariablesOpen(false);
    setConversationVariablesOpen(false);
    setSystemVariablesOpen(true);
  }

  function openHistory() {
    setEnvironmentVariablesOpen(false);
    setSystemVariablesOpen(false);
    setConversationVariablesOpen(false);
    closeDebugConsole();
    setPanelState({ historyOpen: true });
  }

  const nodePreviewAction = nodePreviewMutation.isPending
    ? (nodePreviewMutation.variables?.action ?? null)
    : null;

  return (
    <section
      aria-label={`${applicationName} editor`}
      className="agent-flow-editor"
      data-application-id={applicationId}
    >
      <AgentFlowOverlay
        applicationName={applicationName}
        autosaveLabel={i18nText(
          'agentFlow',
          'auto.automatically_save_seconds',
          { value1: Math.round(autosaveIntervalMs / 1000) }
        )}
        autosaveStatus={autosaveStatus}
        onSaveDraft={() => {
          void draftSync.saveNow();
        }}
        saveDisabled={
          autosaveStatus === 'saving' || variableAggregatorSaveBlocked
        }
        saveLoading={autosaveStatus === 'saving'}
        onOpenDebugConsole={openDebugConsole}
        onExportTemplate={() => exportTemplateMutation.mutate()}
        onOpenIssues={() => setPanelState({ issuesOpen: true })}
        onOpenHistory={openHistory}
        onOpenConversationVariables={openConversationVariables}
        onOpenEnvironmentVariables={openEnvironmentVariables}
        onOpenSystemVariables={openSystemVariables}
        onOpenPublish={() => publishMutation.mutate()}
        issueErrorCount={issueErrorCount}
        exportLoading={exportTemplateMutation.isPending}
        publishDisabled={publishMutation.isPending || issueErrorCount > 0}
      />
      {activeContainerId ? (
        <div className="agent-flow-editor__breadcrumb">
          <Button onClick={navigation.returnToRoot}>
            {i18nText('agentFlow', 'auto.return_main_canvas')}
          </Button>
          <Typography.Text type="secondary">
            {i18nText('agentFlow', 'auto.currently_located_container_node')}{' '}
            {
              workingDocument.graph.nodes.find(
                (node) => node.id === activeContainerId
              )?.alias
            }
          </Typography.Text>
        </div>
      ) : null}
      <div
        ref={bodyRef}
        className="agent-flow-editor__body agent-flow-editor__shell"
        data-testid="agent-flow-editor-body"
      >
        <AgentFlowCanvas
          issueCountByNodeId={issueCountByNodeId}
          nodePickerOptions={nodePickerOptions}
          onRunNode={handleRunNode}
          onViewportSnapshotChange={(viewport) => {
            viewportSnapshotRef.current = viewport;
          }}
          onViewportGetterReady={(getter) => {
            viewportGetterRef.current = getter;
          }}
        />
        <Button
          className="agent-flow-editor__variable-cache-trigger"
          size="small"
          type="link"
          style={{ left: variableCacheCenterLeft }}
          onClick={() => setVariableCacheOpen(true)}
        >
          {i18nText('agentFlow', 'auto.view_cache')}
        </Button>
        {variablesDockOpen ? (
          <AgentFlowSideDock
            className="agent-flow-editor__variables-dock"
            data-testid="agent-flow-editor-variables-dock"
            isResizing={isResizingVariablesDock}
            resizeLabel={
              conversationVariablesOpen
                ? i18nText(
                    'agentFlow',
                    'auto.adjust_conversation_variable_width'
                  )
                : environmentVariablesOpen
                  ? i18nText(
                      'agentFlow',
                      'auto.adjust_environment_variable_width'
                    )
                  : i18nText('agentFlow', 'auto.adjust_system_variable_width')
            }
            width={boundedVariablesDockWidth}
            onResizeStart={handleVariablesDockResizeStart}
          >
            {systemVariablesOpen ? (
              <SystemVariablesPanel
                onClose={() => setSystemVariablesOpen(false)}
              />
            ) : conversationVariablesOpen ? (
              <ConversationVariablesPanel
                variables={conversationVariables}
                onClose={() => setConversationVariablesOpen(false)}
                onSave={(nextVariables) => {
                  setWorkingDocument((currentDocument) =>
                    replaceConversationVariables(currentDocument, nextVariables)
                  );
                  message.success(
                    i18nText('agentFlow', 'auto.conversation_variables_saved')
                  );
                }}
              />
            ) : (
              <ApplicationEnvironmentVariablesPanel
                loading={environmentVariablesMutation.isPending}
                variables={environmentVariables}
                onClose={() => setEnvironmentVariablesOpen(false)}
                onSave={(nextVariables) =>
                  environmentVariablesMutation.mutate(nextVariables)
                }
              />
            )}
          </AgentFlowSideDock>
        ) : null}
        {selectedNodeId ? (
          <div
            className="agent-flow-editor__detail-dock"
            data-layout={nodeDetailLayout}
            data-testid="agent-flow-editor-detail-dock"
            data-resizing={isResizingNodeDetail ? 'true' : 'false'}
            style={{
              right: sideDockOccupiedWidth
                ? `${sideDockOccupiedWidth + 16}px`
                : undefined,
              width: `${boundedNodeDetailWidth}px`
            }}
          >
            <div
              aria-label={i18nText(
                'agentFlow',
                'auto.adjust_node_detail_width'
              )}
              aria-orientation="vertical"
              className="agent-flow-editor__detail-resize-handle"
              onKeyDown={handleNodeDetailResizeKeyDown}
              onMouseDown={handleNodeDetailResizeStart}
              role="separator"
              tabIndex={0}
            />
            <NodeDetailPanel
              activeTab={nodeDetailTab}
              onTabChange={setNodeDetailTab}
              activeRunId={debugSession.activeRunId}
              applicationId={applicationId}
              debugLoading={nodePreviewAction === 'debug'}
              environmentVariables={environmentVariables}
              issues={issues}
              onClose={detailActions.closeDetail}
              onDebugNode={selectedNodeId ? handleDebugSelectedNode : undefined}
              onResolveRunScope={debugSession.selectRunScope}
              onRunNode={selectedNodeId ? handleRunSelectedNode : undefined}
              previewActionsDisabled={nodePreviewMutation.isPending}
              runLoading={nodePreviewAction === 'run'}
            />
          </div>
        ) : null}
        {variableCacheOpen ? (
          <AgentFlowVariableCachePanel
            applicationId={applicationId}
            groups={debugSession.variableGroups}
            height={boundedVariableCacheHeight}
            isResizing={isResizingVariableCache}
            isSidebarResizing={isResizingVariableCacheSidebar}
            onClose={() => setVariableCacheOpen(false)}
            onReset={handleResetVariableCache}
            onResizeStart={handleVariableCacheResizeStart}
            onSelectedChange={setSelectedVariable}
            onSelectedValueChange={handleVariableCacheValueChange}
            onSidebarResizeStart={handleVariableCacheSidebarResizeStart}
            rightOffset={variableCacheRightOffset}
            selectedVariable={selectedVariable}
            sidebarMaxWidth={variableCacheSidebarMaxWidth}
            sidebarMinWidth={VARIABLE_CACHE_MIN_SIDEBAR_WIDTH}
            sidebarWidth={boundedVariableCacheSidebarWidth}
          />
        ) : null}
        {conversationLogOpen && conversationLogMessage ? (
          <AgentFlowSideDock
            className="agent-flow-editor__conversation-log-dock"
            data-testid="agent-flow-editor-conversation-log-dock"
            isResizing={isResizingConversationLog}
            resizeLabel={i18nText(
              'agentFlow',
              'auto.adjust_conversation_log_width'
            )}
            style={{
              right: `${16 + boundedDebugConsoleWidth + DEBUG_CONSOLE_GAP}px`
            }}
            width={boundedConversationLogWidth}
            onResizeStart={handleConversationLogResizeStart}
          >
            <ConversationLogPanel
              message={conversationLogMessage}
              onClose={() => setConversationLogMessageId(null)}
              onLoadArtifact={(artifactRef) =>
                fetchRuntimeDebugArtifact(applicationId, artifactRef)
              }
              onLoadArtifacts={(artifactRefs) =>
                fetchRuntimeDebugArtifacts(applicationId, artifactRefs)
              }
            />
          </AgentFlowSideDock>
        ) : null}
        {debugConsoleOpen ? (
          <AgentFlowSideDock
            className="agent-flow-editor__debug-console-dock"
            data-testid="agent-flow-editor-debug-console-dock"
            isResizing={isResizingDebugConsole}
            resizeLabel={i18nText('agentFlow', 'auto.adjust_preview_width')}
            width={boundedDebugConsoleWidth}
            onResizeStart={handleDebugConsoleResizeStart}
          >
            <AgentFlowDebugConsole
              messages={debugSession.messages}
              runContext={debugSession.runContext}
              status={debugSession.status}
              stopping={debugSession.stopping}
              onChangeRunContextValue={debugSession.setRunContextValue}
              onClearSession={() => {
                setConversationLogMessageId(null);
                debugSession.clearSession();
              }}
              onClose={closeDebugConsole}
              onLoadArtifact={(artifactRef) =>
                fetchRuntimeDebugArtifact(applicationId, artifactRef)
              }
              onLoadArtifacts={(artifactRefs) =>
                fetchRuntimeDebugArtifacts(applicationId, artifactRefs)
              }
              onOpenMessageLog={(debugMessage) =>
                setConversationLogMessageId(debugMessage.id)
              }
              onStopRun={() => {
                void debugSession.stopRun();
              }}
              onSubmitPrompt={(prompt) => {
                void debugSession.submitPrompt(prompt);
              }}
            />
          </AgentFlowSideDock>
        ) : null}
        {historyOpen ? (
          <AgentFlowSideDock
            className="agent-flow-editor__history-dock"
            data-testid="agent-flow-editor-history-dock"
            isResizing={isResizingHistoryDock}
            resizeLabel={i18nText(
              'agentFlow',
              'auto.adjust_historical_version_width'
            )}
            width={boundedHistoryDockWidth}
            onResizeStart={handleHistoryDockResizeStart}
          >
            <VersionHistoryPanel
              versions={versions}
              userProtectionLimit={userProtectionLimit}
              restoring={isRestoringVersion}
              updatingVersionId={
                versionMetadataMutation.isPending
                  ? (versionMetadataMutation.variables?.versionId ?? null)
                  : null
              }
              onClose={() => setPanelState({ historyOpen: false })}
              onRestore={draftSync.restoreVersion}
              onUpdate={(versionId, input) =>
                versionMetadataMutation.mutateAsync({ versionId, input })
              }
            />
          </AgentFlowSideDock>
        ) : null}
      </div>
      {issues.some((issue) => issue.scope === 'global') ? (
        <Typography.Text type="danger">
          {i18nText(
            'agentFlow',
            'auto.global_issues_draft_check_issues_panel_first_deal'
          )}
        </Typography.Text>
      ) : null}
      <NodePreviewVariablesModal
        confirmLoading={nodePreviewMutation.isPending}
        fields={pendingNodePreview?.fields ?? []}
        open={Boolean(pendingNodePreview)}
        onCancel={() => setPendingNodePreview(null)}
        onSubmit={handleSubmitNodePreviewVariables}
      />
      <IssuesDrawer
        open={issuesOpen}
        issues={issues}
        onClose={() => setPanelState({ issuesOpen: false })}
        onSelectIssue={navigation.jumpToIssue}
      />
    </section>
  );
}
