import type { AgentFlowIssue } from '../../lib/validate-document';
import { getContainerPathForNode } from '../../lib/document/transforms/container';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import { selectWorkingDocument } from '../../store/editor/selectors';

export function useContainerNavigation() {
  const document = useAgentFlowEditorStore(selectWorkingDocument);
  const activeContainerPath = useAgentFlowEditorStore(
    (state) => state.activeContainerPath
  );
  const setInteractionState = useAgentFlowEditorStore(
    (state) => state.setInteractionState
  );
  const setPanelState = useAgentFlowEditorStore((state) => state.setPanelState);
  const setSelection = useAgentFlowEditorStore((state) => state.setSelection);
  const focusIssueField = useAgentFlowEditorStore(
    (state) => state.focusIssueField
  );

  return {
    openContainer(nodeId: string) {
      const firstChildNode =
        document.graph.nodes.find((node) => node.containerId === nodeId)?.id ??
        null;

      setInteractionState({
        activeContainerPath: [
          ...getContainerPathForNode(document, nodeId),
          nodeId
        ],
        pendingLocateNodeId: nodeId
      });
      setSelection({
        selectedNodeId: firstChildNode,
        selectedNodeIds: firstChildNode ? [firstChildNode] : [],
        selectedEdgeId: null
      });
    },
    returnToRoot() {
      const currentContainerId = activeContainerPath.at(-1) ?? null;

      setInteractionState({ activeContainerPath: [] });
      setSelection({
        selectedNodeId: currentContainerId,
        selectedNodeIds: currentContainerId ? [currentContainerId] : [],
        selectedEdgeId: null
      });
    },
    jumpToIssue(issue: AgentFlowIssue) {
      setPanelState({ issuesOpen: false });

      if (!issue.nodeId) {
        return;
      }

      setInteractionState({
        activeContainerPath: getContainerPathForNode(document, issue.nodeId)
      });
      focusIssueField({
        nodeId: issue.nodeId,
        sectionKey: issue.sectionKey ?? null,
        fieldKey: issue.fieldKey ?? null
      });
    }
  };
}
