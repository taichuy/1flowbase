import { useQuery } from '@tanstack/react-query';
import { Select } from 'antd';

import type { SchemaFieldRendererProps } from '../../../../../../shared/schema-ui/v1/registry/create-renderer-registry';
import {
  dataSourceOptionsQueryKey,
  fetchDataSourceOptions
} from '../../../../api/data-source-options';

export function DataSourceField({ adapter, block }: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const optionsQuery = useQuery({
    queryKey: dataSourceOptionsQueryKey,
    queryFn: fetchDataSourceOptions,
    staleTime: 60_000
  });

  return (
    <Select
      aria-label={block.label}
      loading={optionsQuery.isLoading}
      options={(optionsQuery.data ?? []).map((option) => ({
        value: option.data_source_instance_id,
        label: option.display_name
      }))}
      value={typeof value === 'string' && value ? value : undefined}
      onChange={(nextValue) => adapter.setValue(block.path, nextValue)}
    />
  );
}
