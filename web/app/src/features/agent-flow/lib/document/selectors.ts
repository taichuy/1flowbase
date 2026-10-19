import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

export function getNodeById(
  document: FlowAuthoringDocument,
  nodeId: string | null
) {
  if (!nodeId) {
    return null;
  }

  return document.graph.nodes.find((node) => node.id === nodeId) ?? null;
}

export function getEdgeById(
  document: FlowAuthoringDocument,
  edgeId: string | null
) {
  if (!edgeId) {
    return null;
  }

  return document.graph.edges.find((edge) => edge.id === edgeId) ?? null;
}

export function getOutgoingEdges(
  document: FlowAuthoringDocument,
  nodeId: string
) {
  return document.graph.edges.filter((edge) => edge.source === nodeId);
}
