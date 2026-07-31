import type {
  FlowAuthoringDocument,
  FlowNodeDocument
} from '@1flowbase/flow-schema';

import { getDefaultIfElseSourceHandle } from '../../if-else-branches';
import {
  getLlmVisibleInternalTools,
  getLlmVisibleInternalToolsEnabled,
  parseLlmToolSourceHandleId
} from '../../llm-node-config';
import { createEdgeDocument } from '../edge-factory';
import { getEdgeById, getNodeById } from '../selectors';
import { shiftDownstreamNodesBFS } from './layout';
import { getNodeFlowRole } from '../../node-definitions';

export interface EdgeConnection {
  source?: string | null;
  target?: string | null;
  sourceHandle?: string | null;
  targetHandle?: string | null;
}

export function validateConnection(
  document: FlowAuthoringDocument,
  connection: EdgeConnection
) {
  if (!connection.source || !connection.target) {
    return false;
  }

  const sourceNode = getNodeById(document, connection.source);
  const targetNode = getNodeById(document, connection.target);

  if (
    !sourceNode ||
    !targetNode ||
    sourceNode.id === targetNode.id ||
    sourceNode.containerId !== targetNode.containerId ||
    getNodeFlowRole(sourceNode.type) === 'terminal' ||
    getNodeFlowRole(targetNode.type) === 'entry'
  ) {
    return false;
  }

  return true;
}

export function validateVisibleInternalLlmToolConnection(
  document: FlowAuthoringDocument,
  connection: EdgeConnection
) {
  const connectorId = parseLlmToolSourceHandleId(connection.sourceHandle);

  if (!connectorId || !validateConnection(document, connection)) {
    return false;
  }

  const sourceNode = getNodeById(document, connection.source ?? null);

  return Boolean(
    sourceNode?.type === 'llm' &&
    getLlmVisibleInternalToolsEnabled(sourceNode.config) &&
    getLlmVisibleInternalTools(sourceNode.config).some(
      (tool) => (tool.connector_id || tool.tool_name) === connectorId
    )
  );
}

export function reconnectEdge(
  document: FlowAuthoringDocument,
  payload: {
    edgeId: string;
    connection: EdgeConnection;
  }
): FlowAuthoringDocument {
  const edge = getEdgeById(document, payload.edgeId);

  if (
    !edge ||
    !validateConnection(document, payload.connection)
  ) {
    return document;
  }

  return {
    ...document,
    graph: {
      ...document.graph,
      edges: document.graph.edges.map((item) =>
        item.id === payload.edgeId
          ? {
              ...item,
              source: payload.connection.source ?? item.source,
              target: payload.connection.target ?? item.target,
              sourceHandle: payload.connection.sourceHandle ?? null,
              targetHandle: payload.connection.targetHandle ?? null
            }
          : item
      )
    }
  };
}

function createNextEdgeId(
  document: FlowAuthoringDocument,
  connection: {
    source: NonNullable<EdgeConnection['source']>;
    target: NonNullable<EdgeConnection['target']>;
  }
) {
  const baseId = `edge-${connection.source.replace(/^node-/, '')}-${connection.target.replace(/^node-/, '')}`;

  if (!document.graph.edges.some((edge) => edge.id === baseId)) {
    return baseId;
  }

  let index = 2;

  while (
    document.graph.edges.some((edge) => edge.id === `${baseId}-${index}`)
  ) {
    index += 1;
  }

  return `${baseId}-${index}`;
}

export function connectNodes(
  document: FlowAuthoringDocument,
  payload: {
    connection: EdgeConnection;
  }
): FlowAuthoringDocument {
  const toolConnectorId = parseLlmToolSourceHandleId(
    payload.connection.sourceHandle
  );

  if (toolConnectorId) {
    if (
      !validateVisibleInternalLlmToolConnection(document, payload.connection)
    ) {
      return document;
    }
  } else if (!validateConnection(document, payload.connection)) {
    return document;
  }

  const sourceNode = getNodeById(document, payload.connection.source ?? null);
  const targetNode = getNodeById(document, payload.connection.target ?? null);

  if (!sourceNode || !targetNode) {
    return document;
  }

  const alreadyExists = document.graph.edges.some(
    (edge) =>
      edge.source === payload.connection.source &&
      edge.target === payload.connection.target &&
      edge.sourceHandle === (payload.connection.sourceHandle ?? null) &&
      edge.targetHandle === (payload.connection.targetHandle ?? null)
  );

  if (alreadyExists) {
    return document;
  }

  return {
    ...document,
    graph: {
      ...document.graph,
      edges: [
        ...document.graph.edges,
        createEdgeDocument({
          id: createNextEdgeId(document, {
            source: sourceNode.id,
            target: targetNode.id
          }),
          source: sourceNode.id,
          target: targetNode.id,
          sourceHandle: payload.connection.sourceHandle ?? null,
          targetHandle: payload.connection.targetHandle ?? null,
          containerId: sourceNode.containerId
        })
      ]
    }
  };
}

