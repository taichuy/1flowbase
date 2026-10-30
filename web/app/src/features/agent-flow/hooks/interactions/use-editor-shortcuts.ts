import { removeEdge } from '../../lib/document/transforms/edge';
import { useEffect, useRef } from 'react';

import { useNodeDetailActions } from './use-node-detail-actions';
import {
  duplicateNodeSubgraph,
  getDuplicatedNodeId
} from '../../lib/document/transforms/duplicate';
import { useAgentFlowEditorStore } from '../../store/editor/provider';

export function useEditorShortcuts() {
  const workingDocument = useAgentFlowEditorStore(
    (state) => state.workingDocument
  );
  const selectedNodeId = useAgentFlowEditorStore(
    (state) => state.selectedNodeId
  );
  const selectedEdgeId = useAgentFlowEditorStore(
    (state) => state.selectedEdgeId
  );
  const setSelection = useAgentFlowEditorStore((state) => state.setSelection);
  const setWorkingDocument = useAgentFlowEditorStore(
    (state) => state.setWorkingDocument
  );
  const restoreNextDocument = useAgentFlowEditorStore(
    (state) => state.restoreNextDocument
  );
  const restorePreviousDocument = useAgentFlowEditorStore(
    (state) => state.restorePreviousDocument
  );
  const setInteractionState = useAgentFlowEditorStore(
    (state) => state.setInteractionState
  );
  const setPanelState = useAgentFlowEditorStore((state) => state.setPanelState);
  const detailActions = useNodeDetailActions();
  const copiedNodeIdRef = useRef<string | null>(null);

  useEffect(() => {
    function isEditableTarget(target: EventTarget | null) {
      if (!(target instanceof HTMLElement)) {
        return false;
      }

      const isContentEditable =
        target.isContentEditable ||
        target.closest('[contenteditable="true"]') !== null;
      const isEditorSurface =
        target.closest('.monaco-editor') !== null ||
        target.closest('.cm-editor') !== null ||
        target.closest('[role="textbox"]') !== null;

      return (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        isContentEditable ||
        isEditorSurface
      );
    }

    function handleKeyDown(event: KeyboardEvent) {
      const shortcutKey = event.key.toLowerCase();
      const isModifierShortcut = event.ctrlKey || event.metaKey;

      if (isModifierShortcut) {
        if (isEditableTarget(event.target)) {
          return;
        }

        if (shortcutKey === 'c') {
          if (!selectedNodeId) {
            return;
          }

          event.preventDefault();
          copiedNodeIdRef.current = selectedNodeId;
          return;
        }

        if (shortcutKey === 'v') {
          const copiedNodeId = copiedNodeIdRef.current;

          if (!copiedNodeId) {
            return;
          }

          const duplicatedNodeId = getDuplicatedNodeId(
            workingDocument.graph.nodes.map((node) => node.id),
            copiedNodeId
          );
          const nextDocument = duplicateNodeSubgraph(workingDocument, {
            nodeId: copiedNodeId
          });

          if (nextDocument === workingDocument) {
            return;
          }

          event.preventDefault();
          setWorkingDocument(nextDocument);
          setSelection({
            selectedNodeId: duplicatedNodeId,
            selectedNodeIds: [duplicatedNodeId],
            selectedEdgeId: null
          });
          return;
        }

        if ((shortcutKey === 'z' && event.shiftKey) || shortcutKey === 'y') {
          event.preventDefault();
          restoreNextDocument();
          return;
        }

        if (shortcutKey === 'z' && !event.shiftKey) {
          event.preventDefault();
          restorePreviousDocument();
          return;
        }
      }

      if (event.key === 'Delete' || event.key === 'Backspace') {
        if (isEditableTarget(event.target)) {
          return;
        }

        event.preventDefault();

        if (selectedNodeId) {
          detailActions.deleteSelectedNode();
          return;
        }

        if (selectedEdgeId) {
          const nextDocument = removeEdge(workingDocument, {
            edgeId: selectedEdgeId
          });

          if (nextDocument !== workingDocument) {
            setWorkingDocument(nextDocument);
          }

          setSelection({
            selectedEdgeId: null
          });
        }

        return;
      }

      if (event.key !== 'Escape') {
        return;
      }

      if (isEditableTarget(event.target)) {
        return;
      }

      setSelection({
        selectedNodeId: null,
        selectedEdgeId: null,
        selectedNodeIds: [],
        focusedFieldKey: null,
        openInspectorSectionKey: null
      });
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
          sourceNodeId: null,
          sourceHandleId: null,
          sourceNodeType: null
        }
      });
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    detailActions,
    selectedNodeId,
    selectedEdgeId,
    restorePreviousDocument,
    restoreNextDocument,
    setInteractionState,
    setPanelState,
    setSelection,
    setWorkingDocument,
    workingDocument
  ]);
}
