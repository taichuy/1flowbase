import { fetchConsoleHttpNodeProxyOptions } from '@1flowbase/api-client';
import { getApplicationsApiBaseUrl } from '../../applications/api/applications';

export const httpNodeProxyOptionsQueryKey = [
  'agent-flow',
  'http-node-proxy-options'
] as const;

export function fetchHttpNodeProxyOptions() {
  return fetchConsoleHttpNodeProxyOptions(getApplicationsApiBaseUrl());
}
