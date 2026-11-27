import { fetchConsoleMcpInstances } from '@1flowbase/api-client';

export const agentFlowMcpInstanceOptionsQueryKey = [
  'agent-flow',
  'mcp-instance-options'
] as const;

export async function fetchAgentFlowMcpInstanceOptions() {
  const instances = await fetchConsoleMcpInstances({ status: 'enabled' });
  return instances.flatMap((instance) =>
    instance.status === 'enabled'
      ? [
          {
            value: instance.instance_id,
            label: instance.name,
            registrationPrefix: instance.llm_tool_registration.prefix
          }
        ]
      : []
  );
}
