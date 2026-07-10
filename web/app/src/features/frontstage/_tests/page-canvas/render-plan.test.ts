/* eslint-disable testing-library/render-result-naming-convention */

import { describe, expect, test } from 'vitest';

import type { FrontstagePageContent } from '../../api/page-content';
import {
  createFrontstagePageDocument,
  type FrontstageBlockInstance
} from '../../lib/page-document';
import {
  createFrontstageBlockRenderPlanItem,
  createFrontstagePageRenderPlan
} from '../../lib/page-canvas/render-plan';

function createPageContent(
  overrides: Partial<FrontstagePageContent> = {}
): FrontstagePageContent {
  return {
    page: {
      id: 'page-1',
      title: 'Landing',
      kind: 'page',
      parentId: null,
      rank: '001000',
    },
    schema: {
      rootUid: 'root-1',
      payload: {}
    },
    root: {
      uid: 'root-1',
      payload: {}
    },
    ...overrides
  };
}

function createBlock(
  overrides: Partial<FrontstageBlockInstance> = {}
): FrontstageBlockInstance {
  return {
    id: 'hero',
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
    layout: { order: 0, region: 'main' },
    order: 0,
    runtime: {
      kind: 'iframe',
      entry: 'blocks/hero/index.js',
      hint: 'iframe'
    },
    ...overrides
  };
}

