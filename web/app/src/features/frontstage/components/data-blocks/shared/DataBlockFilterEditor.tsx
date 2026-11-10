import {
  Button,
  DatePicker,
  Dropdown,
  Input,
  InputNumber,
  Select,
  Space,
  theme
} from 'antd';
import dayjs from 'dayjs';
import { FunctionOutlined, CloseOutlined } from '@ant-design/icons';
import type { DataFilterVariable } from '../../../lib/data-blocks/filter-variables';
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

function emptyCondition(condition: DataFilterCondition) {
  return (
    condition.value_source !== 'variable' &&
    condition.value === null &&
    (condition.operator === '$eq' || condition.operator === '$ne')
  );
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
    const choices =
      field?.field_kind === 'boolean'
        ? [
            {
              value: true,
              label: i18nText('frontstage', 'data_blocks.common.yes')
            },
            {
              value: false,
              label: i18nText('frontstage', 'data_blocks.common.no')
            }
          ]
        : options;
    const value = Array.isArray(condition.value)
      ? condition.value.map(String)
      : [];
    return (
      <Select
        aria-label={i18nText('frontstage', 'data_blocks.filter.value')}
        style={{ minWidth: 140, flex: 1 }}
        mode={choices.length ? 'multiple' : 'tags'}
        value={value}
        options={choices.map((item) => ({
          ...item,
          value: String(item.value)
        }))}
        onChange={(next: string[]) =>
          onChange(
            next.map(
              (item) =>
                choices.find((choice) => String(choice.value) === item)
                  ?.value ??
                (field?.field_kind === 'number' ? Number(item) : item)
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
  if (field?.field_kind === 'datetime')
    return (
      <DatePicker
        showTime
        aria-label={i18nText('frontstage', 'data_blocks.filter.value')}
        value={
          typeof condition.value === 'string' &&
          dayjs(condition.value).isValid()
            ? dayjs(condition.value)
            : null
        }
        onChange={(value) => onChange(value?.toISOString())}
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
  fields,
  variables
}: {
  value: DataFilterGroup;
  onChange(value: DataFilterGroup): void;
  fields: readonly DataBlockField[];
  variables?: readonly DataFilterVariable[];
}) {
  const { token } = theme.useToken();
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
      style={{
        display: 'grid',
        gap: 12,
        minWidth: 260,
        borderInlineStart: `2px solid ${token.colorBorderSecondary}`,
        paddingInlineStart: 12
      }}
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
              border: `1px solid ${token.colorBorderSecondary}`,
              borderRadius: 4
            }}
          >
            <DataBlockFilterEditor
              value={condition}
              fields={fields}
              variables={variables}
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
            style={{
              display: 'flex',
              gap: 6,
              flexWrap: 'wrap',
              alignItems: 'center'
            }}
          >
            <Select
              aria-label={i18nText('frontstage', 'data_blocks.filter.field')}
              style={{ width: 140 }}
              showSearch
              optionFilterProp="label"
              placeholder={i18nText('frontstage', 'data_blocks.filter.field')}
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
                  value: undefined,
                  value_source: 'literal'
                })
              }
            />
            <Select
              aria-label={i18nText('frontstage', 'data_blocks.filter.operator')}
              style={{ width: 100 }}
              value={
                emptyCondition(condition)
                  ? condition.operator === '$eq'
                    ? 'empty'
                    : 'not_empty'
                  : condition.operator
              }
              options={[
                ...operators(
                  fields.find((field) => field.code === condition.field_code)
                ),
                {
                  value: 'empty',
                  label: i18nText('frontstage', 'data_blocks.filter.is_empty')
                },
                {
                  value: 'not_empty',
                  label: i18nText(
                    'frontstage',
                    'data_blocks.filter.is_not_empty'
                  )
                }
              ]}
              onChange={(operator) =>
                update(condition.id, {
                  ...condition,
                  operator:
                    operator === 'empty'
                      ? '$eq'
                      : operator === 'not_empty'
                        ? '$ne'
                        : (operator as DataFilterOperator),
                  value_source: 'literal',
                  value:
                    operator === 'empty' || operator === 'not_empty'
                      ? null
                      : undefined
                })
              }
            />
            {!emptyCondition(condition) && (
              <Space.Compact
                style={{ display: 'flex', flex: '1 1 160px', minWidth: 120 }}
              >
                {condition.value_source === 'variable' ? (
                  <Select
                    showSearch
                    optionFilterProp="label"
                    style={{ flex: 1, minWidth: 0 }}
                    aria-label={i18nText(
                      'frontstage',
                      'data_blocks.filter.variable'
                    )}
                    placeholder={i18nText(
                      'frontstage',
                      'data_blocks.filter.select_variable'
                    )}
                    value={
                      typeof condition.value === 'string'
                        ? condition.value
                        : undefined
                    }
                    options={
                      variables?.map((variable) => ({
                        value: variable.value,
                        label: variable.label
                      })) ?? []
                    }
                    onChange={(value) =>
                      update(condition.id, { ...condition, value })
                    }
                  />
                ) : (
                  <FilterValue
                    field={fields.find(
                      (field) => field.code === condition.field_code
                    )}
                    condition={condition}
                    onChange={(next) =>
                      update(condition.id, { ...condition, value: next })
                    }
                  />
                )}
                {variables && (
                  <Dropdown
                    trigger={['click']}
                    menu={{
                      selectedKeys: [
                        condition.value_source === 'variable'
                          ? String(condition.value)
                          : 'literal'
                      ],
                      items: [
                        {
                          key: 'literal',
                          label: i18nText(
                            'frontstage',
                            'data_blocks.filter.literal'
                          )
                        },
                        {
                          type: 'group',
                          label: i18nText(
                            'frontstage',
                            'data_blocks.filter.variable'
                          ),
                          children: variables.map((variable) => ({
                            key: variable.value,
                            label: variable.label
                          }))
                        }
                      ],
                      onClick: ({ key }) =>
                        update(condition.id, {
                          ...condition,
                          value_source:
                            key === 'literal' ? 'literal' : 'variable',
                          value: key === 'literal' ? undefined : key
                        })
                    }}
                  >
                    <Button
                      icon={<FunctionOutlined />}
                      aria-label={i18nText(
                        'frontstage',
                        'data_blocks.filter.value_source'
                      )}
                    />
                  </Dropdown>
                )}
              </Space.Compact>
            )}

            <Button
              type="text"
              size="small"
              icon={<CloseOutlined />}
              aria-label={i18nText('frontstage', 'data_blocks.common.remove')}
              onClick={() =>
                onChange({
                  ...value,
                  conditions: value.conditions.filter(
                    (item) => item.id !== condition.id
                  )
                })
              }
            />
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
                  field_code: '',
                  operator: '$eq',
                  value: undefined
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
