import type {
  FlowAuthoringDocument,
  FlowBinding,
  FlowNodeDocument
} from '@1flowbase/flow-schema';
import { getLlmNodeOutputs } from '@1flowbase/flow-schema';

import { createEdgeDocument } from '../edge-factory';
import { createNodeDocument } from '../node-factory';
import type { NodePickerOption } from '../../plugin-node-definitions';
import { getOutgoingEdges, getNodeById } from '../selectors';
import {
  getDefaultIfElseSourceHandle,
  getIfElseBranches
} from '../../if-else-branches';
import { shiftDownstreamNodesBFS } from './layout';
import {
  createLlmToolSourceHandleId,
  getLlmVisibleInternalTools,
  isLlmToolSourceHandle,
  type LlmVisibleInternalTool
} from '../../llm-node-config';
import { getNodeFlowRole } from '../../node-definitions';

const NODE_GAP_X = 280;
const NODE_HEIGHT = 96;
const NODE_GAP_Y = 40;
const NODE_GRID_SIZE = 20;

function snapNodeCoordinate(value: number) {
  return Math.round(value / NODE_GRID_SIZE) * NODE_GRID_SIZE;
}

type NodeFieldValue =
  | string
  | number
  | boolean
  | null
  | FlowBinding
  | Record<string, unknown>
  | LlmVisibleInternalTool[]
  | string[]
  | string[][];

function replaceOutputTitle(
  outputs: FlowNodeDocument['outputs'],
  outputKey: string,
  title: string
): FlowNodeDocument['outputs'] {
  return outputs.map((output) =>
    output.key === outputKey ? { ...output, title } : output
  );
}

function deriveLlmOutputs(
  config: Record<string, unknown>,
  currentOutputs: FlowNodeDocument['outputs']
): FlowNodeDocument['outputs'] {
  return getLlmNodeOutputs(config).map((derivedOutput) => {
    const currentOutput = currentOutputs.find(
      (output) => output.key === derivedOutput.key
    );

    return currentOutput
      ? { ...derivedOutput, ...currentOutput }
      : derivedOutput;
  });
}

export function replaceNodeOutputs(
  document: FlowAuthoringDocument,
  nodeId: string,
  outputs: FlowNodeDocument['outputs']
): FlowAuthoringDocument {
  return {
    ...document,
    graph: {
      ...document.graph,
      nodes: document.graph.nodes.map((node) =>
        node.id === nodeId
          ? {
              ...node,
              outputs
            }
          : node
      )
    }
  };
}

function llmToolConnectorId(tool: LlmVisibleInternalTool) {
  return tool.connector_id || tool.tool_name;
}

export function updateLlmVisibleInternalTools(
  document: FlowAuthoringDocument,
  nodeId: string,
  nextTools: LlmVisibleInternalTool[]
): FlowAuthoringDocument {
  const node = getNodeById(document, nodeId);

  if (!node || node.type !== 'llm') {
    return document;
  }

  const currentTools = getLlmVisibleInternalTools(node.config);
  const nextDocument = updateNodeField(document, {
    nodeId,
    fieldKey: 'config.visible_internal_llm_tools',
    value: nextTools
  });
  const sourceHandleUpdates = new Map<string, string>();

  for (const [index, nextTool] of nextTools.entries()) {
    const currentTool = currentTools[index];

    if (!currentTool) {
      continue;
    }

    const currentConnectorId = llmToolConnectorId(currentTool);
    const nextConnectorId = llmToolConnectorId(nextTool);

    if (
      currentConnectorId &&
      nextConnectorId &&
      currentConnectorId !== nextConnectorId
    ) {
      sourceHandleUpdates.set(
        createLlmToolSourceHandleId(currentConnectorId),
        createLlmToolSourceHandleId(nextConnectorId)
      );
    }
  }

  if (sourceHandleUpdates.size === 0) {
    return nextDocument;
  }

  return {
    ...nextDocument,
    graph: {
      ...nextDocument.graph,
      edges: nextDocument.graph.edges.map((edge) =>
        edge.source === nodeId &&
        edge.sourceHandle &&
        sourceHandleUpdates.has(edge.sourceHandle)
          ? {
              ...edge,
              sourceHandle:
                sourceHandleUpdates.get(edge.sourceHandle) ?? edge.sourceHandle
            }
          : edge
      )
    }
  };
}

