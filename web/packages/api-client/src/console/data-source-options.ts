import { apiFetch } from '../transport';

export interface ConsoleAgentFlowDataSourceOption {
  data_source_instance_id: string;
  display_name: string;
  capability: string;
}

export function fetchConsoleAgentFlowDataSourceOptions(baseUrl?: string) {
  return apiFetch<ConsoleAgentFlowDataSourceOption[]>({
    path: '/api/console/data-sources/agent-flow-options',
    baseUrl
  });
}
