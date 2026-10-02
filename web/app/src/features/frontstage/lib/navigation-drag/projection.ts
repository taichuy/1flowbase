import type { FrontStageTreeNode } from '../page-tree';
import { findNodeById } from '../page-tree';

export type NavigationDropPosition = 'before' | 'inside' | 'after';

// Interval hit testing uses the stable row bounds, never the projected slot.
export function projectNavigationPosition(
  coordinate: number,
  start: number,
  size: number,
  acceptsChildren: boolean
): NavigationDropPosition | null {
  if (
    !Number.isFinite(coordinate) ||
    !Number.isFinite(start) ||
    !Number.isFinite(size) ||
    size <= 0
  )
    return null;
  const ratio = (coordinate - start) / size;
  if (acceptsChildren && ratio > 0.28 && ratio < 0.72) return 'inside';
  return ratio <= 0.5 ? 'before' : 'after';
}

export function canProjectNavigationMove(
  nodes: FrontStageTreeNode[],
  sourceId: string,
  targetId: string,
  position: NavigationDropPosition
): boolean {
  const source = findNodeById(nodes, sourceId);
  const target = findNodeById(nodes, targetId);
  return Boolean(
    source &&
    target &&
    sourceId !== targetId &&
    !findNodeById(source.children ?? [], targetId) &&
    (position !== 'inside' || target.kind === 'group')
  );
}
