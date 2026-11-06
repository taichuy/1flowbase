import { Button, Input, InputNumber, Select, Space } from 'antd';
import { i18nText } from '../../../../../shared/i18n/text';
import { dataFieldOptions } from '../../../lib/data-blocks/fields';
import { isDataFilterGroup } from '../../../lib/data-blocks/filters';
import type {
  DataBlockField,
  DataFilterCondition,
  DataFilterGroup,
  DataFilterOperator
} from '../../../lib/data-blocks/types';

function operators(
  field: DataBlockField | undefined
): Array<{ value: DataFilterOperator; label: string }> {
  const values: Array<{ value: DataFilterOperator; label: string }> = [
    { value: '$eq', label: i18nText('frontstage', 'data_blocks.filter.equal') },
    {
      value: '$ne',
      label: i18nText('frontstage', 'data_blocks.filter.not_equal')
    },
    { value: '$in', label: i18nText('frontstage', 'data_blocks.filter.in') }
  ];
  if (field?.field_kind === 'string' || field?.field_kind === 'text') {
    values.push(
      {
        value: '$includes',
        label: i18nText('frontstage', 'data_blocks.filter.includes')
      },
      {
        value: '$notIncludes',
        label: i18nText('frontstage', 'data_blocks.filter.not_includes')
      }
    );
  }
  if (field?.field_kind === 'number' || field?.field_kind === 'datetime') {
    values.push(
      {
        value: '$gt',
        label: i18nText('frontstage', 'data_blocks.filter.greater')
      },
      {
        value: '$gte',
        label: i18nText('frontstage', 'data_blocks.filter.greater_equal')
      },
      {
        value: '$lt',
        label: i18nText('frontstage', 'data_blocks.filter.less')
      },
      {
        value: '$lte',
        label: i18nText('frontstage', 'data_blocks.filter.less_equal')
      }
    );
  }
  return values;
}

