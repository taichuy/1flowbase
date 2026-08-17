import type { NodeChange } from '@xyflow/react';
import { useCallback, useMemo } from 'react';

import { arrangeCanvasLeftToRight } from '../../lib/document/transforms/layout';
import { moveNodes } from '../../lib/document/transforms/node';
import { setViewport } from '../../lib/document/transforms/viewport';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import { selectActiveContainerId } from '../../store/editor/selectors';

function getPositionChanges(changes: NodeChange[]) {
  return changes.filter(
    (
      change
    ): change is NodeChange & {
      id: string;
      dragging?: boolean;
      position: { x: number; y: number };
    } =>
      change.type === 'position' &&
      'id' in change &&
      'position' in change &&
      Boolean(change.position)
  );
}

function toPositions(
  changes: Array<{
    id: string;
    position: { x: number; y: number };
  }>
) {
  return Object.fromEntries(
    changes.map((change) => [change.id, change.position])
  );
}

export function useCanvasInteractions() {
  const activeContainerId = useAgentFlowEditorStore(selectActiveContainerId);
  const setWorkingDocument = useAgentFlowEditorStore(
    (state) => state.setWorkingDocument
  );

  const onNodesChange = useCallback(
    (changes: NodeChange[]) => {
      const committedChanges = getPositionChanges(changes).filter(
        (change) => change.dragging !== true
      );

      if (committedChanges.length === 0) {
        return;
      }

      const committedPositions = toPositions(committedChanges);

      setWorkingDocument((currentDocument) =>
        moveNodes(currentDocument, committedPositions)
      );
    },
    [setWorkingDocument]
  );

  const commitViewportChange = useCallback(
    (viewport: { x: number; y: number; zoom: number }) => {
      setWorkingDocument((currentDocument) =>
        setViewport(currentDocument, {
          x: viewport.x,
          y: viewport.y,
          zoom: viewport.zoom
        })
      );
    },
    [setWorkingDocument]
  );

  const arrangeCanvas = useCallback(() => {
    setWorkingDocument((currentDocument) =>
      arrangeCanvasLeftToRight(currentDocument, activeContainerId)
    );
  }, [activeContainerId, setWorkingDocument]);

  return useMemo(
    () => ({
      onNodesChange,
      commitViewportChange,
      arrangeCanvas
    }),
    [arrangeCanvas, commitViewportChange, onNodesChange]
  );
}
