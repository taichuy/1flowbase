import { containerBodyPickerOptions } from '../node-definitions/nodes/containers/body-policy';
import type {
  FlowAuthoringDocument,
  FlowNodeDocument
} from '@1flowbase/flow-schema';

import type { AgentFlowCanvasEdge } from '../../components/canvas/node-types';
import { MAIN_SOURCE_HANDLE_ID } from '../canvas/handle-ids';
import type { NodePickerOption } from '../plugin-node-definitions';

export function toCanvasEdges(
  document: FlowAuthoringDocument,
  _activeContainerId: string | null,
  selectedEdgeId: string | null,
  actions: {
    nodePickerOptions: NodePickerOption[];
    onInsertNode: (edgeId: string, option: NodePickerOption) => void;
  }
): AgentFlowCanvasEdge[] {
  const nodesById = new Map<string, FlowNodeDocument>();
  const visibleNodeIds = new Set<string>();
  for (const node of document.graph.nodes) {
    // Keep the first-match behavior of node lookup, including duplicate IDs.
    if (!nodesById.has(node.id)) nodesById.set(node.id, node);
    visibleNodeIds.add(node.id);
  }

  return document.graph.edges
    .filter(
      (edge) =>
        nodesById.get(edge.source)?.containerId === edge.containerId &&
        nodesById.get(edge.target)?.containerId === edge.containerId &&
        visibleNodeIds.has(edge.source) &&
        visibleNodeIds.has(edge.target)
    )
    .map((edge) => {
      const sourceNode = nodesById.get(edge.source);
      const canInsertNode = sourceNode?.type !== 'answer';

      return {
        id: edge.id,
        type: 'agentFlowEdge' as const,
        selected: edge.id === selectedEdgeId,
        source: edge.source,
        target: edge.target,
        sourceHandle: edge.sourceHandle ?? MAIN_SOURCE_HANDLE_ID,
        targetHandle: edge.targetHandle,
        animated: false,
        zIndex: edge.containerId ? 1 : 0,
        style: { stroke: '#b2c8b9', strokeWidth: 2 },
        data: {
          nodePickerOptions: containerBodyPickerOptions(
            actions.nodePickerOptions,
            sourceNode?.containerId ?? null
          ),
          onInsertNode: canInsertNode ? actions.onInsertNode : undefined
        }
      };
    });
}
