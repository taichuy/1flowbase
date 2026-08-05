import type {
  FlowAuthoringDocument,
  FlowNodeDocument
} from '@1flowbase/flow-schema';
import {
  cancelConsoleFlowRun,
  deleteConsoleDebugVariableCacheEntries,
  getConsoleApplicationRunDebugSnapshot,
  getConsoleApplicationRunNodeLastRun,
  getConsoleDebugVariableSnapshot,
  getConsoleRuntimeDebugArtifact,
  resolveConsoleRuntimeDebugArtifacts,
  startConsoleFlowDebugRun,
  startConsoleFlowDebugRunStream,
  getConsoleNodeLastRun,
  startConsoleNodeDebugPreview,
  upsertConsoleDebugVariableCacheEntry,
  type ConsoleAnswerSnapshot,
  type ConsoleApplicationRunDetail,
  type ConsoleDebugVariableSnapshot,
  type ConsoleFlowDebugStreamEvent,
  type ConsoleFlowDebugStreamCursor,
  type ConsoleFlowDebugStreamHandlers,
  type ConsoleNodeLastRun,
  type ConsoleRuntimeDebugArtifactPreview
} from '@1flowbase/api-client';

import { getApplicationsApiBaseUrl } from '../../applications/api/applications';
import type { NodeDebugPreviewVariableCache } from './runtime-preview';

export type NodeLastRun = ConsoleNodeLastRun;
export type FlowDebugRunDetail = ConsoleApplicationRunDetail;
export type RuntimeDebugArtifactPreview = ConsoleRuntimeDebugArtifactPreview;
export type DebugVariableSnapshot = ConsoleDebugVariableSnapshot & {
  variable_cache: NodeDebugPreviewVariableCache;
};
export type FlowDebugRunStreamEvent = ConsoleFlowDebugStreamEvent;
export type FlowDebugRunStreamHandlers = ConsoleFlowDebugStreamHandlers;
export {
  buildFlowDebugRunInput,
  buildNodeDebugPreviewInput,
  buildNodeDebugPreviewPlan,
  buildNodeDebugVariableConfirmationPlan,
  extractNodePreviewVariableOutput
} from './runtime-preview';
export type {
  NodeDebugPreviewPlan,
  NodeDebugPreviewVariableCache,
  NodeDebugPreviewVariableField,
  NodeDebugVariableConfirmationPlan
} from './runtime-preview';

export type AgentFlowDebugMessageStatus =
  | 'running'
  | 'completed'
  | 'waiting_callback'
  | 'waiting_human'
  | 'cancelled'
  | 'failed';

export interface AgentFlowAnswerSnapshot {
  kind: ConsoleAnswerSnapshot['kind'];
  text: string;
  outputPayload: Record<string, unknown>;
  complete: boolean;
  materializedFrom: string;
  answerNodeId: string | null;
  answerNodeRunId: string | null;
  waitingNodeId?: string | null;
  waitingNodeRunId?: string | null;
}

export interface AgentFlowTraceItem {
  nodeId: string;
  nodeRunId?: string;
  nodeAlias: string;
  nodeType: string;
  status: string;
  startedAt: string;
  finishedAt: string | null;
  durationMs: number | null;
  inputPayload: Record<string, unknown>;
  outputPayload: Record<string, unknown>;
  errorPayload: Record<string, unknown> | null;
  metricsPayload: Record<string, unknown>;
  debugPayload?: Record<string, unknown>;
  answerSnapshot?: AgentFlowAnswerSnapshot;
}

export interface AgentFlowRunStatistics {
  total_tokens: number | null;
  unique_node_count: number;
  tool_callback_count: number;
}

export interface AgentFlowVariableItem {
  key: string;
  label: string;
  value: unknown;
  isReadOnly?: boolean;
  isTruncated?: boolean;
  artifactRef?: string;
  helperText?: string;
}

export interface AgentFlowVariableGroup {
  title: string;
  items: AgentFlowVariableItem[];
}

export interface AgentFlowDebugMessage {
  id: string;
  role: 'system' | 'user' | 'assistant';
  content: string;
  status: AgentFlowDebugMessageStatus;
  runId: string | null;
  detailRunId?: string | null;
  canOpenDetail?: boolean;
  compatibilityMode?: string | null;
  compatibilityModeLabel?: string | null;
  rawOutput: Record<string, unknown> | null;
  statistics?: AgentFlowRunStatistics;
  traceSummary: AgentFlowTraceItem[];
}

