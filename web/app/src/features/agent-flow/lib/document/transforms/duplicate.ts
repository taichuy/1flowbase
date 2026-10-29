import { fitContainerBounds } from './container-geometry';
import { collectContainerSubgraphNodeIds } from './container';
import type {
  FlowAuthoringDocument,
  FlowNodeDocument
} from '@1flowbase/flow-schema';

import { getNodeById } from '../selectors';
import { i18nText } from '../../../../../shared/i18n/text';
import { transformNodeSelectorReferences } from './selector-references';

function availableCopyId(existingIds: ReadonlySet<string>, sourceId: string) {
  let nextId = `${sourceId}-copy`;
  let index = 2;

  while (existingIds.has(nextId)) {
    nextId = `${sourceId}-copy-${index}`;
    index += 1;
  }

  return nextId;
}

export function getDuplicatedNodeId(
  existingIds: string[],
  sourceNodeId: string
) {
  return availableCopyId(new Set(existingIds), sourceNodeId);
}

function remapSelector(selector: string[], idMap: Map<string, string>) {
  if (selector.length === 0 || !idMap.has(selector[0])) {
    return selector;
  }

  return [idMap.get(selector[0])!, ...selector.slice(1)];
}

function getNodeIdsToDuplicate(
  document: FlowAuthoringDocument,
  sourceNode: FlowNodeDocument
) {
  return sourceNode.type === 'iteration' || sourceNode.type === 'loop'
    ? collectContainerSubgraphNodeIds(document, sourceNode.id)
    : [sourceNode.id];
}

export function duplicateNodeSubgraph(
  document: FlowAuthoringDocument,
  payload: { nodeId: string }
) {
  const sourceNode = getNodeById(document, payload.nodeId);

  if (!sourceNode || sourceNode.type === 'container_start') {
    return document;
  }

  const sourceIds = getNodeIdsToDuplicate(document, sourceNode);
  const sourceIdSet = new Set(sourceIds);
  const existingNodeIds = new Set(document.graph.nodes.map((node) => node.id));
  const idMap = new Map<string, string>();
  for (const sourceId of sourceIds) {
    const copiedId = availableCopyId(existingNodeIds, sourceId);
    idMap.set(sourceId, copiedId);
    existingNodeIds.add(copiedId);
  }
  const duplicatedNodes = document.graph.nodes
    .filter((node) => sourceIdSet.has(node.id))
    .map((node) =>
      transformNodeSelectorReferences(
        {
          ...node,
          id: idMap.get(node.id)!,
          alias: i18nText('agentFlow', 'auto.copy_option', {
            value1: node.alias
          }),
          containerId:
            node.containerId && idMap.has(node.containerId)
              ? idMap.get(node.containerId)!
              : node.containerId,
          position:
            node.id === sourceNode.id
              ? { x: node.position.x + 48, y: node.position.y + 48 }
              : { ...node.position }
        },
        (selector) => remapSelector(selector, idMap)
      )
    );
  const duplicatedEdgeIds = new Set(
    document.graph.edges.map((edge) => edge.id)
  );
  const duplicatedEdges = document.graph.edges
    .filter(
      (edge) => sourceIdSet.has(edge.source) && sourceIdSet.has(edge.target)
    )
    .map((edge) => {
      const copiedId = availableCopyId(duplicatedEdgeIds, edge.id);
      duplicatedEdgeIds.add(copiedId);
      return {
        ...edge,
        id: copiedId,
        source: idMap.get(edge.source)!,
        target: idMap.get(edge.target)!,
        containerId:
          edge.containerId && idMap.has(edge.containerId)
            ? idMap.get(edge.containerId)!
            : edge.containerId
      };
    });

  return fitContainerBounds({
    ...document,
    graph: {
      ...document.graph,
      nodes: [...document.graph.nodes, ...duplicatedNodes],
      edges: [...document.graph.edges, ...duplicatedEdges]
    }
  });
}
