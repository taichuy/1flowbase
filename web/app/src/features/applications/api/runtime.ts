import {
  toApplicationRunSummary,
  toApplicationConversation,
  toApplicationConversationMessage,
  toApplicationRunNodeRun,
  toApplicationRunEvent,
  toApplicationRunCheckpoint,
  toApplicationRunCallbackTask
} from './runtime-records';
import type { StatisticsLogFilters } from '../lib/statistics-log-filters';
import {
  type ApplicationLogRecordSummary,
  queryConsoleApplicationLogConversations,
  completeConsoleCallbackTask,
  completeConsoleRunArchiveUploadSession,
  getConsoleApplicationConversationMessages,
  getConsoleApplicationRunConversationMessages,
  getConsoleApplicationRunMonitoringReport,
  getConsoleApplicationRuntimeActivity,
  getConsoleApplicationRunResumeTimelineSummary,
  getConsoleApplicationRunOverview,
  getConsoleApplicationRunTraceNodeChildren,
  getConsoleApplicationRunTraceNodeContent,
  getConsoleApplicationRunTraceNodeDetail,
  getConsoleApplicationRunTraceToolCallbackContent,
  getConsoleApplicationRunTraceTree,
  createConsoleRunArchiveUploadSession,
  exportConsoleApplicationRunTraceDump,
  exportConsoleApplicationRunsTraceDumpZip,
  fetchConsoleRuntimeModelRecords,
  getConsoleRuntimeDebugArtifact,
  getConsoleRunArchiveImportJob,
  resolveConsoleRuntimeDebugArtifacts,
  getConsoleRuntimeDebugStream,
  uploadConsoleRunArchiveChunk,
  type ApiBlobResponse,
  type ConsoleApplicationConversationContextSource,
  type ConsoleApplicationConversationMessage,
  type ConsoleApplicationConversationMessagesPage,
  type ConsoleApplicationRunConversationOutputState,
  type ConsoleApplicationRunOutputSource,
  type ConsoleApplicationRunMonitoringApiKeyUsage,
  type ConsoleApplicationRunMonitoringAuthorizedAccountUsage,
  type ConsoleApplicationRunMonitoringBucket,
  type ConsoleApplicationRunMonitoringExternalConversationUsage,
  type ConsoleApplicationRunMonitoringProtocolBreakdown,
  type ConsoleApplicationRunMonitoringReport,
  type ConsoleApplicationRunOverview,
  type ConsoleApplicationRuntimeActivity,
  type ConsoleApplicationRunMonitoringRunRank,
  type ConsoleApplicationRunMonitoringSourceBreakdown,
  resumeConsoleFlowRun,
  type ConsoleApplicationRunSummary,
  type ConsoleApplicationRunResumeTimelineSummary,
  type ConsoleApplicationRunTraceNodeChildren,
  type ConsoleApplicationRunTraceNodeContent,
  type ConsoleApplicationRunTraceNodeDetail,
  type ConsoleApplicationRunTraceTree,
  type ConsoleRunArchiveImportJob,
  type ConsoleRunArchiveUploadSession,
  type ConsoleRunArchiveChunkUpload,
  type ConsoleCallbackTask,
  type ConsoleNodeRunDetail,
  type ConsoleRunCheckpoint,
  type ConsoleRunEvent,
  type RuntimeDebugStreamPart
} from '@1flowbase/api-client';

import { getApplicationsApiBaseUrl } from './applications';

