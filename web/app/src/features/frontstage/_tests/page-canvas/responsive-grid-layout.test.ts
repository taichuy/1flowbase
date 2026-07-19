import { describe, expect, test } from 'vitest';

import {
  createFrontstagePersistedGridLayout,
  createFrontstageResponsiveLayouts,
  replaceFrontstageBreakpointLayout
} from '../../lib/responsive-grid-layout';
import type { FrontstageBlockRenderPlanItem } from '../../lib/page-canvas/render-plan';

function renderItem(): FrontstageBlockRenderPlanItem {
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
    const layouts = createFrontstageResponsiveLayouts([renderItem()]);

    expect(layouts.lg?.[0]).toMatchObject({ i: 'hero', x: 4, y: 3, w: 12, h: 8 });
    expect(layouts.sm?.[0]).toMatchObject({ i: 'hero', x: 0, y: 1, w: 8, h: 8 });
    expect(layouts.xs?.[0]).toMatchObject({ i: 'hero', x: 0, w: 1, h: 8 });
    expect(layouts.md?.[0]).toMatchObject({ i: 'hero', h: 8 });
    expect(createFrontstagePersistedGridLayout(layouts)).toMatchObject({
      hero: {
        gridColumns: 24,
        lg: { x: 4, y: 3, w: 12 },
        sm: { x: 0, y: 1, w: 8 }
      }
    });
  });

  test('AC-003 exposes only horizontal resize for auto height and vertical resize for fixed height', () => {
    const autoItem = {
      ...renderItem(),
      blockId: 'auto',
      presentation: { heightMode: 'auto' as const, height: null }
    };
    const layouts = createFrontstageResponsiveLayouts([autoItem, renderItem()]);

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
    const layouts = createFrontstageResponsiveLayouts([renderItem()]);
    const nextLayouts = replaceFrontstageBreakpointLayout(layouts, 'lg', [
      { i: 'hero', x: 0, y: 8, w: 12, h: 5 }
    ]);

    expect(nextLayouts.lg).toEqual([
      { i: 'hero', x: 0, y: 8, w: 12, h: 5 }
    ]);
    expect(nextLayouts.sm).toBe(layouts.sm);
    expect(createFrontstagePersistedGridLayout(nextLayouts)).toMatchObject({
      hero: {
        gridColumns: 24,
        lg: { x: 0, y: 8, w: 12 },
        sm: { x: 0, y: 1, w: 8 }
      }
    });
  });
});
