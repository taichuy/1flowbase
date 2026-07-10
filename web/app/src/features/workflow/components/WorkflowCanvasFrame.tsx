import { useMutation, useQueryClient } from '@tanstack/react-query';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { App, Button, Typography } from 'antd';
import { useMemo, useRef, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import type {
  ConsoleApplicationEnvironmentVariable,
  ConsoleNodeContributionEntry,
  SaveConsoleApplicationDraftInput,
  ConsoleApplicationOrchestrationState
} from '@1flowbase/api-client';

import { useAuthStore } from '../../../state/auth-store';
import { i18nText } from '../../../shared/i18n/text';
import {
  applicationEnvironmentVariablesQueryKey,
  replaceApplicationEnvironmentVariables
} from '../../applications/api/applications';
import {
  orchestrationQueryKey,
  updateVersion
} from '../../agent-flow/api/orchestration';
import { AgentFlowCanvas } from '../../agent-flow/components/editor/AgentFlowCanvas';
import { AgentFlowSideDock } from '../../agent-flow/components/editor/AgentFlowSideDock';
import { ApplicationEnvironmentVariablesPanel } from '../../agent-flow/components/editor/ApplicationEnvironmentVariablesPanel';
import { VersionHistoryPanel } from '../../agent-flow/components/history/VersionHistoryPanel';
import { IssuesDrawer } from '../../agent-flow/components/issues/IssuesDrawer';
import { useContainerNavigation } from '../../agent-flow/hooks/interactions/use-container-navigation';
import { useDraftSync } from '../../agent-flow/hooks/interactions/use-draft-sync';
import { useEditorShortcuts } from '../../agent-flow/hooks/interactions/use-editor-shortcuts';
import { useNodeDetailActions } from '../../agent-flow/hooks/interactions/use-node-detail-actions';
import { clampNodeDetailWidth } from '../../agent-flow/lib/detail-panel-width';
import type { AgentFlowEnvironmentVariable } from '../../agent-flow/lib/variables/application-environment-variables';
import { useAgentFlowEditorStore } from '../../agent-flow/store/editor/provider';
import {
  selectAutosaveStatus,
  selectLastSavedDocument,
  selectUserProtectionLimit,
  selectVersions,
  selectWorkingDocument
} from '../../agent-flow/store/editor/selectors';
import {
  countIssuesByNodeId,
  getDocumentWithLatestViewport
} from '../../agent-flow/components/editor/canvas-frame/document';
import { buildWorkflowNodePickerOptions } from '../lib/picker-options';
import type { WorkflowTriggerContext } from '../lib/trigger-context';
import { validateWorkflowDocument } from '../lib/validate-document';
import { WorkflowNodeDetailPanel } from './WorkflowNodeDetailPanel';
import { WorkflowOverlay } from './WorkflowOverlay';
import { WorkflowTestRunPanel } from './WorkflowTestRunPanel';

const ENVIRONMENT_DOCK_WIDTH = 520;
const HISTORY_DOCK_WIDTH = 460;

interface WorkflowCanvasFrameProps {
  applicationId: string;
  applicationName: string;
  initialEnvironmentVariables: ConsoleApplicationEnvironmentVariable[];
  nodeContributions: ConsoleNodeContributionEntry[];
  triggerContext: WorkflowTriggerContext;
  saveDraftOverride?: (
    input: SaveConsoleApplicationDraftInput
  ) => Promise<ConsoleApplicationOrchestrationState>;
  restoreVersionOverride?: (
    versionId: string
  ) => Promise<ConsoleApplicationOrchestrationState>;
}

export function WorkflowCanvasFrame({
  applicationId,
  applicationName,
  initialEnvironmentVariables,
  nodeContributions,
  triggerContext,
  saveDraftOverride,
  restoreVersionOverride
}: WorkflowCanvasFrameProps) {
  const { message } = App.useApp();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const workingDocument = useAgentFlowEditorStore(selectWorkingDocument);
  const lastSavedDocument = useAgentFlowEditorStore(selectLastSavedDocument);
  const autosaveStatus = useAgentFlowEditorStore(selectAutosaveStatus);
  const versions = useAgentFlowEditorStore(selectVersions);
  const userProtectionLimit = useAgentFlowEditorStore(
    selectUserProtectionLimit
  );
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
  documentRef.current = workingDocument;
  lastSavedDocumentRef.current = lastSavedDocument;

  const [environmentVariablesOpen, setEnvironmentVariablesOpen] =
    useState(false);
  const [environmentVariables, setEnvironmentVariables] = useState<
    AgentFlowEnvironmentVariable[]
  >(initialEnvironmentVariables);
  const environmentVariablesSourceRef = useRef(initialEnvironmentVariables);
  if (environmentVariablesSourceRef.current !== initialEnvironmentVariables) {
    environmentVariablesSourceRef.current = initialEnvironmentVariables;
    setEnvironmentVariables(initialEnvironmentVariables);
  }

  const navigation = useContainerNavigation();
  const detailActions = useNodeDetailActions();
  useEditorShortcuts();
  const draftSync = useDraftSync({
    applicationId,
    saveDraftOverride,
    restoreVersionOverride,
    getCurrentDocument: () =>
      getDocumentWithLatestViewport(
        documentRef.current,
        viewportGetterRef.current?.() ?? viewportSnapshotRef.current
      ),
    getLastSavedDocument: () => lastSavedDocumentRef.current
  });
  const issues = useMemo(
    () => validateWorkflowDocument(workingDocument),
    [workingDocument]
  );
  const issueCountByNodeId = useMemo(
    () => countIssuesByNodeId(issues),
    [issues]
  );
  const issueErrorCount = issues.filter(
    (issue) => issue.level === 'error'
  ).length;
  const nodePickerOptions = useMemo(
    () => buildWorkflowNodePickerOptions(nodeContributions),
    [nodeContributions]
  );
  const activeContainerId = activeContainerPath.at(-1) ?? null;
  const boundedNodeDetailWidth = clampNodeDetailWidth(nodeDetailWidth, 1200);

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
    }
  });

  function openHistory() {
    setEnvironmentVariablesOpen(false);
    setPanelState({ historyOpen: true });
  }

  return (
    <section
      aria-label={`${applicationName} workflow editor`}
      className="agent-flow-editor"
      data-application-id={applicationId}
    >
      <WorkflowOverlay
        applicationName={applicationName}
        autosaveLabel={i18nText(
          'agentFlow',
          'auto.automatically_save_seconds',
          { value1: Math.round(autosaveIntervalMs / 1000) }
        )}
        autosaveStatus={autosaveStatus}
        issueErrorCount={issueErrorCount}
        saveDisabled={autosaveStatus === 'saving'}
        saveLoading={autosaveStatus === 'saving'}
        testRunAction={
          <WorkflowTestRunPanel
            applicationId={applicationId}
            document={workingDocument}
            triggerContext={triggerContext}
            onOpenTrace={(runId) => {
              void navigate({
                to: '/applications/$applicationId/logs',
                params: { applicationId },
                search: { run_id: runId, view: 'trace' }
              });
            }}
          />
        }
        onOpenEnvironmentVariables={() => {
          setPanelState({ historyOpen: false });
          setEnvironmentVariablesOpen(true);
        }}
        onOpenHistory={openHistory}
        onOpenIssues={() => setPanelState({ issuesOpen: true })}
        onSaveDraft={() => {
          void draftSync.saveNow();
        }}
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
        className="agent-flow-editor__body agent-flow-editor__shell"
        data-testid="workflow-editor-body"
      >
        <AgentFlowCanvas
          issueCountByNodeId={issueCountByNodeId}
          nodePickerOptions={nodePickerOptions}
          onViewportSnapshotChange={(viewport) => {
            viewportSnapshotRef.current = viewport;
          }}
          onViewportGetterReady={(getter) => {
            viewportGetterRef.current = getter;
          }}
        />
        {selectedNodeId ? (
          <div style={{ width: boundedNodeDetailWidth }}>
            <WorkflowNodeDetailPanel
              environmentVariables={environmentVariables}
              issues={issues}
              triggerContext={triggerContext}
              onClose={detailActions.closeDetail}
            />
          </div>
        ) : null}
        {environmentVariablesOpen ? (
          <AgentFlowSideDock
            className="agent-flow-editor__variables-dock"
            resizeLabel="Adjust environment variables width"
            width={ENVIRONMENT_DOCK_WIDTH}
            onResizeStart={() => undefined}
          >
            <ApplicationEnvironmentVariablesPanel
              loading={environmentVariablesMutation.isPending}
              variables={environmentVariables}
              onClose={() => setEnvironmentVariablesOpen(false)}
              onSave={(variables) =>
                environmentVariablesMutation.mutate(variables)
              }
            />
          </AgentFlowSideDock>
        ) : null}
        {historyOpen ? (
          <AgentFlowSideDock
            className="agent-flow-editor__history-dock"
            resizeLabel={i18nText(
              'agentFlow',
              'auto.adjust_historical_version_width'
            )}
            width={HISTORY_DOCK_WIDTH}
            onResizeStart={() => undefined}
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
      <IssuesDrawer
        open={issuesOpen}
        issues={issues}
        onClose={() => setPanelState({ issuesOpen: false })}
        onSelectIssue={navigation.jumpToIssue}
      />
    </section>
  );
}