export type ApplicationRunSummary = {
  source_kind?: 'native' | 'imported' | null;
  source_client?: string | null;
  source_session_id?: string | null;
  source_task_id?: string | null;
  native_run_id?: string | null;
  log_conversation_id: string | null;
  log_task_run_id?: string | null;
  parent_run_id?: string | null;
  caused_by_run_id?: string | null;
  call_kind?: string;
  invocation_count?: number;
  compaction_count?: number;
  member_run_ids?: string[];
  parent_task_run_id?: string | null;
  outcome: string;
  user_input?: string | null;
  final_output?: string | null;
  id: string;
  application_id: string;
  scope_id?: string;
  run_mode?: ConsoleApplicationRunSummary['run_mode'] | 'imported';
  execution_stage?:
    | ConsoleApplicationRunSummary['execution_stage']
    | 'collected';
  invocation_source?:
    | ConsoleApplicationRunSummary['invocation_source']
    | 'client_collector';
  principal?: ConsoleApplicationRunSummary['principal'];
  status: string;
  target_node_id?: string | null;
  title: string;
  expand_id?: string | null;
  external_user?: string | null;
  authorized_account?: string | null;
  api_key_id?: string | null;
  api_key_name_snapshot?: string | null;
  requested_model_id: string | null;
  reasoning_effort: string | null;
  publication_version_id?: string | null;
  external_conversation_id?: string | null;
  external_trace_id?: string | null;
  compatibility_mode?: string | null;
  idempotency_key?: string | null;
  total_cost: number | null;
  total_tokens: number | null;
  input_tokens: number | null;
  output_tokens: number | null;
  count_tokens_input_tokens?: number | null;
  input_cache_hit_tokens: number | null;
  input_cache_hit_rate?: number | null;
  unique_node_count?: number;
  tool_callback_count?: number;
  started_at: string;
  finished_at: string | null;
  created_at: string;
  updated_at: string;
};
export interface ApplicationRunsPage {
  items: ApplicationRunSummary[];
  total: number;
  page: number;
  page_size: number;
}
export type ApplicationRunTraceTree = ConsoleApplicationRunTraceTree;
export type ApplicationRunOverview = ConsoleApplicationRunOverview;
export type ApplicationRunResumeTimelineSummary =
  ConsoleApplicationRunResumeTimelineSummary;
export type ApplicationRunTraceNodeChildren =
  ConsoleApplicationRunTraceNodeChildren;
export type ApplicationRunTraceNodeContent =
  ConsoleApplicationRunTraceNodeContent;
export type ApplicationRunMonitoringBucket =
  ConsoleApplicationRunMonitoringBucket;
export type ApplicationRunMonitoringReport =
  ConsoleApplicationRunMonitoringReport;
export type ApplicationRuntimeActivity = ConsoleApplicationRuntimeActivity;
export type ApplicationRunMonitoringApiKeyUsage =
  ConsoleApplicationRunMonitoringApiKeyUsage;
export type ApplicationRunMonitoringAuthorizedAccountUsage =
  ConsoleApplicationRunMonitoringAuthorizedAccountUsage;
export type ApplicationRunMonitoringExternalConversationUsage =
  ConsoleApplicationRunMonitoringExternalConversationUsage;
export type ApplicationRunMonitoringProtocolBreakdown =
  ConsoleApplicationRunMonitoringProtocolBreakdown;
export type ApplicationRunMonitoringRunRank =
  ConsoleApplicationRunMonitoringRunRank;
export type ApplicationRunMonitoringSourceBreakdown =
  ConsoleApplicationRunMonitoringSourceBreakdown;
export interface ApplicationConversationRecord {
  id: string;
  application_id: string;
  scope_id: string;
  external_conversation_id: string;
  external_user?: string | null;
  api_key_id?: string | null;
  created_at: string;
  updated_at: string;
}
export interface ApplicationConversationsPage {
  items: ApplicationConversationRecord[];
  total: number;
  page: number;
  page_size: number;
}
export interface ApplicationConversationMessageRecord {
  id: string;
  application_id: string;
  scope_id: string;
  conversation_id?: string | null;
  flow_run_id?: string | null;
  role: 'system' | 'user' | 'assistant';
  content: string;
  sequence: number;
  status: string;
  created_at: string;
  started_at?: string | null;
  finished_at?: string | null;
}
export interface ApplicationConversationMessagesPage {
  items: ApplicationConversationMessageRecord[];
  total: number;
  page: number;
  page_size: number;
}
export type ApplicationRunConversationMessage =
  ConsoleApplicationConversationMessage;
export type ApplicationRunConversationMessagesPage =
  ConsoleApplicationConversationMessagesPage;
