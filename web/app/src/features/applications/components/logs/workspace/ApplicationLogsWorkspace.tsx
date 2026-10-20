import {
  ARCHIVE_IMPORT_TARGET_KEY,
  EMPTY_APPLICATIONS,
  writeApplicationLogsSearchState
} from './application-logs-controls';
import { useApplicationLogsController } from './useApplicationLogsController';

import {
  getRunDetailInitialRect,
  getConversationLogInitialRect,
  getResumeTimelineInitialRect
} from './floating-window-layout';

import type { ConsoleApplicationType } from '@1flowbase/api-client';
import { fetchRunPayload } from '../../../api/trajectory';

import { Modal, Select } from 'antd';

import { ConversationLogPanel } from '../../../../agent-flow/components/debug-console/ConversationLogPanel';
import {
  fetchApplicationRunOverview,
  fetchRuntimeDebugArtifact,
  fetchRuntimeDebugArtifacts
} from '../../../api/runtime';
import { ApplicationRunDetailPanel } from '../ApplicationRunDetailPanel';
import { buildApplicationRunTraceMessage } from '../application-log-trace-loader';
import { ApplicationLogsFloatingWindow } from '../ApplicationLogsFloatingWindow';
import { ApplicationRunResumeTimelinePanel } from '../ApplicationRunResumeTimelinePanel';

import '../../../pages/application-logs-page.css';

