import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import { Button, Input, Select } from 'antd';
import type {
  NamedBindingEntry,
  NamedBindingExpression
} from '@1flowbase/flow-schema';

import type { FlowSelectorOption } from '../../lib/selector-options';
import {
  getNamedBindingExpression,
  isNamedBindingNameAllowed
} from '../../lib/named-binding-expressions';
import {
  createTemplateSelectorToken,
  parseTemplateSelectorTokens
} from '../../lib/template-binding';
import { useStableListItemKeys } from '../../hooks/interactions/use-stable-list-item-keys';
import { SelectorField } from './SelectorField';
import { TemplatedTextField } from './TemplatedTextField';
import { i18nText } from '../../../../shared/i18n/text';

export type TemplatedNamedBindingValue = NamedBindingEntry;

type ParameterValueType =
  | 'string'
  | 'number'
  | 'boolean'
  | 'object'
  | 'array'
  | 'json'
  | 'unknown';

interface TemplatedNamedBindingsFieldProps {
  ariaLabel: string;
  value: TemplatedNamedBindingValue[];
  options: FlowSelectorOption[];
  onChange: (value: TemplatedNamedBindingValue[]) => void;
}

const parameterValueTypeOptions: Array<{
  label: string;
  value: ParameterValueType;
}> = [
  { label: i18nText('agentFlow', 'auto.string'), value: 'string' },
  { label: i18nText('agentFlow', 'auto.number'), value: 'number' },
  { label: i18nText('agentFlow', 'auto.boolean'), value: 'boolean' },
  { label: i18nText('agentFlow', 'auto.object'), value: 'object' },
  { label: i18nText('agentFlow', 'auto.array'), value: 'array' },
  { label: 'JSON', value: 'json' },
  { label: i18nText('agentFlow', 'auto.unknown'), value: 'unknown' }
];

function isParameterNameValid(name: string) {
  return name.length === 0 || isNamedBindingNameAllowed(name);
}

function normalizeParameterValueType(value: string | undefined) {
  if (!value) {
    return undefined;
  }

  if (value.startsWith('array')) {
    return 'array' as const;
  }

  if (
    value === 'string' ||
    value === 'number' ||
    value === 'boolean' ||
    value === 'object' ||
    value === 'array' ||
    value === 'json' ||
    value === 'unknown'
  ) {
    return value;
  }

  return 'unknown' as const;
}

function findSelectorOption(options: FlowSelectorOption[], selector: string[]) {
  return options.find(
    (option) =>
      option.value.length === selector.length &&
      option.value.every((segment, index) => selector[index] === segment)
  );
}

function inferValueTypeFromExpression(
  expression: NamedBindingExpression | null,
  options: FlowSelectorOption[]
) {
  if (!expression) {
    return undefined;
  }

  if (expression.kind === 'selector') {
    return normalizeParameterValueType(
      findSelectorOption(options, expression.selector)?.valueType
    );
  }

  return undefined;
}

function defaultConstantValue(valueType: ParameterValueType | undefined) {
  switch (valueType) {
    case 'number':
      return 0;
    case 'boolean':
      return false;
    case 'string':
    case 'unknown':
    default:
      return '';
  }
}

function createConstantExpression(
  valueType: ParameterValueType | undefined
): NamedBindingExpression {
  return {
    kind: 'constant',
    value: defaultConstantValue(valueType)
  };
}

function isSelectorOnlyValueType(valueType: ParameterValueType | undefined) {
  return (
    valueType === 'array' || valueType === 'object' || valueType === 'json'
  );
}

function expressionToSingleLineText(expression: NamedBindingExpression | null) {
  if (expression?.kind === 'selector') {
    return createTemplateSelectorToken(expression.selector);
  }

  if (expression?.kind === 'templated_text') {
    return expression.value;
  }

  if (expression?.kind !== 'constant') {
    return '';
  }

  if (typeof expression.value === 'string') {
    return expression.value;
  }

  return String(expression.value ?? '');
}

function singleLineTextToExpression(
  nextValue: string,
  valueType: ParameterValueType | undefined
): NamedBindingExpression {
  const hasSelectorToken = parseTemplateSelectorTokens(nextValue).length > 0;

  if (valueType === 'number') {
    const parsed = Number(nextValue);

    return !hasSelectorToken &&
      nextValue.trim().length > 0 &&
      Number.isFinite(parsed)
      ? { kind: 'constant', value: parsed }
      : { kind: 'templated_text', value: nextValue };
  }

  return hasSelectorToken
    ? { kind: 'templated_text', value: nextValue }
    : { kind: 'constant', value: nextValue };
}

