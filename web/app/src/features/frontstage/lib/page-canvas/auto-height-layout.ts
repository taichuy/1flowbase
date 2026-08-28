import { verticalCompactor, type Layout } from 'react-grid-layout';

import { pixelsToFrontstageGridRows } from '../responsive-grid-layout';

export const FRONTSTAGE_AUTO_HEIGHT_SETTLE_MS = 250;
export const FRONTSTAGE_AUTO_HEIGHT_SETTLE_FRAMES = 3;

interface FrontstageAutoHeightRecord {
  identity: string | null;
  observedRows: number;
  committedRows: number | null;
  changedAtMs: number;
  stableFrames: number;
}

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

/** Accumulates animated measurements and exposes only stable grid-row changes. */
export class FrontstageAutoHeightBatch {
  private readonly records = new Map<string, FrontstageAutoHeightRecord>();
  private readonly pendingBlockIds = new Set<string>();
  private readonly settleMs: number;
  private readonly settleFrames: number;

  constructor({
    settleMs = 0,
    settleFrames = 1
  }: { settleMs?: number; settleFrames?: number } = {}) {
    this.settleMs = Math.max(0, settleMs);
    this.settleFrames = Math.max(1, Math.round(settleFrames));
  }

  measure(
    blockId: string,
    height: number,
    identity: string | null = null,
    nowMs = performance.now()
  ): void {
    if (!Number.isFinite(height) || height <= 0) return;
    const observedRows = pixelsToFrontstageGridRows(height);
    const current = this.records.get(blockId);
    if (!current || current.identity !== identity) {
      this.records.set(blockId, {
        identity,
        observedRows,
        committedRows: null,
        changedAtMs: nowMs,
        stableFrames: 0
      });
      this.pendingBlockIds.add(blockId);
      return;
    }
    if (current.observedRows !== observedRows) {
      current.observedRows = observedRows;
      current.changedAtMs = nowMs;
      current.stableFrames = 0;
    }
    if (current.committedRows !== observedRows) {
      this.pendingBlockIds.add(blockId);
    }
  }

  commit(
    currentRows: Readonly<Record<string, number>>,
    nowMs = performance.now()
  ): Record<string, number> {
    let nextRows: Record<string, number> | null = null;
    for (const blockId of [...this.pendingBlockIds]) {
      const record = this.records.get(blockId);
      if (!record) continue;
      record.stableFrames += 1;
      if (
        nowMs - record.changedAtMs < this.settleMs ||
        record.stableFrames < this.settleFrames
      ) {
        continue;
      }
      const rows = record.observedRows;
      if (currentRows[blockId] !== rows) {
        nextRows ??= { ...currentRows };
        nextRows[blockId] = rows;
      }
      record.committedRows = rows;
      this.pendingBlockIds.delete(blockId);
    }
    return nextRows ?? currentRows;
  }

  hasPendingMeasurements(): boolean {
    return this.pendingBlockIds.size > 0;
  }
}
