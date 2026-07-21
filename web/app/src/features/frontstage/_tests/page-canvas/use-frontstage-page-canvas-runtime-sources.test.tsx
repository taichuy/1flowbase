/* eslint-disable testing-library/render-result-naming-convention */

import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type { FrontstageBlockInstance } from '../../lib/page-document';
import {
  createFrontstageBlockRenderPlanItem,
  type FrontstageBlockRenderPlanItem,
  type FrontstagePageRenderPlan
} from '../../lib/page-canvas/render-plan';
import { useFrontstagePageCanvasRuntimeSources } from '../../hooks/use-frontstage-page-canvas-runtime-sources';
import type { FrontstageRuntimeDemandByBlockId } from '../../lib/page-canvas/runtime-demand';

const frontstageBlockCodeApi = vi.hoisted(() => ({
  fetchFrontstageBlockCode: vi.fn(),
  frontstageBlockCodeQueryKey: vi.fn(
    (workspaceId: string, pageId: string, codeRef: string) =>
      [
        'frontstage',
        workspaceId,
        'pages',
        pageId,
        'block-code',
        codeRef
      ] as const
  )
}));

vi.mock('../../api/block-code', () => frontstageBlockCodeApi);

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

function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false }
    }
  });
}

function setupRuntimeSources({
  workspaceId = 'workspace-1',
  renderPlan = createRenderPlan([]),
  demandsByBlockId
}: {
  workspaceId?: string | null;
  renderPlan?: FrontstagePageRenderPlan | null;
  demandsByBlockId?: FrontstageRuntimeDemandByBlockId;
}) {
  const queryClient = createQueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return renderHook(
    () =>
      useFrontstagePageCanvasRuntimeSources({
        workspaceId,
        renderPlan,
        demandsByBlockId
      }),
    { wrapper }
  );
}

