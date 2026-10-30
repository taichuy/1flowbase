import { getNodeAbsolutePosition } from '../../lib/document/transforms/container-geometry';
import { useEffect, useRef } from 'react';

import {
  createNextNodeId,
  createNodeDocumentWithCountedAlias
} from '../../lib/document/node-factory';
import { isLlmToolSourceHandle } from '../../lib/llm-node-config';
import type { NodePickerOption } from '../../lib/plugin-node-definitions';
import {
  connectNodeFromSource,
  connectNodes,
  insertNodeOnEdge,
  removeEdge,
  reconnectEdge,
  type EdgeConnection,
  validateConnection,
  validateVisibleInternalLlmToolConnection
} from '../../lib/document/transforms/edge';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import { selectWorkingDocument } from '../../store/editor/selectors';

export function useEdgeInteractions() {
  const document = useAgentFlowEditorStore(selectWorkingDocument);
  const connectingPayload = useAgentFlowEditorStore(
    (state) => state.connectingPayload
  );
  const connectingPayloadRef = useRef(connectingPayload);
  const setWorkingDocument = useAgentFlowEditorStore(
    (state) => state.setWorkingDocument
  );
  const setSelection = useAgentFlowEditorStore((state) => state.setSelection);
  const setPanelState = useAgentFlowEditorStore((state) => state.setPanelState);
  const setInteractionState = useAgentFlowEditorStore(
    (state) => state.setInteractionState
  );
  const nodePickerState = useAgentFlowEditorStore(
    (state) => state.nodePickerState
  );

  useEffect(() => {
    connectingPayloadRef.current = connectingPayload;
  }, [connectingPayload]);

  function closeNodePicker() {
    setPanelState({
      nodePickerState: {
        open: false,
        anchorNodeId: null,
        anchorEdgeId: null,
        anchorCanvasPosition: null
      }
    });
  }

  function clearConnectingPayload() {
    setInteractionState({
      connectingPayload: {
        sourceNodeId: null,
        sourceHandleId: null,
        sourceNodeType: null
      }
    });
    connectingPayloadRef.current = {
      sourceNodeId: null,
      sourceHandleId: null,
      sourceNodeType: null
    };
  }

  return {
    connect(connection: Parameters<typeof connectNodes>[1]['connection']) {
      const nextDocument = connectNodes(document, {
        connection
      });

      closeNodePicker();
      clearConnectingPayload();

      if (nextDocument === document) {
        return;
      }

      setWorkingDocument(nextDocument);
    },
    reconnect(
      edgeId: string,
      connection: Parameters<typeof reconnectEdge>[1]['connection']
    ) {
      const nextDocument = reconnectEdge(document, {
        edgeId,
        connection
      });

      if (nextDocument === document) {
        return;
      }

      setWorkingDocument(nextDocument);
      setSelection({
        selectedEdgeId: edgeId,
        selectedNodeId: null,
        selectedNodeIds: []
      });
    },
    insertOnEdge(edgeId: string, option: NodePickerOption) {
      const nextNode = createNodeDocumentWithCountedAlias(
        document,
        option,
        createNextNodeId(document, option)
      );
      const nextDocument = insertNodeOnEdge(document, {
        edgeId,
        node: nextNode
      });

      setWorkingDocument(nextDocument);
      setSelection({
        selectedNodeId: nextNode.id,
        selectedNodeIds: [nextNode.id],
        selectedEdgeId: null
      });
    },
    insertFromConnection(option: NodePickerOption) {
      const sourceNodeId = connectingPayloadRef.current.sourceNodeId;
      const sourceHandleId = connectingPayloadRef.current.sourceHandleId;
      const anchorCanvasPosition = nodePickerState.anchorCanvasPosition;
      const zoom = document.editor.viewport.zoom || 1;

      if (!sourceNodeId || !anchorCanvasPosition) {
        return;
      }

      const sourceNode = document.graph.nodes.find(
        (node) => node.id === sourceNodeId
      );
      const parentPosition = getNodeAbsolutePosition(
        document,
        sourceNode?.containerId ?? null
      );
      const flowPosition = {
        x: Math.round(
          (anchorCanvasPosition.x - document.editor.viewport.x) / zoom -
            parentPosition.x
        ),
        y: Math.round(
          (anchorCanvasPosition.y - document.editor.viewport.y) / zoom -
            parentPosition.y
        )
      };
      const nextNode = createNodeDocumentWithCountedAlias(
        document,
        option,
        createNextNodeId(document, option),
        flowPosition.x,
        flowPosition.y
      );
      const nextDocument = connectNodeFromSource(document, {
        sourceNodeId,
        sourceHandleId,
        node: nextNode
      });

      if (nextDocument === document) {
        return;
      }

      setWorkingDocument(nextDocument);
      setSelection({
        selectedNodeId: nextNode.id,
        selectedNodeIds: [nextNode.id],
        selectedEdgeId: null
      });
      closeNodePicker();
      clearConnectingPayload();
    },
    startConnection(payload: {
      nodeId: string | null;
      handleType: 'source' | 'target' | null;
      handleId: string | null;
    }) {
      if (!payload.nodeId || payload.handleType !== 'source') {
        return;
      }

      const sourceNode = document.graph.nodes.find(
        (node) => node.id === payload.nodeId
      );

      setPanelState({
        nodePickerState: {
          open: false,
          anchorNodeId: null,
          anchorEdgeId: null,
          anchorCanvasPosition: null
        }
      });
      setInteractionState({
        connectingPayload: {
          sourceNodeId: payload.nodeId,
          sourceHandleId: payload.handleId,
          sourceNodeType: sourceNode?.type ?? null
        }
      });
      connectingPayloadRef.current = {
        sourceNodeId: payload.nodeId,
        sourceHandleId: payload.handleId,
        sourceNodeType: sourceNode?.type ?? null
      };
    },
    finishConnectionOnPane(position: { x: number; y: number }) {
      const sourceNodeId = connectingPayloadRef.current.sourceNodeId;

      if (!sourceNodeId) {
        return;
      }

      setPanelState({
        nodePickerState: {
          open: true,
          anchorNodeId: sourceNodeId,
          anchorEdgeId: null,
          anchorCanvasPosition: position
        }
      });
    },
    cancelConnection() {
      clearConnectingPayload();
    },
    isValidConnection(connection: EdgeConnection) {
      if (isLlmToolSourceHandle(connection.sourceHandle)) {
        return validateVisibleInternalLlmToolConnection(document, connection);
      }

      return validateConnection(document, connection);
    },
    remove(edgeId: string) {
      const nextDocument = removeEdge(document, {
        edgeId
      });

      if (nextDocument === document) {
        return;
      }

      setWorkingDocument(nextDocument);
      setSelection({
        selectedEdgeId: null
      });
    }
  };
}
