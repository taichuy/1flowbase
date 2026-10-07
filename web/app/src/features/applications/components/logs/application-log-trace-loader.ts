import type { AgentFlowDebugMessage } from '../../../agent-flow/api/runtime';
import type { ConversationLogTraceLoader } from '../../../agent-flow/components/debug-console/conversation-log-trace-model';
import {
  fetchApplicationLogRecordClientTrajectory,
  fetchApplicationLogRecordClientTrajectorySection,
  fetchClientTrajectory,
  fetchClientTrajectorySection,
  fetchProviderTrajectoryBody,
  fetchWorkflowTrajectory,
  fetchWorkflowTrajectoryBody
} from '../../api/trajectory';
import {
  fetchApplicationRunTraceNodeChildren,
  fetchApplicationRunTraceNodeContent,
  fetchApplicationRunTraceNodeDetail,
  fetchApplicationRunTraceToolCallbackContent,
  fetchApplicationRunTraceTree,
  fetchRuntimeDebugArtifact,
  fetchRuntimeDebugArtifacts
} from '../../api/runtime';

export function buildApplicationRunTraceMessage(
  runId: string
): AgentFlowDebugMessage {
  return {
    id: `application-log-trace:${runId}`,
    role: 'assistant',
    content: '',
    status: 'completed',
    runId,
    detailRunId: runId,
    canOpenDetail: true,
    rawOutput: null,
    traceSummary: []
  };
}

export function createApplicationLogTraceLoader(
  applicationId: string,
  sourceKind: 'native' | 'imported' = 'native'
): ConversationLogTraceLoader {
  if (sourceKind === 'imported') {
    return {
      sourceKind,
      loadClientTrajectory: (recordId, _nodeRunId, cursor, options) =>
        fetchApplicationLogRecordClientTrajectory(
          applicationId,
          recordId,
          cursor,
          options
        ),
      loadClientTrajectorySection: (
        recordId,
        stepId,
        section,
        _nodeRunId,
        cursor
      ) =>
        fetchApplicationLogRecordClientTrajectorySection(
          applicationId,
          recordId,
          stepId,
          section,
          cursor
        )
    };
  }
  return {
    loadArtifact: (artifactRef) =>
      fetchRuntimeDebugArtifact(applicationId, artifactRef),
    loadArtifacts: (artifactRefs) =>
      fetchRuntimeDebugArtifacts(applicationId, artifactRefs),
    loadWorkflowTrajectory: (runId, cursor, options) =>
      fetchWorkflowTrajectory(applicationId, runId, cursor, options),
    loadWorkflowTrajectoryBody: (runId, eventId) =>
      fetchWorkflowTrajectoryBody(applicationId, runId, eventId),
    loadClientTrajectory: (runId, nodeRunId, cursor, options) =>
      fetchClientTrajectory(applicationId, runId, nodeRunId, cursor, options),
    loadClientTrajectorySection: (runId, stepId, section, nodeRunId, cursor) =>
      fetchClientTrajectorySection(
        applicationId,
        runId,
        stepId,
        section,
        nodeRunId,
        cursor
      ),
    loadTrajectoryBody: (runId, nodeRunId, eventId, cursor, view) =>
      fetchProviderTrajectoryBody(
        applicationId,
        runId,
        nodeRunId,
        eventId,
        cursor,
        view
      ),
    loadTree: (runId) => fetchApplicationRunTraceTree(applicationId, runId),
    loadChildren: (runId, traceNodeId, cursor) =>
      fetchApplicationRunTraceNodeChildren(
        applicationId,
        runId,
        traceNodeId,
        cursor
      ),
    loadContent: (runId, traceNodeId) =>
      fetchApplicationRunTraceNodeContent(applicationId, runId, traceNodeId),
    loadDetail: (runId, traceNodeId, detailRefId, section) =>
      fetchApplicationRunTraceNodeDetail(
        applicationId,
        runId,
        traceNodeId,
        detailRefId,
        section
      ),
    loadToolCallbackDetail: (runId, traceNodeId, toolCallId) =>
      fetchApplicationRunTraceToolCallbackContent(
        applicationId,
        runId,
        traceNodeId,
        toolCallId
      )
  };
}