function exactSelectorFromSingleLineText(nextValue: string) {
  const trimmedValue = nextValue.trim();
  const selectors = parseTemplateSelectorTokens(trimmedValue);

  if (selectors.length !== 1) {
    return null;
  }

  const [selector] = selectors;

  return createTemplateSelectorToken(selector) === trimmedValue
    ? selector
    : null;
}

function optionMatchesValueType(
  option: FlowSelectorOption,
  valueType: ParameterValueType | undefined
) {
  if (!valueType) {
    return true;
  }

  const optionValueType = normalizeParameterValueType(option.valueType);

  if (!optionValueType) {
    return false;
  }

  if (optionValueType === 'json') {
    return (
      valueType === 'json' || valueType === 'object' || valueType === 'array'
    );
  }

  return optionValueType === valueType;
}

function selectorOptionsForValueType(
  options: FlowSelectorOption[],
  valueType: ParameterValueType | undefined
) {
  if (valueType === 'string' || !valueType) {
    return options;
  }

  if (valueType === 'number') {
    return options.filter((option) => optionMatchesValueType(option, 'number'));
  }

  return options.filter((option) => optionMatchesValueType(option, valueType));
}

function coerceExpressionForValueType(
  expression: NamedBindingExpression | null,
  valueType: ParameterValueType | undefined,
  options: FlowSelectorOption[]
): NamedBindingExpression {
  if (isSelectorOnlyValueType(valueType)) {
    if (expression?.kind === 'selector') {
      const matchedOption = findSelectorOption(options, expression.selector);

      if (matchedOption && optionMatchesValueType(matchedOption, valueType)) {
        return expression;
      }
    }

    return { kind: 'selector', selector: [] };
  }

  if (valueType === 'boolean') {
    if (expression?.kind === 'selector') {
      const matchedOption = findSelectorOption(options, expression.selector);

      if (matchedOption && optionMatchesValueType(matchedOption, 'boolean')) {
        return expression;
      }
    }

    if (
      expression?.kind === 'constant' &&
      typeof expression.value === 'boolean'
    ) {
      return expression;
    }

    return createConstantExpression(valueType);
  }

  if (expression?.kind === 'selector') {
    return {
      kind: 'templated_text',
      value: createTemplateSelectorToken(expression.selector)
    };
  }

  if (valueType === 'number' && expression) {
    return singleLineTextToExpression(
      expressionToSingleLineText(expression),
      valueType
    );
  }

  if (valueType === 'string' && expression?.kind === 'constant') {
    return {
      kind: 'constant',
      value:
        typeof expression.value === 'string'
          ? expression.value
          : expressionToSingleLineText(expression)
    };
  }

  if (expression) {
    return expression;
  }

  return createConstantExpression(valueType);
}

function singleLineTextToTypedEntryValue(
  nextValue: string,
  valueType: ParameterValueType | undefined,
  options: FlowSelectorOption[]
): Pick<TemplatedNamedBindingValue, 'valueType' | 'value'> {
  const exactSelector = exactSelectorFromSingleLineText(nextValue);
  const matchedOption = exactSelector
    ? findSelectorOption(options, exactSelector)
    : undefined;
  const nextValueType =
    valueType ??
    normalizeParameterValueType(matchedOption?.valueType) ??
    'string';

  if (
    exactSelector &&
    (isSelectorOnlyValueType(nextValueType) || nextValueType === 'boolean')
  ) {
    return {
      valueType: nextValueType,
      value: { kind: 'selector', selector: exactSelector }
    };
  }

  return {
    valueType: nextValueType,
    value: singleLineTextToExpression(nextValue, nextValueType)
  };
}

