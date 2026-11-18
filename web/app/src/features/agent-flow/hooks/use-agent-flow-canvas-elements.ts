import { useEffect, useLayoutEffect, useMemo, useRef } from 'react';
import { useReactFlow } from '@xyflow/react';
import { useCanvasInteractions } from './interactions/use-canvas-interactions';
import { useEdgeInteractions } from './interactions/use-edge-interactions';
import { useNodeInteractions } from './interactions/use-node-interactions';
import { useSelectionInteractions } from './interactions/use-selection-interactions';
import { toCanvasEdges } from '../lib/adapters/to-canvas-edges';
import { toCanvasNodes } from '../lib/adapters/to-canvas-nodes';
import {
  syncCanvasEdges,
  syncCanvasNodes
} from '../lib/adapters/sync-canvas-elements';
import { useAgentFlowEditorStore } from '../store/editor/provider';
import {
  selectActiveContainerId,
  selectSelectedNodeId,
  selectWorkingDocument
} from '../store/editor/selectors';
import type {
  AgentFlowCanvasEdge,
  AgentFlowCanvasNode
} from '../components/canvas/node-types';
import type { NodePickerOption } from '../lib/plugin-node-definitions';

export function useAgentFlowCanvasElements({
  issueCountByNodeId,
  interactionLocked,
  readOnly,
  nodeRunStatuses,
  workflowTriggerContext,
  nodePickerOptions,
  onRunNode
}: {
  issueCountByNodeId: Record<string, number>;
  interactionLocked: boolean;
  readOnly: boolean;
  nodeRunStatuses?: Record<string, 'running' | 'succeeded' | 'failed'>;
  workflowTriggerContext?: AgentFlowCanvasNode['data']['workflowTriggerContext'];
  nodePickerOptions: NodePickerOption[];
  onRunNode?: (nodeId: string) => void;
}) {
  const reactFlow = useReactFlow<AgentFlowCanvasNode, AgentFlowCanvasEdge>();
  const document = useAgentFlowEditorStore(selectWorkingDocument);
  const activeContainerId = useAgentFlowEditorStore(selectActiveContainerId);
  const selectedEdgeId = useAgentFlowEditorStore(
    (state) => state.selectedEdgeId
  );
  const selectedNodeId = useAgentFlowEditorStore(selectSelectedNodeId);
  const nodePickerState = useAgentFlowEditorStore(
    (state) => state.nodePickerState
  );
  const connectingPayload = useAgentFlowEditorStore(
    (state) => state.connectingPayload
  );
  const canvasInteractions = useCanvasInteractions();
  const nodeInteractions = useNodeInteractions();
  const edgeInteractions = useEdgeInteractions();
  const selectionInteractions = useSelectionInteractions();
  const actionsRef = useRef({ nodeInteractions, edgeInteractions, onRunNode });
  useLayoutEffect(() => {
    actionsRef.current = { nodeInteractions, edgeInteractions, onRunNode };
  });
  // Canvas objects retain their callbacks while actions read the latest draft.
  const actions = useMemo(
    () => ({
      onOpenPicker: (nodeId: string, handleId?: string | null) =>
        actionsRef.current.nodeInteractions.openNodePicker(nodeId, handleId),
      onClosePicker: () =>
        actionsRef.current.nodeInteractions.closeNodePicker(),
      onOpenContainer: (nodeId: string) =>
        actionsRef.current.nodeInteractions.openContainer(nodeId),
      onSelectNode: (nodeId: string) =>
        actionsRef.current.nodeInteractions.selectNode(nodeId),
      onInsertNode: (
        nodeId: string,
        option: NodePickerOption,
        handleId?: string | null
      ) =>
        actionsRef.current.nodeInteractions.insertAfterNode(
          nodeId,
          option,
          handleId
        ),
      onReplaceNode: (nodeId: string, option: NodePickerOption) =>
        actionsRef.current.nodeInteractions.replaceNode(nodeId, option),
      onDeleteNode: (nodeId: string) =>
        actionsRef.current.nodeInteractions.deleteNode(nodeId),
      onRunNode: (nodeId: string) => actionsRef.current.onRunNode?.(nodeId),
      onInsertOnEdge: (edgeId: string, option: NodePickerOption) =>
        actionsRef.current.edgeInteractions.insertOnEdge(edgeId, option),
      ignore: () => {}
    }),
    []
  );
  const baseNodes = useMemo(
    () =>
      toCanvasNodes(
        document,
        activeContainerId,
        selectedNodeId,
        nodePickerState.open && !nodePickerState.anchorCanvasPosition
          ? nodePickerState.anchorNodeId
          : null,
        connectingPayload.sourceHandleId,
        issueCountByNodeId,
        {
          readOnly,
          onOpenPicker: readOnly ? actions.ignore : actions.onOpenPicker,
          onClosePicker: actions.onClosePicker,
          onOpenContainer: actions.onOpenContainer,
          onSelectNode: actions.onSelectNode,
          onInsertNode: readOnly ? actions.ignore : actions.onInsertNode,
          onRunNode:
            !interactionLocked && onRunNode ? actions.onRunNode : undefined,
          workflowTriggerContext,
          onReplaceNode: readOnly ? actions.ignore : actions.onReplaceNode,
          onDeleteNode: readOnly ? actions.ignore : actions.onDeleteNode,
          onResizeContainer: readOnly
            ? undefined
            : canvasInteractions.commitContainerResize,
          nodePickerOptions
        }
      ).map((node) =>
        nodeRunStatuses
          ? {
              ...node,
              data: { ...node.data, runStatus: nodeRunStatuses[node.id] }
            }
          : node
      ),
    [
      interactionLocked,
      nodeRunStatuses,
      readOnly,
      activeContainerId,
      canvasInteractions.commitContainerResize,
      document,
      issueCountByNodeId,
      actions,
      nodePickerOptions,
      onRunNode,
      workflowTriggerContext,
      nodePickerState.anchorCanvasPosition,
      nodePickerState.anchorNodeId,
      nodePickerState.open,
      connectingPayload.sourceHandleId,
      selectedNodeId
    ]
  );
  const edges = useMemo(
    () =>
      toCanvasEdges(document, activeContainerId, selectedEdgeId, {
        nodePickerOptions,
        onInsertNode: readOnly ? undefined : actions.onInsertOnEdge
      }).map((edge) => {
        const running =
          nodeRunStatuses?.[edge.target] === 'running' &&
          nodeRunStatuses?.[edge.source] === 'succeeded';
        return nodeRunStatuses
          ? {
              ...edge,
              animated: running,
              style: {
                ...edge.style,
                ...(running ? { stroke: 'var(--status-running)' } : {})
              }
            }
          : edge;
      }),
    [
      nodeRunStatuses,
      readOnly,
      activeContainerId,
      document,
      actions,
      nodePickerOptions,
      selectedEdgeId
    ]
  );

  useEffect(() => {
    reactFlow.setNodes((current) => syncCanvasNodes(current, baseNodes));
  }, [baseNodes, reactFlow]);

  useEffect(() => {
    reactFlow.setEdges((current) => syncCanvasEdges(current, edges));
  }, [edges, reactFlow]);

  return {
    document,
    nodePickerState,
    connectingPayload,
    canvasInteractions,
    nodeInteractions,
    edgeInteractions,
    selectionInteractions,
    baseNodes,
    edges
  };
}