export function moveNodes(
  document: FlowAuthoringDocument,
  positions: Record<string, { x: number; y: number }>
): FlowAuthoringDocument {
  if (Object.keys(positions).length === 0) {
    return document;
  }

  return {
    ...document,
    graph: {
      ...document.graph,
      nodes: document.graph.nodes.map((node) =>
        positions[node.id]
          ? {
              ...node,
              position: positions[node.id]
            }
          : node
      )
    }
  };
}

function collectNodeIdsToRemove(
  document: FlowAuthoringDocument,
  rootNodeId: string
) {
  const queue = [rootNodeId];
  const collectedIds: string[] = [];

  while (queue.length > 0) {
    const currentNodeId = queue.shift();

    if (!currentNodeId || collectedIds.includes(currentNodeId)) {
      continue;
    }

    collectedIds.push(currentNodeId);

    for (const candidate of document.graph.nodes) {
      if (candidate.containerId === currentNodeId) {
        queue.push(candidate.id);
      }
    }
  }

  return collectedIds;
}

export function removeNodeSubgraph(
  document: FlowAuthoringDocument,
  payload: { nodeId: string }
): FlowAuthoringDocument {
  const node = getNodeById(document, payload.nodeId);

  if (!node) {
    return document;
  }

  const removedNodeIds = new Set(
    collectNodeIdsToRemove(document, payload.nodeId)
  );

  return {
    ...document,
    graph: {
      ...document.graph,
      nodes: document.graph.nodes.filter(
        (candidate) => !removedNodeIds.has(candidate.id)
      ),
      edges: document.graph.edges.filter(
        (edge) =>
          !removedNodeIds.has(edge.source) && !removedNodeIds.has(edge.target)
      )
    }
  };
}

export function replaceNodeWithOption(
  document: FlowAuthoringDocument,
  payload: { nodeId: string; option: NodePickerOption }
): FlowAuthoringDocument {
  const node = getNodeById(document, payload.nodeId);

  if (!node) {
    return document;
  }

  const replacement = createNodeDocument(
    payload.option,
    node.id,
    node.position.x,
    node.position.y
  );

  return {
    ...document,
    graph: {
      ...document.graph,
      nodes: document.graph.nodes.map((candidate) =>
        candidate.id === payload.nodeId
          ? {
              ...replacement,
              id: node.id,
              containerId: node.containerId,
              position: node.position
            }
          : candidate
      )
    }
  };
}

export function updateNodeField(
  document: FlowAuthoringDocument,
  payload: {
    nodeId: string;
    fieldKey: string;
    value: NodeFieldValue;
  }
): FlowAuthoringDocument {
  const nextNodes = document.graph.nodes.map((node) => {
    if (node.id !== payload.nodeId) {
      return node;
    }

    if (payload.fieldKey === 'alias' && typeof payload.value === 'string') {
      return {
        ...node,
        alias: payload.value
      };
    }

    if (
      payload.fieldKey === 'description' &&
      typeof payload.value === 'string'
    ) {
      return {
        ...node,
        description: payload.value
      };
    }

    if (payload.fieldKey.startsWith('config.')) {
      const configKey = payload.fieldKey.slice('config.'.length);
      const nextConfig = {
        ...node.config,
        [configKey]: payload.value
      };

      return {
        ...node,
        config: nextConfig,
        outputs:
          node.type === 'llm' && configKey === 'response_format'
            ? deriveLlmOutputs(nextConfig, node.outputs)
            : node.outputs
      };
    }

    if (payload.fieldKey.startsWith('bindings.')) {
      const bindingKey = payload.fieldKey.slice('bindings.'.length);

      return {
        ...node,
        bindings: {
          ...node.bindings,
          [bindingKey]: payload.value as FlowBinding
        }
      };
    }

    if (
      payload.fieldKey.startsWith('outputs.') &&
      typeof payload.value === 'string'
    ) {
      const outputKey = payload.fieldKey.slice('outputs.'.length);

      return {
        ...node,
        outputs: replaceOutputTitle(node.outputs, outputKey, payload.value)
      };
    }

    return node;
  });
  const branches =
    payload.fieldKey === 'bindings.branches'
      ? getIfElseBranches(payload.value as FlowBinding)
      : null;
  const nextEdges = branches
    ? document.graph.edges.filter((edge) => {
        if (edge.source !== payload.nodeId) {
          return true;
        }

        return (
          edge.sourceHandle !== null &&
          branches.some((branch) => branch.sourceHandle === edge.sourceHandle)
        );
      })
    : document.graph.edges;

  return {
    ...document,
    graph: {
      ...document.graph,
      nodes: nextNodes,
      edges: nextEdges
    }
  };
}

