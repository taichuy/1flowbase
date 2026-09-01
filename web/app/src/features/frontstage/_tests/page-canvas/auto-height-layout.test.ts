import { describe, expect, test } from 'vitest';

import {
  FRONTSTAGE_AUTO_HEIGHT_SETTLE_MS,
  FrontstageAutoHeightBatch,
  resolveFrontstageAutoHeightScrollDelta
} from '../../lib/page-canvas/auto-height-layout';
import { pixelsToFrontstageGridRows } from '../../lib/responsive-grid-layout';

const domMeasurement = (epoch: number) =>
  ({ epoch, source: 'dom-intrinsic' }) as const;
const explicitMeasurement = (epoch: number) =>
  ({ epoch, source: 'runtime-explicit' }) as const;

describe('FrontstageAutoHeightBatch', () => {
  test('AC-005 coalesces measurements by block and preserves state identity when rows do not change', () => {
    const batch = new FrontstageAutoHeightBatch();
    batch.measure('hero', 319);
    batch.measure('hero', 320);
    batch.measure('details', 517);

    const committed = batch.commit({ hero: pixelsToFrontstageGridRows(319) });
    expect(committed).toEqual({
      hero: pixelsToFrontstageGridRows(320),
      details: pixelsToFrontstageGridRows(517)
    });

    batch.measure('hero', 319.5);
    expect(batch.commit(committed)).toBe(committed);
  });

  test('AC-004 keeps animated measurements pending and commits only the stable terminal rows', () => {
    const batch = new FrontstageAutoHeightBatch({
      settleMs: 64,
      settleFrames: 3
    });
    const initial = { hero: pixelsToFrontstageGridRows(434) };

    batch.measure('hero', 400, domMeasurement(1), 0);
    expect(batch.commit(initial, 16)).toBe(initial);
    expect(batch.hasPendingMeasurements()).toBe(true);

    batch.measure('hero', 350, domMeasurement(1), 32);
    expect(batch.commit(initial, 64)).toBe(initial);

    batch.measure('hero', 296, domMeasurement(1), 80);
    expect(batch.commit(initial, 128)).toBe(initial);
    expect(batch.commit(initial, 144)).toBe(initial);
    const committed = batch.commit(initial, 160);
    expect(committed).toEqual({ hero: pixelsToFrontstageGridRows(296) });
    expect(batch.hasPendingMeasurements()).toBe(false);
  });

  test('AC-1926-004 invalidates an accumulated measurement when the layout epoch advances', () => {
    const batch = new FrontstageAutoHeightBatch({ settleMs: 0 });
    batch.measure('hero', 434, domMeasurement(1), 0);
    const first = batch.commit({}, 0);
    expect(first).toEqual({ hero: pixelsToFrontstageGridRows(434) });
    expect(batch.takeCommittedMeasurements()).toEqual([
      {
        blockId: 'hero',
        epoch: 1,
        source: 'dom-intrinsic',
        rows: pixelsToFrontstageGridRows(434)
      }
    ]);

    batch.measure('hero', 296, domMeasurement(2), 1);
    expect(batch.commit(first, 1)).toEqual({
      hero: pixelsToFrontstageGridRows(296)
    });
    expect(batch.takeCommittedMeasurements()).toEqual([
      {
        blockId: 'hero',
        epoch: 2,
        source: 'dom-intrinsic',
        rows: pixelsToFrontstageGridRows(296)
      }
    ]);
  });

  test('AC-1926-002 commits an explicit allocation owner even when its row count is unchanged', () => {
    const batch = new FrontstageAutoHeightBatch({ settleMs: 0 });
    batch.measure('hero', 320, domMeasurement(1), 0);
    const rows = batch.commit({}, 0);
    batch.takeCommittedMeasurements();

    batch.measure('hero', 320, explicitMeasurement(1), 1);
    expect(batch.commit(rows, 1)).toBe(rows);
    expect(batch.takeCommittedMeasurements()).toEqual([
      {
        blockId: 'hero',
        epoch: 1,
        source: 'runtime-explicit',
        rows: pixelsToFrontstageGridRows(320)
      }
    ]);
  });

  test('AC-004 does not treat a motion sample gap caused by a long task as terminal stability', () => {
    const batch = new FrontstageAutoHeightBatch({
      settleMs: FRONTSTAGE_AUTO_HEIGHT_SETTLE_MS,
      settleFrames: 3
    });
    const initial = { hero: pixelsToFrontstageGridRows(296) };

    batch.measure('hero', 299, domMeasurement(1), 0);
    expect(batch.commit(initial, 80)).toBe(initial);
    expect(batch.commit(initial, 160)).toBe(initial);
    expect(batch.commit(initial, 200)).toBe(initial);

    batch.measure('hero', 434, domMeasurement(1), 212);
    expect(batch.commit(initial, 300)).toBe(initial);
    expect(batch.commit(initial, 400)).toBe(initial);
    expect(batch.commit(initial, 462)).toEqual({
      hero: pixelsToFrontstageGridRows(434)
    });
  });

  test('AC-1926-004 ignores a late measurement from a retired epoch', () => {
    const batch = new FrontstageAutoHeightBatch({ settleMs: 0 });
    batch.measure('hero', 296, domMeasurement(2), 0);
    const current = batch.commit({}, 0);
    batch.takeCommittedMeasurements();

    batch.measure('hero', 434, domMeasurement(1), 1);

    expect(batch.commit(current, 1)).toBe(current);
    expect(batch.takeCommittedMeasurements()).toEqual([]);
  });

  test('AC-1926-002 keeps explicit intrinsic reporting authoritative within one epoch', () => {
    const batch = new FrontstageAutoHeightBatch({ settleMs: 0 });
    batch.measure('hero', 320, explicitMeasurement(1), 0);
    const current = batch.commit({}, 0);
    batch.takeCommittedMeasurements();

    batch.measure('hero', 800, domMeasurement(1), 1);

    expect(batch.commit(current, 1)).toBe(current);
    expect(batch.takeCommittedMeasurements()).toEqual([]);
  });
});

describe('resolveFrontstageAutoHeightScrollDelta', () => {
  test('AC-006 derives the anchor correction from compacted grid rows', () => {
    const currentLayout = [
      { i: 'first', x: 0, y: 0, w: 24, h: 22 },
      { i: 'second', x: 0, y: 107, w: 24, h: 22 },
      { i: 'anchor', x: 0, y: 214, w: 24, h: 100 }
    ];
    const nextLayout = currentLayout.map((item) =>
      item.i === 'first' ? { ...item, h: 102 } : item
    );

    expect(
      resolveFrontstageAutoHeightScrollDelta({
        anchorBlockId: 'anchor',
        columns: 24,
        compact: true,
        currentLayout,
        nextLayout,
        rowHeight: 3,
        rowMargin: 0
      })
    ).toBe(240);
  });

  test('keeps free-layout anchors unchanged and tolerates missing identities', () => {
    const layout = [{ i: 'anchor', x: 0, y: 12, w: 1, h: 20 }];
    expect(
      resolveFrontstageAutoHeightScrollDelta({
        anchorBlockId: 'anchor',
        columns: 1,
        compact: false,
        currentLayout: layout,
        nextLayout: layout,
        rowHeight: 3,
        rowMargin: 0
      })
    ).toBe(0);
    expect(
      resolveFrontstageAutoHeightScrollDelta({
        anchorBlockId: 'missing',
        columns: 1,
        compact: true,
        currentLayout: layout,
        nextLayout: layout,
        rowHeight: 3,
        rowMargin: 0
      })
    ).toBe(0);
  });
});
