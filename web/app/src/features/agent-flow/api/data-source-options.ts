import {
  fetchConsoleAgentFlowDataSourceOptions,
  type ConsoleAgentFlowDataSourceOption
} from '@1flowbase/api-client';

import { getApplicationsApiBaseUrl } from '../../applications/api/applications';

export type AgentFlowDataSourceOption = ConsoleAgentFlowDataSourceOption;

export const dataSourceOptionsQueryKey = [
  'agent-flow',
  'data-source-options'
] as const;

export async function fetchDataSourceOptions() {
  return fetchConsoleAgentFlowDataSourceOptions(getApplicationsApiBaseUrl());
}
