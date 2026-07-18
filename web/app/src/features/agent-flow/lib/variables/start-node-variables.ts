import type {
  FlowNodeDocument,
  FlowNodeOutputDocument,
  FlowStartInputField,
  FlowStartInputType
} from '@1flowbase/flow-schema';
import {
  getLlmNodeOutputs,
  isValidPublicOutputKey
} from '@1flowbase/flow-schema';

import { getBuiltinNodeRuntimeContract } from '../node-definitions/contracts';
import { normalizeCodeOutput } from '../output-contract/code-output';
import { getRegisteredNodeDefinition } from '../node-definitions/registry';
import { LLM_CONTEXT_MESSAGES_JSON_SCHEMA } from '../output-contract/schema';
import { i18nText } from '../../../../shared/i18n/text';

export const startInputTypeOptions = [
  {
    value: 'text',
    label: i18nText('agentFlow', 'auto.text'),
    valueType: 'string'
  },
  {
    value: 'paragraph',
    label: i18nText('agentFlow', 'auto.paragraph'),
    valueType: 'string'
  },
  {
    value: 'select',
    label: i18nText('agentFlow', 'auto.drop_down_options'),
    valueType: 'string'
  },
  {
    value: 'number',
    label: i18nText('agentFlow', 'auto.numbers'),
    valueType: 'number'
  },
  {
    value: 'checkbox',
    label: i18nText('agentFlow', 'auto.checkbox'),
    valueType: 'boolean'
  },
  {
    value: 'file',
    label: i18nText('agentFlow', 'auto.file'),
    valueType: 'json'
  },
  {
    value: 'file_list',
    label: i18nText('agentFlow', 'auto.file_list'),
    valueType: 'array[object]'
  },
  { value: 'url', label: 'URL', valueType: 'string' }
] satisfies Array<{
  value: FlowStartInputType;
  label: string;
  valueType: FlowNodeOutputDocument['valueType'];
}>;

export const startSystemVariables = [
  {
    key: 'query',
    title: 'userinput.query',
    valueType: 'string'
  },
  {
    key: 'system',
    title: 'userinput.system',
    valueType: 'string'
  },
  {
    key: 'model',
    title: 'userinput.model',
    valueType: 'string'
  },
  {
    key: 'reasoning_effort',
    title: 'userinput.reasoning_effort',
    valueType: 'string'
  },
  {
    key: 'max_output_tokens',
    title: 'userinput.max_output_tokens',
    valueType: 'number'
  },
  {
    key: 'history',
    title: 'userinput.history',
    valueType: 'array',
    jsonSchema: LLM_CONTEXT_MESSAGES_JSON_SCHEMA
  },
  {
    key: 'files',
    title: 'userinput.files',
    valueType: 'array[object]'
  },
  {
    key: 'tools',
    title: 'userinput.tools',
    valueType: 'array[object]'
  },
  {
    key: 'tool_choice',
    title: 'userinput.tool_choice',
    valueType: 'json'
  }
] satisfies FlowNodeOutputDocument[];

function isStartInputType(value: unknown): value is FlowStartInputType {
  return startInputTypeOptions.some((option) => option.value === value);
}

export function getStartInputValueType(inputType: FlowStartInputType) {
  return (
    startInputTypeOptions.find((option) => option.value === inputType)
      ?.valueType ?? 'string'
  );
}

function normalizeString(value: unknown, fallback: string) {
  return typeof value === 'string' && value.trim().length > 0
    ? value
    : fallback;
}

function normalizeOptionalString(value: unknown) {
  return typeof value === 'string' && value.trim().length > 0
    ? value
    : undefined;
}

function normalizeDefaultValue(value: unknown, inputType: FlowStartInputType) {
  switch (inputType) {
    case 'number':
      return typeof value === 'number' && Number.isFinite(value)
        ? value
        : undefined;
    case 'checkbox':
      return typeof value === 'boolean' ? value : undefined;
    case 'file':
    case 'file_list':
      return undefined;
    case 'text':
    case 'paragraph':
    case 'select':
    case 'url':
      return typeof value === 'string' && value.length > 0 ? value : undefined;
  }
}

function normalizeMaxLength(value: unknown) {
  return typeof value === 'number' && Number.isInteger(value) && value > 0
    ? value
    : undefined;
}

function normalizeOptions(value: unknown) {
  return Array.isArray(value)
    ? value
        .filter((option): option is string => typeof option === 'string')
        .map((option) => option.trim())
        .filter(Boolean)
    : undefined;
}

function normalizeStartInputSource(value: unknown) {
  return value === 'path' ||
    value === 'query' ||
    value === 'body' ||
    value === 'form'
    ? value
    : undefined;
}

export function normalizeStartInputField(
  value: unknown,
  index: number
): FlowStartInputField {
  const source =
    typeof value === 'object' && value !== null
      ? (value as Record<string, unknown>)
      : {};
  const inputType = isStartInputType(source.inputType)
    ? source.inputType
    : 'text';
  const key = normalizeString(source.key, `input_${index + 1}`);

  return {
    key,
    label: normalizeString(source.label, key),
    inputType,
    valueType: getStartInputValueType(inputType),
    required: Boolean(source.required),
    placeholder: normalizeOptionalString(source.placeholder),
    defaultValue: normalizeDefaultValue(source.defaultValue, inputType),
    maxLength: normalizeMaxLength(source.maxLength),
    hidden: Boolean(source.hidden),
    options: normalizeOptions(source.options),
    source: normalizeStartInputSource(source.source)
  };
}

export function getStartInputFields(
  node: Pick<FlowNodeDocument, 'config'> | null | undefined
) {
  const rawFields = node?.config.input_fields;

  return Array.isArray(rawFields)
    ? rawFields.map((field, index) => normalizeStartInputField(field, index))
    : [];
}

export function getStartNodeVariableOutputs(
  node: Pick<FlowNodeDocument, 'config' | 'outputs'>
): FlowNodeOutputDocument[] {
  if (node.outputs.length > 0) {
    throw new Error('Start node outputs must be empty');
  }

  const fields = getStartInputFields(node).map((field) => ({
    key: field.key,
    title: `userinput.${field.key}`,
    valueType: field.valueType
  }));
  const usedKeys = new Set(fields.map((field) => field.key));

  return [
    ...fields,
    ...startSystemVariables.filter((variable) => !usedKeys.has(variable.key))
  ];
}

export function getNodeVariableOutputs(
  node: FlowNodeDocument
): FlowNodeOutputDocument[] {
  const registeredVariableOutputs = getRegisteredNodeDefinition(
    node.type
  )?.variableOutputs;

  if (registeredVariableOutputs) {
    return registeredVariableOutputs(node);
  }

  if (node.type === 'start') {
    return getStartNodeVariableOutputs(node);
  }

  if (node.type === 'if_else') {
    return [];
  }

  if (node.type === 'llm') {
    return getLlmNodeOutputs(node.config);
  }

  if (node.type === 'code') {
    return node.outputs
      .filter((output) => isValidPublicOutputKey(output.key))
      .map(normalizeCodeOutput);
  }

  if (node.type === 'plugin_node') {
    return node.outputs.filter((output) => isValidPublicOutputKey(output.key));
  }

  if (node.type === 'variable_assigner') {
    return node.outputs.filter((output) => isValidPublicOutputKey(output.key));
  }

  const contract = getBuiltinNodeRuntimeContract(node.type);
  if (contract) {
    return contract.defaults.outputs.filter((output) =>
      isValidPublicOutputKey(output.key)
    );
  }

  return node.outputs.filter((output) => isValidPublicOutputKey(output.key));
}
