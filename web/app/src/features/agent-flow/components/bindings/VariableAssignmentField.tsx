import { Button, Input, InputNumber, Select, Switch } from 'antd';
import { useStableListItemKeys } from '../../hooks/interactions/use-stable-list-item-keys';

import type { FlowSelectorOption } from '../../lib/selector-options';
import type { AgentFlowConversationVariable } from '../../lib/variables/conversation-variables';
import {
  conversationVariableNodeId,
  formatConversationVariableTitle
} from '../../lib/variables/conversation-variables';
import { createTemplateSelectorToken } from '../../lib/template-binding';
import { TemplatedTextField } from './TemplatedTextField';
import { i18nText } from '../../../../shared/i18n/text';

export type VariableAssignmentExpression =
  | { kind: 'selector'; selector: string[] }
  | { kind: 'constant'; value: unknown }
  | { kind: 'templated_text'; value: string };

export interface VariableAssignmentValue {
  path: string[];
  operator: 'set' | 'append' | 'clear' | 'increment';
  source?: string[] | null;
  value?: VariableAssignmentExpression | null;
}

interface VariableAssignmentFieldProps {
  ariaLabel: string;
  value: VariableAssignmentValue[];
  conversationVariables: AgentFlowConversationVariable[];
  selectorOptions: FlowSelectorOption[];
  onChange: (value: VariableAssignmentValue[]) => void;
}

function getTargetName(entry: VariableAssignmentValue) {
  return entry.path[0] === conversationVariableNodeId
    ? (entry.path[1] ?? '')
    : '';
}

function findConversationVariable(
  conversationVariables: AgentFlowConversationVariable[],
  entry: VariableAssignmentValue
) {
  const targetName = getTargetName(entry);

  return conversationVariables.find((variable) => variable.name === targetName);
}

function isNumberValueType(valueType: string) {
  return valueType === 'number';
}

function isBooleanValueType(valueType: string) {
  return valueType === 'boolean';
}

function isStringValueType(valueType: string) {
  return valueType === 'string';
}

function createDefaultValueExpression(
  variable: AgentFlowConversationVariable | undefined
): VariableAssignmentExpression | null {
  if (!variable) {
    return null;
  }

  if (isStringValueType(variable.valueType)) {
    return { kind: 'templated_text', value: '' };
  }

  if (isNumberValueType(variable.valueType)) {
    return { kind: 'constant', value: 0 };
  }

  if (isBooleanValueType(variable.valueType)) {
    return { kind: 'constant', value: false };
  }

  return null;
}

function getTemplatedTextValue(entry: VariableAssignmentValue) {
  if (entry.value?.kind === 'templated_text') {
    return entry.value.value;
  }

  if (entry.value?.kind === 'constant') {
    return entry.value.value === null || entry.value.value === undefined
      ? ''
      : String(entry.value.value);
  }

  if (entry.value?.kind === 'selector') {
    return createTemplateSelectorToken(entry.value.selector);
  }

  return entry.source ? createTemplateSelectorToken(entry.source) : '';
}

function getNumberValue(entry: VariableAssignmentValue) {
  return entry.value?.kind === 'constant' &&
    typeof entry.value.value === 'number'
    ? entry.value.value
    : null;
}

function getBooleanValue(entry: VariableAssignmentValue) {
  return entry.value?.kind === 'constant' &&
    typeof entry.value.value === 'boolean'
    ? entry.value.value
    : false;
}

