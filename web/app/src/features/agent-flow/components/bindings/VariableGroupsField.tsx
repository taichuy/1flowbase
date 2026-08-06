import {
  ArrowDownOutlined,
  ArrowUpOutlined,
  DeleteOutlined,
  PlusOutlined
} from '@ant-design/icons';
import type {
  FlowVariableGroupDocument,
  FlowVariableGroupValueType
} from '@1flowbase/flow-schema';
import { Button, Card, Flex, Select, Space, Typography } from 'antd';

import type { FlowSelectorOption } from '../../lib/selector-options';
import { i18nText } from '../../../../shared/i18n/text';
import { SelectorField } from './SelectorField';

interface VariableGroupsFieldProps {
  ariaLabel: string;
  value: FlowVariableGroupDocument[];
  options: FlowSelectorOption[];
  onChange: (value: FlowVariableGroupDocument[]) => void;
}

const VALUE_TYPE_OPTIONS: Array<{
  label: string;
  value: FlowVariableGroupValueType;
}> = [
  { label: i18nText('agentFlow', 'auto.string'), value: 'string' },
  { label: i18nText('agentFlow', 'auto.number'), value: 'number' },
  { label: i18nText('agentFlow', 'auto.boolean'), value: 'boolean' },
  { label: i18nText('agentFlow', 'auto.object'), value: 'object' },
  { label: i18nText('agentFlow', 'auto.array'), value: 'array' }
];

function selectorEquals(left: string[], right: string[]) {
  return (
    left.length === right.length &&
    left.every((segment, index) => segment === right[index])
  );
}

function declaredSelectorType(valueType: string) {
  return valueType.startsWith('array[') ? 'array' : valueType;
}

function compatibleOptions(
  options: FlowSelectorOption[],
  valueType: FlowVariableGroupValueType
) {
  return options.filter(
    (option) => declaredSelectorType(option.valueType) === valueType
  );
}

function nextGroupKey(groups: FlowVariableGroupDocument[]) {
  const maxSuffix = groups.reduce((currentMax, group) => {
    const match = /^group(\d+)$/.exec(group.key);
    const suffix = match ? Number(match[1]) : 0;

    return Math.max(currentMax, suffix);
  }, 0);

  return `group${maxSuffix + 1}`;
}

function replaceGroup(
  groups: FlowVariableGroupDocument[],
  index: number,
  group: FlowVariableGroupDocument
) {
  return groups.map((candidate, candidateIndex) =>
    candidateIndex === index ? group : candidate
  );
}

function moveCandidate(candidates: string[][], index: number, offset: number) {
  const targetIndex = index + offset;

  if (targetIndex < 0 || targetIndex >= candidates.length) {
    return candidates;
  }

  const nextCandidates = [...candidates];
  [nextCandidates[index], nextCandidates[targetIndex]] = [
    nextCandidates[targetIndex] ?? [],
    nextCandidates[index] ?? []
  ];

  return nextCandidates;
}

export function VariableGroupsField({
  ariaLabel,
  value,
  options,
  onChange
}: VariableGroupsFieldProps) {
  function updateGroup(index: number, group: FlowVariableGroupDocument) {
    onChange(replaceGroup(value, index, group));
  }

  return (
    <Flex vertical gap="small" data-testid="variable-groups-field">
      {value.map((group, groupIndex) => {
        const selectorOptions = compatibleOptions(options, group.valueType);

        return (
          <Card
            key={group.key}
            size="small"
            title={<Typography.Text>{group.key}</Typography.Text>}
            extra={
              <Space size="small">
                <Select
                  aria-label={`${ariaLabel}-${group.key}-${i18nText('agentFlow', 'auto.type')}`}
                  options={VALUE_TYPE_OPTIONS}
                  value={group.valueType}
                  onChange={(valueType) =>
                    updateGroup(groupIndex, { ...group, valueType })
                  }
                />
                <Button
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.delete_variable_group',
                    {
                      value1: group.key
                    }
                  )}
                  danger
                  disabled={value.length === 1}
                  icon={<DeleteOutlined />}
                  type="text"
                  onClick={() =>
                    onChange(value.filter((_, index) => index !== groupIndex))
                  }
                />
              </Space>
            }
          >
            <Flex vertical gap="small">
              {group.candidates.map((selector, candidateIndex) => {
                const selectedOption = options.find((option) =>
                  selectorEquals(option.value, selector)
                );
                const incompatible =
                  selector.length > 0 &&
                  (!selectedOption ||
                    declaredSelectorType(selectedOption.valueType) !==
                      group.valueType);

                return (
                  <Flex key={`${group.key}-${candidateIndex}`} vertical gap={4}>
                    <Space.Compact block>
                      <SelectorField
                        ariaLabel={`${ariaLabel}-${group.key}-${candidateIndex + 1}`}
                        options={selectorOptions}
                        value={selector}
                        onChange={(nextSelector) =>
                          updateGroup(groupIndex, {
                            ...group,
                            candidates: group.candidates.map(
                              (candidate, index) =>
                                index === candidateIndex
                                  ? (nextSelector as string[])
                                  : candidate
                            )
                          })
                        }
                      />
                      <Button
                        aria-label={i18nText(
                          'agentFlow',
                          'auto.move_candidate_up'
                        )}
                        disabled={candidateIndex === 0}
                        icon={<ArrowUpOutlined />}
                        onClick={() =>
                          updateGroup(groupIndex, {
                            ...group,
                            candidates: moveCandidate(
                              group.candidates,
                              candidateIndex,
                              -1
                            )
                          })
                        }
                      />
                      <Button
                        aria-label={i18nText(
                          'agentFlow',
                          'auto.move_candidate_down'
                        )}
                        disabled={
                          candidateIndex === group.candidates.length - 1
                        }
                        icon={<ArrowDownOutlined />}
                        onClick={() =>
                          updateGroup(groupIndex, {
                            ...group,
                            candidates: moveCandidate(
                              group.candidates,
                              candidateIndex,
                              1
                            )
                          })
                        }
                      />
                      <Button
                        aria-label={i18nText(
                          'agentFlow',
                          'auto.delete_candidate'
                        )}
                        danger
                        disabled={group.candidates.length === 1}
                        icon={<DeleteOutlined />}
                        onClick={() =>
                          updateGroup(groupIndex, {
                            ...group,
                            candidates: group.candidates.filter(
                              (_, index) => index !== candidateIndex
                            )
                          })
                        }
                      />
                    </Space.Compact>
                    {incompatible ? (
                      <Typography.Text type="danger">
                        {i18nText(
                          'agentFlow',
                          'auto.variable_group_candidate_type_mismatch',
                          { value1: group.valueType }
                        )}
                      </Typography.Text>
                    ) : null}
                  </Flex>
                );
              })}
              <Button
                block
                icon={<PlusOutlined />}
                type="dashed"
                onClick={() =>
                  updateGroup(groupIndex, {
                    ...group,
                    candidates: [...group.candidates, []]
                  })
                }
              >
                {i18nText('agentFlow', 'auto.add_candidate')}
              </Button>
            </Flex>
          </Card>
        );
      })}
      <Button
        block
        icon={<PlusOutlined />}
        type="dashed"
        onClick={() =>
          onChange([
            ...value,
            {
              key: nextGroupKey(value),
              valueType: 'string',
              candidates: [[]]
            }
          ])
        }
      >
        {i18nText('agentFlow', 'auto.add_variable_group')}
      </Button>
    </Flex>
  );
}
