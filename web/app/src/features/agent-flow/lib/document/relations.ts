import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

export function getDirectDownstreamNodes(
  document: FlowAuthoringDocument,
  nodeId: string
) {
  const outgoingIds = new Set(
    document.graph.edges
      .filter((edge) => edge.source === nodeId)
      .map((edge) => edge.target)
  );

  return document.graph.nodes.filter((node) => outgoingIds.has(node.id));
}
