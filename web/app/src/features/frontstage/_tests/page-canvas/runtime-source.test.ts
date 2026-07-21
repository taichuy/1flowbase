/* eslint-disable testing-library/render-result-naming-convention */
import { describe, expect, test } from 'vitest';

import type { FrontstageBlockInstance } from '../../lib/page-document';
import {
  createFrontstageBlockRenderPlanItem,
  type FrontstageBlockRenderPlanItem,
  type FrontstagePageRenderPlan
} from '../../lib/page-canvas/render-plan';
import {
  createFrontstagePageCanvasBlockCodeReadPlan,
  createFrontstagePageCanvasRuntimeSourceState,
  type FrontstagePageCanvasBlockCodeReadResult
} from '../../lib/page-canvas/runtime-source';

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
    presentation: overrides.presentation ?? { heightMode: 'auto', height: null },
    layout: { order: 0, region: 'main' },
    order: 0,
    runtime: {
      kind: 'iframe',
      entry: 'blocks/hero/index.js',
      hint: 'iframe'
    },
    ...overrides,
    rendererVersion: overrides.rendererVersion ?? 'v1'
  };
}

function createSlot(
  overrides: Partial<FrontstageBlockInstance> = {},
  sourceIndex = 0
): FrontstageBlockRenderPlanItem {
  return createFrontstageBlockRenderPlanItem(
    createBlock({
      id: overrides.id ?? `block-${sourceIndex + 1}`,
      sourceId:
        overrides.sourceId ?? overrides.id ?? `block-${sourceIndex + 1}`,
      codeRef: overrides.codeRef ?? `block-${sourceIndex + 1}-code`,
      sourceCodeRef:
        overrides.sourceCodeRef ??
        overrides.codeRef ??
        `block-${sourceIndex + 1}-code`,
      order: sourceIndex,
      layout: { order: sourceIndex },
      runtime: {
        kind: 'iframe',
        entry: `blocks/block-${sourceIndex + 1}.js`,
        hint: 'iframe'
      },
      ...overrides
    }),
    sourceIndex
  );
}

function createRenderPlan(
  items: FrontstageBlockRenderPlanItem[]
): FrontstagePageRenderPlan {
  return {
    pageId: 'page-1',
    rootUid: 'root-1',
    isEmpty: items.length === 0,
    diagnostics: [],
    items
  };
}

