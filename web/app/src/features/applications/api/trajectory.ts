import {
  getConsoleWorkflowTrajectory,
  getConsoleWorkflowTrajectoryBody,
  type WorkflowTrajectoryOptions,
  getConsoleClientTrajectory,
  getConsoleClientTrajectorySection,
  getConsoleRunPayload,
  getConsoleProviderTrajectoryBody,
  type ClientTrajectoryOptions,
  type ProviderTrajectoryView
} from '@1flowbase/api-client';
import { getApplicationsApiBaseUrl } from './applications';

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
  cursor?: number,
  options?: ClientTrajectoryOptions
) {
  return getConsoleClientTrajectory(
    applicationId,
    runId,
    nodeRunId,
    cursor,
    getApplicationsApiBaseUrl(),
    options
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

export function fetchWorkflowTrajectory(
  applicationId: string,
  runId: string,
  cursor?: string,
  options?: WorkflowTrajectoryOptions
) {
  return getConsoleWorkflowTrajectory(
    applicationId,
    runId,
    cursor,
    options,
    getApplicationsApiBaseUrl()
  );
}
export function fetchWorkflowTrajectoryBody(
  applicationId: string,
  runId: string,
  eventId: string
) {
  return getConsoleWorkflowTrajectoryBody(
    applicationId,
    runId,
    eventId,
    getApplicationsApiBaseUrl()
  );
}
