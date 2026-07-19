import { describe, expect, test } from 'vitest';

import {
  createFrontstagePersistedGridLayout,
  createFrontstageResponsiveLayouts
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
    layout: {
      order: 0,
      lg: { x: 2, y: 3, w: 6, h: 5 },
      sm: { x: 0, y: 1, w: 4, h: 3 }
    },
    props: {}
  };
}

describe('frontstage responsive grid layout', () => {
  test('AC-007 restores and persists x/y/w/h for every responsive breakpoint', () => {
    const layouts = createFrontstageResponsiveLayouts([renderItem()]);

    expect(layouts.lg?.[0]).toMatchObject({ i: 'hero', x: 2, y: 3, w: 6, h: 5 });
    expect(layouts.sm?.[0]).toMatchObject({ i: 'hero', x: 0, y: 1, w: 4, h: 3 });
    expect(layouts.md?.[0]).toMatchObject({ i: 'hero', h: 8 });
    expect(createFrontstagePersistedGridLayout(layouts)).toMatchObject({
      hero: {
        lg: { x: 2, y: 3, w: 6, h: 5 },
        sm: { x: 0, y: 1, w: 4, h: 3 }
      }
    });
  });
});
