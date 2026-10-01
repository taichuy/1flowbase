import { useMemo, useState } from 'react';
import { createPortal } from 'react-dom';
import { ConfigProvider } from 'antd';

import type { AgentFlowDebugMessage } from '../../../agent-flow/api/runtime';
import { ConversationLogPanel } from '../../../agent-flow/components/debug-console/ConversationLogPanel';
import { fetchRunPayload } from '../../api/trajectory';
import {
  fetchApplicationRunOverview,
  fetchRuntimeDebugArtifact,
  fetchRuntimeDebugArtifacts
} from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';
import { useNativeBlockSurface } from '../../../frontstage/lib/native-modules/native-block-surface-context';
import { ApplicationLogsFloatingWindow } from './ApplicationLogsFloatingWindow';
import { ApplicationRunDetailPanel } from './ApplicationRunDetailPanel';
import { ApplicationRunResumeTimelinePanel } from './ApplicationRunResumeTimelinePanel';
import {
  buildApplicationRunTraceMessage,
  createApplicationLogTraceLoader
} from './application-log-trace-loader';
import '../../pages/application-logs-page.css';
import windowWorkspaceStyles from '../../../../shared/ui/window-workspace/window-workspace.css?inline';
import editorShellStyles from '../../../agent-flow/components/editor/styles/shell.css?inline';
import dockPanelStyles from '../../../agent-flow/components/editor/styles/dock-panel.css?inline';
import conversationLogStyles from '../../../agent-flow/components/debug-console/conversation-log-panel.css?inline';
import debugMessageStyles from '../../../agent-flow/components/debug-console/conversation/debug-message.css?inline';
import providerTrajectoryStyles from '../../../agent-flow/components/debug-console/trajectory/provider-trajectory.css?inline';
import workflowTrajectoryStyles from '../../../agent-flow/components/debug-console/trajectory/workflow/workflow-trajectory.css?inline';
import clientTrajectoryStyles from '../../../agent-flow/components/debug-console/trajectory/client/client-trajectory.css?inline';
import runDetailStyles from './application-run-detail-panel.css?inline';
import applicationLogsStyles from '../../pages/application-logs-page.css?inline';

const shadowViewerStyles = [
  windowWorkspaceStyles,
  editorShellStyles,
  dockPanelStyles,
  conversationLogStyles,
  debugMessageStyles,
  providerTrajectoryStyles,
  workflowTrajectoryStyles,
  clientTrajectoryStyles,
  runDetailStyles,
  applicationLogsStyles
].join('\n');

export type ApplicationRunLogViewerProps = {
  applicationId: string;
  runId: string;
  view: 'detail' | 'trace';
  log_conversation_id?: string | null;
  requested_model_id?: string | null;
  reasoning_effort?: string | null;
  onClose: () => void;
};

function initialRect() {
  const width = Math.min(600, Math.max(360, window.innerWidth - 48));
  return {
    left: Math.max(16, window.innerWidth - width - 32),
    top: Math.min(112, Math.max(16, window.innerHeight - 340)),
    width,
    height: Math.max(320, Math.min(720, window.innerHeight - 144))
  };
}

/** The same run-detail, conversation, trace and timeline views used by application logs. */
export function ApplicationRunLogViewer({
  applicationId,
  runId,
  view,
  log_conversation_id,
  requested_model_id,
  reasoning_effort,
  onClose
}: ApplicationRunLogViewerProps) {
  const blockSurface = useNativeBlockSurface();
  const [panel, setPanel] = useState<'detail' | 'trace' | 'timeline'>(view);
  const [traceTab, setTraceTab] = useState<'detail' | 'trace'>('trace');
  const [traceMessage, setTraceMessage] = useState<AgentFlowDebugMessage>(() =>
    buildApplicationRunTraceMessage(runId)
  );
  const [timelineRunId, setTimelineRunId] = useState(runId);
  const traceLoader = useMemo(
    () => createApplicationLogTraceLoader(applicationId),
    [applicationId]
  );

  return createPortal(
    <>
      {blockSurface ? (
        <style data-application-run-log-viewer-styles>
          {shadowViewerStyles}
        </style>
      ) : null}
      <ConfigProvider prefixCls="ant">
        <ApplicationLogsFloatingWindow
          active
          initialRect={initialRect}
          testId="application-run-log-viewer"
          title={i18nText(
            'applications',
            panel === 'timeline' ? 'auto.resume_timeline' : 'auto.run_details'
          )}
          onActivate={() => undefined}
        >
          {panel === 'detail' ? (
            <ApplicationRunDetailPanel
              applicationId={applicationId}
              runId={runId}
              logConversationId={log_conversation_id}
              requested_model_id={requested_model_id}
              reasoning_effort={reasoning_effort}
              traceLoader={traceLoader}
              onClose={onClose}
              onOpenMessageLog={(message) => {
                setTraceMessage(message);
                setTraceTab('detail');
                setPanel('trace');
              }}
              onOpenResumeTimeline={(message) => {
                setTimelineRunId(message.detailRunId ?? message.runId ?? runId);
                setPanel('timeline');
              }}
            />
          ) : panel === 'timeline' ? (
            <div className="application-logs-page__resume-timeline-panel">
              <ApplicationRunResumeTimelinePanel
                applicationId={applicationId}
                runId={timelineRunId}
                onClose={onClose}
              />
            </div>
          ) : (
            <div className="application-logs-page__conversation-log-panel">
              <ConversationLogPanel
                activeTab={traceTab}
                onTabChange={setTraceTab}
                defaultTraceToolsExpanded
                message={traceMessage}
                onClose={onClose}
                onLoadArtifact={(artifactRef) =>
                  fetchRuntimeDebugArtifact(applicationId, artifactRef)
                }
                onLoadArtifacts={(artifactRefs) =>
                  fetchRuntimeDebugArtifacts(applicationId, artifactRefs)
                }
                traceLoader={traceLoader}
                overviewLoader={{
                  loadPayload: (targetRunId, section) =>
                    fetchRunPayload(applicationId, targetRunId, section),
                  loadOverview: (targetRunId) =>
                    fetchApplicationRunOverview(applicationId, targetRunId)
                }}
              />
            </div>
          )}
        </ApplicationLogsFloatingWindow>
      </ConfigProvider>
    </>,
    blockSurface?.overlayHost.container ?? document.body
  );
}
