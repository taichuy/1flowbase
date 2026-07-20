import type { BuiltinFlowNodeType } from '@1flowbase/flow-schema';

import { getNodeRuntimeContract } from './runtime-contract-registry';

export interface BuiltinNodePickerOption {
  kind: 'builtin';
  type: BuiltinFlowNodeType;
  label: string;
  description: string;
  category: string | null;
  inputKeys: string[];
  outputKeys: string[];
}

// Product assemblies add only their boundary nodes around this single set.
export const SHARED_EXECUTION_NODE_PICKER_TYPES = [
  'llm',
  'if_else',
  'code',
  'template_transform',
  'http_request',
  'tool_result',
  'data_model_list',
  'data_model_get',
  'data_model_create',
  'data_model_update',
  'data_model_delete',
  'variable_assigner'
] as const satisfies readonly BuiltinFlowNodeType[];

export function buildBuiltinNodePickerOptions(
  nodeTypes: readonly BuiltinFlowNodeType[]
): BuiltinNodePickerOption[] {
  return nodeTypes.map((nodeType) => {
    const contract = getNodeRuntimeContract(nodeType);

    if (!contract) {
      throw new Error(`Missing runtime contract for node picker: ${nodeType}`);
    }

    return {
      kind: 'builtin',
      type: nodeType,
      label: contract.meta.title,
      description:
        contract.defaults.description ?? contract.card.description ?? '',
      category: contract.card.category ?? null,
      inputKeys: contract.ports.inputs.map((port) => port.key),
      outputKeys: contract.ports.outputs.map((port) => port.key)
    };
  });
}
