import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

function uniqueNodeIds(nodeIds: string[]) {
  return [...new Set(nodeIds)];
}

export function getDirectDownstreamNodes(
  document: FlowAuthoringDocument,
  nodeId: string
) {
  const outgoingIds = uniqueNodeIds(
    document.graph.edges
      .filter((edge) => edge.source === nodeId)
      .map((edge) => edge.target)
  );

  return document.graph.nodes.filter((node) => outgoingIds.includes(node.id));
}
