import {
  getConsoleClientTrajectory,
  getConsoleClientTrajectorySection,
  getConsoleProviderTrajectory,
  getConsoleRunTrajectory,
  getConsoleRunPayload,
  getConsoleProviderTrajectoryBody,
  type ProviderTrajectoryView
} from '@1flowbase/api-client';
import { getApplicationsApiBaseUrl } from './applications';

export function fetchProviderTrajectory(
  applicationId: string,
  runId: string,
  nodeRunId: string,
  cursor?: number
) {
  return getConsoleProviderTrajectory(
    applicationId,
    runId,
    nodeRunId,
    cursor,
    getApplicationsApiBaseUrl()
  );
}
export function fetchProviderTrajectoryBody(
  applicationId: string,
  runId: string,
  nodeRunId: string,
  eventId: string,
  cursor?: number,
  view: ProviderTrajectoryView = 'semantic'
) {
  return getConsoleProviderTrajectoryBody(
    applicationId,
    runId,
    nodeRunId,
    eventId,
    cursor,
    view,
    getApplicationsApiBaseUrl()
  );
}

export function fetchRunTrajectory(
  applicationId: string,
  runId: string,
  cursor?: number
) {
  return getConsoleRunTrajectory(
    applicationId,
    runId,
    cursor,
    getApplicationsApiBaseUrl()
  );
}

export function fetchRunPayload(
  applicationId: string,
  runId: string,
  section: 'input_payload' | 'output_payload'
) {
  return getConsoleRunPayload(
    applicationId,
    runId,
    section,
    getApplicationsApiBaseUrl()
  );
}

export function fetchClientTrajectory(
  applicationId: string,
  runId: string,
  nodeRunId?: string,
  cursor?: number
) {
  return getConsoleClientTrajectory(
    applicationId,
    runId,
    nodeRunId,
    cursor,
    getApplicationsApiBaseUrl()
  );
}
export function fetchClientTrajectorySection(
  applicationId: string,
  runId: string,
  stepId: string,
  section: string,
  nodeRunId?: string,
  cursor?: number
) {
  return getConsoleClientTrajectorySection(
    applicationId,
    runId,
    stepId,
    section,
    nodeRunId,
    cursor,
    getApplicationsApiBaseUrl()
  );
}
