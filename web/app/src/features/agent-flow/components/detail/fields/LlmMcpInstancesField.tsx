import { useQuery } from '@tanstack/react-query';
import { Select } from 'antd';

import {
  agentFlowMcpInstanceOptionsQueryKey,
  fetchAgentFlowMcpInstanceOptions
} from '../../../api/mcp-instance-options';
import type { SchemaFieldRendererProps } from '../../../../../shared/schema-ui/v1/registry/create-renderer-registry';

export function LlmMcpInstancesField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const optionsQuery = useQuery({
    queryKey: agentFlowMcpInstanceOptionsQueryKey,
    queryFn: fetchAgentFlowMcpInstanceOptions
  });
  const rawValue = adapter.getValue(block.path);
  const value = Array.isArray(rawValue)
    ? rawValue.filter((item): item is string => typeof item === 'string')
    : [];

  return (
    <Select
      allowClear
      aria-label={block.label}
      loading={optionsQuery.isLoading}
      mode="multiple"
      options={optionsQuery.data ?? []}
      value={value}
      onChange={(nextValue) => adapter.setValue(block.path, nextValue)}
    />
  );
}
