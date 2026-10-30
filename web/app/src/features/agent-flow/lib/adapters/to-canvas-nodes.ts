import { containerBodyPickerOptions } from '../node-definitions/nodes/containers/body-policy';
import {
  CONTAINER_CONTENT_TOP,
  CONTAINER_PADDING,
  getContainerMinimumSize,
  getNodeSize,
  parentFirstNodes,
  fitContainerBounds
} from '../document/transforms/container-geometry';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import type {
  AgentFlowCanvasNode,
  AgentFlowCanvasNodeData
} from '../../components/canvas/node-types';
import type { NodePickerOption } from '../plugin-node-definitions';
import { resolveAgentFlowNodeSchema } from '../../schema/node-schema-registry';
import { getIfElseBranchesFromBindings } from '../if-else-branches';
import {
  createLlmToolSourceHandleId,
  getLlmVisibleInternalTools,
  getLlmVisibleInternalToolsEnabled
} from '../llm-node-config';
import { getNodeFlowRole } from '../node-definitions';

function nodeTypeLabel(nodeType: AgentFlowCanvasNodeData['nodeType']) {
  if (nodeType === 'llm') {
    return 'LLM';
  }

  if (nodeType === 'plugin_node') {
    return 'Plugin Node';
  }

  return nodeType
    .split('_')
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(' ');
}

function llmToolSourceHandles(config: Record<string, unknown>) {
  if (!getLlmVisibleInternalToolsEnabled(config)) {
    return [];
  }

  return getLlmVisibleInternalTools(config).map((tool) => ({
    id: createLlmToolSourceHandleId(tool.connector_id || tool.tool_name),
    title: tool.connector_id || tool.tool_name
  }));
}

export function toCanvasNodes(
  document: FlowAuthoringDocument,
  _activeContainerId: string | null,
  selectedNodeId: string | null,
  pickerNodeId: string | null,
  pickerSourceHandleId: string | null,
  issueCountByNodeId: Record<string, number>,
  actions: Pick<
    AgentFlowCanvasNodeData,
    | 'onOpenPicker'
    | 'onClosePicker'
    | 'onOpenContainer'
    | 'onSelectNode'
    | 'onInsertNode'
    | 'onRunNode'
    | 'onReplaceNode'
    | 'onDeleteNode'
    | 'onResizeContainer'
  > & {
    nodePickerOptions: NodePickerOption[];
    workflowTriggerContext?: AgentFlowCanvasNodeData['workflowTriggerContext'];
  }
): AgentFlowCanvasNode[] {
  const sizedDocument = fitContainerBounds(document);
  const nodesById = new Map(
    sizedDocument.graph.nodes.map((node) => [node.id, node])
  );
  const populatedContainers = new Set(
    sizedDocument.graph.nodes
      .filter((node) => node.containerId && node.type !== 'container_start')
      .map((node) => node.containerId)
  );
  return parentFirstNodes(sizedDocument.graph.nodes).map((node) => {
    const flowRole = getNodeFlowRole(node.type);
    const branchSourceHandles =
      node.type === 'if_else'
        ? (getIfElseBranchesFromBindings(node.bindings) ?? []).map(
            (branch) => ({
              id: branch.sourceHandle,
              title: branch.title
            })
          )
        : [];
    const toolSourceHandles =
      node.type === 'llm' ? llmToolSourceHandles(node.config) : [];
    const container = node.type === 'loop' || node.type === 'iteration';
    const internalStart = node.type === 'container_start';
    const size = getNodeSize(node);
    const parent = node.containerId
      ? nodesById.get(node.containerId)
      : undefined;
    const parentSize = parent ? getNodeSize(parent) : undefined;
    const extent: [[number, number], [number, number]] | undefined = parentSize
      ? [
          [CONTAINER_PADDING, CONTAINER_CONTENT_TOP],
          [
            parentSize.width - CONTAINER_PADDING,
            parentSize.height - CONTAINER_PADDING
          ]
        ]
      : undefined;
    return {
      id: node.id,
      type: container
        ? 'agentFlowContainer'
        : internalStart
          ? 'containerStart'
          : 'agentFlowNode',
      parentId: node.containerId ?? undefined,
      extent,
      expandParent: false,
      deletable: false,
      dragHandle: container ? '.agent-flow-container__header' : undefined,
      ...(container
        ? {
            height: size.height,
            style: { width: size.width, height: size.height }
          }
        : {}),
      selected: node.id === selectedNodeId,
      position: node.position,
      width: size.width,
      data: {
        nodeId: node.id,
        nodeType: node.type,
        nodeSchema: resolveAgentFlowNodeSchema(node.type),
        typeLabel: nodeTypeLabel(node.type),
        alias: node.alias,
        description: node.description,
        config: node.config,
        issueCount: issueCountByNodeId[node.id] ?? 0,
        canEnterContainer: false,
        emptyContainer:
          internalStart && !populatedContainers.has(node.containerId),
        minimumContainerSize: container
          ? getContainerMinimumSize(sizedDocument.graph.nodes, node.id)
          : undefined,
        pickerOpen: pickerNodeId === node.id,
        pickerSourceHandleId,
        showTargetHandle: !internalStart && flowRole !== 'entry',
        showSourceHandle: flowRole !== 'terminal',
        branchSourceHandles,
        toolSourceHandles,
        isContainer: node.type === 'iteration' || node.type === 'loop',
        workflowTriggerContext: actions.workflowTriggerContext ?? null,
        ...actions,
        nodePickerOptions: containerBodyPickerOptions(
          actions.nodePickerOptions,
          node.containerId
        )
      }
    };
  });
}