export type ApplicationRunConversationContextSource =
  ConsoleApplicationConversationContextSource;
export type ApplicationRunConversationOutputState =
  ConsoleApplicationRunConversationOutputState;
export type ApplicationRunOutputSource = ConsoleApplicationRunOutputSource;
export interface ApplicationRunNodeRunsPage {
  items: ConsoleNodeRunDetail[];
  total: number;
  page: number;
  page_size: number;
}
export interface ApplicationRunEventsPage {
  items: ConsoleRunEvent[];
  total: number;
  page: number;
  page_size: number;
}
export interface ApplicationRunCheckpointsPage {
  items: ConsoleRunCheckpoint[];
  total: number;
  page: number;
  page_size: number;
}
export interface ApplicationRunCallbackTasksPage {
  items: ConsoleCallbackTask[];
  total: number;
  page: number;
  page_size: number;
}
export type ApplicationRuntimeDebugStreamPart = RuntimeDebugStreamPart;
export type { RuntimeDebugStreamPart };
export type ApplicationRunExportDownload = ApiBlobResponse;
export type ApplicationRunArchiveUploadSession = ConsoleRunArchiveUploadSession;
export type ApplicationRunArchiveChunkUpload = ConsoleRunArchiveChunkUpload;
export type ApplicationRunArchiveImportJob = ConsoleRunArchiveImportJob;
export type ApplicationRunSortField =
  | 'created_at'
  | 'started_at'
  | 'finished_at'
  | 'updated_at'
  | 'total_cost'
  | 'total_tokens';
export type ApplicationRunSortOrder = 'asc' | 'desc';
export type ApplicationRunCacheMode = 'default' | 'refresh';

const TRACE_NODE_ARTIFACT_PREVIEW_AUTO_QUERY = {
  artifact_preview: 'auto'
} as const;

export interface FetchApplicationRunMonitoringReportInput {
  from?: string;
  to?: string;
  timeRangeDays?: number | null;
  bucket?: ApplicationRunMonitoringBucket;
}

export interface FetchApplicationRunsInput extends StatisticsLogFilters {
  page?: number;
  pageSize?: number;
  timeRangeDays?: number | null;
  sortBy?: ApplicationRunSortField;
  sortOrder?: ApplicationRunSortOrder;
  cacheMode?: ApplicationRunCacheMode;
  titleIncludes?: string;
}

export const applicationRunsQueryKey = (
  applicationId: string | string[],
  input: FetchApplicationRunsInput = {}
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'runs',
    input.page ?? 1,
    input.pageSize ?? 20,
    input.timeRangeDays ?? 'all',
    input.sortBy ?? 'started_at',
    input.sortOrder ?? 'desc',
    input.titleIncludes ?? '',
    input.started_from,
    input.started_to,
    input.requested_model_id,
    input.user_id,
    input.missing_model,
    input.missing_user
  ] as const;

export const applicationConversationsQueryKey = (
  applicationId: string,
  input: {
    externalConversationId?: string | null;
  } = {}
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'conversations',
    input.externalConversationId ?? ''
  ] as const;

export const applicationRunTraceTreeQueryKey = (
  applicationId: string,
  runId: string
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'runs',
    runId,
    'trace-tree'
  ] as const;

export const applicationRunOverviewQueryKey = (
  applicationId: string,
  runId: string
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'runs',
    runId,
    'overview'
  ] as const;

export const applicationRunTraceNodeChildrenQueryKey = (
  applicationId: string,
  runId: string,
  traceNodeId: string
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'runs',
    runId,
    'trace-tree',
    traceNodeId,
    'children'
  ] as const;

export const applicationRunTraceNodeContentQueryKey = (
  applicationId: string,
  runId: string,
  traceNodeId: string
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'runs',
    runId,
    'trace-tree',
    traceNodeId,
    'content'
  ] as const;

export const applicationRunResumeTimelineQueryKey = (
  applicationId: string,
  runId: string
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'runs',
    runId,
    'resume-timeline'
  ] as const;

