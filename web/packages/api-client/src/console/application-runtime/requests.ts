import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import { ApiClientError } from '../../errors';
import { apiFetch, apiFetchBlob } from '../../transport';
import type {
  ConsoleApplicationRunArchive,
  ConsoleApplicationRunArchiveVersionInput,
  ConsoleApplicationRunDetail,
  ConsoleApplicationRunMonitoringReport,
  ConsoleApplicationRunOverview,
  ConsoleApplicationRunsPage,
  ConsoleApplicationRuntimeActivity,
  ConsoleApplicationRunTraceNodeChildren,
  ConsoleApplicationRunTraceNodeContent,
  ConsoleApplicationRunTraceNodeDetail,
  ConsoleApplicationRunTraceToolCallbackContent,
  ConsoleApplicationRunTraceTree,
  ConsoleApplicationRunResumeTimeline,
  ConsoleApplicationRunResumeTimelineSummary,
  ConsoleRunArchiveChunkUpload,
  ConsoleRunArchiveImportJob,
  ConsoleRunArchiveUploadSession,
  ConsoleRunArchiveUploadSessionCreateInput,
  ConsoleDebugVariableSnapshot,
  ConsoleNodeLastRun,
  ConsoleRuntimeDebugArtifactsResolveResponse,
  DeleteConsoleDebugVariableCacheEntriesInput,
  GetConsoleApplicationRunMonitoringReportInput,
  GetConsoleApplicationRunsInput,
  RuntimeDebugStreamQuery,
  RuntimeDebugStreamResponse,
  UpsertConsoleDebugVariableCacheEntryInput
} from './types';

export interface ConsoleApplicationRunTraceNodeArtifactPreviewQuery {
  artifact_preview?: 'auto';
  artifact_preview_field?: string[];
}

function traceNodeArtifactPreviewQueryString(
  query?: ConsoleApplicationRunTraceNodeArtifactPreviewQuery
) {
  const searchParams = new URLSearchParams();
  if (query?.artifact_preview) {
    searchParams.set('artifact_preview', query.artifact_preview);
  }
  for (const fieldPath of query?.artifact_preview_field ?? []) {
    searchParams.append('artifact_preview_field', fieldPath);
  }

  const queryString = searchParams.toString();
  return queryString ? `?${queryString}` : '';
}

function archiveVersionQueryString(input?: ConsoleApplicationRunArchiveVersionInput) {
  const searchParams = new URLSearchParams();
  if (input?.archive_version !== undefined) {
    searchParams.set('archive_version', String(input.archive_version));
  }
  const queryString = searchParams.toString();
  return queryString ? `?${queryString}` : '';
}