export interface AgentFlowRunContextField {
  nodeId: string;
  nodeLabel: string;
  key: string;
  title: string;
  valueType: FlowNodeDocument['outputs'][number]['valueType'];
  value: unknown;
}

export interface AgentFlowRunContext {
  environmentLabel: 'draft' | 'published';
  remembered: boolean;
  fields: AgentFlowRunContextField[];
}

export const nodeLastRunQueryKey = (applicationId: string, nodeId: string) =>
  [
    'applications',
    applicationId,
    'runtime',
    'nodes',
    nodeId,
    'last-run'
  ] as const;

export const applicationRunNodeLastRunQueryKey = (
  applicationId: string,
  runId: string,
  nodeId: string
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'runs',
    runId,
    'nodes',
    nodeId,
    'last-run'
  ] as const;

export function fetchNodeLastRun(applicationId: string, nodeId: string) {
  return getConsoleNodeLastRun(
    applicationId,
    nodeId,
    getApplicationsApiBaseUrl()
  );
}

export function fetchDebugVariableSnapshot(applicationId: string) {
  return getConsoleDebugVariableSnapshot(
    applicationId,
    getApplicationsApiBaseUrl()
  );
}

export function upsertDebugVariableCacheEntry(
  applicationId: string,
  input: {
    node_id: string;
    variable_key: string;
    value: unknown;
  },
  csrfToken: string
) {
  return upsertConsoleDebugVariableCacheEntry(
    applicationId,
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function deleteDebugVariableCacheEntries(
  applicationId: string,
  input: {
    keys?: Array<{
      node_id: string;
      variable_key: string;
    }>;
  },
  csrfToken: string
) {
  return deleteConsoleDebugVariableCacheEntries(
    applicationId,
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function nodeLastRunToFlowDebugRunDetail(
  lastRun: ConsoleNodeLastRun
): FlowDebugRunDetail {
  return {
    flow_run: lastRun.flow_run,
    node_runs: [lastRun.node_run],
    checkpoints: lastRun.checkpoints,
    callback_tasks: [],
    events: lastRun.events
  };
}

export function startNodeDebugPreview(
  applicationId: string,
  nodeId: string,
  input: {
    input_payload: Record<string, Record<string, unknown>>;
    document?: FlowAuthoringDocument;
    debug_session_id?: string;
  },
  csrfToken: string
) {
  return startConsoleNodeDebugPreview(
    applicationId,
    nodeId,
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function startFlowDebugRun(
  applicationId: string,
  input: {
    input_payload: Record<string, Record<string, unknown>>;
    document?: FlowAuthoringDocument;
    debug_session_id?: string;
  },
  csrfToken: string
) {
  return startConsoleFlowDebugRun(
    applicationId,
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function startFlowDebugRunStream(
  applicationId: string,
  input: {
    input_payload: Record<string, Record<string, unknown>>;
    document?: FlowAuthoringDocument;
    debug_session_id?: string;
  },
  csrfToken: string,
  handlers: FlowDebugRunStreamHandlers,
  cursor?: ConsoleFlowDebugStreamCursor
) {
  return startConsoleFlowDebugRunStream(
    applicationId,
    input,
    csrfToken,
    handlers,
    {
      cursor,
      baseUrl: getApplicationsApiBaseUrl()
    }
  );
}

export async function fetchApplicationRunDebugSnapshot(
  applicationId: string,
  runId: string
): Promise<FlowDebugRunDetail> {
  return getConsoleApplicationRunDebugSnapshot(
    applicationId,
    runId,
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationRunNodeLastRun(
  applicationId: string,
  runId: string,
  nodeId: string
) {
  return getConsoleApplicationRunNodeLastRun(
    applicationId,
    runId,
    nodeId,
    getApplicationsApiBaseUrl()
  );
}

export function fetchRuntimeDebugArtifact(
  applicationId: string,
  artifactId: string
) {
  return getConsoleRuntimeDebugArtifact(
    applicationId,
    artifactId,
    getApplicationsApiBaseUrl()
  );
}

export function fetchRuntimeDebugArtifacts(
  applicationId: string,
  artifactRefs: string[]
) {
  return resolveConsoleRuntimeDebugArtifacts(
    applicationId,
    artifactRefs,
    getApplicationsApiBaseUrl()
  );
}

export function cancelFlowDebugRun(
  applicationId: string,
  runId: string,
  csrfToken: string
) {
  return cancelConsoleFlowRun(
    applicationId,
    runId,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}
