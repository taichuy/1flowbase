import { useRef } from 'react';
import type {
  DataModelQueryBindingValue,
  DataModelQueryFilter,
  DataModelQueryOperator,
  DataModelQuerySort,
  DataModelQueryValue
} from '@1flowbase/flow-schema';
import {
  Button,
  Flex,
  Input,
  InputNumber,
  Select,
  Space,
  Typography
} from 'antd';

import type { AgentFlowDataModelFieldOption } from '../../api/data-model-options';
import {
  DATA_MODEL_QUERY_OPERATORS,
  normalizeDataModelQueryBindingValue
} from '../../lib/data-model-query-binding';
import type { FlowSelectorOption } from '../../lib/selector-options';
import { SelectorField } from './SelectorField';
import { i18nText } from '../../../../shared/i18n/text';

const FILTER_TYPES = new Set([
  'string',
  'enum',
  'text',
  'datetime',
  'number',
  'boolean',
  'json',
  'many_to_one'
]);
const SORT_TYPES = new Set([
  'string',
  'enum',
  'text',
  'datetime',
  'number',
  'boolean'
]);
const EXPAND_TYPES = new Set(['many_to_one', 'one_to_many']);
const RANGE_TYPES = new Set(['number', 'datetime']);

interface DataModelQueryFieldProps {
  ariaLabel: string;
  value: unknown;
  hasDataModelSelected: boolean;
  fields: AgentFlowDataModelFieldOption[];
  selectorOptions: FlowSelectorOption[];
  includePagination?: boolean;
  onChange: (value: DataModelQueryBindingValue) => void;
}

function fieldOptions(
  fields: AgentFlowDataModelFieldOption[],
  types: Set<string>
) {
  return fields
    .filter((field) => types.has(field.valueType))
    .map((field) => ({
      value: field.code,
      label: field.title || field.code
    }));
}

function operators(fields: AgentFlowDataModelFieldOption[], fieldCode: string) {
  const field = fields.find((entry) => entry.code === fieldCode);
  const values =
    field && RANGE_TYPES.has(field.valueType)
      ? DATA_MODEL_QUERY_OPERATORS
      : (['eq', 'ne'] satisfies DataModelQueryOperator[]);

  return values.map((value) => ({ value, label: value }));
}

function constantValueForType(valueType?: string) {
  if (valueType === 'number') {
    return 0;
  }

  if (valueType === 'boolean') {
    return false;
  }

  return '';
}

function nextFilter(
  fields: AgentFlowDataModelFieldOption[]
): DataModelQueryFilter {
  const field = fieldOptions(fields, FILTER_TYPES)[0];
  const fieldMeta = fields.find((entry) => entry.code === field?.value);

  return {
    field_code: field?.value ?? '',
    operator: 'eq',
    value: {
      kind: 'constant',
      value: constantValueForType(fieldMeta?.valueType)
    }
  };
}

function nextSort(fields: AgentFlowDataModelFieldOption[]): DataModelQuerySort {
  return {
    field_code: fieldOptions(fields, SORT_TYPES)[0]?.value ?? '',
    direction: 'asc'
  };
}

