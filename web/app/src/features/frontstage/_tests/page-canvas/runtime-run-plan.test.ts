import { describe, expect, test } from 'vitest';

import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type {
  FrontstagePageCanvasRuntimeSource,
  FrontstagePageCanvasRuntimeSourceState
} from '../../lib/page-canvas/runtime-source';
import { createFrontstagePageCanvasRuntimeRunPlanState } from '../../lib/page-canvas/runtime-run-plan';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import type { RestrictedBlockLoaderLimits } from '../../lib/restricted-block-loader';

function createBlock(
  overrides: Partial<FrontstageBlockInstance> = {}
): FrontstageBlockInstance {
  return {
    id: 'hero-block',
    sourceId: 'hero-block',
    codeRef: 'hero-code',
    sourceCodeRef: 'hero-code',
    catalog: {
      providerCode: 'official',
      installationId: 'installation-1'
    },
    contribution: {
      pluginId: 'official.blocks',
      pluginVersion: '1.0.0',
      code: 'hero.banner'
    },
    props: { title: 'Hello' },
    layout: { order: 10 },
    order: 10,
    runtime: {
      kind: 'iframe',
      entry: 'blocks/hero/index.js',
      hint: 'iframe'
    },
    ...overrides
  };
}

function createCatalogEntry(
  overrides: Partial<NormalizedFrontstageBlockCatalogEntry> = {}
): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: 'official:hero.banner',
    runtimeKind: 'iframe',
    installationId: 'installation-1',
    providerCode: 'official',
    pluginId: 'official.blocks',
    pluginVersion: '1.0.0',
    contributionCode: 'hero.banner',
    title: 'Hero Banner',
    entry: 'blocks/hero/index.js',
    permissions: {
      network: 'none',
      storage: 'none',
      secrets: 'none'
    },
    contextContract: {
      primitives: ['text', 'button', 'data_record'],
      inputSchema: { type: 'object' }
    },
    uiCapabilities: ['responsive', 'data_binding'],
    raw: {} as NormalizedFrontstageBlockCatalogEntry['raw'],
    ...overrides
  };
}

function createLimits(
  overrides: Partial<RestrictedBlockLoaderLimits> = {}
): RestrictedBlockLoaderLimits {
  return {
    timeoutMs: 1000,
    maxRenderDepth: 8,
    maxRenderNodes: 250,
    allowedActions: ['record.save'],
    allowedEvents: ['record.saved'],
    allowedQueries: ['records'],
    allowedDataOperations: ['query'],
    maxEventChainDepth: 4,
    ...overrides
  };
}

function createSourceState(
  sources: FrontstagePageCanvasRuntimeSource[]
): FrontstagePageCanvasRuntimeSourceState {
  return {
    workspaceId: 'workspace-1',
    pageId: 'page-1',
    sources
  };
}

function createReadySource({
  block = createBlock(),
  code = 'export default { render() {} }',
  sourceIndex = 0,
  slotIndex = 0
}: {
  block?: FrontstageBlockInstance;
  code?: string;
  sourceIndex?: number;
  slotIndex?: number;
} = {}): FrontstagePageCanvasRuntimeSource {
  const runtimeEntry = block.runtime.entry ?? '';

  return {
    ...createSourceBase(block, sourceIndex, slotIndex),
    status: 'ready',
    code,
    block,
    request: {
      requestId: [
        'frontstage-page-canvas-block-code',
        'workspace-1',
        'page-1',
        String(slotIndex),
        block.id,
        block.codeRef
      ].join(':'),
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      blockId: block.id,
      sourceBlockId: block.sourceId,
      codeRef: block.codeRef,
      sourceCodeRef: block.sourceCodeRef,
      runtimeEntry,
      runtimeKind: block.runtime.kind,
      order: block.order,
      sourceIndex,
      slotIndex,
      contributionCode: block.contribution.code
    }
  };
}