export function insertNodeOnEdge(
  document: FlowAuthoringDocument,
  payload: {
    edgeId: string;
    node: FlowNodeDocument;
  }
): FlowAuthoringDocument {
  const edge = getEdgeById(document, payload.edgeId);

  if (!edge) {
    return document;
  }

  const sourceNode = getNodeById(document, edge.source);
  const targetNode = getNodeById(document, edge.target);

  if (!sourceNode || !targetNode) {
    return document;
  }

  if (
    getNodeFlowRole(sourceNode.type) === 'terminal' ||
    getNodeFlowRole(payload.node.type) !== 'processing'
  ) {
    return document;
  }

  const insertedNode = {
    ...payload.node,
    containerId: sourceNode.containerId,
    position: {
      x: Math.round((sourceNode.position.x + targetNode.position.x) / 2),
      y: Math.round((sourceNode.position.y + targetNode.position.y) / 2)
    }
  };

  const intermediateDoc = {
    ...document,
    graph: {
      nodes: [...document.graph.nodes, insertedNode],
      edges: [
        ...document.graph.edges.filter((item) => item.id !== payload.edgeId),
        createEdgeDocument({
          id: `edge-${edge.source}-${insertedNode.id}`,
          source: edge.source,
          target: insertedNode.id,
          sourceHandle: edge.sourceHandle,
          targetHandle: null,
          containerId: edge.containerId
        }),
        createEdgeDocument({
          id: `edge-${insertedNode.id}-${edge.target}`,
          source: insertedNode.id,
          target: edge.target,
          sourceHandle: null,
          targetHandle: edge.targetHandle,
          containerId: edge.containerId
        })
      ]
    }
  };

  return shiftDownstreamNodesBFS(intermediateDoc, sourceNode.id, 280);
}

export function connectNodeFromSource(
  document: FlowAuthoringDocument,
  payload: {
    sourceNodeId: string;
    sourceHandleId?: string | null;
    node: FlowNodeDocument;
  }
): FlowAuthoringDocument {
  const sourceNode = getNodeById(document, payload.sourceNodeId);

  if (!sourceNode) {
    return document;
  }

  if (
    getNodeFlowRole(sourceNode.type) === 'terminal' ||
    getNodeFlowRole(payload.node.type) === 'entry'
  ) {
    return document;
  }

  const insertedNode = {
    ...payload.node,
    containerId: sourceNode.containerId
  };
  const sourceHandleId =
    payload.sourceHandleId ?? getDefaultIfElseSourceHandle(sourceNode);

  const intermediateDoc = {
    ...document,
    graph: {
      ...document.graph,
      nodes: [...document.graph.nodes, insertedNode],
      edges: [
        ...document.graph.edges,
        createEdgeDocument({
          id: createNextEdgeId(document, {
            source: sourceNode.id,
            target: insertedNode.id
          }),
          source: sourceNode.id,
          target: insertedNode.id,
          sourceHandle: sourceHandleId,
          targetHandle: null,
          containerId: sourceNode.containerId
        })
      ]
    }
  };

  return shiftDownstreamNodesBFS(intermediateDoc, sourceNode.id, 280);
}

export function removeEdge(
  document: FlowAuthoringDocument,
  payload: {
    edgeId: string;
  }
): FlowAuthoringDocument {
  if (!document.graph.edges.some((edge) => edge.id === payload.edgeId)) {
    return document;
  }

  return {
    ...document,
    graph: {
      ...document.graph,
      edges: document.graph.edges.filter((edge) => edge.id !== payload.edgeId)
    }
  };
}
