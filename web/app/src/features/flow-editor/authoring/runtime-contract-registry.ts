import type {
  FlowNodeType,
  NodeRuntimeUiContract
} from '@1flowbase/flow-schema';

import { getRegisteredNodeDefinition } from './node-definition-registry';

const runtimeContracts = new Map<FlowNodeType, NodeRuntimeUiContract>();

function cloneContract(contract: NodeRuntimeUiContract): NodeRuntimeUiContract {
  return structuredClone(contract);
}

export function registerNodeRuntimeContract(contract: NodeRuntimeUiContract) {
  runtimeContracts.set(contract.meta.type, cloneContract(contract));
}

export function registerNodeRuntimeContracts(
  contracts: Iterable<NodeRuntimeUiContract>
) {
  for (const contract of contracts) {
    registerNodeRuntimeContract(contract);
  }
}

export function getNodeRuntimeContract(
  nodeType: FlowNodeType
): NodeRuntimeUiContract | null {
  const contract =
    runtimeContracts.get(nodeType) ??
    getRegisteredNodeDefinition(nodeType)?.contract;
  return contract ? cloneContract(contract) : null;
}
