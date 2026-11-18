import isEqual from 'lodash/isEqual';

import type {
  AgentFlowCanvasEdge,
  AgentFlowCanvasNode
} from '../../components/canvas/node-types';

export function syncCanvasNodes(
  current: AgentFlowCanvasNode[],
  projected: AgentFlowCanvasNode[]
): AgentFlowCanvasNode[] {
  const byId = new Map(current.map((node) => [node.id, node]));
  const next = projected.map((node) => {
    const previous = byId.get(node.id);
    if (!previous || previous.type !== node.type) return node;

    // Measurements belong to React Flow, not the authoring document. Dropping
    // them makes existing nodes disappear until ResizeObserver measures again.
    const merged = { ...previous, ...node, measured: previous.measured };
    if (isEqual(previous.data, node.data)) merged.data = previous.data;
    if (isEqual(previous.position, node.position)) {
      merged.position = previous.position;
    }
    return isEqual(previous, merged) ? previous : merged;
  });
  return next.length === current.length &&
    next.every((node, index) => node === current[index])
    ? current
    : next;
}

export function syncCanvasEdges(
  current: AgentFlowCanvasEdge[],
  projected: AgentFlowCanvasEdge[]
): AgentFlowCanvasEdge[] {
  const byId = new Map(current.map((edge) => [edge.id, edge]));
  const next = projected.map((edge) => {
    const previous = byId.get(edge.id);
    return previous && isEqual(previous, edge) ? previous : edge;
  });
  return next.length === current.length &&
    next.every((edge, index) => edge === current[index])
    ? current
    : next;
}