export function startConsoleNodeDebugPreview(
  applicationId: string,
  nodeId: string,
  input: {
    input_payload: Record<string, unknown>;
    document?: FlowAuthoringDocument;
    debug_session_id?: string;
  },
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNodeLastRun>({
    path: `/api/console/applications/${applicationId}/orchestration/nodes/${nodeId}/debug-runs`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function startConsoleFlowDebugRun(
  applicationId: string,
  input: {
    input_payload: Record<string, unknown>;
    document?: FlowAuthoringDocument;
    debug_session_id?: string;
  },
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationRunDetail>({
    path: `/api/console/applications/${applicationId}/orchestration/debug-runs`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function getConsoleApplicationRunDebugSnapshot(
  applicationId: string,
  runId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationRunDetail>({
    path: `/api/console/applications/${applicationId}/orchestration/runs/${runId}/debug-snapshot`,
    baseUrl
  });
}

export function resumeConsoleFlowRun(
  applicationId: string,
  runId: string,
  input: { checkpoint_id: string; input_payload: Record<string, unknown> },
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationRunDetail>({
    path: `/api/console/applications/${applicationId}/orchestration/runs/${runId}/resume`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function cancelConsoleFlowRun(
  applicationId: string,
  runId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationRunDetail>({
    path: `/api/console/applications/${applicationId}/orchestration/runs/${runId}/cancel`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function completeConsoleCallbackTask(
  applicationId: string,
  callbackTaskId: string,
  input: { response_payload: Record<string, unknown> },
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationRunDetail>({
    path: `/api/console/applications/${applicationId}/orchestration/callback-tasks/${callbackTaskId}/complete`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function getConsoleApplicationRuns(
  applicationId: string,
  input: GetConsoleApplicationRunsInput = {},
  baseUrl?: string
) {
  const page = input.page ?? 1;
  const pageSize = input.page_size ?? 20;
  const searchParams = new URLSearchParams({
    page: String(page),
    page_size: String(pageSize)
  });
  if (input.time_range_days !== undefined) {
    searchParams.set('time_range_days', String(input.time_range_days));
  }
  if (input.sort_by !== undefined) {
    searchParams.set('sort_by', input.sort_by);
  }
  if (input.sort_order !== undefined) {
    searchParams.set('sort_order', input.sort_order);
  }
  if (input.cache_mode !== undefined && input.cache_mode !== 'default') {
    searchParams.set('cache_mode', input.cache_mode);
  }
  return apiFetch<ConsoleApplicationRunsPage>({
    path:
      `/api/console/applications/${applicationId}/logs/runs?` +
      searchParams.toString(),
    baseUrl
  });
}

export function getConsoleApplicationRunMonitoringReport(
  applicationId: string,
  input: GetConsoleApplicationRunMonitoringReportInput = {},
  baseUrl?: string
) {
  const searchParams = new URLSearchParams();
  if (input.from !== undefined) {
    searchParams.set('from', input.from);
  }
  if (input.to !== undefined) {
    searchParams.set('to', input.to);
  }
  if (input.time_range_days !== undefined) {
    searchParams.set('time_range_days', String(input.time_range_days));
  }
  if (input.bucket !== undefined) {
    searchParams.set('bucket', input.bucket);
  }
  const query = searchParams.toString();

  return apiFetch<ConsoleApplicationRunMonitoringReport>({
    path:
      `/api/console/applications/${applicationId}/monitoring/run-metrics` +
      (query ? `?${query}` : ''),
    baseUrl
  });
}

export function getConsoleApplicationRuntimeActivity(
  applicationId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationRuntimeActivity>({
    path: `/api/console/applications/${applicationId}/monitoring/runtime-activity`,
    method: 'GET',
    baseUrl
  });
}

export function getConsoleApplicationRunTraceTree(
  applicationId: string,
  runId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationRunTraceTree>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/trace-tree`,
    baseUrl
  });
}

export function exportConsoleApplicationRunTraceDump(
  applicationId: string,
  runId: string,
  baseUrl?: string
) {
  return apiFetchBlob({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/export`,
    method: 'GET',
    baseUrl
  });
}

export function exportConsoleApplicationRunsTraceDumpZip(
  applicationId: string,
  runIds: string[],
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchBlob({
    path: `/api/console/applications/${applicationId}/logs/runs/export`,
    method: 'POST',
    body: {
      run_ids: runIds
    },
    csrfToken,
    baseUrl
  });
}

export function getConsoleApplicationRunArchive(
  applicationId: string,
  runId: string,
  baseUrl?: string,
  input?: ConsoleApplicationRunArchiveVersionInput
) {
  return apiFetch<ConsoleApplicationRunArchive>({
    path:
      `/api/console/applications/${applicationId}/logs/runs/${runId}/archive` +
      archiveVersionQueryString(input),
    method: 'GET',
    baseUrl,
    unwrapSuccess: false
  });
}

export function createConsoleApplicationRunsArchive(
  applicationId: string,
  runIds: string[],
  csrfToken: string,
  baseUrl?: string,
  input: ConsoleApplicationRunArchiveVersionInput = {}
) {
  return apiFetch<ConsoleApplicationRunArchive>({
    path: `/api/console/applications/${applicationId}/logs/runs/archive`,
    method: 'POST',
    body: {
      ...input,
      run_ids: runIds
    },
    csrfToken,
    baseUrl,
    unwrapSuccess: false
  });
}

export function createConsoleRunArchiveUploadSession(
  applicationId: string,
  input: ConsoleRunArchiveUploadSessionCreateInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleRunArchiveUploadSession>({
    path: `/api/console/applications/${applicationId}/logs/runs/archive/import-sessions`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function uploadConsoleRunArchiveChunk(
  applicationId: string,
  sessionId: string,
  chunkIndex: number,
  chunk: BodyInit,
  chunkSha256: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleRunArchiveChunkUpload>({
    path:
      `/api/console/applications/${applicationId}/logs/runs/archive/import-sessions/` +
      `${sessionId}/chunks/${chunkIndex}`,
    method: 'PUT',
    rawBody: chunk,
    contentType: 'application/octet-stream',
    headers: {
      'x-chunk-sha256': chunkSha256
    },
    csrfToken,
    baseUrl
  });
}

export function completeConsoleRunArchiveUploadSession(
  applicationId: string,
  sessionId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleRunArchiveImportJob>({
    path:
      `/api/console/applications/${applicationId}/logs/runs/archive/import-sessions/` +
      `${sessionId}/complete`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function getConsoleRunArchiveImportJob(
  applicationId: string,
  jobId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleRunArchiveImportJob>({
    path: `/api/console/applications/${applicationId}/logs/runs/archive/import-jobs/${jobId}`,
    method: 'GET',
    baseUrl
  });
}

export function getConsoleApplicationRunOverview(
  applicationId: string,
  runId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationRunOverview>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/overview`,
    baseUrl
  });
}

export function getConsoleApplicationRunTraceNodeChildren(
  applicationId: string,
  runId: string,
  parentTraceNodeId: string,
  baseUrl?: string,
  query?: {
    cursor?: string;
    page_size?: number;
  }
) {
  const searchParams = new URLSearchParams({
    parent_trace_node_id: parentTraceNodeId
  });
  if (query?.cursor) {
    searchParams.set('cursor', query.cursor);
  }
  if (query?.page_size) {
    searchParams.set('page_size', String(query.page_size));
  }

  return apiFetch<ConsoleApplicationRunTraceNodeChildren>({
    path:
      `/api/console/applications/${applicationId}/logs/runs/${runId}/trace-tree/nodes?` +
      searchParams.toString(),
    baseUrl
  });
}

export function getConsoleApplicationRunTraceNodeContent(
  applicationId: string,
  runId: string,
  traceNodeId: string,
  baseUrl?: string,
  query?: ConsoleApplicationRunTraceNodeArtifactPreviewQuery
) {
  return apiFetch<ConsoleApplicationRunTraceNodeContent>({
    path:
      `/api/console/applications/${applicationId}/logs/runs/${runId}` +
      `/trace-tree/nodes/${encodeURIComponent(traceNodeId)}/content` +
      traceNodeArtifactPreviewQueryString(query),
    baseUrl
  });
}

export function getConsoleApplicationRunTraceNodeDetail(
  applicationId: string,
  runId: string,
  traceNodeId: string,
  detailRefId: string,
  baseUrl?: string,
  query?: ConsoleApplicationRunTraceNodeArtifactPreviewQuery
) {
  return apiFetch<ConsoleApplicationRunTraceNodeDetail>({
    path:
      `/api/console/applications/${applicationId}/logs/runs/${runId}` +
      `/trace-tree/nodes/${encodeURIComponent(traceNodeId)}` +
      `/details/${encodeURIComponent(detailRefId)}` +
      traceNodeArtifactPreviewQueryString(query),
    baseUrl
  });
}

export function getConsoleApplicationRunTraceToolCallbackContent(
  applicationId: string,
  runId: string,
  traceNodeId: string,
  toolCallId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationRunTraceToolCallbackContent>({
    path:
      `/api/console/applications/${applicationId}/logs/runs/${runId}` +
      `/trace-tree/nodes/${encodeURIComponent(traceNodeId)}` +
      `/tool-callbacks/${encodeURIComponent(toolCallId)}/content`,
    baseUrl
  });
}

export function getConsoleApplicationRunResumeTimeline(
  applicationId: string,
  runId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationRunResumeTimeline>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/resume-timeline`,
    baseUrl
  });
}

export function getConsoleApplicationRunResumeTimelineSummary(
  applicationId: string,
  runId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationRunResumeTimelineSummary>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/resume-timeline-summary`,
    baseUrl
  });
}

export function getConsoleApplicationRunNodeLastRun(
  applicationId: string,
  runId: string,
  nodeId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNodeLastRun | null>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/nodes/${nodeId}`,
    baseUrl
  });
}

export function getConsoleRuntimeDebugStream(
  applicationId: string,
  runId: string,
  baseUrl?: string,
  query: RuntimeDebugStreamQuery = {}
) {
  const searchParams = new URLSearchParams();
  if (query.from_sequence !== undefined) {
    searchParams.set('from_sequence', String(query.from_sequence));
  }
  if (query.limit !== undefined) {
    searchParams.set('limit', String(query.limit));
  }
  const queryString = searchParams.toString();

  return apiFetch<RuntimeDebugStreamResponse>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/debug-stream${queryString ? `?${queryString}` : ''}`,
    baseUrl
  });
}

export function getConsoleDebugVariableSnapshot(
  applicationId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleDebugVariableSnapshot>({
    path: `/api/console/applications/${applicationId}/orchestration/debug-variable-snapshot`,
    baseUrl
  });
}

export function upsertConsoleDebugVariableCacheEntry(
  applicationId: string,
  input: UpsertConsoleDebugVariableCacheEntryInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<{ ok: boolean }>({
    path: `/api/console/applications/${applicationId}/orchestration/debug-variable-cache`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleDebugVariableCacheEntries(
  applicationId: string,
  input: DeleteConsoleDebugVariableCacheEntriesInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<{ ok: boolean }>({
    path: `/api/console/applications/${applicationId}/orchestration/debug-variable-cache`,
    method: 'DELETE',
    body: input,
    csrfToken,
    baseUrl
  });
}

export async function getConsoleRuntimeDebugArtifact(
  applicationId: string,
  artifactId: string,
  baseUrl?: string
) {
  const response = await fetch(
    `${baseUrl ?? ''}/api/console/applications/${applicationId}/orchestration/debug-artifacts/${artifactId}`,
    {
      method: 'GET',
      credentials: 'include',
      headers: {
        accept: 'application/json, text/plain;q=0.9, */*;q=0.1'
      }
    }
  );

  if (!response.ok) {
    throw await ApiClientError.fromResponse(response);
  }

  const contentType = response.headers.get('content-type') ?? '';
  if (contentType.includes('application/json')) {
    return response.json() as Promise<unknown>;
  }

  return response.text();
}

export function resolveConsoleRuntimeDebugArtifacts(
  applicationId: string,
  artifactRefs: string[],
  baseUrl?: string
) {
  return apiFetch<ConsoleRuntimeDebugArtifactsResolveResponse>({
    path: `/api/console/applications/${applicationId}/orchestration/debug-artifacts/resolve`,
    method: 'POST',
    body: { artifact_refs: artifactRefs },
    baseUrl
  });
}

export function getConsoleNodeLastRun(
  applicationId: string,
  nodeId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNodeLastRun | null>({
    path: `/api/console/applications/${applicationId}/orchestration/nodes/${nodeId}/last-run`,
    baseUrl
  });
}
