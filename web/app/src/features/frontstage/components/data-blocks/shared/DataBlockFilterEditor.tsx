import { Button, Select, Space, theme } from 'antd';
import type { DataFilterVariable } from '../../../lib/data-blocks/filter-variables';
import { i18nText } from '../../../../../shared/i18n/text';
import { FilterConditionRow } from './filter-editor/FilterConditionRow';
import { isDataFilterGroup } from '../../../lib/data-blocks/filters';
import type {
  DataBlockField,
  DataFilterCondition,
  DataFilterGroup
} from '../../../lib/data-blocks/types';

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
          <FilterConditionRow
            key={condition.id}
            condition={condition}
            fields={fields}
            variables={variables}
            onChange={(next) => update(condition.id, next)}
            onRemove={() =>
              onChange({
                ...value,
                conditions: value.conditions.filter(
                  (item) => item.id !== condition.id
                )
              })
            }
          />
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