export function insertNodeAfter(
  document: FlowAuthoringDocument,
  anchorNodeId: string,
  node: FlowNodeDocument,
  sourceHandle: string | null = null
): FlowAuthoringDocument {
  const anchorNode = getNodeById(document, anchorNodeId);

  if (!anchorNode) {
    return document;
  }

  const resolvedSourceHandle =
    sourceHandle ?? getDefaultIfElseSourceHandle(anchorNode);
  const outgoingEdges = getOutgoingEdges(document, anchorNodeId).filter(
    (edge) => edge.sourceHandle === resolvedSourceHandle
  );

  if (getNodeFlowRole(anchorNode.type) === 'terminal') {
    return document;
  }

  const insertedNodeFlowRole = getNodeFlowRole(node.type);

  if (
    insertedNodeFlowRole === 'entry' ||
    (insertedNodeFlowRole === 'terminal' && outgoingEdges.length > 0)
  ) {
    return document;
  }
  const nextPositionX = anchorNode.position.x + NODE_GAP_X;
  const mountedToolEdges = isLlmToolSourceHandle(resolvedSourceHandle)
    ? getOutgoingEdges(document, anchorNodeId)
        .filter((edge) => isLlmToolSourceHandle(edge.sourceHandle))
        .map((edge) => getNodeById(document, edge.target)?.position.y)
        .filter(
          (positionY): positionY is number => typeof positionY === 'number'
        )
        .sort((left, right) => left - right)
    : [];
  const nextMountedPositionY =
    mountedToolEdges.length > 0
      ? Math.max(
          anchorNode.position.y + NODE_HEIGHT + NODE_GAP_Y,
          mountedToolEdges[mountedToolEdges.length - 1] +
            NODE_HEIGHT +
            NODE_GAP_Y
        )
      : anchorNode.position.y + NODE_HEIGHT + NODE_GAP_Y;
  const nextMountedGridPositionY = snapNodeCoordinate(nextMountedPositionY);

  const insertedNode = {
    ...node,
    containerId: anchorNode.containerId,
    position: {
      x: nextPositionX,
      y: isLlmToolSourceHandle(resolvedSourceHandle)
        ? nextMountedGridPositionY
        : anchorNode.position.y
    }
  };

  const intermediateDoc = {
    ...document,
    graph: {
      nodes: [...document.graph.nodes, insertedNode],
      edges: [
        ...document.graph.edges.filter(
          (edge) =>
            edge.source !== anchorNodeId ||
            edge.sourceHandle !== resolvedSourceHandle
        ),
        createEdgeDocument({
          id: `edge-${anchorNodeId}-${insertedNode.id}`,
          source: anchorNodeId,
          target: insertedNode.id,
          sourceHandle: resolvedSourceHandle,
          containerId: anchorNode.containerId
        }),
        ...outgoingEdges.map((edge) =>
          createEdgeDocument({
            id: `edge-${insertedNode.id}-${edge.target}`,
            source: insertedNode.id,
            target: edge.target,
            sourceHandle: null,
            targetHandle: edge.targetHandle,
            containerId: edge.containerId,
            points: edge.points
          })
        )
      ]
    }
  };

  return shiftDownstreamNodesBFS(intermediateDoc, insertedNode.id, NODE_GAP_X);
}
