import type {
  FlowAuthoringDocument,
  FlowNodeDocument,
  FlowNodeType
} from '@1flowbase/flow-schema';

import {
  createPluginNodeOutputs,
  getNodePickerOptionNodeType,
  toPluginContributionRef,
  type NodePickerOption
} from '../plugin-node-definitions';
import { getBuiltinNodeRuntimeContract } from '../node-definitions/contracts';

type NodeFactoryInput = FlowNodeType | NodePickerOption;

function isNodePickerOption(
  value: NodeFactoryInput
): value is NodePickerOption {
  return typeof value === 'object' && value !== null && 'kind' in value;
}

function createNodeDocumentFromRuntimeContract(
  nodeType: FlowNodeType,
  id: string,
  x: number,
  y: number
): FlowNodeDocument | null {
  const contract = getBuiltinNodeRuntimeContract(nodeType);

  if (!contract) {
    return null;
  }

  return {
    id,
    type: contract.meta.type,
    alias: contract.defaults.alias,
    description: contract.defaults.description ?? '',
    containerId: null,
    position: { x, y },
    configVersion: contract.defaults.configVersion,
    config: contract.defaults.config,
    bindings: contract.defaults.bindings,
    outputs: contract.defaults.outputs
  };
}

function isSameNodeAliasFamily(
  existingNode: FlowNodeDocument,
  nextNode: FlowNodeDocument
) {
  if (nextNode.type !== 'plugin_node') {
    return existingNode.type === nextNode.type;
  }

  return (
    existingNode.type === 'plugin_node' &&
    existingNode.plugin_unique_identifier ===
      nextNode.plugin_unique_identifier &&
    existingNode.contribution_code === nextNode.contribution_code
  );
}

function createCountedNodeAlias(
  document: FlowAuthoringDocument,
  nextNode: FlowNodeDocument
) {
  const sameFamilyNodeCount = document.graph.nodes.filter((node) =>
    isSameNodeAliasFamily(node, nextNode)
  ).length;

  return sameFamilyNodeCount > 0
    ? `${nextNode.alias}${sameFamilyNodeCount}`
    : nextNode.alias;
}

export function createNodeDocument(
  nodeTypeOrOption: NodeFactoryInput,
  id: string,
  x = 0,
  y = 0
): FlowNodeDocument {
  if (isNodePickerOption(nodeTypeOrOption)) {
    if (nodeTypeOrOption.disabled) {
      throw new Error(
        `Catalog node is unavailable: ${nodeTypeOrOption.disabledReason ?? nodeTypeOrOption.label}`
      );
    }

    if (nodeTypeOrOption.kind === 'plugin_contribution') {
      return {
        id,
        type: 'plugin_node',
        alias: nodeTypeOrOption.label,
        description: nodeTypeOrOption.plugin.description,
        containerId: null,
        position: { x, y },
        configVersion: 1,
        config: {},
        bindings: {},
        outputs: createPluginNodeOutputs(nodeTypeOrOption.plugin),
        ...toPluginContributionRef(nodeTypeOrOption.plugin)
      };
    }

    return createNodeDocument(nodeTypeOrOption.type, id, x, y);
  }

  const contractNode = createNodeDocumentFromRuntimeContract(
    nodeTypeOrOption,
    id,
    x,
    y
  );

  if (contractNode) {
    return contractNode;
  }

  throw new Error(
    `Missing runtime contract for node type: ${nodeTypeOrOption}`
  );
}

export function createNodeDocumentWithCountedAlias(
  document: FlowAuthoringDocument,
  nodeTypeOrOption: NodeFactoryInput,
  id: string,
  x = 0,
  y = 0
): FlowNodeDocument {
  const node = createNodeDocument(nodeTypeOrOption, id, x, y);

  return {
    ...node,
    alias: createCountedNodeAlias(document, node)
  };
}

export function createNextNodeId(
  documentOrIds: FlowAuthoringDocument | string[],
  nodeTypeOrOption: NodeFactoryInput
) {
  const ids = Array.isArray(documentOrIds)
    ? documentOrIds
    : documentOrIds.graph.nodes.map((node) => node.id);
  const nodeType = isNodePickerOption(nodeTypeOrOption)
    ? getNodePickerOptionNodeType(nodeTypeOrOption)
    : nodeTypeOrOption;
  const prefixSeed =
    isNodePickerOption(nodeTypeOrOption) &&
    nodeTypeOrOption.kind === 'plugin_contribution'
      ? nodeTypeOrOption.plugin.contribution_code
      : nodeType;
  const prefix = `node-${prefixSeed.replaceAll('_', '-')}`;
  let nextIndex = 1;

  while (ids.includes(`${prefix}-${nextIndex}`)) {
    nextIndex += 1;
  }

  return `${prefix}-${nextIndex}`;
}
