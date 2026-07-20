import type {
  FlowAuthoringDocument,
  FlowNodeOutputDocument
} from '@1flowbase/flow-schema';

import { getRegisteredNodeDefinition } from './node-definition-registry';
import { getNodeRuntimeContract } from './runtime-contract-registry';

export interface AuthoringVariableSource {
  id: string;
  label: string;
  outputs: FlowNodeOutputDocument[];
}

export interface AuthoringVariableOption {
  nodeId: string;
  nodeLabel: string;
  outputKey: string;
  outputLabel: string;
  valueType: string;
  value: string[];
  displayLabel: string;
}

function collectUpstreamNodeIds(
  document: FlowAuthoringDocument,
  nodeId: string
) {
  const visited = new Set<string>();
  const queue = [nodeId];

  while (queue.length > 0) {
    const target = queue.shift();
    if (!target) continue;

    for (const edge of document.graph.edges) {
      if (edge.target === target && !visited.has(edge.source)) {
        visited.add(edge.source);
        queue.push(edge.source);
      }
    }
  }

  return visited;
}

function getVariableOutputs(
  node: FlowAuthoringDocument['graph']['nodes'][number]
) {
  const registered = getRegisteredNodeDefinition(node.type)?.variableOutputs;
  if (registered) return registered(node);
  if (node.outputs.length > 0) return node.outputs;
  return getNodeRuntimeContract(node.type)?.defaults.outputs ?? [];
}

export function listAuthoringVariableOptions(
  document: FlowAuthoringDocument,
  nodeId: string,
  sources: readonly AuthoringVariableSource[]
): AuthoringVariableOption[] {
  const sourceOptions = sources.flatMap((source) =>
    source.outputs.map((output) => ({
      nodeId: source.id,
      nodeLabel: source.label,
      outputKey: output.key,
      outputLabel: output.title,
      valueType: output.valueType,
      value: [source.id, output.key],
      displayLabel: output.title
    }))
  );
  const upstream = collectUpstreamNodeIds(document, nodeId);
  const nodeOptions = document.graph.nodes
    .filter((node) => upstream.has(node.id))
    .flatMap((node) =>
      getVariableOutputs(node).map((output) => ({
        nodeId: node.id,
        nodeLabel: node.alias,
        outputKey: output.key,
        outputLabel: output.title,
        valueType: output.valueType,
        value: [node.id, output.key],
        displayLabel: `${node.alias}.${output.title}`
      }))
    );

  return [...sourceOptions, ...nodeOptions];
}
