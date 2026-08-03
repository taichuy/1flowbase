import type {
  ConsoleApplicationNodeCatalogEntry,
  ConsoleApplicationNodeFieldContract
} from '@1flowbase/api-client';
import type { BuiltinFlowNodeType, FlowNodeType } from '@1flowbase/flow-schema';

import { getNodeRuntimeContract } from './runtime-contract-registry';

export interface BuiltinNodePickerOption {
  kind: 'builtin';
  type: BuiltinFlowNodeType;
  label: string;
  category: ConsoleApplicationNodeCatalogEntry['category'];
  field_contract: ConsoleApplicationNodeFieldContract;
  disabled: boolean;
}

function getBuiltinNodeType(nodeType: string): BuiltinFlowNodeType {
  const contract = getNodeRuntimeContract(nodeType as FlowNodeType);

  if (
    !contract ||
    contract.meta.type === 'plugin_node' ||
    contract.meta.type === 'unresolved_node'
  ) {
    throw new Error(`Missing built-in renderer for catalog node: ${nodeType}`);
  }

  return contract.meta.type;
}

export function toBuiltinNodePickerOption(
  node: ConsoleApplicationNodeCatalogEntry
): BuiltinNodePickerOption {
  if (node.source_kind !== 'builtin') {
    throw new Error(`Expected built-in catalog node: ${node.node_type}`);
  }

  return {
    kind: 'builtin',
    type: getBuiltinNodeType(node.node_type),
    label: node.title,
    category: node.category,
    field_contract: node.field_contract,
    disabled: node.runtime_status === 'unavailable'
  };
}