export function VariableAssignmentField({
  ariaLabel,
  value,
  conversationVariables,
  selectorOptions,
  onChange
}: VariableAssignmentFieldProps) {
  const rowKeys = useStableListItemKeys('variable-assignment', value.length);
  const targetOptions = conversationVariables.map((variable) => ({
    label: formatConversationVariableTitle(variable.name),
    value: variable.name
  }));

  function updateEntry(
    index: number,
    updater: (entry: VariableAssignmentValue) => VariableAssignmentValue
  ) {
    onChange(
      value.map((item, itemIndex) =>
        itemIndex === index ? updater(item) : item
      )
    );
  }

  function renderValueEditor(entry: VariableAssignmentValue, index: number) {
    const variable = findConversationVariable(conversationVariables, entry);

    if (!variable) {
      return (
        <Input
          aria-label={`${ariaLabel}-${index}-value`}
          disabled
          value=""
          placeholder={i18nText(
            'agentFlow',
            'auto.select_conversation_variable'
          )}
        />
      );
    }

    if (isStringValueType(variable.valueType)) {
      return (
        <TemplatedTextField
          ariaLabel={`${ariaLabel}-${index}-value`}
          displayMode="input"
          label={formatConversationVariableTitle(variable.name)}
          options={selectorOptions}
          placeholder={i18nText(
            'agentFlow',
            'auto.support_text_variable_block_enter_left_curly_bracket_quick_reference'
          )}
          value={getTemplatedTextValue(entry)}
          onChange={(nextValue) =>
            updateEntry(index, (item) => ({
              ...item,
              operator: 'set',
              source: null,
              value: { kind: 'templated_text', value: nextValue }
            }))
          }
        />
      );
    }

    if (isNumberValueType(variable.valueType)) {
      return (
        <InputNumber
          aria-label={`${ariaLabel}-${index}-value`}
          value={getNumberValue(entry)}
          onChange={(nextValue) =>
            updateEntry(index, (item) => ({
              ...item,
              operator: 'set',
              source: null,
              value: { kind: 'constant', value: nextValue ?? 0 }
            }))
          }
        />
      );
    }

    if (isBooleanValueType(variable.valueType)) {
      return (
        <Switch
          aria-label={`${ariaLabel}-${index}-value`}
          checked={getBooleanValue(entry)}
          onChange={(checked) =>
            updateEntry(index, (item) => ({
              ...item,
              operator: 'set',
              source: null,
              value: { kind: 'constant', value: checked }
            }))
          }
        />
      );
    }

    return (
      <Input
        aria-label={`${ariaLabel}-${index}-value`}
        disabled
        value=""
        placeholder={i18nText(
          'agentFlow',
          'auto.conversation_variable_assignment_type_not_available'
        )}
      />
    );
  }

  return (
    <div className="agent-flow-binding-list">
      {value.map((entry, index) => (
        <div
          key={rowKeys.itemKeys[index]}
          className="agent-flow-variable-assignment-row"
        >
          <Select
            aria-label={`${ariaLabel}-${index}-target`}
            options={targetOptions}
            placeholder={i18nText(
              'agentFlow',
              'auto.select_conversation_variable'
            )}
            value={getTargetName(entry) || undefined}
            onChange={(targetName) =>
              updateEntry(index, (item) => {
                const variable = conversationVariables.find(
                  (candidate) => candidate.name === targetName
                );

                return {
                  ...item,
                  path: [conversationVariableNodeId, targetName],
                  operator: 'set',
                  source: null,
                  value: createDefaultValueExpression(variable)
                };
              })
            }
          />
          <Select
            aria-label={`${ariaLabel}-${index}-operator`}
            disabled
            options={[
              { label: i18nText('agentFlow', 'auto.overwrite'), value: 'set' }
            ]}
            value="set"
          />
          {renderValueEditor(entry, index)}
          <Button
            danger
            type="text"
            onClick={() => {
              rowKeys.removeItemKey(index);
              onChange(value.filter((_, itemIndex) => itemIndex !== index));
            }}
          >
            {i18nText('agentFlow', 'auto.delete')}
          </Button>
        </div>
      ))}
      <Button
        type="dashed"
        onClick={() => {
          rowKeys.insertItemKey();
          onChange([
            ...value,
            {
              path: [conversationVariableNodeId, ''],
              operator: 'set',
              source: null,
              value: null
            }
          ]);
        }}
      >
        {i18nText('agentFlow', 'auto.add_variable_assignment')}
      </Button>
    </div>
  );
}
