import {
  createConsoleApplicationLogDeletionJob,
  getConsoleApplicationLogDeletionJob,
  previewConsoleApplicationLogDeletion,
  stopConsoleApplicationLogDeletionJob,
  type AgentLogsDeleteScope,
  type AgentLogsDeleteJobCreate
} from '@1flowbase/api-client';
import { getApplicationsApiBaseUrl } from './applications';

export const logDeletionJobKey = (applicationId: string, jobId?: string) =>
  [
    'applications',
    applicationId,
    'log-deletion-job',
    jobId ?? 'latest'
  ] as const;
export const logDeletionPreviewKey = (
  applicationId: string,
  scope: AgentLogsDeleteScope | null
) => ['applications', applicationId, 'log-deletion-preview', scope] as const;
export function previewApplicationLogDeletion(
  applicationId: string,
  scope: AgentLogsDeleteScope,
  signal?: AbortSignal
) {
  return previewConsoleApplicationLogDeletion(
    applicationId,
    scope,
    getApplicationsApiBaseUrl(),
    signal
  );
}
export async function fetchApplicationLogDeletionJob(
  applicationId: string,
  jobId?: string,
  signal?: AbortSignal
) {
  return (
    await getConsoleApplicationLogDeletionJob(
      applicationId,
      jobId,
      getApplicationsApiBaseUrl(),
      signal
    )
  ).job;
}
export async function startApplicationLogDeletion(
  applicationId: string,
  input: AgentLogsDeleteJobCreate,
  csrfToken: string
) {
  return (
    await createConsoleApplicationLogDeletionJob(
      applicationId,
      input,
      csrfToken,
      getApplicationsApiBaseUrl()
    )
  ).job;
}
export async function stopApplicationLogDeletion(
  applicationId: string,
  jobId: string,
  csrfToken: string
) {
  return (
    await stopConsoleApplicationLogDeletionJob(
      applicationId,
      jobId,
      csrfToken,
      getApplicationsApiBaseUrl()
    )
  ).job;
}