export const applicationConversationMessagesQueryKey = (
  applicationId: string,
  input: {
    conversationId?: string | null;
    flowRunId?: string | null;
    page?: number;
    pageSize?: number;
  }
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'conversation-messages',
    input.conversationId ?? '',
    input.flowRunId ?? '',
    input.page ?? 1,
    input.pageSize ?? 5
  ] as const;

export const applicationRunConversationMessagesQueryKey = (
  applicationId: string,
  runId: string,
  input: {
    before?: string | null;
    after?: string | null;
    limit?: number;
  } = {}
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'runs',
    runId,
    'conversation-messages',
    input.before ?? '',
    input.after ?? '',
    input.limit ?? 5
  ] as const;

export const applicationLogConversationMessagesQueryKey = (
  applicationId: string,
  externalConversationId: string,
  input: {
    aroundRunId?: string | null;
    before?: string | null;
    after?: string | null;
    limit?: number;
  } = {}
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'logs',
    'conversations',
    externalConversationId,
    input.aroundRunId ?? '',
    input.before ?? '',
    input.after ?? '',
    input.limit ?? 5
  ] as const;

export const applicationRunTraceFragmentsQueryKey = (
  applicationId: string,
  flowRunId: string
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'runs',
    flowRunId,
    'trace-fragments'
  ] as const;

export const applicationRuntimeDebugStreamQueryKey = (
  applicationId: string,
  runId: string
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'runs',
    runId,
    'debug-stream'
  ] as const;

export const applicationRunMonitoringReportQueryKey = (
  applicationId: string,
  input: FetchApplicationRunMonitoringReportInput = {}
) =>
  [
    'applications',
    applicationId,
    'runtime',
    'monitoring',
    'run-metrics',
    input.timeRangeDays ?? 7,
    input.bucket ?? 'day',
    input.from,
    input.to
  ] as const;

export const applicationRuntimeActivityQueryKey = (applicationId: string) =>
  [
    'applications',
    applicationId,
    'runtime',
    'monitoring',
    'runtime-activity'
  ] as const;

export function fetchApplicationRuns(
  applicationId: string | string[],
  input: FetchApplicationRunsInput = {}
) {
  const page = input.page ?? 1;
  const pageSize = input.pageSize ?? 20;

  return fetchConsoleRuntimeModelRecords(
    'application_run_log_tasks',
    {
      page,
      page_size: pageSize,
      filter: applicationRunLogTaskFilter(applicationId, input),
      sort: {
        field: input.sortBy ?? 'started_at',
        direction: input.sortOrder ?? 'desc'
      }
    },
    getApplicationsApiBaseUrl()
  ).then((recordPage) => ({
    items: recordPage.items.map(toApplicationRunSummary),
    total: recordPage.total,
    page,
    page_size: pageSize
  }));
}

/** Agent Logs consumes canonical backend conversations; native run lists keep their own reader. */
export function fetchApplicationLogConversations(
  applicationId: string | string[],
  input: FetchApplicationRunsInput = {}
) {
  const filter = applicationRunLogTaskFilter(applicationId, input);
  delete filter.is_root;
  return queryConsoleApplicationLogConversations(
    {
      application_ids:
        typeof applicationId === 'string' ? [applicationId] : applicationId,
      filter,
      page: input.page ?? 1,
      page_size: input.pageSize ?? 20,
      sort_field: input.sortBy ?? 'started_at',
      descending: (input.sortOrder ?? 'desc') === 'desc'
    },
    getApplicationsApiBaseUrl()
  ).then((page) => ({
    ...page,
    items: page.items.map(toApplicationLogConversationTableRow)
  }));
}

/** Canonical DTO -> shared table row. Native-only facts remain absent. */
function toApplicationLogConversationTableRow(
  record: ApplicationLogRecordSummary
): ApplicationRunSummary {
  return {
    ...record,
    // The shared table's existing row key locates the actual main task record.
    id: record.record_id,
    total_cost: record.total_cost === null ? null : Number(record.total_cost)
  };
}

