import {
  getConsoleProviderTrajectory,
  getConsoleProviderTrajectoryBody
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
  eventId: string
) {
  return getConsoleProviderTrajectoryBody(
    applicationId,
    runId,
    nodeRunId,
    eventId,
    getApplicationsApiBaseUrl()
  );
}