describe('useFrontstagePageCanvasRuntimeSources', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  test('queries only eligible render plan slots with the block code query key', async () => {
    const renderPlan = createRenderPlan([
      createSlot(
        {
          id: 'hero',
          codeRef: 'hero-code',
          sourceCodeRef: 'hero-code',
          runtime: {
            kind: 'iframe',
            entry: 'blocks/hero.js',
            hint: 'iframe'
          },
          order: 20
        },
        0
      ),
      createSlot(
        {
          id: 'legacy',
          codeRef: 'legacy-code',
          sourceCodeRef: 'legacy-code',
          runtime: {
            kind: 'inline',
            entry: 'blocks/legacy.js',
            hint: 'inline'
          },
          order: 10
        },
        1
      )
    ]);
    frontstageBlockCodeApi.fetchFrontstageBlockCode.mockResolvedValue({
      pageId: 'page-1',
      codeRef: 'hero-code',
      code: 'export default "hero";',
      source_sha256: 'hero-source-sha256'
    });

    const { result } = setupRuntimeSources({ renderPlan });

    await waitFor(() => {
      expect(result.current.sourceState?.sources[0]).toMatchObject({
        status: 'ready',
        blockId: 'hero',
        codeRef: 'hero-code',
        code: 'export default "hero";',
        source_sha256: 'hero-source-sha256'
      });
    });

    expect(result.current.readPlan?.requests).toHaveLength(1);
    expect(frontstageBlockCodeApi.frontstageBlockCodeQueryKey).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'hero-code'
    );
    expect(frontstageBlockCodeApi.fetchFrontstageBlockCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'hero-code'
    );
    expect(result.current.sourceState?.sources[1]).toMatchObject({
      status: 'skipped',
      blockId: 'legacy',
      codeRef: 'legacy-code'
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.hasError).toBe(false);
    expect(result.current.errors).toEqual([]);
  });

  test('maps loading, failed, and empty successful code responses into source states', async () => {
    const pending = new Promise(() => {});
    const readError = new Error('read failed');
    const renderPlan = createRenderPlan([
      createSlot(
        {
          id: 'loading',
          codeRef: 'loading-code',
          sourceCodeRef: 'loading-code'
        },
        0
      ),
      createSlot(
        {
          id: 'missing',
          codeRef: 'missing-code',
          sourceCodeRef: 'missing-code'
        },
        1
      ),
      createSlot(
        {
          id: 'failed',
          codeRef: 'failed-code',
          sourceCodeRef: 'failed-code'
        },
        2
      )
    ]);
    frontstageBlockCodeApi.fetchFrontstageBlockCode.mockImplementation(
      (_workspaceId: string, _pageId: string, codeRef: string) => {
        if (codeRef === 'loading-code') {
          return pending;
        }

        if (codeRef === 'missing-code') {
          return Promise.resolve({
            pageId: 'page-1',
            codeRef,
            code: ''
          });
        }

        return Promise.reject(readError);
      }
    );

    const { result } = setupRuntimeSources({ renderPlan });

    await waitFor(() => {
      expect(result.current.sourceState?.sources.map((source) => source.status))
        .toEqual(['loading', 'missing', 'failed']);
    });

    expect(result.current.sourceState?.sources[1]).toMatchObject({
      status: 'missing',
      blockId: 'missing',
      codeRef: 'missing-code',
      message: 'Block code is empty for missing-code.'
    });
    expect(result.current.sourceState?.sources[2]).toMatchObject({
      status: 'failed',
      blockId: 'failed',
      codeRef: 'failed-code',
      error: {
        name: 'Error',
        message: 'read failed'
      }
    });
    expect(result.current.loading).toBe(true);
    expect(result.current.hasError).toBe(true);
    expect(result.current.errors).toEqual([readError]);
  });

  test('does not query when workspace id or render plan is missing', async () => {
    const missingWorkspace = setupRuntimeSources({
      workspaceId: null,
      renderPlan: createRenderPlan([
        createSlot(
          {
            id: 'hero',
            codeRef: 'hero-code',
            sourceCodeRef: 'hero-code'
          },
          0
        )
      ])
    });
    const missingRenderPlan = setupRuntimeSources({
      renderPlan: null
    });

    await waitFor(() => {
      expect(missingWorkspace.result.current.loading).toBe(false);
      expect(missingRenderPlan.result.current.loading).toBe(false);
    });

    expect(frontstageBlockCodeApi.fetchFrontstageBlockCode).not.toHaveBeenCalled();
    expect(missingWorkspace.result.current.readPlan).toBeNull();
    expect(missingWorkspace.result.current.sourceState).toBeNull();
    expect(missingRenderPlan.result.current.readPlan).toBeNull();
    expect(missingRenderPlan.result.current.sourceState).toBeNull();
  });

  test('keeps dormant blocks out of the network until viewport demand arrives', async () => {
    const renderPlan = createRenderPlan([
      createSlot({ id: 'hero', codeRef: 'hero-code', sourceCodeRef: 'hero-code' }, 0),
      createSlot({ id: 'below', codeRef: 'below-code', sourceCodeRef: 'below-code' }, 1)
    ]);
    frontstageBlockCodeApi.fetchFrontstageBlockCode.mockResolvedValue({
      pageId: 'page-1',
      codeRef: 'hero-code',
      code: 'export default "ready";',
      source_sha256: 'ready-source-sha256'
    });

    const { result } = setupRuntimeSources({
      renderPlan,
      demandsByBlockId: { hero: 1, below: 3 }
    });

    await waitFor(() => expect(result.current.sourceState?.sources[0]?.status).toBe('ready'));
    expect(result.current.sourceState?.sources[1]?.status).toBe('dormant');
    expect(frontstageBlockCodeApi.fetchFrontstageBlockCode).toHaveBeenCalledTimes(1);
    expect(frontstageBlockCodeApi.fetchFrontstageBlockCode).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'hero-code'
    );
  });
});
