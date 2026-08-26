import { describe, expect, test } from 'vitest';

import {
  FrontstageAutoHeightBatch,
  resolveFrontstageAutoHeightScrollDelta
} from '../../lib/page-canvas/auto-height-layout';
import { pixelsToFrontstageGridRows } from '../../lib/responsive-grid-layout';

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