export function ApplicationLogsWorkspace({
  applicationId,
  applicationIds,
  applications = EMPTY_APPLICATIONS,
  overlayContainer,
  applicationType = 'agent_flow'
}: {
  applicationId: string;
  applicationIds?: string[];
  applications?: Array<{ id: string; name: string }>;
  overlayContainer?: HTMLElement;
  applicationType?: ConsoleApplicationType;
}) {
  const {
    selectedApplicationId,
    archiveApplicationId,
    setArchiveApplicationId,
    importTargetOpen,
    setImportTargetOpen,
    isWorkflow,
    executionTab,
    setExecutionTab,
    t,
    selectedRunId,
    openConversationLogMessage,
    setOpenConversationLogMessage,
    traceViewRequested,
    setTraceViewRequested,
    openResumeTimelineRunId,
    setOpenResumeTimelineRunId,
    runDetailRect,
    conversationLogRect,
    setConversationLogRect,
    resumeTimelineRect,
    setResumeTimelineRect,
    exportingRunId,
    activeFloatingWindow,
    setActiveFloatingWindow,
    archiveImportInputRef,
    message,
    runs,
    selectRun,
    handleRectChange,
    exportRunTraceDump,
    changeLogTab,
    openConversationLog,
    openResumeTimeline,
    archiveImportStatus,
    isRecordSource,
    traceLoader,
    overviewLoader,
    logsHeader,
    logsList,
    renderOverlays
  } = useApplicationLogsController({
    applicationId,
    applicationIds,
    applications,
    overlayContainer,
    applicationType
  });

  return (
    <div className="application-logs-page" data-testid="application-logs-page">
      <Modal
        title={t('auto.import_run_archive')}
        open={importTargetOpen}
        onCancel={() => setImportTargetOpen(false)}
        okButtonProps={{ disabled: !archiveApplicationId }}
        onOk={() => {
          setImportTargetOpen(false);
          archiveImportInputRef.current?.click();
        }}
      >
        <Select
          aria-label={t('logs.import_target_application')}
          placeholder={t('logs.import_target_application')}
          style={{ width: '100%' }}
          value={archiveApplicationId || undefined}
          options={applications.map((app) => ({
            value: app.id,
            label: app.name
          }))}
          onChange={(targetApplicationId) => {
            setArchiveApplicationId(targetApplicationId);
            window.localStorage.setItem(
              ARCHIVE_IMPORT_TARGET_KEY,
              targetApplicationId
            );
          }}
        />
      </Modal>
      <div className="application-logs-page__stack">
        {logsHeader}
        {archiveImportStatus}
        {logsList}
      </div>
      {renderOverlays(
        <>
          {!isWorkflow && openConversationLogMessage ? (
            <ApplicationLogsFloatingWindow
              active={activeFloatingWindow === 'conversation-log'}
              initialRect={getConversationLogInitialRect}
              rect={conversationLogRect ?? undefined}
              onRectChange={(rect) =>
                handleRectChange('conversation-log', rect)
              }
              testId="application-logs-floating-conversation-log"
              title={t('auto.conversation_logs')}
              onActivate={() => setActiveFloatingWindow('conversation-log')}
            >
              <div className="application-logs-page__conversation-log-panel">
                <ConversationLogPanel
                  activeTab={traceViewRequested ? 'trace' : 'detail'}
                  onTabChange={changeLogTab}
                  defaultTraceToolsExpanded
                  message={openConversationLogMessage}
                  onClose={() => {
                    writeApplicationLogsSearchState({
                      runId: selectedRunId,
                      view: null
                    });
                    setTraceViewRequested(false);
                    setOpenConversationLogMessage(null);
                    setConversationLogRect(null);
                    setActiveFloatingWindow('run-detail');
                  }}
                  onLoadArtifact={(artifactRef) =>
                    fetchRuntimeDebugArtifact(
                      applicationId || selectedApplicationId,
                      artifactRef
                    )
                  }
                  onLoadArtifacts={(artifactRefs) =>
                    fetchRuntimeDebugArtifacts(
                      applicationId || selectedApplicationId,
                      artifactRefs
                    )
                  }
                  traceLoader={traceLoader}
                  overviewLoader={overviewLoader}
                  exportingRun={
                    exportingRunId ===
                    (openConversationLogMessage.detailRunId ??
                      openConversationLogMessage.runId)
                  }
                  onExportRun={
                    isRecordSource
                      ? undefined
                      : (runId) => {
                          void exportRunTraceDump(runId);
                        }
                  }
                />
              </div>
            </ApplicationLogsFloatingWindow>
          ) : null}
          {openResumeTimelineRunId ? (
            <ApplicationLogsFloatingWindow
              active={activeFloatingWindow === 'resume-timeline'}
              initialRect={getResumeTimelineInitialRect}
              rect={resumeTimelineRect ?? undefined}
              onRectChange={(rect) => handleRectChange('resume-timeline', rect)}
              testId="application-logs-floating-resume-timeline"
              title={t('auto.resume_timeline')}
              onActivate={() => setActiveFloatingWindow('resume-timeline')}
            >
              <div className="application-logs-page__resume-timeline-panel">
                <ApplicationRunResumeTimelinePanel
                  applicationId={applicationId || selectedApplicationId}
                  runId={openResumeTimelineRunId}
                  onClose={() => {
                    setOpenResumeTimelineRunId(null);
                    setResumeTimelineRect(null);
                  }}
                />
              </div>
            </ApplicationLogsFloatingWindow>
          ) : null}
          {selectedRunId ? (
            <ApplicationLogsFloatingWindow
              active={activeFloatingWindow === 'run-detail'}
              initialRect={getRunDetailInitialRect}
              rect={runDetailRect ?? undefined}
              onRectChange={(rect) => handleRectChange('run-detail', rect)}
              testId="application-logs-floating-run-detail"
              title={t('auto.run_details')}
              onActivate={() => setActiveFloatingWindow('run-detail')}
            >
              {isWorkflow ? (
                <div className="application-logs-page__conversation-log-panel">
                  <ConversationLogPanel
                    key={selectedRunId}
                    title={t('auto.run_details')}
                    closeLabel={t('auto.close_run_details')}
                    traceLabel={t('auto.node_execution')}
                    activeTab={executionTab}
                    onTabChange={(tab) => {
                      setExecutionTab(tab);
                      changeLogTab(tab);
                    }}
                    defaultTraceToolsExpanded
                    message={buildApplicationRunTraceMessage(selectedRunId)}
                    onClose={() => selectRun(null)}
                    traceLoader={traceLoader}
                    overviewLoader={{
                      loadPayload: (runId, section) =>
                        fetchRunPayload(
                          applicationId || selectedApplicationId,
                          runId,
                          section
                        ),
                      loadOverview: (runId) =>
                        fetchApplicationRunOverview(
                          applicationId || selectedApplicationId,
                          runId
                        )
                    }}
                    onLoadArtifact={(artifactRef) =>
                      fetchRuntimeDebugArtifact(
                        applicationId || selectedApplicationId,
                        artifactRef
                      )
                    }
                    onLoadArtifacts={(artifactRefs) =>
                      fetchRuntimeDebugArtifacts(
                        applicationId || selectedApplicationId,
                        artifactRefs
                      )
                    }
                    exportingRun={exportingRunId === selectedRunId}
                    onExportRun={(runId) => {
                      void exportRunTraceDump(runId);
                    }}
                  />
                </div>
              ) : (
                <ApplicationRunDetailPanel
                  traceLoader={traceLoader}
                  recordId={isRecordSource ? selectedRunId : undefined}
                  applicationId={applicationId || selectedApplicationId}
                  requested_model_id={
                    runs.find((run) => run.id === selectedRunId)
                      ?.requested_model_id
                  }
                  reasoning_effort={
                    runs.find((run) => run.id === selectedRunId)
                      ?.reasoning_effort
                  }
                  onClose={() => selectRun(null)}
                  onOpenMessageLog={openConversationLog}
                  onOpenRunTrace={() => {
                    if (!selectedRunId) return;
                    openConversationLog(
                      buildApplicationRunTraceMessage(selectedRunId)
                    );
                    changeLogTab('trace');
                  }}
                  onOpenResumeTimeline={openResumeTimeline}
                  runId={selectedRunId}
                />
              )}
            </ApplicationLogsFloatingWindow>
          ) : null}
        </>
      )}
    </div>
  );
}