function QueryValueInput({
  ariaLabel,
  value,
  valueType,
  selectorOptions,
  numeric = false,
  sourceAriaLabel,
  selectorAriaLabel,
  onChange
}: {
  ariaLabel: string;
  value: DataModelQueryValue;
  valueType?: string;
  selectorOptions: FlowSelectorOption[];
  numeric?: boolean;
  sourceAriaLabel?: string;
  selectorAriaLabel?: string;
  onChange: (value: DataModelQueryValue) => void;
}) {
  return (
    <Space.Compact block>
      <Select
        aria-label={
          sourceAriaLabel ??
          i18nText('agentFlow', 'auto.source', { value1: ariaLabel })
        }
        options={[
          { value: 'constant', label: i18nText('agentFlow', 'auto.constant') },
          {
            value: 'selector',
            label: i18nText('agentFlow', 'auto.variable_alt')
          }
        ]}
        value={value.kind}
        onChange={(kind) =>
          onChange(
            kind === 'selector'
              ? { kind: 'selector', selector: [] }
              : {
                  kind: 'constant',
                  value: numeric ? 1 : constantValueForType(valueType)
                }
          )
        }
      />
      {value.kind === 'selector' ? (
        <SelectorField
          ariaLabel={selectorAriaLabel ?? ariaLabel}
          options={selectorOptions}
          value={value.selector}
          onChange={(nextValue) =>
            onChange({ kind: 'selector', selector: nextValue as string[] })
          }
        />
      ) : numeric ? (
        <InputNumber
          aria-label={ariaLabel}
          min={1}
          value={typeof value.value === 'number' ? value.value : null}
          onChange={(nextValue) =>
            onChange({ kind: 'constant', value: nextValue ?? 1 })
          }
        />
      ) : valueType === 'number' ? (
        <InputNumber
          aria-label={ariaLabel}
          value={typeof value.value === 'number' ? value.value : null}
          onChange={(nextValue) =>
            onChange({ kind: 'constant', value: nextValue ?? null })
          }
        />
      ) : valueType === 'boolean' ? (
        <Select
          aria-label={ariaLabel}
          options={[
            { value: true, label: 'true' },
            { value: false, label: 'false' }
          ]}
          value={typeof value.value === 'boolean' ? value.value : undefined}
          onChange={(nextValue) =>
            onChange({ kind: 'constant', value: nextValue })
          }
        />
      ) : (
        <Input
          aria-label={ariaLabel}
          value={
            typeof value.value === 'string'
              ? value.value
              : JSON.stringify(value.value ?? '')
          }
          onChange={(event) =>
            onChange({ kind: 'constant', value: event.target.value })
          }
        />
      )}
    </Space.Compact>
  );
}