function FilterValue({
  field,
  condition,
  onChange
}: {
  field: DataBlockField | undefined;
  condition: DataFilterCondition;
  onChange(value: unknown): void;
}) {
  const options = field ? dataFieldOptions(field) : [];
  if (condition.operator === '$in') {
    const value = Array.isArray(condition.value)
      ? condition.value.map(String)
      : [];
    return (
      <Select
        aria-label={i18nText('frontstage', 'data_blocks.filter.value')}
        style={{ minWidth: 140, flex: 1 }}
        mode={options.length ? 'multiple' : 'tags'}
        value={value}
        options={options.map((item) => ({
          ...item,
          value: String(item.value)
        }))}
        onChange={(next: string[]) =>
          onChange(
            next.map((item) =>
              field?.field_kind === 'number' ? Number(item) : item
            )
          )
        }
      />
    );
  }
  if (field?.field_kind === 'boolean')
    return (
      <Select
        aria-label={i18nText('frontstage', 'data_blocks.filter.value')}
        style={{ width: 120 }}
        value={condition.value == null ? undefined : String(condition.value)}
        options={[
          {
            value: 'true',
            label: i18nText('frontstage', 'data_blocks.common.yes')
          },
          {
            value: 'false',
            label: i18nText('frontstage', 'data_blocks.common.no')
          }
        ]}
        onChange={(value) => onChange(value === 'true')}
      />
    );
  if (options.length)
    return (
      <Select
        aria-label={i18nText('frontstage', 'data_blocks.filter.value')}
        style={{ minWidth: 140, flex: 1 }}
        value={condition.value == null ? undefined : String(condition.value)}
        options={options.map((item) => ({
          ...item,
          value: String(item.value)
        }))}
        onChange={(value) =>
          onChange(
            options.find((option) => String(option.value) === value)?.value
          )
        }
      />
    );
  if (field?.field_kind === 'number')
    return (
      <InputNumber
        aria-label={i18nText('frontstage', 'data_blocks.filter.value')}
        value={typeof condition.value === 'number' ? condition.value : null}
        onChange={onChange}
      />
    );
  return (
    <Input
      aria-label={i18nText('frontstage', 'data_blocks.filter.value')}
      style={{ flex: 1, minWidth: 120 }}
      value={condition.value == null ? '' : String(condition.value)}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}

export function DataBlockFilterEditor({
  value,
  onChange,
  fields
}: {
  value: DataFilterGroup;
  onChange(value: DataFilterGroup): void;
  fields: readonly DataBlockField[];
}) {
  const update = (
    id: string,
    replacement: DataFilterGroup | DataFilterCondition
  ) =>
    onChange({
      ...value,
      conditions: value.conditions.map((item) =>
        item.id === id ? replacement : item
      )
    });
  return (
    <div
      className="data-block-filter-group"
      style={{ display: 'grid', gap: 10, minWidth: 260 }}
    >
      <Space>
        <span>{i18nText('frontstage', 'data_blocks.filter.match')}</span>
        <Select
          aria-label={i18nText('frontstage', 'data_blocks.filter.conjunction')}
          value={value.conjunction}
          options={[
            {
              value: 'and',
              label: i18nText('frontstage', 'data_blocks.filter.all')
            },
            {
              value: 'or',
              label: i18nText('frontstage', 'data_blocks.filter.any')
            }
          ]}
          onChange={(conjunction) => onChange({ ...value, conjunction })}
        />
      </Space>
      {value.conditions.map((condition) =>
        isDataFilterGroup(condition) ? (
          <div
            key={condition.id}
            style={{
              padding: 10,
              border: '1px solid #e8e8e8',
              borderRadius: 4
            }}
          >
            <DataBlockFilterEditor
              value={condition}
              fields={fields}
              onChange={(next) => update(condition.id, next)}
            />
            <Button
              size="small"
              type="text"
              onClick={() =>
                onChange({
                  ...value,
                  conditions: value.conditions.filter(
                    (item) => item.id !== condition.id
                  )
                })
              }
            >
              {i18nText('frontstage', 'data_blocks.filter.remove_group')}
            </Button>
          </div>
        ) : (
          <div
            key={condition.id}
            style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}
          >
            <Select
              aria-label={i18nText('frontstage', 'data_blocks.filter.field')}
              style={{ minWidth: 130 }}
              value={condition.field_code || undefined}
              options={fields.map((field) => ({
                value: field.code,
                label: field.title
              }))}
              onChange={(field_code) =>
                update(condition.id, {
                  ...condition,
                  field_code,
                  operator: '$eq',
                  value: undefined
                })
              }
            />
            <Select
              aria-label={i18nText('frontstage', 'data_blocks.filter.operator')}
              style={{ minWidth: 110 }}
              value={condition.operator}
              options={operators(
                fields.find((field) => field.code === condition.field_code)
              )}
              onChange={(operator) =>
                update(condition.id, {
                  ...condition,
                  operator,
                  value: undefined
                })
              }
            />
            <FilterValue
              field={fields.find(
                (field) => field.code === condition.field_code
              )}
              condition={condition}
              onChange={(next) =>
                update(condition.id, { ...condition, value: next })
              }
            />
            <Button
              aria-label={i18nText('frontstage', 'data_blocks.common.remove')}
              onClick={() =>
                onChange({
                  ...value,
                  conditions: value.conditions.filter(
                    (item) => item.id !== condition.id
                  )
                })
              }
            >
              ×
            </Button>
          </div>
        )
      )}
      <Space wrap>
        <Button
          type="link"
          disabled={!fields.length}
          onClick={() =>
            onChange({
              ...value,
              conditions: [
                ...value.conditions,
                {
                  id: crypto.randomUUID(),
                  field_code: fields[0]!.code,
                  operator: '$eq',
                  value: ''
                }
              ]
            })
          }
        >
          {i18nText('frontstage', 'data_blocks.filter.add_condition')}
        </Button>
        <Button
          type="link"
          onClick={() =>
            onChange({
              ...value,
              conditions: [
                ...value.conditions,
                { id: crypto.randomUUID(), conjunction: 'and', conditions: [] }
              ]
            })
          }
        >
          {i18nText('frontstage', 'data_blocks.filter.add_group')}
        </Button>
      </Space>
    </div>
  );
}
