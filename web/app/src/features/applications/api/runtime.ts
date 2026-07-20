import {
  completeConsoleCallbackTask,
  completeConsoleRunArchiveUploadSession,
  getConsoleApplicationConversationMessages,
  getConsoleApplicationRunConversationMessages,
  getConsoleApplicationRunMonitoringReport,
  getConsoleApplicationRuntimeActivity,
  getConsoleApplicationRunResumeTimeline,
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
  type ConsoleApplicationConversationMessage,
  type ConsoleApplicationConversationMessagesPage,
  type ConsoleApplicationRunMonitoringApiKeyUsage,
  type ConsoleApplicationRunMonitoringAuthorizedAccountUsage,
  type ConsoleApplicationRunMonitoringBucket,
  type ConsoleApplicationRunMonitoringExternalConversationUsage,
  type ConsoleApplicationRunMonitoringExternalUserUsage,
  type ConsoleApplicationRunMonitoringProtocolBreakdown,
  type ConsoleApplicationRunMonitoringReport,
  type ConsoleApplicationRunOverview,
  type ConsoleApplicationRuntimeActivity,
  type ConsoleApplicationRunMonitoringRunRank,
  type ConsoleApplicationRunMonitoringSourceBreakdown,
  resumeConsoleFlowRun,
  type ConsoleApplicationRunSummary,
  type ConsoleApplicationRunResumeTimeline,
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
  id: string;
  application_id: string;
  scope_id: string;
  run_mode: ConsoleApplicationRunSummary['run_mode'];
  execution_stage: ConsoleApplicationRunSummary['execution_stage'];
  invocation_source: ConsoleApplicationRunSummary['invocation_source'];
  principal: ConsoleApplicationRunSummary['principal'];
  status: string;
  target_node_id: string | null;
  title: string;
  expand_id?: string | null;
  external_user?: string | null;
  authorized_account?: string | null;
  api_key_id?: string | null;
  api_key_name_snapshot?: string | null;
  publication_version_id?: string | null;
  external_conversation_id?: string | null;
  external_trace_id?: string | null;
  compatibility_mode?: string | null;
  idempotency_key?: string | null;
  total_tokens: number | null;
  input_tokens: number | null;
  output_tokens: number | null;
  input_cache_hit_tokens: number | null;
  input_cache_hit_rate: number | null;
  unique_node_count: number;
  tool_callback_count: number;
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
export type ApplicationRunResumeTimeline = ConsoleApplicationRunResumeTimeline;
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
export type ApplicationRunMonitoringExternalUserUsage =
  ConsoleApplicationRunMonitoringExternalUserUsage;
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
  | 'updated_at';
export type ApplicationRunSortOrder = 'asc' | 'desc';
export type ApplicationRunCacheMode = 'default' | 'refresh';

const TRACE_NODE_ARTIFACT_PREVIEW_AUTO_QUERY = {
  artifact_preview: 'auto'
} as const;

export interface FetchApplicationRunMonitoringReportInput {
  timeRangeDays?: number | null;
  bucket?: ApplicationRunMonitoringBucket;
}

export interface FetchApplicationRunsInput {
  page?: number;
  pageSize?: number;
  timeRangeDays?: number | null;
  sortBy?: ApplicationRunSortField;
  sortOrder?: ApplicationRunSortOrder;
  cacheMode?: ApplicationRunCacheMode;
  titleIncludes?: string;
}

export const applicationRunsQueryKey = (
  applicationId: string,
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
    input.titleIncludes ?? ''
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
    input.bucket ?? 'day'
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
  applicationId: string,
  input: FetchApplicationRunsInput = {}
) {
  const page = input.page ?? 1;
  const pageSize = input.pageSize ?? 20;

  return fetchConsoleRuntimeModelRecords(
    'application_run_log_summaries',
    {
      page,
      page_size: pageSize,
      filter: applicationRunLogSummaryFilter(applicationId, input),
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

export function fetchApplicationRunMonitoringReport(
  applicationId: string,
  input: FetchApplicationRunMonitoringReportInput = {}
) {
  return getConsoleApplicationRunMonitoringReport(
    applicationId,
    {
      time_range_days: input.timeRangeDays ?? 7,
      bucket: input.bucket ?? 'day'
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
  detailRefId: string
): Promise<ConsoleApplicationRunTraceNodeDetail> {
  return getConsoleApplicationRunTraceNodeDetail(
    applicationId,
    runId,
    traceNodeId,
    detailRefId,
    getApplicationsApiBaseUrl(),
    TRACE_NODE_ARTIFACT_PREVIEW_AUTO_QUERY
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
  return getConsoleApplicationRunResumeTimeline(
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

function applicationRunLogSummaryFilter(
  applicationId: string,
  input: FetchApplicationRunsInput
) {
  const filter: Record<string, unknown> = {
    application_id: { $eq: applicationId }
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

function stringField(record: Record<string, unknown>, field: string): string {
  const value = record[field];

  if (typeof value !== 'string') {
    throw new Error(`invalid_${field}`);
  }

  return value;
}

function optionalStringField(
  record: Record<string, unknown>,
  field: string
): string | null {
  const value = record[field];

  if (value === null || value === undefined) {
    return null;
  }

  if (typeof value !== 'string') {
    throw new Error(`invalid_${field}`);
  }

  return value;
}

function numberField(record: Record<string, unknown>, field: string): number {
  const value = record[field];

  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`invalid_${field}`);
  }

  return value;
}

function optionalNumberField(
  record: Record<string, unknown>,
  field: string
): number | null {
  const value = record[field];

  if (value === null || value === undefined) {
    return null;
  }

  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`invalid_${field}`);
  }

  return value;
}

function recordPayload(
  record: Record<string, unknown>,
  field: string
): Record<string, unknown> {
  const value = record[field];

  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`invalid_${field}`);
  }

  return value as Record<string, unknown>;
}

function optionalRecordPayload(
  record: Record<string, unknown>,
  field: string
): Record<string, unknown> | null {
  const value = record[field];

  if (value === null || value === undefined) {
    return null;
  }

  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`invalid_${field}`);
  }

  return value as Record<string, unknown>;
}

function toApplicationRunSummary(
  record: Record<string, unknown>
): ApplicationRunSummary {
  const id = stringField(record, 'id');

  return {
    id,
    application_id: stringField(record, 'application_id'),
    scope_id: stringField(record, 'scope_id'),
    run_mode: stringField(
      record,
      'run_mode'
    ) as ApplicationRunSummary['run_mode'],
    execution_stage: stringField(
      record,
      'execution_stage'
    ) as ApplicationRunSummary['execution_stage'],
    invocation_source: stringField(
      record,
      'invocation_source'
    ) as ApplicationRunSummary['invocation_source'],
    principal: recordPayload(
      record,
      'principal'
    ) as unknown as ApplicationRunSummary['principal'],
    status: stringField(record, 'status'),
    target_node_id: optionalStringField(record, 'target_node_id'),
    title: stringField(record, 'title'),
    expand_id: optionalStringField(record, 'expand_id'),
    external_user: optionalStringField(record, 'external_user'),
    authorized_account: optionalStringField(record, 'authorized_account'),
    api_key_id: optionalStringField(record, 'api_key_id'),
    api_key_name_snapshot: optionalStringField(record, 'api_key_name_snapshot'),
    publication_version_id: optionalStringField(
      record,
      'publication_version_id'
    ),
    external_conversation_id: optionalStringField(
      record,
      'external_conversation_id'
    ),
    external_trace_id: optionalStringField(record, 'external_trace_id'),
    compatibility_mode: optionalStringField(record, 'compatibility_mode'),
    idempotency_key: optionalStringField(record, 'idempotency_key'),
    total_tokens: optionalNumberField(record, 'total_tokens'),
    input_tokens: optionalNumberField(record, 'input_tokens'),
    output_tokens: optionalNumberField(record, 'output_tokens'),
    input_cache_hit_tokens: optionalNumberField(
      record,
      'input_cache_hit_tokens'
    ),
    input_cache_hit_rate: optionalNumberField(record, 'input_cache_hit_rate'),
    unique_node_count: numberField(record, 'unique_node_count'),
    tool_callback_count: numberField(record, 'tool_callback_count'),
    started_at: stringField(record, 'started_at'),
    finished_at: optionalStringField(record, 'finished_at'),
    created_at: stringField(record, 'created_at'),
    updated_at: stringField(record, 'updated_at')
  };
}

function toApplicationConversation(
  record: Record<string, unknown>
): ApplicationConversationRecord {
  return {
    id: stringField(record, 'id'),
    application_id: stringField(record, 'application_id'),
    scope_id: stringField(record, 'scope_id'),
    external_conversation_id: stringField(record, 'external_conversation_id'),
    external_user: optionalStringField(record, 'external_user'),
    api_key_id: optionalStringField(record, 'api_key_id'),
    created_at: stringField(record, 'created_at'),
    updated_at: stringField(record, 'updated_at')
  };
}

function conversationMessageRole(
  record: Record<string, unknown>
): ApplicationConversationMessageRecord['role'] {
  const role = stringField(record, 'role');

  if (role !== 'system' && role !== 'user' && role !== 'assistant') {
    throw new Error('invalid_role');
  }

  return role;
}

function toApplicationConversationMessage(
  record: Record<string, unknown>
): ApplicationConversationMessageRecord {
  return {
    id: stringField(record, 'id'),
    application_id: stringField(record, 'application_id'),
    scope_id: stringField(record, 'scope_id'),
    conversation_id: optionalStringField(record, 'conversation_id'),
    flow_run_id: optionalStringField(record, 'flow_run_id'),
    role: conversationMessageRole(record),
    content: stringField(record, 'content'),
    sequence: numberField(record, 'sequence'),
    status: stringField(record, 'status'),
    created_at: stringField(record, 'created_at'),
    started_at: optionalStringField(record, 'started_at'),
    finished_at: optionalStringField(record, 'finished_at')
  };
}

function toApplicationRunNodeRun(
  record: Record<string, unknown>
): ConsoleNodeRunDetail {
  return {
    id: stringField(record, 'id'),
    flow_run_id: stringField(record, 'flow_run_id'),
    node_id: stringField(record, 'node_id'),
    node_type: stringField(record, 'node_type'),
    node_alias: stringField(record, 'node_alias'),
    status: stringField(record, 'status'),
    input_payload: recordPayload(record, 'input_payload'),
    input_payload_view:
      optionalRecordPayload(record, 'input_payload_view') ?? undefined,
    output_payload: recordPayload(record, 'output_payload'),
    error_payload: optionalRecordPayload(record, 'error_payload'),
    metrics_payload: recordPayload(record, 'metrics_payload'),
    debug_payload: optionalRecordPayload(record, 'debug_payload') ?? undefined,
    started_at: stringField(record, 'started_at'),
    finished_at: optionalStringField(record, 'finished_at')
  };
}

function toApplicationRunEvent(
  record: Record<string, unknown>
): ConsoleRunEvent {
  return {
    id: stringField(record, 'id'),
    flow_run_id: stringField(record, 'flow_run_id'),
    node_run_id: optionalStringField(record, 'node_run_id'),
    sequence: numberField(record, 'sequence'),
    event_type: stringField(record, 'event_type'),
    payload: recordPayload(record, 'payload'),
    created_at: stringField(record, 'created_at')
  };
}

function toApplicationRunCheckpoint(
  record: Record<string, unknown>
): ConsoleRunCheckpoint {
  return {
    id: stringField(record, 'id'),
    flow_run_id: stringField(record, 'flow_run_id'),
    node_run_id: optionalStringField(record, 'node_run_id'),
    status: stringField(record, 'status'),
    reason: stringField(record, 'reason'),
    locator_payload: recordPayload(record, 'locator_payload'),
    variable_snapshot: recordPayload(record, 'variable_snapshot'),
    external_ref_payload: optionalRecordPayload(record, 'external_ref_payload'),
    created_at: stringField(record, 'created_at')
  };
}

function toApplicationRunCallbackTask(
  record: Record<string, unknown>
): ConsoleCallbackTask {
  return {
    id: stringField(record, 'id'),
    flow_run_id: stringField(record, 'flow_run_id'),
    node_run_id: stringField(record, 'node_run_id'),
    callback_kind: stringField(record, 'callback_kind'),
    status: stringField(record, 'status') as ConsoleCallbackTask['status'],
    request_payload: recordPayload(record, 'request_payload'),
    response_payload: optionalRecordPayload(record, 'response_payload'),
    external_ref_payload: optionalRecordPayload(record, 'external_ref_payload'),
    created_at: stringField(record, 'created_at'),
    completed_at: optionalStringField(record, 'completed_at')
  };
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
