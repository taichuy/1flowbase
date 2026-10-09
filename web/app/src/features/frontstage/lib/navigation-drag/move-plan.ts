import type { MoveFrontstageNodeInput } from '../../api/page-tree';
import { findNodeById, type FrontStageTreeNode } from '../page-tree';
import {
  canProjectNavigationMove,
  type NavigationDropPosition
} from './projection';

export type NavigationMoveDraft = {
  targetNodeId: string | null;
  position: NavigationDropPosition;
};

export function findSiblingContext(
  nodes: FrontStageTreeNode[],
  targetNodeId: string,
  parentId: string | null = null
): {
  parentId: string | null;
  siblings: FrontStageTreeNode[];
  index: number;
} | null {
  const index = nodes.findIndex((node) => node.id === targetNodeId);
  if (index >= 0) return { parentId, siblings: nodes, index };
  for (const node of nodes) {
    const context = findSiblingContext(
      node.children ?? [],
      targetNodeId,
      node.id
    );
    if (context) return context;
  }
  return null;
}

/** The preview describes a destination and anchor; the backend owns its rank. */
export function resolveNavigationMove(
  nodes: FrontStageTreeNode[],
  sourceId: string,
  draft: NavigationMoveDraft
): MoveFrontstageNodeInput | null {
  const source = findNodeById(nodes, sourceId);
  if (!source) return null;
  let parentId: string | null;
  let siblings: FrontStageTreeNode[];
  if (draft.targetNodeId === null) {
    if (draft.position !== 'inside') return null;
    parentId = null;
    siblings = nodes;
  } else {
    const target = findNodeById(nodes, draft.targetNodeId);
    const context = findSiblingContext(nodes, draft.targetNodeId);
    if (
      !target ||
      !context ||
      !canProjectNavigationMove(nodes, sourceId, target.id, draft.position)
    )
      return null;
    if (
      source.placement &&
      target.placement &&
      source.placement !== target.placement
    ) {
      if (
        !(
          draft.position === 'inside' &&
          source.placement === 'sidebar' &&
          target.placement === 'topbar' &&
          context.parentId === null
        )
      )
        return null;
    }
    if (draft.position !== 'inside') {
      return {
        parentId: context.parentId,
        ...(draft.position === 'before'
          ? { before_id: target.id }
          : { after_id: target.id })
      };
    }
    parentId = target.id;
    siblings = target.children ?? [];
  }
  const destination = siblings.filter(
    (node) =>
      node.id !== sourceId &&
      (!source.placement ||
        !node.placement ||
        node.placement === source.placement)
  );
  const last = destination.at(-1);
  return last ? { parentId, after_id: last.id } : { parentId, rank: '001000' };
}
