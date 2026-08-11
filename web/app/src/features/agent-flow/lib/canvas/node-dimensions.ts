import type { FlowNodeDocument } from '@1flowbase/flow-schema';

import { getIfElseBranchesFromBindings } from '../if-else-branches';
import {
  ERROR_BRANCH_SOURCE_HANDLE,
  nodeUsesErrorBranch
} from '../policy/node-error-policy';

export const CANVAS_NODE_WIDTH = 196;
export const CANVAS_NODE_BASE_HEIGHT = 96;
export const CANVAS_NODE_SOURCE_HANDLE_PITCH = 32;

export function getCanvasNodeMinimumHeight(sourceHandleCount: number) {
  const visibleSourceHandleCount = Math.max(1, sourceHandleCount);

  return Math.max(
    CANVAS_NODE_BASE_HEIGHT,
    (visibleSourceHandleCount + 1) * CANVAS_NODE_SOURCE_HANDLE_PITCH
  );
}

export function getCanvasNodeSourceHandleCount(
  node: Pick<FlowNodeDocument, 'type' | 'config' | 'bindings'>
) {
  const branchHandleIds =
    node.type === 'if_else'
      ? (getIfElseBranchesFromBindings(node.bindings) ?? []).map(
          (branch) => branch.sourceHandle
        )
      : [];
  const primaryHandleIds: Array<string | null> =
    branchHandleIds.length > 0 ? branchHandleIds : [null];
  const addsErrorHandle =
    nodeUsesErrorBranch(node) &&
    !primaryHandleIds.includes(ERROR_BRANCH_SOURCE_HANDLE);

  return primaryHandleIds.length + (addsErrorHandle ? 1 : 0);
}

export function getCanvasNodeMinimumHeightForDocument(
  node: Pick<FlowNodeDocument, 'type' | 'config' | 'bindings'>
) {
  return getCanvasNodeMinimumHeight(getCanvasNodeSourceHandleCount(node));
}
