/* eslint-disable testing-library/render-result-naming-convention */

import { describe, expect, test } from 'vitest';

import {
  createFrontstagePageDocument,
  type FrontstageBlockInstance
} from '../../lib/page-document';
import {
  createFrontstageBlockRenderPlanItem,
  createFrontstagePageRenderPlan
} from '../../lib/page-canvas/render-plan';
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';

function createBlock(
  overrides: Partial<FrontstageBlockInstance> = {}
): FrontstageBlockInstance {
  return {
    id: 'hero',
    rendererVersion: 'v1',
    sourceId: 'hero',
    codeRef: 'hero-code',
    sourceCodeRef: 'hero-code',
    catalog: {
      providerCode: 'official',
      installationId: 'installation-1'
    },
    contribution: {
      pluginId: 'official.blocks',
      pluginVersion: '1.0.0',
      code: 'official.hero'
    },
    props: { title: 'Hello' },
    presentation: { heightMode: 'auto', height: null },
    layout: { order: 0, region: 'main' },
    order: 0,
    runtime: {
      kind: 'native_react',
      entry: 'blocks/hero/index.js',
      hint: 'native_react'
    },
    ...overrides
  };
}

describe('frontstage page canvas render plan', () => {
  test('orders explicit Block Node descriptors without reading Page Document blocks', () => {
    const metadata = createFrontstagePageDocument(
      createFrontstagePageContentFixture()
    );
    const document = {
      ...metadata,
      blocks: [
        createBlock({
          id: 'second',
          codeRef: 'second-code',
          order: 20,
          layout: { order: 20, region: 'main' }
        }),
        createBlock({
          id: 'first',
          codeRef: 'first-code',
          order: 10,
          layout: { order: 10, region: 'header' }
        }),
        createBlock({
          id: 'same-order',
          codeRef: 'same-order-code',
          order: 10,
          layout: { order: 10, region: 'footer' }
        })
      ],
      isEmpty: false
    };

    const plan = createFrontstagePageRenderPlan(document);
    expect(plan.items.map((item) => item.blockId)).toEqual([
      'first',
      'same-order',
      'second'
    ]);
    expect(plan.items.map((item) => item.order)).toEqual([10, 10, 20]);
  });

  test('selects isolated iframe without requiring Native code refs', () => {
    const item = createFrontstageBlockRenderPlanItem(
      createBlock({
        codeRef: '',
        sourceCodeRef: null,
        runtime: {
          kind: 'isolated_iframe',
          entry: '@1flowbase/isolated-chart',
          hint: 'isolated_iframe'
        }
      })
    );

    expect(item).toMatchObject({
      renderMode: 'isolated_iframe',
      canPrepareNativeReact: false,
      canMountIsolatedIframe: true,
      fallbackReasons: []
    });
  });

  test('does not mutate source blocks or share mutable plan objects', () => {
    const block = createBlock({ props: { nested: { value: 1 } } });
    const first = createFrontstageBlockRenderPlanItem(block);
    const second = createFrontstageBlockRenderPlanItem(block);

    expect(first).toEqual(second);
    expect(first).not.toBe(second);
    expect(first.props).not.toBe(block.props);
  });

  test('uses a caller supplied source index in fallback paths', () => {
    const item = createFrontstageBlockRenderPlanItem(
      createBlock({
        rendererVersion: null,
        runtime: { kind: 'native_react', entry: null, hint: 'native_react' }
      }),
      7
    );

    expect(item.fallbackReasons).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ path: 'blocks.7.renderer_version' }),
        expect.objectContaining({ path: 'blocks.7.runtime.entry' })
      ])
    );
  });
});
