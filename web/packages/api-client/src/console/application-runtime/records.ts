import { apiFetch } from '../../transport';
import type {
  ClientTrajectoryOptions,
  ClientTrajectoryStep,
  ClientTrajectorySection
} from './client-trajectory';

export interface ConsoleApplicationLogRecordOverview {
  record_id: string;
  source_kind: 'native' | 'imported';
  source_client: string | null;
  source_session_id: string | null;
  source_task_id: string | null;
  native_run_id: string | null;
  title: string;
  outcome: string;
  messages: Array<{ role: string; content: string; sequence: number }>;
  total_tokens: number | null;
  input_tokens: number | null;
  output_tokens: number | null;
  input_cache_hit_tokens: number | null;
  cost_breakdown: { total_cost: string | null };
  available_views: Array<'conversation' | 'client_trajectory'>;
}
export type RecordClientTrajectoryCursor = number | string;
export interface RecordClientTrajectoryPage {
  items: ClientTrajectoryStep[];
  next_cursor: RecordClientTrajectoryCursor | null;
  integrity: string;
}
const recordPath = (applicationId: string, recordId: string) =>
  `/api/console/applications/${applicationId}/logs/records/${recordId}`;
export function getConsoleApplicationLogRecord(
  applicationId: string,
  recordId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationLogRecordOverview>({
    path: recordPath(applicationId, recordId),
    baseUrl
  });
}
export function getConsoleApplicationLogRecordClientTrajectory(
  applicationId: string,
  recordId: string,
  cursor?: RecordClientTrajectoryCursor,
  options?: ClientTrajectoryOptions,
  baseUrl?: string
) {
  const query = new URLSearchParams({
    limit: cursor === undefined ? '50' : '100'
  });
  if (cursor !== undefined) query.set('cursor', String(cursor));
  if (options?.request_id) query.set('request_id', options.request_id);
  if (options?.focus_step_id && cursor === undefined)
    query.set('focus_step_id', options.focus_step_id);
  return apiFetch<RecordClientTrajectoryPage>({
    path: `${recordPath(applicationId, recordId)}/client-trajectory?${query}`,
    baseUrl
  });
}
export function getConsoleApplicationLogRecordClientTrajectorySection(
  applicationId: string,
  recordId: string,
  stepId: string,
  section: string,
  cursor?: number,
  baseUrl?: string
) {
  const query = new URLSearchParams({ limit: '8', section });
  if (cursor !== undefined) query.set('cursor', String(cursor));
  return apiFetch<ClientTrajectorySection>({
    path: `${recordPath(applicationId, recordId)}/client-trajectory/${stepId}?${query}`,
    baseUrl
  });
}

/** Imported turns are selected by started_at, using [from,to). */
export type AgentLogsDeleteScope = (
  | { mode: 'all_time' }
  | { mode: 'time_range'; started_at_from: string; started_at_to: string }
) & { batch_size?: number; ingested_at_before?: string };
export interface AgentLogsDeleteReceipt {
  deleted_records: number;
  has_more: boolean;
  ingested_at_before: string;
}
export function deleteConsoleApplicationLogs(
  applicationId: string,
  scope: AgentLogsDeleteScope,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<AgentLogsDeleteReceipt>({
    path: `/api/console/applications/${applicationId}/logs`,
    method: 'DELETE',
    body: scope,
    csrfToken,
    baseUrl
  });
}

export interface AgentLogsDeletePreview {
  total_records: number;
}
export interface AgentLogsDeleteJob {
  job_id: string;
  application_id: string;
  scope: AgentLogsDeleteScope;
  ingested_at_before: string;
  status: 'queued' | 'running' | 'succeeded' | 'stopped' | 'failed';
  total_records: number;
  deleted_records: number;
  stop_requested: boolean;
  error_code: string | null;
  created_at: string;
  updated_at: string;
}
export interface AgentLogsDeleteJobCreate {
  job_id: string;
  scope: AgentLogsDeleteScope;
}
const deletionPath = (applicationId: string) =>
  `/api/console/applications/${applicationId}/logs`;
export function previewConsoleApplicationLogDeletion(
  applicationId: string,
  scope: AgentLogsDeleteScope,
  baseUrl?: string,
  signal?: AbortSignal
) {
  return apiFetch<AgentLogsDeletePreview>({
    path: `${deletionPath(applicationId)}/deletion-preview`,
    method: 'POST',
    body: scope,
    baseUrl,
    signal
  });
}
export function createConsoleApplicationLogDeletionJob(
  applicationId: string,
  input: AgentLogsDeleteJobCreate,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<{ job: AgentLogsDeleteJob }>({
    path: `${deletionPath(applicationId)}/deletion-jobs`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}
export function getConsoleApplicationLogDeletionJob(
  applicationId: string,
  jobId?: string,
  baseUrl?: string,
  signal?: AbortSignal
) {
  return apiFetch<{ job: AgentLogsDeleteJob | null }>({
    path: `${deletionPath(applicationId)}/deletion-jobs/${jobId ?? 'latest'}`,
    baseUrl,
    signal
  });
}
export function stopConsoleApplicationLogDeletionJob(
  applicationId: string,
  jobId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<{ job: AgentLogsDeleteJob }>({
    path: `${deletionPath(applicationId)}/deletion-jobs/${jobId}/stop`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}