export function fetchApplicationRunMonitoringReport(
  applicationId: string,
  input: FetchApplicationRunMonitoringReportInput = {}
) {
  return getConsoleApplicationRunMonitoringReport(
    applicationId,
    {
      time_range_days: input.timeRangeDays ?? 7,
      bucket: input.bucket ?? 'day',
      from: input.from,
      to: input.to
    },
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationRuntimeActivity(applicationId: string) {
  return getConsoleApplicationRuntimeActivity(
    applicationId,
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationRunTraceTree(
  applicationId: string,
  runId: string
) {
  return getConsoleApplicationRunTraceTree(
    applicationId,
    runId,
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationRunOverview(
  applicationId: string,
  runId: string
) {
  return getConsoleApplicationRunOverview(
    applicationId,
    runId,
    getApplicationsApiBaseUrl()
  );
}

export function exportApplicationRunTraceDump(
  applicationId: string,
  runId: string
) {
  return exportConsoleApplicationRunTraceDump(
    applicationId,
    runId,
    getApplicationsApiBaseUrl()
  );
}

export function exportSelectedApplicationRunsTraceDumpZip(
  applicationId: string,
  runIds: string[],
  csrfToken: string
) {
  return exportConsoleApplicationRunsTraceDumpZip(
    applicationId,
    runIds,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function createApplicationRunArchiveUploadSession(
  applicationId: string,
  input: {
    filename?: string | null;
    total_size_bytes: number;
    expected_sha256: string;
    chunk_size_bytes: number;
  },
  csrfToken: string
) {
  return createConsoleRunArchiveUploadSession(
    applicationId,
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function uploadApplicationRunArchiveChunk(
  applicationId: string,
  sessionId: string,
  chunkIndex: number,
  chunk: BodyInit,
  chunkSha256: string,
  csrfToken: string
) {
  return uploadConsoleRunArchiveChunk(
    applicationId,
    sessionId,
    chunkIndex,
    chunk,
    chunkSha256,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function completeApplicationRunArchiveUploadSession(
  applicationId: string,
  sessionId: string,
  csrfToken: string
) {
  return completeConsoleRunArchiveUploadSession(
    applicationId,
    sessionId,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationRunArchiveImportJob(
  applicationId: string,
  jobId: string
) {
  return getConsoleRunArchiveImportJob(
    applicationId,
    jobId,
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationRunTraceNodeChildren(
  applicationId: string,
  runId: string,
  traceNodeId: string,
  cursor?: string
) {
  return getConsoleApplicationRunTraceNodeChildren(
    applicationId,
    runId,
    traceNodeId,
    getApplicationsApiBaseUrl(),
    cursor ? { cursor } : undefined
  );
}

export function fetchApplicationRunTraceNodeContent(
  applicationId: string,
  runId: string,
  traceNodeId: string
) {
  return getConsoleApplicationRunTraceNodeContent(
    applicationId,
    runId,
    traceNodeId,
    getApplicationsApiBaseUrl(),
    TRACE_NODE_ARTIFACT_PREVIEW_AUTO_QUERY
  );
}

export function fetchApplicationRunTraceNodeDetail(
  applicationId: string,
  runId: string,
  traceNodeId: string,
  detailRefId: string,
  section?: 'input_payload' | 'debug_payload' | 'output_payload'
): Promise<ConsoleApplicationRunTraceNodeDetail> {
  return getConsoleApplicationRunTraceNodeDetail(
    applicationId,
    runId,
    traceNodeId,
    detailRefId,
    getApplicationsApiBaseUrl(),
    { ...TRACE_NODE_ARTIFACT_PREVIEW_AUTO_QUERY, section }
  );
}

export async function fetchApplicationRunTraceToolCallbackContent(
  applicationId: string,
  runId: string,
  traceNodeId: string,
  toolCallId: string
) {
  const response = await getConsoleApplicationRunTraceToolCallbackContent(
    applicationId,
    runId,
    traceNodeId,
    toolCallId,
    getApplicationsApiBaseUrl()
  );

  return response.payload;
}

export function fetchApplicationRunResumeTimeline(
  applicationId: string,
  runId: string
) {
  return getConsoleApplicationRunResumeTimelineSummary(
    applicationId,
    runId,
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationConversations(
  applicationId: string,
  input: {
    externalConversationId?: string | null;
    page?: number;
    pageSize?: number;
  } = {}
) {
  const page = input.page ?? 1;
  const pageSize = input.pageSize ?? 1;

  return fetchConsoleRuntimeModelRecords(
    'application_conversations',
    {
      page,
      page_size: pageSize,
      filter: applicationConversationFilter(applicationId, input),
      sort: {
        field: 'updated_at',
        direction: 'desc'
      }
    },
    getApplicationsApiBaseUrl()
  ).then((recordPage) => ({
    items: recordPage.items.map(toApplicationConversation),
    total: recordPage.total,
    page,
    page_size: pageSize
  }));
}

export function fetchApplicationConversationMessages(
  applicationId: string,
  input: {
    conversationId?: string | null;
    flowRunId?: string | null;
    page?: number;
    pageSize?: number;
  } = {}
) {
  const page = input.page ?? 1;
  const pageSize = input.pageSize ?? 5;

  return fetchConsoleRuntimeModelRecords(
    'application_conversation_messages',
    {
      page,
      page_size: pageSize,
      filter: applicationConversationMessageFilter(applicationId, input),
      sort: {
        field: 'sequence',
        direction: 'desc'
      }
    },
    getApplicationsApiBaseUrl()
  ).then((recordPage) => ({
    items: recordPage.items.map(toApplicationConversationMessage),
    total: recordPage.total,
    page,
    page_size: pageSize
  }));
}

export function fetchApplicationRunConversationMessages(
  applicationId: string,
  runId: string,
  input: {
    before?: string | null;
    after?: string | null;
    limit?: number;
  } = {}
) {
  return getConsoleApplicationRunConversationMessages(
    applicationId,
    runId,
    {
      before: input.before ?? undefined,
      after: input.after ?? undefined,
      limit: input.limit
    },
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationLogConversationMessages(
  applicationId: string,
  externalConversationId: string,
  input: {
    aroundRunId?: string | null;
    before?: string | null;
    after?: string | null;
    limit?: number;
  } = {}
) {
  return getConsoleApplicationConversationMessages(
    applicationId,
    externalConversationId,
    {
      around_run_id: input.aroundRunId ?? undefined,
      before: input.before ?? undefined,
      after: input.after ?? undefined,
      limit: input.limit
    },
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationRunNodeRuns(
  applicationId: string,
  flowRunId: string,
  input: {
    page?: number;
    page_size?: number;
  } = {}
) {
  return fetchApplicationRunFragmentRecords<ConsoleNodeRunDetail>(
    'node_runs',
    applicationId,
    flowRunId,
    input,
    { field: 'started_at', direction: 'asc' },
    toApplicationRunNodeRun
  );
}

export function fetchApplicationRunEvents(
  applicationId: string,
  flowRunId: string,
  input: {
    page?: number;
    page_size?: number;
  } = {}
) {
  return fetchApplicationRunFragmentRecords<ConsoleRunEvent>(
    'flow_run_events',
    applicationId,
    flowRunId,
    input,
    { field: 'sequence', direction: 'asc' },
    toApplicationRunEvent
  );
}

export function fetchApplicationRunCheckpoints(
  applicationId: string,
  flowRunId: string,
  input: {
    page?: number;
    page_size?: number;
  } = {}
) {
  return fetchApplicationRunFragmentRecords<ConsoleRunCheckpoint>(
    'flow_run_checkpoints',
    applicationId,
    flowRunId,
    input,
    { field: 'created_at', direction: 'asc' },
    toApplicationRunCheckpoint
  );
}

export function fetchApplicationRunCallbackTasks(
  applicationId: string,
  flowRunId: string,
  input: {
    page?: number;
    page_size?: number;
  } = {}
) {
  return fetchApplicationRunFragmentRecords<ConsoleCallbackTask>(
    'flow_run_callback_tasks',
    applicationId,
    flowRunId,
    input,
    { field: 'created_at', direction: 'asc' },
    toApplicationRunCallbackTask
  );
}

export function fetchRuntimeDebugStream(applicationId: string, runId: string) {
  return getConsoleRuntimeDebugStream(
    applicationId,
    runId,
    getApplicationsApiBaseUrl()
  );
}

export { getConsoleRuntimeDebugStream };

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

export function resumeFlowRun(
  applicationId: string,
  runId: string,
  checkpointId: string,
  inputPayload: Record<string, unknown>,
  csrfToken: string
) {
  return resumeConsoleFlowRun(
    applicationId,
    runId,
    {
      checkpoint_id: checkpointId,
      input_payload: inputPayload
    },
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function completeCallbackTask(
  applicationId: string,
  callbackTaskId: string,
  responsePayload: Record<string, unknown>,
  csrfToken: string
) {
  return completeConsoleCallbackTask(
    applicationId,
    callbackTaskId,
    {
      response_payload: responsePayload
    },
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

function applicationRunLogTaskFilter(
  applicationId: string | string[],
  input: FetchApplicationRunsInput
) {
  // Child tasks (subagent threads) are shown inside their parent's trace tree.
  const filter: Record<string, unknown> = {
    application_id:
      typeof applicationId === 'string'
        ? { $eq: applicationId }
        : { $in: applicationId },
    is_root: { $eq: true }
  };
  const titleIncludes = input.titleIncludes?.trim();

  if (titleIncludes) {
    filter.title = { $includes: titleIncludes };
  }

  if (input.timeRangeDays !== undefined && input.timeRangeDays !== null) {
    filter.started_at = {
      $gte: new Date(
        Date.now() - input.timeRangeDays * 24 * 60 * 60 * 1000
      ).toISOString()
    };
  }

  if (input.started_from || input.started_to) {
    filter.started_at = {
      ...(input.started_from ? { $gte: input.started_from } : {}),
      ...(input.started_to ? { $lt: input.started_to } : {})
    };
  }
  if (input.missing_model) filter.requested_model_id = { $eq: null };
  else if (input.requested_model_id !== undefined)
    filter.requested_model_id = { $eq: input.requested_model_id };
  // The report user dimension is the root task creator, not the external user.
  if (input.missing_user) filter.created_by = { $eq: null };
  else if (input.user_id !== undefined)
    filter.created_by = { $eq: input.user_id };
  return filter;
}

function applicationConversationFilter(
  applicationId: string,
  input: {
    externalConversationId?: string | null;
  }
) {
  const filter: Record<string, unknown> = {
    application_id: { $eq: applicationId }
  };

  if (input.externalConversationId) {
    filter.external_conversation_id = { $eq: input.externalConversationId };
  }

  return filter;
}

function applicationConversationMessageFilter(
  applicationId: string,
  input: {
    conversationId?: string | null;
    flowRunId?: string | null;
  }
) {
  const filter: Record<string, unknown> = {
    application_id: { $eq: applicationId }
  };

  if (input.conversationId) {
    filter.conversation_id = { $eq: input.conversationId };
  }

  if (input.flowRunId) {
    filter.flow_run_id = { $eq: input.flowRunId };
  }

  return filter;
}

function fetchApplicationRunFragmentRecords<T>(
  modelCode: string,
  applicationId: string,
  flowRunId: string,
  input: {
    page?: number;
    page_size?: number;
  },
  sort: {
    field: string;
    direction: string;
  },
  toFragmentRecord: (record: Record<string, unknown>) => T
) {
  const page = input.page ?? 1;
  const pageSize = input.page_size ?? 50;

  return fetchConsoleRuntimeModelRecords(
    modelCode,
    {
      page,
      page_size: pageSize,
      filter: {
        application_id: { $eq: applicationId },
        flow_run_id: { $eq: flowRunId }
      },
      sort
    },
    getApplicationsApiBaseUrl()
  ).then((recordPage) => ({
    items: recordPage.items.map(toFragmentRecord),
    total: recordPage.total,
    page,
    page_size: pageSize
  }));
}
