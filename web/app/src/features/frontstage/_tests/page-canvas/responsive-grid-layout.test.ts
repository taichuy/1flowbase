import { describe, expect, test } from 'vitest';

import {
  FRONTSTAGE_GRID_ROW_GAP,
  FRONTSTAGE_GRID_ROW_HEIGHT,
  FRONTSTAGE_GRID_VERTICAL_MARGIN,
  createFrontstagePersistedGridLayout,
  createFrontstageResponsiveLayouts,
  frontstageGridRowsToPixels,
  normalizeFrontstageAutomaticResponsiveLayouts,
  pixelsToFrontstageGridRows,
  replaceFrontstageBreakpointLayout
} from '../../lib/responsive-grid-layout';
import type { FrontstageBlockRenderPlanItem } from '../../lib/page-canvas/render-plan';

function frontstageBlockFixture(): FrontstageBlockRenderPlanItem {
  return {
    blockId: 'hero',
    rendererVersion: 'v1',
    sourceBlockId: null,
    codeRef: 'hero-code',
    sourceCodeRef: null,
    sourceIndex: 0,
    order: 0,
    renderMode: 'restricted_js_block',
    canEnterRestrictedJsRuntime: true,
    fallbackReasons: [],
    catalog: { providerCode: null, installationId: null },
    contribution: { pluginId: null, pluginVersion: null, code: 'hero' },
    runtime: { kind: 'restricted_js', entry: null, hint: 'restricted_js' },
    presentation: { heightMode: 'fixed', height: 320 },
    layout: {
      order: 0,
      lg: { x: 2, y: 3, w: 6, h: 5 },
      sm: { x: 0, y: 1, w: 4, h: 3 }
    },
    props: {}
  };
}

describe('frontstage responsive grid layout', () => {
  test('AC-001/002 migrates legacy width to 24 units and derives mobile single-column layout', () => {
    const layouts = createFrontstageResponsiveLayouts([frontstageBlockFixture()]);

    expect(layouts.lg?.[0]).toMatchObject({
      i: 'hero',
      x: 4,
      y: 44,
      w: 12,
      h: 110
    });
    expect(layouts.sm?.[0]).toMatchObject({
      i: 'hero',
      x: 0,
      y: 15,
      w: 8,
      h: 110
    });
    expect(layouts.xs?.[0]).toMatchObject({ i: 'hero', x: 0, w: 1, h: 110 });
    expect(layouts.md?.[0]).toMatchObject({ i: 'hero', h: 110 });
    expect(createFrontstagePersistedGridLayout(layouts)).toMatchObject({
      hero: {
        gridColumns: 24,
        verticalGridVersion: 2,
        lg: { x: 4, y: 44, w: 12 },
        sm: { x: 0, y: 15, w: 8 }
      }
    });
  });

  test('AC-010 keeps auto-height quantization within 8px and persists the vertical grid version', () => {
    expect(FRONTSTAGE_GRID_ROW_HEIGHT).toBe(3);
    expect(FRONTSTAGE_GRID_ROW_GAP).toBe(10);
    expect(FRONTSTAGE_GRID_VERTICAL_MARGIN).toBe(0);

    for (const height of [120, 319, 320, 321, 517]) {
      const rows = pixelsToFrontstageGridRows(height);
      const quantizedHeight = frontstageGridRowsToPixels(rows);

      expect(quantizedHeight).toBeGreaterThanOrEqual(height);
      expect(quantizedHeight - height).toBeLessThanOrEqual(2);
      expect(
        rows * FRONTSTAGE_GRID_ROW_HEIGHT - height
      ).toBeGreaterThanOrEqual(10);
      expect(rows * FRONTSTAGE_GRID_ROW_HEIGHT - height).toBeLessThanOrEqual(
        12
      );
    }
  });

  test('AC-010 preserves the pixel position of persisted layouts when migrating the vertical grid', () => {
    const legacyItem = frontstageBlockFixture();
    legacyItem.layout = {
      ...legacyItem.layout,
      gridColumns: 24,
      lg: { x: 4, y: 3, w: 12 }
    };
    const currentItem = frontstageBlockFixture();
    currentItem.blockId = 'current';
    currentItem.layout = {
      ...currentItem.layout,
      gridColumns: 24,
      verticalGridVersion: 2,
      lg: { x: 4, y: 44, w: 12 }
    };

    const layouts = createFrontstageResponsiveLayouts([
      legacyItem,
      currentItem
    ]);

    expect(layouts.lg?.map(({ i, y }) => ({ i, y }))).toEqual([
      { i: 'hero', y: 44 },
      { i: 'current', y: 44 }
    ]);
  });

  test('AC-003 exposes only horizontal resize for auto height and vertical resize for fixed height', () => {
    const autoItem = {
      ...frontstageBlockFixture(),
      blockId: 'auto',
      presentation: { heightMode: 'auto' as const, height: null }
    };
    const layouts = createFrontstageResponsiveLayouts([
      autoItem,
      frontstageBlockFixture()
    ]);

    expect(layouts.lg?.[0]).toMatchObject({
      i: 'auto',
      resizeHandles: ['e', 'w']
    });
    expect(layouts.lg?.[1]).toMatchObject({
      i: 'hero',
      resizeHandles: ['e', 'w', 's', 'se', 'sw']
    });
  });

  test('persists the drag-stop layout for the active breakpoint without changing the others', () => {
    const layouts = createFrontstageResponsiveLayouts([frontstageBlockFixture()]);
    const nextLayouts = replaceFrontstageBreakpointLayout(layouts, 'lg', [
      { i: 'hero', x: 0, y: 8, w: 12, h: 5 }
    ]);

    expect(nextLayouts.lg).toEqual([{ i: 'hero', x: 0, y: 8, w: 12, h: 5 }]);
    expect(nextLayouts.sm).toBe(layouts.sm);
    expect(createFrontstagePersistedGridLayout(nextLayouts)).toMatchObject({
      hero: {
        gridColumns: 24,
        lg: { x: 0, y: 8, w: 12 },
        sm: { x: 0, y: 15, w: 8 }
      }
    });
  });

  test('AC-009 normalizes sparse rows when switching to automatic layout', () => {
    const normalized = normalizeFrontstageAutomaticResponsiveLayouts({
      lg: [
        { i: 'first', x: 0, y: 0, w: 8, h: 6 },
        { i: 'second', x: 16, y: 0, w: 8, h: 6 },
        { i: 'third', x: 4, y: 8, w: 8, h: 6 }
      ],
      xs: [
        { i: 'first', x: 0, y: 0, w: 1, h: 6 },
        { i: 'second', x: 0, y: 8, w: 1, h: 6 },
        { i: 'third', x: 0, y: 16, w: 1, h: 6 }
      ]
    });

    expect(normalized.lg?.map(({ i, x, y, w }) => ({ i, x, y, w }))).toEqual([
      { i: 'first', x: 0, y: 0, w: 12 },
      { i: 'second', x: 12, y: 0, w: 12 },
      { i: 'third', x: 0, y: 8, w: 24 }
    ]);
    expect(normalized.xs).toHaveLength(3);
    expect(normalized.xs?.every((item) => item.x === 0 && item.w === 1)).toBe(
      true
    );
  });
});
