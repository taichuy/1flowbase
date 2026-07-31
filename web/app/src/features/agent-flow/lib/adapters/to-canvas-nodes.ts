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

const CANVAS_NODE_WIDTH = 196;
const CANVAS_NODE_HEIGHT = 96;

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
  activeContainerId: string | null,
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
  > & {
    nodePickerOptions: NodePickerOption[];
    workflowTriggerContext?: AgentFlowCanvasNodeData['workflowTriggerContext'];
  }
): AgentFlowCanvasNode[] {
  return document.graph.nodes
    .filter((node) => node.containerId === activeContainerId)
    .map((node) => {
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
      return {
        id: node.id,
        type: 'agentFlowNode',
        selected: node.id === selectedNodeId,
        position: node.position,
        width: CANVAS_NODE_WIDTH,
        height: CANVAS_NODE_HEIGHT,
        measured: {
          width: CANVAS_NODE_WIDTH,
          height: CANVAS_NODE_HEIGHT
        },
        data: {
          nodeId: node.id,
          nodeType: node.type,
          nodeSchema: resolveAgentFlowNodeSchema(node.type),
          typeLabel: nodeTypeLabel(node.type),
          alias: node.alias,
          description: node.description,
          config: node.config,
          issueCount: issueCountByNodeId[node.id] ?? 0,
          canEnterContainer: node.type === 'iteration' || node.type === 'loop',
          pickerOpen: pickerNodeId === node.id,
          pickerSourceHandleId,
          showTargetHandle: flowRole !== 'entry',
          showSourceHandle: flowRole !== 'terminal',
          branchSourceHandles,
          toolSourceHandles,
          isContainer: node.type === 'iteration' || node.type === 'loop',
          workflowTriggerContext: actions.workflowTriggerContext ?? null,
          ...actions,
          nodePickerOptions: actions.nodePickerOptions
        }
      };
    });
}