export function TemplatedNamedBindingsField({
  ariaLabel,
  value,
  options,
  onChange
}: TemplatedNamedBindingsFieldProps) {
  const { itemKeys, insertItemKey, removeItemKey } = useStableListItemKeys(
    'templated-named-binding',
    value.length
  );

  function updateEntry(index: number, nextEntry: TemplatedNamedBindingValue) {
    onChange(
      value.map((entry, entryIndex) =>
        entryIndex === index ? nextEntry : entry
      )
    );
  }

  function renderValueEditor(
    entry: TemplatedNamedBindingValue,
    index: number,
    expression: NamedBindingExpression | null,
    valueType: ParameterValueType | undefined,
    entryLabel: string,
    selectorOptions: FlowSelectorOption[]
  ) {
    if (valueType === 'boolean') {
      const booleanOptions = [
        { label: 'true', value: 'constant:true' },
        { label: 'false', value: 'constant:false' },
        ...selectorOptions.map((option) => ({
          label: option.displayLabel,
          value: `selector:${JSON.stringify(option.value)}`
        }))
      ];
      const booleanValue =
        expression?.kind === 'selector'
          ? `selector:${JSON.stringify(expression.selector)}`
          : expression?.kind === 'constant' && expression.value === true
            ? 'constant:true'
            : 'constant:false';

      return (
        <Select
          aria-label={`${ariaLabel}-${index}-value`}
          options={booleanOptions}
          value={booleanValue}
          onChange={(nextValue) => {
            if (nextValue.startsWith('selector:')) {
              const selector = JSON.parse(
                nextValue.slice('selector:'.length)
              ) as string[];

              updateEntry(index, {
                ...entry,
                valueType,
                value: { kind: 'selector', selector }
              });
              return;
            }

            updateEntry(index, {
              ...entry,
              valueType,
              value: { kind: 'constant', value: nextValue === 'constant:true' }
            });
          }}
        />
      );
    }

    if (isSelectorOnlyValueType(valueType)) {
      return (
        <SelectorField
          ariaLabel={`${ariaLabel}-${index}-value`}
          options={selectorOptions}
          value={expression?.kind === 'selector' ? expression.selector : []}
          onChange={(nextSelector) => {
            const selector = Array.isArray(nextSelector[0])
              ? []
              : (nextSelector as string[]);
            const matchedOption = findSelectorOption(options, selector);

            updateEntry(index, {
              ...entry,
              valueType:
                valueType ??
                normalizeParameterValueType(matchedOption?.valueType),
              value: { kind: 'selector', selector }
            });
          }}
        />
      );
    }

    return (
      <TemplatedTextField
        ariaLabel={`${ariaLabel}-${index}-value`}
        displayMode="input"
        label={entryLabel}
        options={selectorOptions}
        placeholder={i18nText(
          'agentFlow',
          'auto.enter_text_enter_reference_variable'
        )}
        value={expressionToSingleLineText(expression)}
        onChange={(nextValue) => {
          const typedEntryValue = singleLineTextToTypedEntryValue(
            nextValue,
            valueType,
            selectorOptions
          );

          updateEntry(index, {
            ...entry,
            ...typedEntryValue
          });
        }}
      />
    );
  }

  return (
    <div className="agent-flow-templated-binding-list">
      {value.map((entry, index) => {
        const entryLabel =
          entry.name ||
          i18nText('agentFlow', 'auto.variable', { value1: index + 1 });
        const expression = getNamedBindingExpression(entry);
        const valueType =
          normalizeParameterValueType(entry.valueType) ??
          inferValueTypeFromExpression(expression, options);
        const normalizedExpression = coerceExpressionForValueType(
          expression,
          valueType,
          options
        );
        const selectorOptions = selectorOptionsForValueType(options, valueType);

        return (
          <div
            key={itemKeys[index]}
            className="agent-flow-templated-binding-row"
          >
            <div className="agent-flow-templated-binding-row__name">
              <Input
                aria-label={`${ariaLabel}-${index}-name`}
                placeholder={i18nText('agentFlow', 'auto.variable_name')}
                status={isParameterNameValid(entry.name) ? undefined : 'error'}
                value={entry.name}
                onChange={(event) =>
                  updateEntry(index, { ...entry, name: event.target.value })
                }
              />
            </div>
            <div className="agent-flow-templated-binding-row__type">
              <Select
                allowClear
                aria-label={`${ariaLabel}-${index}-type`}
                options={parameterValueTypeOptions}
                placeholder={i18nText('agentFlow', 'auto.please_select_type')}
                value={valueType}
                onChange={(nextValue) => {
                  const nextValueType = normalizeParameterValueType(nextValue);

                  updateEntry(index, {
                    ...entry,
                    valueType: nextValueType,
                    value: coerceExpressionForValueType(
                      expression,
                      nextValueType,
                      options
                    )
                  });
                }}
              />
            </div>
            <div className="agent-flow-templated-binding-row__value">
              {renderValueEditor(
                entry,
                index,
                normalizedExpression,
                valueType,
                entryLabel,
                selectorOptions
              )}
            </div>
            <Button
              aria-label={i18nText('agentFlow', 'auto.delete_variable', {
                value1: entry.name || index + 1
              })}
              className="agent-flow-templated-binding-row__delete"
              danger
              icon={<DeleteOutlined />}
              size="small"
              type="text"
              onClick={() => {
                removeItemKey(index);
                onChange(value.filter((_, itemIndex) => itemIndex !== index));
              }}
            />
          </div>
        );
      })}
      <Button
        type="dashed"
        onClick={() => {
          insertItemKey(value.length);
          onChange([
            ...value,
            {
              name: '',
              value: { kind: 'constant', value: '' }
            }
          ]);
        }}
      >
        {i18nText('agentFlow', 'auto.add_new_variable')}
      </Button>
    </div>
  );
}
