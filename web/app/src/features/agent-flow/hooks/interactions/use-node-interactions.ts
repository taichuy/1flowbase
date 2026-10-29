import { getContainerPathForNode } from '../../lib/document/transforms/container';
import {
  createNextNodeId,
  createNodeDocumentWithCountedAlias
} from '../../lib/document/node-factory';
import {
  insertNodeAfter,
  removeNodeSubgraph,
  replaceNodeWithOption
} from '../../lib/document/transforms/node';
import type { NodePickerOption } from '../../lib/plugin-node-definitions';
import { useContainerNavigation } from './use-container-navigation';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import {
  selectSelectedNodeId,
  selectWorkingDocument
} from '../../store/editor/selectors';

export function useNodeInteractions() {
  const document = useAgentFlowEditorStore(selectWorkingDocument);
  const setWorkingDocument = useAgentFlowEditorStore(
    (state) => state.setWorkingDocument
  );
  const setSelection = useAgentFlowEditorStore((state) => state.setSelection);
  const setPanelState = useAgentFlowEditorStore((state) => state.setPanelState);
  const setInteractionState = useAgentFlowEditorStore(
    (state) => state.setInteractionState
  );
  const selectedNodeId = useAgentFlowEditorStore(selectSelectedNodeId);
  const navigation = useContainerNavigation();

  return {
    selectNode(nodeId: string | null) {
      setInteractionState({
        activeContainerPath: getContainerPathForNode(document, nodeId)
      });
      setSelection({
        selectedNodeId: nodeId,
        selectedNodeIds: nodeId ? [nodeId] : [],
        selectedEdgeId: null
      });
      setPanelState({
        nodePickerState: {
          open: false,
          anchorNodeId: null,
          anchorEdgeId: null,
          anchorCanvasPosition: null
        }
      });
    },
    openNodePicker(nodeId: string, sourceHandleId: string | null = null) {
      const sourceNode = document.graph.nodes.find(
        (node) => node.id === nodeId
      );

      setInteractionState({
        connectingPayload: {
          sourceNodeId: nodeId,
          sourceHandleId,
          sourceNodeType: sourceNode?.type ?? null
        }
      });
      setPanelState({
        nodePickerState: {
          open: true,
          anchorNodeId: nodeId,
          anchorEdgeId: null,
          anchorCanvasPosition: null
        }
      });
    },
    closeNodePicker() {
      setInteractionState({
        connectingPayload: {
          sourceNodeId: null,
          sourceHandleId: null,
          sourceNodeType: null
        }
      });
      setPanelState({
        nodePickerState: {
          open: false,
          anchorNodeId: null,
          anchorEdgeId: null,
          anchorCanvasPosition: null
        }
      });
    },
    insertAfterNode(
      anchorNodeId: string,
      option: NodePickerOption,
      sourceHandleId: string | null = null
    ) {
      const anchorNode = document.graph.nodes.find(
        (node) => node.id === anchorNodeId
      );

      if (!anchorNode) {
        return;
      }

      const nextNode = createNodeDocumentWithCountedAlias(
        document,
        option,
        createNextNodeId(document, option),
        anchorNode.position.x + 280,
        anchorNode.position.y
      );
      const nextDocument = insertNodeAfter(
        document,
        anchorNodeId,
        nextNode,
        sourceHandleId
      );

      setWorkingDocument(nextDocument);
      setSelection({
        selectedNodeId: nextNode.id,
        selectedNodeIds: [nextNode.id]
      });
      setInteractionState({
        connectingPayload: {
          sourceNodeId: null,
          sourceHandleId: null,
          sourceNodeType: null
        }
      });
      setPanelState({
        nodePickerState: {
          open: false,
          anchorNodeId: null,
          anchorEdgeId: null,
          anchorCanvasPosition: null
        }
      });
    },
    replaceNode(nodeId: string, option: NodePickerOption) {
      const nextDocument = replaceNodeWithOption(document, { nodeId, option });

      if (nextDocument === document) {
        return;
      }

      setWorkingDocument(nextDocument);
      setSelection({
        selectedNodeId: nodeId,
        selectedNodeIds: [nodeId],
        selectedEdgeId: null
      });
      setInteractionState({
        connectingPayload: {
          sourceNodeId: null,
          sourceHandleId: null,
          sourceNodeType: null
        }
      });
    },
    deleteNode(nodeId: string) {
      const nextDocument = removeNodeSubgraph(document, { nodeId });

      if (nextDocument === document) {
        return;
      }

      setWorkingDocument(nextDocument);

      if (
        selectedNodeId &&
        !nextDocument.graph.nodes.some((node) => node.id === selectedNodeId)
      ) {
        setSelection({
          selectedNodeId: null,
          selectedNodeIds: [],
          selectedEdgeId: null,
          focusedFieldKey: null,
          openInspectorSectionKey: null
        });
      }

      setInteractionState({
        pendingLocateNodeId: null,
        connectingPayload: {
          sourceNodeId: null,
          sourceHandleId: null,
          sourceNodeType: null
        }
      });
    },
    openContainer(nodeId: string) {
      navigation.openContainer(nodeId);
    }
  };
}
