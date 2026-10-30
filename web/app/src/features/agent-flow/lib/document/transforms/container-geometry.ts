import type {
  FlowAuthoringDocument,
  FlowNodeDocument
} from '@1flowbase/flow-schema';
import {
  CANVAS_NODE_WIDTH,
  getCanvasNodeMinimumHeightForDocument
} from '../../canvas/node-dimensions';

export const CONTAINER_PADDING = 32;
export const CONTAINER_CONTENT_TOP = 88;
const CONTAINER_MIN_WIDTH = 560;
const CONTAINER_MIN_HEIGHT = 280;
const CONTAINER_START_SIZE = 40;

export function isContainerNode(node: Pick<FlowNodeDocument, 'type'>) {
  return node.type === 'loop' || node.type === 'iteration';
}

/** Parent-first order is required by xyflow. Malformed cycles must not hang the editor. */
export function parentFirstNodes(nodes: FlowNodeDocument[]) {
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const visiting = new Set<string>();
  const ordered: FlowNodeDocument[] = [];
  function visit(node: FlowNodeDocument) {
    if (visiting.has(node.id)) return;
    visiting.add(node.id);
    const parent = node.containerId ? byId.get(node.containerId) : undefined;
    if (parent) visit(parent);
    ordered.push(node);
  }
  nodes.forEach(visit);
  return ordered;
}

export function getNodeSize(node: FlowNodeDocument) {
  if (isContainerNode(node))
    return {
      width: node.width ?? CONTAINER_MIN_WIDTH,
      height: node.height ?? CONTAINER_MIN_HEIGHT
    };
  if (node.type === 'container_start')
    return { width: CONTAINER_START_SIZE, height: CONTAINER_START_SIZE };
  return {
    width: CANVAS_NODE_WIDTH,
    height: getCanvasNodeMinimumHeightForDocument(node)
  };
}

export function getContainerMinimumSize(
  nodes: FlowNodeDocument[],
  containerId: string
) {
  let width = CONTAINER_MIN_WIDTH;
  let height = CONTAINER_MIN_HEIGHT;
  for (const child of nodes) {
    if (child.containerId !== containerId) continue;
    const size = getNodeSize(child);
    width = Math.max(width, child.position.x + size.width + CONTAINER_PADDING);
    height = Math.max(
      height,
      child.position.y + size.height + CONTAINER_PADDING
    );
  }
  return { width, height };
}

/** Apply after an edit, inside the same history entry: grow ancestors without moving descendants. */
export function fitContainerBounds(
  document: FlowAuthoringDocument
): FlowAuthoringDocument {
  const nodes = parentFirstNodes(document.graph.nodes).map((node) =>
    node.containerId
      ? {
          ...node,
          position: {
            x: Math.max(CONTAINER_PADDING, node.position.x),
            y: Math.max(CONTAINER_CONTENT_TOP, node.position.y)
          }
        }
      : node
  );
  const byId = new Map(nodes.map((node) => [node.id, node]));
  for (const node of [...nodes].reverse()) {
    if (!isContainerNode(node)) continue;
    const minimum = getContainerMinimumSize([...byId.values()], node.id);
    byId.set(node.id, {
      ...node,
      width: Math.max(node.width ?? 0, minimum.width),
      height: Math.max(node.height ?? 0, minimum.height)
    });
  }
  return {
    ...document,
    graph: {
      ...document.graph,
      nodes: document.graph.nodes.map((node) => byId.get(node.id)!)
    }
  };
}

export function resizeContainer(
  document: FlowAuthoringDocument,
  nodeId: string,
  size: { width: number; height: number }
) {
  const node = document.graph.nodes.find(
    (candidate) => candidate.id === nodeId
  );
  if (!node || !isContainerNode(node)) return document;
  const minimum = getContainerMinimumSize(document.graph.nodes, nodeId);
  return fitContainerBounds({
    ...document,
    graph: {
      ...document.graph,
      nodes: document.graph.nodes.map((candidate) =>
        candidate.id === nodeId
          ? {
              ...candidate,
              width: Math.max(minimum.width, size.width),
              height: Math.max(minimum.height, size.height)
            }
          : candidate
      )
    }
  });
}

export function getNodeAbsolutePosition(
  document: FlowAuthoringDocument,
  nodeId: string | null
) {
  const byId = new Map(document.graph.nodes.map((node) => [node.id, node]));
  const position = { x: 0, y: 0 };
  const visited = new Set<string>();
  let node = nodeId ? byId.get(nodeId) : undefined;
  while (node && !visited.has(node.id)) {
    visited.add(node.id);
    position.x += node.position.x;
    position.y += node.position.y;
    node = node.containerId ? byId.get(node.containerId) : undefined;
  }
  return position;
}

/** Only called when a new container is inserted, never as a read/load repair. */
export function initializeContainer(
  document: FlowAuthoringDocument,
  nodeId: string
) {
  const container = document.graph.nodes.find((node) => node.id === nodeId);
  if (!container || !isContainerNode(container))
    return fitContainerBounds(document);
  if (
    document.graph.nodes.some(
      (node) => node.containerId === nodeId && node.type === 'container_start'
    )
  )
    return fitContainerBounds(document);
  let id = `${nodeId}-start`;
  const ids = new Set(document.graph.nodes.map((node) => node.id));
  for (let index = 2; ids.has(id); index++) id = `${nodeId}-start-${index}`;
  const start: FlowNodeDocument = {
    id,
    type: 'container_start',
    alias: container.alias,
    containerId: nodeId,
    position: { x: CONTAINER_PADDING, y: CONTAINER_CONTENT_TOP },
    configVersion: 1,
    config: {},
    bindings: {},
    outputs: []
  };
  return fitContainerBounds({
    ...document,
    graph: { ...document.graph, nodes: [...document.graph.nodes, start] }
  });
}