function createNotReadySource(
  status: Exclude<FrontstagePageCanvasRuntimeSource['status'], 'ready'>,
  slotIndex: number
): FrontstagePageCanvasRuntimeSource {
  const block = createBlock({
    id: `${status}-block`,
    sourceId: `${status}-block`,
    codeRef: `${status}-code`,
    sourceCodeRef: `${status}-code`,
    order: slotIndex,
    layout: { order: slotIndex }
  });
  const base = createSourceBase(block, slotIndex, slotIndex);
  const request = {
    requestId: `request-${status}`,
    workspaceId: 'workspace-1',
    pageId: 'page-1',
    blockId: block.id,
    sourceBlockId: block.sourceId,
    codeRef: block.codeRef,
    sourceCodeRef: block.sourceCodeRef,
    runtimeEntry: block.runtime.entry ?? '',
    runtimeKind: block.runtime.kind,
    order: block.order,
    sourceIndex: slotIndex,
    slotIndex,
    contributionCode: block.contribution.code
  };

  switch (status) {
    case 'loading':
      return { ...base, status, request };
    case 'missing':
      return { ...base, status, request, message: 'Block code is missing.' };
    case 'failed':
      return {
        ...base,
        status,
        request,
        error: { name: 'Error', message: 'Read failed.' }
      };
    case 'skipped':
      return {
        ...base,
        status,
        fallbackReasons: [
          {
            code: 'unsupported_runtime',
            path: `blocks.${slotIndex}.runtime.kind`,
            message: 'Runtime is unsupported.'
          }
        ]
      };
    default:
      throw new Error(`Unsupported source status: ${status}`);
  }
}

function createSourceBase(
  block: FrontstageBlockInstance,
  sourceIndex: number,
  slotIndex: number
) {
  return {
    blockId: block.id,
    sourceBlockId: block.sourceId,
    codeRef: block.codeRef,
    sourceCodeRef: block.sourceCodeRef,
    order: block.order,
    sourceIndex,
    slotIndex,
    renderMode: 'restricted_js_block' as const,
    canEnterRestrictedJsRuntime: true,
    runtimeKind: block.runtime.kind,
    runtimeEntry: block.runtime.entry,
    contributionCode: block.contribution.code
  };
}

