import { verticalCompactor, type Layout } from 'react-grid-layout';

import { pixelsToFrontstageGridRows } from '../responsive-grid-layout';

export function resolveFrontstageAutoHeightScrollDelta({
  anchorBlockId,
  columns,
  compact,
  currentLayout,
  nextLayout,
  rowHeight,
  rowMargin
}: {
  anchorBlockId: string;
  columns: number;
  compact: boolean;
  currentLayout: Layout;
  nextLayout: Layout;
  rowHeight: number;
  rowMargin: number;
}): number {
  const current = compact
    ? verticalCompactor.compact(currentLayout, columns)
    : currentLayout;
  const next = compact
    ? verticalCompactor.compact(nextLayout, columns)
    : nextLayout;
  const currentAnchor = current.find((item) => item.i === anchorBlockId);
  const nextAnchor = next.find((item) => item.i === anchorBlockId);
  return currentAnchor && nextAnchor
    ? (nextAnchor.y - currentAnchor.y) * (rowHeight + rowMargin)
    : 0;
}

/** Collects one animation frame of auto-height measurements by block. */
export class FrontstageAutoHeightBatch {
  private readonly pendingRows = new Map<string, number>();

  measure(blockId: string, height: number): void {
    if (!Number.isFinite(height) || height <= 0) return;
    this.pendingRows.set(blockId, pixelsToFrontstageGridRows(height));
  }

  commit(
    currentRows: Readonly<Record<string, number>>
  ): Record<string, number> {
    let nextRows: Record<string, number> | null = null;
    for (const [blockId, rows] of this.pendingRows) {
      if (currentRows[blockId] === rows) continue;
      nextRows ??= { ...currentRows };
      nextRows[blockId] = rows;
    }
    this.pendingRows.clear();
    return nextRows ?? currentRows;
  }
}
