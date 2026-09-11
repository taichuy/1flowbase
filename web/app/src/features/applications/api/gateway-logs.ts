import { getConsoleGatewayLogs, type GatewayLogQuery } from '@1flowbase/api-client';
import { getApplicationsApiBaseUrl } from './applications';

export function fetchGatewayLogs(applicationId: string, query: GatewayLogQuery) {
  return getConsoleGatewayLogs(applicationId, query, getApplicationsApiBaseUrl());
}