describe('frontstage page canvas runtime run plan model', () => {
  test('builds run plans for ready sources using a stable catalog match', () => {
    const sourceState = createSourceState([
      createReadySource({
        block: createBlock({
          id: 'stable-match',
          codeRef: 'stable-code',
          sourceCodeRef: 'stable-code'
        }),
        slotIndex: 2
      })
    ]);
    const contributionOnlyDecoy = createCatalogEntry({
      id: 'decoy:hero.banner',
      providerCode: 'decoy',
      pluginId: 'decoy.blocks',
      contributionCode: 'hero.banner'
    });

    const runPlanState = createFrontstagePageCanvasRuntimeRunPlanState({
      sourceState,
      catalogEntries: [contributionOnlyDecoy, createCatalogEntry()],
      contextSnapshot: { workspaceId: 'workspace-1', pageId: 'page-1' },
      limits: createLimits()
    });

    expect(runPlanState.items).toHaveLength(1);
    expect(runPlanState.items[0]).toMatchObject({
      status: 'run_plan_ready',
      blockId: 'stable-match',
      codeRef: 'stable-code',
      slotIndex: 2,
      sourceStatus: 'ready',
      catalogId: 'official:hero.banner'
    });
    if (runPlanState.items[0].status !== 'run_plan_ready') {
      throw new Error('Expected a ready run plan.');
    }
    expect(runPlanState.items[0].runPlan.request).toMatchObject({
      requestId: 'restricted-block:stable-match:stable-code',
      blockId: 'stable-match',
      source: 'export default { render() {} }',
      props: { title: 'Hello' },
      contextSnapshot: { workspaceId: 'workspace-1', pageId: 'page-1' },
      limits: {
        timeoutMs: 1000,
        maxRenderDepth: 8,
        maxRenderNodes: 250
      }
    });
  });

  test('returns catalog_missing when no stable catalog entry matches a ready source', () => {
    const sourceState = createSourceState([
      createReadySource({
        block: createBlock({
          id: 'missing-catalog',
          contribution: {
            pluginId: 'official.blocks',
            pluginVersion: '1.0.0',
            code: 'shared.contribution'
          }
        })
      })
    ]);

    const runPlanState = createFrontstagePageCanvasRuntimeRunPlanState({
      sourceState,
      catalogEntries: [
        createCatalogEntry({
          id: 'wrong:shared.contribution',
          providerCode: 'wrong-provider',
          contributionCode: 'shared.contribution'
        })
      ],
      contextSnapshot: {},
      limits: createLimits()
    });

    expect(runPlanState.items).toEqual([
      expect.objectContaining({
        status: 'catalog_missing',
        blockId: 'missing-catalog',
        codeRef: 'hero-code',
        sourceStatus: 'ready',
        reason: expect.objectContaining({
          code: 'catalog_missing',
          path: 'catalogEntries'
        })
      })
    ]);
  });

  test('returns rejected when the restricted run plan builder rejects a ready source', () => {
    const sourceState = createSourceState([
      createReadySource({
        block: createBlock({ id: 'missing-limits' }),
        slotIndex: 1
      })
    ]);

    const runPlanState = createFrontstagePageCanvasRuntimeRunPlanState({
      sourceState,
      catalogEntries: [createCatalogEntry()],
      contextSnapshot: {}
    });

    expect(runPlanState.items).toEqual([
      expect.objectContaining({
        status: 'rejected',
        blockId: 'missing-limits',
        codeRef: 'hero-code',
        slotIndex: 1,
        sourceStatus: 'ready',
        catalogId: 'official:hero.banner',
        rejection: expect.objectContaining({
          ok: false,
          code: 'missing_limits',
          path: 'limits',
          blockId: 'missing-limits'
        })
      })
    ]);
  });

  test('marks non-ready sources as source_not_ready without resolving runtime context', () => {
    const sourceState = createSourceState([
      createNotReadySource('loading', 0),
      createNotReadySource('missing', 1),
      createNotReadySource('failed', 2),
      createNotReadySource('skipped', 3)
    ]);
    let contextCalls = 0;

    const runPlanState = createFrontstagePageCanvasRuntimeRunPlanState({
      sourceState,
      catalogEntries: [createCatalogEntry()],
      contextSnapshot: () => {
        contextCalls += 1;
        throw new Error('Non-ready sources must not resolve context.');
      }
    });

    expect(contextCalls).toBe(0);
    expect(runPlanState.items.map((item) => item.status)).toEqual([
      'source_not_ready',
      'source_not_ready',
      'source_not_ready',
      'source_not_ready'
    ]);
    expect(runPlanState.items.map((item) => item.sourceStatus)).toEqual([
      'loading',
      'missing',
      'failed',
      'skipped'
    ]);
    expect(runPlanState.items).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          status: 'source_not_ready',
          reason: expect.objectContaining({
            code: 'source_not_ready',
            path: 'sources.0.status'
          })
        })
      ])
    );
  });

  test('keeps source order stable and does not mutate caller inputs', () => {
    const catalogEntries = [createCatalogEntry()];
    const sourceState = createSourceState([
      createReadySource({
        block: createBlock({
          id: 'third-slot',
          codeRef: 'third-code',
          sourceCodeRef: 'third-code',
          props: { title: 'Third' }
        }),
        sourceIndex: 2,
        slotIndex: 2
      }),
      createReadySource({
        block: createBlock({
          id: 'first-slot',
          codeRef: 'first-code',
          sourceCodeRef: 'first-code',
          props: { title: 'First' }
        }),
        sourceIndex: 0,
        slotIndex: 0
      }),
      createReadySource({
        block: createBlock({
          id: 'second-slot',
          codeRef: 'second-code',
          sourceCodeRef: 'second-code',
          props: { title: 'Second' }
        }),
        sourceIndex: 1,
        slotIndex: 1
      })
    ]);
    const limits = createLimits();
    const contextSnapshot = { workspaceId: 'workspace-1', pageId: 'page-1' };
    const originalSourceState = structuredClone(sourceState);
    const originalCatalogEntries = structuredClone(catalogEntries);
    const originalLimits = structuredClone(limits);
    const originalContextSnapshot = structuredClone(contextSnapshot);

    const runPlanState = createFrontstagePageCanvasRuntimeRunPlanState({
      sourceState,
      catalogEntries,
      contextSnapshot,
      limits
    });

    expect(runPlanState.items.map((item) => item.slotIndex)).toEqual([
      2, 0, 1
    ]);
    expect(sourceState).toEqual(originalSourceState);
    expect(catalogEntries).toEqual(originalCatalogEntries);
    expect(limits).toEqual(originalLimits);
    expect(contextSnapshot).toEqual(originalContextSnapshot);
  });
});
