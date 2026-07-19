import { useQuery } from '@tanstack/react-query';
import { Select } from 'antd';

import type { SchemaFieldRendererProps } from '../../../../../shared/schema-ui/v1/registry/create-renderer-registry';
import {
  dataModelOptionsQueryKey,
  fetchDataModelOptions
} from '../../../api/data-model-options';

export function DataModelField({ adapter, block }: SchemaFieldRendererProps) {
  const value = adapter.getValue(block.path);
  const optionsQuery = useQuery({
    queryKey: dataModelOptionsQueryKey,
    queryFn: fetchDataModelOptions,
    staleTime: 60_000
  });
  const dataModelOptions = optionsQuery.data ?? [];
  const selectOptions = dataModelOptions.map((option) => ({
    value: option.value,
    label: option.label,
    disabled: option.disabled,
    disabledReason: option.disabledReason
  }));

  function selectDataModel(nextValue: string) {
    const selectedOption = dataModelOptions.find(
      (option) => option.value === nextValue
    );

    adapter.setValue(block.path, nextValue);

    if (!selectedOption) {
      adapter.setValue('config.data_model_id', '');
      adapter.setValue('config.data_model_label', '');
      adapter.setValue('config.data_model_fields', []);
      return;
    }

    adapter.setValue('config.data_model_id', selectedOption.modelId);
    adapter.setValue('config.data_model_label', selectedOption.label);
    adapter.setValue('config.data_model_fields', selectedOption.fields);
  }

  return (
    <Select
      aria-label={block.label}
      loading={optionsQuery.isLoading}
      options={selectOptions}
      optionRender={(option) => {
        const dataModelOption = option.data as {
          value: string;
          label: string;
          disabled?: boolean;
          disabledReason?: string | null;
        };

        return (
          <div
            aria-disabled={dataModelOption.disabled ? true : undefined}
            data-testid={`data-model-option-${dataModelOption.value}`}
          >
            <span>{dataModelOption.label}</span>
            {dataModelOption.disabled && dataModelOption.disabledReason ? (
              <span> · {dataModelOption.disabledReason}</span>
            ) : null}
          </div>
        );
      }}
      value={typeof value === 'string' && value ? value : undefined}
      onChange={selectDataModel}
    />
  );
}