function DataModelQueryConditionsField({
  ariaLabel,
  query,
  fields,
  selectorOptions,
  includePagination,
  onChange
}: {
  ariaLabel: string;
  query: DataModelQueryBindingValue;
  fields: AgentFlowDataModelFieldOption[];
  selectorOptions: FlowSelectorOption[];
  includePagination: boolean;
  onChange: (patch: Partial<DataModelQueryBindingValue>) => void;
}) {
  const filterKeys = useRef<string[]>([]);
  const sortKeys = useRef<string[]>([]);
  const nextKey = useRef(0);
  for (const [keys, length] of [
    [filterKeys.current, query.filters.length],
    [sortKeys.current, query.sorts.length]
  ] as const) {
    while (keys.length < length) keys.push(`query-row-${nextKey.current++}`);
    keys.length = length;
  }
  const filterRows = query.filters.map((filter, index) => ({
    filter,
    key: filterKeys.current[index]!
  }));
  const sortRows = query.sorts.map((sort, index) => ({
    sort,
    key: sortKeys.current[index]!
  }));
  const filterOptions = fieldOptions(fields, FILTER_TYPES);
  const sortOptions = fieldOptions(fields, SORT_TYPES);
  const expandOptions = fieldOptions(fields, EXPAND_TYPES);

  return (
    <Flex aria-label={ariaLabel} vertical gap={12}>
      <Flex vertical gap={8}>
        {filterRows.map(({ filter, key }, index) => (
          <Space.Compact key={key} block>
            <Select
              aria-label={i18nText('agentFlow', 'auto.filter_field', {
                value1: index + 1
              })}
              options={filterOptions}
              value={filter.field_code || undefined}
              onChange={(fieldCode) => {
                const field = fields.find((entry) => entry.code === fieldCode);

                onChange({
                  filters: query.filters.map((entry, entryIndex) =>
                    entryIndex === index
                      ? {
                          ...entry,
                          field_code: fieldCode,
                          operator: 'eq',
                          value:
                            entry.value.kind === 'selector'
                              ? entry.value
                              : {
                                  kind: 'constant',
                                  value: constantValueForType(field?.valueType)
                                }
                        }
                      : entry
                  )
                });
              }}
            />
            <Select
              aria-label={i18nText('agentFlow', 'auto.filter_operator', {
                value1: index + 1
              })}
              options={operators(fields, filter.field_code)}
              value={filter.operator}
              onChange={(operator) =>
                onChange({
                  filters: query.filters.map((entry, entryIndex) =>
                    entryIndex === index
                      ? {
                          ...entry,
                          operator: operator as DataModelQueryOperator
                        }
                      : entry
                  )
                })
              }
            />
            <QueryValueInput
              ariaLabel={i18nText('agentFlow', 'auto.filter_value', {
                value1: index + 1
              })}
              valueType={
                fields.find((field) => field.code === filter.field_code)
                  ?.valueType
              }
              sourceAriaLabel={i18nText(
                'agentFlow',
                'auto.filter_value_source',
                { value1: index + 1 }
              )}
              selectorAriaLabel={i18nText('agentFlow', 'auto.filter_variable', {
                value1: index + 1
              })}
              selectorOptions={selectorOptions}
              value={filter.value}
              onChange={(nextValue) =>
                onChange({
                  filters: query.filters.map((entry, entryIndex) =>
                    entryIndex === index
                      ? { ...entry, value: nextValue }
                      : entry
                  )
                })
              }
            />
            <Button
              danger
              type="text"
              onClick={() => {
                filterKeys.current.splice(index, 1);
                onChange({
                  filters: query.filters.filter(
                    (_, entryIndex) => entryIndex !== index
                  )
                });
              }}
            >
              {i18nText('agentFlow', 'auto.delete')}
            </Button>
          </Space.Compact>
        ))}
        <Button
          type="dashed"
          disabled={filterOptions.length === 0}
          onClick={() =>
            onChange({ filters: [...query.filters, nextFilter(fields)] })
          }
        >
          {i18nText('agentFlow', 'auto.add_new_filter')}
        </Button>
      </Flex>
      <Flex vertical gap={8}>
        {sortRows.map(({ sort, key }, index) => (
          <Space.Compact key={key} block>
            <Select
              aria-label={i18nText('agentFlow', 'auto.sorting_field', {
                value1: index + 1
              })}
              options={sortOptions}
              value={sort.field_code || undefined}
              onChange={(fieldCode) =>
                onChange({
                  sorts: query.sorts.map((entry, entryIndex) =>
                    entryIndex === index
                      ? { ...entry, field_code: fieldCode }
                      : entry
                  )
                })
              }
            />
            <Select
              aria-label={i18nText('agentFlow', 'auto.sort_direction', {
                value1: index + 1
              })}
              options={[
                { value: 'asc', label: 'asc' },
                { value: 'desc', label: 'desc' }
              ]}
              value={sort.direction}
              onChange={(direction) =>
                onChange({
                  sorts: query.sorts.map((entry, entryIndex) =>
                    entryIndex === index
                      ? { ...entry, direction: direction as 'asc' | 'desc' }
                      : entry
                  )
                })
              }
            />
            <Button
              danger
              type="text"
              onClick={() => {
                sortKeys.current.splice(index, 1);
                onChange({
                  sorts: query.sorts.filter(
                    (_, entryIndex) => entryIndex !== index
                  )
                });
              }}
            >
              {i18nText('agentFlow', 'auto.delete')}
            </Button>
          </Space.Compact>
        ))}
        <Button
          type="dashed"
          disabled={sortOptions.length === 0}
          onClick={() =>
            onChange({ sorts: [...query.sorts, nextSort(fields)] })
          }
        >
          {i18nText('agentFlow', 'auto.add_new_sorting_rule')}
        </Button>
      </Flex>
      <Select
        aria-label={i18nText('agentFlow', 'auto.expand_association')}
        mode="multiple"
        options={expandOptions}
        value={query.expand_relations}
        onChange={(expandRelations) =>
          onChange({ expand_relations: expandRelations })
        }
      />
      {includePagination ? (
        <Space.Compact block>
          <QueryValueInput
            ariaLabel={i18nText('agentFlow', 'auto.page_number')}
            numeric
            selectorOptions={selectorOptions}
            value={query.page}
            onChange={(page) => onChange({ page })}
          />
          <QueryValueInput
            ariaLabel={i18nText('agentFlow', 'auto.quantity_per_page')}
            numeric
            selectorOptions={selectorOptions}
            value={query.page_size}
            onChange={(pageSize) => onChange({ page_size: pageSize })}
          />
        </Space.Compact>
      ) : null}
    </Flex>
  );
}

export function DataModelQueryField({
  ariaLabel,
  value,
  hasDataModelSelected,
  fields,
  selectorOptions,
  includePagination = true,
  onChange
}: DataModelQueryFieldProps) {
  const query = normalizeDataModelQueryBindingValue(value);
  const update = (patch: Partial<DataModelQueryBindingValue>) =>
    onChange({ ...query, ...patch });

  if (!hasDataModelSelected) {
    return (
      <Typography.Text type="secondary">
        {i18nText('agentFlow', 'auto.select_data_model_first')}
      </Typography.Text>
    );
  }

  return (
    <DataModelQueryConditionsField
      ariaLabel={ariaLabel}
      query={query}
      fields={fields}
      selectorOptions={selectorOptions}
      includePagination={includePagination}
      onChange={update}
    />
  );
}