describe('frontstage page canvas render plan', () => {
  test('creates stable restricted JS block items ordered by block order', () => {
    const document = createFrontstagePageDocument(
      createPageContent({
        root: {
          uid: 'root-1',
          payload: {
            blocks: [
              {
                id: 'second',
                codeRef: 'second-code',
                contributionCode: 'official.second',
                props: { label: 'Second' },
                runtime: { kind: 'iframe', entry: 'blocks/second.js' },
                layout: { order: 20, region: 'main' }
              },
              {
                id: 'first',
                codeRef: 'first-code',
                contributionCode: 'official.first',
                props: { label: 'First' },
                runtime: { kind: 'iframe', entry: 'blocks/first.js' },
                layout: { order: 10, region: 'header' }
              },
              {
                id: 'same-order',
                codeRef: 'same-order-code',
                contributionCode: 'official.same',
                runtime: { kind: 'iframe', entry: 'blocks/same.js' },
                layout: { order: 10, region: 'footer' }
              }
            ]
          }
        }
      })
    );

    const plan = createFrontstagePageRenderPlan(document);

    expect(plan).toMatchObject({
      pageId: 'page-1',
      rootUid: 'root-1',
      isEmpty: false
    });
    expect(plan.items.map((item) => item.blockId)).toEqual([
      'first',
      'same-order',
      'second'
    ]);
    expect(plan.items.map((item) => item.order)).toEqual([10, 10, 20]);
    expect(plan.items[0]).toMatchObject({
      blockId: 'first',
      codeRef: 'first-code',
      renderMode: 'restricted_js_block',
      canEnterRestrictedJsRuntime: true,
      fallbackReasons: [],
      runtime: {
        kind: 'iframe',
        entry: 'blocks/first.js',
        hint: 'iframe'
      },
      contribution: {
        code: 'official.first'
      },
      layout: {
        order: 10,
        region: 'header'
      },
      props: {
        label: 'First'
      }
    });
  });

  test('marks unsupported and unknown runtime blocks as placeholders', () => {
    const document = createFrontstagePageDocument(
      createPageContent({
        root: {
          uid: 'root-1',
          payload: {
            blocks: [
              {
                id: 'legacy',
                codeRef: 'legacy-code',
                contributionCode: 'official.legacy',
                runtime: { kind: 'inline', entry: 'legacy.js' },
                layout: { order: 0 }
              },
              {
                id: 'unknown',
                codeRef: 'unknown-code',
                contributionCode: 'official.unknown',
                runtime: { kind: 'unknown', entry: 'unknown.js' },
                layout: { order: 1 }
              }
            ]
          }
        }
      })
    );

    const plan = createFrontstagePageRenderPlan(document);

    expect(plan.items).toEqual([
      expect.objectContaining({
        blockId: 'legacy',
        renderMode: 'placeholder',
        canEnterRestrictedJsRuntime: false,
        fallbackReasons: [
          expect.objectContaining({
            code: 'unsupported_runtime',
            path: 'blocks.0.runtime.kind'
          })
        ]
      }),
      expect.objectContaining({
        blockId: 'unknown',
        renderMode: 'placeholder',
        canEnterRestrictedJsRuntime: false,
        fallbackReasons: [
          expect.objectContaining({
            code: 'unknown_runtime',
            path: 'blocks.1.runtime.kind'
          })
        ]
      })
    ]);
  });

  test('keeps normalized fallback fields stable while reporting missing codeRef and entry', () => {
    const document = createFrontstagePageDocument(
      createPageContent({
        root: {
          uid: 'root-1',
          payload: {
            blocks: [
              {
                contributionCode: 'official.missing',
                runtime: { kind: 'iframe' },
                props: 'invalid-props'
              },
              {
                id: 'explicit',
                codeRef: 'explicit-code',
                contributionCode: 'official.explicit',
                runtime: { kind: 'iframe' }
              }
            ]
          }
        }
      })
    );

    const firstPlan = createFrontstagePageRenderPlan(document);
    const secondPlan = createFrontstagePageRenderPlan(document);

    expect(firstPlan).toEqual(secondPlan);
    expect(firstPlan.items).toEqual([
      expect.objectContaining({
        blockId: 'block-1',
        codeRef: 'block-1-code',
        sourceBlockId: null,
        sourceCodeRef: null,
        props: {},
        renderMode: 'placeholder',
        canEnterRestrictedJsRuntime: false,
        fallbackReasons: [
          expect.objectContaining({
            code: 'missing_code_ref',
            path: 'blocks.0.codeRef'
          }),
          expect.objectContaining({
            code: 'missing_runtime_entry',
            path: 'blocks.0.runtime.entry'
          })
        ]
      }),
      expect.objectContaining({
        blockId: 'explicit',
        codeRef: 'explicit-code',
        sourceBlockId: 'explicit',
        sourceCodeRef: 'explicit-code',
        renderMode: 'placeholder',
        canEnterRestrictedJsRuntime: false,
        fallbackReasons: [
          expect.objectContaining({
            code: 'missing_runtime_entry',
            path: 'blocks.1.runtime.entry'
          })
        ]
      })
    ]);
  });

  test('does not mutate source documents or share mutable plan objects', () => {
    const block = createBlock({
      props: { nested: { value: 'before' } },
      layout: { order: 0, nested: { width: 12 } }
    });
    const document = {
      ...createFrontstagePageDocument(createPageContent()),
      blocks: [block],
      isEmpty: false
    };
    const originalDocument = structuredClone(document);

    const plan = createFrontstagePageRenderPlan(document);

    expect(document).toEqual(originalDocument);
    expect(plan.items[0].props).toEqual(block.props);
    expect(plan.items[0].props).not.toBe(block.props);
    expect(plan.items[0].layout).toEqual(block.layout);
    expect(plan.items[0].layout).not.toBe(block.layout);

    const planProps = plan.items[0].props as { nested: { value: string } };
    const planLayout = plan.items[0].layout as unknown as {
      nested: { width: number };
    };
    planProps.nested.value = 'after';
    planLayout.nested.width = 24;

    expect(block.props).toEqual({ nested: { value: 'before' } });
    expect(block.layout).toEqual({ order: 0, nested: { width: 12 } });
  });

  test('builds a block-level item with caller supplied source index', () => {
    const item = createFrontstageBlockRenderPlanItem(
      createBlock({
        id: 'metric',
        codeRef: 'metric-code',
        runtime: {
          kind: 'iframe',
          entry: 'blocks/metric.js',
          hint: 'iframe'
        }
      }),
      7
    );

    expect(item).toMatchObject({
      blockId: 'metric',
      sourceIndex: 7,
      renderMode: 'restricted_js_block',
      canEnterRestrictedJsRuntime: true,
      fallbackReasons: []
    });
  });
});