describe('frontstage page canvas runtime source model', () => {
  test('creates block code read requests only for eligible restricted runtime slots', () => {
    const eligible = createSlot(
      {
        id: 'ready',
        codeRef: 'ready-code',
        sourceCodeRef: 'ready-code',
        runtime: {
          kind: 'iframe',
          entry: 'blocks/ready.js',
          hint: 'iframe'
        },
        order: 20,
        layout: { order: 20, region: 'main' }
      },
      0
    );
    const placeholder = createSlot(
      {
        id: 'legacy',
        codeRef: 'legacy-code',
        sourceCodeRef: 'legacy-code',
        runtime: {
          kind: 'inline',
          entry: 'blocks/legacy.js',
          hint: 'inline'
        },
        order: 10,
        layout: { order: 10, region: 'main' }
      },
      1
    );
    const missingEntry = createSlot(
      {
        id: 'missing-entry',
        codeRef: 'missing-entry-code',
        sourceCodeRef: 'missing-entry-code',
        runtime: { kind: 'iframe', entry: null, hint: 'iframe' },
        order: 30,
        layout: { order: 30, region: 'main' }
      },
      2
    );
    const unsupportedRenderer = createSlot(
      {
        id: 'future',
        rendererVersion: 'v2',
        codeRef: 'future-code',
        sourceCodeRef: 'future-code',
        runtime: {
          kind: 'iframe',
          entry: 'blocks/future.js',
          hint: 'iframe'
        },
        order: 40,
        layout: { order: 40, region: 'main' }
      },
      3
    );
    const renderPlan = createRenderPlan([
      placeholder,
      eligible,
      missingEntry,
      unsupportedRenderer
    ]);

    const readPlan = createFrontstagePageCanvasBlockCodeReadPlan({
      workspaceId: 'workspace-1',
      renderPlan
    });

    expect(readPlan).toEqual({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      requests: [
        expect.objectContaining({
          workspaceId: 'workspace-1',
          pageId: 'page-1',
          blockId: 'ready',
          codeRef: 'ready-code',
          runtimeEntry: 'blocks/ready.js',
          runtimeKind: 'iframe',
          order: 20,
          sourceIndex: 0,
          slotIndex: 1
        })
      ]
    });
    expect(readPlan.requests[0].requestId).toContain('ready');
    expect(readPlan.requests).not.toContainEqual(
      expect.objectContaining({ blockId: 'future' })
    );
  });

  test('merges read results with render slots into runtime source states', () => {
    const renderPlan = createRenderPlan([
      createSlot(
        {
          id: 'ready',
          codeRef: 'ready-code',
          sourceCodeRef: 'ready-code',
          runtime: {
            kind: 'iframe',
            entry: 'blocks/ready.js',
            hint: 'iframe'
          },
          props: { title: 'Ready' },
          order: 0,
          layout: { order: 0, region: 'main' }
        },
        0
      ),
      createSlot(
        {
          id: 'loading',
          codeRef: 'loading-code',
          sourceCodeRef: 'loading-code',
          runtime: {
            kind: 'iframe',
            entry: 'blocks/loading.js',
            hint: 'iframe'
          },
          order: 1
        },
        1
      ),
      createSlot(
        {
          id: 'missing',
          codeRef: 'missing-code',
          sourceCodeRef: 'missing-code',
          runtime: {
            kind: 'iframe',
            entry: 'blocks/missing.js',
            hint: 'iframe'
          },
          order: 2
        },
        2
      ),
      createSlot(
        {
          id: 'failed',
          codeRef: 'failed-code',
          sourceCodeRef: 'failed-code',
          runtime: {
            kind: 'iframe',
            entry: 'blocks/failed.js',
            hint: 'iframe'
          },
          order: 3
        },
        3
      ),
      createSlot(
        {
          id: 'skipped',
          codeRef: 'skipped-code',
          sourceCodeRef: 'skipped-code',
          runtime: {
            kind: 'inline',
            entry: 'blocks/skipped.js',
            hint: 'inline'
          },
          order: 4
        },
        4
      )
    ]);
    const readPlan = createFrontstagePageCanvasBlockCodeReadPlan({
      workspaceId: 'workspace-1',
      renderPlan
    });
    const codeResults: FrontstagePageCanvasBlockCodeReadResult[] = [
      {
        codeRef: 'ready-code',
        status: 'ready',
        code: 'export default { render() {} }',
        source_sha256: 'ready-source-sha256'
      },
      {
        codeRef: 'missing-code',
        status: 'missing',
        message: 'No block code exists for missing-code.'
      },
      {
        codeRef: 'failed-code',
        status: 'failed',
        error: new Error('read failed')
      }
    ];

    const sourceState = createFrontstagePageCanvasRuntimeSourceState({
      renderPlan,
      readPlan,
      codeResults
    });

    expect(sourceState.sources.map((source) => source.status)).toEqual([
      'ready',
      'loading',
      'missing',
      'failed',
      'skipped'
    ]);
    expect(sourceState.sources[0]).toMatchObject({
      status: 'ready',
      blockId: 'ready',
      codeRef: 'ready-code',
      runtimeEntry: 'blocks/ready.js',
      code: 'export default { render() {} }',
      source_sha256: 'ready-source-sha256',
      block: {
        id: 'ready',
        codeRef: 'ready-code',
        runtime: {
          kind: 'iframe',
          entry: 'blocks/ready.js',
          hint: 'iframe'
        },
        props: { title: 'Ready' }
      }
    });
    expect(sourceState.sources[1]).toMatchObject({
      status: 'loading',
      blockId: 'loading',
      codeRef: 'loading-code'
    });
    expect(sourceState.sources[2]).toMatchObject({
      status: 'missing',
      blockId: 'missing',
      codeRef: 'missing-code',
      message: 'No block code exists for missing-code.'
    });
    expect(sourceState.sources[3]).toMatchObject({
      status: 'failed',
      blockId: 'failed',
      codeRef: 'failed-code',
      error: {
        name: 'Error',
        message: 'read failed'
      }
    });
    expect(sourceState.sources[4]).toMatchObject({
      status: 'skipped',
      blockId: 'skipped',
      codeRef: 'skipped-code',
      fallbackReasons: [
        expect.objectContaining({
          code: 'unsupported_runtime',
          path: 'blocks.4.runtime.kind'
        })
      ]
    });
  });

  test('keeps duplicate codeRef slots ordered and applies the last read result consistently', () => {
    const renderPlan = createRenderPlan([
      createSlot(
        {
          id: 'shared-a',
          codeRef: 'shared-code',
          sourceCodeRef: 'shared-code',
          runtime: {
            kind: 'iframe',
            entry: 'blocks/shared-a.js',
            hint: 'iframe'
          },
          order: 20
        },
        0
      ),
      createSlot(
        {
          id: 'shared-b',
          codeRef: 'shared-code',
          sourceCodeRef: 'shared-code',
          runtime: {
            kind: 'iframe',
            entry: 'blocks/shared-b.js',
            hint: 'iframe'
          },
          order: 20
        },
        1
      )
    ]);
    const readPlan = createFrontstagePageCanvasBlockCodeReadPlan({
      workspaceId: 'workspace-1',
      renderPlan
    });

    const sourceState = createFrontstagePageCanvasRuntimeSourceState({
      renderPlan,
      readPlan,
      codeResults: [
        {
          codeRef: 'shared-code',
          status: 'failed',
          error: 'stale failure'
        },
        {
          codeRef: 'shared-code',
          status: 'ready',
          code: 'export default "latest";',
          source_sha256: 'latest-source-sha256'
        }
      ]
    });

    expect(readPlan.requests.map((request) => request.blockId)).toEqual([
      'shared-a',
      'shared-b'
    ]);
    expect(readPlan.requests.map((request) => request.slotIndex)).toEqual([
      0, 1
    ]);
    expect(sourceState.sources).toEqual([
      expect.objectContaining({
        status: 'ready',
        blockId: 'shared-a',
        codeRef: 'shared-code',
        code: 'export default "latest";',
        source_sha256: 'latest-source-sha256'
      }),
      expect.objectContaining({
        status: 'ready',
        blockId: 'shared-b',
        codeRef: 'shared-code',
        code: 'export default "latest";',
        source_sha256: 'latest-source-sha256'
      })
    ]);
  });

  test('does not mutate render plans or expose mutable plan objects through source state', () => {
    const renderPlan = createRenderPlan([
      createSlot(
        {
          id: 'ready',
          codeRef: 'ready-code',
          sourceCodeRef: 'ready-code',
          props: { nested: { value: 'before' } },
          layout: { order: 0, nested: { width: 12 } },
          runtime: {
            kind: 'iframe',
            entry: 'blocks/ready.js',
            hint: 'iframe'
          }
        },
        0
      ),
      createSlot(
        {
          id: 'skipped',
          codeRef: 'skipped-code',
          sourceCodeRef: 'skipped-code',
          runtime: {
            kind: 'inline',
            entry: 'blocks/skipped.js',
            hint: 'inline'
          }
        },
        1
      )
    ]);
    const originalRenderPlan = structuredClone(renderPlan);
    const readPlan = createFrontstagePageCanvasBlockCodeReadPlan({
      workspaceId: 'workspace-1',
      renderPlan
    });

    const sourceState = createFrontstagePageCanvasRuntimeSourceState({
      renderPlan,
      readPlan,
      codeResults: [
        {
          codeRef: 'ready-code',
          status: 'ready',
          code: 'export default {}',
          source_sha256: 'ready-source-sha256'
        }
      ]
    });

    expect(renderPlan).toEqual(originalRenderPlan);
    expect(sourceState.sources[0]).toMatchObject({ status: 'ready' });
    if (sourceState.sources[0].status === 'ready') {
      const props = sourceState.sources[0].block.props as {
        nested: { value: string };
      };
      const layout = sourceState.sources[0].block.layout as unknown as {
        nested: { width: number };
      };
      props.nested.value = 'after';
      layout.nested.width = 24;
    }
    expect(sourceState.sources[1]).toMatchObject({ status: 'skipped' });
    if (sourceState.sources[1].status === 'skipped') {
      sourceState.sources[1].fallbackReasons[0].message = 'changed';
    }

    expect(renderPlan).toEqual(originalRenderPlan);
  });
});
