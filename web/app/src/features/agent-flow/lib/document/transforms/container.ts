import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import { getNodeById } from '../selectors';

export function getContainerPathForNode(
  document: FlowAuthoringDocument,
  nodeId: string | null
) {
  if (!nodeId) {
    return [];
  }

  const path: string[] = [];
  let currentNode = getNodeById(document, nodeId);

  while (currentNode?.containerId) {
    path.unshift(currentNode.containerId);
    currentNode = getNodeById(document, currentNode.containerId);
  }

  return path;
}

export function collectContainerSubgraphNodeIds(
  document: FlowAuthoringDocument,
  rootNodeId: string
) {
  const childrenByContainer = new Map<string, string[]>();
  for (const node of document.graph.nodes) {
    if (!node.containerId) continue;
    const children = childrenByContainer.get(node.containerId) ?? [];
    children.push(node.id);
    childrenByContainer.set(node.containerId, children);
  }

  const queue = [rootNodeId];
  const collectedIds: string[] = [];
  const visited = new Set<string>();
  for (let cursor = 0; cursor < queue.length; cursor += 1) {
    const currentNodeId = queue[cursor];
    if (!currentNodeId || visited.has(currentNodeId)) continue;
    visited.add(currentNodeId);
    collectedIds.push(currentNodeId);
    for (const childId of childrenByContainer.get(currentNodeId) ?? []) {
      queue.push(childId);
    }
  }
  return collectedIds;
}
