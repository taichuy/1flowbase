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
import { solveFrontstageAutomaticLayout } from '../../lib/page-canvas/frontstage-row-layout';

function frontstageBlockFixture(): FrontstageBlockRenderPlanItem {
  return {
    blockId: 'hero',
    rendererVersion: 'v1',
    sourceBlockId: null,
    codeRef: 'hero-code',
    sourceCodeRef: null,
    sourceIndex: 0,
    order: 0,
    renderMode: 'native_react',
    canPrepareNativeReact: true,
    canMountIsolatedIframe: false,
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
    const layouts = createFrontstageResponsiveLayouts([
      frontstageBlockFixture()
    ]);

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
      expect(rows * FRONTSTAGE_GRID_ROW_HEIGHT - height).toBeGreaterThanOrEqual(
        10
      );
      expect(rows * FRONTSTAGE_GRID_ROW_HEIGHT - height).toBeLessThanOrEqual(
        12
      );
    }
  });

  test('AC-005 consumes committed grid rows without re-quantizing pixel noise', () => {
    const autoItem = {
      ...frontstageBlockFixture(),
      blockId: 'auto',
      presentation: { heightMode: 'auto' as const, height: null }
    };

    expect(
      createFrontstageResponsiveLayouts([autoItem], { auto: 173 }).lg?.[0]
    ).toMatchObject({ i: 'auto', h: 173 });
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
    const layouts = createFrontstageResponsiveLayouts([
      frontstageBlockFixture()
    ]);
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
      { i: 'third', x: 0, y: 6, w: 24 }
    ]);
    expect(normalized.xs).toHaveLength(3);
    expect(normalized.xs?.every((item) => item.x === 0 && item.w === 1)).toBe(
      true
    );
  });

  test('AC-001 AC-004 allocates the row maximum height and prefix-sums following rows', () => {
    const normalized = normalizeFrontstageAutomaticResponsiveLayouts({
      lg: [
        { i: 'short', x: 0, y: 0, w: 12, h: 6 },
        { i: 'tall', x: 12, y: 0, w: 12, h: 10 },
        { i: 'following', x: 0, y: 20, w: 24, h: 4 }
      ]
    });

    expect(normalized.lg?.map(({ i, y, h }) => ({ i, y, h }))).toEqual([
      { i: 'short', y: 0, h: 10 },
      { i: 'tall', y: 0, h: 10 },
      { i: 'following', y: 10, h: 4 }
    ]);
  });

  test('AC-1926-001/003 is permutation-invariant and idempotent', () => {
    const input = [
      { i: 'short', x: 0, y: 0, w: 12, h: 6 },
      { i: 'tall', x: 12, y: 0, w: 12, h: 10 },
      { i: 'following', x: 0, y: 20, w: 24, h: 4 }
    ];
    const project = (
      layout: ReturnType<typeof solveFrontstageAutomaticLayout>
    ) =>
      layout
        .map(({ i, x, y, w, h }) => ({ i, x, y, w, h }))
        .sort((left, right) => left.i.localeCompare(right.i));
    const solved = solveFrontstageAutomaticLayout(input, 24);
    const permuted = solveFrontstageAutomaticLayout(
      [input[2]!, input[1]!, input[0]!],
      24
    );

    expect(project(permuted)).toEqual(project(solved));
    expect(project(solveFrontstageAutomaticLayout(solved, 24))).toEqual(
      project(solved)
    );
  });

  test('AC-006 stacks an infeasible multi-member row at a single-column breakpoint', () => {
    const normalized = normalizeFrontstageAutomaticResponsiveLayouts({
      xs: [
        { i: 'short', x: 0, y: 0, w: 1, h: 6 },
        { i: 'tall', x: 0, y: 0, w: 1, h: 10 }
      ]
    });

    expect(
      normalized.xs?.map(({ i, x, y, w, h }) => ({ i, x, y, w, h }))
    ).toEqual([
      { i: 'short', x: 0, y: 0, w: 1, h: 6 },
      { i: 'tall', x: 0, y: 6, w: 1, h: 10 }
    ]);
  });

  test('I1913-AC-001/002 appends an unpositioned Block after tall positioned rows', () => {
    const first = frontstageBlockFixture();
    first.blockId = 'first';
    first.presentation = { heightMode: 'auto', height: null };
    first.layout = {
      order: 0,
      gridColumns: 24,
      verticalGridVersion: 2,
      lg: { x: 0, y: 0, w: 24 }
    };
    const second = frontstageBlockFixture();
    second.blockId = 'second';
    second.presentation = { heightMode: 'auto', height: null };
    second.layout = {
      order: 1,
      gridColumns: 24,
      verticalGridVersion: 2,
      lg: { x: 0, y: 815, w: 24 }
    };
    const created = frontstageBlockFixture();
    created.blockId = 'created';
    created.order = 2;
    created.presentation = { heightMode: 'auto', height: null };
    created.layout = { order: 2, region: 'main' };

    const layouts = createFrontstageResponsiveLayouts(
      [first, second, created],
      { first: 815, second: 844, created: 110 }
    );

    expect(layouts.lg?.map(({ i, y }) => ({ i, y }))).toEqual([
      { i: 'first', y: 0 },
      { i: 'second', y: 815 },
      { i: 'created', y: 1659 }
    ]);
    expect(
      normalizeFrontstageAutomaticResponsiveLayouts(layouts).lg?.map(
        ({ i, y }) => ({ i, y })
      )
    ).toEqual([
      { i: 'first', y: 0 },
      { i: 'second', y: 815 },
      { i: 'created', y: 1659 }
    ]);
  });

  test('I1913-AC-003/004/005 uses each breakpoint row maximum as a monotonic frontier', () => {
    const short = frontstageBlockFixture();
    short.blockId = 'short';
    short.presentation = { heightMode: 'fixed', height: 120 };
    short.layout = {
      order: 0,
      gridColumns: 24,
      verticalGridVersion: 2,
      lg: { x: 0, y: 100, w: 12 },
      md: { x: 0, y: 40, w: 10 },
      sm: { x: 0, y: 10, w: 6 },
      xs: { x: 0, y: 20, w: 1 },
      xxs: { x: 0, y: 30, w: 1 }
    };
    const tall = frontstageBlockFixture();
    tall.blockId = 'tall';
    tall.order = 1;
    tall.presentation = { heightMode: 'fixed', height: 200 };
    tall.layout = {
      order: 1,
      gridColumns: 24,
      verticalGridVersion: 2,
      lg: { x: 12, y: 100, w: 12 },
      md: { x: 10, y: 40, w: 10 },
      sm: { x: 6, y: 10, w: 6 },
      xs: { x: 0, y: 20, w: 1 },
      xxs: { x: 0, y: 30, w: 1 }
    };
    const firstCreated = frontstageBlockFixture();
    firstCreated.blockId = 'created-1';
    firstCreated.order = 2;
    firstCreated.layout = { order: 2, region: 'main' };
    const secondCreated = frontstageBlockFixture();
    secondCreated.blockId = 'created-2';
    secondCreated.order = 3;
    secondCreated.layout = { order: 3, region: 'main' };

    const layouts = createFrontstageResponsiveLayouts([
      short,
      tall,
      firstCreated,
      secondCreated
    ]);

    expect(layouts.lg?.map(({ i, y }) => ({ i, y }))).toEqual([
      { i: 'short', y: 100 },
      { i: 'tall', y: 100 },
      { i: 'created-1', y: 170 },
      { i: 'created-2', y: 280 }
    ]);
    expect(layouts.md?.map(({ i, y }) => ({ i, y }))).toEqual([
      { i: 'short', y: 40 },
      { i: 'tall', y: 40 },
      { i: 'created-1', y: 110 },
      { i: 'created-2', y: 220 }
    ]);
    expect(layouts.sm?.map(({ i, y }) => ({ i, y }))).toEqual([
      { i: 'short', y: 10 },
      { i: 'tall', y: 10 },
      { i: 'created-1', y: 80 },
      { i: 'created-2', y: 190 }
    ]);
    expect(layouts.xs?.map(({ i, y }) => ({ i, y }))).toEqual([
      { i: 'short', y: 20 },
      { i: 'tall', y: 20 },
      { i: 'created-1', y: 90 },
      { i: 'created-2', y: 200 }
    ]);
    expect(layouts.xxs?.map(({ i, y }) => ({ i, y }))).toEqual([
      { i: 'short', y: 30 },
      { i: 'tall', y: 30 },
      { i: 'created-1', y: 100 },
      { i: 'created-2', y: 210 }
    ]);
  });
});
