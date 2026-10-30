import { useCanvasDockLayout } from './canvas-frame/use-canvas-dock-layout';
import { useCanvasNodePreview } from './canvas-frame/use-canvas-node-preview';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { App, Button, Typography } from 'antd';
import {
  type ReactNode,
  useCallback,
  useEffect,
  lazy,
  useMemo,
  useRef,
  Suspense,
  useState
} from 'react';

import { useContainerNavigation } from '../../hooks/interactions/use-container-navigation';
import { useDraftSync } from '../../hooks/interactions/use-draft-sync';
import { useEditorShortcuts } from '../../hooks/interactions/use-editor-shortcuts';
import { useNodeDetailActions } from '../../hooks/interactions/use-node-detail-actions';
import { useAgentFlowDebugSession } from '../../hooks/runtime/useAgentFlowDebugSession';
import {
  fetchRuntimeDebugArtifact,
  fetchRuntimeDebugArtifacts
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
import { type AgentFlowEnvironmentVariable } from '../../lib/variables/application-environment-variables';
import {
  listConversationVariables,
  replaceConversationVariables
} from '../../lib/variables/conversation-variables';

import {
  fetchModelProviderOptions,
  modelProviderOptionsQueryKey
} from '../../api/model-provider-options';

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
import { type SelectedVariableInfo } from './AgentFlowVariableCachePanel';
import { AgentFlowCanvas } from './AgentFlowCanvas';
import { AgentFlowOverlay } from './AgentFlowOverlay';
import { AgentFlowSideDock } from './AgentFlowSideDock';
import { ApplicationEnvironmentVariablesPanel } from './ApplicationEnvironmentVariablesPanel';
import { ConversationVariablesPanel } from './ConversationVariablesPanel';
import { SystemVariablesPanel } from './SystemVariablesPanel';
import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';
import { i18nText } from '../../../../shared/i18n/text';
import { downloadApplicationArchive } from '../../../applications/lib/template-download';
import {
  DEBUG_CONSOLE_GAP,
  DEBUG_CONSOLE_MIN_WIDTH,
  CONVERSATION_LOG_MIN_WIDTH,
  VARIABLES_DOCK_MIN_WIDTH,
  HISTORY_DOCK_MIN_WIDTH,
  VARIABLE_CACHE_MIN_HEIGHT,
  VARIABLE_CACHE_MIN_SIDEBAR_WIDTH
} from './canvas-frame/layout';

import {
  countIssuesByNodeId,
  getDocumentWithLatestViewport
} from './canvas-frame/document';
import { type AgentFlowCanvasFrameProps } from './canvas-frame/types';

const DraftAssistantPreview = lazy(() =>
  import('../assistant-plugin/DraftAssistantPreview').then((module) => ({
    default: module.DraftAssistantPreview
  }))
);
const ConversationLogPanel = lazy(() =>
  import('../debug-console/ConversationLogPanel').then((module) => ({
    default: module.ConversationLogPanel
  }))
);
const AgentFlowVariableCachePanel = lazy(() =>
  import('./AgentFlowVariableCachePanel').then((module) => ({
    default: module.AgentFlowVariableCachePanel
  }))
);
const NodeDetailPanel = lazy(() =>
  import('../detail/NodeDetailPanel').then((module) => ({
    default: module.NodeDetailPanel
  }))
);
const NodePreviewVariablesModal = lazy(() =>
  import('../detail/NodePreviewVariablesModal').then((module) => ({
    default: module.NodePreviewVariablesModal
  }))
);
const VersionHistoryPanel = lazy(() =>
  import('../history/VersionHistoryPanel').then((module) => ({
    default: module.VersionHistoryPanel
  }))
);
const IssuesDrawer = lazy(() =>
  import('../issues/IssuesDrawer').then((module) => ({
    default: module.IssuesDrawer
  }))
);

function DemandIsland({ children }: { children: ReactNode }) {
  return <Suspense fallback={null}>{children}</Suspense>;
}

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
  const [conversationLogMessageId, setConversationLogMessageId] = useState<
    string | null
  >(null);
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
  const handleViewportSnapshotChange = useCallback(
    (viewport: FlowAuthoringDocument['editor']['viewport']) => {
      viewportSnapshotRef.current = viewport;
    },
    []
  );
  const handleViewportGetterReady = useCallback(
    (getter: (() => FlowAuthoringDocument['editor']['viewport']) | null) => {
      viewportGetterRef.current = getter;
    },
    []
  );
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
  const {
    nodeDetailTab,
    setNodeDetailTab,
    pendingNodePreview,
    setPendingNodePreview,
    nodePreviewMutation,
    handleCanvasRunNode,
    handleSubmitNodePreviewVariables,
    handleRunSelectedNode,
    handleDebugSelectedNode
  } = useCanvasNodePreview({
    applicationId,
    flowId: draftMeta.flowId,
    selectedNodeId,
    environmentVariables,
    debugSession,
    documentRef,
    viewportGetterRef,
    viewportSnapshotRef
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
  const {
    stopDebugConsoleResizeRef,
    stopConversationLogResizeRef,
    bodyRef,
    debugConsoleOpen,
    setDebugConsoleOpen,
    isResizingNodeDetail,
    isResizingDebugConsole,
    isResizingConversationLog,
    isResizingVariablesDock,
    isResizingHistoryDock,
    variableCacheOpen,
    setVariableCacheOpen,
    systemVariablesOpen,
    setSystemVariablesOpen,
    environmentVariablesOpen,
    setEnvironmentVariablesOpen,
    conversationVariablesOpen,
    setConversationVariablesOpen,
    isResizingVariableCache,
    isResizingVariableCacheSidebar,
    boundedConversationLogWidth,
    boundedDebugConsoleWidth,
    boundedHistoryDockWidth,
    boundedNodeDetailWidth,
    boundedVariableCacheHeight,
    boundedVariableCacheSidebarWidth,
    boundedVariablesDockWidth,
    conversationLogOpen,
    nodeDetailLayout,
    sideDockOccupiedWidth,
    variableCacheCenterLeft,
    variableCacheMaxHeight,
    variableCacheRightOffset,
    variableCacheSidebarMaxWidth,
    variablesDockOpen,
    handleConversationLogResizeStart,
    handleDebugConsoleResizeStart,
    handleHistoryDockResizeStart,
    handleNodeDetailResizeStart,
    handleVariableCacheResizeStart,
    handleVariableCacheSidebarResizeStart,
    handleVariablesDockResizeStart,
    maxDebugConsoleWidth,
    maxConversationLogWidth,
    maxVariablesDockWidth,
    maxHistoryDockWidth,
    handleDebugConsoleResizeKeyDown,
    handleConversationLogResizeKeyDown,
    handleVariablesDockResizeKeyDown,
    handleHistoryDockResizeKeyDown,
    handleVariableCacheSidebarResizeKeyDown,
    handleNodeDetailResizeKeyDown,
    handleVariableCacheResizeKeyDown
  } = useCanvasDockLayout({
    selectedNodeId,
    historyOpen,
    nodeDetailWidth,
    setPanelState,
    conversationLogMessage
  });
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
  const detailActions = useNodeDetailActions();
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

  useEditorShortcuts();

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
    if (debugConsoleOpen) {
      debugSession.closeSession();
    }
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
      <div
        ref={bodyRef}
        className="agent-flow-editor__body agent-flow-editor__shell"
        data-testid="agent-flow-editor-body"
      >
        <AgentFlowCanvas
          issueCountByNodeId={issueCountByNodeId}
          nodePickerOptions={nodePickerOptions}
          onRunNode={handleCanvasRunNode}
          onViewportSnapshotChange={handleViewportSnapshotChange}
          onViewportGetterReady={handleViewportGetterReady}
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
            minWidth={VARIABLES_DOCK_MIN_WIDTH}
            maxWidth={maxVariablesDockWidth}
            onResizeKeyDown={handleVariablesDockResizeKeyDown}
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
            <Suspense
              fallback={
                <LoadingState compact className="loading-state--panel" />
              }
            >
              <NodeDetailPanel
                activeTab={nodeDetailTab}
                onTabChange={setNodeDetailTab}
                activeRunId={debugSession.activeRunId}
                applicationId={applicationId}
                debugLoading={nodePreviewAction === 'debug'}
                environmentVariables={environmentVariables}
                issues={issues}
                onClose={detailActions.closeDetail}
                onDebugNode={
                  selectedNodeId ? handleDebugSelectedNode : undefined
                }
                onResolveRunScope={debugSession.selectRunScope}
                onRunNode={selectedNodeId ? handleRunSelectedNode : undefined}
                previewActionsDisabled={nodePreviewMutation.isPending}
                runLoading={nodePreviewAction === 'run'}
              />
            </Suspense>
          </div>
        ) : null}
        {variableCacheOpen ? (
          <DemandIsland>
            <AgentFlowVariableCachePanel
              applicationId={applicationId}
              groups={debugSession.variableGroups}
              height={boundedVariableCacheHeight}
              isResizing={isResizingVariableCache}
              isSidebarResizing={isResizingVariableCacheSidebar}
              onClose={() => setVariableCacheOpen(false)}
              onReset={handleResetVariableCache}
              onResizeStart={handleVariableCacheResizeStart}
              onResizeKeyDown={handleVariableCacheResizeKeyDown}
              minHeight={VARIABLE_CACHE_MIN_HEIGHT}
              maxHeight={variableCacheMaxHeight}
              onSelectedChange={setSelectedVariable}
              onSelectedValueChange={handleVariableCacheValueChange}
              onSidebarResizeStart={handleVariableCacheSidebarResizeStart}
              onSidebarResizeKeyDown={handleVariableCacheSidebarResizeKeyDown}
              rightOffset={variableCacheRightOffset}
              selectedVariable={selectedVariable}
              sidebarMaxWidth={variableCacheSidebarMaxWidth}
              sidebarMinWidth={VARIABLE_CACHE_MIN_SIDEBAR_WIDTH}
              sidebarWidth={boundedVariableCacheSidebarWidth}
            />
          </DemandIsland>
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
            minWidth={CONVERSATION_LOG_MIN_WIDTH}
            maxWidth={maxConversationLogWidth}
            onResizeKeyDown={handleConversationLogResizeKeyDown}
            onResizeStart={handleConversationLogResizeStart}
          >
            <Suspense
              fallback={
                <LoadingState compact className="loading-state--panel" />
              }
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
            </Suspense>
          </AgentFlowSideDock>
        ) : null}
        {debugConsoleOpen ? (
          <AgentFlowSideDock
            className="agent-flow-editor__debug-console-dock"
            data-testid="agent-flow-editor-debug-console-dock"
            isResizing={isResizingDebugConsole}
            resizeLabel={i18nText('agentFlow', 'auto.adjust_preview_width')}
            width={boundedDebugConsoleWidth}
            minWidth={DEBUG_CONSOLE_MIN_WIDTH}
            maxWidth={maxDebugConsoleWidth}
            onResizeKeyDown={handleDebugConsoleResizeKeyDown}
            onResizeStart={handleDebugConsoleResizeStart}
          >
            <Suspense
              fallback={
                <LoadingState compact className="loading-state--panel" />
              }
            >
              <DraftAssistantPreview
                applicationId={applicationId}
                mcp_instance_ids={debugSession.mcp_instance_ids}
                onChangeMcpInstanceIds={debugSession.setMcpInstanceIds}
                document={workingDocument}
                applicationName={applicationName}
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
            </Suspense>
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
            minWidth={HISTORY_DOCK_MIN_WIDTH}
            maxWidth={maxHistoryDockWidth}
            onResizeKeyDown={handleHistoryDockResizeKeyDown}
            onResizeStart={handleHistoryDockResizeStart}
          >
            <Suspense
              fallback={
                <LoadingState compact className="loading-state--panel" />
              }
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
            </Suspense>
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
      {pendingNodePreview ? (
        <DemandIsland>
          <NodePreviewVariablesModal
            confirmLoading={nodePreviewMutation.isPending}
            fields={pendingNodePreview.fields}
            open
            onCancel={() => setPendingNodePreview(null)}
            onSubmit={handleSubmitNodePreviewVariables}
          />
        </DemandIsland>
      ) : null}
      {issuesOpen ? (
        <DemandIsland>
          <IssuesDrawer
            open
            issues={issues}
            onClose={() => setPanelState({ issuesOpen: false })}
            onSelectIssue={navigation.jumpToIssue}
          />
        </DemandIsland>
      ) : null}
    </section>
  );
}
