import type { FlowNodeType } from '@1flowbase/flow-schema';
import type { EdgeTypes, Node, NodeTypes } from '@xyflow/react';

import type { CanvasNodeSchema } from '../../../../shared/schema-ui/v1/contracts/canvas-node-schema';
import type { NodePickerOption } from '../../lib/plugin-node-definitions';
import { AgentFlowNodeCard } from '../nodes/AgentFlowNodeCard';
import { AgentFlowCustomEdge, type AgentFlowCanvasEdge } from './custom-edge';

export interface AgentFlowCanvasNodeData extends Record<string, unknown> {
  nodeId: string;
  nodeType: FlowNodeType;
  nodeSchema: CanvasNodeSchema;
  typeLabel: string;
  alias: string;
  description?: string;
  config: Record<string, unknown>;
  issueCount: number;
  canEnterContainer: boolean;
  pickerOpen: boolean;
  showTargetHandle: boolean;
  showSourceHandle: boolean;
  branchSourceHandles: Array<{ id: string; title: string }>;
  compactSourceHandle: { id: string; title: string } | null;
  compactSourceHandleOccupied: boolean;
  compactNodePickerOptions: NodePickerOption[];
  toolSourceHandles: Array<{ id: string; title: string }>;
  pickerSourceHandleId: string | null;
  isContainer: boolean;
  workflowTriggerContext?: unknown;
  nodePickerOptions: NodePickerOption[];
  onOpenPicker: (nodeId: string, sourceHandleId?: string | null) => void;
  onClosePicker: () => void;
  onOpenContainer: (nodeId: string) => void;
  onSelectNode: (nodeId: string) => void;
  onInsertNode: (
    targetId: string,
    option: NodePickerOption,
    sourceHandleId?: string | null
  ) => void;
  onRunNode?: (nodeId: string) => void;
  onReplaceNode: (targetId: string, option: NodePickerOption) => void;
  onDeleteNode: (nodeId: string) => void;
}

export type AgentFlowCanvasNode = Node<
  AgentFlowCanvasNodeData,
  'agentFlowNode'
>;
export type { AgentFlowCanvasEdge };

export const agentFlowNodeTypes: NodeTypes = {
  agentFlowNode: AgentFlowNodeCard
};

export const agentFlowEdgeTypes: EdgeTypes = {
  agentFlowEdge: AgentFlowCustomEdge
};
