import type {
  FlowNodeDocument,
  FlowNodeOutputDocument,
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
import {
  getStartInputFields,
  startInputTypes
} from '../../../flow-editor/authoring/start-input-fields';

export {
  getStartInputFields,
  getStartInputValueType,
  normalizeStartInputField
} from '../../../flow-editor/authoring/start-input-fields';

const startInputTypeLabels: Record<FlowStartInputType, string> = {
  text: i18nText('agentFlow', 'auto.text'),
  paragraph: i18nText('agentFlow', 'auto.paragraph'),
  select: i18nText('agentFlow', 'auto.drop_down_options'),
  number: i18nText('agentFlow', 'auto.numbers'),
  checkbox: i18nText('agentFlow', 'auto.checkbox'),
  file: i18nText('agentFlow', 'auto.file'),
  file_list: i18nText('agentFlow', 'auto.file_list'),
  url: 'URL'
};

export const startInputTypeOptions = startInputTypes.map((option) => ({
  ...option,
  label: startInputTypeLabels[option.value]
})) satisfies Array<{
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
  },
  {
    key: 'protocol_context',
    title: 'userinput.protocol_context',
    valueType: 'protocol_context'
  }
] satisfies FlowNodeOutputDocument[];

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
